# Tool Belt — a Horizon port

**Tool Belt** is a Minecraft mod. A simple mod that adds a belt for carrying tools. This repository is a **port of it to [Horizon](https://github.com/horizon-rust/horizon)**, the Rust engine for Minecraft Java 26.3. It carries the complete source of the port plus the content files it ships, and installs as a single `.hmod` package.

Upstream: [gigaherz/ToolBelt](https://github.com/gigaherz/ToolBelt) — licence **BSD-3-Clause**.

| | |
|---|---|
| Upstream mod | Tool Belt ([source](https://github.com/gigaherz/ToolBelt)) |
| Ported to | Horizon 0b14f8a, Minecraft Java 26.3, mod API `0.2` |
| Package | `toolbelt.hmod` (install it with the Horizon launcher) |
| Licence | **BSD-3-Clause** — the upstream mod's licence; see [Licence and credit](#licence-and-credit) |

## What this port implements

- The menu is the pin's native menu, so it has no custom belt texture and
- A sneak/off-hand click mapping substitutes for the radial swap, which needs a
- `/toolbelt take` places the exact stack (components kept) into the player's
- `/toolbelt put ... -1` answers "Count must be at least 1." instead of
- The block click is cancelled on the **server** only, as the retired hook was

## Not yet ported

- **No in-game run of the SDK package.** There is no `horizon-launcher` on this
- The retired e2e read the belt through `/toolbelt info`; that readout is
- The rest of the behaviour gaps are unchanged from slice 1 (see Gaps above).
- `MenuItem`-level rules: the native menu still cannot refuse a stackable item

## Installing

1. Install the [Horizon launcher](https://github.com/horizon-rust/horizon/releases) (0.2.70 or newer).
2. Add `toolbelt.hmod` to the instance's mods.
3. The package is pinned to a specific Horizon build; the launcher checks that pin.

## Licence and credit

Tool Belt was created by its upstream authors; the code, models and assets in the original mod
remain theirs and are used here under **BSD-3-Clause**. The Rust in this repository is a clean-room
rewrite for Horizon, not a translation of the original source.

- `LICENSE` — the upstream licence text.
- `LICENSE-NOTICE.md` — what this port carries and where it came from.
- The upstream project: https://github.com/gigaherz/ToolBelt

## Technical details

```
horizon-mod.toml   package manifest (id, version, api, [native] pin)
Cargo.toml         the crate; depends on `horizon-mod-api` from the fetched SDK
src/               lib.rs (the one `impl Hooks`), runtime.rs (pure logic + tests), glue
assets/ data/      the content the package ships
tools/             content generators and the e2e scenario
```

- Build: `cargo horizon-mod build` (packages; it does not compile against the game).
- Test: `cargo test`, `cargo clippy --all-targets -- -D warnings`.
- In-game: `ports/tools/launcher-game.sh run toolbelt` in the campaign tree, or install the
  `.hmod` and play; the harness scenario is `tools/e2e.sh`.

This repository is generated from the campaign's port at `ports/toolbelt/`; the port's own
`notes.md` is the full record (decisions, traps, evidence).
