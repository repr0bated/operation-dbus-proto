# WireGuard Identity-Based Gating — Design

## 1 · Design Status

This design closes the gap between CLAUDE.md's claimed zero-trust model and actual
code. Investigation on 2026-08-04 established that identity headers are currently
self-asserted by clients (`ClientConfig.wg_pubkey` at `client_config.rs:568`),
not cryptographically bound to the WireGuard peer that terminated the transport.

The core insight: the verification infrastructure (IdentitySled, handshake watcher,
footprint derivation) is correct and live. The gap is in the **binding** — proving
that a given TCP connection originates from the entity that completed a verified
WireGuard handshake.

**Key architectural decisions (2026-08-04 revision):**

1. **WireGuard termination**: Uses `wg-lan`, a standalone WireGuard interface for
   identity verification, deliberately decoupled from netmaker's mesh to avoid
   MTU constraints. Netmaker's mesh is untouched.

2. **xray remains passthrough**: xray performs SNI/protocol sniffing for routing
   only (`"security": "none"` with `"sniffing": {"routeOnly": true}`). It does NOT
   decrypt TLS, inject headers, or modify traffic. The previous design's xray
   plugin/sidecar approach is removed — it was unworkable for passthrough TLS.

3. **Per-registration identity containers**: Verification logic lives in dedicated
   containers provisioned at netmaker registration time, NOT in xray or existing
   infrastructure. Each registered identity gets its own workspace with its own egress.

4. **OpenFlow identity tagging**: Each identity container's OVS port IS the identity
   at the datapath level. Traffic entering on that port is tagged with a register/ct_mark
   value identifying the container — an additional, unforgeable binding.

---

## 2 · Current Architecture (Self-Asserted)

```text
WireGuard peer
    │
    ▼ (WG handshake verified by kernel)
┌─────────────────────────────────────────────────────┐
│ wg-lan interface (standalone identity WG server)    │
│   watch_wireguard_handshakes() → IdentitySled       │
│   (pubkey IS verified here)                         │
│   NOTE: op-identity-shuttle binary exists but       │
│         NO runit service runs it today              │
└─────────────────────────────────────────────────────┘
