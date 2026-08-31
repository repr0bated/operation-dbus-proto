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


### 6.1 Provisioning trigger

Container provisioning is triggered by netmaker's enrollment-key/registration flow:

1. New peer registers via enrollment key (join-token flow).
2. Registration creates entry in `enrollment_keys_v1`/`tenants_v1` tables.
3. **NEW**: Registration event triggers identity container provisioning.

```rust
// Pseudocode for provisioning trigger
async fn on_netmaker_registration(event: RegistrationEvent) {
    let container_name = format!("identity-{}", event.tenant_id);
    let alias = generate_petname_alias(&event.tenant_id, &event.wg_pubkey);
    
    // Create container with:
    // - Read-only bind mount of /dev/shm/opdbus/transport-binding.dat
    // - Verification binary (Rust)
    // - Container-specific egress network config
    // - Capability grants for this identity
    // - Dedicated OVS port with identity register value
    // - Human-friendly alias for display
    
    provision_identity_container(&container_name, &alias, &event)?;
}
```

### 6.2 Container contents

Each identity container contains:

1. **Transport binding index reader**: Read-only access to host's
   `/dev/shm/opdbus/transport-binding.dat` via bind mount.

2. **Verification binary** (Rust, per CLAUDE.md "Rust-first"):
   - Reads source IP from incoming connection
   - Looks up binding in transport index
   - Optionally verifies OpenFlow register/ct_mark matches expected value
   - Computes footprint from verified pubkey
   - Checks capability grants
   - Rejects if no valid binding (fail-closed)

3. **Capability grants**: Per-identity grants materialized into container.

4. **Container-specific egress** (**consumer** direction): Network configuration
   for this identity's outbound traffic. Each container initiates its own
   outbound connections, isolated from other containers.

5. **Human-friendly alias**: A deterministic petname-style identifier (e.g.,
   `curious-falcon`) derived from Blake3(tenant_id ‖ wg_pubkey), truncated and
   mapped to adjective-noun pairs. Used in:
   - Container labels/metadata (visible in `incus list`)
   - Log entries and audit trails
   - Operator dashboards and alerts

**Constraint — Alias is identifying, not authenticating**: The human-friendly
alias is strictly for display and reference. It MUST NEVER be:
- Accepted as input to verification logic
- Used in capability-grants matching
- Used in footprint computation or comparison
- Trusted as proof of identity

Only the Blake3-derived footprint from the verified WG pubkey is authoritative
for authorization decisions. The alias exists solely to make logs and container
lists human-readable.

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
| Enrollment key revocation | Deprovision container (permanent removal) |
| Tenant removal | Deprovision container |
| TTL expiry (configurable) | Warning notification, then deprovision |
| Manual operator action | Deprovision via D-Bus method |

**Deprovisioning steps:**
1. Stop container gracefully.
2. Remove container and its storage.
3. Optionally archive logs/audit trail.
4. Clean up any related binding entries (entries expire naturally anyway).
5. Remove dedicated OVS port and associated OpenFlow rules.

**Expiry policy:**
- Configurable TTL per enrollment key (default: none = no auto-expiry).
- Warning notification N days before expiry (configurable).
- Expired containers are deprovisioned automatically.

### 6.5 Extended lifecycle actions — scope decisions

| Action | Decision | Reason |
| --- | --- | --- |
| **Workspace upgrade** | **Backlog** | Requires container image versioning strategy and hot-reload mechanism not yet designed. Would need to define how verification binary updates propagate without service interruption. |
| **OS upgrade** | **Backlog** | Requires base image versioning strategy. Depends on incus image management workflow not yet established. Full reprovisioning is acceptable for now. |
| **Account activation** | **Backlog** | Re-enabling a deactivated identity could reuse existing container state if preserved, but requires defining what "deactivated" means at the container level vs. just blocking at grants. Defer until deactivation semantics are clear. |
| **Account deactivation** | **In-scope** | Suspending access without full deprovisioning. Distinct from revocation (L-1, permanent removal). See §6.6. |

### 6.6 Account deactivation (in-scope)

Account deactivation suspends an identity's access without destroying its container:

1. **Mechanism**: Set a `deactivated` flag in the identity's metadata (stored in
   container labels or a host-side state file).
2. **Enforcement**: The in-container verification binary checks the deactivation
   flag before processing any request. If deactivated, reject with a specific
   error code (e.g., `VerifyError::AccountDeactivated`).
3. **OpenFlow**: Optionally, remove or modify the container's OpenFlow rules to
   drop traffic at the datapath level (defense in depth).
4. **Reactivation**: Clear the flag via D-Bus method. Verification resumes
   immediately without reprovisioning.
5. **Distinction from revocation**: Deactivation is reversible and preserves
   container state. Revocation (L-1 in tasks.md) is permanent removal.

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
