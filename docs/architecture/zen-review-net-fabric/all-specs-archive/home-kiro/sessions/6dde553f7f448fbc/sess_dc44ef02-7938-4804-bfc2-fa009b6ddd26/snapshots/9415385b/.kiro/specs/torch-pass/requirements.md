# Torch-Pass — Requirements

> Replace xray's static IP/port ACLs with cryptographically verified WireGuard
> identity-based gating. A peer's identity must be verified at the point where
> WireGuard actually terminates, not trusted because a header claims it.

| Field | Value |
| --- | --- |
| Status | Design |
| Problem | Self-asserted X-Ghostbridge-Footprint headers create identity "theater" |
| Solution | Bind footprint cryptographically to verified WireGuard transport peer |
| Related crates | `op-identity`, `op-grpc-bridge`, `op-xray-daemon`, `op-plugins` |

---

## 1 · Problem Statement

CLAUDE.md claims a zero-trust model where `X-Ghostbridge-Footprint` and
`X-WireGuard-Pubkey` headers are "the only real gate" and IP ACLs are "theater."
Investigation reveals the gap has merely shifted layers:

| Layer | Claimed | Actual |
| --- | --- | --- |
| IP/port ACLs | Theater | Correct — xray uses static `/etc/xray/xray_config.json` |
| X-Ghostbridge-Footprint | Verified identity gate | **Self-asserted by client** via `ClientConfig.with_wg_pubkey()` |
| GhostbridgeInterceptor | Validates footprint | Validates header **presence and format**, not transport binding |
| SchemaBackedInterface | Capability gate | Real gate, but trusts unverified footprint origin |
| WireGuard termination | Source of identity truth | **Disjoint** from identity-sled/footprint system |

The capability grant system (`capability-grants.json`) and D-Bus dispatch gate
(`SchemaBackedInterface::call()`) are real and wired. The problem is that the
footprint they check originates from a self-asserted header, not from the
WireGuard peer that actually terminated the transport connection.

### 1.1 Concrete evidence

1. **Client self-assertion**: `op-cognitive-mcp/src/client_config.rs` line 568:
   `.header("X-Ghostbridge-Footprint", pubkey)` — client sets its own identity.

2. **No transport binding**: `GhostbridgeInterceptor` validates header presence
   and checks against sled, but nothing proves the header matches the actual
   WireGuard peer on the transport.

3. **Test-only xray config**: `schema_bridge.rs` functions `build_xray_config`
   and `route_to_outbound` are `#[cfg(test)]`-gated — never compiled into
   production. The `_footprint` and `_trace_id` parameters in `route_to_outbound`
   are unused placeholders.

4. **Disjoint systems**: `WireGuardPlugin` in `wireguard.rs` does peer CRUD but
   has zero connection to the identity-sled/footprint system. These systems
   were designed separately and never integrated.

5. **No datapath identity**: OpenFlow translation (`openflow_translate.rs`)
   supports only standard OF1.3 match fields. No `reg[N]`, `metadata`, or
   `ct_mark` fields are wired for per-peer identity marking.

### 1.2 Capability-grants staleness failure mode

The capability-grants system has a known staleness failure mode documented in
operational history: the SHM copy at `/dev/shm/opdbus/capability-grants.json`
drifted from the source after a network outage. The `opdbus-grants` service
had to be manually restarted to rematerialize grants. Any solution must address
this failure mode with automatic invalidation or bounded staleness guarantees.

---

## 2 · Required Outcome and Hard Constraints

1. A peer's WireGuard identity SHALL be verified at the point where WireGuard
   actually terminates (netmaker, xray, or wherever the tunnel lands), not
   trusted because a header claims it.

2. Xray SHALL gate on that verified identity, replacing today's static IP/port
   ACL model. The gating mechanism SHALL be cryptographically bound to the
   verified transport peer.

3. The solution SHALL NOT require OpenFlow/datapath-level identity gating.
   Per-peer WireGuard-pubkey gating cannot be an OF1.3 match key without
   significant new infrastructure. Identity gating SHALL be application-layer
   (gRPC/D-Bus capability gate).

4. Xray application configuration SHALL remain at `/etc/xray/xray_config.json`
   inside the container. Models SHALL NOT write or reload xray directly until
   the validated model/control-plane generator is implemented.

5. The capability-grants materialization staleness failure mode SHALL be
   explicitly addressed with automatic invalidation, bounded staleness, or
   event-driven refresh.

6. Host service supervision SHALL remain runit (`sudo sv ...`). Container
   lifecycle operations SHALL use D-Bus through `busctl`, not `systemctl`.

---

## 3 · Functional Requirements

### FR-1 — Verified peer identity at WireGuard termination

- WireGuard handshake completion SHALL be the authoritative source of peer
  identity. The `watch_wireguard_handshakes()` pattern in `schema_bridge.rs`
  (monitoring `ip monitor route` + `wg show <iface> latest-handshakes`) is
  a valid foundation.

- A successful handshake SHALL produce a verified `(wg_pubkey, source_ip,
  timestamp)` tuple stored in a verified-peers registry.

- The verified-peers registry SHALL have bounded staleness: entries expire
  after a configurable handshake-age threshold (default: 180 seconds per
  existing convention).

- The registry SHALL be the **sole authoritative source** for peer identity
  verification. Self-asserted headers SHALL NOT be accepted without registry
  confirmation.

### FR-2 — Cryptographic binding of footprint to transport peer

- The footprint presented in `X-Ghostbridge-Footprint` SHALL be validated
  against the verified-peers registry using the connection's **actual source
  IP** as the binding key.

- Validation SHALL confirm:
  1. Source IP has a verified WireGuard peer entry
  2. The claimed pubkey matches the registry entry for that source IP
  3. The handshake timestamp is within the staleness threshold
  4. The footprint computation (Blake3 of pubkey||schema_hash||mutation_index||
     source_port per `etch_footprint()`) matches

- Validation failure SHALL result in request rejection with `AccessDenied`
  before any capability grant lookup occurs.

### FR-3 — Xray identity-based routing (replacing static ACLs)

- Xray SHALL route inbound connections based on verified peer identity rather
  than static IP/port rules.

- The xray configuration generator SHALL:
  1. Query the verified-peers registry for current peer set
  2. Generate routing rules that bind each peer's tunnel IP to their identity
  3. Atomically update xray configuration and reload

- Configuration generation SHALL be event-driven by:
  - WireGuard peer add/remove events
  - Handshake timeout/refresh events
  - Explicit operator-triggered regeneration

- The static bootstrap configuration SHALL remain at
  `/etc/xray/xray_config.json` until the dynamic generator is verified
  and deployed.

### FR-4 — Capability-grants freshness guarantee

- The capability-grants materialization SHALL be event-driven, not static file
  copy.

- On bridge startup, grants SHALL be loaded from the durable source
  (`deploy/security/capability-grants.json`), verified against the SHM copy,
  and rematerialized if divergent.

- Grant changes SHALL propagate within a bounded window (≤5 seconds) via:
  - Inotify watch on the durable source, OR
  - D-Bus signal on grant modification, OR
  - Periodic refresh with generation tracking

- The SHM materialization SHALL include a generation counter. Consumers SHALL
  reject grants with stale generations.

- Grant staleness beyond the threshold SHALL fail closed: requests rejected
  until grants refresh.

### FR-5 — Integration of disjoint systems

- `WireGuardPlugin` peer CRUD SHALL emit events to the verified-peers registry.
  Peer addition SHALL seed an initial (unverified) entry; handshake completion
  verifies it.

- `IdentitySled` SHALL be extended (or a parallel verified-peers sled created)
  to store verified peer state alongside schema/footprint data.

- The `GhostbridgeInterceptor` SHALL be modified to:
  1. Extract actual source IP from the connection (not from headers)
  2. Query the verified-peers registry
  3. Validate the claimed footprint against the verified entry
  4. Reject if no verified entry exists or footprint mismatches

### FR-6 — Fail-closed behavior

- Missing verified-peers entry: reject request
- Expired handshake (>threshold): reject request
- Footprint mismatch: reject request
- Registry unavailable: reject request (do not fall back to header trust)
- Stale capability-grants: reject request

### FR-7 — Observability and audit

- Every identity verification decision SHALL produce audit evidence:
  - Peer pubkey (truncated for logs)
  - Source IP
  - Verification result (success/failure reason)
  - Handshake age at verification time
  - Capability grants applied (if successful)

- Failed verifications SHALL be logged at WARN level with sufficient detail
  for diagnosis without exposing secrets.

---

## 4 · Component Analysis: Reusable vs. From-Scratch

### 4.1 Reusable (with modification)

| Component | Location | Reuse Assessment |
| --- | --- | --- |
| IdentitySled SHM layout | `schema_bridge.rs` | Reusable as template for verified-peers registry |
| `watch_wireguard_handshakes()` | `schema_bridge.rs` | Reusable as handshake monitoring foundation |
| `etch_footprint()` Blake3 computation | `schema_bridge.rs` | Reusable for footprint validation |
| Argon2/Blake3 session-id derivation | `session.rs` | Reusable for any derived identifiers |
| `GhostbridgeInterceptor` structure | `interceptor.rs` | Reusable; add source-IP extraction and registry lookup |
| `load_capability_grants()` | `schema_router.rs` | Reusable; add freshness check |
| `SchemaBackedInterface::call()` gate | `schema_router.rs` | Reusable; the gate is real, just needs verified input |
| Capability-grants JSON format | `capability-grants.json` | Reusable; add generation counter |

### 4.2 From-scratch (new implementation required)

| Component | Reason |
| --- | --- |
| Verified-peers registry | `WireGuardPlugin` and identity-sled are currently disjoint |
| Source-IP extraction in interceptor | Current interceptor trusts headers, not connection metadata |
| Xray config generator | `build_xray_config`/`route_to_outbound` are test-only stubs |
| Event-driven grant refresh | Current materialization is static file copy |
| WireGuard→registry event bridge | No integration exists between peer CRUD and identity system |
| Dynamic xray reload orchestration | `op-xray-daemon` is lifecycle-only, no config injection |

### 4.3 Not reusable

| Component | Reason |
| --- | --- |
| Static xray ACLs | The whole point is to replace them |
| Self-asserted header trust | The security gap being closed |
| OpenFlow identity fields | OF1.3 has no reg/metadata/ct_mark wired; datapath gating not viable |

---

## 5 · Acceptance Criteria

### 5.1 Identity verification

| Test | Required Result |
| --- | --- |
| Valid peer with fresh handshake | Request proceeds to capability check |
| Valid peer with expired handshake | Request rejected before capability check |
| Unknown source IP | Request rejected |
| Footprint mismatch | Request rejected |
| Registry unavailable | Request rejected (fail-closed) |

### 5.2 Xray routing

| Test | Required Result |
| --- | --- |
| Known peer connects | Routed based on verified identity |
| Unknown peer connects | Connection rejected or routed to deny handler |
| Peer removed from WireGuard | Subsequent connections rejected |
| Config regeneration | Atomic update with no traffic interruption |

### 5.3 Grants freshness

| Test | Required Result |
| --- | --- |
| Grant file modified | SHM refreshed within 5 seconds |
| SHM diverged from source | Rematerialized on next bridge startup |
| Generation counter stale | Requests rejected until refresh |

### 5.4 End-to-end

| Test | Required Result |
| --- | --- |
| Legitimate client with valid WG tunnel | Full access per capability grants |
| Spoofed header from non-WG source | Rejected at identity verification |
| Spoofed header claiming different peer | Rejected (source IP mismatch) |
| Replay of old valid request | Rejected if handshake expired |

---

## 6 · Non-functional Requirements

| ID | Requirement |
| --- | --- |
| NFR-1 | Verification latency ≤1ms for registry lookup |
| NFR-2 | Registry updates propagate within 1 second of handshake |
| NFR-3 | Xray config regeneration completes within 5 seconds |
| NFR-4 | No request proceeds without verified identity (fail-closed) |
| NFR-5 | Audit log entries for all verification decisions |
| NFR-6 | Registry memory footprint ≤1KB per peer |
| NFR-7 | Graceful degradation: if WG monitor fails, existing verified entries honor their TTL |

---

## 7 · Out of Scope

- OpenFlow/datapath-level identity gating (not viable with current OF1.3 support)
- Replacing WireGuard with another tunnel protocol
- Modifying xray core (changes are config-level only)
- Multi-site federation of verified-peers registries
- Automatic peer provisioning (peer CRUD remains manual/API-driven)
- Changes to the Netmaker integration (separate spec)
