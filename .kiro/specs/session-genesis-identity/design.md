# Design — Session Genesis Identity

**Implements:** `requirements.md` (amended 2026-08-16)

---

## 1 · Arrival Sequence (end-to-end)

```
             ORACLE DECOY                          OP-GRPC-BRIDGE
             ─────────────                         ──────────────
User ──WG──► decoy terminates WG                         │
             │ issues Ed25519 assertion                   │
             │ (OIA1 wire envelope)                       │
             │                                           │
             └──NetMaker──► xray ──TLS──► bridge:8090    │
                            (passthrough)   │             │
                                           ▼             │
                            ┌──────────────────────────┐ │
                            │ GhostbridgeInterceptor   │ │
                            │                          │ │
                            │ 1. assertion present?    │ │
                            │    → validate (pipeline) │ │
                            │    → HumanPrincipalId    │ │
                            │                          │ │
                            │ 2. session has genesis?   │ │
                            │    YES → compare header  │ │
                            │          == stored → OK  │ │
                            │          != stored → REJ │ │
                            │    NO  → this is arrival │ │
                            │          ↓               │ │
                            └──────────┼───────────────┘ │
                                       ▼                 │
                            ┌──────────────────────────┐ │
                            │ MutationEngine           │ │
                            │ (arrival = mutation one) │ │
                            │                          │ │
                            │ 3. read chain head       │ │
                            │    last_hash() → O(1)    │ │
                            │    head.timestamp        │ │
                            │                          │ │
                            │ 4. read catalog_hash     │ │
                            │    schema_catalog_hash() │ │
                            │                          │ │
                            │ 5. arrival_timestamp     │ │
                            │    = Utc::now()          │ │
                            │                          │ │
                            │ 6. MINT GENESIS          │ │
                            │    mint_genesis(         │ │
                            │      pubkey,             │ │
                            │      chain_head_hash,    │ │
                            │      head_ts,            │ │
                            │      catalog_hash,       │ │
                            │      arrival_ts          │ │
                            │    ) → [u8; 32]          │ │
                            │                          │ │
                            │ 7. record arrival event  │ │
                            │    in EventChain         │ │
                            │    (session's first      │ │
                            │     chain entry)         │ │
                            │                          │ │
                            │ 8. write session record  │ │
                            │    → in-process cache    │ │
                            │    → SHM projection      │ │
                            │      (atomic write)      │ │
                            │    → persist to Cozo     │ │
                            │      (durability)        │ │
                            │                          │ │
                            │ 9. stamp genesis into    │ │
                            │    response extensions   │ │
                            └──────────────────────────┘ │
                                       │                 │
                                       ▼                 │
                            ┌──────────────────────────┐ │
                            │ Subsequent requests      │ │
                            │                          │ │
                            │ Interceptor reads        │ │
                            │ session record from      │ │
                            │ in-process state cache   │ │
                            │ and compares genesis     │ │
                            │ → one == , no hashing    │ │
                            └──────────────────────────┘ │
                                       │                 │
                                       ▼                 │
                            ┌──────────────────────────┐ │
                            │ Per-mutation stamping     │ │
                            │                          │ │
                            │ event_to_footprint adds: │ │
                            │  • session_genesis       │ │
                            │  • session_id            │ │
                            │  • wireguard_pubkey      │ │
                            │                          │ │
                            │ Chain is sliceable +     │ │
                            │ offline-verifiable       │ │
                            └──────────────────────────┘ │
```

---

## 2 · Replay Mitigation (FR-11 Resolution)

**Chosen option: 1 — Assertion-bound.**

The genesis is accepted only alongside a valid, unexpired Ed25519 oracle
assertion. The genesis itself is not a credential — it is the state-and-moment
binding on an identity that was independently proven by the assertion's
cryptographic pipeline (signature → trust store → expiry → replay cache →
source-IP binding → HumanPrincipal resolution).

**Threat model:** An observer of the `x-ghostbridge-genesis` header alone
cannot replay it. They would also need:
- A valid, unexpired oracle assertion (Ed25519-signed, ≤ 900 s TTL)
- The same source IP (assertion-bound to `netmaker_inner_ip`)
- A nonce not in the replay cache

The transitional header-only path (OQ-6 Phase 1, where genesis is accepted
without an assertion during migration) is the only replayable surface. It is
time-boxed by the Phase 2 cutover and its risk is equivalent to the current
self-asserted footprint header — no regression.

**Compensating controls:**
- Genesis value is redacted from all log output (`tracing` spans, structured
  logs) — a lint/test confirms no `tracing::info!`/`warn!`/`debug!` includes
  the raw genesis hex.
- Transport confidentiality (TLS) between all hops.
- Header not echoed in responses (response carries the trace_id, not the
  genesis).

**After OQ-6 Phase 2 completes:** a request presenting a genesis without a
valid assertion is rejected. The genesis becomes purely an internal session
state binding, never a standalone credential.

**Deliberate consequence for "every packet carries the header" (PROMPT §4):**
For assertion-carrying traffic (all authenticated public users), the genesis
does NOT travel on the wire as a separate header. The assertion is the
per-request stamp; the genesis is looked up server-side from the session record
after assertion validation. The wire carries the assertion (which changes per
request due to nonce/expiry), not the genesis (which is constant and therefore
replayable). This is a deliberate departure from the original framing — "the
point of the whole thing was every packet had the header" — but the *intent*
(every packet is identity-verified) is preserved: every packet carries the
assertion, and the genesis is verified from the session record as part of that
verification. The constant-header model survives only for UDS container
traffic (Path A), where the peer credential is the transport binding and
replay within a UDS is not a meaningful threat.

---

## 3 · OQ-1 Path B Resolution (xray-terminated public traffic)

**The oracle assertion IS the per-request identity for public traffic.**

For xray-terminated connections, the assertion travels inside the TLS channel
as gRPC metadata (`x-oracle-identity-assertion-bin`). It is per-request, not
per-connection, so HTTP/2 multiplexing is irrelevant — each request carries
its own assertion with its own nonce, expiry, and source-IP binding. The
bridge resolves `human_pubkey → HumanPrincipal → session record → genesis`.

The join key for Path B is the assertion itself. No sealed identity, no
UDS peer credential, no PROXY v2 needed. FR-8 (sealed identity) applies
only to Path A (session containers on the shared ghostbridge socket).

**What the assertion gives that a join key would:**
- Which session this request belongs to (assertion carries `human_pubkey` →
  session lookup)
- Proof it's from the authenticated user (Ed25519 signature, source-IP binding)
- Freshness (nonce + expiry, replay cache)

**What does NOT happen on Path B:**
- No `x-ghostbridge-genesis` header on the wire (genesis looked up server-side)
- No sealed identity (unnecessary — assertion is the per-request proof)
- No per-connection session binding (impossible with H2 mux; unnecessary with
  per-request assertion)

---

## 4 · Session Lifetime (FR-12 Resolution)

| Event | Effect |
|-------|--------|
| **Start** | First authenticated mutation (arrival). Genesis minted. `session_started_at` set. `expires_at` set (configurable TTL, default 24h). Chain event recorded. |
| **Renewal** | Re-authentication (new assertion with same pubkey, after expiry or explicit teardown). Mints a **new genesis** — new chain head, new arrival timestamp, new anchor. The old session's chain span is bounded; the new session's span begins fresh. |
| **Expiry** | `now > expires_at`. Interceptor rejects with `PERMISSION_DENIED("Identity term has expired. Re-authenticate to renew.")` — existing behavior at `interceptor.rs:243-247`. In-flight requests that started before expiry but arrive after are rejected (no grace period beyond what the assertion's 30s leeway provides). |
| **Teardown** | Last authenticated D-Bus binding logout/disconnect stops a provisioned container and sets `active = false`; interceptor rejects. The stopped instance remains provisioned (parked), and boot never starts it. |

Provisioned identity containers are deliberately not host services. Incus
creates them stopped with `boot.autostart=false`; omitted profiles resolve to
the NIC-less `identity` profile (root disk plus shared fabric UDS), not the
device-empty host `default` profile. The first authenticated binding for a
session starts the exact UUID-named instance; additional bindings share the
same live term. Logout or `NameOwnerChanged` removes only that sender, and the
last binding parks the instance through the native Incus Unix-socket API. Host
identities have no `instance`, so the same transition only updates their sled
liveness. No lifecycle path shells out to the Incus CLI.

**`expires_at` semantics for version 3 records:**
- `None` / `0` = session has no expiry (long-lived host/system identity).
- Non-zero = unix seconds when the current term lapses.
- A lifelong account (host, chatbot) gets `expires_at` renewed on every
  authenticated touch (existing `touch_identity_sled` behavior).
- A consumer/subscriber identity does NOT get renewed — its term actually
  lapses, forcing re-authentication.

**Chain growth from arrival-as-mutation:** At the expected session rate
(< 100 sessions/day on this host), one chain event per login adds < 100
events/day. The chain already handles ~1.8M events. This is acceptable and the
durability gain (every login is auditable and sliceable) outweighs the cost.

---

## 5 · Authoritative Store Architecture (FR-4)

**Correction from earlier draft.** The identity_sled plugin's state is ONE
file (`/dev/shm/opdbus/state/identity_sled.json`) holding ALL session records,
not one blob per session. The plugin-blobs catalog
(`/dev/shm/opdbus/plugin-blobs/`) holds the plugin *schema* blob, not
per-session sealed IDs. The per-request read path never opens a file —
it reads the in-process deserialized state cache.

### 5.1 Store hierarchy

```
┌──────────────────────────────────────────────────────────────────┐
│  HOT READ PATH (gate decisions):                                 │
│                                                                  │
│  In-process state cache (MutationEngine plugin state)            │
│  • Vec<ContainerIdentitySled> behind RwLock                      │
│  • Written by MutationEngine (genesis, mutation_index) and the   │
│    stream path (liveness fields) — disjoint fields, FR-6         │
│  • Read by interceptor via dispatch get_identity                 │
│  • Never stale: updated synchronously on every mutation          │
│                                                                  │
│  The gate reads HERE. One lookup, one equality. No file I/O.     │
└──────────────────────────────────────────────────────────────────┘
         │ atomic write on mutation
         ▼
┌──────────────────────────────────────────────────────────────────┐
│  SHM projection: /dev/shm/opdbus/state/identity_sled.json        │
│  • Atomic temp+rename per mutation (op_core::projection_shm)     │
│  • Read by external consumers (schema_router, op-web state_tree) │
│  • This IS the present state for everything outside the process  │
└──────────────────────────────────────────────────────────────────┘
         │
         ▼
┌──────────────────────────────────────────────────────────────────┐
│  Cozo: /var/lib/op-dbus/identity-cozo                            │
│  • DURABLE — survives process restart                            │
│  • Hydration source on cold start (ensure_hydrated, once)        │
│  • Write timing: see §5.2                                        │
└──────────────────────────────────────────────────────────────────┘
         │ DEAD
         ▼
┌──────────────────────────────────────────────────────────────────┐
│  /dev/shm/plugin_schema.dat (global 152-byte sled)               │
│  • NO LONGER WRITTEN. NO LONGER READ.                            │
│  • File may linger on disk; nothing touches it.                  │
└──────────────────────────────────────────────────────────────────┘
```

### 5.2 Genesis durability — inline Cozo write at mint time

The genesis (and its inputs: arrival_timestamp, chain_head_at_arrival,
catalog_hash_at_arrival, head_timestamp_at_arrival) is **irreproducible** —
arrival_timestamp cannot be recovered after the fact, and without it the
genesis cannot be recomputed. If the process crashes between minting and
persist, the session is permanently unverifiable.

Therefore: **genesis persist to Cozo is synchronous and inline on the mint
path** (HC-5 — durability stays inline on the dispatch path). The sequence at
arrival is:

```
1. mint_genesis(...)                   // pure computation
2. cache.genesis = hex::encode(result) // in-process state
3. persist_sled(&cache_entry).await    // Cozo write — INLINE, awaited
4. publish_projection(...)             // SHM projection (can tolerate gap)
5. return genesis to interceptor       // session is now verifiable
```

Step 3 is the existing `persist_sled` (spawn_blocking + Cozo put), but
**awaited before returning** rather than fire-and-forget. This is the same
durability guarantee the event chain already provides (`persist_audit_event`
is inline, logged-and-swallowed on failure). If the Cozo write fails:

- The genesis IS in the in-process cache (gate works for this process lifetime)
- The genesis IS in the SHM projection (external readers see it)
- The genesis is NOT durable — a restart loses it
- This is logged as a **warning** (same severity as `persist_audit_event`
  failure), not a hard rejection — failing the user's first request because
  Cozo had a hiccup is worse than accepting the durability risk for the
  window until the next mutation persists it as a side effect

For subsequent mutations: `advance_session_record` writes mutation_index to
the cache and projects to SHM. The Cozo persist of the full record
(including genesis) piggybacks on the next mutation's persist — so a genesis
that failed its initial Cozo write will be durably persisted on the next
successful mutation anyway.

**On cold start:** `ensure_hydrated` loads from Cozo. If a session's genesis
is absent in Cozo (crash between steps 2 and 3 above, no subsequent mutation
persisted it), the session's `genesis` field is `None`. The interceptor treats
this as "session exists but genesis not yet minted" — the next authenticated
request triggers a re-mint (same as a first arrival).

**Two distinct paths reach that state, and only one of them is invisible to
clients:**

1. *Crash between steps 2 and 3.* The genesis was never returned, so no client
   holds it. Re-mint is silent and lossless.
2. *Cozo write failed but the request proceeded* (the warn-don't-fail bullet
   above). The genesis **was** returned and clients are presenting it. If the
   process restarts before a later mutation persists the record, hydration
   finds no genesis, re-mints, and every client holding the previous value now
   mismatches and is rejected.

Path 2 is an accepted outcome, not a silent one: those clients receive
`UNAUTHENTICATED` and must re-authenticate, which mints a fresh session per
FR-12. It must not be justified by "no client holds a stale value" — in path 2
they do. The window is bounded by the next successful mutation persist, and it
requires a Cozo failure *and* a restart before the next mutation.

### 5.3 Per-request verification path

1. Interceptor calls `dispatch_identity_sled_method(engine, "get_identity",
   { session_id })` — this reads the in-process cache (already deserialized).
2. Compares `stored_record.genesis == presented_genesis` — one equality on
   `String` (64 hex chars), no hashing, no file open.
3. Done. No Cozo query on the per-request path. No SHM file open.

**The SHM projection file is NOT re-sealed per mutation.** It is an atomic
JSON write of the entire plugin state. This already happens today
(`publish_plugin_projection_from_cache` at identity_sled_dispatch.rs:216) and
is not new overhead. Concurrent readers see either the old or new JSON via the
atomic rename.

---

## 6 · File Changes

### 6.1 New Files

| File | Purpose |
|------|---------|
| `crates/op-identity/src/session_genesis.rs` | `mint_genesis()` — the single author of the genesis formula. Pure function, no I/O. |

### 6.2 Modified Files

| File | Change |
|------|--------|
| `crates/op-identity/src/lib.rs` | Add `pub mod session_genesis;`. Remove re-export of `anna_scribe`, `write_sled_from_wg`, `write_sled_full`, `watch_wireguard_handshakes`. |
| `crates/op-identity/src/schema_bridge.rs` | Remove `write_sled_from_wg`, `write_sled_full`, `watch_wireguard_handshakes`. `etch_footprint` is **deleted** (not renamed — see §6.4). `SLED_SCHEMA_VERSION` replaced by content-hash-derived version (FR-10, §8). |
| `crates/op-grpc-bridge/src/interceptor.rs` | Genesis verification path: after assertion validation, look up session's stored genesis from state cache. Compare presented header. Reject on mismatch. Add `x-ghostbridge-genesis` header reading alongside existing `x-ghostbridge-footprint`. Remove the `verify_ghostbridge_footprint` call from the fallback path. For assertion-carrying traffic: genesis looked up server-side after assertion validation, no header required from client. Carry explicit `principal_id`, `session_id`, and `session_genesis`; do not expose an auth-facing footprint field. |
| `crates/op-grpc-bridge/src/oracle_assertion.rs` and `mcp_frontend.rs` | Remove `derive_human_footprint`/`HumanPrincipalIdentity.footprint` and footprint-keyed grant lookup. Resolve grants/audience by the registered `principal_id`; genesis remains session context only. |
| `crates/op-grpc-bridge/src/mutation_engine.rs` | `advance_identity_sled` → `advance_session_record`: writes genesis (once) + mutation_index to the session's entry in the state cache. Add `mint_and_store_genesis` called on first mutation of a session. Remove `write_sled_full` import and call. `event_to_footprint` gains `session_genesis`, `session_id`, `wireguard_pubkey` in metadata. `event_to_footprint` takes a `SessionContext` parameter (derived from request extensions). |
| `crates/op-grpc-bridge/src/identity_sled_dispatch.rs` | **Lines 388 and 428** (both in `write_identity`): replace `etch_footprint(&pubkey_bytes, 0, 0)` with `mint_genesis(...)`. These are the initial genesis mint for provisioned sessions (backfill at :388, creation at :428). They now call `mint_genesis` with the current chain head + arrival timestamp, making provisioning equivalent to arrival. Remove `write_sled_from_wg` call at :465. `write_identity` (stream path) writes only liveness fields. `ensure_hydrated` **changed** (§5.2): a hydrated record whose `genesis` is absent is treated as "session exists, not yet minted" so the next authenticated request re-mints; it also compares the record's stored schema hash against `SCHEMA_CONTENT_HASH` and skips stale-shaped records (§8). |
| `crates/op-grpc-bridge/src/shared_socket.rs` | `CanonicalPeerIdentity::from_sled()` replaced by `CanonicalPeerIdentity::from_session(engine, session_id)`. Derives `session_id` from peer credential: `SO_PEERCRED` gives uid → lookup uid-to-container mapping (Incus rootfs owner) → container name = session_id. For the host (uid 0 or the bridge's own uid), uses the host's session_id. The lookup is a static map built at startup from `incus list` output (reactive: built once, not polled). The `uds_identity_interceptor` uses `block_in_place` + `Handle::current().block_on(...)` (same pattern as `verify_per_identity` at interceptor.rs:236) since it's a sync `Interceptor::call`. |
| `crates/op-grpc-bridge/src/tracing.rs` | Remove `SENTINEL_FOOTPRINT`. `TraceContext::from_headers` returns `None` when footprint absent (no more zero-fill). Genesis value NEVER appears in trace output — redacted. |
| `crates/op-grpc-bridge/src/grpc_web.rs` | Add `HeaderName::from_static("x-ghostbridge-genesis")` to `ALLOW_HEADERS`. |
| `crates/op-grpc-bridge/src/grpc_client.rs` | Add `x-ghostbridge-genesis` to outbound header injection alongside existing footprint. |
| `crates/op-plugins/src/state_plugins/identity_sled.rs` | `ContainerIdentitySled` gains: `genesis: Option<String>` (hex, immutable after first write), `arrival_timestamp: i64`, `chain_head_at_arrival: String`, `catalog_hash_at_arrival: String`, `head_timestamp_at_arrival: i64`. Remove `hashed_footprint` field — `genesis` replaces it (§6.5). |
| `crates/op-state-store/src/event_chain.rs`, `crates/op-snowball/src/footprint.rs`, legacy `plugin_footprint.rs`, and append/vector wiring | Replace `json_args_footprint` with bounded/redacted canonical arguments, keep one footprint payload envelope, delete duplicate hash generators, make Snowball the sole current-event hash author, and vectorize payload text with receipt provenance. |
| `crates/op-plugins/src/state_plugins/oscal_subid_registry.rs` | Register: `mut.service.session-genesis.mint@v1`, `evt.service.event-chain.session-stamp@v1`, `obs.service.identity-sled.genesis-verify@v1`, `evt.service.session-genesis.arrival@v1`. |
| `crates/op-cognitive-mcp/src/main.rs` | Remove `write_sled_from_wg` call at :66. |
| `crates/op-mcp/src/main.rs` | Remove `write_sled_from_wg` call at :113. |
| `crates/op-mcp/src/compact.rs` | Remove `write_sled_from_wg` call at :585. |
| `crates/op-identity/src/bin/op-identity-sled.rs` | Remove `write_sled_from_wg` at :52. CLI becomes read-only diagnostic. |

### 6.3 Deleted Files / Dead Code

| Item | Rationale |
|------|-----------|
| `crates/op-identity/src/anna_scribe.rs` | Duplicate reader + redundant genesis derivation. Module removed from `lib.rs`. |
| `SENTINEL_FOOTPRINT` in `tracing.rs` | Fail-closed eliminates the need for a degraded-pass sentinel. |
| `write_sled_from_wg` function body in `schema_bridge.rs` | All 5 external call sites deleted; function itself removed. |
| `write_sled_full` function body in `schema_bridge.rs` | Same. |
| `watch_wireguard_handshakes` in `schema_bridge.rs` | Competing writer; identity from assertion not handshake; HC-9 (no host WG). |
| `etch_footprint` in `schema_bridge.rs` | See §6.4. |
| Uncommitted patch: `schema_bridge.rs` (pin footprint once) | Superseded by genesis. |
| Uncommitted patch: `interceptor.rs` (mismatch → None) | Superseded by genesis verification path. |

### 6.4 Why `etch_footprint` is deleted (not renamed)

The design's earlier draft proposed renaming `etch_footprint` to
`etch_chain_footprint` on the assumption its only live consumer was the
snowball record path. That was wrong.

**Actual live callers:**
1. `identity_sled_dispatch.rs:388` — backfills empty hashed_footprint on
   re-registration (identity-anchor usage, not snowball).
2. `identity_sled_dispatch.rs:428` — mints footprint at initial provisioning
   (identity-anchor usage).
3. `anna_scribe.rs` — deleted.
4. The `advance_identity_sled` / `write_sled_full` path — deleted.

The snowball path (`event_to_footprint`) never calls `etch_footprint`. It
assembles `PluginFootprint` metadata from the chain event fields directly.

With callers 1 and 2 replaced by `mint_genesis` and callers 3 and 4 deleted,
`etch_footprint` has **zero live consumers**. Renaming a dead function to
something snowball-sounding would create a misleading zombie. Delete it.

### 6.5 `hashed_footprint` → `genesis` (no dual storage)

The earlier draft stored genesis in both `genesis: Option<String>` and
`hashed_footprint: String`. That is a transcription duplicate inside one
record — the pattern FR-9 forbids.

**Resolution:** The `hashed_footprint` field is replaced by `genesis`. There
is one field, one name, one author. For backward compatibility during
migration:
- Existing version ≤ 2 records still carry `hashed_footprint` (their old
  etch_footprint-derived value). The gate recognizes version ≤ 2 and compares
  against the old field.
- Version 3 records carry `genesis` only. `hashed_footprint` is absent or
  empty and is never consulted.
- A `#[serde(alias = "hashed_footprint")]` on the `genesis` field handles
  deserialization of old records during the transition.

---

## 7 · Single Author Table (§9 Completion Criterion)

| Fact | Single author | Consumers (derive, never restate) |
|------|---------------|-----------------------------------|
| Principal identity | Authoritative HumanPrincipal/service-principal registration; human IDs use `derive_principal_id(raw_wireguard_pubkey)` once | assertion/request context, principal-grant projection, audience, D-Bus binding, footprint `actor_id` metadata |
| Genesis formula | `op_identity::session_genesis::mint_genesis` | interceptor (compares stored output), chain stamper `event_to_footprint` (embeds stored output), UDS injector (reads stored output from session record), offline verifier (recomputes from stored inputs), `identity_sled_dispatch` write_identity (calls mint_genesis for initial provisioning) |
| Session record shape | `ContainerIdentitySled` in `identity_sled.rs` (PluginSchema, schemars-derived) | In-process state cache (deserialized), SHM projection (serialized JSON), Cozo relation (derived from schemars), gRPC reflection (derived from schema) |
| Chain position (mutation_index) | `MutationEngine::advance_session_record` | `event_to_footprint` (reads from session context on engine), per-session record (written by engine only) |
| Footprint payload shape | One canonical `PluginFootprint` schema/type | sled emitter, Shuttle, Snowball append, deterministic vector renderer |
| Current event hash | Snowball append function | receipt/outbox, chain verifier, vector provenance; never principal/session authorization |
| Catalog binding | `schema_catalog_hash()` in `schema_bridge.rs` | `mint_genesis` (reads as input at arrival), no other site computes or caches it |
| Schema version / shape hash | Content hash of `ContainerIdentitySled`'s canonical schemars serialization (`SCHEMA_CONTENT_HASH`) | SHM projection (carries hash in manifest for drift detection), Cozo record (embeds hash), gate cold-start hydration (compares hash to reject stale records) |
| Genesis inputs (arrival_ts, chain_head, catalog_hash, head_ts) | Stored immutably in the session record at mint time by `mint_and_store_genesis` | Offline re-verification (reads stored inputs, recomputes genesis), audit tooling |

**No consumer holds its own copy.** The genesis blake3 invocation exists only
in `session_genesis.rs`. A CI grep gate confirms this:
```bash
grep -rn 'blake3.*chain_head\|mint_genesis' \
  crates/ --include='*.rs' | grep -v session_genesis.rs | grep -v 'use \|pub use '
# Should match only CALL sites, never a re-implementation
```

---

## 8 · Drift Detection (FR-10)

The `ContainerIdentitySled` struct derives `schemars::JsonSchema`. At build
time (or as a `const fn`), the canonical JSON schema is serialized and hashed:

```rust
// crates/op-plugins/src/state_plugins/identity_sled.rs

/// Content hash of the record shape. Changes when any field is added, removed,
/// reordered, or retyped. Consumers compare at load time.
pub const SCHEMA_CONTENT_HASH: &str = include_str!(concat!(env!("OUT_DIR"), "/identity_sled_schema_hash.txt"));
```

Generated by `build.rs`:
```rust
let schema = schemars::schema_for!(ContainerIdentitySled);
let canonical = serde_json::to_string(&schema).unwrap();
let hash = sha2::Sha256::digest(canonical.as_bytes());
std::fs::write(out.join("identity_sled_schema_hash.txt"), hex::encode(hash)).unwrap();
```

**Detection points:**
1. `ensure_hydrated`: compares Cozo record's stored schema hash against
   `SCHEMA_CONTENT_HASH`. Mismatch → log error, skip record (forces
   re-registration rather than silent misinterpretation).
2. SHM projection manifest: `write_projection` includes the hash in
   `.manifest.json`. External readers (schema_router, op-web) can detect
   staleness.
3. CI test: mutate a field in `ContainerIdentitySled` → assert hash changes →
   assert hydration rejects records with old hash.

**Schema version — two distinct facts, one author each.** An earlier draft
derived `schema_version: u32` from "a monotonic counter incremented by
`build.rs` when the hash changes." A build script has no memory across clean
builds, so unless the counter is committed to the tree two machines building
the same source emit different versions — a hand-maintained fact wearing a
generated costume. Corrected:

| Fact | What it answers | Author | Changes |
|---|---|---|---|
| `schema_version: u32` | *Which record format is this?* — the legacy discriminator §6.5 relies on (`≤ 2` = old `hashed_footprint` record, `3` = genesis record) | A single `const RECORD_FORMAT: u32 = 3;` declared **in the record definition itself**, next to the fields it describes | Only on a deliberate format generation — rare, human-set, reviewed |
| `SCHEMA_CONTENT_HASH` | *Is this exactly the shape I compile against?* — drift detection | Generated from the canonical serialization of the record definition | Automatically, on any field add/remove/reorder/retype |

No build-time counter, no committed counter file, no cross-machine
nondeterminism. Both values come from the one definition; no consumer declares
either independently, and no call site hard-codes a version literal — they read
`RECORD_FORMAT` and `SCHEMA_CONTENT_HASH`.

The ordinal cannot be replaced by the hash: §6.5's legacy comparison needs
ordering (`version ≤ 2`), and a hash has none. The hash cannot be replaced by
the ordinal: an ordinal only moves when a human moves it, which is exactly the
silent-drift failure FR-10 exists to catch. They are different facts and both
are needed.

---

## 9 · Header Migration (OQ-4 + OQ-6)

### Phase 1: Stamp genesis, accept both

```
Interceptor reads (for legacy/UDS path):
  1. x-ghostbridge-genesis       (new, preferred)
  2. x-ghostbridge-footprint     (legacy, accepted during transition)

Both are compared against the stored genesis in the session record.
Assertion path: no header needed — genesis looked up server-side.
```

- `grpc_web.rs` ALLOW_HEADERS gains `x-ghostbridge-genesis`.
- UDS injector stamps `x-ghostbridge-genesis` (reads from session record).
- Assertion path stamps genesis into extensions (no wire header from client).
- Legacy footprint path: value compared to stored genesis, not recomputed.

### Phase 2: Fail-closed

- Gate rejects absent genesis for UDS/legacy path (sentinel deleted).
- Assertion path requires assertion (genesis alone insufficient).
- `x-ghostbridge-footprint` acceptance removed from interceptor.
- ALLOW_HEADERS entry for `x-ghostbridge-footprint` retained for
  backward-compat preflight but value is ignored by the gate.

---

## 10 · Genesis Redaction (FR-11 Compensating Control)

The genesis hex is classified as sensitive (bearer-equivalent during Phase 1
transition). It **shall not** appear in:
- `tracing` spans or events at any level (info, warn, debug, trace)
- Structured log fields
- gRPC response metadata
- Error messages returned to callers

Implementation: the genesis is stored as `String` (hex) in the session record
and compared as a string. It is never bound to a `tracing` field. A test greps
the crate source for any `tracing::` macro invocation that references a
variable named `genesis` or `genesis_hex` and fails if found.

---

## 11 · `mint_genesis` Function Design

```rust
// crates/op-identity/src/session_genesis.rs

use blake3::Hasher;

/// Mint a session genesis — the immutable identity anchor.
///
/// Called exactly once per session, at arrival (first authenticated mutation).
/// The output is stored and never recomputed.
///
/// All inputs are raw bytes — no encoding ambiguity. Callers must decode
/// base64 pubkeys and hex hashes before calling.
///
/// OSCAL subid: mut.service.session-genesis.mint@v1
pub fn mint_genesis(
    wg_pubkey: &[u8; 32],        // decoded from base64
    chain_head_hash: &[u8; 32],  // decoded from hex (EventChain.last_hash())
    head_timestamp: i64,          // unix seconds of the chain head event
    catalog_hash: &[u8; 32],     // from schema_catalog_hash()
    arrival_timestamp: i64,       // Utc::now().timestamp() at mint time
) -> [u8; 32] {
    let mut hasher = Hasher::new();
    hasher.update(wg_pubkey);
    hasher.update(chain_head_hash);
    hasher.update(&head_timestamp.to_le_bytes());
    hasher.update(catalog_hash);
    hasher.update(&arrival_timestamp.to_le_bytes());
    *hasher.finalize().as_bytes()
}
```

**Properties:**
- Pure function, no I/O, no side effects.
- Deterministic: same inputs → same output.
- All inputs are `&[u8; 32]` or `i64` — no encoding ambiguity. Both
  `chain_head_hash` and `catalog_hash` are decoded to raw bytes by the caller,
  eliminating the hex-vs-raw inconsistency.
- The function takes `&[u8; 32]` for the pubkey — callers must decode base64
  before calling.

---

## 12 · Per-Session Record Shape (version 3)

Extension to `ContainerIdentitySled`:

```rust
pub struct ContainerIdentitySled {
    // Existing fields retained:
    pub session_id: String,
    pub wireguard_pubkey: String,
    pub mutation_index: u64,
    pub trace_id: String,
    pub schema_version: u32,          // = RECORD_FORMAT (3); format discriminator,
                                      // not the drift check — see §8
    pub expires_at: Option<i64>,
    pub last_seen_at: i64,
    pub active: bool,
    pub peer_ip: Option<String>,
    pub session_started_at: i64,
    // ... other existing fields (interface, vector_id, sealed_id, etc.)

    // REPLACES hashed_footprint (§6.5):
    #[serde(alias = "hashed_footprint")]
    pub genesis: Option<String>,         // hex blake3, immutable after first write

    // NEW fields (version 3):
    pub arrival_timestamp: i64,          // unix seconds, stored for re-verification
    pub chain_head_at_arrival: String,   // hex hash of chain head at genesis time
    pub catalog_hash_at_arrival: String, // hex hash of catalog at genesis time
    pub head_timestamp_at_arrival: i64,  // timestamp of chain head event
}
```

**Field ownership (FR-6):**

| Field | Writer | Write pattern |
|-------|--------|---------------|
| `genesis` | Mutation engine | Once (arrival via `mint_and_store_genesis`), then immutable |
| `arrival_timestamp` | Mutation engine | Once (arrival), then immutable |
| `chain_head_at_arrival` | Mutation engine | Once (arrival), then immutable |
| `catalog_hash_at_arrival` | Mutation engine | Once (arrival), then immutable |
| `head_timestamp_at_arrival` | Mutation engine | Once (arrival), then immutable |
| `mutation_index` | Mutation engine | Advance-only, every mutation |
| `last_seen_at` | Stream path | Touch on every lifecycle event |
| `active` | Stream path | Set/clear on connect/disconnect |
| `peer_ip` | Stream path | Set on connection |
| `session_started_at` | Stream path | Set once at session open |

---

## 13 · `event_to_footprint` Enhancement (FR-3)

```rust
fn event_to_footprint(event: &ChainEvent, session: &SessionContext) -> PluginFootprint {
    let mut metadata: HashMap<String, simd_json::OwnedValue> = HashMap::new();
    metadata.insert(
        "actor_id".to_string(),
        simd_json::OwnedValue::from(event.actor_id.as_str()), // registered principal_id
    );
    // ... existing fields (capability_id, method_name, etc.) ...

    // Session identity stamp (FR-3)
    metadata.insert(
        "session_genesis".to_string(),
        simd_json::OwnedValue::from(session.genesis_hex.as_str()),
    );
    metadata.insert(
        "session_id".to_string(),
        simd_json::OwnedValue::from(session.session_id.as_str()),
    );
    metadata.insert(
        "wireguard_pubkey".to_string(),
        simd_json::OwnedValue::from(session.wireguard_pubkey.as_str()),
    );

    // ... rest unchanged ...
}
```

The `SessionContext` is derived from the request's extensions
(`GhostbridgeIdentity` or `HumanPrincipalIdentity`), which carry the genesis
after verification. The chain stamper never recomputes the genesis — it embeds
the stored output. `ChainEvent.actor_id` is the already-resolved registered
`principal_id`; no footprint/genesis/hash derivation occurs here.

### 13.1 Footprint and Snowball Hash Boundary

```text
VerifiedIdentity { principal_id, session_id, session_genesis }
  → authorized MutationEngine event
  → sled emits canonical PluginFootprint payload
       metadata.actor_id = principal_id
       metadata.session_id = session_id
       metadata.session_genesis = session_genesis
  → Shuttle delivers the same payload
       ├─ Snowball append computes
       │    H(domain || previous_event_hash || canonical_payload_bytes) once
       │    and returns {event_id, event_hash}
       └─ vectorization renders canonical payload text and records the receipt
```

The footprint is the envelope shown above, not an identity and not a precomputed
current-event digest. The previous event hash is the normal chain-link input; the
current payload itself is not first reduced to `json_args_footprint`, `data_hash`, or
`content_hash` and then hashed again. `json_args_footprint` is removed and canonical
bounded/redacted arguments occupy the payload. The receipt never feeds back into
authentication or authorization.

---

## 14 · UDS Injector Rework (FR-5, shared_socket.rs)

**Peer credential → session_id derivation:**

`SO_PEERCRED` provides `pid`, `uid`, `gid` of the connecting process. The
session_id derivation:

1. **Host process (uid matches bridge's own uid, or uid 0):** use the host's
   own session_id (derived from the host's WireGuard pubkey via
   `derive_session_id`).
2. **Container process (uid != bridge uid, uid != 0):** Incus maps container
   root to a subuid range. The bridge builds a uid→container_name map at
   startup by reading Incus subuid allocations (`/etc/subuid` + `incus list
   --format json` → extract name + volatile.idmap). This map is static for
   the process lifetime (containers are not created/destroyed at high
   frequency). Container name = session_id.

**Sync interceptor with async lookup:**

The UDS identity interceptor (`Interceptor::call` is sync) uses the same
`block_in_place` + `Handle::current().block_on(...)` pattern already
established by `verify_per_identity` (interceptor.rs:236):

```rust
pub fn uds_identity_interceptor(req: Request<()>) -> Result<Request<()>, Status> {
    let cred = extract_peer_cred(&req)
        .ok_or_else(|| Status::unauthenticated("UDS peer credentials unavailable"))?;

    let session_id = resolve_session_from_uid(cred.uid());  // static map lookup, O(1)

    let genesis_hex = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let engine = ENGINE.read().expect("lock").clone()?;
            let result = crate::identity_sled_dispatch::dispatch_identity_sled_method(
                engine.as_ref(), "get_identity",
                &serde_json::json!({ "session_id": session_id }),
            ).await.ok()?;
            result.get("identity")?.get("genesis")?.as_str().map(str::to_string)
        })
    });

    match genesis_hex {
        Some(genesis) if !genesis.is_empty() => {
            // Inject x-ghostbridge-genesis so downstream interceptor passes
            let mut req = req;
            let header_val = genesis.parse::<MetadataValue<Ascii>>()
                .map_err(|_| Status::internal("genesis encode failed"))?;
            req.metadata_mut().insert("x-ghostbridge-genesis", header_val);
            Ok(req)
        }
        _ => Err(Status::failed_precondition("Session genesis not yet minted")),
    }
}
```

---

## 15 · Offline Re-Verification Algorithm

Given a session record and a chain segment, an auditor verifies:

```
1. Extract from session record:
   - genesis, arrival_timestamp, chain_head_at_arrival,
     head_timestamp_at_arrival, catalog_hash_at_arrival, wireguard_pubkey

2. Recompute:
   expected = mint_genesis(
       decode_base64(wireguard_pubkey),
       decode_hex(chain_head_at_arrival),
       head_timestamp_at_arrival,
       decode_hex(catalog_hash_at_arrival),
       arrival_timestamp
   )

3. Assert: expected == decode_hex(genesis)

4. Verify chain segment ancestry:
   - The committed head (chain_head_at_arrival) is an ANCESTOR of the
     segment's first event — NOT necessarily its immediate parent.
   - Rationale: between the head read (step 3 of mint) and the arrival event
     being appended, other sessions may have appended events. The genesis
     commits to a head that existed at the moment of arrival, not to the
     immediate predecessor of the arrival event.
   - Verification: walk prev_hash links backward from the segment's first
     event; confirm chain_head_at_arrival appears in that chain. This is
     O(gap) where gap = number of intervening events (typically small;
     bounded by concurrency × latency between head read and append).

5. Verify segment internal integrity:
   - Each event in segment has prev_hash == previous event's event_hash
   - All events in segment have metadata.session_genesis == genesis

6. Verify completeness:
   - Segment is bounded: starts at the session's first event (the arrival
     event itself), ends at session teardown event or last mutation before
     expires_at.
   - No duplicate event_ids within the segment.
```

No SHM read. No network. Pure data verification from durable chain + session
record.

---

## 16 · Test Strategy

| Test | Location | Verifies |
|------|----------|----------|
| `mint_genesis_deterministic` | `op-identity/src/session_genesis.rs` | Same inputs → same output |
| `mint_genesis_uniqueness` | same | Different arrival_ts → different genesis |
| `mint_genesis_all_bytes` | same | All inputs as raw `[u8; 32]`, no encoding ambiguity |
| `genesis_not_reminted` | `op-grpc-bridge` integration | Second mutation reads stored genesis, doesn't call mint |
| `absent_header_rejected` | interceptor unit | No genesis header → UNAUTHENTICATED |
| `mismatched_genesis_rejected` | interceptor unit | Wrong genesis → UNAUTHENTICATED |
| `sentinel_removed` | grep gate | No `SENTINEL_FOOTPRINT` in codebase |
| `chain_carries_session_identity` | mutation_engine unit | `event_to_footprint` metadata has principal plus all three session fields |
| `chain_carries_principal_metadata` | mutation_engine unit | `actor_id` is the resolved `principal_id`, not a derived footprint/hash |
| `one_payload_one_chain_hash` | op-snowball integration | Sled/Shuttle payload is hashed by Snowball once; no prehashed current-event body |
| `vectorization_uses_payload` | chain-vector integration | Embedding text contains canonical payload; receipt is provenance only |
| `single_plugin_footprint_type` | semantic/grep gate | Duplicate legacy footprint generator and `data_hash → content_hash` path are gone |
| `chain_sliceable_by_session` | integration | Two sessions interleaved, filter recovers each |
| `offline_reverification_with_gap` | integration | Ancestor check passes when intervening events exist between head and arrival |
| `stream_does_not_overwrite_genesis` | identity_sled_dispatch | Stream write preserves genesis + mutation_index |
| `mutation_does_not_overwrite_liveness` | identity_sled_dispatch | Mutation write preserves last_seen_at + active |
| `reauth_new_genesis` | integration | Same pubkey, new session → different genesis |
| `expired_session_rejected` | interceptor | Valid genesis for expired session → PERMISSION_DENIED |
| `genesis_not_in_logs` | grep gate | No `tracing::` macro references genesis value |
| `schema_hash_drift` | build test | Field mutation → hash changes → hydration rejects |
| `no_global_sled_writers` | grep gate | No `write_sled_from_wg` / `write_sled_full` outside feature gate |
| `single_genesis_author` | grep gate | blake3 genesis invocation only in `session_genesis.rs` |
| `assertion_bound_genesis` | interceptor integration | Genesis without assertion rejected (Phase 2) |
| `etch_footprint_deleted` | grep gate | No `etch_footprint` call site anywhere |
| `write_identity_mints_genesis` | identity_sled_dispatch | Provisioning calls mint_genesis, not etch_footprint |
| `uid_to_session_mapping` | shared_socket unit | Known uid resolves to correct session_id |

---

## 17 · Migration Sequence (OQ-6 Operationalized)

### Phase 1 (this implementation)

1. Implement `mint_genesis`, session record v3, chain stamping.
2. Interceptor accepts BOTH `x-ghostbridge-genesis` AND legacy
   `x-ghostbridge-footprint` (compared to stored genesis).
3. For assertion-carrying traffic: genesis looked up server-side (no header
   from client needed).
4. UDS injector stamps `x-ghostbridge-genesis`.
5. `write_identity` (provisioning) calls `mint_genesis` instead of
   `etch_footprint`.
6. Seven global-sled writers deleted.
7. `etch_footprint` deleted.
8. Global sled file no longer written or read by any path.
9. Existing version ≤ 2 records accepted via `serde(alias)` on `genesis` field.

### Phase 2 (separate deployment, after confirming all sessions stamp)

1. Remove legacy `x-ghostbridge-footprint` acceptance from interceptor.
2. Remove `SENTINEL_FOOTPRINT` and zero-fill paths.
3. Require assertion alongside genesis for non-UDS traffic (FR-11 option 1
   enforced).
4. Clean up version ≤ 2 record compatibility (optional — records will
   naturally roll over as sessions expire and re-auth).

---

## 18 · Crate Dependency Changes

None. `blake3` and `sha2` are already workspace dependencies. No new external
crates.
