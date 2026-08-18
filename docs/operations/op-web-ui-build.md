# op-web Dashboard Build and Serving

`op-web` serves a prebuilt single-page application from the filesystem. The
dashboard is not compiled into the Rust binary.

## Runtime contract

The router reads `OP_WEB_STATIC_DIR` once at startup:

```text
OP_WEB_STATIC_DIR=/path/to/dashboard/dist
```

If the variable is unset, the server uses:

```text
/usr/local/share/op-dbus/dashboard
```

The directory must contain `index.html` and every asset path referenced by that
file. API, MCP, and WebSocket routes take precedence; all other paths fall back
to `index.html` so client-side routes work on a direct page load.

For local development:

```bash
OP_WEB_STATIC_DIR=/absolute/path/to/dashboard/dist \
  cargo run -p op-web --bin op-web-server
```

At startup, the server opens the sealed plugin catalog in
`/dev/shm/opdbus/plugin-blobs`. It creates an absent directory and can start
with an empty catalog. It exits before binding port 8080 only if the directory
cannot be created or read. Plugin-backed routes still require the expected
blobs to be populated.

## Source ownership

The deployed dashboard source is maintained outside this repository. Build it
with the commands in that UI repository, then point `OP_WEB_STATIC_DIR` at its
output directory or publish the output through that repository's deployment
workflow.

These in-repository trees are not the served dashboard:

- `crates/` is a Vite/React development prototype. `npm run build` writes its
  own `dist/`, but `op-web` does not select it automatically.
- `crates/op-web/ui/` contains native Rust/egui source plus stale Node metadata.
  It has no Cargo target or package build script that feeds `op-web`.
- The removed `lovable/` submodule was retired and is not a build input.

Do not copy a build into `crates/op-web/ui/dist` expecting the server or a
release build to discover it.

## Build and update behavior

The backend build is independent of dashboard assets:

```bash
cargo build -p op-web --release
```

Replacing files under the currently configured directory does not require
rebuilding or restarting `op-web`; `ServeDir` reads them from disk. Changing
`OP_WEB_STATIC_DIR` does require a service restart because the environment
variable is read while the router is created.

Publish the whole asset set as one release. `index.html` is served with
`Cache-Control: no-cache, must-revalidate`, while `/assets/*` is cached for one
year as immutable. A partially published build can therefore make a new
`index.html` refer to assets that are not present yet.

## Verification

With `op-web` running:

```bash
curl -I http://127.0.0.1:8080/
curl -I http://127.0.0.1:8080/assets/<content-hashed-file>
curl -fsS http://127.0.0.1:8080/api/health
```

Expected results:

- `/` returns HTML with `Cache-Control: no-cache, must-revalidate`.
- A real file under `/assets/` returns
  `Cache-Control: public, max-age=31536000, immutable`.
- `/api/health` returns JSON rather than the SPA document.

## Troubleshooting

- **Dashboard returns 404:** confirm `$OP_WEB_STATIC_DIR/index.html` exists and
  that the service account can traverse and read the directory.
- **Direct navigation returns 404:** verify the request reaches `op-web`; its
  static fallback returns `index.html` for client-side routes.
- **Old UI remains visible:** inspect the response headers and the asset names
  referenced by the current `index.html`. Do not reuse filenames for changed
  assets.
- **Backend starts without a dashboard:** this is expected when only the static
  directory is missing. API routes remain separate; dashboard requests fail
  until valid files are published.
