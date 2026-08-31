# Requirements — NetMaker / Xray Identity Handoff (corrected)

> **Canonical consumer:** this spec is the authoritative Oracle-signed assertion
> (OIA1) identity model. The MCP/cognitive control plane at `op-grpc-bridge` TLS
> `:8090` (see `.kiro/specs/unified-authenticated-mcp-cognitive-control-plane/`)
> **consumes** this pipeline as its SEC-2 authentication step and does not redefine
> it. Any `:50052`/`:3003`/`mcp.internal` route referenced below is being retired in
> favor of the single `:8090` ingress; the identity mechanics are unchanged.

> One trust chain: a human authenticates with WireGuard at the Oracle decoy, the
> decoy issues a short-lived Ed25519-signed identity assertion, and that assertion
> rides as gRPC metadata inside the existing TLS channel through passthrough xray
> to `op-grpc-bridge`, which is the sole validator and the application
> authorization boundary. No WireGuard on the main host. One NetMaker transport.
> No watchers, no polling.

| | |
|---|---|
| Status | Corrected spec — supersedes `claude-redo/netmaker-xray-identity-handoff/` (rejected) |
| Mission | `factory-missions-20260804/02-netmaker-xray-identity.md` |
| WG termination | Oracle decoy ONLY — never the main host |
| Transport | Exactly ONE NetMaker tunnel |
| Assertion carriage | INNER: signed assertion as gRPC metadata inside existing TLS |
| Sole validator | `op-grpc-bridge` |
| Related crates | `op-identity`, `op-plugins`, `op-grpc-bridge`, `op-cozo-store` |

## 1 · Problem Statement and Gap Analysis

### 1.1 Claimed model

`CLAUDE.md` describes the intended zero-trust model: identity = WireGuard pubkey
→ Argon2(PSK, salt=pubkey) sessionid; the xray router injects identity headers
(`X-Ghostbridge-Footprint` / `X-WireGuard-Pubkey`); "that header is meant to be
the only gate".

### 1.2 Actual code state (verified 2026-08-04 at bf7a9090)

| Component | Claimed | Actual |
|---|---|---|
| `etch_footprint` (`op-identity/src/schema_bridge.rs:1075`) | identity anchor | LIVE, but binds pubkey+mutation_index+port, not a human session |
| `GhostbridgeInterceptor` (`op-grpc-bridge/src/interceptor.rs`) | identity gate | Reads `x-ghostbridge-footprint`, `x-wireguard-pubkey`, `x-ghostbridge-trace-id` — all CLIENT SELF-ASSERTED headers |
| `ClientConfig.wg_pubkey` | authenticated peer | CLIENT SELF-ASSERTED |
| `op-xray-daemon` | identity injection | LIFECYCLE ONLY (start/stop/restart/status/reload/get_config); no identity logic |
| `wireguard.rs` (`op-identity`) | peer verification | DISCONNECTED from the request path |
| Capability gate (`enforce_bridge_capability`, `grpc_server.rs:148`) | authorization | LIVE and correct — but its identity input is the self-asserted footprint |

### 1.3 Core security gap

1. WireGuard verifies the peer — but only at the Oracle decoy, off-host.
2. GAP: the client self-asserts identity headers over TLS; nothing
   cryptographically binds the presented identity to the authenticated peer.
3. Therefore any client that can reach the bridge can claim any footprint.

The fix must provide an actual cryptographic binding between the
WireGuard-authenticated peer and the presented identity. A source-IP lookup
table populated by a handshake watcher (the rejected `TransportBindingIndex`
design) is NOT such a binding — it replaces self-asserted headers with
self-asserted source IPs and adds a polling loop.

### 1.4 Infrastructure state

- The main host has NO incoming identity WireGuard interface and must never gain one.
- The Oracle decoy is the sole incoming WireGuard termination point (external;
  documented in `boundaries.md`, not deployed by this mission).
- NetMaker is transport, not the human identity authority. Multiple tunnels
  caused MTU issues — exactly one NetMaker transport exists.
- Xray is a passthrough: SNI/protocol sniffing only. It cannot inject HTTP
  headers into opaque TLS without becoming a TLS-terminating MITM, which is
  rejected.

## 2 · Hard Constraints

1. **No WireGuard on the main host.** WG terminates ONLY at the Oracle decoy.
   Never add `wg-lan` or any host WG interface.
2. **Exactly one NetMaker transport.** No additional tunnels.
3. **Fail-closed.** Missing/invalid/expired/replayed assertion, unknown or
   revoked key, IP/alias/container substitution ⇒ rejection. Never fall back
   to a weaker path when verification fails.
4. **Reactive, not polled.** Connection/login arrival triggers resolution.
   No handshake watchers, no polling loops, no D-Bus watchers for identity.
5. **Single validation point.** `op-grpc-bridge` is the SOLE assertion
   validator. No in-container verifiers, no second identity control plane.
6. **PluginSchema is the source of truth.** Identity operations are methods on
   a PluginSchema-backed plugin surfaced through the generated gRPC surface
   (`PluginService` → D-Bus → `MutationEngine`). No hand-written per-plugin
   proto, no direct backend RPC.
7. **Xray remains passthrough.** No xray identity logic, no header injection,
   no TLS termination at xray. Xray live config exists only at
   `/etc/xray/xray_config.json` inside the xray container; models never write
   or reload xray.
8. **Concept separation.** Human identity, WireGuard key, login session,
   workspace container, and display alias are separate concepts. A workspace
   container is not the human. System containers are never users. The display
   alias is display-only and never authoritative.

## 3 · Functional Requirements

### FR-1: OracleIdentityAssertion type and canonical wire format

`op-identity` gains an `oracle_assertion` module defining:

- `OracleIdentityAssertion { human_pubkey, issued_at, expires_at, nonce,
  netmaker_inner_ip, decoy_key_id }` — exactly these fields.
- A deterministic canonical signing/wire encoding (fixed field order,
  length-prefixed, versioned envelope `OIA1`). serde JSON is NOT used for
  signing bytes.
- `SignedAssertion { assertion, signature }` with Ed25519 signatures
  (`ed25519-dalek` 2.2.x, new workspace dependency).

**Acceptance criteria**: round-trip encode/decode is identity; tampering with
any byte fails signature verification; decoding rejects trailing/garbage bytes.

### FR-2: Decoy issuance API (local issuer + simulator)

- `DecoyIssuer { signing_key, key_id, max_lifetime }` in `op-identity` issues
  signed assertions: `issue(human_pubkey, netmaker_inner_ip, ttl) ->
  Result<SignedAssertion>`.
- Issuance rejects `ttl > max_lifetime` (hard cap 900 s) and `ttl <= 0`.
- The local decoy simulator is a test harness (not a service): it drives the
  issuance API and presents assertions to a real TLS bridge over ephemeral
  localhost ports.

**Acceptance criteria**: `cargo test -p op-identity --lib` covers issue/sign/
verify round-trip, over-long TTL rejection, and deterministic encoding.

### FR-3: HumanPrincipal registry plugin

A NEW PluginSchema-backed plugin `human_principal`, following the canonical
plugin pattern (schemars state struct, `human_principal_schema()`,
`inventory::submit!`, dispatch module in `op-grpc-bridge`, MutationEngine
arms, Cozo persistence via `CozoGraphShuttle` on its own DB path):

- `HumanPrincipal { principal_id, human_pubkey, display_alias, registered_at,
  revoked_at }`. `principal_id` is DERIVED via a new
  `op_identity::session::derive_principal_id(pubkey)` (blake3 derive_key with
  context `"op-identity human-principal v1"` → UUID) — never supplied. This
  derivation context is distinct from `derive_session_id`, so a principal id
  can never collide with a container session id.
- Methods: `register_key` (Mutation, cap `human_principal.write`),
  `resolve_key` (Query, cap `human_principal.read`), `revoke_key` (Mutation,
  cap `human_principal.write`), `get_principal` (Query), `list_principals`
  (Query), `set_alias` (Mutation, cap `human_principal.write`).
- `register_key` rejects duplicate pubkeys and duplicate non-empty aliases
  among active principals.
- `revoke_key` sets `revoked_at`; revoking an already-revoked key is a
  successful no-op; unknown key is an error.
- `display_alias` is display-only. No auth path ever resolves by alias.

**Acceptance criteria**: schema declares the full method surface; register →
resolve round-trips through the real `PluginService` surface; state survives
`CozoGraphShuttle` reopen; OSCAL subids registered and unique
(`all_plugin_subids_are_valid_and_unique` passes).

### FR-4: Bridge assertion validation (sole validator)

`op-grpc-bridge` gains an `oracle_assertion` validation module:

- New optional gRPC metadata key `x-oracle-identity-assertion-bin` carries the
  versioned wire encoding of a `SignedAssertion`.
- Validation pipeline, in this exact order, each step fail-closed:
  1. **Parse** — malformed envelope ⇒ reject.
  2. **Trusted decoy key** — `decoy_key_id` must resolve in the trust store
     (JSON: `{ "decoy_keys": { "<key_id>": "<base64 verifying key>" } }`,
     path from `OP_DECOY_TRUST_STORE`, default `/etc/opdbus/decoy-trust.json`;
     missing/unreadable store = empty = reject everything).
  3. **Signature** — Ed25519 verify over canonical bytes ⇒ reject on failure.
  4. **Expiry** — `now > expires_at + leeway` ⇒ reject; `issued_at` in the
     future beyond leeway ⇒ reject; `expires_at - issued_at > 900 s` ⇒ reject.
     Leeway 30 s.
  5. **Replay cache** — nonce seen before (within TTL = `expires_at + leeway`)
     ⇒ reject. In-process cache, lazy purge on access; NO background task.
  6. **Source-IP binding** — peer address from tonic `ConnectInfo<SocketAddr>`
     must equal `netmaker_inner_ip` ⇒ reject on mismatch; missing ConnectInfo
     when an assertion is presented ⇒ reject.
  7. **HumanPrincipal resolution** — `resolve_key(human_pubkey)` via the
     registry plugin; unknown ⇒ reject; `revoked_at` set ⇒ reject.
- On success the request extensions gain `HumanPrincipalIdentity {
  principal_id, human_pubkey, footprint, expires_at }` where `footprint =
  blake3 derive_key("op-identity human-footprint v1", pubkey)`.
- Precedence: if the assertion metadata is present, the assertion path
  governs (footprint headers are neither required nor consulted). If absent,
  the existing ghostbridge footprint path is unchanged.

**Acceptance criteria**: unit tests for each pipeline step's accept and reject
branches; ordering test proves signature is checked before expiry before
replay before IP binding before resolution.

### FR-5: Capability gate integration (existing gate, unchanged mechanism)

- `enforce_bridge_capability` remains the authorization gate and remains
  fail-closed with the `/dev/shm/opdbus/capability-grants.json` (`OP_GRANTS_PATH`
  override) wildcard-fallback semantics.
- The gate gains a minimal principal extraction: if `HumanPrincipalIdentity`
  is present in request extensions, its `footprint` hex keys the grants
  lookup; otherwise the existing `GhostbridgeIdentity` path applies.
- Human grants therefore live in the SAME grants file, keyed by the human
  footprint — no second grants mechanism.

**Acceptance criteria**: a valid assertion whose human footprint lacks the
method's `required_capability` is denied; granting the capability allows the
call; the existing ghostbridge grant tests still pass.

### FR-6: E2E via local decoy simulator over real TLS

Integration tests in `crates/op-grpc-bridge/tests/` (reusing the
`tonic_tls_reflection.rs` fixture pattern: rcgen self-signed identity,
`127.0.0.1:0` ephemeral bind, real `MutationEngine`, `build_operation_routes`,
trusting TLS client channel):

1. Register a human key through the real `PluginService` surface.
2. Issue an assertion via the simulator; call a capability-gated method over
   the real TLS channel with the assertion as metadata ⇒ success.
3. Negative battery (each its own test): unknown key, revoked key, expired
   assertion, replayed nonce, source-IP substitution, alias substitution,
   container substitution (a provisioned container's key never resolves as a
   human), over-long TTL, unknown decoy key, bad signature, missing
   capability grant.

**Acceptance criteria**: `cargo test -p op-grpc-bridge --test
oracle_assertion_e2e` passes with the full battery.

### FR-7: Negative topology gates

A cargo-test gate (`crates/op-grpc-bridge/tests/negative_topology_gates.rs`,
locating the workspace root via `CARGO_MANIFEST_DIR`) plus a thin shell
wrapper `scripts/check-identity-topology.sh` asserting:

- No `wg-lan`, `op-identity-shuttle`, or `TransportBindingIndex` anywhere in
  `crates/`.
- No per-peer OpenFlow identity tagging (no `NXM_NX_REG` identity tags, no
  identity-driven flow installation).
- `op-xray-daemon` contains no identity/session/assertion logic.
- No new `.proto` files and no new gRPC service packages.
- No `Command::new` subprocess invocation in the new identity code paths.

**Acceptance criteria**: gates pass on the corrected tree; deliberately
introducing a forbidden token makes them fail (self-test).

## 4 · Non-Functional Requirements

- **NFR-1** Rust-first: no new Python; scripts are shell.
- **NFR-2** No deploy, no sudo, no `/etc` edits, no service restarts, no
  live-host mutation. Cargo tests only.
- **NFR-3** No polling loops, watchers, or background tasks in the identity
  path; the replay cache purges lazily on access.
- **NFR-4** OSCAL subid taxonomy for every new method/event; registered in
  `oscal_subid_registry.rs`; uniqueness CI-enforced.
- **NFR-5** `anyhow::Result` for app errors, `thiserror` for the rejection
  enum; `simd_json` preferred over `serde_json`; rustfmt 4-space/100-col;
  `cargo clippy -- -D warnings` clean for touched crates.
- **NFR-6** All new behavior covered by tests written BEFORE implementation
  (red → green).

## 5 · Out of Scope

### 5.1 Traffic and deployment boundaries (documented, not implemented)

| Path | Direction | Status |
|---|---|---|
| Oracle decoy WG termination + real assertion issuance | service | EXTERNAL — documented in `boundaries.md` |
| NetMaker mesh (`OP_NETMK_*` ACLs), inner-IP preservation (no NAT) | transport | EXTERNAL — documented assumption |
| Xray container passthrough config | service | EXTERNAL — unchanged, lifecycle only |
| Customer privacy tunnels | consumer | Untouched — pure passthrough |
| mail / qdrant / netmaker per-port incus proxies | service | Untouched |
| `assistant` shared-socket model | service | Untouched |

### 5.2 Other exclusions

- Deploying or configuring the real Oracle decoy, NetMaker, or xray.
- Rejected mechanisms: `wg-lan`, `op-identity-shuttle`,
  `TransportBindingIndex`, per-peer OpenFlow tagging, per-registration
  identity containers, xray header injection.
- Grants materialization reliability rework (staleness detection /
  rematerialization) — a real pre-existing concern, but a separate spec.
- Any UI surface. Validation is terminal-only.

## 6 · Adjacent Issues (NOT in scope, documented for awareness)

| Issue | Evidence | Disposition |
|---|---|---|
| `schema_router::tests::required_capability_check_allows_granted` flakes under parallel test runs (env var / `/dev/shm` races); passes with `--test-threads=1` | baseline 2026-08-04: 68+1 failed parallel, 69/69 serial | Pre-existing test-isolation defect; workers run bridge lib tests single-threaded; do not "fix" by weakening assertions |
| `op-cozo-store` (cozorocks/RocksDB) fails to compile without `CXXFLAGS="-include cstdint"` | baseline 2026-08-04 cc-rs error | Documented in `AGENTS.md`; all cargo commands in this mission set the flag |
| Grants staleness after outage (durable vs `/etc` vs SHM drift) | rejected spec design.md §7.1 | Separate spec (see 5.2) |
