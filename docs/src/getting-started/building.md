# Building the Workspace

All Cargo commands run from the repository root.

```bash
cargo build --workspace
```

For a faster incremental check on a single crate:

```bash
cargo check -p <crate>
```

## Frontend

The Vite/React development prototype lives in `crates/`:

```bash
cd crates
npm ci
npm run build
```

This build is useful for frontend development, but `op-web` does not embed or
automatically serve it. The deployed dashboard is maintained outside this
repository and provided to `op-web` as a prebuilt static directory.

`crates/op-web/ui/` is also not an embedded Vite application. It currently
contains native Rust/egui source and stale Node metadata, with no package build
script or Cargo target that feeds `op-web`.

## Release builds

Dashboard assets are not a prerequisite for an `op-web` release build:

```bash
cargo build -p op-web --release
```

At runtime, set `OP_WEB_STATIC_DIR` to a dashboard build containing
`index.html`. The default is `/usr/local/share/op-dbus/dashboard`. See the
dashboard serving runbook at `docs/operations/op-web-ui-build.md` for the full
contract and verification steps.
