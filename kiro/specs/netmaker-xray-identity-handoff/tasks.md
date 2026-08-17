# Tasks — NetMaker / Xray Identity Handoff (corrected)

Convention: every task is independently verifiable. Tests are written BEFORE
implementation (red → green). All cargo commands run from the workspace root
with `CXXFLAGS="-include cstdint"`. Bridge lib tests run single-threaded
(`-- --test-threads=1`) due to a pre-existing env/SHM race (requirements §6).
Fail-closed: every negative test must assert rejection, never a fallback.

## Phase 1 — Assertion core + HumanPrincipal registry

### Task 1.1 — `oracle_assertion` module (op-identity)

**Requirements**: FR-1, FR-2, NFR-5

- [ ] Add `ed25519-dalek = "2.2"` (and workspace-consistent `rand`) to
  `crates/op-identity/Cargo.toml`.
- [ ] Tests first: round-trip encode/decode identity; tamper any byte ⇒
  `BadSignature`; trailing garbage ⇒ `Malformed`; issue with `ttl > 900 s` ⇒
  `LifetimeTooLong`; `ttl == 0` ⇒ error; deterministic encoding (same input ⇒
  same bytes).
- [ ] Implement `OracleIdentityAssertion`, `SignedAssertion` (`OIA1`
  envelope), canonical `signing_bytes`, `to_wire`/`from_wire`, `DecoyIssuer`,
  `verify_signature`, `AssertionError` (thiserror).
- [ ] Evidence: `cargo test -p op-identity --lib oracle_assertion` green.

### Task 1.2 — `derive_principal_id` (op-identity)

**Requirements**: FR-3, constraint 8

- [ ] Test first: deterministic, UUID-shaped, differs from
  `derive_session_id(pubkey)` for the same pubkey (context separation).
- [ ] Implement `op_identity::session::derive_principal_id` with context
  `"op-identity human-principal v1"`.
- [ ] Evidence: `cargo test -p op-identity --lib session` green.

### Task 1.3 — `human_principal` plugin (op-plugins + op-cozo-store)

**Requirements**: FR-3, NFR-4

- [ ] Tests first: schema declares all six methods with correct
  capabilities/subids; `all_plugin_subids_are_valid_and_unique` passes with
  the new subids registered in `oscal_subid_registry.rs`.
- [ ] `HumanPrincipal` / `HumanPrincipalState` (schemars + serde),
  `HumanPrincipalPlugin`, `human_principal_schema()` via
  `method_decl_from_schemars_with_output`, `inventory::submit!`.
- [ ] `HumanPrincipalRecord` + put/get/list/revoke helpers in
  `op-cozo-store` (own relations; `CozoGraphShuttle::new_persistent`).
- [ ] Cozo tests: register → resolve round-trip across reopen; duplicate
  pubkey rejected; duplicate active alias rejected; revoke idempotent;
  unknown revoke errors.
- [ ] Evidence: `cargo test -p op-plugins --lib` and
  `cargo test -p op-cozo-store --lib` green.

### Task 1.4 — Dispatch wiring (op-grpc-bridge)

**Requirements**: FR-3

- [ ] `human_principal_dispatch.rs`: `dispatch_human_principal_method` with
  all six methods; Cozo path `OP_HUMAN_PRINCIPAL_COZO_DB_PATH` or
  `/var/lib/op-dbus/human-principal-cozo`; all Cozo I/O in `spawn_blocking`.
- [ ] Wire BOTH touch-points: `else if` branch in `MutationEngine::mutate`
  and `"human_principal"` arm in `dispatch_method_call`.
- [ ] Test: register/resolve/revoke through the real `PluginService` surface
  (bridge lib test, single-threaded).
- [ ] Evidence: `cargo test -p op-grpc-bridge --lib -- --test-threads=1`
  green.

## Phase 2 — Bridge integration + decoy simulator + E2E

### Task 2.1 — Assertion validator (op-grpc-bridge)

**Requirements**: FR-4, NFR-3

- [ ] Tests first, one per rejection: `Malformed`, `UnknownDecoyKey`,
  `BadSignature`, `NotYetValid`, `Expired`, `LifetimeTooLong`, `Replay`,
  `MissingConnectInfo`, `SourceIpMismatch`, `UnknownPrincipal`,
  `RevokedPrincipal`; plus an ordering test (signature before expiry before
  replay before IP before resolve) and a lazy-purge test (expired nonces
  evicted on access, no background task).
- [ ] Implement `DecoyTrustStore::load` (fail-closed empty),
  `AssertionReplayCache` (lazy purge), `AssertionValidator::validate` with
  the contractual pipeline order, `HumanPrincipalIdentity` (footprint =
  blake3 derive `"op-identity human-footprint v1"`).
- [ ] Evidence: `cargo test -p op-grpc-bridge --lib oracle_assertion --
  --test-threads=1` green.

### Task 2.2 — Interceptor + ConnectInfo + capability gate wiring

**Requirements**: FR-4, FR-5

- [ ] Tests first: assertion present + valid ⇒ `HumanPrincipalIdentity` in
  extensions; assertion present + invalid ⇒ `unauthenticated`, footprint
  headers NOT consulted; assertion absent ⇒ ghostbridge path unchanged;
  human footprint keys the grants file (grant ⇒ allow, no grant ⇒ deny).
- [ ] `ghostbridge_interceptor`: read `x-oracle-identity-assertion-bin`,
  validator in `OnceLock`, registry resolve via `block_in_place`.
- [ ] Add `ConnectInfo<SocketAddr>` capture to the tonic serve builders.
- [ ] `enforce_bridge_capability` call sites: human identity first, else
  ghostbridge; mechanism otherwise untouched.
- [ ] Evidence: bridge lib tests green single-threaded; existing
  `schema_router` / interceptor tests unmodified and green.

### Task 2.3 — E2E decoy simulator battery

**Requirements**: FR-6

- [ ] `crates/op-grpc-bridge/tests/oracle_assertion_e2e.rs` on the
  `tonic_tls_reflection.rs` fixture pattern; per-test temp dirs for
  `OP_DECOY_TRUST_STORE`, `OP_GRANTS_PATH`, `OP_SLED_PATH`,
  `OP_HUMAN_PRINCIPAL_COZO_DB_PATH`.
- [ ] Happy path: register key via real `PluginService` ⇒ issue assertion
  (inner IP `127.0.0.1`) ⇒ call capability-gated method over real TLS ⇒
  success.
- [ ] Negative battery, one test each: unknown key; revoked key; expired;
  replayed nonce; source-IP substitution; alias substitution (alias in place
  of pubkey ⇒ rejection; no resolve-by-alias exists); container substitution
  (provisioned container's key ⇒ `UnknownPrincipal`); over-long TTL; unknown
  decoy key; bad signature; valid assertion without capability grant ⇒
  `PermissionDenied`.
- [ ] Evidence: `cargo test -p op-grpc-bridge --test oracle_assertion_e2e`
  green.

## Phase 3 — Boundary docs + negative topology gates

### Task 3.1 — Negative topology gates

**Requirements**: FR-7

- [ ] `crates/op-grpc-bridge/tests/negative_topology_gates.rs`: scan
  `crates/` for forbidden tokens (`wg-lan`, `op-identity-shuttle`,
  `TransportBindingIndex`, `NXM_NX_REG` identity tagging, new `.proto`,
  `Command::new` in new identity modules); assert `op-xray-daemon/src` has
  no identity/session/assertion refs; self-test with a fixture containing a
  forbidden token.
- [ ] `scripts/check-identity-topology.sh` wrapper.
- [ ] Evidence: gates green on the corrected tree; self-test proves they
  trip.

### Task 3.2 — External boundary documentation

**Requirements**: mission doc "Document external Oracle/NetMaker/Xray
integration boundaries", constraint 7

- [ ] `boundaries.md` finalized: non-negotiable boundaries; rejected
  mechanisms; external assumptions (decoy WG termination, NetMaker inner-IP
  preservation / no-NAT, trust-store provisioning, xray passthrough).
- [ ] Evidence: doc reviewed; gates reference it.

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

- [ ] All phase evidence commands green.
- [ ] Full required-test battery present and passing (unknown/revoked keys,
  expired/replayed assertions, alias/IP/container substitution, session
  freshness, bridge authorization).
- [ ] `cargo clippy -p op-identity -p op-plugins -p op-cozo-store
  -p op-grpc-bridge --all-targets -- -D warnings` and `cargo fmt --all --
  --check` clean.
- [ ] Negative topology gates pass, including self-test.
- [ ] No deploy, no sudo, no `/etc` edits, no service restarts, no live-host
  mutation at any point.
