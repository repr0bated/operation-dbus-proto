# Brief: finish the schema-driven gRPC bridge (no stubs)

Your branch `feature/schema-driven-grpc-bridge` is the right direction and the
empty-destination auto-resolution is good. But it does **not** build as pushed,
and two of the three new modules are scaffolding, not working code. Read this in
full before continuing — it states the non-negotiable architecture rules, the
identity model you must respect, and exactly what "done" means here.

## 0. It didn't compile
`crates/op-grpc-bridge/src/shared_socket.rs` used `std::os::unix::net::UCred`,
which is nightly-only (`peer_credentials_unix_socket`) and exposes `pid`/`uid`
as fields, not methods. The listener is a tokio `UnixListener`, so use
`tokio::net::unix::UCred` (stable, `.pid()` / `.uid()` are methods). Already
fixed locally — but **build the whole workspace in release before you push**:
`cargo build --workspace --release` must be clean. Several new files also have
unused-import / unused-variable warnings; treat warnings as errors.

## 1. Hard rules (do not violate)
- **Only valid path is `org.opdbus.v1.plugins`.** Every backend capability is a
  plugin under that bus name, reached via the canonical
  `/org/opdbus/v1/plugins/<plugin_id>` path. Never invent new
  `operation.<domain>.v1.*Service` proto packages, never call raw `ip:port`, and
  never add a code path that bypasses the plugin surface. If an object is
  missing, create the plugin for it — do not route around it.
- **One schema, one source of truth, computed in exactly ONE place.** A
  derivation duplicated across files is the violation, not a constraint. Do not
  re-hash or recompute the catalog hash anywhere; consumers read the manifest
  hash, they never re-hash.
- **No stubs, no placeholders, no "in a full implementation this would…".** If a
  module is wired in, it must do its job end-to-end. A bound socket that accepts,
  logs, and drops the connection is worse than no socket — it lies about being
  ready. Either finish the data plane or don't claim the surface exists.
- **No container gets a NIC or IP.** All container I/O is over Unix domain
  sockets (the shared `container.sock` / `ovsbr0-sock`), never TCP host:port.
  gRPC reflection works directly over UDS. `10.200.0.x` / `10.220.35.x` are HOST
  bridge addresses, not container endpoints.
- **Reactive, never polling.** `/dev/shm` is the authoritative present state.
  Components READ it. Do not add watchers, poll loops, or periodic refreshers.
  Action is triggered by arrival (an inbound connection), not by a timer.
- **Durability is the mutation chain.** The per-mutation immutable chain IS the
  record of truth; `/dev/shm/.../schemas` is the present-state projection. No
  SQL, no btrfs-snapshot backups, no parallel persistence machinery.

## 2. Identity model — this is where your shared_socket is wrong
- **The wristband is the only gate.** Identity is the `X-Ghostbridge-Footprint`
  header injected by xray (transport/WG is plumbing only). The bridge's
  `GhostbridgeInterceptor` enforces this. Any new ingress surface — including the
  UDS — must be gated by the same identity, not by a second parallel mechanism.
- **Container identity = sessionid**, and the sessionid is the non-spoofable
  `Argon2(secret=PSK, salt=WG-pubkey)`. System containers use their service name.
  Your `shared_socket.rs` derives `session_id = "container:<cgroup-name>"` by
  parsing `/proc/<pid>/cgroup`. **That cgroup name is not the sessionid** and is
  not authoritative — do not mint identity from it.
- For UDS (a sub-HTTP transport with no HTTP headers and no peer IP),
  `SO_PEERCRED` is an acceptable *anchor* — but it must resolve to the **canonical
  sessionid** (look the peer up against the identity/keypair source of truth),
  and the resolved identity must be injected as the footprint into the onward
  D-Bus/gRPC call so the same interceptor logic applies. Peer-cred is the *lookup
  key*, not the identity itself.

## 3. What this system actually is
This is a **single, system-wide surface**: multi-protocol, multi-device,
multi-interface, multi-user — all driven from **one source in `/dev/shm`**. The
bridge is not a qdrant proxy; it is the universal front for every plugin object
for every connecting client/container/user over one shared socket. Design every
new module for that generality, not for the qdrant special case.

## 4. The schema source you must read (both paths)
The SHM schema layout is canonical (do not change it):
- **Per-plugin schemas**: a folder of bare, per-plugin schema objects — one file
  per plugin (the individual plugin schema path).
- **Combined monolith**: `/dev/shm/live-schema.json`, the derived catalog of all
  plugins.
- **Manifest**: an atomic manifest holding the single `catalog_hash`
  (leaf+fold, incremental). Consumers read this hash; never re-hash.

Requirements:
- The router must support **both** access paths: resolve a single plugin from its
  individual schema file, AND load the full set from the combined monolith.
- **All schema must be included** — every plugin in the catalog is auto-exposed,
  not a curated subset. If it's in the source, it's routable.

## 5. Concrete gaps to close (no stubs)
1. **`SchemaRouter` is dead-wired.** It's constructed in `main()`, cloned into the
   socket task as `sr`, and never used. The live request path
   (`resolve_from_schema_or_explicit`) is pure path-string parsing — it never
   opens the schema catalog, so there is **no schema validation** at all. Make the
   live `DbusPassthrough` path go through `SchemaRouter`: load from the
   per-plugin or combined SHM source, validate the method/property exists in the
   schema, then route. If the method isn't in the schema, reject it.
2. **The UDS data plane is a stub.** `container.sock` binds, resolves peer, logs,
   and drops the connection (your own comment admits it). Finish it: serve the
   tonic stack over the accepted `UnixStream` so a container's call actually
   reaches the plugin and returns. For a single passed stream, keep H2 alive with
   `serve_with_incoming(tokio_stream::once(Ok(stream)).chain(futures::stream::pending()))`
   so the connection doesn't EOF. The same interceptor/identity gating applies on
   this path.
3. **Identity injection on the UDS path** (see §2): peer-cred → canonical
   sessionid → footprint injected into the onward call.

## 6. Definition of done
- `cargo build --workspace --release` clean (no errors, no warnings).
- A container connecting over `container.sock` can call any plugin method named
  in the schema and get a real response; unknown methods are rejected by schema
  validation; the call is identity-gated by the canonical sessionid.
- No module is constructed-but-unused. No "for now" comments. No second identity
  mechanism. Both schema access paths (per-plugin file + combined monolith) work.
