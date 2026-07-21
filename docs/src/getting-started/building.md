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

There are two Vite apps. The primary UI dev tree lives in `crates/` and is
built with:

```bash
cd crates
npm run build
```

The embedded UI that `op-web` actually serves lives in `crates/op-web/ui/`
and is built with:

```bash
cd crates/op-web/ui
npx vite build
```

## Release builds

`op-web` release builds require `crates/op-web/ui/dist/index.html` to exist
because the assets are embedded with RustEmbed. Dev builds compile with an
empty asset set, so the requirement only applies to `--release`.
