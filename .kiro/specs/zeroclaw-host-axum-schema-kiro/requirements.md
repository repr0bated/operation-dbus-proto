# Requirements — zeroclaw-host-axum-schema-kiro

## Scope
Stand up an Axum HTTP/gRPC-Web host inside the `op-grpc-bridge` crate that serves
the **zeroclaw plugin's schema** to external consumers, while keeping every
architectural boundary intact.

---

## Functional Requirements

### FR-01 — Single schema source of truth
- The `zeroclaw` plugin's `schema()` method (defined in
  `plugin_schema_defs.rs`) is the **sole** origin for all schema data served by
  this host.
- The projection layer (D-Bus object tree, MCP tool surface, gRPC shape) is
  upstream of this host; zeroclaw must not duplicate projection-layer schema.
- No schema structs may be re-defined inside the Axum host crate.

### FR-02 — Schema file exposure via plugin-owned artifact
- The zeroclaw plugin writes (or symlinks) its rendered schema JSON to a
  well-known path: `/dev/shm/opdbus/schemas/zeroclaw.json`.
- The Axum host reads that file at startup and on `SIGHUP`; it never writes to
  that path.
- Ownership of the file is the plugin's responsibility, not the Axum host's.

### FR-03 — Btrfs subvolume scope
- The Btrfs subvolume at `@zeroclaw` is scoped **strictly** to zeroclaw's local
  install artefacts, schema cache, and rollback snapshots.
- The Btrfs subvolume is **not** a plugin tree; no D-Bus object path, projection
  artefact, or OSCAL record is stored there.
- NVMe I/O to Btrfs is only for vectorised footprint transport (blockchain
  chain-of-custody). All live state lives in `/dev/shm`.

### FR-04 — Native gRPC on a host Unix socket
- The Axum server exposes a native gRPC endpoint via a Unix domain socket at
  `/run/opdbus/zeroclaw-grpc.sock`.
- No TCP port is opened for native gRPC.

### FR-05 — HTTP and gRPC-Web via tonic-web
- The same Axum router wraps the tonic service with `tonic_web::enable(…)`.
- HTTP/1.1 and HTTP/2 consumers (browsers, NotebookLM, Droid) reach the service
  on a configurable TCP bind address (default `0.0.0.0:8090`).
- CORS preflight is handled by `tower-http`'s `CorsLayer`.

### FR-06 — D-Bus authority
- The Axum host registers itself as a D-Bus object at
  `/org/opdbus/v1/services/zeroclaw_axum_host`.
- Runtime config (bind address, schema path, reload interval) is readable and
  writable through that D-Bus object.
- No config-file polling; changes come through D-Bus mutations only.

### FR-07 — OSCAL subid and accountability
- All D-Bus objects, plugin artefacts, mutations, and events carry a `subid`
  following the `<category>.<component-type>.<subject>.<verb>[@vN]` taxonomy.
- The canonical subid registry lives in the plugin crate (`op-plugins`), not in
  the Axum host crate.
- The Axum host injects `X-Ghostbridge-Footprint` and `X-Ghostbridge-Trace-ID`
  into every gRPC response's trailing metadata.

### FR-08 — Zero Python; Rust-first
- All server logic is Rust. Deployment/bootstrap scripts are POSIX shell only.

---

## Non-Functional Requirements

| ID | Requirement |
|----|-------------|
| NFR-01 | Binary footprint: release build must not exceed 12 MB stripped. |
| NFR-02 | Schema reload must complete within 50 ms after `SIGHUP`. |
| NFR-03 | gRPC-Web request latency p99 < 20 ms for schema-fetch calls under 100 RPS. |
| NFR-04 | No `unsafe` blocks outside explicitly audited FFI boundary code. |
| NFR-05 | Clippy with `-D warnings` must pass on every commit. |
| NFR-06 | `cargo fmt --check` must pass on every commit. |

---

## Out of Scope
- Plugin lifecycle management (owned by `op-plugins`).
- WireGuard key negotiation / A.N.N.A. Scribe session (owned by `op-dbus`).
- Qdrant semantic search (owned by `op-cognitive-mcp`).
- Any Btrfs subvolume administration commands.
