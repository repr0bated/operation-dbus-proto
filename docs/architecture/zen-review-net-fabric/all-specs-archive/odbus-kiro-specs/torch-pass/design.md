# Torch-Pass — Design

## 1 · Design Status

This design addresses the gap between CLAUDE.md's claimed zero-trust model and
the actual implementation. The claimed model treats `X-Ghostbridge-Footprint`
as "the only real gate" with IP ACLs as "theater." Investigation reveals that
the header itself is self-asserted theater — the gap has shifted layers, not
closed.

### 1.1 Current architecture (the gap)

```text
┌─────────────────────────────────────────────────────────────────────────┐
│                         CLIENT (e.g., op-cognitive-mcp)                 │
│                                                                         │
│  ClientConfig.with_wg_pubkey(pubkey)                                    │
│       │                                                                 │
│       ▼                                                                 │
│  .header("X-Ghostbridge-Footprint", pubkey)  ◄── SELF-ASSERTED         │
│       │                                                                 │
└───────┼─────────────────────────────────────────────────────────────────┘
        │ gRPC request with self-asserted header
        ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         XRAY (static config)                            │
│                                                                         │
│  /etc/xray/xray_config.json                                             │
│       │                                                                 │
│       ▼                                                                 │
│  Route by static IP/port ACL  ◄── NO IDENTITY CHECK                    │
│       │                                                                 │
└───────┼─────────────────────────────────────────────────────────────────┘
        │
        ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                      GhostbridgeInterceptor                             │
│                                                                         │
│  Extract X-Ghostbridge-Footprint header                                 │
│       │                                                                 │
│       ▼                                                                 │
│  verify_ghostbridge_footprint()                                         │
│       │                                                                 │
│       ▼                                                                 │
│  Check header PRESENT and FORMAT valid  ◄── NO TRANSPORT BINDING       │
│       │                                                                 │
└───────┼─────────────────────────────────────────────────────────────────┘
        │
        ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                      SchemaBackedInterface::call()                      │
│                                                                         │
│  load_capability_grants(&hex::encode(sled.hashed_footprint))            │
│       │                                                                 │
│       ▼                                                                 │
│  Grant lookup and AccessDenied gate  ◄── GATE IS REAL                  │
│       │                                         BUT TRUSTS UNVERIFIED   │
│       │                                         FOOTPRINT ORIGIN        │
└───────┼─────────────────────────────────────────────────────────────────┘
        │
        ▼
     D-Bus dispatch (if granted)

SEPARATE, DISJOINT SYSTEM:

┌─────────────────────────────────────────────────────────────────────────┐
│                      WireGuard (actual identity)                        │
│                                                                         │
│  WireGuardPlugin: peer CRUD via D-Bus                                   │
│       │                                                                 │
│       ▼                                                                 │
│  wg0 interface with authenticated peers  ◄── REAL CRYPTO IDENTITY      │
│       │                                                                 │
│       ▼                                                                 │
│  Handshakes prove peer identity          ◄── NOT CONNECTED TO          │
│                                               IDENTITY-SLED/FOOTPRINT   │
└─────────────────────────────────────────────────────────────────────────┘
```

The WireGuard system **knows** peer identity through cryptographic handshakes.
The identity-sled/footprint system **trusts** self-asserted headers. These
systems are disjoint — the cryptographic proof never reaches the gate.

### 1.2 Target architecture (torch-pass)

```text
┌─────────────────────────────────────────────────────────────────────────┐
│                         WireGuard Termination                           │
│                                                                         │
│  watch_wireguard_handshakes()                                           │
│       │                                                                 │
│       ▼                                                                 │
│  Handshake completes → (pubkey, tunnel_ip, timestamp)                   │
│       │                                                                 │
│       ▼                                                                 │
│  ┌─────────────────────────────────────────────────────────────┐        │
│  │              VERIFIED-PEERS REGISTRY (SHM)                  │        │
│  │                                                             │        │
│  │  tunnel_ip → { pubkey, handshake_ts, footprint_hash }       │        │
│  │                                                             │        │
│  │  Entries expire after HANDSHAKE_STALENESS_THRESHOLD (180s)  │        │
│  └─────────────────────────────────────────────────────────────┘        │
└───────┼─────────────────────────────────────────────────────────────────┘
        │ Events: peer_verified, peer_expired, peer_removed
        ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                      Xray Config Generator                              │
│                                                                         │
│  Subscribe to registry events                                           │
│       │                                                                 │
│       ▼                                                                 │
│  Generate routing rules: tunnel_ip → identity-based outbound            │
│       │                                                                 │
│       ▼                                                                 │
│  Atomic write to /etc/xray/xray_config.json + reload                    │
│                                                                         │
│  (Until generator is verified: static bootstrap config remains)         │
└─────────────────────────────────────────────────────────────────────────┘
        │
        ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                         CLIENT REQUEST                                  │
│                                                                         │
│  gRPC request from tunnel_ip with X-Ghostbridge-Footprint header        │
│       │                                                                 │
└───────┼─────────────────────────────────────────────────────────────────┘
        │
        ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                  GhostbridgeInterceptor (MODIFIED)                      │
│                                                                         │
│  1. Extract ACTUAL source IP from connection metadata                   │
│       │                                                                 │
│       ▼                                                                 │
│  2. Query verified-peers registry by source IP                          │
│       │                                                                 │
│       ├── Not found → REJECT (unknown peer)                             │
│       │                                                                 │
│       ▼                                                                 │
│  3. Check handshake_ts within staleness threshold                       │
│       │                                                                 │
│       ├── Expired → REJECT (stale handshake)                            │
│       │                                                                 │
│       ▼                                                                 │
│  4. Validate claimed footprint against registry entry                   │
│       │                                                                 │
│       ├── Mismatch → REJECT (footprint mismatch)                        │
│       │                                                                 │
│       ▼                                                                 │
│  5. Proceed with VERIFIED footprint                                     │
│       │                                                                 │
└───────┼─────────────────────────────────────────────────────────────────┘
        │
        ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                      SchemaBackedInterface::call()                      │
│                                                                         │
│  load_capability_grants(&hex::encode(VERIFIED_footprint))               │
│       │                                                                 │
│       ▼                                                                 │
│  Grant lookup and AccessDenied gate  ◄── NOW TRUSTS VERIFIED IDENTITY  │
│       │                                                                 │
└───────┼─────────────────────────────────────────────────────────────────┘
        │
        ▼
     D-Bus dispatch
```

---

## 2 · Component Design

### 2.1 Verified-peers registry

A new SHM-backed registry that bridges the WireGuard and identity systems.

**Location**: `/dev/shm/opdbus/verified-peers.dat`

**Structure**:
```rust
#[repr(C)]
pub struct VerifiedPeersHeader {
    magic: [u8; 8],           // "VPEERS01"
    generation: u64,          // Increments on any mutation
    entry_count: u32,
    staleness_threshold_secs: u32,
    last_update_ts: u64,      // Unix timestamp
}

#[repr(C)]
pub struct VerifiedPeerEntry {
    tunnel_ip: [u8; 4],       // IPv4 tunnel address
    pubkey: [u8; 32],         // WireGuard public key
    handshake_ts: u64,        // Unix timestamp of last handshake
    footprint_hash: [u8; 32], // Pre-computed Blake3 footprint
    flags: u32,               // Reserved for future use
}
```

**Operations**:
- `verify_peer(source_ip, claimed_footprint) -> Result<VerifiedIdentity, RejectReason>`
- `update_peer(pubkey, tunnel_ip, handshake_ts)` — called by handshake watcher
- `remove_peer(pubkey)` — called on peer deletion
- `expire_stale()` — periodic cleanup of entries past threshold

**Staleness model**:
- Default threshold: 180 seconds (matches existing convention)
- Entries are not deleted on expiry, but verification fails
- Handshake refresh updates `handshake_ts` and re-enables entry
- This provides graceful handling of brief connectivity interruptions

### 2.2 Handshake watcher integration

Extend the existing `watch_wireguard_handshakes()` pattern:

```rust
pub async fn watch_wireguard_handshakes(
    interface: &str,
    registry: Arc<VerifiedPeersRegistry>,
) -> Result<(), Error> {
    // Existing: monitor `ip monitor route` + `wg show <iface> latest-handshakes`
    
    loop {
        let handshakes = wg_show_latest_handshakes(interface).await?;
        
        for (pubkey, tunnel_ip, timestamp) in handshakes {
            if timestamp > registry.get_handshake_ts(&pubkey).unwrap_or(0) {
                // New handshake detected
                let footprint = etch_footprint(&pubkey, &schema_hash, mutation_index, 0);
                registry.update_peer(pubkey, tunnel_ip, timestamp, footprint);
                
                // Emit event for xray config generator
                emit_peer_verified_event(&pubkey, &tunnel_ip);
            }
        }
        
        registry.expire_stale();
        
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}
```

### 2.3 GhostbridgeInterceptor modification

The critical change: extract actual source IP and validate against registry.

```rust
impl GhostbridgeInterceptor {
    pub fn intercept(&self, request: Request<()>) -> Result<Request<()>, Status> {
        // NEW: Extract actual source IP from connection metadata
        let source_ip = self.extract_source_ip(&request)?;
        
        // Existing: Extract claimed footprint from header
        let claimed_footprint = request
            .metadata()
            .get("x-ghostbridge-footprint")
            .ok_or_else(|| Status::unauthenticated("missing footprint"))?;
        
        // NEW: Verify against registry
        let verified = self.registry.verify_peer(source_ip, claimed_footprint)
            .map_err(|reason| match reason {
                RejectReason::UnknownPeer => 
                    Status::unauthenticated("unknown peer"),
                RejectReason::StaleHandshake => 
                    Status::unauthenticated("handshake expired"),
                RejectReason::FootprintMismatch => 
                    Status::unauthenticated("footprint mismatch"),
                RejectReason::RegistryUnavailable => 
                    Status::unavailable("identity service unavailable"),
            })?;
        
        // Proceed with VERIFIED identity
        // ... existing capability check logic using verified.footprint ...
    }
    
    fn extract_source_ip(&self, request: &Request<()>) -> Result<Ipv4Addr, Status> {
        // Extract from gRPC connection metadata / socket peer address
        // This is transport-level, not header-level
        request.extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|info| match info.peer_addr() {
                SocketAddr::V4(addr) => Ok(*addr.ip()),
                SocketAddr::V6(_) => Err(Status::invalid_argument("IPv6 not supported")),
            })
            .ok_or_else(|| Status::internal("no peer address"))?
    }
}
```

### 2.4 Capability-grants freshness

Address the staleness failure mode with generation-aware loading:

```rust
pub struct CapabilityGrants {
    grants: HashMap<String, Vec<String>>,
    generation: u64,
    loaded_at: Instant,
}

impl CapabilityGrants {
    pub fn load_with_freshness_check(
        durable_path: &Path,
        shm_path: &Path,
        max_staleness: Duration,
    ) -> Result<Self, GrantsError> {
        let shm_grants = Self::load_from_shm(shm_path)?;
        
        // Check generation staleness
        if shm_grants.loaded_at.elapsed() > max_staleness {
            // Reload from durable source
            let durable_grants = Self::load_from_file(durable_path)?;
            
            if durable_grants.hash() != shm_grants.hash() {
                // Rematerialize SHM
                Self::materialize_to_shm(&durable_grants, shm_path)?;
                return Ok(durable_grants);
            }
        }
        
        Ok(shm_grants)
    }
}
```

Add inotify-based refresh for the durable source:

```rust
pub async fn watch_grants_file(
    durable_path: PathBuf,
    shm_path: PathBuf,
    refresh_signal: broadcast::Sender<()>,
) {
    let mut watcher = notify::recommended_watcher(move |res| {
        if let Ok(Event { kind: EventKind::Modify(_), .. }) = res {
            // Rematerialize SHM from durable source
            if let Ok(grants) = CapabilityGrants::load_from_file(&durable_path) {
                let _ = grants.materialize_to_shm(&shm_path);
                let _ = refresh_signal.send(());
            }
        }
    }).unwrap();
    
    watcher.watch(&durable_path, RecursiveMode::NonRecursive).unwrap();
    
    // Keep watcher alive
    std::future::pending::<()>().await;
}
```

### 2.5 Xray config generator (new component)

A new daemon that generates xray routing config from verified peers.

**Phases**:
1. **Bootstrap (current)**: Static `/etc/xray/xray_config.json` remains in use
2. **Validation**: Generator runs in shadow mode, comparing output to static config
3. **Cutover**: Generator becomes authoritative, static config becomes fallback

```rust
pub struct XrayConfigGenerator {
    registry: Arc<VerifiedPeersRegistry>,
    template: XrayConfigTemplate,
    output_path: PathBuf,
    xray_reload: Box<dyn XrayReloader>,
}

impl XrayConfigGenerator {
    pub async fn run(&self) {
        let mut events = self.registry.subscribe_events();
        
        loop {
            tokio::select! {
                event = events.recv() => {
                    match event {
                        PeerEvent::Verified(peer) => self.add_peer_route(&peer),
                        PeerEvent::Expired(peer) => self.remove_peer_route(&peer),
                        PeerEvent::Removed(peer) => self.remove_peer_route(&peer),
                    }
                    self.regenerate_and_reload().await;
                }
                _ = tokio::time::sleep(PERIODIC_RECONCILE) => {
                    self.reconcile_all_peers().await;
                }
            }
        }
    }
    
    async fn regenerate_and_reload(&self) {
        let peers = self.registry.get_all_verified_peers();
        let config = self.template.render(&peers);
        
        // Atomic write
        let tmp_path = self.output_path.with_extension("tmp");
        std::fs::write(&tmp_path, &config)?;
        std::fs::rename(&tmp_path, &self.output_path)?;
        
        // Reload xray
        self.xray_reload.reload().await?;
    }
}
```

**Xray reload**: Uses D-Bus signal to `op-xray-daemon`, which sends SIGHUP to
the xray process. No direct process management.

---

## 3 · Integration Points

### 3.1 WireGuardPlugin → Registry

Currently disjoint. New integration:

```rust
impl WireGuardPlugin {
    pub async fn add_peer(&self, peer: WireGuardPeer) -> Result<(), Error> {
        // Existing: configure WireGuard peer
        self.wg_client.add_peer(&peer).await?;
        
        // NEW: Seed registry entry (unverified until handshake)
        self.registry.seed_peer(peer.public_key, peer.allowed_ips[0]);
        
        Ok(())
    }
    
    pub async fn remove_peer(&self, pubkey: &[u8; 32]) -> Result<(), Error> {
        // Existing: remove WireGuard peer
        self.wg_client.remove_peer(pubkey).await?;
        
        // NEW: Remove from registry
        self.registry.remove_peer(pubkey);
        
        Ok(())
    }
}
```

### 3.2 op-xray-daemon extension

Currently lifecycle-only. Add config reload trigger:

```rust
// New D-Bus method
#[dbus_interface(name = "software.zeroclaw.XrayDaemon")]
impl XrayDaemonDbus {
    // Existing
    async fn start(&self) -> fdo::Result<()>;
    async fn stop(&self) -> fdo::Result<()>;
    async fn status(&self) -> fdo::Result<String>;
    
    // NEW: Reload configuration
    async fn reload_config(&self) -> fdo::Result<()> {
        // Send SIGHUP to xray process
        let pid = self.find_xray_pid()?;
        nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(pid),
            nix::sys::signal::Signal::SIGHUP
        )?;
        Ok(())
    }
}
```

---

## 4 · Failure Modes and Recovery

| Failure | Behavior | Recovery |
| --- | --- | --- |
| Registry SHM corrupted | Fail closed, reject all | Restart handshake watcher to rebuild |
| Handshake watcher crash | Existing entries honor TTL | Restart watcher; entries refresh on next handshake |
| Xray config generator crash | Static config remains | Restart generator; reconcile on startup |
| Grants file deleted | Fail closed after staleness | Restore file; grants refresh via inotify |
| WireGuard interface down | No new handshakes | Existing entries expire; restore interface |
| Clock skew | Handshake timestamps invalid | Use monotonic time for staleness, UTC for audit |

---

## 5 · Migration Path

### Phase 1: Registry and verification (non-breaking)

1. Deploy verified-peers registry alongside existing system
2. Deploy modified interceptor that logs verification results but does not reject
3. Monitor for mismatches between header trust and registry verification
4. Duration: 1 week of shadow mode

### Phase 2: Verification enforcement

1. Enable rejection on verification failure
2. Keep static xray config
3. Monitor rejection rates and false positives
4. Duration: Until stable (target: 1 week)

### Phase 3: Dynamic xray config (breaking for static config)

1. Deploy xray config generator in shadow mode
2. Compare generated config against static config
3. Cut over to generated config
4. Retain static config as fallback
5. Duration: Until stable (target: 2 weeks)

### Rollback

Each phase is independently reversible:
- Phase 3: Restore static config, disable generator
- Phase 2: Disable rejection, return to log-only
- Phase 1: Remove registry, restore original interceptor

---

## 6 · Security Considerations

### 6.1 Threat model

| Threat | Mitigation |
| --- | --- |
| Spoofed header from non-WG source | Registry lookup by actual source IP fails |
| Spoofed header claiming different peer | Footprint mismatch with registry entry |
| Replay of captured request | Handshake staleness check fails if old |
| Registry tampering | SHM permissions (root:opdbus 0640) |
| Grants tampering | Durable source is authoritative; SHM is derived |

### 6.2 Trust boundaries

```text
UNTRUSTED                      TRUST BOUNDARY                    TRUSTED
─────────────────────────────────────────────────────────────────────────
                                     │
Client-asserted headers ─────────────┼────────────────────────────────X
                                     │                    (rejected)
                                     │
WireGuard handshake ─────────────────┼──────► Verified-peers registry
                                     │
Actual socket source IP ─────────────┼──────► Interceptor verification
                                     │
Registry + source IP match ──────────┼──────► Capability grants lookup
                                     │
Capability grants + verified ID ─────┼──────► D-Bus dispatch
                                     │
```

---

## 7 · Testing Strategy

### 7.1 Unit tests

- Registry operations: add, verify, expire, remove
- Footprint computation matches existing `etch_footprint()`
- Interceptor source IP extraction from various connection types
- Grants staleness detection and refresh

### 7.2 Integration tests

- End-to-end: WG handshake → registry update → request verification
- Xray config generation from registry state
- Grants file modification → SHM refresh

### 7.3 Negative tests

- Request from non-WG source (should reject)
- Request with mismatched footprint (should reject)
- Request after handshake expiry (should reject)
- Request during registry unavailability (should reject)

### 7.4 Performance tests

- Registry lookup latency (target: <1ms)
- Config generation time for 100 peers (target: <1s)
- Memory footprint per peer (target: <1KB)

---

## 8 · Appendix: Code Pointers

| Component | File | Relevant Lines |
| --- | --- | --- |
| Current footprint computation | `crates/op-identity/src/schema_bridge.rs` | `etch_footprint()` |
| Current handshake watcher | `crates/op-identity/src/schema_bridge.rs` | `watch_wireguard_handshakes()` |
| Current interceptor | `crates/op-grpc-bridge/src/interceptor.rs` | `GhostbridgeInterceptor` |
| Current capability gate | `crates/op-grpc-bridge/src/schema_router.rs` | `SchemaBackedInterface::call()` ~710-786 |
| Self-assertion point | `crates/op-cognitive-mcp/src/client_config.rs` | Line 568 `.header()` |
| WireGuard plugin | `crates/op-plugins/src/state_plugins/wireguard.rs` | `WireGuardPlugin` |
| Xray daemon | `crates/op-xray-daemon/src/dbus.rs` | Lifecycle methods |
| Capability grants | `deploy/security/capability-grants.json` | Grant definitions |
