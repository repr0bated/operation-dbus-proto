# Standalone EMQX Identity MCP — Tasks

## Overview

This document defines the phased implementation plan for integrating EMQX as a standalone local service into the OP-DBUS identity pipeline.

**Topology Update:** `10.0.0.3:8090` is the unified fabric surface itself (MCP,
gRPC/gRPC-Web, plugins, mutation, and control); its sole MCP URL is
`https://10.0.0.3:8090/mcp` (supersedes `10.0.0.2`).

**Estimated Total Effort:** 9-12 days  
**Risk Level:** Medium-High (security-critical identity changes)

All Cargo build/test commands in every phase use `CXXFLAGS="-include cstdint"`, even
when an individual command below omits the prefix for readability.

---

## Phase 0: Preparation and Validation

**Duration:** 0.5 day  
**Objective:** Verify prerequisites, preserve existing work, establish baseline

### Task 0.1: Preserve Uncommitted Work
- [ ] Verify git status in all repositories
- [ ] Inventory and preserve all existing uncommitted changes in:
  - `/srv/git/odbus`
  - `/srv/git/emqx`
  - `/srv/git/operation-dashboard-ui-07`
  - `/srv/git/json-render`
- [ ] Do not stash, commit, reset, discard, or overwrite user work without explicit authorization
- [ ] Record overlapping files and work around them

**Validation:** Baseline `git status --short` is recorded and every pre-existing change remains intact

### Task 0.2: Verify EMQX Installation
- [ ] Check if EMQX is installed: `which emqx`
- [ ] Verify EMQX version: `emqx version`
- [ ] Inspect the available EMQX checkout/package; do not assume the example `5.8.0` pin
- [ ] Record edition/license and choose a supported production version
- [ ] Prove the retained loopback MQTT/ExHook integration is compatible with that exact version and record that no MCP Gateway/listener is installed
- [ ] Run a real ExHook target compatibility probe and choose UDS only if its URI scheme works; otherwise choose loopback mTLS
- [ ] Identify a deterministic offline artifact path and record full checksums

**Validation:** A compatibility decision record contains version, edition/license, artifact, checksum, no-MCP-gateway decision, and ExHook/MQTT transport evidence

### Task 0.3: Verify Topology Lock Documentation
- [x] Edit `.kiro/specs/README.md`
- [x] Update: `gRPC at 10.0.0.2:8090` → unified fabric surface at `10.0.0.3:8090` carrying MCP, gRPC, plugins, mutation, and control capabilities
- [x] Add note referencing this spec as the active overlay
- [ ] Scan active deployment/client configuration for stale `10.0.0.2:8090` values before implementation

**Validation:** README identifies `10.0.0.3:8090` as the unified fabric surface itself, with EMQX internal behind it

### Task 0.4: Baseline Test Run
- [ ] Run targeted plugin tests: `CXXFLAGS="-include cstdint" cargo test -p op-plugins --lib emqx`
- [ ] Run targeted bridge tests: `CXXFLAGS="-include cstdint" cargo test -p op-grpc-bridge --lib emqx -- --test-threads=1`
- [ ] Record broader pre-existing compile/test failures separately; do not weaken or rewrite unrelated tests
- [ ] Document current test state

**Validation:** Existing tests pass or failures documented

---

## Phase 1: Plugin Schema Refactoring

**Duration:** 1.5 days  
**Objective:** Refactor emqx.rs to standalone state, add all typed methods
**Dependencies:** Phase 0 complete

### Task 1.1: Remove Netmaker References
- [ ] Edit `crates/op-plugins/src/state_plugins/emqx.rs`
- [ ] Remove ALL Netmaker-specific constants
- [ ] Remove ALL Netmaker-specific fields from `EmqxState`
- [ ] Update plugin description
- [ ] Bump version to "2.0.0"
- [ ] Use `rg` across the relevant code/config for remaining "NetMaker"/"Netmaker" references

**Validation:** `rg -i netmaker crates/op-plugins/src/state_plugins/emqx.rs` returns empty

### Task 1.2: Add Standalone State Types
- [ ] Add loopback-only constants (NO public bindings)
- [ ] Add `EmqxListener` with `is_loopback` validation field
- [ ] Add `EmqxExhookServer` with `is_local` validation field
- [ ] Add `EmqxFabricStatus` with `internal_transport_only` field
- [ ] Refactor `EmqxState` with standalone fields
- [ ] Update OSCAL schema subid to `@v2`

**Validation:** Types compile with JsonSchema derive

### Task 1.3: Add Method I/O Types
- [ ] Add lifecycle outputs with rich results (PID, uptime, status)
- [ ] Add all input/output types per design.md
- [ ] Verify no AckOutput for lifecycle methods
- [ ] AckOutput acceptable for simple confirmations only

**Validation:** All I/O types compile

### Task 1.4: Register Methods in Schema
- [ ] Update `emqx_schema()` function
- [ ] Register all 9 methods
- [ ] Add capability declarations
- [ ] Verify SideEffect correctness
- [ ] Verify idempotent flags

**Validation:** `emqx_schema().methods.len() == 9`

### Task 1.5: CI Subid Uniqueness Test
- [ ] Extend the existing global uniqueness gate in `crates/op-plugins/src/default_registry.rs`; do not add a duplicate partial gate
- [ ] Test collects ALL plugin schema subids and method subids
- [ ] Test fails on duplicates
- [ ] Run: `CXXFLAGS="-include cstdint" cargo test -p op-plugins --lib subid_uniqueness`

**Validation:** CI test passes, no duplicate subids

### Task 1.6: Unit Tests for Schema
- [ ] Test: no Netmaker fields in schema
- [ ] Test: all methods present
- [ ] Test: all methods have typed I/O
- [ ] Test: capabilities registered
- [ ] Test: listeners all loopback/UDS
- [ ] Test: repeated schema/default/example generation is byte-for-byte deterministic with EMQX up or down
- [ ] Test: schema construction performs no socket, filesystem-state, `sv`, or EMQX API probe
- [ ] Test: credentials, OIA bytes, and MQTT secrets never appear in schema/state/examples

**Validation:** All unit tests pass

---

## Phase 2: ExHook Security Hardening

**Duration:** 1 day  
**Objective:** Move ExHook to dedicated authenticated local listener
**Dependencies:** Phase 0 complete (can run parallel to Phase 1)

### Task 2.1: Lock the Supported ExHook Transport
- [ ] Exercise the pinned EMQX release against a minimal real HookProvider
- [ ] Record supported target URI schemes and TLS options
- [ ] Select dedicated UDS only if proven; otherwise select loopback-only mTLS
- [ ] Fail deployment if configuration requests an unproven transport

**Validation:** Compatibility test connects and invokes a hook with the exact pinned package

### Task 2.2: Create the Dedicated ExHook Listener
- [ ] Add a listener for the selected transport; never mount it on client-facing `:8090`
- [ ] For UDS: bind `/run/opdbus/emqx-exhook.sock`, set parent traversal policy, and install `0660 root:emqx`
- [ ] For UDS: wrap accepted `UnixStream`s with tonic `Connected` metadata populated from `SO_PEERCRED`
- [ ] For loopback: bind only `127.0.0.1` and require mutually authenticated TLS with a dedicated EMQX identity
- [ ] Add bounded request size, deadline, concurrency, and shutdown behavior

**Validation:** Selected listener compiles and cannot be reached through the shared route

### Task 2.3: Implement Local Service Identity Verification
- [ ] For UDS, verify UID/GID/PID, process start time, and executable/cgroup provenance only from accept-time request extensions
- [ ] Reject attempts to supply/override peer identity through gRPC metadata or request fields
- [ ] For loopback, validate and pin the dedicated EMQX client certificate
- [ ] Reject UID-only, uncredentialed, stale-PID, wrong-process, and non-local callers

**Validation:** Legitimate pinned EMQX connects; forged local and remote calls are rejected

### Task 2.4: Update grpc_server.rs and EMQX Configuration
- [ ] Remove ExHook from shared route (line ~867)
- [ ] Start dedicated ExHook listener in server startup
- [ ] Ensure ExHook is NOT accessible on client-facing `:8090`
- [ ] Render the proven target scheme into `deploy/config/emqx.conf.template`
- [ ] Add anonymous MQTT denial; deny `$mcp-*` outright; ACL only the internal `$opdbus/fabric/*` event namespace to pinned local service identities

**Validation:** ExHook not in `build_operation_routes`

### Task 2.5: ExHook Security Tests
- [ ] Test: forged ExHook calls rejected
- [ ] Test: non-local connections rejected
- [ ] Test: unauthorized UID/certificate and same-UID wrong process rejected
- [ ] Test: PID reuse/start-time mismatch rejected
- [ ] Test: legitimate EMQX calls accepted
- [ ] Test: pinned-package config loads and reconnects after bridge restart

**Validation:** Security tests pass

---

## Phase 3: Dispatch Implementation

**Duration:** 1 day  
**Objective:** Implement method dispatch and wire into MutationEngine
**Dependencies:** Phase 1 complete

### Task 3.1: Implement Backend Query Functions
- [ ] Add `query_emqx_status()` — authenticated EMQX API call over loopback only, with bounded connect/request timeouts and redacted errors
- [ ] Add `query_fabric_status()` — reports loopback MQTT and authenticated ExHook attachment behind the unified fabric surface; exposes no MCP gateway
- [ ] Add `query_listeners()` — validates all loopback/UDS
- [ ] Add `query_exhook_servers()` — validates all local
- [ ] Add `check_emqx_runit_service()` — fixed EMQX status query, with no caller-selected service name

**Validation:** Functions return expected types

### Task 3.2: Implement Lifecycle Delegation
- [ ] Add D-Bus service manager delegation (NOT shell)
- [ ] Hard-code the delegated service target to `emqx`
- [ ] Implement `lifecycle_start()` with rich output
- [ ] Implement `lifecycle_stop()` with confirmation
- [ ] Implement `lifecycle_restart()` with uptime tracking

**Validation:** Lifecycle methods delegate, don't shell out

### Task 3.3: Implement Main Dispatch
- [ ] Update `dispatch_emqx_method(method, args)`
- [ ] Handle all 9 methods
- [ ] Return errors for unknown methods
- [ ] For `configure_hooks`, enforce the hook-name allowlist and closed transport enum
- [ ] Implement validate → temporary render → atomic replace → authorized reload → health check, with rollback on failure
- [ ] Redact API credentials, certificates, MQTT credentials, and OIA material from all results/errors/audit

**Validation:** Dispatch handles all methods

### Task 3.4: Wire into MutationEngine
- [ ] Edit `mutation_engine.rs`
- [ ] Add `"emqx"` match arm BEFORE fallback
- [ ] Verify no echo fallback

**Validation:** EMQX methods execute, don't echo

### Task 3.5: Dispatch Tests
- [ ] Test: each method returns typed output
- [ ] Test: unknown method fails
- [ ] Test: methods route through MutationEngine
- [ ] Test: lifecycle input cannot target another runit service
- [ ] Test: arbitrary hook URL/name is rejected and failed reload restores prior config
- [ ] Test: API timeout and authentication failure return typed, redacted errors

**Validation:** Dispatch tests pass

---

## Phase 4: D-Bus Identity Resolution

**Duration:** 1.5 days  
**Objective:** Implement D-Bus sender → registered principal/session binding without using footprint as identity
**Dependencies:** Phase 3 complete

### Task 4.1: Create DbusIdentityResolver
- [ ] Implement `DbusIdentityResolver` struct
- [ ] Add binding storage with unique-name key
- [ ] Read unique-name and UID/GID/PID from the D-Bus message/bus credentials, not arguments
- [ ] Add PID plus `/proc/<pid>/stat` process-start-time tracking
- [ ] Add registration time tracking
- [ ] Add a capability-gated `register_current_sender` operation that resolves `principal_id` and current session from an already-authenticated session/OIA proof
- [ ] Never accept caller-supplied `principal_id`, footprint, genesis, target unique-name, UID, or PID

**Validation:** Resolver compiles

### Task 4.2: Implement Process-Reuse Protection
- [ ] Verify unique owner, bus credentials, PID, and process start time on each resolution
- [ ] Revoke binding if any binding component or authoritative session differs
- [ ] Handle process exit and D-Bus `NameOwnerChanged`

**Validation:** Recycled PID doesn't inherit identity

### Task 4.3: Implement Revocation
- [ ] Monitor for D-Bus NameOwnerChanged
- [ ] Revoke on unique-name disappearance
- [ ] Make explicit logout a current-sender operation; it cannot name another sender

**Validation:** Bindings revoked on exit

### Task 4.4: Integrate with SchemaBackedInterface
- [ ] Update `call()` method in schema_router.rs
- [ ] Inject the supported zbus message header (`#[zbus(header)]` for the pinned zbus version); do not invent `self.message_sender()`
- [ ] Resolve sender identity before dispatch
- [ ] Fail closed if no binding
- [ ] Pass identity to `dispatch_call_for_identity`

**Validation:** D-Bus calls require valid binding

### Task 4.5: D-Bus Identity Tests
- [ ] Test: unregistered sender denied
- [ ] Test: registered sender allowed
- [ ] Test: process reuse denied
- [ ] Test: revocation on exit
- [ ] Test: concurrent sender handling
- [ ] Test: self-chosen principal/footprint and binding another unique-name are denied
- [ ] Test: one sender cannot log out another sender
- [ ] Test: stale authoritative session and PID-start-time mismatch are denied

**Validation:** D-Bus identity tests pass

### Task 4.6: Remove Identity/Footprint Overload and Hash-Key Authorization
- [ ] Replace auth-facing `GhostbridgeIdentity.footprint` with explicit `principal_id`, `session_id`, and `session_genesis` fields
- [ ] Remove `HUMAN_FOOTPRINT_KDF_CONTEXT`, `derive_human_footprint`, and `HumanPrincipalIdentity.footprint`; do not replace them with a different hash of genesis/event/footprint data
- [ ] Replace `AuthenticatedCaller.footprint`, `load_exact_capability_grants(footprint)`, and `capabilities_for_footprint` with registered-`principal_id` equivalents
- [ ] Make the protected principal-grant projection the grant authority; identity-sled supplies session/genesis context but does not assign grants
- [ ] Migrate known legacy grant entries by resolving their owning registered principal from authoritative records; reject ambiguous/unmapped 64-hex keys and reject `"*"`
- [ ] Preserve `x-ghostbridge-footprint` only if the `session-genesis-identity` transition still requires it as a genesis alias; never route that value into grants/audience
- [ ] Add a semantic CI gate forbidding fields/functions named identity footprint in auth code and forbidding footprint/genesis/hash inputs to principal derivation or grant lookup

**Validation:** One stable `principal_id` drives authn/authz across gRPC, MCP, and D-Bus; changing session/genesis/payload hashes leaves identity and grants unchanged

### Task 4.7: Preserve A.N.N.A. Scribe; Remove Only the Unsafe Duplicate Reader
- [ ] Preserve the A.N.N.A. Scribe name/persona and its established email identity exactly
- [ ] Remove only the direct `File::open("/dev/shm/plugin_schema.dat")`/unsafe mmap reader and duplicate genesis/identity derivation from the Scribe path
- [ ] Route any Scribe identity read through the canonical selected-sled projection; do not make Scribe another SID1, genesis, footprint, or chain-hash author
- [ ] If the whole Scribe surface was deleted by an earlier migration, restore its persona/email contract without restoring the reader
- [ ] Add preservation tests for the persona/email and a negative grep/test for the mmap/path

**Validation:** A.N.N.A. Scribe's public identity is unchanged and no duplicate mmap identity reader remains

---

## Phase 5: Deployment Artifacts

**Duration:** 0.5 day  
**Objective:** Create tracked deployment artifacts with provenance
**Dependencies:** Phase 0 complete (can run parallel)

### Task 5.1: Create Runit Service Definition
- [ ] Create `deploy/runit/emqx/run`
- [ ] Configure loopback-only listeners
- [ ] Set environment variables
- [ ] Make executable

**Validation:** Run script is valid shell

### Task 5.2: Create Version Pinning Files
- [ ] Create `deploy/config/emqx.version`
- [ ] Add the Phase-0-selected VERSION, edition/license record, SHA256, deterministic offline ARTIFACT path, and provenance-only SOURCE_URL
- [ ] If a separate ExHook/MQTT integration artifact is retained, create `deploy/config/emqx-integration.version`
- [ ] Pin that internal integration only after exact-version compatibility is proven; install no MCP Gateway artifact
- [ ] Add checksum files and verify the offline artifacts exist before deployment

**Validation:** Version files exist with checksums

### Task 5.3: Update build-golden.sh
- [ ] Keep `build-golden.sh` POSIX `/bin/sh`; do not add Bash-only `local` or `[[ ... ]]`
- [ ] Add `emqx` to existing `deploy/runit/managed-services` and `enabled-services` manifests
- [ ] Resolve and checksum the pinned offline artifact before changing golden or live targets
- [ ] Stage the same verified artifact into golden and live paths through existing script helpers
- [ ] Add data/runtime subvolume separation
- [ ] Do not create a custom service symlink path or verify an unrelated pre-existing `/usr/bin/emqx`

**Validation:** `--dry-run`, `--golden-only`, and `--live-only` use the same pin/checksum and existing managed-service logic

### Task 5.4: Verify No Public Bindings
- [ ] Scan EMQX configuration for public bindings
- [ ] Verify all listeners are loopback/UDS
- [ ] Add deployment-time validation

**Validation:** No `0.0.0.0` or public IP bindings

---

## Phase 6: SID1 Header Integration and Optional OIA1

**Duration:** 1.5 days  
**Objective:** Make the selected local identity sled work end to end with native Codex header injection while retaining OIA1 as an optional credential for other callers
**Dependencies:** Phase 4 complete

### Task 6.1: Make MutationEngine the One SID1 Author
- [ ] Define the bounded deterministic `SID1` session envelope separately from OPBLOB01, PluginFootprint, and Snowball receipt types
- [ ] After genesis mint/recovery, have `MutationEngine` seal the immutable principal/session/WireGuard/genesis/trace/schema/arrival/catalog/transport claims exactly once
- [ ] Store the inline value as `sid1:<canonical-unpadded-base64url>` in that selected session sled's existing `sealed_id`
- [ ] Publish the full sled only to the root-only durable identity store and private `root:secrets` tmpfs credential projection; publish a separately serialized `sealed_id`-free public identity state
- [ ] On migration, permit MutationEngine to populate a missing blob for an already immutable anchor; no other writer may create or replace it
- [ ] Prove the SID1 integrity hash is not used as identity, grant key, footprint, vector payload, or chain input

**Validation:** deterministic round-trip/tamper/trailing-byte tests pass and one anchored sled contains exactly the MutationEngine-authored SID1

### Task 6.2: Implement the Header-Only Reader and Direct Codex Configuration
- [ ] Implement `op-identity-headers` as a short-lived binary that accepts at most one bounded `--session` selector and reads only the private credential projection without mmap or public/legacy fallback
- [ ] Require the selected sled to be active, anchored, current, unambiguous, and internally consistent; validate its SID1 before forwarding the exact encoded bytes
- [ ] Emit exactly `{"x-opdbus-sealed-id-bin":"<canonical-SID1>"}` plus newline on stdout; send only redacted diagnostics to stderr and exit
- [ ] Open no listener, implement no MCP protocol/transport, and add no shim, proxy, broker, daemon, or OAuth support
- [ ] Configure project Codex with direct `url = "https://10.0.0.3:8090/mcp"` and `http_headers_helper = "/usr/local/bin/op-identity-headers --session <selected-session-id>"`; use no stdio `command`, bearer/static header, or OAuth login
- [ ] Pin/record the deployed Codex version and prove its local config schema recognizes `http_headers_helper`; a client version that ignores or lacks the field blocks deployment instead of selecting a fallback
- [ ] In the same bridge router, accept native Codex `initialize` and `notifications/initialized` compatibility messages without issuing authorization state; derive method/target from the parsed JSON-RPC body instead of requiring custom `Mcp-Method`/`Mcp-Name` headers, and follow standard `MCP-Protocol-Version` behavior

**Validation:** a real Codex process invokes the helper for native Streamable HTTP startup/list/call and reaches the sole `:8090` surface directly; process/listener tracing shows no intermediary

### Task 6.3: Exact-Match SID1 and Retain Optional OIA1
- [ ] Accept exactly one credential per request: local selected-sled SID1 or fresh OIA1, never both
- [ ] Decode canonical SID1, verify its seal/transport scope/derived identifiers, and exact-match its encoded bytes plus every claim against the authoritative active, anchored, unexpired sled
- [ ] Check principal revocation and load capabilities only from the exact registered `principal_id`; SID1/genesis/footprint/hash values never key grants
- [ ] Keep the direct OIA1 path for other callers, including signature/time/source/target validation and durable one-use nonce consumption
- [ ] Keep EMQX on loopback MQTT/ExHook for authenticated health/catalog/lifecycle events only; it never forwards MCP or carries either credential

**Validation:** SID1 and OIA1 resolve the same principal/grants/MutationEngine path, while a dual-credential request and every non-exact match fail before discovery

### Task 6.4: Verify Credential Lifecycle and Failure Matrix
- [ ] Test: the same exact SID1 is accepted on two requests in one active immutable session term and is not reminted on retry
- [ ] Test: missing, duplicate, padded/non-canonical, oversized, malformed, tampered, wrong-transport, projection-drifted, inactive, expired, revoked, and no-exact-grant SID1 are denied
- [ ] Test: absent/ambiguous helper selection fails closed and no blob/header bytes reach logs, MQTT, config dumps, or audit payloads
- [ ] Test: public projection, D-Bus reads, StateSync get/subscribe, snapshots, method results, Snowball/vector payloads, schemas, and web responses contain neither `sealed_id` nor an `sid1:` value; the private file retains exact bytes with `0640 root:secrets`
- [ ] Test: replayed OIA1 is denied; retry with a newly minted OIA1 succeeds; concurrent/cross-restart replay and source/target substitution fail
- [ ] Test: native Codex initialize/list/call succeeds without custom method/name headers and every protected message contains exactly one credential
- [ ] Test: OAuth metadata/token/callback routes and MCP shim/proxy listeners are absent

**Validation:** real-client and protocol-level SID1/OIA1 lifecycle tests pass on the one unified fabric surface

---

## Phase 7: MCP Audience, HOT Path, and Tool Sets

**Duration:** 2 days  
**Objective:** Keep one unified fabric surface while giving the singleton chatbot exactly five typed HOT tools and every authorized identity one-at-a-time typed WARM/COLD drill-down through `toolsets`
**Dependencies:** Phases 1, 3, 4, and 6 complete

### Task 7.1: Resolve the Canonical Spec Visibility Conflict
- [x] Add the exact supersedence rule to the canonical control-plane requirements/design/tasks: the four compact meta-tools are removed from the active MCP surface and `toolsets` replaces their lazy loader
- [x] Preserve direct typed target-schema validation, exact target capability, single MutationEngine entry, and inline audit
- [x] State that modes/tool sets are projections of one registry and one endpoint, not services

**Validation:** No active requirement still mandates compact visibility for every authenticated caller

### Task 7.2: Implement Server-Side Audience Projection
- [ ] Create/load protected `deploy/config/mcp-audience-policy.json` with exactly one validated `singleton_chatbot_principal_id`, version, and rotation epoch
- [ ] Migrate current wildcard/digest-keyed capability grants to registered exact `principal_id` entries and make production grant loading reject wildcard, footprint, genesis, and hash identity keys
- [ ] Add an audited two-step singleton rotation procedure
- [ ] Derive audience only from the SID1/OIA1-resolved registered `principal_id`
- [ ] Ignore `clientInfo`, model name, user-agent, headers, MQTT client ID, and self-declared roles for authority
- [ ] Remove `list_tools`, `search_tools`, `get_tool_schema`, `execute_tool`, and compatibility aliases from active discovery and deny them by name for every principal

**Validation:** Singleton receives exactly the five HOT tools; it, Grok, and a second identity receive zero compact/generic tools and direct-call denial; wildcard grant/startup fixtures fail closed

### Task 7.3: Implement the Five Typed HOT Tools
- [ ] Inventory canonical registry/generated names and legacy aliases for memory/workflow operations; record every collision before choosing mappings
- [ ] Define stable typed projection facades `memory_recall`, `memory_store`, `workflow_query`, `workflow_run`, and `toolsets` over existing canonical implementations where available
- [ ] Register/pre-warm the facades in the single bridge-owned runtime without constructing a second registry, memory store, or workflow engine
- [ ] Fail catalog activation on unresolved canonical/legacy alias collisions and prove every facade crosses canonical dispatch exactly once
- [ ] Provision the configured singleton chatbot `principal_id` with exact capabilities for those five tools
- [ ] Keep durable memory/workflow stores authoritative; caches are accelerators only
- [ ] Ensure no HOT call performs a synchronous EMQX/MQTT/provider-discovery hop after admission
- [ ] Measure Phase-0 and post-change p50/p95 admission-to-dispatch latency and set the rollout budget

**Validation:** The singleton chatbot's default `tools/list` is exactly those five, and all execute with EMQX stopped

### Task 7.4: Implement Stateless Tool-Set Selection
- [ ] Add `deploy/config/mcp-toolsets.json` with generation, exact typed membership, temperature, provider identity, and health policy—but no grants
- [ ] Make `toolsets` list only sets intersecting the caller's exact grants
- [ ] Permit exactly one selected WARM/COLD set at a time; selecting another replaces rather than unions with the previous set
- [ ] Return a short-lived, signed, principal/client-bound, catalog-generation-bound projection handle (or equivalently validated canonical `_meta` selector)
- [ ] Project `tools/list` to `HOT ∪ selected typed set`, intersected with audience, exact grants, and provider health
- [ ] Re-check the projection on `tools/call`; guessed hidden names fail
- [ ] Emit `notifications/tools/list_changed` only when negotiated and give other clients an explicit re-list response
- [ ] Drive the real installed Codex and Grok clients: prove the MCP host captures the selector, re-lists with `_meta`, installs returned typed schemas, and carries the selector on `tools/call`
- [ ] If either client lacks selector-aware re-list support, keep its visible/callable catalog exactly HOT five and record drill-down as unsupported; do not treat returned schemas as tool registration

**Validation:** Authorized selection narrows correctly; forged, stale, cross-principal, unknown, and unhealthy selections never expose the full inventory; typed drill-down is claimed only when a real MCP host re-list proves callability

### Task 7.5: Keep Generic Execution Out of External Sets
- [ ] Ensure no external tool-set manifest contains `invoke_tool`, `execute_tool`, or compact discovery tools
- [ ] Return actual typed generated/registry tools after selection
- [ ] Add an `emqx_ops` fixture set whose visible methods are limited by exact EMQX capabilities
- [ ] Verify tool-set selection cannot add a grant

**Validation:** The singleton chatbot can drill into authorized typed EMQX methods but cannot discover or call a generic executor

### Task 7.6: Integrate EMQX as an Async WARM/COLD Coordinator
- [ ] Accept only authenticated, schema-versioned, monotonic-generation health/catalog/event messages
- [ ] Reconcile messages against authoritative local manifests before updating the bounded health snapshot
- [ ] Commit audit inline before any best-effort MQTT mirror
- [ ] Keep OIA material out of every topic and payload
- [ ] Implement typed `provider_unavailable`/`activation_pending` behavior

**Validation:** Broker loss degrades only affected WARM/COLD tools; the unified fabric surface, HOT tools, and `toolsets` remain healthy

### Task 7.7: Canonicalize Sled → Shuttle → Snowball Footprint Flow
- [ ] Inventory both `op-snowball::footprint::PluginFootprint` and legacy `plugin_footprint::PluginFootprint`, plus every generator/conversion call site
- [ ] Define one canonical footprint as a per-mutation payload envelope, not an identity or a precomputed chain digest
- [ ] Inject `actor_id = principal_id`, `session_id`, and `session_genesis` as typed metadata after admission
- [ ] Make the sled emit the canonical bounded/redacted payload and the Shuttle deliver that same payload to Snowball and vectorization
- [ ] Make Snowball the sole current-event chain-hash author: `H(domain || previous_event_hash || canonical_payload_bytes)` once; return `event_id/event_hash` as the receipt
- [ ] Remove `ChainEvent.json_args_footprint`; carry canonical bounded/redacted arguments in the footprint payload so the event chain and vectorizer do not receive only a Blake3 digest
- [ ] Remove legacy `data_hash → content_hash` hash-of-hash generation and any conversion that asks the chain to hash a precomputed current-payload/footprint hash
- [ ] Generate deterministic embedding text from canonical payload fields; store the Snowball receipt as provenance, not as the only vectorized content
- [ ] Remove the duplicate legacy footprint type/generator after migrating all callers

**Validation:** One canonical payload is chain-appended and vectorized; no footprint/hash can affect identity, grants, audience, or session authentication

---

## Phase 8: Integration Testing

**Duration:** 1.5 days  
**Objective:** End-to-end acceptance testing
**Dependencies:** Phases 1-7 complete

### Task 8.1: Service Health Tests
- [ ] Test: EMQX starts under runit
- [ ] Test: `sudo sv status emqx` shows running
- [ ] Test: version matches pinned version

**Validation:** AC-1 passes

### Task 8.2: ExHook Integration Tests
- [ ] Test: ExHook loads and connects
- [ ] Test: callback events in audit trail
- [ ] Test: forged calls rejected

**Validation:** AC-2 passes

### Task 8.3: Authentication Tests
- [ ] Test: unauthenticated D-Bus denied
- [ ] Test: unauthenticated gRPC denied
- [ ] Test: unauthenticated MCP denied

**Validation:** AC-3 passes

### Task 8.4: Authenticated Operation Tests
- [ ] Test: D-Bus `PluginV1.Call` with valid binding
- [ ] Test: gRPC `PluginService.CallMethod` with OIA
- [ ] Test: generated gRPC with OIA
- [ ] Test: paired dashboard uses its optional OIA1 path and real Codex uses direct `:8090/mcp` plus native `http_headers_helper` to forward the exact selected-sled SID1

**Validation:** AC-4 passes

### Task 8.5: MCP Tests
- [ ] Test: direct native Codex `initialize`/`notifications/initialized`, `tools/list`, and `tools/call` on the same router with one exact SID1 credential per protected message and no custom method/name headers
- [ ] Test: canonical stateless direct `tools/list`/`tools/call` succeeds with exact SID1 or optional fresh OIA1 and no authorization-bearing `Mcp-Session-Id`
- [ ] Test: session ID without either accepted identity credential is rejected
- [ ] Test: SID1 exact-match/active-sled/revocation/expiry failure matrix
- [ ] Test: replayed assertion rejected
- [ ] Test: expired assertion rejected
- [ ] Test: source mismatch rejected
- [ ] Test: concurrent replay rejected
- [ ] Test: replay after restart rejected

**Validation:** AC-6 passes

### Task 8.6: Capability Tests
- [ ] Test: exact-`principal_id` denial
- [ ] Test: exact-`principal_id` success
- [ ] Test: production startup rejects wildcard and digest-keyed grants; migrated fixture uses registered principal IDs only
- [ ] Test: two sessions/geneses/footprints for one principal receive identical grants; two principals with identical payload digests do not share authority

**Validation:** AC-7 passes

### Task 8.7: Endpoint Scan
- [ ] Scan for all `/mcp` endpoints
- [ ] Verify only `10.0.0.3:8090/mcp` exists
- [ ] Verify the endpoint is mesh-private and EMQX has no client-facing/public listeners

**Validation:** AC-8 passes

### Task 8.8: MQTT Security Tests
- [ ] Test: anonymous MQTT denied
- [ ] Test: all `$mcp-*` topic access is denied and unauthorized `$opdbus/fabric/*` access is denied
- [ ] Test: authenticated client allowed

**Validation:** AC-10 passes

### Task 8.9: Netmaker Removal Verification
- [ ] Scan codebase for Netmaker references
- [ ] Verify no Netmaker paths in configuration
- [ ] Verify no Netmaker socket bindings

**Validation:** AC-11 passes

### Task 8.10: Audience, Tool-Set, and Failure-Isolation E2E
- [ ] Test singleton chatbot initial discovery is exactly the five typed HOT tools and all five direct calls succeed
- [ ] Test another identity missing one grant receives only four HOT tools; all identities receive zero compact/generic tools
- [ ] Test authorized `emqx_ops` selection, explicit re-list, typed method call, and selection non-escalation
- [ ] Test spoofed client metadata and forged/cross-principal projection handles
- [ ] Test one principal across two sessions keeps the same audience/grants while producing distinct genesis-stamped footprint payloads
- [ ] Test sled → Shuttle sends one canonical payload to Snowball/vectorization, Snowball returns one chain receipt, and no current-payload digest is rehashed as the event
- [ ] Stop EMQX with `sudo sv stop emqx`; verify the unified fabric surface, authentication, HOT tools, and `toolsets` remain healthy
- [ ] Verify affected WARM/COLD tools return typed unavailability, then deliberately restore EMQX with `sudo sv start emqx`

**Validation:** AC-14, AC-15, and AC-16 pass without creating another endpoint

---

## Phase 9: Deployment

**Duration:** 0.5 day  
**Objective:** Deploy through golden subvolume
**Dependencies:** Phase 8 complete

### Task 9.1: Build Release
```bash
CXXFLAGS="-include cstdint" cargo build --workspace --release
```

**Validation:** Build succeeds

### Task 9.2: Dry-Run Deployment
```bash
sudo deploy/runit/build-golden.sh --dry-run
```

**Validation:** Dry-run shows expected changes

### Task 9.3: Golden Subvolume Deployment
```bash
sudo deploy/runit/build-golden.sh --golden-only
```

**Validation:** Golden subvolume created

### Task 9.4: Live Deployment
```bash
sudo deploy/runit/build-golden.sh --live-only
sudo sv restart op-grpc-bridge
# After explicit operator review, EMQX is non-public but stateful:
sudo sv restart emqx
# DO NOT auto-restart network-critical services
```

**Validation:** Services running with new code

### Task 9.5: Post-Deployment Verification
- [ ] Re-run all acceptance tests
- [ ] Verify no regressions
- [ ] Monitor logs for errors

**Validation:** All tests pass post-deployment

---

## Task Summary

| Phase | Tasks | Duration | Dependencies |
|-------|-------|----------|--------------|
| 0: Preparation | 0.1-0.4 | 0.5 day | None |
| 1: Schema Refactoring | 1.1-1.6 | 1.5 days | Phase 0 |
| 2: ExHook Security | 2.1-2.5 | 1 day | Phase 0 |
| 3: Dispatch | 3.1-3.5 | 1 day | Phase 1 |
| 4: D-Bus Identity | 4.1-4.7 | 2 days | Phase 3 |
| 5: Deployment Artifacts | 5.1-5.4 | 0.5 day | Phase 0 |
| 6: SID1 + Optional OIA1 | 6.1-6.4 | 1.5 days | Phase 4 |
| 7: MCP Audience/HOT/Tool Sets | 7.1-7.7 | 2.5 days | Phases 1, 3, 4, 6 |
| 8: Integration Testing | 8.1-8.10 | 1.5 days | Phases 1-7 |
| 9: Deployment | 9.1-9.5 | 0.5 day | Phase 8 |

**Critical Path:** Phase 0 → Phase 1 → Phase 3 → Phase 4 → Phase 6 → Phase 7 → Phase 8 → Phase 9

**Parallelizable:** Phase 2 and Phase 5 can run parallel to Phase 1

---

## Acceptance Criteria Mapping

| AC | Validated By |
|----|--------------|
| AC-1 Service Health | Task 8.1 |
| AC-2 ExHook Integration | Tasks 2.5, 8.2 |
| AC-3 Authentication | Task 8.3 |
| AC-4 Authenticated Operations | Task 8.4 |
| AC-5 Reflection | Tasks 3.4, 8.4 |
| AC-6 MCP Operations | Task 8.5 |
| AC-7 Capability Enforcement | Tasks 7.4-7.5, 8.6 |
| AC-8 Endpoint Configuration | Task 8.7 |
| AC-9 Dashboard Integration | Tasks 6.1, 8.4 |
| AC-10 MQTT Security | Tasks 7.6, 8.8 |
| AC-11 Netmaker Removal | Task 8.9 |
| AC-12 D-Bus Identity | Task 4.5 |
| AC-13 Test Suite | All test tasks |
| AC-14 Audience Projection | Tasks 7.1-7.2, 8.10 |
| AC-15 Five-Tool HOT Surface and Tool Sets | Tasks 7.3-7.5, 8.10 |
| AC-16 EMQX Failure Isolation | Tasks 7.6, 8.10 |
| AC-17 Direct SID1 + Optional OIA1 Paths | Tasks 6.1-6.4, 8.4-8.5 |
| AC-18 Identity/Genesis/Footprint Separation | Tasks 4.6, 7.7, 8.6, 8.10 |
| AC-19 A.N.N.A. Scribe Preservation | Task 4.7 |

---

## Rollback Plan

If deployment fails:

1. **Stop new EMQX activity:** `sudo sv stop emqx`; preserve `/var/lib/emqx` for recovery.
2. **Release rollback:** select and receive/activate the previous known-good btrfs release snapshot through the normal deployment procedure—never hand-copy a previous binary.
3. **Service restart:** deliberately restart affected non-network-critical services with `sudo sv restart <service>` after the previous release is active. Network-critical services remain a console decision.
4. **Source follow-up:** revert through a reviewed source change; do not use destructive worktree commands that could discard unrelated user edits.

Runtime/config changes are reversible through version control and btrfs snapshots;
persistent EMQX data is not deleted by rollback.
