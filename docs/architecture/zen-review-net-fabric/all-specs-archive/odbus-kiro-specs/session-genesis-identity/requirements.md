# Requirements — Session Genesis Identity

> **Split the two meanings.** Genesis is the session anchor — minted once, at
> login, never recomputed. Footprint goes back to meaning the snowball record
> — one per mutation, carrying the genesis as its session stamp. The header is on
> every packet. Constant for the life of the session. Verified with a single
> equality. Fail closed.

| | |
|---|---|
| Status | Draft — amended after review 2026-08-16. **Amendments are binding; do not revert them.** FR-4 rewritten (one authoritative store), OQ-1 reopened for the xray-terminated path, FR-11 added (replay), FR-12 added (session lifetime), FR-6 consequence stated, §9 schema-version row corrected, one deletion rationale corrected |
| Extends | `netmaker-xray-identity-handoff/` (locked) |
| Respects | `3tched-ghostbridge-control-plane/` (topology lock) |
| Crates | `op-identity`, `op-grpc-bridge`, `op-state-store`, `op-snowball` |
| Supersedes | The "footprint = identity anchor" interpretation; `anna_scribe.rs` duplicate reader; `etch_footprint`'s index+port terms; the zeros sentinel; all seven global-sled writer sites |

---

## 1 · Problem Statement

The identity value in the per-request header (`x-ghostbridge-footprint`) is
**recomputed on every mutation** — it incorporates the global `mutation_index`
(currently ~1.9 M and moving constantly) and a `source_port` that is
structurally zero. A.N.N.A. Scribe rejects live callers with "Temporal Hash
Mismatch" because the header is stale before the client can present it.

Two unrelated concepts share the name "footprint":
1. The **identity anchor** (the header the interceptor checks), which chases a
   moving global counter and therefore cannot stay stable.
2. The **snowball record** (`PluginFootprint` in `op-snowball`), which
   carries no session identity at all — no pubkey, no session_id, no genesis —
   making the chain unsliceable per session and unverifiable offline.

The global SHM sled (`/dev/shm/plugin_schema.dat`, 152 bytes, last-write-wins)
is shared by all sessions and has **seven writer call sites across five
processes**, all last-write-wins on the same file:

| # | Call site | Process | Effect |
|---|-----------|---------|--------|
| 1 | `op-cognitive-mcp/src/main.rs:66` | op-cognitive-mcp | `write_sled_from_wg` — process-local counter starting at 0 |
| 2 | `op-mcp/src/main.rs:113` | op-mcp | `write_sled_from_wg` — same zero-counter race |
| 3 | `op-mcp/src/compact.rs:585` | op-mcp | `write_sled_from_wg` — compaction path |
| 4 | `op-grpc-bridge/src/identity_sled_dispatch.rs:465` | op-grpc-bridge | `write_sled_from_wg` — dispatch refresh |
| 5 | `op-grpc-bridge/src/mutation_engine.rs:514` | op-grpc-bridge | `write_sled_full("", event_id, "")` — chain position only |
| 6 | `op-identity/src/bin/op-identity-sled.rs:52` | op-identity-sled | `write_sled_from_wg` — CLI tool |
| 7 | `op-identity/src/schema_bridge.rs:1381` | (handshake monitor thread) | `write_sled_from_wg` — WG watcher callback |

Seven writers cannot be made to agree; one writer per field cannot disagree.
Adding the stream without retiring the others produces nothing. The per-session
store (`identity_sled.json`) disagrees with the global sled. The UDS injector
(`CanonicalPeerIdentity::from_sled`) stamps whichever session mutated last for
*all* callers. `anna_scribe.rs` re-opens the same file and re-derives the same
hash from it — a second reader of a source that is already wrong.

**Root cause:** every defect in this feature is a transcription defect, not a
logic defect — two hash formulas for one identity, two stores for one
footprint, a `#[repr(C)]` struct and a schema describing the same 152 bytes, a
validator holding component names typed out from a vocabulary that had already
moved on. The spec succeeds if every fact has one author and no transcribers.

---

## 2 · Hard Constraints (inherited, not re-litigated)

| ID | Constraint | Source |
|---|---|---|
| HC-1 | D-Bus is the only control plane; new capability = plugin in `default_registry.rs` | CLAUDE.md |
| HC-2 | PluginSchema is the single source of truth; derived values computed in one function, one place | CLAUDE.md |
| HC-3 | The sealed blob IS the record; sole writer is op-blob; consumers read SHM | CLAUDE.md |
| HC-4 | Reactive, not polled; no watchers, no polling loops, no `Command::new` | CLAUDE.md |
| HC-5 | Durability stays inline on the dispatch path (NFR-4 of handoff spec) | handoff spec |
| HC-6 | Ed25519 oracle assertion remains the cryptographic binding of pubkey to peer; genesis does not replace it — genesis binds identity to state and moment | PROMPT §5 |
| HC-7 | Rust-first; no new Python | CLAUDE.md |
| HC-8 | Every new object carries uuid + subid from seven-category taxonomy | CLAUDE.md |
| HC-9 | No WireGuard on main host; no SNI front on public :443; gRPC at 10.0.0.2:8090 mesh-private | topology lock |
| HC-10 | No new gRPC proto service packages; no per-service TCP ports | CLAUDE.md |
| HC-11 | OVS packet tagging is not the identity carrier **for the host→container hop** — containers are nic-less and that hop is a UDS with no packets to steer. Note the corrected premise: the WireGuard `netmaker` interface **is** an `ovsbr0` port (the mesh↔fabric junction, carried by L3→`encap(ethernet)` flows), so mesh traffic *does* traverse OVS. "OVS sees nothing" is false in general; it is true only of the container hop | PROMPT §7, corrected 2026-08-16 against the live fabric |
| HC-12 | No TransportBindingIndex (source-IP table populated by handshake watcher) | handoff spec §1.3 |
| HC-13 | One writer per field — seven writers cannot be made to agree; agreement must be structural (derived from the same event) not maintained | follow-up constraint 1 |
| HC-14 | Nothing restated — each fact authored in exactly one place; the PluginSchema, the `#[repr(C)]` fixed layout, tool inputs, gRPC shapes, UI renderers, validators all generated or derived from that one author | follow-up constraint 2 |
| HC-15 | Drift must be caught — every generated artifact carries a content hash in its manifest; shape changes are announced by hash change, never silent divergence | follow-up constraint 3 |

---

## 3 · Functional Requirements

### FR-1: Genesis minted exactly once per session

**When** a session arrives (first authenticated mutation or explicit login),
**the system shall** compute a genesis value:

```
genesis = blake3( wg_pubkey ‖ chain_head_hash ‖ head_timestamp ‖ catalog_hash ‖ arrival_timestamp )
```

**and** store it immutably in the session record.

The genesis **shall not** be recomputed, re-derived, or updated for the
lifetime of that session. Subsequent mutations for the same session read the
stored genesis; they never call the genesis function again.

**Terms:**
- `wg_pubkey` — the WireGuard public key (32 bytes), the cryptographic root.
- `chain_head_hash` — `EventChain.last_hash()` at arrival (O(1) read, prev_hash-linked).
- `head_timestamp` — the timestamp of the chain head event.
- `catalog_hash` — the published schema catalog hash from the blob manifest.
- `arrival_timestamp` — wall-clock unix seconds at the moment of minting; the uniqueness
  term that separates two logins landing at the same chain head.

**Acceptance criteria:**
- [ ] A single function `mint_genesis(pubkey, chain_head, head_ts, catalog_hash, arrival_ts) -> [u8; 32]` exists in one place.
- [ ] Calling the same function with the same inputs produces the same output (deterministic).
- [ ] No code path ever re-mints genesis for an existing session.
- [ ] The arrival_timestamp is stored in the session record and is not re-derivable from other fields.

### FR-2: Header present on every request, fail-closed on absence

**When** a request arrives at the gRPC bridge with the oracle identity
assertion path active (per the handoff spec), **the system shall** stamp the
genesis into the session identity that downstream handlers observe.

**When** a request arrives via the legacy footprint path, **the system shall**
require `x-ghostbridge-genesis` (or the transitional `x-ghostbridge-footprint`
during migration) to match the stored genesis for that session.

**When** the genesis header is absent or does not match the session record,
**the system shall** reject the request with `UNAUTHENTICATED`. The zeros
sentinel (`SENTINEL_FOOTPRINT` of 64 zeros stamped with a warning) **shall be
removed**; absent identity is a hard rejection, not a degraded pass.

**Acceptance criteria:**
- [ ] Every accepted request carries a verified genesis in its extensions.
- [ ] A request with no genesis/footprint header is rejected (not warned-and-passed).
- [ ] A request with a genesis that differs from the session record is rejected.
- [ ] The zeros sentinel code path is deleted.

### FR-3: Chain record carries session identity for per-session slicing

**When** the mutation engine persists an audit event to the snowball,
**the system shall** include in the `PluginFootprint` metadata:

- `session_genesis` — the genesis hash of the session that delivered this mutation.
- `session_id` — the session identifier (container name / derived id).
- `wireguard_pubkey` — the pubkey that owns this session.

**so that** the durable chain can be:
1. Sliced by session (filter by `session_genesis` or `session_id`).
2. Re-verified offline from the durable data alone — given the stored genesis
   and the chain segment, an auditor can confirm the session's mutations form
   an unbroken prev_hash chain anchored at the genesis's committed head.

**Acceptance criteria:**
- [ ] `event_to_footprint` includes `session_genesis`, `session_id`, `wireguard_pubkey` in the metadata map.
- [ ] A test demonstrates slicing: given two sessions' interleaved mutations, filtering by `session_genesis` recovers each session's chain segment.
- [ ] A test demonstrates offline re-verification: from the stored genesis fields and the chain segment, the genesis can be recomputed and the prev_hash linkage validated without any SHM read.

### FR-4: Exactly one authoritative store for gate decisions

**Correction to an earlier draft of this requirement.** The earlier wording
("verification shall not read any SHM file") was both self-contradictory with
FR-8 and factually wrong about the live code, and following it would create a
third store of one fact — the precise failure this spec exists to eliminate.

Verified live read path: `get_identity`
(`identity_sled_dispatch.rs:299-328`) reads the plugin state cache, which is
`/dev/shm/opdbus/state/identity_sled.json`, hydrated **once** from Cozo at
`/var/lib/op-dbus/identity-cozo` (`ensure_hydrated`, same file, lines 156-181).
The gate therefore already reads SHM today, and per HC-3 it **should** — the
sealed blob in SHM is the record.

**The system shall** designate exactly one authoritative store for gate
decisions: **the sealed per-session identity blob in the SHM catalog** (HC-3).
Every other representation is a projection with a stated role:

| Store | Role | May the gate read it? |
|---|---|---|
| Sealed identity blob in the SHM catalog | **Authoritative** for gate decisions | Yes — this is the read |
| Cozo (`/var/lib/op-dbus/identity-cozo`) | Durability + hydration on cold start | No — hydration only, never per-request |
| `/dev/shm/plugin_schema.dat` (global 152-byte sled) | **Dead.** Retired by FR-6 | No — never |

**The system shall not** consult the global 152-byte sled for any acceptance
decision, and shall not introduce any further store of the genesis. The ban is
on the global last-write-wins sled, **not** on SHM as such.

**Acceptance criteria:**
- [ ] Exactly one store is named authoritative in design.md, and every other named store is labelled a projection with its role.
- [ ] The per-request verification path performs one read of the authoritative store and one equality comparison — no hashing, no second store consulted, no fallback chain.
- [ ] No per-request code path opens `/dev/shm/plugin_schema.dat`.
- [ ] Cozo is read at hydration only; a test confirms no Cozo query occurs on the per-request path.
- [ ] `op_identity::verify_ghostbridge_footprint` (the global SHM reader) is off the acceptance path entirely — it may remain for diagnostics but can never cause acceptance.

### FR-5: Exactly one function computes genesis; exactly one component reads the sled

**The system shall** have exactly one function that computes the genesis hash
(FR-1's `mint_genesis`). No other code path — not `anna_scribe.rs`, not
`etch_footprint`, not `shared_socket.rs` — shall derive or re-derive the
session anchor.

**The system shall** have exactly one component that reads the per-session
identity record for gate decisions: the per-identity lookup in
`identity_sled_dispatch.rs` (via the `get_identity` method on the
`identity_sled` plugin). `CanonicalPeerIdentity::from_sled()` in
`shared_socket.rs` shall read the session-specific record (not the global SHM
sled) or be refactored to use the same lookup.

**Acceptance criteria:**
- [ ] `anna_scribe::notarize_arrival` is deleted or deprecated (its `File::open("/dev/shm/plugin_schema.dat")` is the duplicate reader).
- [ ] `etch_footprint` is no longer called for identity-anchor purposes (it may survive for the snowball footprint record if needed, renamed for clarity).
- [ ] `CanonicalPeerIdentity::from_sled()` no longer reads the global 152-byte SHM file; it reads the session-specific record.
- [ ] A grep-based gate confirms at most one function whose name or doc says "genesis" / "session anchor".

### FR-6: One writer per field — retire all seven global-sled writers

**The system shall** retire all seven `write_sled_from_wg` / `write_sled_full`
call sites that write to the shared 152-byte global SHM sled. The global sled
file (`/dev/shm/plugin_schema.dat`) ceases to be the identity source of truth.

**Required end state — two writers, disjoint field ownership:**

| Writer | Fields owned | Trigger | Location |
|--------|-------------|---------|----------|
| Mutation engine (inline on dispatch) | `mutation_index`, `genesis` (written once at arrival), chain position | Every mutation; genesis only at session start | `mutation_engine.rs` |
| Stream / metadata path (tokio context) | `last_seen_at`, `active`, `peer_ip`, `session_started_at`, span accumulation | Connection lifecycle events | identity_sled_dispatch (stream arm) |

**Neither writer shall** overwrite the other's fields. The mutation path uses
advance-only semantics for `mutation_index`; the stream path uses touch
semantics for liveness fields. Agreement is structural — both derive from the
same event, so they cannot disagree.

**Enumeration of sites to retire:**

| # | Site | Disposition |
|---|------|-------------|
| 1 | `op-cognitive-mcp/src/main.rs:66` | Delete — MCP process has no authority over chain position |
| 2 | `op-mcp/src/main.rs:113` | Delete — same |
| 3 | `op-mcp/src/compact.rs:585` | Delete — compaction does not produce identity |
| 4 | `op-grpc-bridge/src/identity_sled_dispatch.rs:465` | Replace with per-session Cozo write (stream path) |
| 5 | `op-grpc-bridge/src/mutation_engine.rs:514` | Replace with per-session Cozo write (mutation path, carries genesis) |
| 6 | `op-identity/src/bin/op-identity-sled.rs:52` | Delete or convert to diagnostic-only (does not feed the gate) |
| 7 | `op-identity/src/schema_bridge.rs:1381` (handshake monitor) | Delete — polling watcher violates HC-4; identity does not come from WG handshake events |

The mutation engine **shall** mint and store the genesis on the first mutation
of a session (arrival = mutation one). This is the inline durability path
(HC-5). The stream feeds only what tolerates a gap.

**Stated consequence, not a side effect:** arrival being mutation one means
**every login writes to the chain**, including a session that goes on to do
nothing but read. This is intended — logins become durable, auditable, and
sliceable like any other mutation, and the session's span has a real first
element. design.md shall acknowledge the resulting chain growth (one event per
login) and confirm it is acceptable at the expected session rate rather than
leaving it to be discovered later.

**Acceptance criteria:**
- [ ] No call to `write_sled_from_wg` or `write_sled_full` remains in any path that feeds the identity gate.
- [ ] A test demonstrates that a stream-path write does not zero or overwrite `genesis` or `mutation_index`.
- [ ] A test demonstrates that a mutation-path write does not zero or overwrite `last_seen_at` or `active`.
- [ ] The session record assembles and stamps on the same thread the first mutation is on (inline, not deferred).
- [ ] `write_sled_from_wg` and `write_sled_full` are either deleted entirely or behind a `#[cfg(feature = "legacy-sled")]` gate that is off by default.

### FR-7: Genesis composes with Ed25519 oracle assertion

**When** the oracle identity assertion (per the handoff spec) is present,
**the system shall** use it as the cryptographic proof of identity (who), and
attach the genesis as the state-and-moment binding (when + what state).

The composition is:
1. Assertion validates the *peer* (pubkey → registered HumanPrincipal).
2. Genesis validates the *session* (that peer, at this chain position, at this
   moment).

**When** a valid assertion is presented for a session that has not yet had its
genesis minted, **the system shall** treat that as the session arrival and mint
genesis inline before returning success.

**Acceptance criteria:**
- [ ] A valid assertion for an active session with a stored genesis succeeds; the genesis is in extensions.
- [ ] A valid assertion for a new session triggers genesis minting; subsequent requests use the stored value.
- [ ] The assertion path does NOT require the legacy footprint header.
- [ ] Genesis lookup failure for an existing session (corruption) rejects the request.

### FR-8: Identity reaches xray out-of-band as sealed blob

**The system shall** deliver identity to the xray container as a sealed blob in
the SHM catalog, not as an in-band header injection. The component that first
sees the WireGuard key (the oracle decoy / bridge arrival path) seals an
identity blob; the bridge's UDS injector joins on it and stamps the header
after TLS termination.

Xray remains a stock passthrough (HC from handoff spec: "Xray remains
passthrough"). It cannot inject HTTP headers and arbitrary preamble bytes on
its inbound socket would corrupt the ClientHello.

**Acceptance criteria:**
- [ ] No code in `op-xray-daemon` injects identity headers or reads identity state.
- [ ] The identity blob is written to the SHM catalog at session arrival.
- [ ] The UDS injector reads from the session-specific record (FR-5), not from a global sled.

### FR-9: Nothing restated — single-author, generation-only derivation

**For each fact the system introduces** — genesis formula, session record
shape, chain position, catalog binding — **there shall be exactly one
authoritative source** (the single author). Every other representation is
generated or derived from that source, never hand-transcribed.

Specifically:
- The `IdentitySled` `#[repr(C)]` struct (the fixed SHM layout) and the
  `ContainerIdentitySled` PluginSchema (the reflectable schema) **shall be
  generated from a single definition**. The fixed layout is retained because
  two properties are required simultaneously: the gate needs a fixed-layout
  byte comparison per packet (dynamic JSON parsing at the gate is not
  acceptable), and PluginSchema must live in exactly one place. Generation
  satisfies both.
- The genesis formula exists in `mint_genesis` only. The validator, the
  interceptor, and the chain stamper all consume its output — none of them
  re-state the formula.
- Tool inputs, gRPC shapes, UI renderers, and validators that reference
  identity fields **shall** derive them from the PluginSchema, not from
  hand-typed string constants.

**Acceptance criteria:**
- [ ] The design names, for each introduced fact, the single author and lists every consumer that derives it.
- [ ] No consumer holds its own copy of the genesis formula, the record field names, or the schema version constant.
- [ ] A CI gate (grep or AST) confirms that no file outside the single-author location contains the genesis blake3 invocation.

### FR-10: Drift caught by content hash

**Every generated artifact** (the `#[repr(C)]` layout, the SHM projection, the
gRPC field list) **shall** carry a content hash (sha256) in its manifest or
header, following the pattern already proven in
`schemas/json-render/catalog.schema.json`.

**When** the source definition changes, the generated artifact's hash changes,
and any component loading a stale artifact **shall** fail loudly (log + reject)
rather than silently diverging.

**Acceptance criteria:**
- [ ] The identity record schema exports a `SCHEMA_CONTENT_HASH` constant derived from its canonical serialization.
- [ ] The `#[repr(C)]` sled layout and the Cozo schema both embed or reference this hash.
- [ ] A test confirms that mutating a field in the source definition changes the hash and causes a mismatch detection.

### FR-11: Replay — a constant header is a bearer credential

A genesis that is constant for the life of the session is, by construction, a
bearer credential: anyone who observes the header can present it until the
session ends. This is the cost of "every packet carries the header, verified by
one equality," and it must be paid explicitly rather than left unstated.

Today's churn limited the replay window accidentally. Removing the churn
removes that accident. Everything on the path sees the value — including four
unversioned Python relays (`sni-demux.py`, `socket-relay`, `nm-api-tls.py`,
`nm-warp-egress-proxy.py`) and any log or trace that captures headers.

**The system shall** adopt one of the following, and design.md shall state
which and why:

1. **Assertion-bound (recommended).** The genesis is accepted only alongside a
   valid, unexpired Ed25519 oracle assertion. The genesis is then not a
   credential at all — it is the state-and-moment binding on an identity that
   was independently proven, exactly the composition in FR-7. The transitional
   header-only path in FR-2 becomes the only replayable surface and is time-boxed
   by the OQ-6 rollout.
2. **Scoped/rotated.** The genesis stays the session anchor but the presented
   header is a short-lived value derived from it, rotated on a stated interval.
   Costs a derivation per rotation and reintroduces a moving value — the thing
   this spec removed — so it must be justified against option 1.
3. **Accepted and documented.** Replay within a session is an accepted risk, with
   the reasoning, the compensating controls (transport confidentiality, header
   redaction in logs), and the blast radius written down.

**Acceptance criteria:**
- [ ] design.md names the chosen option and the reasoning.
- [ ] The genesis value is redacted from logs and traces; a test or lint confirms it is not logged at any level.
- [ ] If option 1: a request presenting a valid genesis without a valid assertion is rejected once the OQ-6 transition completes.
- [ ] The threat model states explicitly what an observer of the header can do, for the duration of a session.

### FR-12: Session lifetime, expiry, and re-authentication

The genesis is immutable **for the life of a session**, which requires the
session's life to be defined. The live records already carry `expires_at` (one
active record is set to 1787444243), and the interceptor already rejects on
expiry (`interceptor.rs:243-247`).

**The system shall** define:

- what starts a session (arrival / first authenticated mutation — see FR-6),
- what ends it (expiry, explicit teardown, container stop),
- what happens to in-flight requests at the moment of expiry,
- and that **re-authentication mints a new genesis** — a renewed session is a
  new session, with a new chain head, a new arrival timestamp, and therefore a
  new anchor. A genesis is never extended, refreshed, or reused across a
  re-auth.

**Acceptance criteria:**
- [ ] `expires_at` semantics are stated for the new record shape, including the meaning of 0/null.
- [ ] A test confirms re-authentication produces a different genesis than the prior session for the same pubkey.
- [ ] A test confirms an expired session's genesis is rejected even though it is otherwise well-formed.
- [ ] The session's chain span has a defined end, so the "complete account of the session" is bounded at both ends.

---

## 4 · Non-Functional Requirements

| ID | Requirement |
|---|---|
| NFR-1 | Genesis minting completes in < 1 ms (blake3 + one chain-head read + one timestamp). No I/O on the hot path except the Cozo write. |
| NFR-2 | Per-request verification is a single Cozo read + one `==` comparison on fixed-layout bytes. No hashing per request. No dynamic JSON parsing at the gate. |
| NFR-3 | The zeros sentinel removal and fail-closed enablement are sequenced AFTER the edge reliably stamps the header (FR-2 rollout). |
| NFR-4 | `SLED_SCHEMA_VERSION` bumps to 3 for records carrying genesis. Old records (version ≤ 2) are distinguishable by version field; existing chain records retain their values (no retroactive re-derivation). |
| NFR-5 | OSCAL subids registered for: `mut.service.session-genesis.mint@v1`, `evt.service.event-chain.session-stamp@v1`, `obs.service.identity-sled.genesis-verify@v1` (minimum; additional as needed). |
| NFR-6 | Rust-first; `anyhow::Result` for app errors, `thiserror` for rejection enum; `simd_json` preferred; rustfmt 4-space/100-col; clippy clean. |
| NFR-7 | All new behavior covered by tests. Red → green. `cargo test -p op-identity` and `cargo test -p op-grpc-bridge` pass. |

---

## 5 · Open Questions — Resolved, except OQ-1 Path B

### OQ-1: Join key (how the sealed identity blob ties to the connection xray hands over)

**Partially resolved. One of the two paths is still open and design.md must
close it.**

**Path A — UDS-only session containers (resolved): one UDS connection per
session.** The session container's name IS the sessionid and its only device is
the shared ghostbridge socket bind-mount, so the accepted connection is the
session binding. The kernel-supplied peer credential (`SO_PEERCRED`, checked at
`shared_socket.rs:145`) is the anchor. Needs nothing from xray, no PROXY v2, no
SNI.

**Path B — xray-terminated traffic (OPEN — this is the path that carries public
users).** The peer-credential binding does **not** hold here. Xray terminates
Reality TLS and dials upstream itself, so the credential on that connection
belongs to **xray**, not to the user, and HTTP/2 multiplexing means many
distinct users share one upstream connection. A per-connection binding
therefore identifies the router, not the session.

Constraints on any answer: SNI is rejected by the topology lock (no SNI front on
public :443); stock Xray-core parses PROXY v2 but does not surface custom TLVs
downstream, so a TLV cannot carry the join key through it; HC-12 forbids a
source-IP lookup table populated by a watcher.

**design.md shall state the join key for Path B explicitly**, or state that
Path B is out of scope for this phase and name what gates public traffic in the
interim. FR-8 has no join key until this is answered.

### OQ-2: Where the mint happens

**Resolution: the bridge (op-grpc-bridge).** The bridge is the sole validator
(handoff spec); it is also the component that first sees a validated arrival
(after assertion verification or per-identity lookup). The mutation engine,
which lives inside the bridge process, mints genesis on the first mutation of
the session. The oracle decoy authenticates (Ed25519 assertion) but does not
mint; minting requires the chain head, which lives in the bridge's
`EventChain`.

### OQ-3: State term (chain head only, or additionally a blob root)

**Resolution: start with chain head hash + head timestamp + catalog hash.**
The chain head proves "these mutations happened" via prev_hash linkage and is
free (O(1)). The catalog hash proves "the contract looked like this". Adding a
full blob root over the catalog contents is deferred — it costs a hash per
login and the stronger claim is not yet needed. The genesis formula already
includes `catalog_hash` which is the published leaf-fold; extending to a full
root is additive later.

### OQ-4: Header naming and migration

**Resolution: add `x-ghostbridge-genesis` and accept both during transition.**
The new header carries the genesis. The old `x-ghostbridge-footprint` is
accepted during migration (its value is compared against the stored genesis, not
recomputed). After all edge components emit `x-ghostbridge-genesis`, the old
header is dropped. The grpc_web allow-list (`grpc_web.rs:31-33`) and the UI
are updated in the same phase as edge emission.

### OQ-5: schema_version

**Resolution: version 3.** The new record shape (genesis stored, arrival_timestamp
stored, field ownership contract) is `SLED_SCHEMA_VERSION = 3`. Records at
version ≤ 2 are distinguishable by the `schema_version` field in both the
`#[repr(C)]` sled and the Cozo record. No migration of old records — they
keep their values; new sessions get version 3.

### OQ-6: Fail-closed rollout

**Resolution: sequence stamping before gating.** Phase 1: the edge (UDS
injector, assertion path) stamps the genesis header. The gate still accepts
both old footprint AND new genesis. Phase 2: after confirming all active
sessions are stamping, the gate rejects requests without a valid genesis. The
zeros sentinel is removed in Phase 2, not Phase 1.

### OQ-7: The Python edge (sni-demux / socket-relay replacement)

**Resolution: out of scope for this spec.** Replacing `sni-demux.py` and
`socket-relay` with a Rust terminator is a separate spec. The current Python
byte relays are transparent forwarders; they neither read nor inject identity.
The identity blob and UDS-per-session binding (OQ-1) work regardless of
whether the outermost relay is Python or Rust — the stamp happens at the
bridge after termination, not at the edge.

---

## 6 · Deletions

| Item | Reason |
|---|---|
| `anna_scribe::notarize_arrival` (and the `File::open("/dev/shm/plugin_schema.dat")` it contains) | Duplicate reader of a source that is already wrong; genesis replaces it |
| `etch_footprint`'s `mutation_index` and `source_port` terms (identity-anchor usage) | Identity anchor no longer chases a moving counter; genesis uses chain head instead |
| `SENTINEL_FOOTPRINT` (64 zeros) and the warning-instead-of-reject path in `tracing.rs` | Fail-closed means absent = rejected, not degraded |
| `CanonicalPeerIdentity::from_sled()` reading the global 152-byte SHM file | Replaced by per-session record lookup |
| The two uncommitted compensating patches in `schema_bridge.rs` and `interceptor.rs` | The underlying problem (moving footprint) is eliminated by genesis |
| `op_identity::verify_ghostbridge_footprint` as an acceptance gate | Per-session Cozo lookup replaces it for gate decisions |
| `write_sled_from_wg` at 5 call sites (op-cognitive-mcp, op-mcp ×2, identity_sled_dispatch, op-identity-sled CLI) | Global-sled writers that race and cannot agree; replaced by per-session Cozo writes |
| `write_sled_full` at mutation_engine.rs:514 | Replaced by per-session Cozo write with genesis |
| `watch_wireguard_handshakes` (handshake monitor thread) | **Corrected rationale:** it is not a polling loop — it is event-driven (`ip monitor`), so "violates HC-4 by polling" is not a valid objection and would invite resurrection. It is deleted because it is one of the seven competing writers (FR-6) and because identity comes from the Ed25519 assertion, not from observing WG handshake events on the host — which HC-9 says should not be happening here at all |

---

## 7 · Non-Goals

- Do not redesign the MCP gateway surface (tonic-web on :8090 is settled).
- Do not add per-service TCP ports or new proto service packages.
- Do not introduce a WireGuard interface on the main host.
- Do not build OpenFlow/OVS packet tagging as the identity carrier.
- Do not replace the Python edge relays (separate spec).
- Do not redesign the oracle assertion / HumanPrincipal crypto (settled in handoff spec).
- Do not retroactively re-derive or migrate existing chain records to the new format.

---

## 8 · Relation to Locked Specs

### With `netmaker-xray-identity-handoff/` (extends)

Genesis EXTENDS the assertion path — it does not replace it. The assertion
proves *who* (cryptographic binding of pubkey to peer); genesis proves *when
and what state* (this peer, at this chain head, at this moment). Together they
close both the identity gap (solved by the assertion) and the temporal
instability gap (solved by genesis).

The `HumanPrincipalIdentity` extension inserted by the assertion validator
gains a `genesis: [u8; 32]` field that is populated after genesis mint/lookup.

### With `3tched-ghostbridge-control-plane/` (respects)

The topology lock is unchanged: no host WG, no SNI front, gRPC mesh-private.
The UDS-per-session binding (OQ-1) is consistent with the no-NIC container
model and the shared-socket architecture.

---

## 9 · Definition of Done — Single Author Rule

For each fact the design introduces, the spec must:

1. **Name the single author** — the one source file/function/constant that is
   the authoritative definition of that fact.
2. **List every consumer** that derives the fact — the interceptor, the chain
   stamper, the UDS injector, the UI, the gRPC reflection surface.
3. **Confirm no consumer holds its own copy** — no transcription of the
   formula, no hand-typed field names, no duplicated version constant.

| Fact | Single author | Consumers (derive, never restate) |
|------|---------------|-----------------------------------|
| Genesis formula | `op_identity::session_genesis::mint_genesis` | interceptor (compares output), chain stamper (embeds output), UDS injector (reads stored output) |
| Session record shape | `ContainerIdentitySled` in `identity_sled.rs` (PluginSchema) | `#[repr(C)]` layout (generated), Cozo relation (derived), gRPC reflection (derived from schema) |
| Chain position (mutation_index) | `MutationEngine::advance_identity_sled` | `event_to_footprint` (reads from engine state), per-session record (written by engine) |
| Catalog binding | `schema_catalog_hash()` in `schema_bridge.rs` | `mint_genesis` (reads it as input), no other site computes or caches it |
| Schema version | **Derived from the record definition's content hash (FR-10), not a hand-bumped constant.** A hand-maintained `SLED_SCHEMA_VERSION` with two consumers mirroring it is exactly the pattern FR-9 deletes. If a human-readable ordinal is still wanted, it is generated alongside the hash from the same definition, and no consumer declares it independently | Cozo record `schema_version` field (derived), `#[repr(C)]` layout (generated, embeds it), gate compatibility check (compares hashes) |

This table is mandatory in design.md and is a completion criterion.
