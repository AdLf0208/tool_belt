# Tool Belt — upstream findings (slice 1)

Ported from **Tool Belt 2.2.10** (exact ATM10 jar, BSD-3-Clause; provenance
`../../_shared/carry-work/toolbelt/SOURCE.md`). Behaviour read from the
public repository `gigaherz/ToolBelt`, commit
`2737d42e947d96745450cfb066db0b0c60549bbe` (2026-06-26). The newer source
(26.2) keeps the same storage model as 1.21.1: one belt item with a
`belt_size` component and an item-container component. This is a
clean-room rewrite; no code was translated.

## Storage shape

- The belt is **one item**, `toolbelt:belt`, `stacksTo(1)`
  (`ToolBelt.java`: `ITEMS.registerItem("belt", props -> new ToolBeltItem(props.stacksTo(1)))`).
- Two pieces of state live **on the stack** (upstream `ToolBeltItem`):
  - `toolbelt:belt_size` — an `int`, clamped to **2..=9**
    (`getBeltSize`/`setBeltSize`; a missing value reads as 2).
  - the item's container component — `ItemContainerContents` with exactly
    `belt_size` slots, i.e. a list of item stacks kept inside the belt.
- Changing the size keeps the first `min(old,new)` slots and drops the rest
  (`setBeltSize` rebuilds the contents list).
- The slot capacity is a normal slot's: an item's `max_stack_size` per slot
  (upstream's wrapper reports `getMaxStackSize() = 64`, while the vanilla
  slot/stack rule still caps at the item's own max).
- The belt's own inventory slot is **locked** in the menu (`LockedSlot`), so
  the belt cannot be moved while its own menu is open.

## Insertion rules (`ConfigData.isItemStackAllowed`)

1. A stack with count <= 0 is allowed (empty).
2. A **whitelist** entry (server config `general.whitelist`, item ids) allows
   the item; a **blacklist** entry (same shape) refuses it — the whitelist
   wins.
3. `BundleItem`s are always refused.
4. Other `ToolBeltItem`s are always refused (no belt inside a belt).
5. Default (`allowAllNonStackableItems = true`): **only non-stackable items**
   are allowed (`stack.getMaxStackSize() == 1`). With the option false, only
   the configured lists let anything in.
   - Config defaults: `whitelist = []`, `blacklist = []`,
     `allowAllNonStackableItems = true`.

## Key / click flow

- **Right-click** with the belt in the main hand, in the air (`use`) or on a
  block (`useOn`), opens the belt menu (`Screens.openBeltScreen` →
  `BeltContainer`), which shows the belt's slots above the player inventory.
  The menu opens only for a stack instance that is still found in the player
  inventory (`getSlotFor`), and `stillValid` compares the held stack's item
  and components.
- The menu supports vanilla clicks and shift-clicks between the belt slots
  and the player inventory; `quickMoveStack` moves belt→inventory and back.
- A **client keybind**, default `R` (`key.toolbelt.open`, "Swap Tool"), opens
  a **radial menu** (`RadialMenuScreen`) to swap a belt tool with a hotbar
  slot. `key.toolbelt.slot` opens the belt slot inventory; the client also
  cycles tools with two extra keys. `releaseToSwap` (default false) makes
  releasing R perform the swap. Its network messages carry the swap; none of
  this is server-side state.
- Worn on the player: the belt is put in a **Curios belt slot** when Curios
  is present, or in the mod's **own belt slot** (`BeltAttachment`, a custom
  capability; `customBeltSlotMode` default `ON`). The belt also ticks while
  carried in the inventory (`inventoryTick` → `onWornTick` for each stored
  item that implements `IItemInBelt`). `IBeltSlotItem` items get an extra
  `onWornTick` from the worn attachment.
- `BeltFinder` locates the belt in: the Curios belt slot, the mod's belt slot,
  or (debug) a fixed slot; `PlayerEvent.StartTracking` re-syncs a player's
  belt so other players render it. Rendering (`ToolBeltLayer`) is client-only.

## Config defaults (upstream)

| Side | Key | Default |
|---|---|---|
| server | `general.whitelist` | `[]` |
| server | `general.blacklist` | `[]` |
| server | `general.allowAllNonStackableItems` | `true` |
| common | `general.enableSewingKitSupport` | `true` |
| common | `general.anvilUpgrading` | `AUTO` |
| common | `general.enableGridCraftingRecipes` | `AUTO` |
| common | `general.customBeltSlotMode` | `ON` |
| client | `display.showBeltOnPlayers` | `true` |
| client | `display.beltItemScale` | `0.5` (0.1..2.0) |
| client | `menu.releaseToSwap` | `false` |
| client | `menu.clipMouseToCircle` | `false` |
| client | `menu.clickOutsideBounds` | `false` |
| client | `menu.displayEmptySlots` | `false` |
| client | `menu.minecraftHasNoCircles` | `false` |

## What this slice takes, and what it does not

- **In:** the belt item, its `belt_size` + contents components, the 2..9-slot
  storage, insert/extract with the default non-stackable rule, a belt menu
  driven by vanilla clicks, and a `/toolbelt` command fallback for headless
  checks.
- **Out (recorded gaps):** the radial swap menu and its keybinds (client→server
  packet path not present as a stamped surface here), the Curios/custom belt
  slot and wearing, the anvil/sewing upgrades and recipes, the pouch and
  dyed belt, the custom GUI texture/player layer renderer, the server
  whitelist/blacklist config, and `IItemInBelt` per-item ticks.
