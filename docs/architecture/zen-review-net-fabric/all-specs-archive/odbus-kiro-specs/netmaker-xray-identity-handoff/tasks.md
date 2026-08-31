# Tasks — NetMaker / Xray Identity Handoff (corrected)

**Revision notes:** Fixed Task 2.2 — `AssertionValidator` is per-server-instance
(`Arc` captured by interceptor closure), NOT process-global `OnceLock` (banned
by design §4.3). Adjacent ops surface:
`.kiro/specs/3tched-ghostbridge-control-plane/`. Spec review 2026-08-07:
FR-4 pipeline aligned with structural lifetime before trust; residual risks
documented; task checkboxes marked landed against branch commits through
`14151507`.

Convention: every task is independently verifiable. Tests are written BEFORE
implementation (red → green). All cargo commands run from the workspace root
with `CXXFLAGS="-include cstdint"`. Bridge lib tests run single-threaded
(`-- --test-threads=1`) due to a pre-existing env/SHM race (requirements §6).
Fail-closed: every negative test must assert rejection, never a fallback.

## Phase 1 — Assertion core + HumanPrincipal registry

### Task 1.1 — `oracle_assertion` module (op-identity)

**Requirements**: FR-1, FR-2, NFR-5

- [x] Add `ed25519-dalek = "2.2"` (and workspace-consistent `rand`) to
  `crates/op-identity/Cargo.toml`.
- [x] Tests first: round-trip encode/decode identity; tamper any byte ⇒
  `BadSignature`; trailing garbage ⇒ `Malformed`; issue with `ttl > 900 s` ⇒
  `LifetimeTooLong`; `ttl == 0` ⇒ error; deterministic encoding (same input ⇒
  same bytes).
- [x] Implement `OracleIdentityAssertion`, `SignedAssertion` (`OIA1`
  envelope), canonical `signing_bytes`, `to_wire`/`from_wire`, `DecoyIssuer`,
  `verify_signature`, `AssertionError` (thiserror).
- [x] Evidence: `cargo test -p op-identity --lib oracle_assertion` green.
  Landed: `e31cfa8c`.

### Task 1.2 — `derive_principal_id` (op-identity)

**Requirements**: FR-3, constraint 8

- [x] Test first: deterministic, UUID-shaped, differs from
  `derive_session_id(pubkey)` for the same pubkey (context separation).
- [x] Implement `op_identity::session::derive_principal_id` with context
  `"op-identity human-principal v1"`.
- [x] Evidence: `cargo test -p op-identity --lib session` green.
  Landed: `e31cfa8c`.

### Task 1.3 — `human_principal` plugin (op-plugins + op-cozo-store)

**Requirements**: FR-3, NFR-4

- [x] Tests first: schema declares all six methods with correct
  capabilities/subids; `all_plugin_subids_are_valid_and_unique` passes with
  the new subids registered in `oscal_subid_registry.rs`.
- [x] `HumanPrincipal` / `HumanPrincipalState` (schemars + serde),
  `HumanPrincipalPlugin`, `human_principal_schema()` via
  `method_decl_from_schemars_with_output`, `inventory::submit!`.
- [x] `HumanPrincipalRecord` + put/get/list/revoke helpers in
  `op-cozo-store` (own relations; `CozoGraphShuttle::new_persistent`).
- [x] Cozo tests: register → resolve round-trip across reopen; duplicate
  pubkey rejected; duplicate active alias rejected; revoke idempotent;
  unknown revoke errors.
- [x] Evidence: `cargo test -p op-plugins --lib` and
  `cargo test -p op-cozo-store --lib` green. Landed: `7ad8a374`.

### Task 1.4 — Dispatch wiring (op-grpc-bridge)

**Requirements**: FR-3

- [x] `human_principal_dispatch.rs`: `dispatch_human_principal_method` with
  all six methods; Cozo path `OP_HUMAN_PRINCIPAL_COZO_DB_PATH` or
  `/var/lib/op-dbus/human-principal-cozo`; all Cozo I/O in `spawn_blocking`.
- [x] Wire BOTH touch-points: `else if` branch in `MutationEngine::mutate`
  and `"human_principal"` arm in `dispatch_method_call`.
- [x] Test: register/resolve/revoke through the real `PluginService` surface
  (bridge lib test, single-threaded).
- [x] Evidence: `cargo test -p op-grpc-bridge --lib -- --test-threads=1`
  green. Landed: `4b8c8e3a`.

## Phase 2 — Bridge integration + decoy simulator + E2E

### Task 2.1 — Assertion validator (op-grpc-bridge)

**Requirements**: FR-4, NFR-3

- [x] Tests first, one per rejection: `Malformed` (including
  `expires_at <= issued_at` structural lifetime), `UnknownDecoyKey`,
  `BadSignature`, `NotYetValid`, `Expired`, `LifetimeTooLong`, `Replay`,
  `MissingConnectInfo`, `SourceIpMismatch`, `UnknownPrincipal`,
  `RevokedPrincipal`; plus an ordering test (structural lifetime before
  trust before signature before expiry before replay before IP before
  resolve) and a lazy-purge test (expired nonces evicted on access, no
  background task).
- [x] Implement `DecoyTrustStore::load` (fail-closed empty),
  `AssertionReplayCache` (lazy purge), `AssertionValidator::validate` with
  the contractual pipeline order, `HumanPrincipalIdentity` (footprint =
  blake3 derive `"op-identity human-footprint v1"`).
- [x] Evidence: `cargo test -p op-grpc-bridge --lib oracle_assertion --
  --test-threads=1` green. Landed: `842ace37`, `6af1f480`.

### Task 2.2 — Interceptor + ConnectInfo + capability gate wiring

**Requirements**: FR-4, FR-5

- [x] Tests first: assertion present + valid ⇒ `HumanPrincipalIdentity` in
  extensions; assertion present + invalid ⇒ `unauthenticated`, footprint
  headers NOT consulted; assertion absent ⇒ ghostbridge path unchanged;
  human footprint keys the grants file (grant ⇒ allow, no grant ⇒ deny).
- [x] `ghostbridge_interceptor`: read `x-oracle-identity-assertion-bin`;
  capture `Arc<AssertionValidator>` built for THAT server instance in the
  interceptor closure (NOT process-global `OnceLock` — design forbids it so
  tests can isolate trust stores/registries); registry resolve via the
  serving instance's `MutationEngine` under `block_in_place`.
- [x] Add `ConnectInfo<SocketAddr>` capture to the tonic serve builders.
- [x] `enforce_bridge_capability` call sites: human identity first, else
  ghostbridge; mechanism otherwise untouched.
- [x] Evidence: bridge lib tests green single-threaded; two isolated server
  builds can use distinct trust stores; existing `schema_router` /
  interceptor tests unmodified and green. Landed: `6af1f480`.

### Task 2.3 — E2E decoy simulator battery

**Requirements**: FR-6

- [x] `crates/op-grpc-bridge/tests/oracle_assertion_e2e.rs` on the
  `tonic_tls_reflection.rs` fixture pattern; per-test temp dirs for
  `OP_DECOY_TRUST_STORE`, `OP_GRANTS_PATH`, `OP_SLED_PATH`,
  `OP_HUMAN_PRINCIPAL_COZO_DB_PATH`.
- [x] Happy path: register key via real `PluginService` ⇒ issue assertion
  (inner IP `127.0.0.1`) ⇒ call capability-gated method over real TLS ⇒
  success.
- [x] Negative battery, one test each: unknown key; revoked key; expired;
  replayed nonce; source-IP substitution; alias substitution (alias in place
  of pubkey ⇒ rejection; no resolve-by-alias exists); container substitution
  (provisioned container's key ⇒ `UnknownPrincipal`); over-long TTL; unknown
  decoy key; bad signature; valid assertion without capability grant ⇒
  `PermissionDenied`.
- [x] Evidence: `cargo test -p op-grpc-bridge --test oracle_assertion_e2e`
  green. Landed: `14151507`.

## Phase 3 — Boundary docs + negative topology gates

### Task 3.1 — Negative topology gates

**Requirements**: FR-7

- [x] `crates/op-grpc-bridge/tests/negative_topology_gates.rs`: scan
  `crates/` for forbidden tokens (`wg-lan`, `op-identity-shuttle`,
  `TransportBindingIndex`, `NXM_NX_REG` identity tagging, new `.proto`,
  `Command::new` in new identity modules); assert `op-xray-daemon/src` has
  no identity/session/assertion refs; self-test with a fixture containing a
  forbidden token.
- [x] `scripts/check-identity-topology.sh` wrapper.
- [x] Evidence: gates green on the corrected tree; self-test proves they
  trip. Landed: `842ace37`.

### Task 3.2 — External boundary documentation

**Requirements**: mission doc "Document external Oracle/NetMaker/Xray
integration boundaries", constraint 7

- [x] `boundaries.md` finalized: non-negotiable boundaries; rejected
  mechanisms; external assumptions (decoy WG termination, out-of-band
  enrollment, NetMaker inner-IP preservation / no-NAT, trust-store
  provisioning, xray passthrough); residual risks cross-linked from design.
- [x] Evidence: doc reviewed; gates reference it.

## Summary

| Task | Crate(s) | File(s) | Change |
|---|---|---|---|
| 1.1 | op-identity | `src/oracle_assertion.rs`, `Cargo.toml` | new module + dep |
| 1.2 | op-identity | `src/session.rs` | new function |
| 1.3 | op-plugins, op-cozo-store | `state_plugins/human_principal.rs`, `plugin_scaffold_helpers.rs`, `oscal_subid_registry.rs`, `op-cozo-store/src/lib.rs` | new plugin + records |
| 1.4 | op-grpc-bridge | `human_principal_dispatch.rs`, `mutation_engine.rs` | dispatch wiring |
| 2.1 | op-grpc-bridge | `src/oracle_assertion.rs` | validator |
| 2.2 | op-grpc-bridge | `interceptor.rs`, `server.rs`, `grpc_server.rs` | wiring |
| 2.3 | op-grpc-bridge | `tests/oracle_assertion_e2e.rs` | E2E battery |
| 3.1 | op-grpc-bridge, scripts | `tests/negative_topology_gates.rs`, `scripts/check-identity-topology.sh` | gates |
| 3.2 | kiro spec | `boundaries.md` | docs |

## Definition of Done

- [x] All phase evidence commands green (landed on branch through `14151507`).
- [x] Full required-test battery present and passing: unknown/revoked keys,
  expired/replayed assertions, alias/IP/container substitution, capability
  denial (`PermissionDenied`). (No separate "session freshness" criterion —
  freshness is assertion expiry + replay rejection.)
- [ ] `cargo clippy -p op-identity -p op-plugins -p op-cozo-store
  -p op-grpc-bridge --all-targets -- -D warnings` and `cargo fmt --all --
  --check` clean — **verify on next session** (clippy chore commits
  `693bf843` / `269a45c3` exist; re-confirm before merge).
- [x] Negative topology gates pass, including self-test.
- [x] No deploy, no sudo, no `/etc` edits, no service restarts, no live-host
  mutation at any point.
