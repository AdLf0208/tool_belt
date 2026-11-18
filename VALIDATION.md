# Validation

Current ladder (the flat SDK package, 2026-11-18): see
[`notes.md`](notes.md) §*Migration to the SDK format* → §*Verification of this
migration*, with the logs under
[`../_shared/evidence/toolbelt/`](../_shared/evidence/toolbelt/) (`sdk-*.log`).

Short form, run in this directory with `CARGO_INCREMENTAL=0`:

```sh
cargo test                                # 16 passed (10 runtime + 6 glue)
cargo clippy --all-targets -- -D warnings # 0 warnings
cargo fmt --check                         # clean (never --all: the SDK is a path dependency)
rm -f Cargo.lock
cargo horizon-mod build                   # -> target/horizon-mod/toolbelt.hmod
cargo horizon-mod check toolbelt.hmod     # EXIT 0
cargo horizon-mod fits toolbelt.hmod --build nightly
```

The pre-migration ladder (`tools/horizon.py stage/check/test`, the staged
`tools/e2e.sh`, `tools/package.py`) belonged to the retired `.hnmod` container
and its tooling is in
[`../_shared/evidence/toolbelt/retired-hnmod-0.1.0/`](../_shared/evidence/toolbelt/retired-hnmod-0.1.0/).
Its results are the retired slice's behaviour evidence, recorded in
`notes.md` §*Verification*.

No game was launched by this migration: there is no `horizon-launcher` on this
box to build a game carrying the package. The next slice's first item is a
launcher-built e2e (see `notes.md` §*Gaps after the migration*).
