# Validation

Current ladder (the flat SDK package, `0.2.2` at pin
**`0a875da91dbb0fdcb7ba8c42c11341aa229af51a`** + mixins
`e59da9094f0c2fdda6c627fe9eff22dec6a906f0`, 2026-11-21): see
[`notes.md`](notes.md) §*Slice 2* → §*Slice 2 ladder*, with the logs under
[`../_shared/evidence/toolbelt/`](../_shared/evidence/toolbelt/) (`sdk-*.log`,
`upgrade-test-red.log`) and the in-game legs under
[`../_shared/evidence/toolbelt-e2e/`](../_shared/evidence/toolbelt-e2e/)
(`e2e-launcher-0.2.2.log`, `e2e-launcher-control-0.2.2.log`,
`e2e-selftest-0.2.2.log`).

Short form, run in this directory with `CARGO_INCREMENTAL=0`:

```sh
cargo test --offline                      # 20 passed (14 runtime + 6 glue)
cargo clippy --all-targets --offline -- -D warnings   # 0 warnings
cargo fmt --check                         # clean (never --all: the SDK is a path dependency)
rm -f Cargo.lock && cargo horizon-mod build          # -> target/horizon-mod/toolbelt.hmod
cargo horizon-mod check toolbelt.hmod                # EXIT 0
cargo horizon-mod fits toolbelt.hmod --build 0a875da # "made for"
cargo horizon-mod fits toolbelt.hmod --build 0b14f8a # "fits" (a different build)
python3 ../tools/verify-sdk-package.py toolbelt      # 1/1
```

The in-game leg is a launcher-built game carrying the package:

```sh
eval "$(ports/tools/launcher-game.sh env toolbelt)"   # from the campaign root
ports/tools/build-slot.sh run --job W1-toolbelt --port ports/toolbelt --step e2e -- \
    bash ports/toolbelt/tools/e2e.sh
```

Its `tools/craft_check.py` (the belt and pouch recipes in a real table) and
`tools/anvil_check.py` (the two-to-nine-slot upgrade chain in a real anvil) are
driven by the same control socket; the anvil needs the belt *out of hand* when
the block is clicked and shift-clicks for its 600/601/650 station slots.

The pre-`0.2.2` ladder (`0b14f8a`, the migration) is recorded in `notes.md`
§*Migration to the SDK format*. The pre-migration ladder
(`tools/horizon.py stage/check/test`, the staged `tools/e2e.sh`,
`tools/package.py`) belonged to the retired `.hnmod` container and its tooling is
in [`../_shared/evidence/toolbelt/retired-hnmod-0.1.0/`](../_shared/evidence/toolbelt/retired-hnmod-0.1.0/).
