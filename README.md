# Tool Belt for Horizon

A port of **Tool Belt 2.2.10** (`gigaherz/ToolBelt`, BSD-3-Clause) to Horizon
26.3. One flat `.hmod` carries the belt item, its model/texture/lang/recipe and
the native code that stores tools in it.

## What it does

- `toolbelt:belt` — a `max_stack_size: 1` item with two to nine slots of its
  own, held in `toolbelt:belt_size` and `toolbelt:belt_contents` on the stack,
  so a saved, thrown or picked-up belt keeps everything it holds.
- **Right click** with an allowed **off-hand** stack inserts it; while
  **sneaking** it moves the first stored tool back to the inventory; otherwise
  it opens the belt menu, whose slots give a stack back on a click.
- The default rule is upstream's `ConfigData.isItemStackAllowed`: no bundles, no
  belts, and only non-stackable items.
- `/toolbelt put <item> [count]`, `take <slot> [count]`, `info`, `menu`, `help`
  (game-master level) drive the same storage from the keyboard.

Not ported yet: the radial key menu and its keybinds, wearing the belt
(Curios or the mod's own slot), the upgrades, the pouch, the custom menu
background and the player layer renderer. `notes.md` has the full gap list.

## Build and check

The package is built by the launcher against the pinned game build; the SDK is
fetched into `.horizon/sdk/` (gitignored) and never packed.

```sh
cargo test                              # the pure engine and the glue's tests
cargo clippy --all-targets -- -D warnings
cargo fmt --check
rm -f Cargo.lock                        # a packed lock changes the artifact
cargo horizon-mod build                 # -> target/horizon-mod/toolbelt.hmod
cargo horizon-mod check toolbelt.hmod
cargo horizon-mod fits toolbelt.hmod --build nightly
```

`src/runtime.rs` is the storage engine: size clamping, the insert rule, stack
merging, extraction and the component round-trip. It knows nothing about the
game and is byte-identical to the retired `.hnmod` slice; `src/lib.rs` is the
glue (`mod_api` types only).

## Licence

BSD-3-Clause, the original mod's licence; the notice travels inside the package
as [`LICENSE`](LICENSE) and [`LICENSE-NOTICE.md`](LICENSE-NOTICE.md). The
carried assets, models, lang files and the belt recipe are Tool Belt's own,
read from the exact ATM10 jar.
