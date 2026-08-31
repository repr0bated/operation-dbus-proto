# WireGuard Identity-Based Gating — Requirements

> Replace xray's static IP/port ACL model with verified WireGuard-identity-based
> gating ("the torch pass"): WireGuard terminates at the peer's tunnel endpoint,
> that verified identity is cryptographically bound to transport, and xray gates
> purely on identity rather than source IP/port. This closes the gap between
> CLAUDE.md's claimed zero-trust model and the actual code, where identity
> headers are currently self-asserted rather than cryptographically verified.

| Field | Value |
| --- | --- |
| Status | Specification draft — implementation not started |
| Baseline | 2026-08-04 codebase investigation |
| Supersedes | Static IP/port ACL model (`OP_NETMK_*` iptables chains, OVS/OpenFlow rules) |
| Related crates | `op-identity`, `op-grpc-bridge`, `op-xray-daemon`, `op-plugins`, `op-cognitive-mcp` |
| Live xray config | `/etc/xray/xray_config.json` inside container only (per AGENTS.md) |

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
| `watch_wireguard_handshakes()` | WG state monitoring | **LIVE**: Uses `ip monitor route` + `wg show` |
| `build_xray_config` | Dynamic config | **TEST-ONLY**: `#[cfg(test)]`, unused identity params |
| `GhostbridgeInterceptor` | Verified gate | **SELF-ASSERTED**: Validates header, not transport peer |
| `ClientConfig.wg_pubkey` | Header injection | **CLIENT SELF-ASSERTED** |
| `op-xray-daemon` | Header injection | **LIFECYCLE ONLY**: No traffic inspection |
| `wireguard.rs` | Identity integration | **DISCONNECTED**: Zero connection to identity-sled |
| OpenFlow/OVS | Per-peer gating | **NOT VIABLE**: OF1.3 has no pubkey match field |

### 1.3 Core security gap

The current model has a fundamental authentication bypass:
1. WireGuard terminates and verifies the peer cryptographically
2. IdentitySled is updated with verified pubkey
3. **GAP**: Client self-asserts identity in HTTP headers
4. Interceptor validates self-asserted header against sled
5. **No cryptographic binding** proves header-presenter = WG peer

---

## 2 · Hard Constraints

1. **Cryptographic binding**: Identity MUST be verified at WG termination and bound to transport. Self-asserted headers are NOT acceptable.

2. **No header replay**: Binding MUST prevent replay attacks.

3. **Fail-closed**: Missing/invalid verification = rejection, not fallback.

4. **xray config path**: ONLY `/etc/xray/xray_config.json` inside container.

5. **Grants reliability**: SHM materialization staleness (discovered failure mode) MUST be addressed.

6. **Datapath limitations**: Identity gating is application-layer only (OF1.3 cannot match pubkey).

---

## 3 · Functional Requirements

### FR-1 — WireGuard-to-Transport Identity Binding

- The handshake watcher SHALL remain the authoritative verified identity source.
- A binding mechanism SHALL cryptographically prove application requests originate from verified WG peers.
- Binding key: source IP:port + verified pubkey + handshake timestamp.
- Binding expires with WG session (latest-handshake > 180s).

### FR-2 — xray Identity Injection

- xray SHALL inject verified identity headers, NOT accept client-presented ones.
- Injection point: xray inbound handler for WG-terminated traffic.
- Headers: `X-Ghostbridge-Footprint`, `X-WireGuard-Pubkey`, `X-Ghostbridge-Trace-Id`
- xray SHALL strip client-provided identity headers before injection.

### FR-3 — IdentitySled Integration

- Preserve existing 152-byte SHM layout.
- Add transport-binding index: `(src_ip, src_port)` → `(wg_pubkey, handshake_ts)`.
- Binding updated by `watch_wireguard_handshakes()` on new handshakes.
- Stale bindings purged automatically.

### FR-4 — Capability Grants Gate

- `SchemaBackedInterface::call()` continues enforcing grants on verified footprint.
- Footprint: `Blake3(wg_pubkey ‖ catalog_hash ‖ mutation_index ‖ source_port)`.
- Wildcard fallback for unconfigured peers.

### FR-5 — Static IP/Port ACL Deprecation

- Existing `OP_NETMK_*` chains retained as defense-in-depth, not primary control.
- Audit logs distinguish identity-gated vs. IP-ACL-only access.

### FR-6 — Grants Materialization Reliability

- Detect staleness: compare SHM hash against installed file.
- Auto-recover: re-materialize if stale.
- D-Bus method for forced re-materialization.
- Log staleness events at WARN level.

### FR-7 — xray Config Generation

- Generator produces config with identity-based routing.
- Atomic write to `/etc/xray/xray_config.json`.
- D-Bus reload with verification.
- Static bootstrap remains valid until generator deployed.

### FR-8 — WireGuard Plugin Integration

- `WireGuardPlugin` emits peer-added/removed events on D-Bus.
- Events include pubkey and allowed-IPs.
- Integrate with IdentitySled writer.

---

## 4 · Reusable vs. From-Scratch

### Reusable

| Component | Assessment |
| --- | --- |
| IdentitySled SHM layout | Add binding index alongside |
| `read_sled()`/`write_sled()` | Correct primitives |
| `etch_footprint()` | Cryptographically sound |
| `watch_wireguard_handshakes()` | Live, uses proper interfaces |
| `derive_session_id()` | Correct KDF |
| `GhostbridgeInterceptor` parsing | Header parsing logic |
| `load_capability_grants()` | Grant loading/matching |
| `SchemaBackedInterface::call()` | Dispatch gate wiring |

### From-Scratch

| Component | Reason |
| --- | --- |
| Transport binding index | New IP:port → identity mapping |
| xray header injection | No injection logic exists |
| xray config generator | Test-only code unusable |
| WG plugin ↔ identity-sled integration | Disconnected systems |
| Grants materialization reliability | Staleness recovery not implemented |
| Header stripping in xray | Prevent stuffing attacks |

### Justification: IdentitySled IS the correct source

The existing IdentitySled and handshake watcher ARE correct because:
1. `watch_wireguard_handshakes()` observes real kernel WG state
2. Updates within seconds via `ip monitor` + `wg show`
3. `etch_footprint()` produces collision-resistant fingerprint

What's missing: binding TCP connection → verified WG peer, header injection at xray, expiry alignment.

---

## 5 · Out of Scope

- Replacing WireGuard
- Per-peer OpenFlow matching (OF1.3 limitation)
- Removing IP ACLs entirely
- Changing xray config path
- Phase 2 xray NIC migration

---

## 6 · Acceptance Criteria

Phase 1 complete when:
- [ ] Transport binding maps WG sessions → verified pubkeys
- [ ] xray injects verified identity headers
- [ ] xray strips client-provided identity headers
- [ ] Requests without binding rejected (fail-closed)
- [ ] Grants gate uses verified footprint
- [ ] Grants staleness auto-recovered
- [ ] 48-hour stability observation
