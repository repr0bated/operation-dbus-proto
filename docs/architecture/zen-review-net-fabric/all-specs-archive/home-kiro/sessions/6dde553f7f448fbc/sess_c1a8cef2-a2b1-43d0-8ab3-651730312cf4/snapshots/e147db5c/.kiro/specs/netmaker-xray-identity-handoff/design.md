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

    │
    ▼ (inner IP traffic, no identity binding)
┌─────────────────────────────────────────────────────┐
│ xray proxy (passthrough only)                       │
│   SNI/protocol sniffing for routing                 │
│   NO decryption, NO header injection                │
│   passes traffic unchanged                          │
└─────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────┐
│ Client application (e.g., op-cognitive-mcp)         │
│   ClientConfig.wg_pubkey → self-asserts headers     │
│   X-Ghostbridge-Footprint: <self-computed>          │
│   X-WireGuard-Pubkey: <self-claimed>                │
└─────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────┐
│ GhostbridgeInterceptor                              │
│   validates self-asserted header against sled       │
│   GAP: no proof header-presenter = WG peer          │
└─────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────┐
│ SchemaBackedInterface::call()                       │
│   capability grants check on (unverified) footprint │
└─────────────────────────────────────────────────────┘
```

**Security gap**: A malicious process could forge headers claiming any identity
present in the sled, or replay captured headers on a different connection.

---

## 3 · Target Architecture (Per-Registration Identity Containers)

```text
Netmaker registration (enrollment-key flow)
    │
    ▼ (triggers container provisioning)
┌─────────────────────────────────────────────────────┐
│ Container Provisioner                               │
│   Creates per-identity container with:              │
│   - Transport binding index reader                  │
│   - Rust verification logic                         │
│   - Container-specific egress                       │
│   - Dedicated OVS port + identity register value    │
│   - Human-friendly alias (display only)             │
└─────────────────────────────────────────────────────┘


WireGuard peer (identity-specific)
    │
    ▼ (WG handshake verified by kernel)
┌─────────────────────────────────────────────────────┐
│ wg-lan interface (host)                             │
│   op-identity-shuttle runit service                 │
│   WG_INTERFACE=wg-lan                               │
│   watch_wireguard_handshakes() → IdentitySled       │
│   NEW: → TransportBindingIndex                      │
│         (src_ip:port, wg_pubkey, handshake_ts)      │
└─────────────────────────────────────────────────────┘
    │
    ▼ (traffic enters container's dedicated OVS port)
┌─────────────────────────────────────────────────────┐
│ OVS Bridge (OpenFlow identity tagging)              │
│   LoadRegister action tags traffic with identity    │
│   ct_mark/reg[N] set at flow install time           │
│   Port assignment = unforgeable identity binding    │
└─────────────────────────────────────────────────────┘
    │
    ▼ (traffic routed to identity container)
┌─────────────────────────────────────────────────────┐
│ Per-Registration Identity Container                 │
│   1. Read binding index (SHM mapped from host)      │
│   2. Lookup src_ip:port → verified wg_pubkey        │
│   3. Optionally verify register/ct_mark matches     │
│   4. Compute footprint, check capability grants     │
│   5. Reject if no valid binding (fail-closed)       │
│   6. Forward verified request via container egress  │
└─────────────────────────────────────────────────────┘
    │
    ▼ (verified identity, authorized request)
┌─────────────────────────────────────────────────────┐
│ Destination service                                 │
│   Trusts requests from identity container           │
│   (container already verified identity)             │
└─────────────────────────────────────────────────────┘
```

**Separate traffic paths (unchanged, out of scope):**

Traffic paths labeled by direction:
- **service** — something hosted/provided, inbound-facing
- **consumer** — something initiating outbound traffic

```text
Customer/subscriber privacy tunnels (consumer — netmaker product):
  └─► xray passthrough (SNI routing only) ─► destination
      NO inspection, NO decryption, NO modification

Mail (mail-3tched) (service — inbound-facing):
  └─► incus proxy devices (per-port, hand-configured)
      Separate ingress path, untouched

Qdrant (service — inbound-facing):
  └─► incus proxy devices (per-port, hand-configured)
      Separate ingress path, untouched

Netmaker mesh public API (service — inbound-facing):
  └─► OP_NETMK_* iptables chains (IP-ACL model)
      Untouched, continues operating

assistant container port 8090 (service — host-local control plane):
  └─► dokodemo-door path, already decided

wgcf-egress (consumer — Cloudflare WARP outbound):
  └─► Separate egress path, not part of identity containers

Per-registration identity container egress (consumer — in-scope):
  └─► Each container's own outbound path (§6.3)
```

---

## 4 · Transport Binding Index


### 4.1 Data structure

```rust
/// Maps active WireGuard sessions to verified identities
/// Location: /dev/shm/opdbus/transport-binding.dat
struct TransportBindingIndex {
    magic: [u8; 8],          // "OPBIND01"
    version: u32,
    entry_count: u32,
    entries: [BindingEntry; MAX_ENTRIES],
}

struct BindingEntry {
    src_ip: [u8; 4],         // Inner WG source IP (wg-lan)
    src_port: u16,           // Ephemeral source port (0 = any port from this IP)
    _padding: u16,
    wg_pubkey: [u8; 32],     // Verified WireGuard public key
    handshake_ts: u64,       // Unix timestamp of last verified handshake
    footprint: [u8; 32],     // Pre-computed Blake3 footprint
    flags: u32,              // Entry flags (valid, expired, etc.)
    _reserved: [u8; 12],
}
// Entry size: 96 bytes, aligned for atomic operations
```

### 4.2 Binding lifecycle

1. **Creation**: `watch_wireguard_handshakes()` (running in `op-identity-shuttle`
   with `WG_INTERFACE=wg-lan`) observes new handshake via `wg show wg-lan
   latest-handshakes`, extracts pubkey and allowed-IPs, writes binding entry.

2. **Lookup**: Identity container queries binding by source IP.

3. **Expiry**: Entry expires when `now - handshake_ts > 180s` (WireGuard
   handshake timeout). Expired entries are marked invalid and reused.

4. **Update**: Re-handshake updates `handshake_ts`, keeping the binding alive.

### 4.3 Why source IP is sufficient

WireGuard's allowed-IPs already constrains which inner source IPs a peer can
use. A peer with `allowed-ips = 10.200.1.5/32` can ONLY send traffic with
source IP 10.200.1.5 through the tunnel. The kernel enforces this.

Therefore, `(src_ip)` → `(wg_pubkey)` is a valid binding when:
- allowed-IPs is a /32 (single host)
- binding is created at handshake time
- binding expires with the WireGuard session

For peers with broader allowed-IPs (e.g., /24), additional mechanisms may be
needed (DISCUSS: is this a real use case for this deployment?).

---

## 5 · op-identity-shuttle Runit Service

### 5.1 Service definition

The `op-identity-shuttle` binary exists (entry point: `run_schema_shuttle()` in
`crates/op-identity/src/schema_bridge.rs:1244`) but no runit service runs it.

```bash
# /etc/runit/sv/op-identity-shuttle/run
#!/bin/sh
exec 2>&1
export WG_INTERFACE=wg-lan
exec /usr/local/bin/op-identity-shuttle
```

### 5.2 Dependencies

- **wg-lan**: Must be up before shuttle starts watching. Add dependency via
  runit's `sv check wg-lan` or a `check` script.
- **SHM directory**: `/dev/shm/opdbus/` must exist with correct permissions.

### 5.3 Supervision

- Runit will restart on crash.
- Health check: verify binding index SHM is writable and recent.
- Log output via `svlogd` to `/var/log/op-identity-shuttle/`.

---

## 6 · Per-Registration Identity Container
