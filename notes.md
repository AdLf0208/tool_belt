# Tool Belt → Horizon

Ported from **Tool Belt 2.2.10** (`gigaherz/ToolBelt`, **BSD-3-Clause**),
Minecraft Java 1.21.1 NeoForge, read from
`ports/_shared/upstream/toolbelt` at commit
`2737d42e947d96745450cfb066db0b0c60549bbe` (2026-06-26). The upstream source
was read for behaviour only; this is a clean-room Rust rewrite. The exact
ATM10 jar is `ToolBelt-1.21.1-2.2.10.jar`, SHA-1
`96862e5d8df67d63b749fc15d18262f3736d1828`, SHA-256
`918b35093dfaf8033115acd8dacd8978e325dcba36355f8d1a938ee92d5ab4ff`
(provenance: [`../_shared/carry-work/toolbelt/SOURCE.md`](../_shared/carry-work/toolbelt/SOURCE.md)).
**LOCAL-ONLY** until the maintainer approves publishing (ARCHITECTURE.md §8).

**Format: the SDK package (2026-11-18).** This port was migrated from the
retired `.hnmod` container to the flat SDK package — see
[`## Migration to the SDK format`](#migration-to-the-sdk-format) for the file
map, the helper mapping and every trap. `src/runtime.rs` **carries** the retired
`mixins/toolbelt/runtime.rs`, the review-fix `container_bearing` rule and the
0.2.2 anvil-upgrade rules (see [`## Slice 2`](#slice-2--the-belt-upgrade-the-pouch-and-the-anvil-022));
its pre-0.2.2 file was SHA-256
`e6ed170244c7e46e112dfc406efe5abe9ef8efb259ef29be8674226706ceba78`.
Version **0.2.2**; the retired submission was `toolbelt` 0.1.0 (PR **#573**,
closed by the format purge). Pin: SDK nightly
**`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`** (2026-11-20) + horizon-mixins
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0` (repinned from `0b14f8a` in 0.2.2).

Status: **slices 1 and 2 implemented and verified in-game** — the belt item, its
two-to-nine-slot on-item storage, the insert/extract rules, a belt menu, the
click interactions, the pouch item with both carried recipes, and the anvil
upgrade from two to nine slots. The radial key menu, wearing/Curios, the
sewing-kit upgrades and the server config are deferred (see Gaps).

## Approach (from the pin's `docs/modpacks.md`)

- **Class `content`, tier `machines`** (`ports/ledger.csv` row `toolbelt`;
  provenance `ports/_shared/source-map/atm10-sources.csv`). This is a
  **machines** agent port, not a data carry: the belt is a container item with
  its own slot storage, a menu and item-component persistence. The content half
  is the belt's models, textures, lang and recipe carried from the exact ATM10
  jar.
- **Licence:** the jar's `META-INF/neoforge.mods.toml` says
  `license="BSD (3 clause)"`, and `LICENSE.txt` is the three-clause BSD text
  (copyright 2024 David Quintana). → **BSD-3-Clause, rank 0 (permissive)**;
  the source map's "MIT" is superseded by the jar. Still local-only until the
  maintainer approves publishing.
- **Carry-over vs agent port:** the belt's item model, the two item textures,
  the GUI texture, all 15 lang files and the shaped belt recipe are carried
  (`assets/toolbelt/**`, `data/toolbelt/**`, 22 files); the storage engine, the
  components, the menu and the interaction are the agent port.

## Slice 1 — the belt item and its storage

- `toolbelt:belt` on the `minecraft:stick` item template, `max_stack_size 1`,
  no cooking fuel (`data/toolbelt/horizon/item/belt.json`).
- Two custom components, registered before content loads (`component_types`):
  `toolbelt:belt_size` (int, clamped **2..=9**, missing reads as 2) and
  `toolbelt:belt_contents` (a list of `{slot, id, count, components?}` entries —
  the item escape beside its position, so a slot's place and its damage,
  enchantments and name survive).
- The pure engine (`src/runtime.rs`): `clamp_size`/`slot_count`, `allows`
  (`ConfigData.isItemStackAllowed`: blacklist, no bundles, no belts, only
  non-stackable items by default), `find_insertion` (merge an equal stack and
  component variant, else the first empty slot, else full), `extract`, and the
  component round-trip (`to_entries`/`from_entries`, slot positions kept).
- **Right-click** with the belt in the main hand: with an allowed **off-hand**
  stack, insert it; while **sneaking**, extract the first occupied slot to the
  inventory; otherwise open the belt menu (deferred to the end of the server
  tick, which is where a menu can be opened).
- The **belt menu** (the pin's native chest menu, one row + the player
  inventory) lists the belt's contents; a click on a belt slot moves that stack
  to the player. It is display-only (the pin's menu slots cannot be rule-limited
  per item), refreshed from the held belt each server tick.
- `/toolbelt put|take|info|menu|help` at permission level 2: the
  headless-testable path over the same runtime/storage code.

### Behaviour read from upstream (and where this port differs)

| Area | Upstream | This port |
|---|---|---|
| storage | `toolbelt:belt_size` + `ItemContainerContents` on the stack | `toolbelt:belt_size` + a custom `belt_contents` list |
| slots | 2..9, default 2, size clamp 2..9 | same |
| slot capacity | a slot's item stack (item's max stack) | same |
| insert rule | blacklist/whitelist, no bundles, no belts, non-stackable by default | blacklist (no whitelist), no bundles, no belts, non-stackable by default |
| insert path | belt menu drag/shift-click | off-hand right-click, `/toolbelt put` |
| extract path | belt menu drag/shift-click, radial swap | belt-menu click, sneak right-click, `/toolbelt take` |
| menu | custom `BeltContainer`/`BeltScreen` | a native chest menu, one row + the player inventory |
| menu texture | `textures/gui/belt.png` | carried but not drawn (the native menu has no custom background) |
| key | radial menu on `R`, swap/cycle keybinds, packets | **not implemented** (see Gaps) |
| wearing | Curios belt slot or the mod's own belt slot; `onWornTick` | **not implemented** (see Gaps) |
| comparator/hopper | item capability (`Capabilities.Item`) | **not implemented** (no item-container capability of our own at this pin) |

### Hooks and helpers (all stamped at the pin)

| Hook | Where | What it does |
|---|---|---|
| `register` | `Hooks::register` | attaches the belt's `ItemPart` to `toolbelt:belt` |
| `component_types` | `Hooks::component_types` | registers `toolbelt:belt_size` and `toolbelt:belt_contents` |
| `right_click_block` | `Hooks::right_click_block` | a block click with the belt: insert / sneak-extract / queue the menu, and cancel the click so the block is not also used |
| `use_item` | `ItemPart::use_item` | the same click in the air |
| `server_tick` | `Hooks::server_tick` | end of a running tick: open queued menus, refresh open ones, age the click guard |
| `menu_clicked` | `Hooks::menu_clicked` | a belt-slot click moves that stack to the player |
| `menu_closed` | `Hooks::menu_closed` | forget the menu state |
| `register_commands` | `Hooks::register_commands` | `toolbelt put|take|info|menu|help` |
| `run_mod_command` | `Hooks::run_mod_command` | runs them with typed arguments |

Helpers used: `MenusServerHost::{open_menu, menu_set_items, close_menu}`,
`ItemStack::{get, set, to_nbt, from_nbt}`, `EntityRef::capability` with
`ENTITY_ITEMS`, `ItemPlace::{PlayerSlot::extract, edit}`, `LivingRef::{hand_item,
set_hand_item}`, `PlayersHost::player`, `PlayerRef::{inventory_slots,
selected_slot, is_secondary_use_active, tell}`, `ItemsHost::{item_key,
max_stack_size}`, `ItemsRegistrar::item_key`, `HostTransaction::transaction`.
No pending hook or helper is named.

## Slice boundary (what this port is)

**In:** the belt item, the 2..9-slot component storage, the insert/extract
rules (including merging and the capacity limit), the belt menu listing the
contents with a click-to-take, the off-hand right-click insert, the sneak
extract, the `/toolbelt` command, and the carried belt content (22 files).

**Out (gaps, next slices):** the client radial menu and its keybinds
(`key.toolbelt.open`/`swap`/`slot`/`cycle.*`) — the pin's `key_mapping*` hooks
are client notifications and there is no stamped client→server packet path for
the swap; the packet set and `BeltFinder`; wearing the belt (Curios and the
mod's own belt slot); the `IItemInBelt` per-item tick (`ItemPart::inventory_tick`
is the pin's shape for it); anvil/sewing upgrades; the pouch item and the dyed
belt; the custom screen background and the player layer renderer; the server
whitelist/blacklist config and the `allowAllNonStackableItems` toggle; a
hopper/item-capability bridge (`ITEM_ITEMS` is the pin's item-container
capability, so a `ResourceHandler` registered through `item_capability` is now
possible); tooltips (`ItemPart::tooltip`); sounds and particles.

**Deliberate deviations:**

1. The menu is the pin's native menu, so it has no custom belt texture and
   cannot refuse a stackable item by a slot rule — the belt's own rule is
   enforced by the insert engine (the off-hand click and `/toolbelt put`), and
   the menu is a read/click-to-take view.
2. A sneak/off-hand click mapping substitutes for the radial swap, which needs a
   missing client→server packet surface.
3. `/toolbelt take` places the exact stack (components kept) into the player's
   inventory.
4. `/toolbelt put ... -1` answers "Count must be at least 1." instead of
   inserting one: the typed dispatcher refuses a negative integer where the
   count wants a `u32`, and the retired line parser read `-1` as the default 1.
5. The block click is cancelled on the **server** only, as the retired hook was
   server-side: `right_click_block` reaches the mod on the client too and
   answers `Flow::Continue` there (cancelling on the client would stop the
   interaction packet, and with it the belt action), so a client may predict the
   block's own use for the tick before the server's cancel arrives. The same
   asymmetry existed in the retired format.

## Upstream defects fixed

None found in this slice: the jar's item, component and container behaviour is
internally consistent, and the sizes the code clamps (2..9) match its own data.
The two **[P1] bugs fixed at review of PR #573 were the port's own** — extraction
could delete a tool when the inventory was full, and `/toolbelt put` rebuilt a
plain stack instead of moving the real one — and the new glue keeps both
repairs: every extraction path gives the stack to the player first and removes
only what the player's inventory accepted (`give_to_player`'s count), and `put`
extracts the actual stack and its components through `ItemPlace::PlayerSlot`.

## Migration to the SDK format

*Recipe written 2026-11-18 from `ports/imfast` (the first migration) and
`ports/pocketstorage` (the container/second-migration reference), applied here
to a **content + native** package with an item part, a menu and a command.*

**Why.** Horizon's mod-API merge (#16941, launcher ≥ 0.2.70, PR #17992) retired
`.hnmod`; the registry closed this port's submission (#573: the campaign's own
`_shared/HANDOVER.md` rows 375/421 and this port's review section name it; a
brief draft said #873, which no record corroborates) with one reason, and an
`.hnmod` no longer loads. The new unit is **one flat `.hmod`**: content,
native code, or both. Pins: nightly
**`0b14f8acfd97fa2c750e42795249dcc981178ca4`**, mixins
**`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`** (launcher 0.2.78).

### 1. Files created, carried and deleted

| Action | Path | Note |
|---|---|---|
| created | `horizon-mod.toml` | the one manifest: id/name/version/api/side/capabilities + `[native]` |
| rewritten | `Cargo.toml` | depends on `horizon-mod-api` from `.horizon/sdk/`; `publish = false` |
| created | `src/runtime.rs` | the old `mixins/toolbelt/runtime.rs`, byte-identical (`cp`, then `cmp`) |
| created | `src/lib.rs` | the one `#[mod_api::horizon_mod] impl Hooks` + the belt's `#[horizon_part] impl ItemPart` (was `support.rs` + 8 hook helpers + 2 modules) |
| carried | `assets/toolbelt/**`, `data/toolbelt/**` | the old `content/assets` + `content/data`, byte-identical (`diff -r`, "CONTENT IDENTICAL") |
| created | `.gitignore` | `/target`, `/.horizon`, `/Cargo.lock` |
| rewritten | `README.md` | it ships inside the `.hmod`; the old one was the **template's** README and described the retired layout |
| rewritten | `LICENSE-NOTICE.md` | it ships too; the "data/minecraft/tags" line described content this mod does not have |
| created | `toolbelt.hmod` | the deliverable, in the port root |
| deleted | `mixins/toolbelt/` | `horizon-mixin.toml` + `support.rs` + 10 helpers + the embedded content `.hmod` |
| deleted | `content/` | the content half is now `assets/` + `data/` at the package root |
| deleted | `compatibility.json`, `rust-toolchain.toml` | the pin is `[native]`; the toolchain is the launcher's (the file is refused by the build) |
| deleted | `Cargo.lock` | a packed lock changes the artifact (trap 5) |
| deleted | `.github/`, `AGENTS.md` | they described the retired staging/CI pipeline |
| deleted | `tools/{horizon.py, package.py, ladder-embed.sh, carry_content.py, test_stage.py, test_template.py, add-notices.py, audit/}` | the retired ladder and packaging are gone |
| retired | `_shared/evidence/toolbelt/retired-hnmod-0.1.0/` | `mixins/`, `content/`, `compatibility.json`, `rust-toolchain.toml`, `Cargo.lock`, the 0.1.0 `.hnmod`, `AGENTS.md`, `README.md`, `FINDINGS.md`, `VALIDATION.md`, `tools/`, `.github/` — the record, not the package |
| kept | `tools/e2e.sh` | the behaviour's fail-capable scenario; it drives the retired staged pipeline and needs a launcher-built game (gap) |

### 2. Manifest mapping

| Old (`mixins/toolbelt/horizon-mixin.toml`) | New (`horizon-mod.toml`) |
|---|---|
| `[mod] id = "toolbelt"` | `id = "toolbelt"` (also the resource namespace) |
| `[mod] version = "0.1.0"` | `version = "0.2.0"` (a resubmission is a new version) |
| `[mod] priority = 100` | gone (listener order is `#[listener(priority = …)]`, unused here) |
| `[compatibility] horizon_revision = "1d63e0a5…"` | `[native] horizon = "0b14f8ac…"` (exact 40-hex) |
| — | `[native] mixins = "e59da909…"` (it moved too: `5a64af93` → `e59da909`) |
| — | `[native] game = "26.3"` |
| `[[module]] runtime` (injected as `crate::game::toolbelt`) | `src/runtime.rs` + `pub mod runtime` |
| `[[module]] state` (`support.rs`) | methods/handlers in `src/lib.rs`, state in `Arc<Mutex<State>>` on the mod |
| `[[injection]] component-types/use-item/use-item-on/server-tick/menu-clicked/menu-closed/register-commands/perform-command` | `impl Hooks` methods + one `ItemPart` (table below) |
| `[[injection]] embedded-content` (`content_root` HEAD) | **removed, not mapped**: one flat `.hmod` carries `assets/`+`data/`, so there is no second half to write out |
| content inside the bundle at `data/toolbelt-content.hmod` | `assets/` + `data/` in the package (nothing embedded) |

### 3. Hook → method, helper → host call

| Old hook (API v1) | Old helper | SDK equivalent |
|---|---|---|
| `component_types` | `component_types.rs` | `Hooks::component_types` → `event.types.push(id)` |
| `use_item` (13) | `use_item.rs` | `ItemPart::use_item(HostSide, &mut UseItemEvent) -> Option<InteractionResult>`; the item is the belt's own part, attached in `register` |
| `use_item_on` (13) | `use_item_on.rs` | **`Hooks::right_click_block`**, not the item's `use_on`: at this pin `use_on` runs *after* the block's own use ("a chest still opens"), and the retired port claimed the click *before* the block. The global hook cancels with `InteractionResult::Success`, so the block is not used and the item does not run |
| `server_tick` (44) | `server_tick.rs` | `Hooks::server_tick`; `event.phase` is `ServerTickPhase::ServerTickEnd { world_running }` (the enum is `#[non_exhaustive]`, so the match keeps a wildcard) |
| `menu_clicked` (10) | `menu_clicked.rs` | `Hooks::menu_clicked`; `event.slot` is the menu slot (our row first) |
| `menu_closed` (10) | `menu_closed.rs` | `Hooks::menu_closed` |
| `register_commands` (8) | `register_commands.rs` | `Hooks::register_commands` → `Command::op(usage, 2)` |
| `perform_command` (8) | `perform_command.rs` | `Hooks::run_mod_command -> Flow`; the line arrives **parsed** (`CommandInvocation`: literals + typed `item`/`integer` arguments) |
| `content_root` (raw target) | `embedded_content.rs` | **removed, not mapped** (one package carries its own content) |
| — | — | `Hooks::register`: `registrar.item_key("toolbelt:belt")` + `registrar.item_part(key, Belt { state })` |

| Old helper (`support.rs`) | SDK call |
|---|---|
| `native_component` / `native_set_component` | `ItemStack::get` / `ItemStack::set` with a `Component` descriptor (`SizeComponent`, `ContentsComponent`) |
| `encode_components` / `decode_components` (the entry's `{damage, extra}`) | `ItemStack::to_nbt` / `ItemStack::from_nbt` — the same `{id, count, components?}` escape, so the retired component shape still reads |
| `game.native_open_menu` / `native_set_menu_items` / `native_close_menu` | `host.open_menu(uuid, 1, title, items, MenuReplace::CloseFirst)` / `menu_set_items` / `close_menu` |
| `game.held()` / `held_mut()` / `held_slot()` | `LivingRef::hand_item` / `set_hand_item` (the game's own equip write, which syncs the client) and `PlayerRef::selected_slot` |
| `game.offhand` (read/write) | `LivingRef::hand_item(Hand::Off)` / `set_hand_item(Hand::Off, …)` |
| `game.native_acting_player()` | `PlayersHost::player(uuid)` — the uuid comes from the event (`click.player`, `event.player`, `CommandSource::Player`) |
| `game.player.shift_key_down` | `PlayerRef::is_secondary_use_active` |
| `game.inventory` (the 36 slots + off hand) | `PlayerRef::inventory_slots` (0..=35 main, 36..=39 armour, 40 off hand), `EntityRef::capability(host, &ENTITY_ITEMS, ())` → `ResourceHandler`, and `ItemPlace::PlayerSlot` |
| `give_to_acting` / `give_to_guest` | `give_to_player`: `ResourceHandler::insert` per index (0..36 then 40); returns **how many fitted**, so a full inventory leaves the tool in the belt (PR #573 P1) |
| `game.native_feedback` | `PlayerRef::tell` |
| `game.native_take_items` (via the inventory walk) | `ItemPlace::PlayerSlot::extract` per slot (the real stack, components included) |
| `write_guest_slot` + `hub.send_inventory` | gone: `ItemPlace::edit` on the player's slot journals and the game syncs it |
| `static STATE: Mutex<Option<State>>` | `Mod { state: Arc<Mutex<State>> }`, shared with the part by an `Arc` clone; every lock is a few map operations and **no host call** (a hook that re-enters the mod cannot deadlock) |

`State` keeps the retired bookkeeping — the click guard, the queued opens, the
open menus — and is now unit-tested (`sdk-test.log`, 6 glue tests): the guard
lasts two ticks and is dropped when spent, a queued open is taken once, a close
for another window keeps this menu, the menu-slot limit, the take default, and
the `Inventory` ↔ `ItemPlace` slot numbering.

### 4. Verification of this migration

| Step | Command | Log | Result |
|---|---|---|---|
| runtime byte-identical | `cmp src/runtime.rs retired/mixins/toolbelt/runtime.rs` | — | identical, SHA-256 `9ba28129…1aa0` (also `unzip -p toolbelt.hmod src/runtime.rs`) |
| TDD red | `cargo test` on the tests-first `src/lib.rs` | not archived | **21 compile errors** (`E0422`/`E0425`/`E0433`: `State`, `place_of`, `take_args`, `belt_slot` did not exist) |
| runtime + glue tests | `cargo test` | `sdk-test.log` | **16 passed**, 0 failed, `TEST_EXIT:0` (10 runtime + 6 glue) |
| the glue tests fail when the mapping is wrong | `place_of(40)` flipped to `Some(40)`, re-run | `sdk-glue-test-red.log` | **FAILED**, `left: Some(40)`, `right: Some(-106)`, `TEST_EXIT:101`; restored, green again |
| formatting | `cargo fmt --check` | `sdk-fmt.log` | `FMT_EXIT:0` |
| lints | `cargo clippy --all-targets -- -D warnings` | `sdk-clippy.log` | `CLIPPY_EXIT:0`, 0 warnings |
| build | `cargo horizon-mod build` (after `rm -f Cargo.lock`) | `sdk-build.log` | `Built ./target/horizon-mod/toolbelt.hmod`, `BUILD_EXIT:0` |
| registry check | `cargo horizon-mod check toolbelt.hmod` (also once through `build-slot.sh run --job sdk-tb --step check`, slot 5, released exit 0) | `sdk-check.log`, `sdk-check-json.log`, `sdk-check-slot.log` | `toolbelt 0.2.0 (native-mod, format native-1/0b14f8a/e59da90, for 26.3)`, **40,939 B**, sha256 `15aee383…eedd`, **CHECK_EXIT:0** |
| fits | `cargo horizon-mod fits toolbelt.hmod --build nightly` | `sdk-fits.log` | `0b14f8a  made for`, `FITS_EXIT:0` |
| surface | `unzip -p toolbelt.hmod uses.json` | `sdk-uses.json` | 298 entries; the hooks, `capability:horizon:entity_items`, `capability:registries`, `macro:horizon_mod`, `macro:horizon_part`, and the host methods listed above |
| package hash | `sha256sum toolbelt.hmod` | `sdk-sha256.txt` | `15aee383066f320bcfa0196255aecc00981fc180a7fe0432cb7e74bd0da0eedd` |
| reproducible | two more `cargo horizon-mod build` runs | — | identical bytes and sha256 each time |
| fresh clone | `.horizon/`, `target/` and `Cargo.lock` removed, then build | — | SDK fetched by itself (`SDK 0b14f8a … in ./.horizon/sdk`), same 40,939 B, same sha256 |

**Deliverable:** `toolbelt.hmod` **40,939 B**
`15aee383066f320bcfa0196255aecc00981fc180a7fe0432cb7e74bd0da0eedd`, **30
entries** (133,092 B unpacked): `horizon-mod.toml`, `Cargo.toml`, `LICENSE`,
`LICENSE-NOTICE.md`, `README.md`, `src/lib.rs`, `src/runtime.rs`, `uses.json`,
20 `assets/toolbelt/**`, 2 `data/toolbelt/**`.

### 5. Gaps after the migration

- **No in-game run of the SDK package.** There is no `horizon-launcher` on this
  box, so the flat package cannot be compiled into a game binary
  (`horizon-launcher native-build <port> --instance <i> --trust-native-code`) and
  `tools/e2e.sh` (which drives the retired staged tree) cannot run. The previous
  slice's fail-capable e2e is what proves the *behaviour* — see below — and this
  job's evidence is the byte-identical runtime, the glue tests, and
  `check`/`fits` on the exact pinned build. **First item of the next slice: a
  launcher-built e2e of this package**, including the two click paths
  (`right_click_block` then `use_item`), the menu refresh and the sneak extract.
- The retired e2e read the belt through `/toolbelt info`; that readout is
  unchanged by the migration (`toolbelt:belt size=… slots=…/… contents=[…]`), so
  the same scenario still has its readout — once a game can be built.
- The rest of the behaviour gaps are unchanged from slice 1 (see Gaps above).
- `MenuItem`-level rules: the native menu still cannot refuse a stackable item
  per slot, so a click into a belt slot is ignored rather than "refused with a
  message" as the upstream screen does.

### 6. Traps this migration hit (the next port must know)

1. **`use_on` is not the old `use_item_on`.** At this pin the item's `use_on`
   runs *after* the block's own use, so a chest opens instead of the belt
   acting; `right_click_block` is the hook that runs before both and can cancel.
   Read the hook's own doc before mapping a retired `use_item_on`.
2. **`use_item`/`use_on` are `ItemPart` methods, not `Hooks` methods.** They do
   not appear on the `Hooks` trait at all: an item's use behaviour is a part
   (`#[horizon_part] impl ItemPart for …`) attached in `Hooks::register` with
   `registrar.item_key(id)` + `registrar.item_part(key, part)`. Only then does
   the item's event reach the mod.
3. **`right_click_block` takes `HostSide`, not `&mut dyn ServerHost`** (the
   engine fires it on both sides), so the method starts with
   `let HostSide::Server(host) = host else { return Flow::Continue }`. A
   `&mut dyn ServerHost` parameter is `E0053`.
4. **`use_item`/`use_on` take `HostSide` too** and return
   `Option<InteractionResult>`; the retired `&mut bool claimed` is gone.
5. **The build refuses anything outside the package schema**, naming the path
   (`error: unexpected package path: …`): the retired `mixins/`, `content/`,
   `compatibility.json` and `rust-toolchain.toml` must be **deleted**, not kept
   beside the new tree.
6. **A leftover `content/` would have been refused**: a content half is
   `assets/<id>/` + `data/<id>/` at the package root, not a nested `.hmod`.
7. **`Cargo.lock` is packed and changes the sha256**; `cargo test`/`clippy`
   write one. `rm -f Cargo.lock` right before `cargo horizon-mod build`, and
   `/Cargo.lock` in `.gitignore`. Verified: no `Cargo.lock` in this package.
8. **`cargo fmt --check`, not `--all`**: the SDK is a path dependency, and
   `--all` reports *its* files (two `build.rs` diffs).
9. **`JavaString` is not exported by `mod_api`.** `Compound` is
   `BTreeMap<JavaString, Tag>`, so `Compound::get(&JavaString)` needs a keyed
   type you cannot name; read with `Tag::get(&str)` on the tag itself, write
   with `entry.insert(KEY.into(), tag)`, and drop a key with
   `Compound::retain(|key, _| key.to_string_lossy() != KEY)`.
10. **`ItemStack.components()` is the patch *without* damage** (damage is a
    scalar field). A merge fingerprint built from it alone would merge a damaged
    tool with a fresh one, so `variant_of` hashes `damage_component()` first and
    then the patch (`Tag`/`BTreeMap` both hash by value).
11. **The two inventory numberings bite again.** `PlayerRef::inventory_slots`
    numbers the armour 36..=39 and the off hand 40, while
    `ItemPlace::PlayerSlot` wants 100..=103 and **`-106`**; only 0..=35 share a
    number. `place_of` maps them and has a test that fails when it is wrong.
12. **`PlayerRef::selected_slot` is the hotbar index (0..8)**, which is also the
    `Inventory` slot of a held hotbar stack — that is the slot the menu
    remembers, as the retired `held_slot()` did.
13. **Writing an inventory slot is `ItemPlace::edit`, in a transaction.** It
    keeps item and count and journals the change; `exchange` cannot carry a
    component-only change (the old resource no longer matches), and the closure
    cannot call the host, so build the new stack *before* the call.
14. **A hand write is the game's equip write** (`LivingRef::set_hand_item`),
    which syncs the client; the retired `hub.send_inventory` resync is gone.
    Its `Err`/`Ok(false)` means the game refused: the belt write paths put the
    tool back into the player instead of duplicating it.
15. **Never hold the state lock across a host call.** `open_menu` can close
    another menu and fire `menu_closed` on the same mod, which would deadlock a
    held lock; every critical section here is map work only (`locked()`).
16. **`ServerTickPhase` is `#[non_exhaustive]`**: keep the wildcard arm, and do
    not treat `ServerTickStart` as the end of a tick.
17. **The package's root files are whitelisted and packed**:
    `horizon-mod.toml`, `Cargo.toml`, `Cargo.lock`, `LICENSE`,
    `LICENSE-NOTICE.md`, `README.md`, `icon.png`, `src/`, `assets/`, `data/`,
    `uses.json`. `README.md` therefore ships — a template README left in place
    becomes the mod's page — and `notes.md`/`FINDINGS.md`/`VALIDATION.md`/`tools/`
    do not.
18. **`build-slot.sh` does not speak this layout** (it runs the retired
    `tools/horizon.py` ladder): run the `cargo horizon-mod` commands directly in
    the port with `CARGO_INCREMENTAL=0`, and the brief's
    `build-slot.sh run --job … --port … --step check -- bash -c 'cd ports/<id> && cargo horizon-mod check <file>.hmod'`
    when a slot lease is wanted.
19. **A 38-character pin reads as missing**: both `[native]` revisions must be
    the exact 40 hex from `sdk.json`, and the mixins commit is `e59da909…`, not
    the old campaign pin.

## Slice 2 — the belt upgrade: the pouch and the anvil (0.2.2)

**The gap line this closes**, from `## Slice boundary` above: *"anvil/sewing upgrades;
the pouch item and the dyed belt"*. It is one family, not three: the jar's pouch exists
**only** as the anvil's upgrade material and as a crafted ingredient
(`ToolBelt.POUCH = ITEMS.registerSimpleItem("pouch")`), and the anvil is the only
upgrade path that does not need another mod (the seven `belt_upgrade_*_via_sewing`
recipes carry `neoforge:mod_loaded sewingkit`, which is not in the registry). Tier
`machines`: a carried item, its recipe and a menu mechanic, no renderer and no entity.

Read from upstream (`gigaherz/ToolBelt` at `2737d42`, `ToolBelt.java` and
`belt/ToolBeltItem.java`):

| Rule | Upstream | This port |
|---|---|---|
| inputs | left item is `BELT`, right item is `POUCH` | same |
| output | `makeUpgradedStack(left)`: the belt with `belt_size + 1` | the left stack cloned, `toolbelt:belt_size` raised |
| level cost | `XP_COSTS = {3, 5, 8, 12, 15, 20, 30}` for 2→3 … 8→9 | `UPGRADE_XP_COSTS`, same table |
| material cost | `setMaterialCost(1)` (one pouch) | `material_cost = 1` |
| nine slots | `getUpgradeXP` answers −1 and the event is cancelled | `upgrade_cost(9) == None`, the hook answers `Flow::Cancel` |
| contents | kept (the copy keeps its container) | kept: only the size component changes |
| config | `enableAnvilUpgrading` (AUTO: on without the sewing kit) | **always on** — the config family is still a gap |

### TDD — red, then green

The pure rules landed first as tests in `src/runtime.rs`
(`an_upgrade_costs_the_jars_seven_level_steps`,
`one_upgrade_adds_one_slot_and_never_past_nine`, `only_the_pouch_upgrades_a_belt`).
Before the implementation existed the run failed to compile:

```
error[E0425]: cannot find value `POUCH_ITEM` in this scope
error[E0425]: cannot find function `upgrade_cost` in this scope
error[E0425]: cannot find function `upgraded_size` in this scope
error[E0425]: cannot find function `is_upgrade_material` in this scope
…
error: could not compile `toolbelt` (lib test) due to 18 previous errors
```

(`../_shared/evidence/toolbelt/upgrade-test-red.log`.) Then the engine
(`upgrade_cost`, `upgraded_size`, `is_upgrade_material`, `POUCH_ITEM`) and the
`anvil_update` hook, and `cargo test`: **20 passed, 0 failed** (`sdk-test.log`
quotes two `test result: ok.` lines: 20 library tests and the 0-test binary).

### The hook

`Hooks::anvil_update` (stamped, `hook:anvil_update`, side both) answers on both sides
with the same pure rule: the belt's size and a pouch in the right input make the left
stack with one more slot, `xp_cost` from the table and `material_cost = 1`; a nine-slot
belt answers `Flow::Cancel`, so no result. Both sides answer because the event fires on
both — the server's menu and the client's copy must agree.

### A `uses.json` name collision, fixed at the source of the record

`cargo horizon-mod check` warned that `type:Item` was "built field by field", and
`fits --build 0b14f8a` refused the package (`does not fit: 1 item changed`). Root cause is
the same class `ports/artifacts` recorded on 2026-11-20: `uses.json` is a **source-text
scan** that resolves a bare type name against the SDK surface, and our
`CommandArgumentValue::Item(..)` pattern in `action_of` was recorded as a build of the
SDK's content-declaration type `Item` — a type this port never touches. Renaming the SDK
variant is not possible, and an `as` alias on the import is resolved back to the variant
(measured: the record stayed), so the one match lives in a small `item_argument!` macro
(the scan does not expand macros) with a comment saying exactly why. `uses.json` is now
truthful: no `type:Item` entry, and `fits --build 0b14f8a` prints `fits`
(`sdk-fits-other.log`).

### Slice 2 ladder (pin `0a875da`, mixins `e59da90`)

Skills: superpowers/using-superpowers, brainstorming, writing-plans, test-driven-development, systematic-debugging, verification-before-completion; BeautifulRust/rust-best-practices

Every step ran through `ports/tools/build-slot.sh` with `CARGO_INCREMENTAL=0` and
`BUILD_LOCAL=1` (the SDK fetch and the launcher build are local).

| Step | Command | Result | Log |
|---|---|---|---|
| red | `cargo test` with the three new tests only | **18 × `E0425`**, `could not compile` | `upgrade-test-red.log` |
| tests | `cargo test --offline` | `test result: ok. 20 passed; 0 failed` + `TEST_EXIT:0` | `sdk-test.log` |
| lints | `cargo clippy --all-targets --offline -- -D warnings` | `CLIPPY_EXIT:0`, no warning | `sdk-clippy.log` |
| format | `cargo fmt && cargo fmt --check` | `FMT_EXIT:0` | `sdk-fmt.log` |
| build ×2 | `rm -f Cargo.lock && cargo horizon-mod build` | both `BUILD_EXIT:0`, `cmp` identical | `sdk-build.log`, `sdk-build-second.log` |
| registry check | `cargo horizon-mod check toolbelt.hmod` | `48303 bytes, SHA-256 1eb80603…aa20b`, `CHECK_EXIT:0` | `sdk-check.log` |
| fits, pin | `cargo horizon-mod fits toolbelt.hmod --build 0a875da` | `0a875da  made for` | `sdk-fits.log` |
| fits, other | `… --build 0b14f8a` | `0b14f8a  fits`, `FITS_EXIT:0` | `sdk-fits-other.log` |
| gate | `python3 ports/tools/verify-sdk-package.py toolbelt` | `1/1 package(s) verified` | — |
| e2e | `build-slot.sh run --step e2e -- tools/e2e.sh` | `PASS` + `EXIT:0` | `toolbelt-e2e/e2e-launcher-0.2.2.log` |
| e2e control | `GAME=$CONTROL_GAME …` | `FAIL: a new belt is not size 2` + `EXIT:1` | `toolbelt-e2e/e2e-launcher-control-0.2.2.log` |
| e2e selftest | `SELFTEST=1 …` | `FAIL: SELFTEST: … (the check is alive)` + `EXIT:1` | `toolbelt-e2e/e2e-selftest-0.2.2.log` |
| log gate | `check-e2e-logs.py toolbelt-e2e --strict` | `logs=5 ok=5 missing-exit=0 bad-exit=0` | — |

The package: **48,303 B**, SHA-256
`1eb8060368a9f676606d19672b42367aae220502ea62c81e8209f32ac89aa20b`, **35 entries**
(`horizon-mod.toml`, `Cargo.toml`, `LICENSE`, `LICENSE-NOTICE.md`, `README.md`,
`src/lib.rs`, `src/runtime.rs`, `uses.json`, 23 `assets/toolbelt/**`, 4
`data/toolbelt/**`); **no `Cargo.lock`**; `uses.json` 331 entries, `made_for 0a875da`.
The two builds above were run after the in-game leg; the only change between the
package the leg played and the delivered bytes is `README.md` (documentation, packed but
not code), and a third build reproduced these exact bytes from the current sources.

### Files

| Action | Path |
|---|---|
| created | `data/toolbelt/horizon/item/pouch.json` (stick template, cooked-fuel removed: a plain 64-stack item) |
| created | `assets/toolbelt/items/pouch.json`, `assets/toolbelt/models/item/pouch.json` (byte-identical to the jar), `assets/toolbelt/textures/item/pouch.png` (byte-identical, 17,670 B) |
| created | `data/toolbelt/recipe/pouch.json` — the jar's `"sgs" / "l l" / "sls"` shaped recipe, its `neoforge:conditions` dropped like the belt's |
| edited | `src/runtime.rs` (the three rules), `src/lib.rs` (the pouch key, `anvil_update`, `anvil_upgrade`, the `item_argument!` macro), `horizon-mod.toml`/`Cargo.toml` (`0.2.2`, `[native] horizon = 0a875da…`) |
| created | `tools/craft_check.py` (both recipes + the belt pattern without its iron), `tools/anvil_check.py` (the eight steps + two controls) |
| edited | `tools/e2e.sh` (the `/toolbelt help` leg, the recipe leg, the anvil leg; `CTL` now comes from `launcher-game.sh env`) |

### In game (launcher-built package)

`PASS` + `EXIT:0`, `../_shared/evidence/toolbelt-e2e/e2e-launcher-0.2.2.log`. Verbatim:

```
help lists: 5 usages
belt recipe: 1 toolbelt:belt, 0 ingredients left
pouch recipe: 1 toolbelt:pouch, 0 ingredients left
belt pattern without iron: null
belt + leather -> result None, cost 0
step 2->3: result 'toolbelt:belt x1', cost 3, level 200
  taken: belt at slot 8 size 3, pouches 8->7, level 200->197
…
step 8->9: result 'toolbelt:belt x1', cost 30, level 137
  taken: belt at slot 8 size 9, pouches 2->1, level 137->107
nine-slot belt + pouch -> result None, cost 0, pouches 1->1
final readout: toolbelt:belt size=9 slots=1/9 contents=[0:minecraft:iron_pickaxe(1)]
anvil PASS: a pouch takes a belt from two to nine slots at the jar's seven level steps, one pouch each, with the stored tool kept; leather and a nine-slot belt produce no result
```

Every assertion can fail: the leather leg is the same code path with the wrong second
item, the nine-slot leg is the table's missing tenth step, each level drop is checked
against the jar's own distinct step (a constant cost fails at step 2→3), and the tool
inside the belt is checked after every step (an upgrade that rebuilt the stack would lose
it). The control game (`GAME=$CONTROL_GAME`) fails at the first readout —
`FAIL: a new belt is not size 2` + `EXIT:1` (`e2e-launcher-control-0.2.2.log`) — and
`SELFTEST=1` fails as designed (`e2e-selftest-0.2.2.log`). `check-e2e-logs.py
toolbelt-e2e --strict`: `logs=5 ok=5 missing-exit=0 bad-exit=0`.

Two harness facts this leg needed (they cost four probe runs):

- **`ctl interact` with the belt in hand never opens the anvil**: the belt's own
  `right_click_block` takes the click and opens the belt menu. Select another hotbar
  slot first, or the "anvil" is really a 9-slot belt menu (its slot 300 is belt slot 0).
- **An anvil opened by `interact` is a station** (`ctl menu` → `"station": "Anvil"`) with
  inputs at **600/601** and the result at **650**; `click-slot` on the container ids
  300.. does nothing for it. Shift-click (`click-slot N shift`) is the path that moves
  the belt and the pouch in and the result out; the result slot's `components` (read
  with `ctl screen`) carry `toolbelt:belt_size`, which is what the e2e asserts before
  taking it.

## Coverage — every family in the jar

| Family (jar) | State | Where |
|---|---|---|
| `toolbelt:belt` item, `belt_size` 2..9 + contents components | tested | `e2e-launcher-0.2.2.log` (readout size/slots/contents legs) |
| insert rule (only non-stackables, no belt/bundle/container) | tested | stone and shulker-box legs; `runtime.rs` unit tests |
| capacity/merge/extract + full-inventory safety | tested | capacity leg, "extraction that does not fit" leg |
| belt menu (list + click-to-take) | tested | menu leg |
| off-hand right-click insert, sneak extract, click-to-open | tested | off-hand leg (sneak extract is the same `clicked` path, unit-tested) |
| `/toolbelt put take info menu help` | tested | help (5 usages), put/take/info/menu legs |
| belt recipe (shaped) | tested | `craft_check.py` |
| `toolbelt:pouch` item + recipe | tested | `craft_check.py` |
| anvil upgrade (2→9, XP table, one pouch, contents kept) | tested | `anvil_check.py` (leather and nine-slot controls) |
| sewing-kit upgrade recipes | not ported | needs `sewingkit`, not in the registry |
| radial menu + `key.toolbelt.*` | blocked(no client→server action helper at this pin) | ARCHITECTURE §3 gap table |
| wearing the belt (Curios / own belt slot, `onWornTick`) | not ported | needs an accessory-slot API and the renderer |
| `IItemInBelt` per-item tick | not ported | no item in this mod implements it; `ItemPart::inventory_tick` is the future shape |
| dyed belt (dye component model override) | not ported | model/texture carried only for the belt's own model override |
| custom screen background, player-layer renderer | not ported | client renderer; tier boundary |
| tooltips (`text.toolbelt.tooltip`) | not ported | `ItemPart::tooltip` exists at the pin; client-side, unprovable through this harness |
| server config: whitelist/blacklist, `allowAllNonStackableItems`, `anvilUpgrading` | not ported | needs `config_files()`/settings; today the port behaves as the AUTO defaults without the sewing kit |
| hopper/item-capability (`Capabilities.Item`) | not ported | nothing to attach it to without the worn belt |
| sounds, particles, creative-tab membership | not ported | cosmetic; `give`/recipes are the tested paths |

## Verification (the retired 0.1.0 slice, still the behaviour's evidence)

Skills: superpowers/using-superpowers,writing-plans,test-driven-development,systematic-debugging,verification-before-completion; BeautifulRust/rust-best-practices,rust-port

Pin `1d63e0a5a37a7a708eb0b32b49bf0dfeb28ed8a9` / horizon-mixins
`5a64af938ec86b00d53d16a408aabc0038fb3e57` (schema 6). Every build/test/e2e
step ran through `ports/tools/build-slot.sh` with `CARGO_INCREMENTAL=0`.
Evidence: [`../_shared/evidence/toolbelt/`](../_shared/evidence/toolbelt/).

| Step | Command | Result |
|---|---|---|
| runtime tests, red | `cargo test --locked --offline` before the implementation | **0 passed, 10 failed** (`not yet implemented`) |
| runtime tests | `cargo test --locked --offline` | **10 passed**, 0 failed |
| fmt | `cargo fmt --all --check` | exit 0 |
| clippy | `cargo clippy --all-targets --locked --offline -- -D warnings` | clean, **0 warnings** |
| stage | `python3 tools/horizon.py stage --horizon …/horizon-1d63e0a` | `status: ok`, **2 modules, 8 injections** |
| check | `python3 tools/horizon.py check --horizon …` | `cargo_exit: 0`, **0 warnings** (`horizon-check.log`) |
| embedded tests | `python3 tools/horizon.py test --horizon …` | **10 passed; 0 failed; 1240 filtered out** (`horizon-test.log`) |
| fast build + e2e | `build-slot.sh e2e --job tb-1d63e0a-e2e … -- tools/e2e.sh` | runner **exit 0**; `e2e-default.log` ends `PASS: the belt stores tools…` / **`EXIT:0`** |
| e2e selftest | `build-slot.sh e2e --job tb-1d63e0a-selftest … -- env SELFTEST=1 tools/e2e.sh` | runner **exit 1**; `e2e-selftest.log` ends `FAIL: SELFTEST: the no-stackable control did not put the stone in the belt (the check is alive)` / **`EXIT:1`** |
| content carry | `python3 tools/carry_content.py --check` | **23 files, 0 pending**, jar SHA-1 `96862e5d…` |
| content `.hmod` | `cargo-horizon-mod build` in `content/` | exit 0, **14,384 B**, SHA-256 `1a1104f2…867c` (the copy embedded in the 0.1.0 `.hnmod`); the later rebuild with the licence notices inside is 16,060 B `dd21e2c0…a99f` |
| content check | `cargo-horizon-mod check toolbelt.hmod` | exit 0, `toolbelt 2.2.10 (mod, API 0.2, both)`, **"no code"** |
| content check, negative | the same build with the manifest id `ToolBelt` | **exit 1**, `id "ToolBelt" is not [a-z0-9_]{1,64}` |
| native `.hnmod` | `python3 tools/package.py` → `toolbelt.hnmod` | **30,900 B**, SHA-256 `58a9895f…471d`, 14 entries, pins `horizon`/`mixins` |
| `.hmod` loads | the same game with `--dev-mod content/` | give + belt storage worked (see transcript) |
| `.hmod` negative | the same world **without** `--dev-mod` | `give toolbelt:belt` added nothing; `/toolbelt` answered nothing |

E2E transcript (abridged from `e2e-default.log`; every line is a `ctl` reply or
an assertion):

```
readout: toolbelt:belt size=2 slots=0/2 contents=[]
readout: toolbelt:belt size=2 slots=1/2 contents=[0:minecraft:iron_pickaxe(1)]
readout: toolbelt:belt size=2 slots=0/2 contents=[]
readout: toolbelt:belt size=2 slots=0/2 contents=[]            # stone refused
readout: toolbelt:belt size=2 slots=2/2 contents=[0:minecraft:iron_pickaxe(1),1:minecraft:diamond_sword(1)]
readout: toolbelt:belt size=2 slots=2/2 contents=[0:minecraft:iron_pickaxe(1),1:minecraft:diamond_sword(1)]  # axe refused
menu slots: 300 iron_pickaxe, 301 diamond_sword
click-slot 300 -> readout: toolbelt:belt size=2 slots=1/2 contents=[1:minecraft:diamond_sword(1)]
right-click with an off-hand shovel -> readout: toolbelt:belt size=2 slots=1/2 contents=[0:minecraft:iron_shovel(1)]
offhand after: none
without --dev-mod: give toolbelt:belt added nothing
without --dev-mod: /toolbelt answered nothing
PASS: the belt stores tools, refuses stackables and overflow, shows them in its menu,
      gives one back on a click, takes an off-hand tool on a right-click, and registers
      nothing without --dev-mod
EXIT:0
```

Controls that make it fail-capable: a stackable stone and a third tool are
refused (the belt/inventory counts stay put), the no-mod world registers no
item and no command, and `SELFTEST=1` asserts the impossible and exits 1.

## Retired format notes (kept for the record)

### Embedded content — one `.hnmod` deliverable (2026-11-10, retired 2026-11-18)

The retired bundle carried its content half **inside** the native bundle at
`mixins/toolbelt/data/toolbelt-content.hmod`; `embedded_content.rs` wrote it to
`.horizon-cache/` at start and appended it to the dev list from one
`content_root` HEAD injection. One `.hnmod` installed both halves. **The SDK
format retires this**: a flat `.hmod` carries `assets/` + `data/` itself, so the
injection, the embedded copy and `tools/package.py`'s equality check are gone
(see the migration table). Its verification — `EMBEDDED=1` e2e `PASS`/`EXIT:0`,
the unusable-`.horizon-cache` control `EXIT:0`, the `SELFTEST=1` fail leg
`EXIT:1`, and `check-e2e-logs.py toolbelt --strict` green — is in
`evidence/toolbelt/` (`e2e.log`, `e2e-control.log`, `e2e-selftest.log`).

### Review repairs (PR #573, 2026-11-11, carried into the new glue)

*(The retired submission was PR #573 — `_shared/HANDOVER.md` rows 375 and 421,
and this section, which is why the resubmission's changelog supersedes #573.)*

1. Staging is the orchestrator's (horizon-mods#686); the reviewer's own
   follow-up found the exact pinned file in the source repository, so the
   intended bytes are known.
2. **[P1] "Extraction permanently deletes tools when inventory is full"** —
   **fixed**: every extraction path (the sneak click, `/toolbelt take`, the menu
   click) gives the stack first and removes only what the player actually
   accepted, so a full inventory leaves the tool in the belt. In the SDK glue
   that is `give_to_player`'s returned count.
3. **[P1] "`/toolbelt put` destroys item components and resets durability"** —
   **fixed**: `put` moves the real inventory stacks (damage and components
   included, through `ItemPlace::PlayerSlot::extract`) into the belt and gives a
   refused stack back unchanged instead of manufacturing a fresh one.
4. **[P2] "Ship the required BSD notice"** — `LICENSE` and
   `LICENSE-NOTICE.md` travel at the package root, which the packer includes.

## Class + tier, approach, licence (recorded)

Class **content**, tier **machines**; the shape chosen is the tier's (carried
belt content + an on-item container + a menu + item components), not a plain
data item and not a deep entity/renderer port. Licence **BSD-3-Clause → rank 0**
(permissive); local-only until the maintainer approves publishing.

## Changelog for the resubmission (verbatim)

*"Supersedes #573, closed because Horizon retired `.hnmod` (#17992). Changes:
flat SDK package with `api = "0.2"` and `[native]` pinned to `0b14f8a`;
the eight retired injections are now `impl mod_api::Hooks` methods
(`component_types`, `server_tick`, `menu_clicked`, `menu_closed`,
`register_commands`, `run_mod_command`, `right_click_block`) plus one
`ItemPart` for the belt's own `use_item`, attached by a new `register` hook; the
`content_root` embedded-content injection is gone (one package carries its own
`assets/` and `data/`); the belt's slots are read and written through the host's
item components, `ENTITY_ITEMS`/`ItemPlace` for the player's inventory and the
native menu helpers (`open_menu`, `menu_set_items`, `close_menu`); the menu now
opens from the command directly and from a click at the end of the tick;
`runtime.rs` and its 10 tests unchanged. `/toolbelt put|take|info|menu|help`
keep their text and permission level; the parsed arguments are typed
(`item`/`integer`), so a negative count answers "Count must be at least 1."
instead of inserting one."*

## Files

- `horizon-mod.toml` — the only manifest: `id = "toolbelt"`, `version = "0.2.2"`,
  `api = "0.2"`, `side = "both"`, `[native]` pinned to `0a875da` / `e59da90`.
- `Cargo.toml` — one crate; the only dependency is `horizon-mod-api` from
  `.horizon/sdk/`.
- `src/lib.rs` — the one `#[mod_api::horizon_mod] impl Hooks` (11 hooks, `anvil_update`
  among them) and the belt's `ItemPart`, with 6 glue tests.
- `src/runtime.rs` — the pure engine with its 14 tests (the retired 10 plus the
  container-bearing rule and the three upgrade rules).
- `assets/toolbelt/**` (23 files), `data/toolbelt/**` (4 files) — the carried content
  (the pouch's model, definition, texture, recipe and lang are part of it).
- `LICENSE`, `LICENSE-NOTICE.md`, `README.md` — inside the package.
- `tools/e2e.sh`, `tools/craft_check.py`, `tools/anvil_check.py` — the in-game
  scenario and its two recipe/menu drivers (they need a launcher-built game).
- `toolbelt.hmod` — the delivered package, **48,303 B**, SHA-256
  `1eb8060368a9f676606d19672b42367aae220502ea62c81e8209f32ac89aa20b`, 35 entries.
- `FINDINGS.md`, `VALIDATION.md`, this file.
- Evidence: `../_shared/evidence/toolbelt/` (`sdk-*.log`, `sdk-uses.json`,
  `sdk-sha256.txt`, `upgrade-test-red.log`, `sdk-fresh-clone.md` for determinism and
  the fresh-clone run, the retired 0.1.0 tree under `retired-hnmod-0.1.0/`) and
  `../_shared/evidence/toolbelt-e2e/` (`e2e-launcher-0.2.2.log`,
  `e2e-launcher-control-0.2.2.log`, `e2e-selftest-0.2.2.log`). Jar kept at
  `../_shared/licence-cache/ToolBelt-1.21.1-2.2.10.jar`.

## Review fixes (2026-11-18, PR #1206)

The reviewer found the nesting check refused belts and bundles but accepted **shulker boxes** as
ordinary non-stackable items, while Horizon lets a belt sit in a shulker box: ordinary players could
build unbounded belt-box-belt nesting.

- **Fix.** `BeltStack` gained `container_bearing`, set by `carries_a_container` from the item id
  (`*_shulker_box`, bundle), the item's default `minecraft:container` component (every shulker box
  declares it) and the stack's own `minecraft:container`/`minecraft:bundle_contents` patch;
  `allows` refuses such a stack. The belt can no longer hold any container-bearing item.
- **Test.** `a_container_bearing_stack_never_enters_a_belt` (permissive policy still refuses a
  shulker box and a bundle; a plain tool still fits).
- **In-game.** A shulker box is offered to the belt and refused (still in the inventory, readout
  unchanged). Evidence: `../_shared/evidence/toolbelt-e2e/e2e-launcher.log` (`PASS` + `EXIT:0`) and
  `e2e-launcher-control.log` (`EXIT:1`); checker green.
- **Runtime file.** The pure engine gained `container_bearing`; its sha256 is now
  `e6ed170244c7e46e112dfc406efe5abe9ef8efb259ef29be8674226706ceba78` (the retired bundle's file was
  `9ba28129…`).
- **Package.** `toolbelt.hmod`, **0.2.1**, 42,104 B, sha256
  `966749f4998da4cc37eaaca0ccef6784bcf6e50ff5160e02ee6fc458f8242079`; two builds byte-identical,
  `check` and `fits --build 0b14f8a` exit 0.

Skills: superpowers/using-superpowers, brainstorming, writing-plans, test-driven-development, systematic-debugging, verification-before-completion; BeautifulRust/rust-best-practices, rust-port, rust-review
