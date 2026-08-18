# UI Source Boundaries

The dashboard served by `op-web` is a prebuilt SPA maintained outside this
repository. `op-web` reads it from `OP_WEB_STATIC_DIR`; no frontend is embedded
in the Rust binary.

## In-repository frontend trees

### `crates/`

This is the Vite/React development prototype:

```bash
cd crates
npm ci
npm run dev
```

Use `npm run build`, `npm run lint`, `npm run typecheck`, and `npm test` for its
development workflow. Its `dist/` directory is not selected by `op-web`
automatically.

### `crates/op-web/ui/`

This directory currently contains native Rust/egui source and leftover Node
metadata. It is not the served dashboard:

- its `package.json` has no build script;
- it has no `Cargo.toml` target;
- `op-web` does not read `crates/op-web/ui/dist`.

Do not use the old `npm run build:prod`, `wasm-pack`, or RustEmbed instructions
for this directory.

## Serving a dashboard

Build the external dashboard with its own repository workflow, then run:

```bash
OP_WEB_STATIC_DIR=/absolute/path/to/dashboard/dist \
  cargo run -p op-web --bin op-web-server
```

The directory must contain `index.html` and its referenced assets. See
[op-web Dashboard Build and Serving](../operations/op-web-ui-build.md) for
runtime behavior, cache policy, verification, and troubleshooting.
