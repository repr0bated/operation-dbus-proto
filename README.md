# OP-DBUS core-only experiment

This branch gives Blitzy one crate, `crates/op-core`, to ingest and develop.
The crate source and tests are copied unchanged from OP-DBUS. The root is a
minimal Cargo workspace containing only that member and its inherited registry
dependency declarations. No application crates or old Git history are included.

`op-core` contains shared types, configuration, errors, security types,
execution tracking, runit helpers, shared-memory projection support and D-Bus
state publication. Read `AGENTS.md` before making changes.

## Starting dependency gap

`op-core` declares the local path dependency `../op-execution-tracker` and uses
it in state publication. That sibling crate is intentionally absent: this
experiment leaves dependency resolution to Blitzy. Cargo cannot currently
resolve the workspace, compile, or run its tests. The original crate manifest
is preserved so the missing dependency is explicit; no stub replaces it.

Start by resolving that dependency while preserving the public API and behavior,
then generate `Cargo.lock` and run:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-targets --all-features
```

Use the single-crate scope for this experiment. Expand only as needed for the
work being attempted. The complete backend, deployment configuration, external
UI and live host state are outside this starting snapshot. Netmaker is retired.

Source baseline: `1b82c594acd8d17a7e1b64b1b6ba52a21e5d9e97` in
`repr0bated/operation-dbus-proto`. This branch starts with fresh history.
