//! Tool Belt for Horizon: a belt item that stores tools in its own slots.
//!
//! The rules — the belt's 2..=9 slots, which stacks it takes, where an insert
//! goes, what an extract takes — live in [`runtime`], which knows nothing about
//! the game and is byte-identical to the retired `.hnmod` slice. This file is the
//! glue: the belt's two item components, the belt's own item part (a right click
//! inserts the off hand, a sneak right click extracts, anything else opens the
//! menu), the menu's tick-time refresh, the click that gives a stack back, and
//! `/toolbelt put|take|info|menu|help`. `notes.md` records how each retired
//! helper maps onto this file.

pub mod runtime;

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, MutexGuard};

use mod_api::{
    self, AnvilUpdateEvent, Command, CommandArgumentValue, CommandInvocation, CommandSource,
    Component, ComponentTypesEvent, ENTITY_ITEMS, Flow, Hand, Hooks, HostSide, HostTransaction,
    InteractionResult, ItemPart, ItemPlace, ItemResource, ItemStack, MenuClickedEvent,
    MenuClosedEvent, MenuReplace, PlayerRef, RegisterCommandsEvent, Registrar, RegistrationContext,
    RightClickBlockEvent, RunModCommandEvent, ServerHost, ServerTickEvent, ServerTickPhase, Tag,
    Text, UseItemEvent,
};

use crate::runtime::{
    BELT_ITEM, BUNDLE_ITEM, BeltStack, DEFAULT_SIZE, Insertion, POUCH_ITEM, Policy, allows,
    clamp_size, find_insertion, is_upgrade_material, slot_count, upgrade_cost, upgraded_size,
};

/// The two custom component ids, registered before content loads.
const BELT_SIZE: &str = "toolbelt:belt_size";
const BELT_CONTENTS: &str = "toolbelt:belt_contents";

/// The key of one stored slot's slot number.
const KEY_SLOT: &str = "slot";

/// The component a container item declares (a shulker box's contents) and the one a bundle
/// declares. Either on a stack — in its defaults or its own patch — makes it container-bearing.
const CONTAINER_COMPONENT: &str = "minecraft:container";
const BUNDLE_CONTENTS_COMPONENT: &str = "minecraft:bundle_contents";

/// The menu title, how many rows a belt menu needs (one row holds all nine
/// belt slots) and how many slots that is.
const TITLE: &str = "Tool Belt";
const MENU_ROWS: u8 = 1;
const MENU_SLOTS: usize = 9;

/// A player's main inventory in `ENTITY_ITEMS`' numbering (0..9 the hotbar,
/// 9..36 the rest) and their off hand; 36..40 are the armour, which the belt
/// never touches.
const MAIN_SLOTS: std::ops::Range<usize> = 0..36;
const OFF_HAND_ITEM: usize = 40;

/// The command root, its help text, and the count `/toolbelt take` takes when
/// the line names none.
const ROOT: &str = "toolbelt";
const TAKE_DEFAULT: u32 = 64;

const HELP: [&str; 5] = [
    "/toolbelt put <item> <count>   insert from your inventory",
    "/toolbelt take <slot> <count>  move a belt slot to your inventory",
    "/toolbelt info                 size and contents of the held belt",
    "/toolbelt menu                 open the held belt's menu",
    "/toolbelt help                 this text",
];

/// `toolbelt:belt_size`, an int: the number of slots the belt has. A missing
/// component reads as two, and every read clamps to 2..=9.
struct SizeComponent;

impl Component for SizeComponent {
    type Value = i32;

    fn id(&self) -> &str {
        BELT_SIZE
    }

    fn decode(&self, value: &Tag) -> Option<Self::Value> {
        match value {
            Tag::Int(size) => Some(*size),
            _ => None,
        }
    }

    fn encode(&self, value: Self::Value) -> Tag {
        Tag::Int(value)
    }
}

/// `toolbelt:belt_contents`, one compound per filled slot (`{slot, id, count,
/// components?}` — an item escape beside its position, so a slot's place
/// survives). The shape is the retired slice's, so a belt saved before the
/// migration keeps everything it held.
struct ContentsComponent;

impl Component for ContentsComponent {
    type Value = Vec<Tag>;

    fn id(&self) -> &str {
        BELT_CONTENTS
    }

    fn decode(&self, value: &Tag) -> Option<Self::Value> {
        match value {
            Tag::List(entries) => Some(entries.clone()),
            _ => None,
        }
    }

    fn encode(&self, value: Self::Value) -> Tag {
        Tag::List(value)
    }
}

/// The belt menu a player has open: its window, the inventory slot the belt was
/// in when it opened, and the belt's size then.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct OpenBelt {
    window: i32,
    slot: usize,
    size: u8,
}

/// What a session remembers: the menu a right click queued for the next server
/// tick, the belt menus now open, and the click guard that makes one physical
/// right click count once (its block path and its air path both fire).
#[derive(Default)]
struct State {
    pending: BTreeMap<u128, usize>,
    menus: BTreeMap<u128, OpenBelt>,
    /// Ticks left before another right click is handled for a player.
    guard: BTreeMap<u128, u8>,
}

impl State {
    /// Whether this player's last handled click is still the physical one in
    /// progress, so this path must not handle it again.
    fn guarded(&self, uuid: u128) -> bool {
        self.guard.get(&uuid).is_some_and(|ticks| *ticks > 0)
    }

    /// Remembers that a click was just handled: for the next two ticks this
    /// player's other click path is the same physical click.
    fn guard(&mut self, uuid: u128) {
        self.guard.insert(uuid, 2);
    }

    /// One server tick passes: every guard ages and a spent one is dropped, so
    /// the map does not keep an entry per player seen.
    fn tick(&mut self) {
        self.guard.retain(|_, ticks| {
            *ticks = ticks.saturating_sub(1);
            *ticks > 0
        });
    }

    /// Queues the belt menu a click asked for; the last click in a tick wins.
    fn queue_open(&mut self, uuid: u128, slot: usize) {
        self.pending.insert(uuid, slot);
    }

    /// Takes the queued opens out, leaving none behind.
    fn take_pending(&mut self) -> BTreeMap<u128, usize> {
        std::mem::take(&mut self.pending)
    }

    /// Records the belt menu `uuid` now has open.
    fn open(&mut self, uuid: u128, belt: OpenBelt) {
        self.menus.insert(uuid, belt);
    }

    /// The belt menu `uuid` has open, if any.
    fn open_of(&self, uuid: u128) -> Option<OpenBelt> {
        self.menus.get(&uuid).copied()
    }

    /// Forgets the belt menu `uuid` had in `window`; a close for another window
    /// leaves this one alone.
    fn forget_open(&mut self, uuid: u128, window: i32) {
        if self.open_of(uuid).is_some_and(|open| open.window == window) {
            self.menus.remove(&uuid);
        }
    }

    /// The open menus, as the tick's work list.
    fn open_menus(&self) -> Vec<(u128, OpenBelt)> {
        self.menus
            .iter()
            .map(|(uuid, open)| (*uuid, *open))
            .collect()
    }
}

/// The session state behind one lock. Every critical section is a few map
/// operations and no host call, so a hook that re-enters the mod cannot
/// deadlock, and a poisoned lock is recovered rather than taken as fatal.
fn locked(state: &Mutex<State>) -> MutexGuard<'_, State> {
    state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The mod: one `State` shared with the belt's item part, which the `register`
/// hook attaches.
#[derive(Default)]
struct Mod {
    state: Arc<Mutex<State>>,
}

#[mod_api::horizon_mod]
impl Hooks for Mod {
    /// Once, at startup: attach the belt's part to the belt item, so the item's
    /// own right clicks reach it. The belt is content this package carries, so
    /// its key is requested here and validated after every declaration loads.
    fn register(&self, registrar: &mut dyn Registrar, _context: &RegistrationContext) {
        let belt = registrar.item_key(BELT_ITEM);
        registrar.item_part(
            belt,
            Belt {
                state: Arc::clone(&self.state),
            },
        );
        // The pouch is the anvil's upgrade material: plain content this package
        // carries, so its key is requested here and validated with the belt's.
        registrar.item_key(POUCH_ITEM);
    }

    /// A belt in an anvil's left slot with a pouch in the right one becomes a
    /// belt with one more slot, for the jar's level cost (`ToolBelt.anvilChange`):
    /// 3 levels for the first step up to 30 for the last, one pouch consumed. The
    /// upgraded stack is the input stack with its `belt_size` raised, so every
    /// stored slot keeps its place and contents. A nine-slot belt has no result.
    /// Both sides answer because the event fires on both: the server's menu and
    /// the client's copy of it must agree on the output.
    fn anvil_update(&self, host: HostSide, event: &mut AnvilUpdateEvent<'_>) -> Flow {
        match host {
            HostSide::Server(server) => anvil_upgrade(&*server, event),
            HostSide::Client(client) => anvil_upgrade(&*client, event),
        }
    }

    /// A right click on a block with the belt in the main hand: the belt takes
    /// the click before the block's own use, so a chest does not open and the
    /// item's own use does not run. The click is cancelled as used, with the
    /// swing vanilla's `useOn` answers with.
    fn right_click_block(&self, host: HostSide, event: &mut RightClickBlockEvent<'_>) -> Flow {
        let HostSide::Server(host) = host else {
            return Flow::Continue;
        };
        let Some(stack) = event.click.stack else {
            return Flow::Continue;
        };
        if !matches!(event.click.hand, Hand::Main) {
            return Flow::Continue;
        }
        let hand = event.click.hand;
        let uuid = event.click.player;
        match clicked(host, &self.state, stack, hand, uuid) {
            Some(_) => {
                *event.cancellation_result = InteractionResult::Success;
                Flow::Cancel
            }
            None => Flow::Continue,
        }
    }

    /// Before content loads: register the belt's two component ids, so a saved
    /// belt's `toolbelt:belt_size`/`belt_contents` are known, networked
    /// components.
    fn component_types(&self, _context: &RegistrationContext, event: &mut ComponentTypesEvent<'_>) {
        for id in [BELT_SIZE, BELT_CONTENTS] {
            if !event.types.iter().any(|known| known == id) {
                event.types.push(id.to_owned());
            }
        }
    }

    /// At the end of a running server tick: open the belt menus clicks queued (a
    /// menu cannot be opened inside a use hook) and refresh the open ones from
    /// their belt item.
    fn server_tick(&self, host: &mut dyn ServerHost, event: &mut ServerTickEvent<'_>) {
        let world_running = match event.phase {
            ServerTickPhase::ServerTickEnd { world_running } => world_running,
            // A frozen step advances nothing; neither opens nor refreshes.
            _ => return,
        };
        if !world_running {
            return;
        }
        let (pending, open) = {
            let mut state = locked(&self.state);
            state.tick();
            (state.take_pending(), state.open_menus())
        };
        for (uuid, slot) in pending {
            let Some(player) = host.player(uuid) else {
                continue;
            };
            open_menu(host, &self.state, player, slot);
        }
        for (uuid, belt) in open {
            refresh_menu(host, &self.state, uuid, belt);
        }
    }

    /// A click in an open belt menu: that belt slot's stack goes to the player,
    /// and only what the player's inventory actually took leaves the belt.
    fn menu_clicked(&self, host: &mut dyn ServerHost, event: &mut MenuClickedEvent) {
        let Some(open) = locked(&self.state).open_of(event.uuid) else {
            return;
        };
        if open.window != event.window {
            return;
        }
        let Some(index) = belt_slot(event.slot, open.size) else {
            return;
        };
        let Ok(belt_slot) = i32::try_from(open.slot) else {
            return;
        };
        let Some(player) = host.player(event.uuid) else {
            return;
        };
        let Some(belt) = inventory_stack(host, player, open.slot) else {
            return;
        };
        if !is_belt(&*host, &belt) {
            return;
        }
        let mut slots = read_slots(&*host, &belt, open.size);
        let Some(offered) = slots.get(index).cloned().flatten() else {
            return;
        };
        let placed = give_to_player(host, player, &offered);
        if placed == 0 {
            return;
        }
        take_slot(&mut slots, index, placed);
        let place = ItemPlace::PlayerSlot {
            player: player.uuid(),
            slot: belt_slot,
        };
        let written = with_contents(&*host, &belt, open.size, &slots)
            .is_some_and(|built| write_at(host, &place, built));
        if !written {
            // The game refused the belt write, so the player has the tool and
            // the belt still holds it: take it back rather than duplicate it.
            take_from_player(host, player, &offered, placed);
            return;
        }
        refresh_menu(host, &self.state, event.uuid, open);
    }

    /// The belt menu closed (or another menu replaced it): forget its state.
    fn menu_closed(&self, _host: &mut dyn ServerHost, event: &mut MenuClosedEvent) {
        locked(&self.state).forget_open(event.uuid, event.window);
    }

    /// Declares `/toolbelt put|take|info|menu|help`, at game-master level as the
    /// retired slice declared them.
    fn register_commands(&self, _host: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        for usage in [
            format!("{ROOT} put <item:item> [count:integer]"),
            format!("{ROOT} take <slot:integer> [count:integer]"),
            format!("{ROOT} info"),
            format!("{ROOT} menu"),
            format!("{ROOT} help"),
        ] {
            event.commands.push(Command::op(&usage, 2));
        }
    }

    /// Runs a `/toolbelt` usage for the player who typed it. The dispatcher has
    /// already parsed and resolved the arguments, so this reads them typed
    /// instead of re-parsing a line.
    fn run_mod_command(
        &self,
        host: &mut dyn ServerHost,
        event: &mut RunModCommandEvent<'_>,
    ) -> Flow {
        let CommandSource::Player { player, .. } = *event.source else {
            return Flow::Continue;
        };
        let Some(action) = action_of(event.command, &*host) else {
            return Flow::Continue;
        };
        let lines = match action {
            Action::Help => HELP.iter().map(|line| (*line).to_owned()).collect(),
            Action::Info => vec![info_line(host, player)],
            Action::Menu => {
                let slot = held_slot(host, player);
                let opened = open_menu(host, &self.state, player, slot);
                vec![
                    if opened {
                        "Tool Belt menu opened."
                    } else {
                        "Hold the belt first."
                    }
                    .to_owned(),
                ]
            }
            Action::Put { item, count } => vec![put(host, player, &item, count)],
            Action::Take { slot, count } => vec![take(host, player, slot, count)],
        };
        for line in lines {
            let _ = player.tell(host, Text::literal(line));
        }
        Flow::Cancel
    }
}

/// The belt's own item part: what a right click with the belt does.
struct Belt {
    state: Arc<Mutex<State>>,
}

#[mod_api::horizon_part]
impl ItemPart for Belt {
    /// A right click in the air with the belt: insert the off hand's stack,
    /// sneak to extract, else queue the menu.
    fn use_item(&self, host: HostSide, event: &mut UseItemEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else {
            return None;
        };
        clicked(host, &self.state, event.stack, event.hand, event.player)
    }
}

/// A right click with the belt in the main hand. A consuming answer means the
/// belt took the click — the block is not also used and the item's own use does
/// not run — and `None` leaves the click to the game.
fn clicked(
    host: &mut dyn ServerHost,
    state: &Mutex<State>,
    stack: &ItemStack,
    hand: Hand,
    uuid: u128,
) -> Option<InteractionResult> {
    if !matches!(hand, Hand::Main) || !is_belt(&*host, stack) {
        return None;
    }
    let player = host.player(uuid)?;
    if locked(state).guarded(uuid) {
        return None;
    }
    if sneaking(host, player) {
        extract_into_inventory(host, player, stack, None);
    } else if let Some(offhand) = hand_item(host, player, Hand::Off).filter(|s| allowed(&*host, s))
    {
        // The belt keeps what it accepts and the off hand keeps the rest. Both
        // writes are the game's own equip writes, so the client is told as it is
        // for any equip; a refused belt write puts the off hand back.
        let mut belt = stack.clone();
        let leftover = insert_into(&*host, &mut belt, offhand.clone());
        if set_hand(host, player, Hand::Off, leftover)
            && !set_hand(host, player, Hand::Main, Some(belt))
        {
            set_hand(host, player, Hand::Off, Some(offhand));
        }
    } else {
        let slot = held_slot(host, player);
        locked(state).queue_open(uuid, slot);
    }
    locked(state).guard(uuid);
    Some(InteractionResult::Consume)
}

// ---------------------------------------------------------------- components

/// Whether `stack` is the belt.
fn is_belt(host: &dyn mod_api::Host, stack: &ItemStack) -> bool {
    stack.item().id(host) == BELT_ITEM
}

/// An item's own stack limit, with a refused read reading as vanilla's 64.
fn max_stack(host: &dyn mod_api::Host, item: mod_api::ItemKey) -> u32 {
    host.max_stack_size(item).map_or(64, |size| size.max(1))
}

/// The belt's slot count, clamped to 2..=9 as upstream's `setBeltSize` does.
fn belt_size(belt: &ItemStack) -> u8 {
    match belt.get(&SizeComponent) {
        Some(value) => clamp_size(u8::try_from(value).unwrap_or(DEFAULT_SIZE)),
        None => DEFAULT_SIZE,
    }
}

/// A descriptor for the pure engine: item, count, the item's max stack, and a
/// fingerprint of the components (`ItemStack.isSameItemSameComponents`: two
/// stacks only merge when item, damage and components all agree).
fn describe(host: &dyn mod_api::Host, stack: &ItemStack) -> BeltStack {
    BeltStack {
        item: stack.item().id(host),
        count: stack.count(),
        max_stack: max_stack(host, stack.item()),
        variant: variant_of(stack),
        container_bearing: carries_a_container(host, stack),
    }
}

/// A stable fingerprint of a stack's damage and component patch. `Tag` and
/// `Compound` (`BTreeMap`) both hash by value, so equal patches hash equally.
fn variant_of(stack: &ItemStack) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    stack.damage_component().hash(&mut hasher);
    if let Some(patch) = stack.components() {
        patch.as_nbt().hash(&mut hasher);
    }
    hasher.finish()
}

/// Whether the belt takes `stack` (`ConfigData.isItemStackAllowed`).
fn allowed(host: &dyn mod_api::Host, stack: &ItemStack) -> bool {
    allows(&Policy::default(), &describe(host, stack))
}

/// The anvil's belt-upgrade rule, answering for one side: a belt on the left and
/// a pouch on the right make the belt one slot larger for the jar's level cost,
/// and vanilla's result (none for those inputs) is replaced by it. A belt already
/// at nine slots is cancelled, so the anvil offers nothing, as upstream does.
fn anvil_upgrade(host: &dyn mod_api::Host, event: &mut AnvilUpdateEvent<'_>) -> Flow {
    let update = &mut *event.update;
    if !is_belt(host, &update.left) {
        return Flow::Continue;
    }
    let Some(right) = update.right.as_ref() else {
        return Flow::Continue;
    };
    if !is_upgrade_material(&right.item().id(host)) {
        return Flow::Continue;
    }
    let size = belt_size(&update.left);
    let (Some(cost), Some(next)) = (upgrade_cost(size), upgraded_size(size)) else {
        return Flow::Cancel;
    };
    // The upgraded stack is the input itself with a larger size: the stored
    // slots stay where they are, which is exactly what growing keeps.
    let mut upgraded = update.left.clone();
    if upgraded.set(&SizeComponent, i32::from(next)).is_err() {
        return Flow::Continue;
    }
    update.output = Some(upgraded);
    update.xp_cost = cost;
    update.material_cost = 1;
    Flow::Continue
}

/// Whether `stack` can itself hold stacks: a shulker box, a bundle, or any item that declares a
/// `minecraft:container` (or `minecraft:bundle_contents`) component in its defaults or its own
/// patch. Such a stack never enters a belt: Horizon lets a belt sit in a shulker box, so accepting
/// a box as "non-stackable" would let a player build unbounded belt-box-belt nesting.
fn carries_a_container(host: &dyn mod_api::Host, stack: &ItemStack) -> bool {
    let id = stack.item().id(host);
    id.ends_with("_shulker_box")
        || id == BUNDLE_ITEM
        || host
            .item_default_component(stack.item(), CONTAINER_COMPONENT)
            .is_some()
        || stack.components().is_some_and(|patch| {
            has_key(patch.as_nbt(), CONTAINER_COMPONENT)
                || has_key(patch.as_nbt(), BUNDLE_CONTENTS_COMPONENT)
        })
}

/// Whether `nbt` holds `key`. A compound is keyed by Java's lossless UTF-16 strings, which the
/// API does not export by name, so the stored units are compared directly.
fn has_key(nbt: &mod_api::Compound, key: &str) -> bool {
    let units: Vec<u16> = key.encode_utf16().collect();
    nbt.iter().any(|(name, _)| name.0 == units)
}

/// The belt's stored stacks by slot, padded to its slot count. An entry that is
/// not a complete item escape, or one past the size, is dropped.
fn read_slots(host: &dyn mod_api::Host, belt: &ItemStack, size: u8) -> Vec<Option<ItemStack>> {
    let mut slots = vec![None; slot_count(size)];
    let Some(entries) = belt.get(&ContentsComponent) else {
        return slots;
    };
    for entry in &entries {
        let Some((index, stack)) = entry_from_tag(host, entry) else {
            continue;
        };
        if let Some(slot) = slots.get_mut(index) {
            *slot = Some(stack);
        }
    }
    slots
}

/// One stored slot as the component's one compound: the item escape
/// (`{id, count, components?}`) beside its position.
fn entry_tag(host: &dyn mod_api::Host, slot: usize, stack: &ItemStack) -> Option<Tag> {
    let mut entry = stack.to_nbt(host)?;
    entry.insert(
        KEY_SLOT.into(),
        Tag::Int(i32::try_from(slot).unwrap_or(i32::MAX)),
    );
    Some(Tag::Compound(entry))
}

/// One stored slot back to a stack, or `None` when the entry is incomplete or
/// names an item this game does not have.
fn entry_from_tag(host: &dyn mod_api::Host, tag: &Tag) -> Option<(usize, ItemStack)> {
    let Tag::Compound(entry) = tag else {
        return None;
    };
    let slot = match tag.get(KEY_SLOT)? {
        Tag::Int(slot) => usize::try_from(*slot).ok()?,
        _ => return None,
    };
    // `Compound` keeps its slot key out of the item escape the stack reads.
    let mut item = entry.clone();
    item.retain(|key, _| key.to_string_lossy() != KEY_SLOT);
    Some((slot, ItemStack::from_nbt(host, &item)?))
}

/// `belt` carrying `slots` as its components: the size and one entry per filled
/// slot. An empty belt still records its size, as the retired slice did.
fn with_contents(
    host: &dyn mod_api::Host,
    belt: &ItemStack,
    size: u8,
    slots: &[Option<ItemStack>],
) -> Option<ItemStack> {
    let mut entries = Vec::with_capacity(slots.len());
    for (index, stack) in slots.iter().enumerate() {
        if let Some(stack) = stack {
            entries.push(entry_tag(host, index, stack)?);
        }
    }
    let mut belt = belt.clone();
    belt.set(&SizeComponent, i32::from(clamp_size(size))).ok()?;
    belt.set(&ContentsComponent, entries).ok()?;
    Some(belt)
}

// ---------------------------------------------------------------- storage ops

/// Inserts as much of `incoming` as fits; returns what did not fit. `belt` is
/// the stack in hand: nothing is written to the world here.
fn insert_into(
    host: &dyn mod_api::Host,
    belt: &mut ItemStack,
    incoming: ItemStack,
) -> Option<ItemStack> {
    // `ConfigData.isItemStackAllowed`: nothing the belt refuses ever lands,
    // whatever the path (command, click, a future hopper).
    if incoming.count() == 0 || !allowed(host, &incoming) {
        return Some(incoming);
    }
    let size = belt_size(belt);
    let mut slots = read_slots(host, belt, size);
    let mut leftover = Some(incoming);
    // Up to nine slots and one placement a pass: a generous loop bound.
    for _ in 0..64 {
        let Some(current) = leftover.take() else {
            break;
        };
        let descriptions: Vec<Option<BeltStack>> = slots
            .iter()
            .map(|slot| slot.as_ref().map(|stack| describe(host, stack)))
            .collect();
        match find_insertion(&descriptions, size, &describe(host, &current)) {
            Insertion::Merge { slot, room } => {
                let put = room.min(current.count());
                if let Some(Some(existing)) = slots.get_mut(slot) {
                    let _ = existing.set_count(existing.count() + put);
                }
                leftover = current.shrink(put);
            }
            Insertion::Empty { slot } => {
                let put = current.count().min(max_stack(host, current.item()));
                let split = current.clone().split(put);
                if let (Some(piece), Some(target)) = (split.taken, slots.get_mut(slot)) {
                    *target = Some(piece);
                }
                leftover = current.shrink(put);
            }
            Insertion::Full => {
                leftover = Some(current);
                break;
            }
        }
    }
    if let Some(built) = with_contents(host, belt, size, &slots) {
        *belt = built;
    }
    leftover.filter(|stack| stack.count() > 0)
}

/// The stack belt slot `slot` would give at `count`, without touching the belt:
/// the extraction paths hand it to the player first and remove only what the
/// player actually took.
fn peek_from(
    host: &dyn mod_api::Host,
    belt: &ItemStack,
    slot: usize,
    count: u32,
) -> Option<ItemStack> {
    if count == 0 {
        return None;
    }
    let slots = read_slots(host, belt, belt_size(belt));
    let existing = slots.get(slot)?.as_ref()?;
    let taken = count.min(existing.count());
    (taken > 0)
        .then(|| existing.clone().split(taken).taken)
        .flatten()
}

/// Takes up to `count` out of `slots` at `slot`, answering how many were taken.
fn take_slot(slots: &mut [Option<ItemStack>], slot: usize, count: u32) -> u32 {
    let Some(held) = slots.get_mut(slot) else {
        return 0;
    };
    let Some(existing) = held.as_ref() else {
        return 0;
    };
    let taken = count.min(existing.count());
    if taken == 0 {
        return 0;
    }
    let left = existing.count() - taken;
    match left {
        0 => *held = None,
        _ => {
            if let Some(existing) = held.as_mut() {
                let _ = existing.set_count(left);
            }
        }
    }
    taken
}

/// The first occupied slot, for a sneak extract.
fn first_occupied(host: &dyn mod_api::Host, belt: &ItemStack) -> Option<usize> {
    read_slots(host, belt, belt_size(belt))
        .iter()
        .position(Option::is_some)
}

// --------------------------------------------------------------- game access

/// The inventory slot a player's held stack sits in (`Inventory`'s numbering:
/// the hotbar is 0..9).
fn held_slot(host: &mut dyn ServerHost, player: PlayerRef) -> usize {
    player.selected_slot(&*host).unwrap_or(0)
}

/// A player's stack in one inventory slot, by `Inventory`'s numbering.
fn inventory_stack(host: &mut dyn ServerHost, player: PlayerRef, slot: usize) -> Option<ItemStack> {
    let wanted = u32::try_from(slot).ok()?;
    player
        .inventory_slots(&*host)?
        .into_iter()
        .find(|(index, _)| *index == wanted)
        .map(|(_, stack)| stack)
}

/// Whether the player is sneaking (`Player.isSecondaryUseActive`).
fn sneaking(host: &mut dyn ServerHost, player: PlayerRef) -> bool {
    player.is_secondary_use_active(&*host).unwrap_or(false)
}

/// A player's stack in one hand.
fn hand_item(host: &mut dyn ServerHost, player: PlayerRef, hand: Hand) -> Option<ItemStack> {
    host.living(player.entity())?.hand_item(&*host, hand)
}

/// Writes a player's hand: the game's own equip write, so the client is told as
/// it is for any equip. `false` means the game refused and nothing changed.
fn set_hand(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    hand: Hand,
    stack: Option<ItemStack>,
) -> bool {
    let Some(living) = host.living(player.entity()) else {
        return false;
    };
    matches!(living.set_hand_item(host, hand, stack), Ok(true))
}

/// Where a player's slot is for `ItemPlace`: `inventory_slots` counts the armour
/// as 36..=39 and the off hand as 40, while `ItemPlace::PlayerSlot` wants
/// 100..=103 and `-106`. Only the main inventory (0..=35) shares its number.
fn place_of(slot: u32) -> Option<i32> {
    match slot {
        0..=35 => i32::try_from(slot).ok(),
        40 => Some(-106),
        _ => None,
    }
}

/// Adds `stack` to a player's inventory the way vanilla's `Inventory.add` does
/// (a matching stack with room first, then an empty slot) and answers how many
/// fitted — the caller may only take that much out of the belt. Only the main
/// inventory and the off hand are offered, as the retired glue's order did.
fn give_to_player(host: &mut dyn ServerHost, player: PlayerRef, stack: &ItemStack) -> u32 {
    let Some(handler) = player.entity().capability(host, &ENTITY_ITEMS, ()) else {
        return 0;
    };
    let resource = ItemResource::of(stack);
    let asked = stack.count();
    host.transaction(|host, tx| {
        let mut moved = 0;
        for index in MAIN_SLOTS.chain(std::iter::once(OFF_HAND_ITEM)) {
            if moved >= asked {
                break;
            }
            moved += handler.insert(host, tx, index, &resource, asked - moved);
        }
        Ok::<u32, ()>(moved)
    })
    .unwrap_or(0)
}

/// Takes up to `amount` of `stack` back out of a player's inventory: the undo of
/// a `give_to_player` whose belt write the game refused.
fn take_from_player(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    stack: &ItemStack,
    amount: u32,
) -> u32 {
    let Some(handler) = player.entity().capability(host, &ENTITY_ITEMS, ()) else {
        return 0;
    };
    let resource = ItemResource::of(stack);
    host.transaction(|host, tx| {
        let mut moved = 0;
        for index in MAIN_SLOTS.chain(std::iter::once(OFF_HAND_ITEM)) {
            if moved >= amount {
                break;
            }
            moved += handler.extract(host, tx, index, &resource, amount - moved);
        }
        Ok::<u32, ()>(moved)
    })
    .unwrap_or(0)
}

/// Takes `amount` of `stack` out of one inventory place.
fn extract_place(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    slot: u32,
    stack: &ItemStack,
    amount: u32,
) -> u32 {
    let Some(slot) = place_of(slot) else {
        return 0;
    };
    let place = ItemPlace::PlayerSlot {
        player: player.uuid(),
        slot,
    };
    let resource = ItemResource::of(stack);
    host.transaction(|host, tx| Ok::<u32, ()>(place.extract(host, tx, &resource, amount)))
        .unwrap_or(0)
}

/// Writes a belt's contents into the inventory place the belt sits at: one
/// transaction, so all of it lands or the belt is unchanged.
fn write_at(host: &mut dyn ServerHost, place: &ItemPlace, built: ItemStack) -> bool {
    host.transaction(|host, tx| {
        Ok::<bool, ()>(place.edit(host, tx, move |stack| {
            let size = built.get(&SizeComponent);
            let entries = built.get(&ContentsComponent);
            if let Some(size) = size {
                let _ = stack.set(&SizeComponent, size);
            }
            if let Some(entries) = entries {
                let _ = stack.set(&ContentsComponent, entries);
            }
        }))
    })
    .unwrap_or(false)
}

// ------------------------------------------------------------- menu lifecycle

/// Opens the belt's menu for a player whose belt is `slot` of their inventory:
/// one row listing the belt's contents.
fn open_menu(
    host: &mut dyn ServerHost,
    state: &Mutex<State>,
    player: PlayerRef,
    slot: usize,
) -> bool {
    let Some(belt) = inventory_stack(host, player, slot) else {
        return false;
    };
    if !is_belt(&*host, &belt) {
        return false;
    }
    let size = belt_size(&belt);
    let slots = read_slots(&*host, &belt, size);
    let items = menu_items(size, &slots);
    let Ok(window) = host.open_menu(
        player.uuid(),
        MENU_ROWS,
        Text::literal(TITLE),
        items,
        MenuReplace::CloseFirst,
    ) else {
        return false;
    };
    let Some(window) = window else {
        return false;
    };
    locked(state).open(player.uuid(), OpenBelt { window, slot, size });
    true
}

/// Updates an open menu to its belt's current contents, and closes it when the
/// slot no longer holds the belt.
fn refresh_menu(host: &mut dyn ServerHost, state: &Mutex<State>, uuid: u128, open: OpenBelt) {
    let Some(player) = host.player(uuid) else {
        return;
    };
    let Some(stack) = inventory_stack(host, player, open.slot) else {
        return;
    };
    if !is_belt(&*host, &stack) {
        host.close_menu(uuid);
        locked(state).forget_open(uuid, open.window);
        return;
    }
    let slots = read_slots(&*host, &stack, open.size);
    let _ = host.menu_set_items(uuid, open.window, menu_items(open.size, &slots));
}

/// The menu's items: the belt's own slots first, padded to one row.
fn menu_items(size: u8, slots: &[Option<ItemStack>]) -> Vec<Option<ItemStack>> {
    let mut items = vec![None; MENU_SLOTS];
    for (index, stack) in slots.iter().take(usize::from(size)).enumerate() {
        if index < MENU_SLOTS {
            items[index] = stack.clone();
        }
    }
    items
}

/// The belt slot a menu click names, or `None` for a click on one of the
/// player's inventory slots that follow the belt's own row.
fn belt_slot(slot: i32, size: u8) -> Option<usize> {
    let index = usize::try_from(slot).ok()?;
    (index < usize::from(size)).then_some(index)
}

/// Extracts one belt stack into the acting player's inventory, and only what the
/// inventory actually took: a full inventory must never delete the tool between
/// the belt and the player.
fn extract_into_inventory(
    host: &mut dyn ServerHost,
    player: PlayerRef,
    belt: &ItemStack,
    slot: Option<usize>,
) {
    let Some(index) = slot.or_else(|| first_occupied(&*host, belt)) else {
        return;
    };
    let Some(offered) = peek_from(&*host, belt, index, TAKE_DEFAULT) else {
        return;
    };
    let placed = give_to_player(host, player, &offered);
    if placed == 0 {
        return;
    }
    let size = belt_size(belt);
    let mut slots = read_slots(&*host, belt, size);
    take_slot(&mut slots, index, placed);
    if let Some(built) = with_contents(&*host, belt, size, &slots) {
        set_hand(host, player, Hand::Main, Some(built));
    }
}

// ------------------------------------------------------------------- commands

enum Action {
    Help,
    Info,
    Menu,
    Put { item: String, count: u32 },
    Take { slot: usize, count: u32 },
}

/// The item argument of a `/toolbelt` usage, from the dispatcher's parsed value.
///
/// It is a macro because `uses.json` is a source-text scan: it resolves a bare
/// type name against the SDK surface, so a `CommandArgumentValue::Item(..)`
/// pattern is recorded as a build of the SDK's *content declaration* type `Item`
/// — a type this port never touches — and `fits` then refuses every other build
/// whose `Item` differs. The scan does not expand macros, so this match keeps the
/// compatibility table true without renaming anything on the SDK's side (the
/// precedent is `ports/artifacts`: a local `Config` collided the same way and was
/// renamed). The pattern below is the whole macro's body.
macro_rules! item_argument {
    ($value:expr) => {
        match $value {
            mod_api::CommandArgumentValue::Item(stack) => Some(stack),
            _ => None,
        }
    };
}

/// The action a parsed `/toolbelt` invocation asks for, or `None` when it is not
/// one of the port's usages (the command then falls through untouched).
fn action_of(invocation: &CommandInvocation, host: &dyn mod_api::Host) -> Option<Action> {
    match invocation.literals.get(1).map(String::as_str) {
        Some("info") => Some(Action::Info),
        Some("menu") => Some(Action::Menu),
        Some("help") | None => Some(Action::Help),
        Some("put") => {
            let item = item_argument!(invocation.argument("item")?)?
                .item()
                .id(host);
            let count = match invocation.argument("count") {
                None => 1,
                Some(CommandArgumentValue::Integer(count)) => u32::try_from(*count).unwrap_or(0),
                Some(_) => return None,
            };
            Some(Action::Put { item, count })
        }
        Some("take") => {
            let slot = match invocation.argument("slot")? {
                CommandArgumentValue::Integer(slot) => *slot,
                _ => return None,
            };
            let count = match invocation.argument("count") {
                None => None,
                Some(CommandArgumentValue::Integer(count)) => Some(*count),
                Some(_) => return None,
            };
            let (slot, count) = take_args(slot, count)?;
            Some(Action::Take { slot, count })
        }
        Some(_) => None,
    }
}

/// `/toolbelt take <slot> [count]`: the count defaults to a stack and nothing is
/// taken for a count that is not a positive number.
fn take_args(slot: i32, count: Option<i32>) -> Option<(usize, u32)> {
    let slot = usize::try_from(slot).ok()?;
    let count = match count {
        None => TAKE_DEFAULT,
        Some(count) => u32::try_from(count).ok()?,
    };
    (count > 0).then_some((slot, count))
}

/// The held belt, or `None` when the main hand does not hold one.
fn held_belt(host: &mut dyn ServerHost, player: PlayerRef) -> Option<ItemStack> {
    hand_item(host, player, Hand::Main).filter(|stack| is_belt(&*host, stack))
}

/// `/toolbelt info`: the held belt's size and contents.
fn info_line(host: &mut dyn ServerHost, player: PlayerRef) -> String {
    let Some(belt) = held_belt(host, player) else {
        return "Hold the belt first.".to_owned();
    };
    let size = belt_size(&belt);
    let slots = read_slots(&*host, &belt, size);
    let used = slots.iter().filter(|slot| slot.is_some()).count();
    let contents = slots
        .iter()
        .enumerate()
        .filter_map(|(index, stack)| {
            stack
                .as_ref()
                .map(|stack| format!("{index}:{}({})", stack.item().id(&*host), stack.count()))
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "toolbelt:belt size={size} slots={used}/{} contents=[{contents}]",
        slot_count(size)
    )
}

/// `/toolbelt put <item> <count>`: moves the *actual* inventory stacks (damage
/// and components included) into the belt, and only what the belt accepts, so a
/// refused stack goes back and nothing is dropped.
fn put(host: &mut dyn ServerHost, player: PlayerRef, item: &str, count: u32) -> String {
    let Some(belt) = held_belt(host, player) else {
        return "Hold the belt first.".to_owned();
    };
    if count == 0 {
        return "Count must be at least 1.".to_owned();
    }
    let places = player.inventory_slots(&*host).unwrap_or_default();
    let mut belt = belt;
    let mut remaining = count;
    let mut taken: Vec<ItemStack> = Vec::new();
    for (slot, stack) in &places {
        if remaining == 0 {
            break;
        }
        if stack.item().id(&*host) != item || place_of(*slot).is_none() {
            continue;
        }
        let wanted = remaining.min(stack.count());
        let Some(piece) = stack.clone().split(wanted).taken else {
            continue;
        };
        let got = extract_place(host, player, *slot, stack, wanted);
        if got == 0 {
            continue;
        }
        remaining -= got;
        let Some(piece) = piece.split(got).taken else {
            continue;
        };
        taken.push(piece);
    }
    if taken.is_empty() {
        return format!("Carry {item} first.");
    }
    let mut inserted = 0;
    for piece in taken {
        let offered = piece.count();
        match insert_into(&*host, &mut belt, piece) {
            Some(rest) => {
                // Only what the belt refused goes back: it is exactly what left
                // the player's inventory, whose slots are now free.
                give_to_player(host, player, &rest);
                inserted += offered - rest.count();
            }
            None => inserted += offered,
        }
    }
    if !set_hand(host, player, Hand::Main, Some(belt)) {
        return "The belt could not be saved; nothing was moved.".to_owned();
    }
    if inserted == 0 {
        return format!("The belt does not take {item}.");
    }
    format!("Inserted {inserted} {item}.")
}

/// `/toolbelt take <slot> <count>`: moves a belt slot's stack to the inventory,
/// and only what the inventory actually took leaves the belt.
fn take(host: &mut dyn ServerHost, player: PlayerRef, slot: usize, count: u32) -> String {
    let Some(belt) = held_belt(host, player) else {
        return "Hold the belt first.".to_owned();
    };
    let Some(offered) = peek_from(&*host, &belt, slot, count) else {
        return format!("Belt slot {slot} is empty.");
    };
    let item = offered.item().id(&*host);
    let placed = give_to_player(host, player, &offered);
    if placed == 0 {
        return format!("No room for {item}; it stays in slot {slot}.");
    }
    let size = belt_size(&belt);
    let mut slots = read_slots(&*host, &belt, size);
    take_slot(&mut slots, slot, placed);
    let Some(built) = with_contents(&*host, &belt, size, &slots) else {
        take_from_player(host, player, &offered, placed);
        return "The belt could not be saved; nothing was moved.".to_owned();
    };
    if !set_hand(host, player, Hand::Main, Some(built)) {
        take_from_player(host, player, &offered, placed);
        return "The belt could not be saved; nothing was moved.".to_owned();
    }
    format!("Took {placed} {item} from slot {slot}.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_main_inventory_and_the_off_hand_are_places_the_belt_touches() {
        assert_eq!(place_of(0), Some(0));
        assert_eq!(place_of(35), Some(35));
        assert_eq!(
            place_of(40),
            Some(-106),
            "the off hand is `-106` in `ItemPlace::PlayerSlot`"
        );
        assert_eq!(place_of(36), None, "feet: the armour is not a belt place");
        assert_eq!(place_of(39), None, "head");
        assert_eq!(place_of(41), None, "past the off hand");
    }

    #[test]
    fn a_menu_click_names_a_belt_slot_only_inside_the_belts_own_row() {
        assert_eq!(belt_slot(0, 2), Some(0));
        assert_eq!(belt_slot(1, 2), Some(1));
        assert_eq!(
            belt_slot(2, 2),
            None,
            "menu slot 2 is the player's first inventory slot, not a belt slot"
        );
        assert_eq!(belt_slot(8, 9), Some(8));
        assert_eq!(belt_slot(9, 9), None);
        assert_eq!(belt_slot(-999, 9), None, "a click outside the menu");
    }

    #[test]
    fn take_defaults_its_count_and_refuses_an_empty_request() {
        assert_eq!(take_args(0, None), Some((0, 64)));
        assert_eq!(take_args(2, Some(1)), Some((2, 1)));
        assert_eq!(take_args(0, Some(0)), None, "taking nothing is nothing");
        assert_eq!(take_args(-1, None), None, "a negative slot is not a slot");
        assert_eq!(take_args(0, Some(-1)), None);
    }

    #[test]
    fn a_physical_click_is_handled_once_across_the_block_and_air_paths() {
        let mut state = State::default();
        assert!(!state.guarded(7), "the first click is not guarded");
        state.guard(7);
        assert!(state.guarded(7), "the same click's second path is refused");
        state.tick();
        assert!(
            state.guarded(7),
            "one tick later the click is still the same physical one"
        );
        state.tick();
        assert!(!state.guarded(7), "two ticks later a new click is handled");
        assert!(
            state.guard.is_empty(),
            "a decayed guard is dropped, so the map does not grow per player"
        );
    }

    #[test]
    fn a_queued_menu_open_is_taken_once() {
        let mut state = State::default();
        state.queue_open(7, 3);
        state.queue_open(7, 4);
        assert_eq!(
            state.take_pending().get(&7),
            Some(&4),
            "the last click wins"
        );
        assert!(
            state.take_pending().is_empty(),
            "and it is only opened once"
        );
    }

    #[test]
    fn forgetting_a_menu_keeps_the_one_another_window_opened() {
        let mut state = State::default();
        state.open(
            7,
            OpenBelt {
                window: 12,
                slot: 3,
                size: 2,
            },
        );
        state.forget_open(7, 11);
        assert_eq!(
            state.open_of(7).map(|open| open.window),
            Some(12),
            "a close for another window does not forget this menu"
        );
        state.forget_open(7, 12);
        assert!(state.open_of(7).is_none());
    }
}
