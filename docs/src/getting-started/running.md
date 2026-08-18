# Running Locally

`op-web-server` exposes the HTTP API, MCP routes, WebSocket endpoint, and a
filesystem-backed dashboard on port 8080 by default.

## Prerequisites

The process exits during startup unless the sealed plugin catalog is available
at `/dev/shm/opdbus/plugin-blobs`. Verify the catalog before starting:

```bash
cargo run -p op-plugins --bin opblob -- \
  catalog /dev/shm/opdbus/plugin-blobs
```

See [Sealed Blob Catalog](../architecture/blob-catalog.md) if it must be
created or refreshed.

The dashboard is optional for backend work. To serve it, obtain a prebuilt SPA
directory containing `index.html`; its source and deployment workflow live
outside this repository.

## Start op-web

Backend only, using the default dashboard directory:

```bash
cargo run -p op-web --bin op-web-server
```

With a local dashboard build:

```bash
OP_WEB_STATIC_DIR=/absolute/path/to/dashboard/dist \
  cargo run -p op-web --bin op-web-server
```

Override the listener port with `PORT`:

```bash
PORT=18080 OP_WEB_STATIC_DIR=/absolute/path/to/dashboard/dist \
  cargo run -p op-web --bin op-web-server
```

`OP_WEB_STATIC_DIR` defaults to `/usr/local/share/op-dbus/dashboard`. The value
is read when the router is created, so changing it requires restarting the
process. Replacing files inside the configured directory does not require a
Rust rebuild.

## Verify

```bash
curl -fsS http://127.0.0.1:8080/api/health
curl -I http://127.0.0.1:8080/
```

The health endpoint should return JSON. The root request returns the dashboard
when `index.html` is readable, or a static-file error when no dashboard has
been published.

For a deployed host, manage the service with `sudo sv`; do not run a second
server by hand. See [Service Management](../operations/service-management.md).
