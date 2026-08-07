# Design — NetMaker / Xray Identity Handoff (corrected)

## 1 · Design Status

This design replaces `claude-redo/netmaker-xray-identity-handoff/` in full.
The rejected design's **diagnosis** (self-asserted identity headers, no
cryptographic binding between the WireGuard-authenticated peer and the
presented identity) was correct and is retained. Its **prescription** —
host-side `wg-lan` termination, a polling `op-identity-shuttle` handshake
watcher writing a source-IP-keyed `TransportBindingIndex`, per-registration
identity containers acting as trusted verifiers, and per-peer OpenFlow
tagging — is repudiated: it replaced self-asserted *headers* with
self-asserted *source IPs*, added a polling loop the architecture forbids,
multiplied trusted containers, and never achieved a cryptographic binding.

**Key architectural decisions** (confirmed with the user 2026-08-04):

1. **D-1 — No WireGuard on the VPS/main host.** WG terminates ONLY at the
   Oracle decoy. Exactly ONE NetMaker transport (multiple tunnels caused MTU
   issues).
2. **D-2 — Assertion carriage is INNER.** A short-lived Ed25519-signed
   `OracleIdentityAssertion` rides as gRPC metadata inside the existing TLS
   channel through passthrough xray. Xray never sees or touches it.
3. **D-3 — `op-grpc-bridge` is the SOLE validator.** Pipeline: parse →
   structural lifetime (`expires_at <= issued_at`) → trusted decoy key →
   signature → expiry → replay cache → source-IP binding → HumanPrincipal
   resolution → existing capability gate.
4. **D-4 — "IdentitySled BECOMES the provisioned container" is reused
   as-is** (`identity_sled.rs`, `identity_sled_dispatch.rs`). Containers stay
   containers; humans are a NEW, separate concept (D-5).
5. **D-5 — HumanPrincipal is a NEW PluginSchema-backed plugin** persisted in
   Cozo, with issue/resolve/revoke via the generated gRPC surface (canonical
   plugin pattern).
6. **D-6 — Display alias is display-only**, never authoritative; an
   alias-substitution test is mandatory.
7. **D-7 — Scope is BOTH sides locally**: decoy issuer + local decoy
   simulator for E2E tests. External Oracle/NetMaker/Xray boundaries are
   documented (`boundaries.md`), not deployed.

### Rejected alternatives (carried forward, do not re-litigate)

1. *`wg-lan` host WG interface + `op-identity-shuttle` handshake watcher +
   `TransportBindingIndex`* — polling-based, source-IP assertion is not a
   cryptographic binding, 180 s race window, forbidden by the
   reactive-not-polled constraint.
2. *Per-peer OpenFlow tagging (reg0 identity tags)* — "unforgeable binding"
   claim was overstated; the tag was only optionally checked and the actual
   gate remained the IP lookup. Datapath theater.
3. *Per-registration identity containers as verifiers* — turns a network
   segment into a trusted intermediary, reintroducing location-based trust;
   verification must happen at the single existing gate.
4. *Xray-side header injection into TLS* — impossible without xray becoming a
   TLS-terminating MITM; rejected (this was the v1 wrong turn, corrected in
   the rejected spec's own v2).
5. *NetMaker as identity authority* — NetMaker is transport only.

## 2 · Current Architecture (self-asserted)

```
human device ──WG──► Oracle decoy        (WG verified HERE, off-host)
                        │  (one NetMaker tunnel)
                        ▼
                   xray container        (passthrough: SNI sniff only)
                        │  TLS
                        ▼
                op-grpc-bridge           reads x-ghostbridge-footprint /
                                         x-wireguard-pubkey  ← SELF-ASSERTED
                        │
                        ▼
        PluginService → D-Bus → PluginSchema / MutationEngine
```

Security gap: nothing binds the presented headers to the WG-verified peer.

## 3 · Target Architecture

```
human device ──WG──► Oracle decoy ──signs──► OracleIdentityAssertion
  (sole WG termination)   Ed25519, short-lived (≤900 s)
                        │  assertion rides as gRPC metadata
                        │  x-oracle-identity-assertion-bin
                        │  inside the existing TLS channel
                        │  (one NetMaker transport, inner IP preserved)
                        ▼
                   xray container        (UNCHANGED passthrough)
                        │  TLS (terminated at bridge, not xray)
                        ▼
                op-grpc-bridge  ── SOLE VALIDATOR ──
                  interceptor: parse → structural lifetime → trust store →
                    signature → expiry → replay cache → ConnectInfo
                    source-IP binding → HumanPrincipal.resolve_key
                  extensions += HumanPrincipalIdentity{principal_id,
                    human_pubkey, footprint, expires_at}
                  enforce_bridge_capability (UNCHANGED mechanism,
                    human footprint keys the same grants file)
                        │
                        ▼
        PluginService → D-Bus → PluginSchema / MutationEngine
                        ▼
              human_principal plugin (Cozo)  ◄── register/resolve/revoke
```

Unchanged paths: ghostbridge footprint path for containers/host sled,
capability grants file format + wildcard fallback + fail-closed semantics,
MutationEngine → EventChain → blockchain audit, xray lifecycle surface,
identity_sled container provisioning.

## 4 · Component Design

### 4.1 `op-identity::oracle_assertion` (NEW module)

```rust
// crates/op-identity/src/oracle_assertion.rs

/// Exactly these fields, in this order, length-prefixed, in the canonical
/// signing/wire encoding. Envelope: b"OIA1" || assertion_bytes || signature.
pub struct OracleIdentityAssertion {
    pub human_pubkey: String,       // base64 WireGuard pubkey of the human
    pub issued_at: i64,             // unix seconds
    pub expires_at: i64,            // unix seconds
    pub nonce: [u8; 16],            // random per assertion; replay-cache key
    pub netmaker_inner_ip: IpAddr,  // human's NetMaker inner IP
    pub decoy_key_id: String,       // which decoy signing key
}

pub struct SignedAssertion { pub assertion: OracleIdentityAssertion,
                             pub signature: [u8; 64] }

impl OracleIdentityAssertion { pub fn signing_bytes(&self) -> Vec<u8>; }
impl SignedAssertion {
    pub fn to_wire(&self) -> Vec<u8>;                 // OIA1 envelope
    pub fn from_wire(bytes: &[u8]) -> Result<Self, AssertionError>;
}

pub struct DecoyIssuer { signing_key: SigningKey, key_id: String,
                         max_lifetime: Duration /* ≤ 900 s */ }
impl DecoyIssuer {
    pub fn issue(&self, human_pubkey: &str, inner_ip: IpAddr,
                 ttl: Duration) -> Result<SignedAssertion, AssertionError>;
    pub fn verifying_key(&self) -> &VerifyingKey;
    pub fn key_id(&self) -> &str;
}

pub fn verify_signature(a: &OracleIdentityAssertion, sig: &[u8; 64],
                        key: &VerifyingKey) -> Result<(), AssertionError>;

#[derive(thiserror::Error, Debug)]
pub enum AssertionError { Malformed, BadSignature, LifetimeTooLong, ... }
```

- `ed25519-dalek = "2.2"` added to `op-identity` (new workspace dependency;
  verified resolvable from crates.io 2026-08-04). Nonce generation uses the
  workspace-consistent `rand` version.
- `DecoyIssuer::issue` is total over strings: it does NOT validate that
  `human_pubkey` is a well-formed key (shape validation is the registry's
  job at `register_key`; the bridge rejects unknown keys at resolution).
  ttl <= 0 is rejected with AssertionError::NonPositiveLifetime.
- `from_wire` is framing-only: it does NOT verify the signature. Decode and
  verify are separate stages — the bridge pipeline's ordering depends on it.
- Canonical encoding: fixed field order, `u32` LE length prefixes for
  strings, 8-byte LE integers, 1-byte IP family + 4/16-byte address. No
  serde JSON anywhere in the signing path.
- Also in `op-identity/src/session.rs`:
  `pub fn derive_principal_id(pubkey: &str) -> String` —
  `blake3::derive_key("op-identity human-principal v1", pubkey)` → first 16
  bytes → UUID string. Distinct context from `derive_session_id`
  (`"op-identity session-id v1"`), so principal ids and container session ids
  can never collide.

### 4.2 `human_principal` plugin (NEW, canonical pattern)

Files, following the verified netmaker/identity_sled pattern:

| File | Change |
|---|---|
| `crates/op-plugins/src/state_plugins/human_principal.rs` | NEW: `HumanPrincipal`, `HumanPrincipalState`, `HumanPrincipalPlugin`, `human_principal_schema()`, typed inputs, `inventory::submit!` |
| `crates/op-plugins/src/state_plugins/plugin_scaffold_helpers.rs` | re-export `human_principal_schema` |
| `crates/op-plugins/src/state_plugins/oscal_subid_registry.rs` | register new subids |
| `crates/op-cozo-store/src/lib.rs` | `HumanPrincipalRecord` + `put/get/list/revoke` helpers (own relations) |
| `crates/op-grpc-bridge/src/human_principal_dispatch.rs` | NEW: `dispatch_human_principal_method(engine, method, args)`; Cozo at `OP_HUMAN_PRINCIPAL_COZO_DB_PATH` or `/var/lib/op-dbus/human-principal-cozo`; all Cozo I/O under `spawn_blocking` |
| `crates/op-grpc-bridge/src/mutation_engine.rs` | TWO touch-points: `else if plugin_id == "human_principal" && change_type == ChangeType::MethodCall` in `MutationEngine::mutate` AND `"human_principal"` arm in `dispatch_method_call` |

Methods (schema-declared, `method_decl_from_schemars_with_output`):

| Method | Effect | Capability | Subid |
|---|---|---|---|
| `register_key` | Mutation | `human_principal.write` | `mut.service.human-principal.key.register@v1` |
| `revoke_key` | Mutation | `human_principal.write` | `mut.service.human-principal.key.revoke@v1` |
| `set_alias` | Mutation | `human_principal.write` | `mut.service.human-principal.alias.set@v1` |
| `resolve_key` | Query | `human_principal.read` | `obs.service.human-principal.key.resolve@v1` |
| `get_principal` | Query | `human_principal.read` | `obs.service.human-principal.get@v1` |
| `list_principals` | Query | `human_principal.read` | `obs.service.human-principal.list@v1` |

Rules: `principal_id` derived, never supplied; `register_key` validates
pubkey shape (base64 decoding to 32 bytes) and rejects malformed/empty keys
with no state change — this gates the key namespace so alias/key
substitution is impossible by construction; duplicate pubkey rejected
(including REVOKED pubkeys: revocation is a permanent tombstone, a revoked
key can never be re-registered); duplicate non-empty alias among active
principals rejected (a revoked principal's alias is reusable; empty aliases
never collide); `revoke_key` idempotent on already-revoked (the original
`revoked_at` timestamp is preserved, never re-stamped), error on unknown;
`set_alias` on an unknown principal errors with no state change, setting the
own current alias is a successful no-op, clearing (empty alias) is allowed,
`set_alias` on a revoked principal is allowed (display-only data); no auth
path resolves by alias (there is no `resolve_alias` method at all).

### 4.3 Bridge validation (NEW module + interceptor/gate wiring)

`crates/op-grpc-bridge/src/oracle_assertion.rs`:

```rust
pub struct DecoyTrustStore { keys: HashMap<String, VerifyingKey> }
impl DecoyTrustStore {
    /// OP_DECOY_TRUST_STORE, default /etc/opdbus/decoy-trust.json.
    /// Missing/unreadable/invalid ⇒ EMPTY store (fail-closed).
    pub fn load() -> Self;
}

pub struct AssertionReplayCache { seen: Mutex<HashMap<[u8;16], i64>> }
impl AssertionReplayCache {
    /// Lazy purge of expired entries on each call; NO background task.
    pub fn check_and_insert(&self, nonce: [u8;16], expires_at: i64,
                            now: i64) -> bool; // false = replay
}

pub struct HumanPrincipalIdentity {
    pub principal_id: String,
    pub human_pubkey: String,
    pub footprint: [u8;32],   // blake3 derive "op-identity human-footprint v1"
    pub expires_at: i64,
}

#[derive(thiserror::Error, Debug)]
pub enum AssertionRejection {
    Malformed, UnknownDecoyKey, BadSignature, NotYetValid, Expired,
    LifetimeTooLong, Replay, MissingConnectInfo,
    SourceIpMismatch { expected: IpAddr, actual: IpAddr },
    UnknownPrincipal, RevokedPrincipal, RegistryUnavailable,
}

pub struct AssertionValidator { /* trust_store, replay_cache, engine, leeway, max_lifetime */ }
impl AssertionValidator {
    /// Pipeline order is contractual: parse → structural lifetime → trust →
    /// signature → expiry → replay → source-IP → resolve. Each step fail-closed.
    pub fn validate(&self, wire: &[u8], source: Option<SocketAddr>)
        -> Result<HumanPrincipalIdentity, AssertionRejection>;
}
```

**Interceptor** (`interceptor.rs`): `ghostbridge_interceptor` reads optional
`x-oracle-identity-assertion-bin`. Present ⇒ assertion path governs:
validate (source IP from `ConnectInfo<SocketAddr>` in request extensions);
success ⇒ insert `HumanPrincipalIdentity`; failure ⇒
`Status::unauthenticated(<rejection>)`. Absent ⇒ existing footprint path,
byte-for-byte unchanged. **Validator state is per-server-instance, NOT
process-global**: the interceptor is a closure capturing
`Arc<AssertionValidator>` built for THAT server build (the existing global
`ENGINE` `OnceLock` pattern is NOT reused for the assertion path — a
process-wide validator makes per-test trust stores/registries impossible and
is explicitly rejected). Registry resolution uses the serving instance's own
`MutationEngine` via `dispatch_human_principal_method(engine, "resolve_key",
…)` under `block_in_place`.

**Pinned validator semantics** (contractual; the validation contract asserts
each):

- `validate(wire, source, now)` takes the current time as a parameter
  (clock-injection seam for deterministic tests).
- The replay cache is keyed by the 16-byte nonce GLOBALLY (not per
  principal, not by wire-hash): any second presentation of a nonce within
  its TTL is `Replay`, regardless of principal or body. Entries purge at
  now >= expires_at + leeway, so the replay window equals the acceptance
  window.
- A nonce is consumed at the replay step even if a LATER step fails
  (fail-closed: a failed validation still burns the nonce).
- The trust store is loaded ONCE at validator construction. Rotation
  requires constructing a new validator; there is no per-request reload.
  ANY corruption in the trust-store file (bad JSON, wrong types, wrong key
  length, duplicate key id) ⇒ the WHOLE store is empty (fail-closed), never
  a partial load.
- Multiple `x-oracle-identity-assertion-bin` metadata values on one request
  ⇒ reject (`Malformed`); exactly one value is required when the key is
  present.
- `expires_at <= issued_at` ⇒ reject (`Malformed`) — the bridge never
  delegates lifetime sanity to the issuer. The check fires immediately after
  parse (structural sanity), before the trust step.
- Source-IP comparison is IP-only (port ignored); IPv4-mapped-IPv6 forms
  are NOT normalized — exact `IpAddr` equality.

**ConnectInfo**: the tonic serve path gains peer-address capture
(`into_make_service_with_connect_info::<SocketAddr>()` on the operation
routes in `server.rs` / `grpc_server.rs` serve builders). Assertion present
without ConnectInfo ⇒ `MissingConnectInfo` rejection (fail-closed).

**Capability gate** (`grpc_server.rs`): `enforce_bridge_capability` keeps its
signature shape; call sites extract the footprint from
`HumanPrincipalIdentity` first, else `GhostbridgeIdentity`. Grants lookup,
schema-blob `required_capability` check, wildcard fallback, and fail-closed
semantics are untouched. Human grants are entries in the SAME
`capability-grants.json`, keyed by the human footprint hex.

### 4.4 Local decoy simulator + E2E (`crates/op-grpc-bridge/tests/`)

`oracle_assertion_e2e.rs` reuses the `tonic_tls_reflection.rs` fixture:
rcgen self-signed identity, `TcpListener::bind("127.0.0.1:0")`, real
`MutationEngine` + `build_operation_routes`, `ClientTlsConfig` trusting the
CA, `install_crypto_provider()` Once. The simulator is a `DecoyIssuer`
instance plus a helper that sets `x-oracle-identity-assertion-bin` on a
tonic request. `OP_DECOY_TRUST_STORE`, `OP_GRANTS_PATH`, `OP_SLED_PATH`, and
`OP_HUMAN_PRINCIPAL_COZO_DB_PATH` all point at per-test temp dirs. Because
the E2E binds loopback, the passing case asserts
`netmaker_inner_ip = 127.0.0.1`; the IP-substitution case asserts any other
IP and expects `SourceIpMismatch`.

### 4.5 Negative topology gates

`crates/op-grpc-bridge/tests/negative_topology_gates.rs` walks the workspace
root (`env!("CARGO_MANIFEST_DIR")/../..`) and asserts absence of forbidden
tokens in `crates/`: `wg-lan`, `op-identity-shuttle`,
`TransportBindingIndex`, `NXM_NX_REG` identity tagging, new `.proto` files,
`Command::new` in the new identity modules; asserts `op-xray-daemon/src`
contains no `identity|session_id|assertion` references. Includes a self-test
(a fixture string containing a forbidden token must trip the scanner).
`scripts/check-identity-topology.sh` wraps `cargo test -p op-grpc-bridge
--test negative_topology_gates`.

## 5 · What Does NOT Change

| Item | Reason |
|---|---|
| Ghostbridge footprint path (headers, sled verify, `verify_per_identity`) | Containers/host sled keep working; assertion path is additive and opt-in per request |
| `enforce_bridge_capability` mechanism, grants file format, wildcard fallback, fail-closed | The gate is correct; only its principal input widens |
| MutationEngine → EventChain → blockchain audit | All new mutations flow through it unchanged |
| `identity_sled` plugin + `provision_container` | Containers are not humans; reused as-is |
| `op-xray-daemon` | Lifecycle only; never gains identity logic |
| Xray live config location and reload path | `/etc/xray/xray_config.json` in-container; models never write/reload |
| NetMaker plugin (`netclient` + REST) | Transport, not identity |
| No new proto / gRPC service packages | PluginSchema-derived surface only |

## 6 · Failure Modes

| Failure | Behavior |
|---|---|
| Trust store missing/unreadable | Empty store ⇒ all assertions rejected (fail-closed) |
| Assertion expired / not yet valid / TTL > 900 s | `unauthenticated` with reason |
| Nonce replayed within TTL | `unauthenticated(Replay)` |
| Source IP ≠ `netmaker_inner_ip`, or ConnectInfo missing | `unauthenticated` |
| Key unknown / revoked | `unauthenticated(UnknownPrincipal / RevokedPrincipal)` |
| Registry (Cozo) unavailable during resolve | `unauthenticated(RegistryUnavailable)` — never allow |
| Valid assertion, capability not granted | `PermissionDenied` at the existing gate |
| Bridge restart | In-process replay cache empties. Still-valid TTLs can be replayed until expiry — accepted residual risk for phase 1 (short TTL ≤ 900 s); durable/cross-restart replay is deferred. Expired assertions stay rejected. |

### 6.1 Residual risks (documented, not closed by this mission)

| Risk | Disposition |
|---|---|
| Ghostbridge footprint path remains when assertion metadata is absent | Intentional: containers/host sled keep working. Assertion path is additive/opt-in per request. Closing the self-asserted-header path for non-assertion clients is a separate mission. |
| Source-IP binding depends on NetMaker inner-IP preservation (no NAT) | External assumption (`boundaries.md` §3.2); if a deployment NATs, re-specify before enabling assertions there. |
| Replay cache is in-process only | Bridge restart opens a short replay window for live TTLs; phase-1 acceptance (see failure table). |

## 7 · Verification Model

- `cargo test -p op-identity --lib` — assertion codec, issuer, derivation.
- `cargo test -p op-plugins --lib` — schema surface, subid uniqueness.
- `CXXFLAGS="-include cstdint" cargo test -p op-cozo-store --lib` — record
  persistence.
- `cargo test -p op-grpc-bridge --lib -- --test-threads=1` — validator unit
  tests (single-threaded: pre-existing env/SHM races, see requirements §6).
- `cargo test -p op-grpc-bridge --test oracle_assertion_e2e` — full E2E
  battery over real TLS.
- `cargo test -p op-grpc-bridge --test negative_topology_gates` (or
  `scripts/check-identity-topology.sh`) — topology gates.
- `cargo clippy -p op-identity -p op-plugins -p op-cozo-store -p op-grpc-bridge
  --all-targets -- -D warnings` and `cargo fmt --all -- --check`.

## 8 · Implementation Order

1. `oracle_assertion` module + issuer + `derive_principal_id` (op-identity).
2. `human_principal` plugin + Cozo records + dispatch + MutationEngine arms.
3. Bridge validator + interceptor + ConnectInfo + capability-gate wiring.
4. E2E simulator battery.
5. Boundary docs + negative topology gates.

## 9 · Verified System Facts

### 9.1 Baseline before implementation (2026-08-04, HEAD bf7a9090)

| Claim | Status | Evidence |
|---|---|---|
| `ContainerIdentitySled` embeds `IncusInstance`; session_id == container name, derived from WG pubkey | VERIFIED | `op-plugins/src/state_plugins/identity_sled.rs` |
| `provision_container` dispatch exists | VERIFIED | `op-grpc-bridge/src/identity_sled_dispatch.rs` |
| Interceptor reads `x-ghostbridge-footprint`, `x-wireguard-pubkey`, `x-ghostbridge-trace-id`; per-identity path via Cozo identity_sled | VERIFIED | `op-grpc-bridge/src/interceptor.rs` |
| `enforce_bridge_capability` gates `call_method`/`mutate`; grants from `/dev/shm/opdbus/capability-grants.json` (`OP_GRANTS_PATH`), wildcard fallback, fail-closed | VERIFIED | `op-grpc-bridge/src/grpc_server.rs:148` |
| New-plugin pattern: `inventory::submit!` only (no `default_registry.rs` edit); methods need BOTH a `dispatch_method_call` arm and a `mutate` else-if branch | VERIFIED | netmaker / identity_sled walk-through |
| Cozo per-plugin persistence via `CozoGraphShuttle::new_persistent`, own DB path, `spawn_blocking` | VERIFIED | `op-cozo-store/src/lib.rs`, identity_sled template |
| TLS at bridge: `ZEROCLAW_TLS_CERT/KEY` or rcgen self-signed; ephemeral TLS test fixture exists | VERIFIED | `server.rs:111`, `tests/tonic_tls_reflection.rs` |
| No ed25519 dependency or signing code anywhere in the workspace | VERIFIED **at baseline** | grep over all `Cargo.toml` |
| `op-xray-daemon` is lifecycle-only | VERIFIED | grep: no identity/session/footprint refs |
| Baseline tests: op-identity 27/27, op-plugins 139/139, op-grpc-bridge 69/69 serial (1 parallel flake), op-cozo-store needs `CXXFLAGS="-include cstdint"` | VERIFIED | `/tmp/odbus-baseline-m02.log` |

### 9.2 Post-implementation (branch `droid/netmaker-xray-identity-handoff`)

| Claim | Status | Evidence |
|---|---|---|
| `op-identity::oracle_assertion` (OIA1) + `derive_principal_id` present | LANDED | `e31cfa8c` |
| `human_principal` plugin + Cozo `HumanPrincipalRecord` | LANDED | `7ad8a374` |
| `human_principal` dispatch through MutationEngine | LANDED | `4b8c8e3a` |
| Assertion validator + interceptor wiring + negative topology gates | LANDED | `842ace37`, `6af1f480` |
| Oracle assertion E2E battery over real TLS | LANDED | `14151507` |
| ed25519 signing via `ed25519-dalek` in `op-identity` | PRESENT (was absent at baseline) | `crates/op-identity/Cargo.toml`, `src/oracle_assertion.rs` |

## 10 · Backlog / Future Work (deferred, one-line reasons)

| Item | Reason deferred |
|---|---|
| Real Oracle decoy deployment + WG termination config | External host; documented in `boundaries.md` |
| Dynamic model-generated xray tag routing | Separate spec; models never write xray until then |
| Grants materialization reliability (staleness auto-recovery) | Real concern, separate spec (requirements §5.2) |
| Assertion renewal / refresh protocol | Short TTL + re-login is sufficient for phase 1 |
| Hardware-backed decoy signing keys | Operational hardening, post-E2E |
