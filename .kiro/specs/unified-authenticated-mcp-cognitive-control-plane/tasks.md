# Tasks: unified-authenticated-mcp-cognitive-control-plane

Ordered phases 0–14. Every task lists: **Reqs** (requirement IDs), **Files**
(crates/files), **Tests first** (red), **Steps**, **Verify** (exact commands),
**DoD**, **Deps**, and **Rollback** where applicable.

Conventions:
- All builds set `CXXFLAGS="-include cstdint"` (cozorocks needs it).
- Host services managed **only** with `sudo sv` (never `systemctl`/s6).
- JSON assertions use `jq`. No Python.
- gRPC checks against `:8090` use a TLS client (`grpcurl` with the CA), never
  `-plaintext`.
- "the bridge" = `op-grpc-bridge`.
- **Rollback** is listed per task **where applicable**: pre-Phase-10 tasks are
  git-revertible (that is the default rollback and is not repeated on every task);
  tasks with special rollback (data/schema/service/irreversible) state it explicitly.
  Phase 10+ rolls binaries/configuration back via the prior btrfs golden snapshot
  (DEP-2) and cognitive data via the verified T12.0 backup/compatibility procedure;
  auth replay history is never rewound and retired unauthenticated relays stay down.

The irreversible step is Phase 10 (deletions); it is gated on Phases 1–9 verifying
and the client inventory (CR-1) being migrated. Rollback of Phase 10+ combines the
prior btrfs golden snapshot with T12.0's verified data procedure and never restores a
deprecated unauthenticated ingress.

---

## Phase 0 — Baseline inventory and red tests

### T0.1 — Capture the live baseline
**Reqs:** CR-1, DEP-4, FR-1, Verified Baseline
**Steps:**
- Set `SPEC=.kiro/specs/unified-authenticated-mcp-cognitive-control-plane` and write
  every committed artifact below that directory. Create a root-only transient backup
  directory outside the repository with `sudo install -d -m 0700
  /var/lib/op-dbus/rollback` followed by `sudo mktemp -d
  /var/lib/op-dbus/rollback/unified-mcp.XXXXXX`; record only its path and SHA-256
  inventory in the spec (never its contents).
- Record listeners (privileged, so the owning PID of every `:8090` bind is visible):
  `sudo ss -lntp | tee "$SPEC/baseline-listeners.txt"`
- Record every process fronting `:8090` (bridge vs `python3`/`socket-relay`):
  `sudo ss -lntp 'sport = :8090' | tee "$SPEC/baseline-8090.txt"`
- Record both forwarders: `sudo sv status fwd-8090 fwd-nm-mesh-8090 | tee
  "$SPEC/baseline-forwarders.txt"`; `rg -n 'socket-relay|tcp-listen'
  deploy/runit/{fwd-8090,fwd-nm-mesh-8090}/run | tee
  "$SPEC/baseline-forwarder-runs.txt"`.
- Record sealed-blob set: `ls -1 /dev/shm/opdbus/plugin-blobs | tee
  "$SPEC/baseline-blobs.txt"` and record the count alongside it.
- Never copy live grants, assertions, private keys, tokens, database contents, or
  unredacted service environments into the repository. Commit only a deterministic
  redacted grant inventory (principal hash, capability/subid names, source-file hash;
  no bearer material or identity attributes). Copy any exact rollback material to the
  root-only transient backup with mode `0600`, then hash it there.
- Record service state: `for s in op-grpc-bridge op-web op-cognitive-mcp op-mcp-agents op-mcp-compact fwd-8090 fwd-nm-mesh-8090; do sudo sv status "$s"; done`.
- Record deployed binary hashes: `sha256sum /usr/local/bin/op-grpc-bridge /usr/local/bin/op-web-server`
**Verify:** the redacted baseline files exist and are non-empty; a secret scanner and
`rg -n '(BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY|Bearer [A-Za-z0-9._~-]{16,}|OIA1[.:][A-Za-z0-9_-]{16,})'
"$SPEC"/baseline-*.txt` find no live secret material; `baseline-8090.txt` records
which PID owns each `:8090` address (documents the `python3`/`fwd-8090` relay to be
retired by FR-1).
**DoD:** only redacted structural artifacts are committed to the spec dir, including
pre-change `:8090` ownership and forwarder topology. Exact rollback material is
root-only, outside git, integrity-hashed, and deleted after acceptance. If the safe
snapshot cannot be created and verified, deployment is blocked fail closed.
**Deps:** none.

### T0.2 — Capture the working tool inventory (compact/full)
**Reqs:** CR-1, FR-3
**Steps:** with an authenticated client, capture `tools/list` (all pages) and
generated `operation.method.*` reflection into `baseline-tools.json` /
`baseline-reflection.txt`. If no authenticated client path exists yet, capture via
the current UDS shim (`op-mcp-server --mode cognitive`) and note the method.
**Verify:** `jq '.tools|length' baseline-tools.json` ≥ 1; reflection list non-empty.
**DoD:** the pre-consolidation tool/method name set is frozen as the migration oracle.
**Deps:** T0.1.

### T0.3 — Write failing acceptance tests (red)
**Reqs:** FR-1, FR-6, FR-7, FR-3b, FR-3c, FR-3d, FR-3e, FR-3f, FR-4a, FR-5a, FR-8,
FR-3g, FR-8b, FR-9, FR-9a, FR-2a, SEC-2, SEC-3, SEC-9, SEC-13, SEC-14,
DR-1, DR-1a, DR-2, DR-7, DR-8, DR-9, DEP-5, TR-4, TR-5, NFR-7
**Files:** `crates/op-grpc-bridge/tests/unified_mcp_e2e.rs` (new),
`crates/op-grpc-bridge/tests/mcp_modern_2026_07_28.rs` (new),
`crates/op-grpc-bridge/tests/mcp_legacy_stateful.rs` (new),
`crates/op-mcp/tests/tool_authz.rs` (new),
`crates/op-blob/tests/blob_resolution.rs` (new),
`crates/op-cognitive-mcp/tests/memory_schema.rs` (new),
`crates/op-chat/tests/two_turn_memory_e2e.rs` (new),
`crates/op-web/tests/no_grpc_proxy.rs` (new),
`scripts/acceptance/btrfs_rollback_e2e.sh` (new),
`scripts/ci/zero-trace-gates.sh` (new).
**Tests first (all initially red) — named tests required by the deliverable in bold:**
- **`only_8090_is_mcp_listener`** — asserts no `:3003/:50051/:50052/:11438` and
  `:8090` present, AND (FR-1) the only PID bound to every `:8090` address is the
  bridge — no `python3`/`socket-relay`/`fwd-8090`.
- `code_tools_are_real` — `execute_tool{code_search}` returns code-search shape.
- `in_process_dispatch_no_loopback` — tool call succeeds with no `:3003` process.
- `per_tool_capability_enforced` — broad-invoke-but-not-shell identity is denied
  `agent_shell_executor_exec`.
- **`session_id_not_auth`** (FR-2a/SEC-2) — a request bearing a valid MCP
  `Mcp-Session-Id` but no fresh OIA1 assertion is REJECTED; a replayed session id
  does not grant access.
- **`execution_context_unforgeable`** (FR-3d) — a caller cannot inject or override
  any `ExecutionContext` field (actor/scope/capability/deadline/approval/binding)
  via `arguments`; a meta-tool creates a target-specific attenuated child context
  that cannot widen scope/capability/deadline/transport binding or reuse an approval
  not bound to that exact target and diff.
- **`output_schema_validated`** (FR-3e) — a tool returning output that violates its
  `output_schema` yields an error; the malformed output is never returned,
  persisted, vectorized, or prompt-injected, while a redacted intent/outcome failure
  pair is still appended to the audit chain.
- **`forged_scope_denied`** (FR-3f) — a caller supplying another identity's
  `container_id`/`identity_id`/`namespace`/`workspace`/`collection`/`session_id` is
  scoped to its OWN authorized scope or denied (covers read/write/delete/
  context-stream/semantic-query/coding); a coding `../`/absolute/symlink-escape path
  and an oversized archive are rejected.
- **`sealed_wildcard_grant_absent`** (FR-4a/SEC-3) — no `"*"` grant for
  MCP/cognitive/agent/tool capabilities in `capability-grants.json` OR inside any
  sealed `PluginSchema.capability_grants`; the caller-supplied `x-opdbus-capability`
  header alone (no resolved grant) is denied; a method with no resolvable grant is
  denied (degraded allow-path removed).
- `nested_args_validated` — `execute_tool` with bad nested args rejected pre-exec.
- `register_rejects_duplicate` — duplicate tool name → `Err`.
- `blob_catalog_full_covers_manifest`.
- **`exact_hash_blob_resolution`** (DR-8) — with two blobs for one plugin id present,
  the reader returns the manifest-pinned `<schema_hash16>`, not an arbitrary
  first-prefix entry.
- `durable_memory_unavailable_not_silent` — locked Cozo → `Unavailable`, no in-mem write.
- **`durable_replay_across_restart`** (SEC-13) — a nonce accepted before a bridge
  restart is rejected after restart, and a nonce used on TCP is rejected when
  replayed on UDS (and vice-versa); pre-auth rejections are recorded as pre-auth
  telemetry, not actor-attributed event-chain records.
- **`uds_not_world_writable`** (TR-5) — the served UDS socket mode is not `0o666`
  (`stat` shows the restricted owner/group mode); a peer whose namespaced uid/gid
  cannot be mapped is denied.
- **`no_op_web_grpc_proxy`** (TR-4) — sending `application/grpc` to `:8080` does not
  reach `:8090`; `/jsonrpc`, `/rpc`, `/.well-known/mcp.json` and MCP discovery
  aliases return 404/410.
- `unauth_rejected_before_dispatch` for TCP/host-UDS/container-UDS.
- `plugin_schema_dat_absent` — asserts `plugin_schema.dat` is absent from runtime
  (`/dev/shm/opdbus`), code, deploy, and docs (DR-2).
- `authenticated_caller_sees_full_schema_catalog` — reflection returns the complete
  sanitized public catalog to an authenticated caller; raw sealed fields stay admin-
  gated and execution stays capability-gated (FR-8b/FR-9a).
- `public_tool_schema_is_sanitized` — MCP tool/resource schema projections expose
  public input/output/side-effect metadata but no grants, principal membership,
  internal paths, key identifiers, secrets, or approval-verifier configuration.
- `audit_records_all_failure_classes` — denied/invalid/cancelled/timed-out/failed each
  produce an intent/outcome pair with the outcome and redacted args (SEC-9); no
  outcome-bearing attempt can disappear because output validation failed.
- `operation_retry_is_durable_and_exactly_once` — idempotent retry/reconnect returns
  the committed response without repeating a side effect; changed-binding reuse and
  every repeated non-idempotent operation ID are denied across restart.
- `cancellation_classifies_commit_races` — pre-admission, prepared, committed, and
  non-transactional partial-effect cancellation fixtures produce the correct durable
  outcome/reconciliation state without claiming a committed effect was rolled back.
- `mcp_notifications_and_origin` — notifications handled; missing/invalid `Origin` on
  the HTTP AND gRPC-Web transports rejected (FR-2a).
- `modern_stateless_has_no_session_authority` and
  `legacy_session_requires_fresh_assertion` cover the modern and legacy protocols as
  separate fixtures, not conditionals inside one lifecycle test.
- `context_cursor_survives_process_restart`, `approval_requires_authorized_approver`,
  `active_call_completes_across_hot_seal`, and `blob_vector_catalog_hash_matches`
  establish the durable-context, approval-authority, route-generation, and vector
  catalog invariants before implementation.
**Verify:** `CXXFLAGS="-include cstdint" cargo test -p op-grpc-bridge -p op-mcp -p op-blob -p op-web --no-run` compiles; the tests run and FAIL (red) against current code.
**DoD:** red tests committed (all named tests above present); `scripts/ci/zero-trace-gates.sh` present and currently failing on live tree.
**Deps:** T0.2.

---

## Phase 1 — Canonical route and authentication stack

### T1.1 — One authenticated catalog projected through shared Axum/Tonic ingress
**Reqs:** FR-17
**Files:** `crates/op-grpc-bridge/src/server.rs`,
`crates/op-grpc-bridge/src/grpc_server.rs` (`build_operation_routes`),
`crates/op-grpc-bridge/src/mcp_frontend.rs` (new),
`crates/op-grpc-bridge/tests/route_surface.rs` (new).
**Tests first:**
- `shared_builder_mounts_all_surfaces` — generated plugin RPCs, MCP adapter, context
  streaming, Waypipe, registration, health, and authenticated reflection occur once
  in one `AuthenticatedDispatchCatalog`; no independent catalog/builder owns a
  surface.
- `one_acceptor_routes_axum_and_tonic` — one TLS acceptor per canonical address
  routes MCP HTTP/JSON-RPC and gRPC-Web through Axum and native gRPC through Tonic by
  protocol/content type without a loopback, relay, or second `:8090` listener.
**Steps:** make `server.rs` own the only `:8090` TLS accept loop. Build one immutable
`AuthenticatedDispatchCatalog`; project it with `build_tonic_routes(catalog)` for
native gRPC/reflection/gRPC-Web and `build_mcp_routes(catalog)` for native Axum MCP,
then combine them with `build_ingress(catalog)`. `build_operation_routes` may remain
the Tonic projection but MUST NOT claim to mount raw HTTP. Both projections share the
same auth/context/limit/audit middleware and dispatch handles; add later surfaces to
the catalog without another listener or authority.
**Verify:** `cargo test -p op-grpc-bridge --test route_surface -- --nocapture`.
**DoD:** one TLS acceptor and one authoritative catalog own the full surface; its
Axum and Tonic projections share authentication, limits, cancellation, and audit.
**Deps:** T0.3.

### T1.2 — Authoritative capability/subid registry; remove wildcard + degraded allow-path
**Reqs:** FR-4a, SEC-2, SEC-3, FR-3b, NFR-5, NFR-7
**Files:** `deploy/config/opdbus-grants*` / grants materialization; `interceptor.rs`
`load_capability_grants`; `crates/op-grpc-bridge/src/grpc_server.rs`
(`enforce_bridge_capability`, `enforce_bridge_capability_with_schema`);
`crates/op-state-store/src/plugin_schema.rs` (`capability_grants` resolution,
`grants.get(footprint).or_else(|| grants.get("*"))`);
`crates/op-plugins/src/state_plugins/tched_router.rs` (removes the `"*"` insert);
`crates/op-plugins/src/state_plugins/oscal_subid_registry.rs`;
`crates/op-grpc-bridge/tests/authz_pipeline.rs` (new),
`crates/op-plugins/tests/plugin_subid_validation.rs` (new),
`crates/op-state-store/tests/capability_grants.rs` (new).
**Tests first:**
- `sealed_embedded_grants_absent` — every active sealed
  `PluginSchema.capability_grants` is empty/non-authoritative; identity grants exist
  only in the sanitized-versioned `identity_sled` projection.
- `sealed_wildcard_grant_absent` (T0.3) → green: `"*"` absent from all active grant
  sources and schemas.
- `caller_header_not_authority` — a request whose only claim is the
  `x-opdbus-capability` header (no resolved grant) is denied.
- `no_grant_denies_not_allows` — a method/tool with no resolvable grant is DENIED
  (the `identity.is_some() && capability_matches` degraded allow-path is removed).
- `unknown_capability_registration_rejected` — registering a tool/method/plugin whose
  capability or subid is unknown to the registry is rejected.
- `auth_ordering` — signature → expiry → replay → binding → capability →
  input-validation → dispatch.
**Steps:** make tool/method `required_capability`+`subid` resolve against the
authoritative registry (reject unknown); make `identity_sled` the only identity-grant
authority; migrate then remove every embedded `PluginSchema.capability_grants` value,
including the `"*"` insert in `tched_router.rs`, and delete
`or_else(|| grants.get("*"))`; remove the degraded allow-path so an undeclared grant
denies; treat `x-opdbus-capability` as intent-only, never authority. Record only the
sanitized identity-grant version/hash in checkpoints and schema projections.
**Verify:** `cargo test -p op-grpc-bridge --test authz_pipeline -- --nocapture`;
`cargo test -p op-plugins --test plugin_subid_validation -- --nocapture`;
`cargo test -p op-state-store --test capability_grants -- --nocapture`; then inspect the materialized grants:
`jq '.["*"].capabilities' /dev/shm/opdbus/capability-grants.json` contains no
MCP/cognitive/agent execution capability; a catalog test asserts every sealed
`capability_grants` is empty; `rg -n '"\*"'
crates/op-plugins/src/state_plugins/tched_router.rs` → none.
**DoD:** identity_sled is the only grant authority; sealed schemas carry no embedded
identity grants or wildcard; degraded allow-path is gone; header is non-authoritative.
**Deps:** T1.1.
**Rollback:** T0.1's exact grant snapshot is forensic input only and MUST NOT be
blindly restored if it contains wildcard/sentinel/schema-assigned authority. Restore
only a verified identity_sled projection that preserves the tightened invariants and
re-seal/verify its sanitized version/hash; if the prior binary cannot operate without
removed authority, keep protected traffic fail closed rather than reintroducing it.

### T1.3 — UDS security model: peer-cred + userns mapping + tightened mode + TLS only
**Reqs:** TR-2, TR-5, SEC-3, SEC-4
**Files:** `interceptor.rs`, `crates/op-grpc-bridge/src/server.rs` (UDS accept path;
`serve_with_incoming`), `crates/op-grpc-bridge/src/shared_socket.rs`
(`set_permissions(0o666)` → restricted mode);
`crates/op-grpc-bridge/tests/uds_binding.rs` (new).
**Tests first:**
- `uds_requires_assertion_and_peercred` — a UDS peer with no valid assertion is
  rejected; peer uid/gid/pid + socket ownership are captured and bound.
- `uds_not_world_writable` (T0.3) → green: `stat` shows the socket is not `0o666`.
- `uds_userns_uid_maps_to_principal` — a container peer's namespaced uid/gid maps to
  the intended principal; an unmappable peer is denied.
- `uds_wire_protocols_enumerated` — host UDS permits native gRPC + MCP HTTP but rejects
  gRPC-Web; container UDS permits native gRPC only and rejects MCP HTTP/gRPC-Web.
- `uds_plaintext_rejected` — plaintext HTTP/2, gRPC, gRPC-Web, and MCP HTTP fail
  before application dispatch; the same configured CA validates TLS on UDS and TCP.
**Steps:** obtain `SO_PEERCRED` for UDS connections; map namespaced uid/gid/pid
through user namespaces to the resolved principal; feed into the same authorization
decision; adapt OIA source binding to peer-credential/session binding for UDS
(TR-2); tighten the socket to the minimal owner/group mode (not world-writable);
require TLS-over-UDS with certificate verification, explicit SNI/authority
`op-grpc-bridge.internal`, and a matching DNS SAN; reject plaintext/wrong-name/
untrusted-root—there is no policy-exception branch. Set host and container socket
ownership/mode to the specified mapped owner/group and `0660` or stricter; enforce
the per-UDS protocol allowlists above. TCP path remains source-bound.
**Verify:** `cargo test -p op-grpc-bridge --test uds_binding -- --nocapture`;
`stat -c '%a' /run/opdbus/grpc.sock` shows a restricted mode (not `666`).
**DoD:** UDS and TCP produce identical authorization outcomes with transport-specific
binding (SEC-4); socket not world-writable; userns mapping + protocol enumeration +
TLS-only transport enforced.
**Deps:** T1.2.

### T1.4 — ExecutionContext: immutable, bridge-created, threaded to execution
**Reqs:** FR-3d, SEC-2
**Files:** `crates/op-mcp/src/tool_registry.rs` (`trait Tool`, `Tool::execute`
signature — trait change adding an `ExecutionContext` param), `op_core`
`ToolDefinition`, every `impl Tool` in `op-cognitive-mcp`/`op-mcp`;
`crates/op-grpc-bridge/src/grpc_server.rs` / `mutation_engine.rs` (construct the ctx;
`CallMethod` path also carries it); `crates/op-grpc-bridge/tests/exec_context.rs` (new).
**Tests first:**
- `execution_context_unforgeable` (T0.3) → green: caller cannot inject/override any
  ctx field via `arguments`.
- `meta_tool_attenuates_context` — a meta-tool (`execute_tool`) derives a child
  context for the selected target: actor, resolved scope, trace, transport binding,
  cancellation, and the earlier deadline are preserved; selected capability is
  narrowed to the target; broad outer capability is not reusable; approval survives
  only when it is bound to the exact target/suggestion/diff/base revision.
**Steps:** define an immutable `ExecutionContext { actor, resolved_identity,
container/workspace/session scope, granted_capability, selected_capability, trace_id,
event_correlation_id, deadline, cancel_token, verified_approval, transport_binding }`
set only by the bridge; change `Tool::execute(&self, ctx: &ExecutionContext, input:
Value)`; thread the context through `PluginService.CallMethod`; expose one
`ExecutionContext::attenuate_for_target(descriptor, approval_binding)` constructor
that can only narrow scope/capability/deadline and drops an inapplicable approval.
Require both the outer meta-tool and target capabilities; set parent invocation,
increment bounded delegation depth, and reject recursion/depth overflow or any
requested widening before the T4.3 audit intent/dispatch.
**Verify:** `CXXFLAGS="-include cstdint" cargo build -p op-mcp -p op-cognitive-mcp -p op-grpc-bridge`; `cargo test -p op-grpc-bridge --test exec_context -- --nocapture`.
**DoD:** execution receives an unforgeable, bridge-built context; target-specific
attenuation is the only meta-dispatch path and cannot widen authority.
**Deps:** T1.2.

### T1.5 — Durable, cross-transport replay cache + pre-auth telemetry + rate limits
**Reqs:** SEC-13, SEC-2
**Files:** `crates/op-grpc-bridge/src/oracle_assertion.rs`
(`AssertionReplayCache`: replace the in-process `Mutex<HashMap>`),
`crates/op-grpc-bridge/src/auth_replay_store.rs` (new), `interceptor.rs` (pre-auth
telemetry sink separate from the event chain; pre-auth + per-principal rate limiters),
`deploy/runit/op-grpc-bridge/run` and environment configuration;
`crates/op-grpc-bridge/tests/replay_durability.rs` (new).
**Tests first:**
- `durable_replay_across_restart` (T0.3) → green: nonce rejected after restart and
  across TCP↔UDS; the test stops/unmounts cognitive-memory Cozo and replay rejection
  still works.
- `replay_domains_are_separate` — OIA and approval nonces use domain-separated keys,
  and insert-if-absent is atomic under concurrent requests.
- `replay_store_unavailable_fails_closed` — no in-memory fallback accepts a request.
- `preauth_telemetry_not_actor_attributed` — an unauthenticated rejection produces a
  pre-auth telemetry record with no actor/event-chain attribution.
- `preauth_and_per_principal_rate_limited` — a burst of unauthenticated requests is
  rate-limited; a single principal exceeding its rate is throttled.
**Steps:** create a dedicated bridge-owned Cozo database at
`/var/lib/op-dbus/auth-replay.db` through the bridge's existing
`op-cozo-store` dependency. It is a separate file, handle, schema, migration, and
health signal from cognitive-memory Cozo, and has no memory subsystem dependency.
Set file mode `0600` and its parent `0700`. Key records by
`(envelope_kind,trusted_issuer_key_id,nonce)` with subject hash, issued/expiry time,
and first execution ID, domain-separated for OIA1/OIG1/OPA1. Perform a non-mutating
lookup first; consume OIA1/OIG1 only after signature/time/binding/principal or genesis
proof succeeds, and consume OPA1 only at the mutation admission boundary after every
approval/target/input check. Use atomic insert-if-absent under concurrency; prune only
after expiry + maximum skew + audit retention. Never fall back to an in-process cache. Share this store across
TCP and every UDS transport and survive `sudo sv restart`; route pre-auth rejections to a separate
telemetry sink (never forge an actor); add pre-auth and per-principal rate limiters
at the ingress (legitimate timers/limits, NFR-2).
**Verify:** `cargo test -p op-grpc-bridge --test replay_durability -- --nocapture`;
then live: accept a nonce, `sudo sv restart op-grpc-bridge`, replay the same nonce →
rejected; stop cognitive-memory storage and repeat → still rejected.
**DoD:** replay protection durable + cross-transport; pre-auth telemetry separated;
rate limits enforced.
**Deps:** T1.2.

### T1.6 — Bridge directly binds the canonical :8090 set; retire both relays
**Reqs:** FR-1, SEC-1, TR-1, NFR-6, NFR-7
**Files:** `crates/op-grpc-bridge/src/server.rs` (parse a validated canonical bind
set; `load_tls_identity` SANs), TLS cert SAN material,
`deploy/runit/op-grpc-bridge/run` and its checked-in env template,
`deploy/runit/{fwd-8090,fwd-nm-mesh-8090}` (retire), checked-in nftables policy,
`crates/op-grpc-bridge/tests/direct_bind.rs` (new).
**Tests first:** `bridge_binds_canonical_set` — the bridge binds loopback plus the
canonical mesh-facing address(es) with TLS; `no_relay_fronts_8090` — no
`python3`/`socket-relay`/`fwd-8090` process fronts `:8090`.
**Steps:** make one runit variable authoritative for
`127.0.0.1:8090,10.0.0.3:8090,${NETMAKER_MESH_IP}:8090` (deployment default
`NETMAKER_MESH_IP=100.69.0.1`) and remove conflicting loopback-only overrides. At
startup verify `10.0.0.3` belongs to svc0 and the configured Netmaker IP belongs to
the configured Netmaker interface; readiness fails if an applicable address cannot
bind. Require SANs for `127.0.0.1`, `localhost`, `10.0.0.3`, the configured Netmaker
IP, and `op-grpc-bridge.internal`. Install a default-deny rule admitting svc0 and
Netmaker `:8090` only on their named interfaces/from enumerated CIDRs. Stage both
relay retirements here; T12.1 performs the conflict-safe live order.
**Verify:** `CXXFLAGS="-include cstdint" cargo build -p op-grpc-bridge`;
`cargo test -p op-grpc-bridge --test direct_bind -- --nocapture`; live
(post-deploy in T12.x) `sudo ss -lntp` shows only the bridge PID on every `:8090`
address and `sudo sv status fwd-8090 fwd-nm-mesh-8090` → both down/absent.
**DoD:** runit—not a code-only default—declares the exact applicable three-address
set; interface ownership, TLS SANs, and firewall rules are startup-validated; both
relay retirements are staged.
**Deps:** T1.3.
**Rollback:** revert the bind-set/SAN change before deployment. After cutover use the
T12.1 fail-closed rollback: keep `fwd-8090` down and the firewall closed while the
prior authenticated golden is restored from the console.

---

## Phase 2 — In-process cognitive registry; remove HTTP loopback

### T2.1 — Construct CognitiveMcpServer in the bridge; hold Arc<ToolRegistry>
**Reqs:** FR-6, NFR-1, NFR-3, DR-1a
**Files:** `crates/op-grpc-bridge/src/grpc_server.rs`, `mutation_engine.rs`,
`crates/op-grpc-bridge/tests/registry_degradation.rs` (new).
**Tests first:** `registry_constructed_and_degrades` — bridge starts with Qdrant down
(registry present, semantic tools degrade); with durable Cozo locked, memory tools
return `Unavailable` (DR-1a), bridge still serves other plugins.
**Steps:** construct `CognitiveMcpServer::new(db_path)` adjacent to the existing
`QdrantSemanticShuttle`; store `Arc<ToolRegistry>` and the context engine in
`MutationEngine`; ensure durable-Cozo-unavailable → `Unavailable` (no silent in-mem
write path used for durable ops).
**Verify:** `cargo build -p op-grpc-bridge`;
`cargo test -p op-grpc-bridge --test registry_degradation -- --nocapture`.
**DoD:** registry owned in-process; degradation correct.
**Deps:** T1.4.
**Rollback:** revert construction; Phase-1 stack still works.

### T2.2 — Replace HTTP loopback; keep PluginSchema→D-Bus as the only dispatch authority
**Reqs:** FR-6, NFR-1, DR-3
**Files:** `mutation_engine.rs` (`dispatch_cognitive_mcp_method`,
`cognitive_mcp_endpoint`, `cognitive_mcp_http`),
`crates/op-plugins/src/state_plugins/cognitive_mcp.rs` (authoritative
`PluginSchema`/`MethodDecl`), `op-grpc-bridge/Cargo.toml`,
`crates/op-grpc-bridge/tests/cognitive_dispatch.rs` (new).
**Tests first:** `in_process_dispatch_no_loopback` (from T0.3) flips to green;
`cognitive_methods_dispatch_via_dbus` asserts every generated cognitive method and
MCP adapter call enters `PluginService.CallMethod`, crosses D-Bus, and reaches the
`MutationEngine` exactly once; `no_parallel_registry_dispatch_authority` rejects a
route that invokes a production tool directly from an externally reachable adapter.
**Steps:** derive cognitive method names and shapes from
`cognitive_mcp_plugin_schema()`; route every externally reachable generated/MCP call
through `PluginService.CallMethod → D-Bus → MutationEngine`. Inside the mutation
engine only, translate the authoritative method into
`tool_registry.execute(ctx,name,args)` / `tool_registry.list(ctx)`; do not expose the
registry as a parallel transport/control plane. Delete the `reqwest` loopback and
`cognitive_mcp_endpoint`; drop `reqwest` from `Cargo.toml` if unused. Do not
hand-author per-tool protobuf services or duplicate PluginSchema declarations.
**Verify:**
- `rg -n '10.200.0.2:3003|cognitive_mcp_endpoint|reqwest' crates/op-grpc-bridge/src` → no match.
- `cargo test -p op-grpc-bridge --test cognitive_dispatch -- --nocapture`.
**DoD:** no loopback and no parallel execution authority; all externally reachable
cognitive/tool dispatch derives from PluginSchema and crosses the canonical D-Bus
mutation path once.
**Deps:** T2.1.
**Rollback:** disable affected cognitive methods behind the authenticated bridge and
return `Unavailable` while reverting/fixing the in-process adapter; never restore the
HTTP loopback or a standalone `:3003` listener.

---

## Phase 3 — Correct code-tool routing

### T3.1 — Route code_* to the real code tools
**Reqs:** FR-7
**Files:** `mutation_engine.rs` `map_schema_method_to_tool`.
**Tests first:** `code_tools_are_real` (T0.3) → green.
**Steps:** map `code_search→code_search`, `code_context→code_context`,
`code_index→code_index`; leave `search_blob_vectors`/`refresh_blob_vectors` distinct.
**Verify:** `rg -n 'code_search|code_context|code_index' crates/op-grpc-bridge/src` shows no `*_blob_vectors` mapping; `cargo test -p op-grpc-bridge --test unified_mcp_e2e code_tools_are_real -- --nocapture`.
**DoD:** code tools reachable and correct.
**Deps:** T2.2.

---

## Phase 4 — Unified MCP protocol adapter on :8090

### T4.1 — Extend the Tool descriptor and enforce per-tool authorization
**Reqs:** FR-3b, FR-3c, FR-4a, FR-8b, FR-9a, NFR-5
**Files:** `crates/op-mcp/src/tool_registry.rs` (`Tool` trait,
`ToolRegistry::register`/`execute`), `op_core::ToolDefinition`, every `impl Tool` in
`op-cognitive-mcp`/`op-mcp`,
`crates/op-plugins/src/state_plugins/cognitive_mcp.rs`, `oscal_subid_registry.rs`,
`crates/op-plugins/tests/plugin_subid_validation.rs` (new).
**Tests first:** `per_tool_capability_enforced`, `register_rejects_duplicate`,
`every_tool_has_descriptor` (all fields incl. `required_capability`, `subid`).
**Steps:** derive public tool descriptors from authoritative PluginSchema
`MethodDecl`s (or require a checked one-to-one mapping); add
`output_schema`/`required_capability`/`subid`/`side_effect`/`idempotent`/
`approval_required` to the trait + `ToolDefinition`; reject a descriptor that
diverges from its sealed method, unknown subid, or duplicate name. Make `execute`
enforce the target tool's `required_capability` against the resolved identity before
running and record the decision in the event chain. Implement one sanitized schema
projection shared by authenticated reflection, MCP `tools/list`/`get_tool_schema`, all
blob tools/resources/search, vectorization, caching, and logging. It includes callable
shapes/public descriptions/capability names/subids/side-effect/idempotency/approval
metadata but strips grants, identity footprints, private org/tenant fields, internal
paths/implementation symbols, key material, and sensitive defaults/examples before
any downstream use. Authenticated reflection returns the complete **sanitized**
catalog per FR-9a. If raw operations access is retained, add only a separately named
Read descriptor `admin_blob_schema_raw` with `schema.raw.admin`, exact pinned hash,
mandatory reason, size bound, and audit; it is absent from ordinary listings and
resources.
**Verify:** `cargo test -p op-mcp --test tool_authz -- --nocapture`;
`cargo test -p op-plugins --test plugin_subid_validation -- --nocapture`.
**DoD:** per-tool authz + collision rejection + descriptors complete.
**Deps:** T2.2, T1.4.

### T4.2 — Nested argument validation (fail closed)
**Reqs:** FR-5a
**Files:** the `invoke_tool`/`execute_tool` dispatch path; `tool_registry.rs`.
**Tests first:** `nested_args_validated` (T0.3) → green; `missing_tool_schema_fails_closed`.
**Steps:** resolve target tool `input_schema`; validate nested `arguments` (draft-07)
before execution; missing/invalid schema → reject.
**Verify:** `cargo test -p op-mcp --test tool_authz nested_args_validated -- --nocapture`;
`cargo test -p op-mcp --test tool_authz missing_tool_schema_fails_closed -- --nocapture`.
**DoD:** unvalidated args never reach a tool.
**Deps:** T4.1.

### T4.3 — Durable operation admission, cancellation, and output validation
**Reqs:** FR-3e, FR-3g, SEC-9, DR-5, NFR-4, DEP-5
**Files:** `crates/op-mcp/src/tool_registry.rs` (`execute` return path),
`crates/op-grpc-bridge/src/mutation_engine.rs`,
`crates/op-grpc-bridge/src/execution_ledger.rs` (new), the CallMethod/adapter dispatch
path; `crates/op-grpc-bridge/tests/output_validation.rs` (new),
`crates/op-grpc-bridge/tests/operation_idempotency.rs` (new).
**Tests first:**
- `output_schema_validated` (T0.3) → green: malformed output → error; nothing
  returned/persisted/vectorized/prompt-injected, but the attempt remains auditable.
- `output_failure_has_intent_and_outcome` validates correlation and redaction without
  storing malformed output; transactional prepared state is not committed and a
  non-transactional partial effect queues reconciliation.
- `operation_retry_is_durable_and_exactly_once`, `operation_binding_reuse_denied`,
  and `non_idempotent_retry_denied` cover transport retry and bridge restart.
- `cancellation_classifies_commit_races` injects cancellation before admission,
  after prepare, after commit, and after a non-transactional external effect.
**Steps:** add a dedicated durable execution ledger via the existing Cozo edge with
versioned relations `execution_intent`, `execution_outcome`, `response_outbox`, and
`reconciliation_action`. Bind a bridge-issued/validated `operation_id` to actor,
derived scope hash, target subid, canonical input hash, policy version,
schema/contract hash, idempotency declaration, and approval ID. Atomically insert the
redacted intent before dispatch. For `idempotent=true`, an identical retry returns
only its committed response-outbox record; changed binding is denied. For
`idempotent=false`, any repeated operation ID is denied. Close/reconcile every open
intent before re-admission on restart. For transactional mutations use prepare →
execute candidate → output validate → commit; record non-transactional limitations
before dispatch. Validate every output before return/persistence/vector/prompt use,
then atomically append the terminal outcome plus validated/redacted response outbox;
release only the committed outbox. Invalid output records `invalid_output` without
the candidate; an external effect records `partial_side_effect` and enqueues a unique
idempotent reconciliation action. Propagate cancellation/deadline and block
uncommitted admission/commit, but never report an already committed effect as rolled
back. Classify all denied/invalid/cancelled/timed-out/failed/successful states.
**Verify:** `cargo test -p op-grpc-bridge --test output_validation -- --nocapture`;
`cargo test -p op-grpc-bridge --test operation_idempotency -- --nocapture`.
**DoD:** output is validated before every use; durable operation dedupe/outbox and
cancellation classification prevent duplicate or untracked side effects across
retry/restart; every attempt has one bound intent with linked outcomes.
**Deps:** T4.1.

### T4.4 — Bridge-derived scope; arguments narrow, never replace
**Reqs:** FR-3f, SEC-6
**Files:** memory input (`crates/op-cognitive-mcp/src/cognitive_tools.rs`), code tools
(`code_tools.rs` — `session_id`, `collections_from`, path handling); scope-derivation
in the dispatch path; `crates/op-grpc-bridge/tests/scope_binding.rs` (new),
`crates/op-cognitive-mcp/tests/coding_scope.rs` (new).
**Tests first:** `forged_scope_denied` (T0.3) → green across read/write/delete/
context-stream/semantic-query/coding; `coding_path_canonicalized_and_bounded` — `../`,
absolute-outside-root, and symlink-escape rejected; oversized archive rejected;
target collection authorized.
**Steps:** derive container/identity/namespace/workspace/collection/session_id/path
root from the ExecutionContext (T1.4), not from caller `arguments`; allow arguments to
NARROW within the authorized scope only; for coding tools enforce a workspace root,
canonicalize paths, reject traversal + symlink escape, authorize the target
collection, and bound archive/input sizes.
**Verify:** `cargo test -p op-grpc-bridge --test scope_binding -- --nocapture`;
`cargo test -p op-cognitive-mcp --test coding_scope -- --nocapture`.
**DoD:** scope is bridge-derived; arguments cannot replace/widen it; coding path +
size + collection guards enforced.
**Deps:** T4.1.

### T4.5 — MCP adapter conformance + capability-filtered views + read/execute plugin-method adapter
**Reqs:** FR-2, FR-2a, FR-3a, FR-4, FR-5, FR-9a, FR-3g, SEC-14, NFR-4
**Files:** `crates/op-grpc-bridge/src/mcp_frontend.rs` mounted into the shared
Axum/Tonic frontend; `crates/op-grpc-bridge/tests/mcp_modern_2026_07_28.rs` (new),
`crates/op-grpc-bridge/tests/mcp_legacy_stateful.rs` (new).
**Tests first:**
- Modern suite: `modern_version_negotiation`, `modern_stateless_tools_list_paginated`,
  `modern_resources_list_read_blob`, `modern_notifications`,
  `modern_cancellation_aborts_tool`, `modern_body_limit_timeout_origin`,
  `modern_http_oia_encoding_is_canonical`, and
  `modern_rejects_session_as_auth`. It sends no `Mcp-Session-Id` and presents a fresh
  one-use OIA1 assertion on every request/turn.
- Legacy suite: `legacy_initialize_before_use`, `legacy_session_correlation_only`,
  `legacy_requires_fresh_assertion_every_request`, `legacy_expiry_and_bounded_state`,
  and `legacy_origin_checked_http_and_grpcweb`.
- Shared invariants: `filtered_views_differ_by_identity`,
  `public_tool_schema_is_sanitized`, and `plugin_adapter_crosses_dbus_once`.
**Steps:** implement canonical MCP **2026-07-28 stateless** negotiation and required
headers as the default, without using a server session as lifecycle or authority.
Implement JSON-RPC errors, notifications, cancellation, body limit, timeout,
pagination, `resources/*`, and exact-allowlist `Origin`/CORS checks on browser HTTP
and gRPC-Web. Reject wildcard/reflected/`null`/missing/duplicate/malformed origins
before assertion parsing; set `Vary: Origin`; allow only required preflight methods
and headers; never forward OIA/approval headers across an origin-changing redirect or
log them. Native gRPC/UDS do not manufacture Origin. Define one bounded
base64url-without-padding OIA1 HTTP header encoding and reject duplicates,
non-canonical/oversized/trailing/conflicting representations. Put the older
stateful initialize/`Mcp-Session-Id` behavior behind an explicit compatibility mode
with bounded TTL/count; the session ID is correlation only and every request still
requires a fresh one-use OIA1 assertion. Capability-filter `list_tools`/
`search_tools`; serve only the sanitized T4.1 schema projection. The read/execute
adapter maps generated sealed-plugin methods to `PluginService.CallMethod → D-Bus →
MutationEngine` with the attenuated ExecutionContext—never directly to the registry.
Mount both modes on T1.1's existing TLS acceptor. Retain SSE only if T0.2 records a
consumer and cover it with the same auth path.
**Verify:** `cargo test -p op-grpc-bridge --test mcp_modern_2026_07_28 -- --nocapture`;
`cargo test -p op-grpc-bridge --test mcp_legacy_stateful -- --nocapture`; a TLS modern
client performs `tools/list`→`resources/list`→safe `tools/call` without a session
header; a legacy initialize flow succeeds only with fresh assertion material on each
request.
**DoD:** modern stateless and legacy stateful behavior are separately specified and
tested; session ID never authenticates; HTTP assertion encoding and browser CORS are
canonical/fail-closed; schemas are sanitized; every call uses the shared ingress and
canonical D-Bus dispatch.
**Deps:** T4.2, T4.3, T4.4, T1.1, T1.5.

---

## Phase 5 — Blob-schema MCP consolidation

### T5.1 — One canonical blob_catalog + resources; remove duplicate
**Reqs:** FR-8, FR-8a, FR-8b, FR-3c, DR-2
**Files:** `crates/op-mcp/src/blob_schema.rs`, `resources.rs`; remove
`crates/op-cognitive-mcp/src/blob_catalog_tool.rs`; registration in
`cognitive_tools.rs`; `crates/op-mcp/tests/blob_resources.rs` (new),
`crates/op-cognitive-mcp/tests/blob_registration.rs` (new).
**Tests first:** `blob_catalog_full_covers_manifest` (T0.3) → green;
`single_blob_catalog_registered`; `blob_full_snapshot_cursor_paginated`;
`blob_resource_read`; `schema_sentinel_absent_from_every_public_sink`;
`raw_schema_requires_admin_reason_and_audit`.
**Steps:** register only the `op-mcp` blob tools; delete the duplicate; ensure
`full` mode and `resources/list` use immutable `{catalog_hash,generation}` snapshots,
lexicographic `(plugin_id,schema_hash)` ordering, default page size 50/max 100, and an
integrity-protected actor/filter/last-key-bound five-minute cursor; expired returns
`CURSOR_EXPIRED`, changed actor/filter returns `INVALID_CURSOR`. Return only T4.1's
deterministic sanitized projection through catalog/schema/manifest/methods/search/
reflection/resources/vector/cache/log sinks. Raw sealed bytes, if retained, use only
the separately authorized/audited admin method. Keep blob-vector semantics
(user-triggered, UUIDv5 from canonical plugin id, one point/plugin, wholesale refresh,
Qdrant-fail-closed).
**Verify:** `cargo test -p op-mcp --test blob_resources -- --nocapture`;
`cargo test -p op-cognitive-mcp --test blob_registration -- --nocapture`; over `:8090`:
`blob_catalog(mode=full)` id set == `read_manifest_plugin_ids_shm` set.
**DoD:** one blob_catalog; resources work; no consumer writes the blob dir.
**Deps:** T4.5.

### T5.2 — Exact schema-hash blob resolution + atomic vector refresh
**Reqs:** DR-8, FR-8a
**Files:** `crates/op-blob/src/catalog.rs` (`read_plugin_schema_shm` — first-prefix →
exact `<schema_hash16>` match), the blob-vector refresh path (atomic generation
swap); `crates/op-blob/tests/blob_resolution.rs` (from T0.3),
`crates/op-cognitive-mcp/tests/blob_vector_catalog.rs` (new).
**Tests first:** `exact_hash_blob_resolution` (T0.3) → green: with two blobs for one
plugin id, the manifest-pinned hash is returned; `vector_refresh_is_atomic` — an
interrupted refresh leaves the prior generation active and queryable (no
half-built collection served); `blob_vector_catalog_hash_matches` — active Qdrant
generation metadata contains the exact canonical manifest/catalog hash, count, and
schema hashes; adding/removing/resealing a plugin changes the generation and no stale
plugin point remains.
**Steps:** resolve a plugin by the manifest's exact `<plugin_id>.<schema_hash16>.blob`
(reject/ignore non-matching prefixes); make blob-vector refresh atomic (build into a
new temporary collection/generation, write one deterministic UUIDv5 point per
canonical `plugin_id` with payload `{plugin_id,schema_hash16,catalog_hash,
generation}`, verify its count and full catalog hash, atomically swap the active
alias, then delete the previous generation only after the swap. Keep the previous
generation on any embed/write/hash/count/alias failure and clean the incomplete
temporary collection. Compute `catalog_hash` from sorted manifest entries and their
exact sealed schema hashes; removed/resealed plugins cannot leave a point in the
active generation. Query refuses a generation whose metadata does not match the
active manifest. Embed only the sanitized FR-8b projection, never raw sealed JSON.
**Verify:** `cargo test -p op-blob --test blob_resolution -- --nocapture`;
`cargo test -p op-cognitive-mcp --test blob_vector_catalog -- --nocapture`;
`rg -n 'first|prefix' crates/op-blob/src/catalog.rs` shows no first-prefix resolution remains.
**DoD:** exact-hash resolution; atomic vector refresh.
**Deps:** T5.1.

---

## Phase 6 — Context-awareness relocation

### T6.1 — Serve context on :8090, event-driven; remove independent server
**Reqs:** FR-10, NFR-2, SEC-5, FR-3f
**Files:** `crates/op-cognitive-mcp/src/context_awareness.rs` (remove 5 s
`EVALUATION_INTERVAL_MS` poll; evaluate on `record_activity`), `context_server.rs`
(routes mounted under the bridge builder, no independent listener);
`build_operation_routes`; `crates/op-cognitive-mcp/tests/context_awareness.rs` (new),
`crates/op-grpc-bridge/tests/context_surface.rs` (new).
**Tests first:** `context_push_is_event_triggered` (a build-error event produces a push
without a timer), `context_scoped_by_identity`, `context_token_budget_bounded`,
`no_independent_context_listener`.
**Steps:** mount context routes on `:8090` via the shared builder; replace the poll
with event-triggered evaluation; enforce identity scope (bridge-derived, FR-3f) +
token budget; keep all existing signals (file opened, edit applied, build error, test
failure, diff viewed, symbol navigation, tool call, query, context switch,
stuck-session, error assistance, topic change, idle recovery).
**Verify:** `cargo test -p op-cognitive-mcp --test context_awareness -- --nocapture`;
`cargo test -p op-grpc-bridge --test context_surface -- --nocapture`;
`rg -n 'EVALUATION_INTERVAL_MS|tokio::time::interval' crates/op-cognitive-mcp/src/context_awareness.rs` → none (deadline/heartbeat timers, if any, justified).
**DoD:** context on `:8090`, reactive, scoped, bounded.
**Deps:** T4.5.

### T6.2 — Onboarding surface + context idle one-shot deadline + stream resume cursor
**Reqs:** FR-18, FR-19, SEC-13, SEC-14, DR-9, NFR-2
**Files:** `context_awareness.rs` (idle recovery → per-session one-shot deadline;
durable stream resume cursor), `context_server.rs` (resume endpoint),
`crates/op-plugins/src/state_plugins/identity_sled.rs` (OIG1-derived principal/session
anchor), the shared ingress onboarding adapter (public registration/session
genesis before an OIA1 exists), cognitive-memory migration files;
`crates/op-grpc-bridge/tests/onboarding_and_resume.rs` (new),
`crates/op-cognitive-mcp/tests/context_restart.rs` (new).
**Tests first:**
- `onboarding_is_distinct_ratelimited_no_exec` — the onboarding surface is
  exactly `POST /genesis/complete`, rate-limited, non-actor-attributed, and cannot reach tool
  discovery/execution; its events are pre-auth telemetry, not event-chain records.
- `genesis_accepts_only_bound_oig1` — exact canonical OIG1 signed by a trusted Oracle
  decoy, with correct purpose/time/source/human-key proof and server-derived IDs,
  succeeds once; unsigned, unknown-key, bad-signature, expired/future, replayed,
  wrong-source/purpose, raw email/alias, and caller-chosen IDs are rejected without
  consuming a still-valid wrong-binding nonce.
- `genesis_origin_csrf_and_anti_enumeration` — browser Origin/CSRF/body limits and
  enumeration-safe duplicate responses hold; no public SendMagicLink,
  VerifyMagicLink, admin registration, schema, or reflection route exists.
- `only_liveness_public` — unauthenticated liveness succeeds; unauthenticated
  `tools/list`, reflection, context stream, memory, `execute_tool` are rejected before
  dispatch.
- `idle_recovery_fires_once_per_episode` — an idle session fires exactly one recovery
  per idle episode (no repeating timer); a returning session re-arms.
- `context_stream_resumes_from_cursor` — a reconnect resumes from the last delivered
  cursor (no gap, no full replay).
- `context_cursor_survives_process_restart` — persist events, acknowledge cursor,
  destroy the bridge/context process and all in-memory rings, reopen the store, and
  resume at the next durable event with no gap/duplicate outside documented
  at-least-once retry semantics.
**Steps:** expose exactly `GET /healthz` and `POST /genesis/complete` in the public
router. Genesis accepts a bounded canonical OIG1 envelope containing version/purpose,
human public key, Netmaker inner IP, server derivation inputs, trusted decoy key ID,
issued-at, expiry ≤15 minutes, and nonce. Apply pre-signature rate/body/depth limits;
for browsers enforce exact Origin+CSRF; verify canonical parse, trust/signature,
purpose/time, transport/Xray source binding, and proof; derive principal/session IDs
server-side; then atomically consume the OIG1 nonce in T1.5 and anchor identity. It is
a special pre-auth genesis transaction and MUST NOT enter the executable dispatch
catalog/D-Bus/tool path, grant capabilities, return an OIA1, or expose existence;
return only `{status,onboarding_version}`. Remove other public registration/bootstrap
interceptors. Replace repeating idle timers with per-session one-shot deadlines.
Add migrated Cozo relations logically separate from user memory:
`context_journal_v1(identity_id,container_id,workspace_id,session_id,sequence,event_id,
event_type,payload_redacted,payload_hash,source_event_id,supersedes_event_id,created_at)`
and `context_checkpoint_v1(identity_id,container_id,workspace_id,session_id,
last_evaluated_sequence,last_acked_sequence,idle_episode_generation,summary_redacted,
summary_hash,updated_at,expires_at)`. Allocate gap-free per-scope sequence and append
atomically before acknowledgement; corrections are linked immutable events. Persist
delivery acknowledgements; load checkpoint/journal before accepting subscriptions;
make rule evaluation idempotent by `(session_id,sequence,rule_id)`. Resume strictly
after an opaque MAC-protected, expiry/scope-bound cursor. Retention writes a sanitized
checkpoint before pruning and returns typed `CursorExpired` with checkpoint cursor +
summary. Locked/unavailable journal makes context mutation/subscription `Unavailable`
with no authoritative in-memory fallback.
**Verify:** `cargo test -p op-grpc-bridge --test onboarding_and_resume -- --nocapture`;
`cargo test -p op-cognitive-mcp --test context_restart -- --nocapture`.
**DoD:** only health and exact OIG1 completion are public; onboarding is
anti-enumerating/rate-limited/exec-free; the scoped context journal is durable,
gap-free, and fail-closed; idle recovery is one-shot and streams resume after restart.
**Deps:** T6.1, T1.5.

---

## Phase 7 — Chatbot memory integration

### T7.1 — One authoritative Cozo writer; op-chat via bridge; no silent fallback
**Reqs:** DR-1, DR-1a, SEC-6, DR-6, FR-3f
**Files:** `crates/op-chat/src/main.rs`, `actor.rs`, `chat_service.rs`,
`memory_loop.rs`, `crates/op-chat/src/bridge_memory_client.rs` (new),
`crates/op-cognitive-mcp/src/server.rs` (durable-unavailable → `Unavailable`), memory
store and runit client TLS/OIA configuration;
`crates/op-chat/tests/bridge_memory_client.rs` (new),
`crates/op-cognitive-mcp/tests/memory_availability.rs` (new).
**Tests first:** `single_cozo_writer` (op-chat opens no cognitive-memory handle),
`durable_memory_unavailable_not_silent` (T0.3) → green, `memory_isolated_by_identity_container_workspace`, `memory_domain_tagged`.
**Steps:** remove every op-chat open of `/var/lib/op-dbus/chat-memory.db`; add one
`BridgeMemoryClient` used by unary and streaming chat paths. It connects with verified
TLS to the shared bridge (`127.0.0.1:8090` or the TLS-only UDS), obtains a fresh OIA1
assertion for every recall/write request, sends no caller-authored ExecutionContext,
and calls the PluginSchema-derived memory methods. Configure CA/client identity and
timeouts through runit; never fall back to a local database or unauthenticated client.
Enforce memory domains and bridge-derived isolation.
**Verify:** `cargo test -p op-chat --test bridge_memory_client -- --nocapture`;
`cargo test -p op-cognitive-mcp --test memory_availability -- --nocapture`; start
op-chat with bridge up → no Cozo lock conflict; invalidate its assertion → memory RPC
is rejected before dispatch.
**DoD:** one writer; isolation + domains enforced; no silent in-mem writes.
**Deps:** T2.1, T4.4, T4.5, T1.3.

### T7.2 — Recall + injection with provenance; untrusted-data injection; redaction
**Reqs:** FR-13, SEC-7, SEC-8, SEC-11, SEC-12
**Files:** memory recall/injection path (op-chat client + bridge memory tools);
redaction/size-bound helper in the persistence + prompt path;
`crates/op-chat/tests/memory_recall.rs` (new),
`crates/op-cognitive-mcp/tests/sink_redaction.rs` (new).
**Tests first:**
- `recall_ranks_and_injects_topk_with_provenance` (six provenance fields).
- `injected_memory_is_not_instruction` (injection text changes nothing).
- `unverified_not_promoted_and_outranked`.
- `noncurated_memory_injected_as_untrusted_delimited` (SEC-12) — non-curated memory
  is placed in a delimited untrusted-data section; a memory whose text mimics a system
  instruction does not change model role assignment or tool selection.
- `secrets_redacted_before_every_sink` (SEC-11) — a credential pattern in an
  arg/result is redacted in Cozo, Qdrant, log, event, and prompt; an oversized
  arg/result is truncated/rejected per policy, never stored raw.
**Steps:** identity-scoped recall → semantic rank vs prompt → top-k injection with
provenance; treat memory as data; inject non-curated memory only as delimited
untrusted data (only curated system memory may take an instruction/system role via the
authorized curation path); redact secrets + bound size BEFORE any sink
(Cozo/Qdrant/log/event/prompt).
**Verify:** `cargo test -p op-chat --test memory_recall -- --nocapture`;
`cargo test -p op-cognitive-mcp --test sink_redaction -- --nocapture`.
**DoD:** FR-13 + SEC-7/8/11/12 green.
**Deps:** T7.1.

### T7.3 — Concrete memory schema + durable idempotent Cozo↔Qdrant outbox
**Reqs:** DR-1, DR-6, DR-7, FR-15, DEP-5
**Files:** `crates/op-cognitive-mcp/src/cozo_shuttle.rs` (versioned migrations),
`memory_store.rs`, `qdrant_shuttle.rs` (transactional outbox/reconciler),
`crates/op-cognitive-mcp/tests/memory_schema.rs` (new),
`crates/op-cognitive-mcp/tests/reconcile_outbox.rs` (new).
**Tests first:**
- `kill_mid_reconcile_restart_converges` — killing the bridge mid-reconcile and
  restarting drains the outbox; Cozo and Qdrant converge (no lost/duplicated points).
- `stale_revision_suppressed_and_deleted_not_returned` — a stale-revision Qdrant entry
  is suppressed; a deleted memory is not returned by a semantic query (no tombstone
  leakage to callers).
- `point_ids_stable_replay_idempotent` — point IDs stable across re-embeds; re-running
  replay does not duplicate.
- `tenant_point_ids_do_not_collide` — equal record IDs/keys in different identity,
  container, workspace, or domain scopes produce distinct point IDs.
- `memory_migration_v1_to_v2_preserves_scope_revision` — fixture data migrates once,
  restart is idempotent, and an incompatible/newer schema fails closed.
**Steps:** add versioned, transactional, restart-idempotent migrations for:
`memory_v2(memory_id,stable_record_key,revision,domain,actor_id,container_id,
workspace_id,session_id,kind,content_redacted,content_hash,trust,confidence,
source_event_id,source_memory_ids,created_at,updated_at,expires_at,tombstoned_at)`,
`memory_outbox_v1(outbox_id,memory_id,revision,op,payload_ref,state,attempts,
next_attempt_at)`, `memory_feedback_v1(feedback_id,suggestion_id,memory_id,actor_id,
outcome,verifier,evidence_hash,event_id,created_at)`, and
`memory_migration_v1(version,checksum,started_at,committed_at,
source_high_watermark,row_count,content_merkle_root)`. Absence of a scope column never
means global; domain rules determine mandatory columns and every access index includes
the full applicable server-derived scope. Define stable IDs/point IDs as UUIDv5 of
the canonical `(domain,actor_id,container_id,workspace_id,stable_record_key)`;
legacy import derives `stable_record_key` deterministically, while a new record gets
a server-issued immutable key. The Qdrant payload carries the same full scope tuple,
revision, tombstone=false, trust/provenance hash, and schema version.
Commit entry mutation and an `outbox_id` deterministically bound to the bridge
`operation_id` atomically; make write/delete/
correction idempotent using the bridge request id plus scope. Drain with conditional
upsert/delete by monotonic revision; acknowledge only after Qdrant success; recover
pending rows on restart and suppress stale results against Cozo truth. A tombstone is
never returned. Maintain a migrated `vector_catalog_v1` relation with collection,
embedding model/dimension, schema version, active generation, and catalog hash; query
fails closed on incompatible dimension/schema and degrades explicitly when Qdrant is
down. Before switching writers, quiesce legacy `/var/lib/op-dbus/chat-memory.db`, use
its supported backup/export path, and record high-water mark/count/Merkle root. Import
to staging with deterministic IDs, quarantine invalid rows, verify per-domain hashes,
and atomically switch only after validation; retain the legacy store read-only through
soak. Journal every post-cutover write in a versioned export format so rollback can
preserve/replay new data without a second writer or silent loss.
**Verify:** `cargo test -p op-cognitive-mcp --test memory_schema -- --nocapture`;
`cargo test -p op-cognitive-mcp --test reconcile_outbox -- --nocapture`.
**DoD:** reconciliation is durable + idempotent + restart-recovering; deleted memory
never leaks as tombstone.
**Deps:** T7.2.

---

## Phase 8 — Suggestion-feedback and outcome-consolidation loop

### T8.1 — Post-turn structured persistence (replace regex)
**Reqs:** FR-14, SEC-9, SEC-11
**Files:** `crates/op-chat/src/memory_loop.rs`,
`crates/op-chat/tests/post_turn_persistence.rs` (new).
**Tests first:** `post_turn_persists_tool_args_and_results`,
`accepted_and_rejected_are_distinct_records`.
**Steps:** replace regex extraction with structured extraction; persist tool
name/args/result (redacted + size-bounded per SEC-11, T7.2); model
accepted/rejected/corrected outcomes.
**Verify:** `cargo test -p op-chat --test post_turn_persistence -- --nocapture`.
**DoD:** structured persistence; tool args/results stored (redacted); feedback modeled.
**Deps:** T7.3.

### T8.2 — Lifecycle, cross-store consistency, consolidation, bounded evolution
**Reqs:** FR-15, FR-16, DR-6, DR-7, SEC-8
**Files:** memory store (Cozo + Qdrant reconciliation via T7.3 outbox), consolidation
path, `crates/op-cognitive-mcp/tests/memory_lifecycle.rs` (new).
**Tests first:** `dedup_correction_expiry_decay`, `deletion_tombstone_reconciles_qdrant`,
`repeated_success_promoted_to_semantic`, `evolution_invokes_no_training_api`,
`promotion_requires_capability`.
**Steps:** dedup/correction/deletion(tombstone)/expiry/decay; Qdrant reconciliation
(via the DR-7 outbox) + derived-memory invalidation; episodic→semantic promotion gated
by capability; loop changes ranking only (no weights/policy/caps/auth).
**Verify:** `cargo test -p op-cognitive-mcp --test memory_lifecycle -- --nocapture`.
**DoD:** FR-15/16 + DR-6 green; no training API invoked.
**Deps:** T8.1.

### T8.3 — Evidence-backed coding suggestion/application + authorized single-use approval
**Reqs:** FR-12, SEC-10, FR-3b, FR-3f
**Files:** `crates/op-cognitive-mcp/src/coding_suggestions.rs` (new typed contract
and orchestration), `crates/op-plugins/src/state_plugins/cognitive_mcp.rs`
(PluginSchema-derived suggest/apply methods), apply tool
(`approval_required=true`), approval-token verifier and authoritative approver
capability lookup;
`crates/op-grpc-bridge/tests/approval_binding.rs` (new).
**Tests first:**
- `suggestion_does_not_apply` — a suggestion request returns a structured suggestion
  (evidence + proposed diff) via the mandatory contract WITHOUT applying a change.
- `apply_requires_capability_and_approval` — applying requires the apply capability
  AND a valid signed single-use approval token bound to
  suggestion/diff-hash/base-revision/actor/session/expiry/nonce; a suggestion-only
  identity is denied.
- `approval_mismatch_expired_reused_denied` — a token whose diff-hash/base-revision/
  actor/session does not match, or whose nonce was already used, or which has expired,
  is rejected (fail closed).
- `approval_requires_authorized_approver` — a cryptographically valid signature from
  an identity lacking `coding.approve`, an inactive/revoked approver or key, an
  untrusted issuer, or a proposer violating separation-of-duty policy is rejected.
- `apply_idempotency_is_exact` — retrying the same apply request returns the prior
  result without applying twice; reusing its idempotency key for another diff/base
  revision fails closed.
- `suggestion_evidence_and_verification_required` — cited code context is in the
  authorized workspace and bound by content hash/base revision; an applied result is
  not `successful` until declared verification commands complete and the resulting
  revision/diff hash are independently observed.
- `all_outcomes_recordable_and_promotion_uses_verified` — accepted/rejected/corrected/
  failed/successful are recordable/readable; promotion uses independently verified
  outcomes, not chatbot self-labels.
**Steps:** define a sealed, typed `CodingSuggestion` output containing
`schema_version,suggestion_id,idempotency_key,actor_id,identity_id,workspace_id,
base_revision,evidence[{path,start_line,end_line,content_hash,reason}],rationale,
proposed_patch,diff_hash,affected_paths,context_cursor,prior_outcome_ids,risk,
verification_commands,created_at,expires_at`; reject missing/unknown fields and
compute IDs/hashes server-side. The
Read-only `coding.suggest` tool gathers scope-bound code/context/test evidence,
produces the patch, and writes no workspace data. Define `CodingApplyRequest` as
`suggestion_id,idempotency_key,diff_hash,base_revision,verification_commands` plus the
bridge-supplied approval—not caller authority. The Mutation `coding.apply` tool
requires `coding.apply` and a signed approval with
`version,purpose,approval_id,approver_id,issuer,key_id,
approval_capability=coding.approve,
suggestion_id,diff_hash,base_revision,apply_actor_id,workspace_id,session_id,
target_tool,target_subid,policy_version,schema_version,issued_at,expires_at,nonce`.
Resolve approver/key status and `coding.approve` from authoritative
identity/capability state at use time and require the approver to differ from both
proposer and applying actor. Consume the nonce in
T1.5's approval replay domain atomically with the apply intent, and re-check the base
revision immediately before mutation. Canonicalize/apply once, run only allowlisted
scope-bounded verification commands, independently observe final diff/revision/test
results, and record proposed/approved/applied/verified or failed outcomes. Only
verified outcomes feed ranking/promotion.
**Verify:** `cargo test -p op-grpc-bridge --test approval_binding -- --nocapture`;
`cargo test -p op-cognitive-mcp --test coding_suggestions -- --nocapture`.
**DoD:** suggestion/apply are separated by a PluginSchema-derived contract;
idempotency, approver authority, separation of duties, exact-change binding, and
independent verification are fail-closed; only verified outcomes drive evolution.
**Deps:** T8.2, T4.1.

---

## Phase 9 — Web MCP route removal

### T9.1 — Repoint dashboard to :8090 gRPC-Web (PRECONDITION for proxy delete)
**Reqs:** TR-4, SEC-14, FR-3g, CR-5
**Files:** `/srv/git/operation-dashboard-ui-07/src/grpc/client.ts` and
`src/grpc/host-runtime.ts` (direct bridge target + per-request assertion hook),
`/srv/git/operation-dashboard-ui-07/tests/e2e/direct-grpc-web.spec.ts` (new),
`/srv/git/operation-dashboard-ui-07/playwright.config.ts` and `package.json`,
`crates/op-web/src/routes/oia_broker.rs` (new narrow issuance route),
`crates/op-grpc-bridge/src/grpc_web.rs`,
`crates/op-grpc-bridge/tests/browser_origin.rs` (new).
**Tests first:** `dashboard_grpcweb_on_8090_e2e` — a real browser obtains a one-use
OIA1 through its authenticated session/CSRF-protected broker, reaches gRPC-Web on
`:8090` directly, and discards the assertion; `browser_origin_matrix` rejects exact
wrong scheme/host/port, `null`, missing/duplicate Origin, hostile preflight, and
cross-origin redirect without credential reflection/logging or dispatch;
`browser_replay_retry_keeps_operation_id` rejects replay and permits at most one fresh
assertion retry with the same application operation ID.
**Steps:** add only `POST /api/oia/v1/assertion` on op-web as an issuance broker—not
a proxy. Require authenticated HttpOnly session, exact Origin, CSRF, per-user/source
limits, and a request naming bridge authority, RPC/MCP method, exact target, and
browser nonce. The broker asks the Oracle/decoy issuer for a short-lived OIA1 bound to
that user/session/source/target and cannot sign or alter identity; response is
`Cache-Control: no-store` and reveals no grants. Configure the bridge with the exact
scheme+host+port allowlist and OIA binary-metadata preflight header. Point the
dashboard at direct `https://<bridge>:8090`; obtain/discard a fresh OIA1 for every
call, never use a static identity header, never follow an origin-changing redirect
with auth metadata, and on expiry/replay retry at most once with the same FR-3g
operation ID. Run the browser E2E to PASS before proxy removal.
**Verify:** `cargo test -p op-grpc-bridge --test browser_origin -- --nocapture`;
`cargo test -p op-web --test oia_broker -- --nocapture`; from
`/srv/git/operation-dashboard-ui-07`, `npm run test:e2e -- direct-grpc-web.spec.ts`.
**DoD:** dashboard works against `:8090` gRPC-Web directly, verified — the precondition
for T9.2 is met; OIA issuance is narrow/no-store and `:8080` never proxies the call.
**Deps:** T4.5.
**Rollback:** revert the dashboard target (git); proxy still present until T9.2.

### T9.2 — Delete op-web gRPC proxy + MCP routes/aliases (→ 404/410)
**Reqs:** TR-4, FR-1, TR-3, CR-5, NFR-7
**Files:** `crates/op-web/src/grpc_proxy.rs` (delete, or replace with a non-MCP
allowlist), `crates/op-web/src/routes/mod.rs`, `mcp.rs`, `mcp_agents.rs`,
`mcp_smart_router.rs`, `mcp_discovery.rs` (`/jsonrpc`, `/rpc`, `/.well-known/mcp.json`,
discovery aliases).
**Tests first:**
- `no_op_web_grpc_proxy` (T0.3) → green: `application/grpc` to `:8080` no longer
  reaches `:8090`.
- `web_mcp_aliases_gone` — `/jsonrpc`, `/rpc`, `/.well-known/mcp.json`, MCP discovery
  aliases, `/mcp`, `/mcp/compact`, `/mcp/agents*` return 404/410.
- `web_serves_dashboard_and_rest` (unaffected).
**Steps:** delete `grpc_proxy.rs` (or replace with a strict non-MCP allowlist that
cannot reach cognitive/generated/memory/context/tool RPCs); delete the MCP router
mounts, handlers, smart-router, and discovery aliases; leave dashboard/REST.
**Verify:** `cargo test -p op-web --test no_grpc_proxy -- --nocapture`;
`curl -s -o /dev/null -w '%{http_code}' -H 'content-type: application/grpc'
http://127.0.0.1:8080/` does not reach `:8090`;
`curl -s -o /dev/null -w '%{http_code}'
http://127.0.0.1:8080/.well-known/mcp.json` → 404/410.
**DoD:** no MCP execution and no gRPC proxy on `:8080`; aliases gone.
**Deps:** T9.1.
**Rollback:** git revert the route/proxy removal (op-web only).

---

## Phase 10 — Standalone service/config/code deletion (IRREVERSIBLE)

⚠️ Gated on Phases 1–9 green AND the client inventory migrated (CR-1). Rollback is via
the prior btrfs golden snapshot (DEP-2).

### T10.1 — Migrate client configs and Xray routes
**Reqs:** CR-1, CR-4, CR-6
**Files:** `.mcp.json`, `~/.factory/mcp.json`, `.kiro/settings/mcp.json`,
`deploy/config/cognitive-mcp-clients.json`, `deploy/config/mcp-servers.json`,
`deploy/config/factory-mcp.json`, Xray route defs (`schema_bridge.rs` / xray config
generation).
**Steps:** repoint every MCP client entry to the `:8090` bridge path; repoint any
Xray `mcp.internal`/`:50052`/`:3003` route to `:8090`.
**Verify:** `rg -n ':3003|:50052|:50051|:11438' deploy/config .mcp.json ~/.factory/mcp.json .kiro/settings/mcp.json` → matches only in removal/comment context.
**DoD:** no client points at a retired endpoint.
**Deps:** T9.2.

### T10.2 — Delete standalone listener code
**Reqs:** DEP-1, TR-3, CR-4
**Files:** `crates/op-cognitive-mcp/src/server.rs` (`start_http_server`,
`start_grpc_server`, `start_dual`, `serve_cognitive_grpc`), `main.rs` invocations;
`crates/op-mcp/src/main.rs` (reduce to client-only shim: no listener, no registry/DB
writer, always calls bridge, per CR-4).
**Tests first:** `no_listener_in_cognitive_mcp`, `op_mcp_server_is_client_shim`.
**Steps:** delete the deprecated listener fns and their invocations; reduce
`op-mcp-server` to the bridge-calling shim.
**Verify:** `cargo build --workspace`; `rg -n 'start_http_server|serve_cognitive_grpc|:3003|:50052' crates/op-cognitive-mcp/src crates/op-mcp/src` → none.
**DoD:** no standalone MCP listener code remains.
**Deps:** T10.1.

### T10.3 — Retire runit service definitions and forwarders (both spec trees where applicable)
**Reqs:** DEP-1, TR-3, FR-1, NFR-6
**Files:** delete `deploy/runit/{op-mcp-agents,op-mcp-compact,op-mcp-blob-schema,op-mcp-cognitive,op-waypipe-grpc}`, `deploy/runit/install-op-mcp-agents.sh`, and forwarders `deploy/runit/{fwd-8090,fwd-nm-mesh-8090,fwd-nm-tonic-8081}` that target retired ports; reduce/retire `deploy/runit/op-cognitive-mcp` to the client shim.
**Steps:** remove the service dirs to match the live host; retire the `fwd-8090`
Python `socket-relay` now that the bridge directly binds `:8090` (T1.6); keep
`op-cognitive-mcp` only if it is a listenerless shim (CR-4).
**Verify:** for each remaining `deploy/runit/*/run`, `rg -n ':11438|:3003|:50052|socket-relay|tcp-listen' deploy/runit` → none in an active service;
`sudo sv status fwd-8090 fwd-nm-mesh-8090` → both down/absent.
**DoD:** repo `deploy/runit/` matches the target (no standalone MCP listener service,
no `:8090` forwarder).
**Deps:** T10.2, T1.6.
**Rollback:** use the prior authenticated btrfs golden plus T12.0's data procedure;
keep retired relays/listeners blocked rather than reviving an old unauthenticated path.

---

## Phase 11 — Schema resealing and route regeneration

### T11.1 — Reseal and regenerate; reflection/callable parity (both directions)
**Reqs:** DR-2, DR-3, FR-9
**Files:** `op-blob` seal path; `dynamic_reflection.rs`; `build.rs`
(`generate_plugin_method_routes`).
**Tests first:**
- `every_reflected_rpc_is_callable` — every reflected `operation.method.*` reaches a
  mounted route (never UNIMPLEMENTED/route-not-found).
- `every_mounted_method_is_reflected` — the reverse direction: no mounted method is
  absent from reflection (parity holds both ways).
- `sealed_methods_appear_in_reflection`.
**Steps:** rebuild so `build.rs` regenerates routes; reseal via `op-blob`; hydrate
reflection from the sealed catalog; assert both-direction parity.
**Verify:** `CXXFLAGS="-include cstdint" cargo build -p op-grpc-bridge`; over `:8090`
TLS: every reflected `operation.method.*` is callable AND every mounted method is
reflected.
**DoD:** parity holds in both directions.
**Deps:** T10.3.

### T11.2 — Hot-seal bounds: compatibility plus active-call generation gate
**Reqs:** FR-9, DR-8
**Files:** `op-blob` activation path (compatibility + schema-hash validation),
`dynamic_reflection.rs` (do not advertise unmounted methods);
`crates/op-grpc-bridge/tests/hot_seal_bounds.rs` (new).
**Tests first:**
- `incompatible_hash_activation_rejected` — activating a blob with an
  incompatible/mismatched schema hash is rejected (fail closed) before it can serve.
- `compiled_but_unsealed_uncallable` — a compiled-in method that is not sealed is not
  callable (sealing is required to activate a route).
- `unsealed_new_shape_not_advertised_as_callable` — a newly sealed method shape not
  compiled into the typed generated service is either not advertised or flagged as
  requiring redeploy — never advertised as callable while returning UNIMPLEMENTED.
- `active_call_completes_across_hot_seal` — hold a long-running call on generation N,
  activate N+1, prove the call keeps its N route/schema lease to completion, new calls
  see N+1 atomically, and N is reclaimed only after its active-call count reaches zero.
**Steps:** run compatibility + exact schema-hash validation before a blob is
activated (fail closed on mismatch); keep compiled-but-unsealed methods uncallable;
ensure dynamic reflection never advertises a method with no mounted callable route;
document that a new sealed shape needs a rebuild/redeploy to gain a typed route. Store
an immutable route/schema generation behind an atomic snapshot; each admitted call
holds a read lease/reference until its outcome audit is appended. Validate N+1 fully,
then atomically publish it for new calls; never mutate/remove N in place. Drain or
cancel under the existing deadline policy before reclaiming N, and reject activation
if the bounded old-generation limit would be exceeded.
**Verify:** `cargo test -p op-grpc-bridge --test hot_seal_bounds -- --nocapture`;
`cargo test -p op-blob --test activation_compatibility -- --nocapture`.
**DoD:** activation is hash/compat-gated fail-closed; no advertise-then-UNIMPLEMENTED;
compiled-but-unsealed stays uncallable; redeploy requirement documented.
**Deps:** T11.1, T5.2.

---

## Phase 12 — Deployment via build-golden.sh

### T12.0 — Back up and prove cognitive data/schema rollback
**Reqs:** DEP-2, DEP-5, DR-1, DR-7, DR-9, FR-3g, SEC-13
**Files:** versioned Cozo migrations from T6.2/T7.3, Qdrant collection/alias metadata,
deployment runbook in this spec directory (no data or secrets committed),
`crates/op-cognitive-mcp/tests/memory_migration.rs` (new).
**Tests first:** `memory_migration_restore_roundtrip` — migrate a v1 fixture through
v2, write/reconcile data, restore the pre-migration backup into temporary paths, and
prove the old binary/schema can read it; `migration_restart_idempotent` — interruption
at every migration step resumes safely.
**Steps:** before any schema migration or irreversible deletion, place the bridge in a
bounded maintenance/write fence and create one coordinated checkpoint manifest for:
cognitive Cozo memory/outbox/migration-export journal; durable context journal/
checkpoints; newest auth replay/OIA1/OIG1/OPA1 nonce ledger; execution audit/
idempotency intents/outcomes/response outbox/reconciliation actions; exact sealed-blob
manifest; active Qdrant collections/aliases/generations; and sanitized identity-grant
version/hash. Use supported online backup/export APIs (never copy a live database
file) into a root-only directory under `/var/lib/op-dbus/rollback/`; hash all
artifacts, record coherent high-water marks, and restore them into temporary paths/
collections for an integrity/read test. No raw grants or secrets enter the manifest.
Migrations must be additive expand/contract transactions:
create/populate/version new relations idempotently, retain v1 until final acceptance,
and never partially advance the schema marker. Qdrant remains derived: on rollback,
restore its snapshot only when hash-consistent, otherwise rebuild a fresh generation
from restored Cozo truth. While the new writer is live, append every post-cutover
memory/context/audit change to versioned backward-export journals. Rollback
forward-restores/merges the **newest** compatible auth replay ledger and never rewinds
consumed nonces; it exports/replays post-cutover data or explicitly blocks on a
non-representable record—silent loss is forbidden. Release the write fence only after
migration and smoke checks pass.
**Verify:** `cargo test -p op-cognitive-mcp --test memory_migration -- --nocapture`;
the live backup manifest hashes verify and its temporary restore passes scoped
read/reconcile checks.
**DoD:** coordinated security/data rollback is tested independently of the btrfs
binary snapshot; v1 remains through acceptance; post-cutover state has a proven
disposition; consumed nonces, deleted memories, retired listeners, and removed grants
cannot be resurrected.
**Deps:** T11.2.
**Rollback:** fence writes, stop the bridge, boot/install the prior golden binary,
restore the verified Cozo backup through the store API, restore or rebuild Qdrant,
merge the newest replay ledger, replay/verify every post-cutover export delta, verify
all saved high-water marks, then reopen traffic. If any integrity check fails,
keep traffic denied and preserve both stores for recovery.

### T12.1 — Build and deploy the golden image
**Reqs:** DEP-2, DEP-3, DEP-4, FR-1
**Steps:**
- `CXXFLAGS="-include cstdint" cargo build --workspace --release`
- `sudo deploy/runit/build-golden.sh --dry-run` (review)
- Preflight from a persistent console: confirm `10.0.0.3` is on svc0 and configured
  `${NETMAKER_MESH_IP}` (default `100.69.0.1`) is on the Netmaker interface; verify
  certificate SANs/CA; syntax-check and install the
  default-deny nftables transaction; run the release bridge temporarily on
  all three applicable addresses at port `18090` and pass TLS/OIA/reflection/MCP/
  D-Bus/browser smoke tests, then
  stop it. Abort without changing `:8090` if any preflight fails.
- `sudo deploy/runit/build-golden.sh --no-restart` to install the golden artifacts and
  runit config while the old topology still serves. Verify hashes before cutover.
- From the console, perform the port-conflict-safe cutover in this exact order:
  `sudo sv down fwd-8090 fwd-nm-mesh-8090`; verify neither relay owns
  `10.0.0.3:8090` nor `${NETMAKER_MESH_IP}:8090`; then
  `sudo sv restart op-grpc-bridge`. Do not try to start the direct-bind bridge while
  the relay still owns the address.
- Verify both canonical binds, TLS/OIA auth, firewall counters, reflection, modern MCP,
  and memory before restarting `op-web` or declaring success.
**Verify:** `sudo sv status op-grpc-bridge op-web` running;
`sudo sv status fwd-8090 fwd-nm-mesh-8090` → both down/absent;
`sudo ss -lntp 'sport = :8090'` shows only the bridge PID on `127.0.0.1`,
`10.0.0.3`, and the applicable Netmaker IP (no `python3`/`socket-relay`);
`sha256sum /usr/local/bin/op-grpc-bridge` matches the built artifact; an unapproved
source cannot cross the nftables rule.
**DoD:** deployed via golden image; artifacts match; no hand-copy; `:8090` bound only
by the bridge; relay retired.
**Deps:** T12.0.
**Rollback:** if the new bridge cannot bind/authenticate after the relay is stopped,
keep the mesh firewall default-deny and the relay down—never re-enable the deprecated
unauthenticated path. Boot/install the prior golden and restore data per T12.0 from
the console; serve loopback-only until a safe authenticated mesh path is available.

### T12.2 — Restart durability
**Reqs:** DEP-3, SEC-13
**Steps:** acknowledge a context cursor, enqueue a memory vector op, consume OIA and
approval nonces, then `sudo sv restart op-grpc-bridge`; re-run reflection + memory and
op-chat client checks; resume the context stream and reconciler; replay both nonces.
**Verify:** reflection lists the same sealed routes; identity resolution works;
authenticated memory access succeeds; a nonce accepted before restart is rejected
after restart; context resumes after the durable cursor; pending outbox rows converge;
op-chat obtains a fresh assertion and recalls through the bridge.
**DoD:** routes, context, memory/outbox, replay, and authenticated op-chat behavior all
survive restart without an in-memory fallback.
**Deps:** T12.1.

---

## Phase 13 — Live E2E acceptance and restart durability

### T13.1 — Full acceptance battery over :8090
**Reqs:** FR-1, FR-2, FR-2a, FR-3, FR-3a, FR-3b, FR-3c, FR-3d, FR-3e,
FR-3f, FR-3g, FR-4, FR-4a, FR-5, FR-5a, FR-6, FR-7, FR-8, FR-8a,
FR-8b, FR-9, FR-9a, FR-10, FR-11, FR-12, FR-13, FR-14, FR-15, FR-16,
FR-17, FR-18, FR-19; SEC-1, SEC-2, SEC-3, SEC-4, SEC-5, SEC-6, SEC-7,
SEC-8, SEC-9, SEC-10, SEC-11, SEC-12, SEC-13, SEC-14; DR-1, DR-1a, DR-2,
DR-3, DR-4, DR-5, DR-6, DR-7, DR-8, DR-9; TR-1, TR-2, TR-3, TR-4,
TR-5; DEP-1, DEP-2, DEP-3, DEP-4, DEP-5; CR-1, CR-2, CR-3, CR-4, CR-5,
CR-6, CR-7; NFR-1, NFR-2, NFR-3, NFR-4, NFR-5, NFR-6, NFR-7
**Files:**
- `crates/op-cognitive-mcp/tests/e2e_voyage_qdrant.rs` (existing, extend cleanup and
  catalog assertions);
- `crates/op-grpc-bridge/tests/unified_mcp_e2e.rs`,
  `mcp_modern_2026_07_28.rs`, and `mcp_legacy_stateful.rs`;
- `crates/op-chat/tests/two_turn_memory_e2e.rs`;
- `scripts/acceptance/btrfs_rollback_e2e.sh`;
- `tests/fixtures/unified-control-plane/{identities.json,grants.json,
  memory_v1.json,blob_manifest.json,coding_workspace/}`. Fixtures contain only
  synthetic public data—test CAs, server keys, OIA issuer keys, nonces, and approval
  keys are generated into a per-run `mktemp` directory and never committed.
**Fixture contract:** every suite creates unique identity/container/workspace/session
IDs and Qdrant collections/aliases prefixed `opdbus-e2e-<run_uuid>`, uses temporary
Cozo/auth-replay paths, seeds least-privilege suggest/apply/approve and denial
principals, and registers a cleanup guard that removes temporary collections,
aliases, databases, certificates, and workspaces on success, failure, or signal.
Live tests acquire an exclusive acceptance lock, snapshot service state, and restore
non-destructive configuration. `VOYAGE_API_KEY` and live signing authority come only
from the test secret provider/environment and are never printed.
**Verify (all must pass):**
- `sudo ss -lntp` → `:8090` bound **only by the bridge PID** on loopback, svc0, and
  the configured/present Netmaker address (no `python3`/`socket-relay`); no
  `10.200.0.2:8090`; `:3003/:50051/:50052/:11438/:11437` closed (FR-1).
- `sudo sv status fwd-8090 fwd-nm-mesh-8090` → both down/absent (FR-1).
- `curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8080/mcp/compact` → 404/410;
  `application/grpc` to `:8080` does not reach `:8090`; `/.well-known/mcp.json`,
  `/jsonrpc`, `/rpc` → 404/410 (TR-4).
- Plaintext gRPC dial to `:8090` fails; a TLS reflection dial succeeds against every
  bound address and the presented cert SANs cover them (FR-1/SEC-1).
- TLS modern MCP `tools/list`→`get_tool_schema`→safe `tools/call` succeeds under
  canonical 2026-07-28 stateless negotiation without `Mcp-Session-Id`; the separate
  legacy initialize/session fixture succeeds only with fresh OIA1 assertion material
  on every request; a session id alone is rejected (FR-2a/SEC-2).
- real-browser exact Origin/CORS matrix passes; its authenticated session/CSRF OIA
  broker issues one no-store target-bound assertion per call, replay is rejected,
  static headers/credentialed redirects fail, and native gRPC needs no Origin
  (FR-2a/SEC-14).
- unauthenticated route enumeration exposes exactly `GET /healthz` and
  `POST /genesis/complete`; valid source-bound signed OIG1 anchors once, all negative
  genesis/enumeration cases fail, and onboarding cannot discover/execute (FR-18).
- `plugin_schema.dat` absent from runtime/code/deploy/docs (`find /dev/shm/opdbus -name plugin_schema.dat` empty; `rg` clean) — DR-2.
- TCP vs host-UDS vs container-UDS return equivalent results for the same identity; a
  forged/absent assertion and every plaintext protocol over UDS are rejected;
  TLS certificate validation succeeds; `stat -c '%a' /run/opdbus/grpc.sock` is not
  `666` (TR-2/TR-5).
- unauth/fake/expired/inactive/replayed/unauthorized fail before dispatch; a
  caller-supplied `x-opdbus-capability` header alone is denied; a method with no
  resolvable grant is denied (SEC-2/SEC-3/FR-4a).
- identity_sled is the sole grant authority; no `"*"`/sentinel grant and no non-empty
  authoritative sealed `PluginSchema.capability_grants` survives (FR-4a/SEC-3).
- a caller cannot forge/override any ExecutionContext field or scope
  (identity/container/namespace/workspace/collection/session/path); a coding traversal/
  symlink/oversized-archive request is rejected (FR-3d/FR-3f).
- a tool returning schema-violating output errors and creates no data/vector/prompt
  artifact, but does append a redacted correlated intent/failure outcome (FR-3e/SEC-9).
- idempotent transport retry/restart executes once and returns its committed outbox;
  changed binding/non-idempotent reuse is denied; cancellation races classify
  pre-commit/committed/partial effects and reconcile without false rollback (FR-3g).
- every reflected RPC callable AND every mounted method reflected (both directions);
  an incompatible-hash blob activation is rejected; a compiled-but-unsealed method is
  uncallable; a long active call completes on its leased old generation while new
  calls atomically see the new generation (FR-9); authenticated reflection and MCP
  return the full sanitized public catalog, ordinary callers cannot read raw schema,
  and execution remains
  capability-gated (FR-9a).
- `blob_catalog(mode=full)` covers one immutable manifest generation through bounded
  snapshot cursors; `blob://` resource
  + per-plugin schema tools work; with two blobs for one plugin the manifest-pinned
  hash is served; Qdrant active-generation `catalog_hash`, count, and point payloads
  exactly match the sorted manifest and removed plugins have no points (DR-8).
- real `code_search`/`code_context`/`code_index` called.
- a suggestion returns without applying; applying requires the apply capability AND a
  valid signed single-use approval from an active principal with `coding.approve`,
  separation-of-duty policy passes, and evidence/base/diff/verification contracts are
  exact; unauthorized/revoked/mismatched/expired/reused approval is denied; retry is
  idempotent and cannot apply twice (FR-12/SEC-10).
- a recorded context event changes a later retrieval; a user correction /
  accepted/rejected suggestion changes a later ranked suggestion (CR-2); an idle
  session fires exactly one recovery per episode; after a full process restart a
  context stream resumes after its durable cursor (FR-19).
- memory isolated by identity/container/workspace; stored injection not treated as
  instruction; non-curated memory injected as delimited untrusted data (SEC-6/7/12);
  a credential pattern is redacted in Cozo/Qdrant/log/event/prompt (SEC-11).
- Voyage/Qdrant down → durable memory still works, auth enforced (DR-4); killing the
  bridge mid-reconcile and restarting converges Cozo↔Qdrant with no lost/duplicated
  points; a deleted memory is not returned as a tombstone (DR-7).
- one Cozo writer (DR-1); op-chat uses the authenticated bridge client on both unary
  and streaming paths; durable Cozo locked → memory write returns `Unavailable`, no
  in-memory/local-chat fallback (DR-1a).
- equal memory record keys in different tenant scopes have distinct deterministic
  Qdrant point IDs; migrations and retry idempotency survive restart.
- every admitted mutation/tool call records a correlated intent/outcome pair with
  `event_id`+`event_hash`; denials/invalid/cancelled/timeouts/failures have redacted
  outcomes; pre-auth rejections
  recorded as pre-auth telemetry (not actor-attributed); a nonce is rejected after a
  bridge restart and across TCP↔UDS (SEC-9/SEC-13/DR-5).
- restart the bridge → sealed routes, identity projections, memory access restored.
- `[manual]` deployed binary hashes/timestamps match built artifacts (DEP-4).
**Required E2E suites (CR-7) — each a release blocker:**
- Voyage/Qdrant semantic:
  `CXXFLAGS="-include cstdint" cargo test -p op-cognitive-mcp --test e2e_voyage_qdrant -- --nocapture`.
- authenticated modern + legacy `:8090` (TLS + fresh OIA1 through discovery/
  execution):
  `CXXFLAGS="-include cstdint" cargo test -p op-grpc-bridge --test unified_mcp_e2e -- --nocapture`;
  `CXXFLAGS="-include cstdint" cargo test -p op-grpc-bridge --test mcp_modern_2026_07_28 -- --nocapture`;
  `CXXFLAGS="-include cstdint" cargo test -p op-grpc-bridge --test mcp_legacy_stateful -- --nocapture`.
- two-turn chatbot memory (turn 1 stores; restart bridge; turn 2 recalls with
  provenance and cross-identity isolation):
  `CXXFLAGS="-include cstdint" cargo test -p op-chat --test two_turn_memory_e2e -- --nocapture`.
- Btrfs binary plus cognitive-data rollback (requires the exclusive live-host lock
  and console): `sudo scripts/acceptance/btrfs_rollback_e2e.sh --dry-run`, then
  `sudo scripts/acceptance/btrfs_rollback_e2e.sh --confirm-host "$(hostname)"`.
**Acceptance artifacts:** each command writes JUnit/log output plus a redacted fixture
manifest, binary/schema/catalog hashes, Qdrant cleanup result, and before/after service
state under a unique `target/acceptance/<run_uuid>/`; CI uploads that directory and
fails if cleanup or any assertion fails.
**DoD:** the battery and all four required E2E suites pass on the deployed host.
**Deps:** T12.2.

---

## Phase 14 — Zero-trace CI gates

### T14.1 — Wire and enforce the grep gates
**Reqs:** NFR-7, FR-1, FR-3g, FR-4 (scoped), FR-4a, FR-8b, SEC-14,
TR-4, DR-8, DEP-5, NFR-2 (scoped)
**Files:** `scripts/ci/zero-trace-gates.sh`, CI config.
**Steps:** implement the gate to fail on active (non-removal, non-negative-test)
occurrences of:
- `plugin_schema.dat`;
- `10.200.0.2:3003`/`:3003` MCP; MCP `:50051`/`:50052`; `:11438`/`:11437`;
- direct `op-web` MCP execution routes (`/mcp`, `/mcp/compact`, `/mcp/agents*`);
- standalone cognitive/Waypipe listeners; retired `op-mcp-*` runit services with a
  network bind; bridge HTTP loopback (`cognitive_mcp_endpoint`, POST `/mcp`);
- sentinel/wildcard/embedded identity-grant authority: any `"*"` grant in active
  sources or any non-empty authoritative sealed `PluginSchema.capability_grants`
  (including the `tched_router.rs` insert); required capability names remain allowed;
- the `op-web` alternate gRPC ingress: `grpc_proxy` forwarding `application/grpc*` to
  `:8090`, and the `/jsonrpc`, `/rpc`, `/.well-known/mcp.json`, `mcp_discovery`,
  `mcp_smart_router` aliases (TR-4);
- a Python `socket-relay`/`tcpfwd` fronting `:8090` (`fwd-8090` or
  `fwd-nm-mesh-8090`), any `:8090` bind owned by a non-bridge process, or any
  non-canonical bridge bind such as `10.200.0.2:8090` (FR-1);
- caller-supplied capability header treated as authority / the degraded
  `identity.is_some() && capability_matches` allow path (FR-4a);
- first-prefix blob resolution instead of exact schema-hash (DR-8);
- raw sealed schema/grant/secret fields reaching ordinary reflection, blob,
  resource, vector, cache, or log surfaces instead of the FR-8b projection;
- any externally reachable MCP/generated adapter that directly calls
  `ToolRegistry::execute` instead of `PluginService.CallMethod → D-Bus →
  MutationEngine`, and any hand-authored cognitive protobuf/method schema that is not
  derived/checked against the authoritative `PluginSchema`;
- committed unredacted baseline grants, assertions, signing keys, or service
  environments; plaintext UDS configuration; or a runit `:8090` bind set that omits
  svc0/Netmaker-interface/SAN/firewall startup validation; wildcard/reflected/`null`
  browser Origin configuration or logging/redirect forwarding of OIA/approval headers.
Scope the second-`ToolRegistry` gate to production execution ownership and the polling
gate to periodic state-discovery only (allow tests, isolated libraries,
deadline/heartbeat timers).
**Verify:** the gate exits non-zero on a tree with an injected forbidden token
(one self-test per pattern above) and zero on the cleaned tree; run against the live
tree in CI.
**DoD:** CI gate green on the cleaned tree; self-test proves it catches every listed
regression class (embedded sealed grants, op-web grpc_proxy/jsonrpc/rpc/well-known,
Python socket-relay fronting `:8090`, caller-supplied-capability authority + degraded
allow-path, first-prefix blob resolution, parallel non-D-Bus execution, schema drift,
raw schema projection leakage, unsafe browser Origin policy, plaintext UDS, and
secret-bearing baseline artifacts).
**Deps:** T13.1.

---

## Dependency summary

```
0 → 1 → 2 → 3 → 4 → 5 ┐
                 4 → 6 ┤
        2 → 7 → 8 ─────┤
                 4 → 9 ┴→ 10 → 11 → 12 → 13 → 14
```

Phase 1 detail (auth/route stack + ingress topology):
```
T1.1 → T1.2 → T1.3 → T1.6            (three-address direct-bind; both relay retirements execute in Phase 12)
        T1.2 → T1.4                  (ExecutionContext)
        T1.2 → T1.5                  (durable cross-transport replay; pre-auth telemetry; rate limits)
```

Phase 4 detail (descriptor → validation → adapter):
```
T4.1 → T4.2 → ┐
T4.1 → T4.3   ┤→ T4.5   (MCP adapter conformance; needs T1.1 + T1.5)
T4.1 → T4.4 → ┘
```

Phase 9 detail (TR-4 ordering — dashboard repoint precedes proxy delete):
```
T9.1 (dashboard → :8090 gRPC-Web, browser E2E PASS)  →  T9.2 (delete grpc_proxy + MCP aliases)
```

Other new-task ordering:
- FR-1 direct-bind lands in Phase 1 (T1.6); the `fwd-8090` relay retirement executes
  in Phase 10 (T10.3, repo) and Phase 12 (T12.1, live console action).
- DR-8 exact-hash resolution (T5.2) precedes the hot-seal hash gate (T11.2).
- DR-7 outbox (T7.3) precedes lifecycle/consolidation (T8.2) and SEC-10 approval (T8.3).
- T12.0 proves data/schema rollback before T12.1 stages the golden; T12.1 stops the
  relay before restarting the direct-bind bridge, and a failure keeps the relay down.

Irreversible boundary: Phase 10. Everything before it is git-revertible; Phase 10+
rolls binaries/configuration back via the prior btrfs golden and cognitive data via
T12.0. Auth replay history is preserved and deprecated unauthenticated ingress is
never restored.
