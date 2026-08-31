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

---

## 2 · Current Architecture (Self-Asserted)

```text
WireGuard peer
    │
    ▼ (WG handshake verified by kernel)
┌─────────────────────────────────────────────────────┐
│ netmaker/WG termination                             │
│   watch_wireguard_handshakes() → IdentitySled       │
│   (pubkey IS verified here)                         │
└─────────────────────────────────────────────────────┘
    │
    ▼ (inner IP traffic, no identity binding)
┌─────────────────────────────────────────────────────┐
│ xray proxy                                          │
│   NO header injection                               │
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

## 3 · Target Architecture (Verified Binding)

```text
WireGuard peer
    │
    ▼ (WG handshake verified by kernel)
┌─────────────────────────────────────────────────────┐
│ netmaker/WG termination                             │
│   watch_wireguard_handshakes() → IdentitySled       │
│   NEW: → TransportBindingIndex                      │
│         (src_ip:port, wg_pubkey, handshake_ts)      │
└─────────────────────────────────────────────────────┘
    │
    ▼ (inner IP traffic with known src IP:port)
┌─────────────────────────────────────────────────────┐
│ xray proxy                                          │
│   NEW: header injection based on binding lookup     │
│   1. Strip any client X-Ghostbridge-* headers       │
│   2. Lookup src_ip:port in TransportBindingIndex    │
│   3. Inject verified headers if binding valid       │
│   4. Reject if no valid binding (fail-closed)       │
└─────────────────────────────────────────────────────┘
    │
    ▼ (verified headers now present)
┌─────────────────────────────────────────────────────┐
│ GhostbridgeInterceptor                              │
│   validates xray-injected headers                   │
│   trusts xray as the injection point                │
└─────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────┐
│ SchemaBackedInterface::call()                       │
│   capability grants check on VERIFIED footprint     │
└─────────────────────────────────────────────────────┘
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
    src_ip: [u8; 4],         // Inner WG source IP
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

1. **Creation**: `watch_wireguard_handshakes()` observes new handshake via
   `wg show <iface> latest-handshakes`, extracts pubkey and allowed-IPs,
   writes binding entry with current timestamp.

2. **Lookup**: xray queries binding by source IP (port may be 0 for "any port
   from this IP" if WireGuard allowed-IPs defines a /32).

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

## 5 · xray Header Injection

### 5.1 Injection point

xray-core supports custom inbound handlers and routing rules. The injection
happens at the **inbound handler** for traffic arriving via WireGuard tunnels,
before routing to destinations.

Configuration approach (in `/etc/xray/xray_config.json`):

```json
{
  "inbounds": [{
    "tag": "wg-inbound",
    "port": 10800,
    "protocol": "dokodemo-door",
    "settings": { "network": "tcp,udp", "followRedirect": true },
    "sniffing": { "enabled": true },
    "streamSettings": {
      "sockopt": {
        "mark": 51821
      }
    }
  }],
  "routing": {
    "rules": [{
      "type": "field",
      "inboundTag": ["wg-inbound"],
      "outboundTag": "identity-inject"
    }]
  },
  "outbounds": [{
    "tag": "identity-inject",
    "protocol": "freedom",
    "settings": {},
    "proxySettings": {
      "tag": "actual-destination"
    }
  }]
}
```

### 5.2 Header injection mechanism

Two approaches, in order of preference:

**Option A: xray plugin (preferred)**

Write a custom xray transport/protocol plugin in Go that:
1. Intercepts inbound connections
2. Queries the TransportBindingIndex via Unix socket
3. Injects headers into HTTP requests
4. Strips client-provided identity headers
5. Forwards to actual destination

**Option B: Sidecar proxy**

Deploy a Rust sidecar between xray and destinations that:
1. Receives traffic from xray
2. Queries TransportBindingIndex
3. Injects/strips headers
4. Forwards to destinations

Option A is preferred because it avoids an extra hop and integrates with
xray's existing connection tracking.

### 5.3 Header stripping

Before injection, the handler MUST strip:
- `X-Ghostbridge-Footprint`
- `X-WireGuard-Pubkey`
- `X-Ghostbridge-Trace-Id`

This prevents header stuffing attacks where a malicious client includes
headers that would be passed through unmodified.

---

## 6 · Capability Grants Materialization Reliability

### 6.1 Current failure mode

During netclient-container-netns investigation, grants materialization was
found to drift after network outage:
- Durable source: `deploy/security/capability-grants.json`
- Installed: `/etc/opdbus/capability-grants.json`
- SHM: `/dev/shm/opdbus/capability-grants.json` (stale after outage)

The `opdbus-grants` runit service only materializes on startup. If it ran
before network recovery, the SHM copy missed newly-granted capabilities.

### 6.2 Reliability mechanism

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

### 6.3 Consumer-side verification

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

## 7 · Integration with Existing Components

### 7.1 IdentitySled unchanged

The existing 152-byte IdentitySled layout is preserved. The TransportBindingIndex
is a new, separate SHM file (`/dev/shm/opdbus/transport-binding.dat`).

### 7.2 Handshake watcher extended

`watch_wireguard_handshakes()` gains a second responsibility:

```rust
async fn watch_wireguard_handshakes(iface: &str) {
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

### 7.3 WireGuard plugin integration

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

### 7.4 GhostbridgeInterceptor trust model

The interceptor's trust model changes:

**Before**: Trust any valid header (vulnerable to forgery)
**After**: Trust headers only when they match expected injection point

```rust
fn verify_identity(req: &Request, expected_injector: SocketAddr) -> Result<Identity> {
    // Verify request came through xray (the injection point)
    if req.peer_addr() != expected_injector {
        return Err(Error::UnexpectedSource);
    }
    
    // Now we can trust the headers were injected by xray
    let footprint = req.header("X-Ghostbridge-Footprint")?;
    let pubkey = req.header("X-WireGuard-Pubkey")?;
    
    // Validate against sled (as before, but now trustworthy)
    verify_against_sled(&footprint, &pubkey)
}
```

---

## 8 · xray Config Generator

### 8.1 Generator responsibilities

```rust
struct XrayConfigGenerator {
    template_path: PathBuf,
    output_path: PathBuf,  // /etc/xray/xray_config.json
    binding_socket: PathBuf,
}

impl XrayConfigGenerator {
    fn generate(&self, peers: &[VerifiedPeer]) -> Result<XrayConfig> {
        let mut config = self.load_template()?;
        
        // Add identity-aware inbound handler
        config.inbounds.push(self.wg_inbound_handler());
        
        // Add routing rules for identity injection
        config.routing.rules.push(self.identity_routing_rule());
        
        // Add identity injection outbound
        config.outbounds.push(self.identity_inject_outbound());
        
        // Validate before returning
        self.validate(&config)?;
        
        Ok(config)
    }
    
    fn apply(&self, config: &XrayConfig) -> Result<()> {
        // Atomic write
        let json = serde_json::to_string_pretty(config)?;
        atomic_write(&self.output_path, json.as_bytes())?;
        
        // Reload via D-Bus
        let proxy = XrayDaemonProxy::new()?;
        proxy.reload()?;
        
        // Verify reload succeeded
        if !proxy.is_healthy()? {
            return Err(Error::ReloadFailed);
        }
        
        Ok(())
    }
}
```

### 8.2 Static bootstrap coexistence

Until the generator is deployed and verified, the static bootstrap at
`/etc/xray/xray_config.json` remains correct. The generator:

1. Reads the existing config as a template
2. Adds identity-aware components
3. Preserves existing routing/proxy rules
4. Writes back atomically

This allows incremental deployment without breaking existing functionality.

---

## 9 · Failure Modes

| Failure | Behavior |
| --- | --- |
| No binding for src_ip | Reject request (fail-closed) |
| Binding expired | Reject until re-handshake |
| SHM grants stale | Auto-rematerialize, retry |
| xray config invalid | Reject write, keep existing |
| xray reload fails | Rollback config, alert |
| Handshake watcher crash | Bindings expire naturally |
| IdentitySled corrupt | Reject all until repaired |

---

## 10 · Verification Model

### 10.1 Binding verification

```bash
# Verify binding exists for WG peer
wg show wg0 latest-handshakes
# Output: <pubkey>  <timestamp>

# Verify binding in SHM
opctl binding lookup --ip 10.200.1.5
# Output: pubkey=<base64>, handshake=<ts>, footprint=<hex>
```

### 10.2 Header injection verification

```bash
# From WG peer, make request and capture headers at destination
curl -v https://destination/api/test 2>&1 | grep X-Ghostbridge

# Expected: headers present, matching binding
# X-Ghostbridge-Footprint: <hex>
# X-WireGuard-Pubkey: <base64>
```

### 10.3 Replay prevention verification

```bash
# Capture valid headers
HEADERS=$(curl -s -D - https://destination/api/test | grep X-Ghostbridge)

# Attempt replay from different source
curl -H "$HEADERS" https://destination/api/test
# Expected: rejected (headers stripped and re-injected from different/no binding)
```

---

## 11 · Implementation Order

1. **TransportBindingIndex** — SHM data structure and read/write primitives
2. **Handshake watcher extension** — Update binding on handshake
3. **Grants materialization reliability** — Staleness detection and recovery
4. **xray header injection** — Plugin or sidecar approach
5. **xray config generator** — Atomic generation and reload
6. **WireGuard plugin integration** — D-Bus signals for peer events
7. **Interceptor trust model** — Verify injection point
8. **End-to-end verification** — All VR-* tests pass
