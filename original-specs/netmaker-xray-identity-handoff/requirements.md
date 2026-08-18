# WireGuard Identity-Based Gating — Requirements

> Establish verified WireGuard-identity-based gating for per-registration identity
> containers: the `wg-lan` interface terminates identity-specific WireGuard tunnels
> (separate from netmaker's mesh), and verification logic in provisioned containers
> gates access based on cryptographically verified identity. This closes the gap
> between CLAUDE.md's claimed zero-trust model and the actual code, where identity
> headers are currently self-asserted rather than cryptographically verified.

| Field | Value |
| --- | --- |
| Status | Specification draft — implementation not started |
| Baseline | 2026-08-04 codebase investigation |
| WG interface | `wg-lan` (standalone identity server, NOT netmaker mesh) |
| Related crates | `op-identity`, `op-grpc-bridge`, `op-plugins` |
| Netmaker mesh | Untouched — continues using `OP_NETMK_*` IP-ACL model |

---

## 1 · Problem Statement and Gap Analysis

### 1.1 Claimed model (CLAUDE.md)

CLAUDE.md's "Transport & identity (zero-trust)" section claims:
- `X-Ghostbridge-Footprint`/`X-WireGuard-Pubkey` headers are "the only real gate"
- IP ACLs are described as "theater"
- Identity = WireGuard pubkey → Argon2(PSK, salt=pubkey) session ID

### 1.2 Actual code state

| Component | Claimed | Actual |
| --- | --- | --- |
| `etch_footprint()` | Identity derivation | **LIVE**: Blake3(wg_pubkey ‖ catalog_hash ‖ mutation_index ‖ source_port) |
| `watch_wireguard_handshakes()` | WG state monitoring | **LIVE**: Uses `ip monitor route` + `wg show`, interface from `WG_INTERFACE` env |
| `run_schema_shuttle()` | Shuttle binary entry | **CODE EXISTS**: reads `WG_INTERFACE` (default `wg0`), but no runit service |
| `build_xray_config` | Dynamic config | **TEST-ONLY**: `#[cfg(test)]`, unused identity params |
| `GhostbridgeInterceptor` | Verified gate | **SELF-ASSERTED**: Validates header, not transport peer |
| `ClientConfig.wg_pubkey` | Header injection | **CLIENT SELF-ASSERTED** |
| `op-xray-daemon` | Header injection | **LIFECYCLE ONLY**: No traffic inspection |
| `wireguard.rs` | Identity integration | **DISCONNECTED**: Zero connection to identity-sled |

### 1.3 Core security gap

The current model has a fundamental authentication bypass:
1. WireGuard terminates and verifies the peer cryptographically
2. IdentitySled is updated with verified pubkey
3. **GAP**: Client self-asserts identity in HTTP headers
4. Interceptor validates self-asserted header against sled
5. **No cryptographic binding** proves header-presenter = WG peer

### 1.4 Infrastructure state

- `wg-lan`: Standalone WireGuard interface for identity verification, deliberately
  decoupled from netmaker's mesh (avoids netmaker's MTU constraints). Config at
  `/etc/wireguard/wg-lan.conf`, runit service at `/etc/runit/sv/wg-lan/`.
- `op-identity-shuttle`: Binary exists in codebase (`run_schema_shuttle()`), but
  **no runit service exists** — it is not running on the live host today.
- Netmaker mesh: Separate system, continues using its own IP-ACL model
  (`OP_NETMK_*` iptables chains) — this spec does NOT touch netmaker.

---

## 2 · Hard Constraints

1. **Cryptographic binding**: Identity MUST be verified at WG termination (`wg-lan`)
   and bound to transport. Self-asserted headers are NOT acceptable.

2. **No header replay**: Binding MUST prevent replay attacks.

3. **Fail-closed**: Missing/invalid verification = rejection, not fallback.

4. **wg-lan scope only**: This system uses `wg-lan`, NOT netmaker's mesh interface.

5. **Grants reliability**: SHM materialization staleness (discovered failure mode)
   MUST be addressed.

6. **Per-registration containers**: Verification logic lives inside provisioned
   identity containers, NOT in xray or existing infrastructure.

7. **xray remains passthrough**: xray does NOT inject, decrypt, or modify traffic.
   It performs SNI/protocol sniffing for routing only.

---

## 3 · Functional Requirements

### FR-1 — WireGuard-to-Transport Identity Binding (wg-lan scoped)

- The handshake watcher SHALL monitor `wg-lan` (set via `WG_INTERFACE=wg-lan`).
- A binding mechanism SHALL cryptographically prove application requests originate
  from verified WG peers on `wg-lan`.
- Binding key: source IP:port + verified pubkey + handshake timestamp.
- Binding expires with WG session (latest-handshake > 180s).

### FR-2 — op-identity-shuttle Runit Service

- A new runit service SHALL run the `op-identity-shuttle` binary.
- Environment: `WG_INTERFACE=wg-lan`.
- Service location: `/etc/runit/sv/op-identity-shuttle/`.
- Dependencies: `wg-lan` must be up before shuttle starts watching.

### FR-3 — IdentitySled Integration

- Preserve existing 152-byte SHM layout.
- Add transport-binding index: `(src_ip, src_port)` → `(wg_pubkey, handshake_ts)`.
- Binding updated by `watch_wireguard_handshakes()` on new handshakes.
- Stale bindings purged automatically.

### FR-4 — Per-Registration Identity Container

- Each netmaker registration (enrollment-key flow) provisions a dedicated container.
- Container contains: transport binding index reader, verification logic, container-
  specific egress.
- Verification: source IP/peer → verified WG pubkey → footprint → capability grants.
- All verification logic is Rust (per CLAUDE.md: "Rust-first: no new Python").

### FR-5 — Capability Grants Gate (in identity container)

- Container's verification logic enforces grants on verified footprint.
- Footprint: `Blake3(wg_pubkey ‖ catalog_hash ‖ mutation_index ‖ source_port)`.
- Wildcard fallback for unconfigured peers.

### FR-6 — Grants Materialization Reliability

- Detect staleness: compare SHM hash against installed file.
- Auto-recover: re-materialize if stale.
- D-Bus method for forced re-materialization.
- Log staleness events at WARN level.

### FR-7 — Container Lifecycle Management

- **Provisioning trigger**: Netmaker registration via enrollment-key/join-token flow
  (see `enrollment_keys_v1`/`tenants_v1` tables).
- **Deprovisioning**: Explicit deletion on enrollment-key revocation or tenant removal.
- **Expiry**: Configurable TTL with warning notifications before expiry.

### FR-8 — OpenFlow Identity Tagging

- Each provisioned identity container SHALL receive a dedicated OVS port.
- Container provisioning SHALL assign a unique register/ct_mark value derived from
  the identity (e.g., lower 32 bits of Blake3(tenant_id)).
- OpenFlow flows SHALL tag traffic entering on that port via `LoadRegister` action.
- Downstream flows and/or in-container verification MAY use the register/ct_mark
  as an additional datapath-level binding (OVS port assignment is unforgeable).

### FR-9 — Human-Friendly Container Alias

- Each identity container SHALL have a human-friendly alias generated deterministically
  at provisioning time (e.g., petname-style adjective-noun from hash of tenant_id/pubkey).
- The alias is for display/reference only: container naming, logs, `incus list` output,
  audit trail readability.
- The alias MUST NEVER be accepted as input to verification, capability-grants, or
  footprint-matching logic. Only the Blake3-derived footprint is authoritative.

---

## 4 · Reusable vs. From-Scratch

### Reusable

| Component | Assessment |
| --- | --- |
| IdentitySled SHM layout | Add binding index alongside |
| `read_sled()`/`write_sled()` | Correct primitives |
| `etch_footprint()` | Cryptographically sound |
| `watch_wireguard_handshakes()` | Live, uses interface from env var |
| `run_schema_shuttle()` | Entry point exists, needs runit service |
| `derive_session_id()` | Correct KDF |
| `GhostbridgeInterceptor` parsing | Header parsing logic (reuse in container) |
| `load_capability_grants()` | Grant loading/matching |
| `JsonFlowAction::LoadRegister` | Already exists in openflow.rs JSON schema |
| `rovs_openflow::Match` metadata/ct_mark | Crate already supports these fields |
| NXM register encoding (REG0–REG15, XXREG0–3) | Full support in oxm.rs |

### From-Scratch

| Component | Reason |
| --- | --- |
| `op-identity-shuttle` runit service | Does not exist |
| Transport binding index | New IP:port → identity mapping |
| Per-registration identity container | New provisioning system |
| Container verification logic | Rust verification in container |
| Container egress configuration | Per-container network isolation |
| Grants materialization reliability | Staleness recovery not implemented |
| Container lifecycle (provision/deprovision) | New netmaker-triggered flow |
| `parse_match()` extension for metadata/ct_mark/reg[N] | Missing plumbing in openflow_translate.rs |
| Human-friendly alias generation | New deterministic petname derivation |

### Justification: IdentitySled IS the correct source

The existing IdentitySled and handshake watcher ARE correct because:
1. `watch_wireguard_handshakes()` observes real kernel WG state
2. Updates within seconds via `ip monitor` + `wg show`
3. `etch_footprint()` produces collision-resistant fingerprint

What's missing: the runit service to run it, binding TCP connection → verified WG peer,
per-container verification logic.

---

## 5 · Out of Scope

### 5.1 Explicitly excluded from this spec

Traffic paths are labeled by direction:
- **service** — something hosted/provided, inbound-facing
- **consumer** — something initiating outbound traffic

| Traffic type | Direction | Description |
| --- | --- | --- |
| Customer/subscriber privacy tunnels | **consumer** | Netmaker's product — subscriber's outbound WG tunnel traffic. MUST remain pure xray passthrough (SNI routing only). xray must never inspect, decrypt, or inject. Hard constraint. |
| Mail (mail-3tched) | **service** | Inbound-facing mail service. Keeps existing incus proxy device path. |
| Qdrant | **service** | Inbound-facing vector DB service. Keeps existing incus proxy device path. |
| Netmaker mesh (public API) | **service** | Netmaker's own public-facing API. `OP_NETMK_*` iptables chains continue unchanged. Uses `wg-lan`, decoupled from netmaker mesh. |
| `assistant` container (port 8090) | **service** | Host-local control-plane service. Dokodemo-door path already decided. |
| `wgcf-egress` | **consumer** | Cloudflare WARP egress for outbound traffic. Separate from identity containers. |
| Per-registration identity container egress | **consumer** | Each provisioned identity container's own outbound path (§3/FR-4). In-scope for this spec, listed here for completeness. |

**xray header injection** remains out of scope: xray is a pure passthrough proxy with
SNI/protocol sniffing for routing only. It does NOT decrypt TLS, inject headers, or
modify traffic.

### 5.2 Other exclusions

- Replacing WireGuard
- Removing IP ACLs entirely
- Phase 2 xray NIC migration

---

## 6 · Acceptance Criteria

Phase 1 complete when:
- [ ] `op-identity-shuttle` runit service running with `WG_INTERFACE=wg-lan`
- [ ] Transport binding maps `wg-lan` sessions → verified pubkeys
- [ ] Per-registration identity container provisioning triggered by enrollment
- [ ] Container verification logic validates requests against binding
- [ ] Requests without valid binding rejected (fail-closed)
- [ ] Grants gate uses verified footprint
- [ ] Grants staleness auto-recovered
- [ ] Container lifecycle (provision/deprovision/expiry) documented and tested
- [ ] OpenFlow identity tagging: `parse_match()`/`build_actions()` extended for metadata/ct_mark/reg[N]
- [ ] Each container assigned dedicated OVS port with identity-derived register value
- [ ] Human-friendly alias generated for each container (display only, never authoritative)
- [ ] 48-hour stability observation
