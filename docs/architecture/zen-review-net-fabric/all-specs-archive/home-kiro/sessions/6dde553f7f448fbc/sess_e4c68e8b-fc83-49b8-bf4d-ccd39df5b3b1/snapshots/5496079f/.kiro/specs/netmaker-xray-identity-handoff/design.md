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
    ▼ (traffic routed to identity container)
┌─────────────────────────────────────────────────────┐
│ Per-Registration Identity Container                 │
│   1. Read binding index (SHM mapped from host)      │
│   2. Lookup src_ip:port → verified wg_pubkey        │
│   3. Compute footprint, check capability grants     │
│   4. Reject if no valid binding (fail-closed)       │
│   5. Forward verified request via container egress  │
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

```text
Customer/subscriber privacy tunnels (netmaker product):
  └─► xray passthrough (SNI routing only) ─► destination
      NO inspection, NO decryption, NO modification

Mail/Qdrant/similar services:
  └─► incus proxy devices (per-port, hand-configured)
      Separate ingress/egress paths, untouched

Netmaker mesh traffic:
  └─► OP_NETMK_* iptables chains (IP-ACL model)
      Untouched, continues operating
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

### 6.1 Provisioning trigger

Container provisioning is triggered by netmaker's enrollment-key/registration flow:

1. New peer registers via enrollment key (join-token flow).
2. Registration creates entry in `enrollment_keys_v1`/`tenants_v1` tables.
3. **NEW**: Registration event triggers identity container provisioning.

```rust
// Pseudocode for provisioning trigger
async fn on_netmaker_registration(event: RegistrationEvent) {
    let container_name = format!("identity-{}", event.tenant_id);
    
    // Create container with:
    // - Read-only bind mount of /dev/shm/opdbus/transport-binding.dat
    // - Verification binary (Rust)
    // - Container-specific egress network config
    // - Capability grants for this identity
    
    provision_identity_container(&container_name, &event)?;
}
```

### 6.2 Container contents

Each identity container contains:

1. **Transport binding index reader**: Read-only access to host's
   `/dev/shm/opdbus/transport-binding.dat` via bind mount.

2. **Verification binary** (Rust, per CLAUDE.md "Rust-first"):
   - Reads source IP from incoming connection
   - Looks up binding in transport index
   - Computes footprint from verified pubkey
   - Checks capability grants
   - Rejects if no valid binding (fail-closed)

3. **Capability grants**: Per-identity grants materialized into container.

4. **Container-specific egress**: Network configuration for this identity's
   outbound traffic.

### 6.3 Verification logic (in-container)

```rust
// In-container verification binary
fn verify_request(conn: &TcpStream) -> Result<VerifiedIdentity, VerifyError> {
    let src_ip = conn.peer_addr()?.ip();
    
    // 1. Read binding index (SHM mounted from host)
    let binding = read_binding_index()?
        .lookup(src_ip)
        .ok_or(VerifyError::NoBinding)?;
    
    // 2. Check binding not expired
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
    if now - binding.handshake_ts > 180 {
        return Err(VerifyError::BindingExpired);
    }
    
    // 3. Compute footprint from verified pubkey
    let footprint = etch_footprint(
        &binding.wg_pubkey,
        &catalog_hash(),
        mutation_index(),
        0, // port
    );
    
    // 4. Check capability grants
    let grants = load_capability_grants()?;
    if !grants.check(&footprint, &requested_capability) {
        return Err(VerifyError::CapabilityDenied);
    }
    
    Ok(VerifiedIdentity {
        pubkey: binding.wg_pubkey,
        footprint,
    })
}
```

### 6.4 Lifecycle management

| Event | Action |
| --- | --- |
| Netmaker registration | Provision identity container |
| Enrollment key revocation | Deprovision container |
| Tenant removal | Deprovision container |
| TTL expiry (configurable) | Warning notification, then deprovision |
| Manual operator action | Deprovision via D-Bus method |

**Deprovisioning steps:**
1. Stop container gracefully.
2. Remove container and its storage.
3. Optionally archive logs/audit trail.
4. Clean up any related binding entries (entries expire naturally anyway).

**Expiry policy:**
- Configurable TTL per enrollment key (default: none = no auto-expiry).
- Warning notification N days before expiry (configurable).
- Expired containers are deprovisioned automatically.

---

## 7 · Capability Grants Materialization Reliability

### 7.1 Current failure mode

During netclient-container-netns investigation, grants materialization was
found to drift after network outage:
- Durable source: `deploy/security/capability-grants.json`
- Installed: `/etc/opdbus/capability-grants.json`
- SHM: `/dev/shm/opdbus/capability-grants.json` (stale after outage)

The `opdbus-grants` runit service only materializes on startup. If it ran
before network recovery, the SHM copy missed newly-granted capabilities.

### 7.2 Reliability mechanism

```rust
// In opdbus-grants service
struct GrantsMaterializer {
    durable_path: &'static str,     // deploy/security/...
    installed_path: &'static str,   // /etc/opdbus/...
    shm_path: &'static str,         // /dev/shm/opdbus/...
}

impl GrantsMaterializer {
    /// Check freshness and re-materialize if needed
    fn ensure_fresh(&self) -> Result<()> {
        let installed_hash = blake3_file(&self.installed_path)?;
        let shm_hash = blake3_file(&self.shm_path)?;
        
        if installed_hash != shm_hash {
            warn!("Grants SHM stale: installed={} shm={}", 
                  hex(&installed_hash), hex(&shm_hash));
            self.materialize()?;
        }
        Ok(())
    }
    
    /// Force re-materialization (D-Bus callable)
    fn materialize(&self) -> Result<()> {
        let content = std::fs::read(&self.installed_path)?;
        atomic_write(&self.shm_path, &content)?;
        info!("Grants materialized: {} bytes", content.len());
        Ok(())
    }
}
```

### 7.3 Consumer-side verification

`load_capability_grants()` should verify freshness before trusting:

```rust
fn load_capability_grants() -> Result<CapabilityGrants> {
    let shm_path = "/dev/shm/opdbus/capability-grants.json";
    let installed_path = "/etc/opdbus/capability-grants.json";
    
    // Verify freshness
    if blake3_file(shm_path)? != blake3_file(installed_path)? {
        warn!("Grants stale, triggering rematerialization");
        // Call opdbus-grants.Rematerialize via D-Bus
        rematerialize_via_dbus()?;
    }
    
    // Load from SHM
    let content = std::fs::read_to_string(shm_path)?;
    serde_json::from_str(&content)
}
```

---

## 8 · Integration with Existing Components

### 8.1 IdentitySled unchanged

The existing 152-byte IdentitySled layout is preserved. The TransportBindingIndex
is a new, separate SHM file (`/dev/shm/opdbus/transport-binding.dat`).

### 8.2 Handshake watcher extended (wg-lan scoped)

`watch_wireguard_handshakes()` gains a second responsibility. The watcher runs
in `op-identity-shuttle` with `WG_INTERFACE=wg-lan`:

```rust
async fn watch_wireguard_handshakes(iface: &str) {
    // iface = "wg-lan" (from WG_INTERFACE env var)
    
    // Existing: update IdentitySled
    write_sled_from_wg(&sled_path, &pubkey, mutation_index)?;
    
    // NEW: update TransportBindingIndex
    let allowed_ips = parse_allowed_ips(&wg_show_output);
    for ip in allowed_ips {
        binding_index.upsert(BindingEntry {
            src_ip: ip,
            src_port: 0,  // Any port from this IP
            wg_pubkey: pubkey,
            handshake_ts: now,
            footprint: etch_footprint(&pubkey, &catalog_hash, mutation_index, 0),
            flags: BINDING_VALID,
        })?;
    }
}
```

### 8.3 WireGuard plugin integration

`WireGuardPlugin` in `wireguard.rs` is extended to emit D-Bus signals:

```rust
impl WireGuardPlugin {
    fn add_peer(&mut self, peer: WireGuardPeer) -> Result<()> {
        // Existing peer CRUD
        self.peers.insert(peer.public_key.clone(), peer.clone());
        
        // NEW: emit signal for identity system
        self.emit_signal("PeerAdded", &PeerAddedSignal {
            interface: self.interface.clone(),
            public_key: peer.public_key,
            allowed_ips: peer.allowed_ips,
        })?;
        
        Ok(())
    }
}
```

The handshake watcher can subscribe to these signals instead of/in addition to
polling `wg show`.

---

## 9 · What xray Does NOT Do

**xray remains a pure passthrough proxy.** Per the live config at
`/etc/xray/xray_config.json` (inside container only, per AGENTS.md):

- Public-facing inbound (`xhttp-in`, port 8444): `"security": "none"` with
  `"sniffing": {"routeOnly": true}` — xray only peeks at SNI/protocol to
  route, never decrypts.

- **xray cannot inject HTTP headers into passthrough TLS** without becoming
  a full TLS-terminating MITM proxy. This is out of scope and unworkable for
  the privacy-tunnel use case.

- The previous design's xray Go plugin/sidecar approach is **removed**. The
  invalid xray-core JSON example (`"proxySettings": {"tag": ...}` on a freedom
  outbound) is also removed — real outbound chaining uses
  `streamSettings.sockopt.dialerProxy`, but this is irrelevant since we're not
  doing xray-side injection at all.

- **tonic's TLS** (in `op-grpc-bridge`) is a completely separate TLS boundary
  from xray's, terminated inside the Rust gRPC server itself.

---

## 10 · Out of Scope (Explicit)

| Traffic type | Path | Status |
| --- | --- | --- |
| Customer/subscriber privacy tunnels | xray passthrough (SNI routing) | **Untouched** — MUST remain pure passthrough |
| Mail (mail-3tched) | incus proxy devices | **Untouched** — separate ingress/egress |
| Qdrant | incus proxy devices | **Untouched** — separate ingress/egress |
| Netmaker mesh | OP_NETMK_* iptables chains | **Untouched** — continues IP-ACL model |
| `assistant` container (port 8090) | Host-local dokodemo-door | **Untouched** — already decided |

---

## 11 · Failure Modes

| Failure | Behavior |
| --- | --- |
| No binding for src_ip | Reject request (fail-closed) |
| Binding expired | Reject until re-handshake |
| SHM grants stale | Auto-rematerialize, retry |
| op-identity-shuttle crash | Runit restarts; bindings expire naturally |
| Identity container crash | Incus restarts; requests rejected until up |
| wg-lan down | No handshakes observed; bindings expire |
| IdentitySled corrupt | Reject all until repaired |
| Container provisioning fails | Registration fails; no identity container |

---

## 12 · Verification Model

### 12.1 Binding verification

```bash
# Verify binding exists for wg-lan peer
wg show wg-lan latest-handshakes
# Output: <pubkey>  <timestamp>

# Verify binding in SHM
opctl binding lookup --ip 10.200.1.5
# Output: pubkey=<base64>, handshake=<ts>, footprint=<hex>
```

### 12.2 Identity container verification

```bash
# From wg-lan peer, make request through identity container
curl -v https://identity-container/api/test 2>&1 | grep "HTTP/"
# Expected: 200 OK (verified identity, authorized)

# Attempt from unbound IP
curl -v https://identity-container/api/test 2>&1
# Expected: 403 Forbidden (no binding)
```

### 12.3 Replay prevention verification

```bash
# Different source IP cannot reuse another peer's binding
# (binding lookup is by source IP, not by presented credentials)
```

---

## 13 · Implementation Order

1. **op-identity-shuttle runit service** — Get the watcher running with `WG_INTERFACE=wg-lan`
2. **TransportBindingIndex** — SHM data structure and read/write primitives
3. **Handshake watcher extension** — Update binding on handshake (wg-lan scoped)
4. **Grants materialization reliability** — Staleness detection and recovery
5. **Per-registration container provisioning** — Triggered by netmaker registration
6. **In-container verification logic** — Rust binary for binding lookup and grants check
7. **Container lifecycle management** — Deprovision on revocation/expiry
8. **WireGuard plugin integration** — D-Bus signals for peer events
9. **End-to-end verification** — All V-* tests pass
