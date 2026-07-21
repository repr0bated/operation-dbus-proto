# Dropped Excerpts from WG-SESSION-ID.md

This file contains implementation-specific sections dropped during consolidation.

**Source:** `/mnt/opt-inspect/home/git/operation-dbus-proto/docs/WG-SESSION-ID.md`  
**Extracted:** 2026-07-20  
**Reason:** Implementation details superseded by current architecture (D-Bus control plane, not JSON-RPC service)

---

## Dropped: wg-auth-service Implementation

### Service Architecture Diagram
```
┌─────────────────────────────────────────────────────────────┐
│                    wg-auth-service (Rust)                   │
│  ┌─────────────────────────────────────────────────────┐    │
│  │          Main Tokio Runtime                         │    │
│  │  • JSON-RPC over D-Bus (zbus)                      │    │
│  │  • Async request handling                          │    │
│  │  • Connection pool management                      │    │
│  └─────────────────────────────────────────────────────┘    │
│                    │                                         │
│                    ▼                                         │
│  ┌─────────────────────────────────────────────────────┐    │
│  │          JSON-RPC Dispatcher                        │    │
│  │  • Request/response handling                       │    │
│  │  • Method routing                                  │    │
│  │  • Error conversion                                │    │
│  └─────────────────────────────────────────────────────┘    │
│                    │                                         │
│                    ▼                                         │
│  ┌─────────────────────────────────────────────────────┐    │
│  │          Service Modules                           │    │
│  │  • AuthManager: Key rotation                       │    │
│  │  • SessionManager: Session tracking                │    │
│  │  • CryptoEngine: Cryptographic operations          │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

### Cargo.toml
```toml
[package]
name = "wg-auth-service"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1.35", features = ["full"] }
zbus = { version = "4.0", features = ["tokio", "json"] }
simd-json = { version = "0.12" }
jsonrpc-core = "18.0"
# ... (full dependencies omitted)
```

### JSON-RPC Interface Definition
```rust
use jsonrpc_core::{Error, ErrorCode, Result};
use jsonrpc_derive::rpc;

#[rpc]
pub trait WireGuardAuthRpc {
    #[rpc(name = "auth_getStatus")]
    fn get_status(&self) -> Result<HashMap<String, String>>;

    #[rpc(name = "auth_rotateKey")]
    fn rotate_key(&self, params: KeyRotationParams) -> Result<KeyRotationResult>;

    #[rpc(name = "auth_createSession")]
    fn create_session(&self, peer_pubkey: String) -> Result<SessionInfo>;

    // ... (10+ methods omitted)
}
```

### Main Service Implementation
```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt().json().init();

    // Load configuration
    let config = Config::load().await?;

    // Create JSON-RPC handler
    let rpc_handler = WireGuardAuthRpcImpl::new(/* ... */);
    let mut io = MetaIoHandler::with_compatibility(jsonrpc_core::Compatibility::V2);
    io.extend_with(rpc_handler.to_delegate());

    // Start HTTP JSON-RPC server if configured
    if let Some(http_config) = &config.http {
        let http_server = ServerBuilder::new(io.clone())
            .start_http(&http_config.listen_addr)?;
        // ...
    }

    // Start D-Bus service with JSON-RPC interface
    let dbus_connection = ConnectionBuilder::system()?
        .name("org.freedesktop.WireGuardAuth1")?
        .serve_at("/org/freedesktop/WireGuardAuth1", rpc_handler)?
        .build()
        .await?;
    
    Ok(())
}
```

---

## Dropped: Cryptographic Engine Implementation

### SIMD JSON-RPC Server
```rust
use simd_json::{BorrowedValue, Mutable, OwnedValue};
use hyper::server::conn::http1;

pub struct JsonRpcServer {
    handler: Arc<dyn JsonRpcHandler + Send + Sync>,
    config: ServerConfig,
}

impl JsonRpcServer {
    pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let listener = TcpListener::bind(&self.config.listen_addr).await?;
        // ... SIMD JSON parsing implementation
    }
}
```

### SIMD-Optimized Crypto Engine
```rust
pub struct SimdCryptoEngine {
    rng: SystemRandom,
    master_key: Arc<[u8; 32]>,
}

impl SimdCryptoEngine {
    pub fn derive_keys_batch(&self, ikms: &[[u8; 32]], salt: &[u8]) -> Vec<[u8; 32]> {
        #[cfg(target_feature = "avx2")]
        {
            // AVX2 batch processing for 4 keys at a time
            // ... (SIMD implementation omitted)
        }
    }
}
```

---

## Dropped: NetworkManager Plugin

### NetworkManager Integration
```rust
pub struct NetworkManagerPlugin {
    nm_client: nm::Client,
    auth_client: Arc<AuthClient>,
}

impl NetworkManagerPlugin {
    pub async fn on_connection_activated(&self, connection_id: &str) {
        // Automatic key rotation on network activation
        let peer_pubkey = self.get_wireguard_peer(connection_id).await?;
        let result = self.auth_client.rotate_key(&peer_pubkey).await?;
        self.update_wg_quick_config(&result.psk).await?;
    }
}
```

---

## Dropped: systemd Integration

### systemd Service Unit
```ini
# /etc/systemd/system/wg-auth-service.service
[Unit]
Description=WireGuard Authentication Service
After=network.target

[Service]
Type=notify
ExecStart=/usr/bin/wg-auth-service
Restart=on-failure
User=wg-auth

[Install]
WantedBy=multi-user.target
```

### systemd Activation
```rust
use systemd::daemon;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ... initialization

    // Notify systemd of readiness
    daemon::notify(false, [(daemon::STATE_READY, "1")].iter())?;

    Ok(())
}
```

---

## Dropped: Deployment Commands

### Installation Script
```bash
#!/bin/bash
set -e

# Build service
RUSTFLAGS="-C target-cpu=native" cargo build --release --features "simd"

# Install binaries
sudo cp target/release/wg-auth-service /usr/local/bin/
sudo chmod 755 /usr/local/bin/wg-auth-service

# Install systemd service
sudo cp deployment/wg-auth-service.service /etc/systemd/system/
sudo systemctl daemon-reload

# Enable and start
sudo systemctl enable wg-auth-service
sudo systemctl start wg-auth-service
```

### Configuration Setup
```bash
# Create user and directories
sudo useradd --system --no-create-home wg-auth
sudo mkdir -p /etc/wg-auth /var/lib/wg-auth
sudo chown -R wg-auth:wg-auth /etc/wg-auth /var/lib/wg-auth
sudo chmod 700 /etc/wg-auth /var/lib/wg-auth

# Generate master key
openssl rand -hex 32 | sudo tee /etc/wg-auth/master.key
sudo chmod 600 /etc/wg-auth/master.key
```

---

## Dropped: Client Libraries

### Rust Client
```rust
pub struct AuthClient {
    jsonrpc: Option<JsonRpcClient>,
    dbus: Option<DBusClient>,
}

impl AuthClient {
    pub async fn auto_connect() -> Result<Self, Error> {
        if let Ok(client) = JsonRpcClient::connect("unix:/run/wg-auth/jsonrpc.sock").await {
            return Ok(Self { jsonrpc: Some(client), dbus: None });
        }
        // Fallback to D-Bus
    }
}
```

### Python Client
```python
import json
import socket

class WireGuardAuthClient:
    def __init__(self, socket_path="/run/wg-auth/jsonrpc.sock"):
        self.socket = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.socket.connect(socket_path)
    
    def rotate_key(self, peer_pubkey):
        request = {
            "jsonrpc": "2.0",
            "method": "auth.rotateKey",
            "params": {"peer_pubkey": peer_pubkey},
            "id": 1
        }
        self.socket.send(json.dumps(request).encode())
        response = json.loads(self.socket.recv(4096))
        return response["result"]
```

---

## Dropped: D-Bus Compatibility Bridge

### D-Bus Adapter
```rust
#[dbus_interface(name = "org.freedesktop.WireGuardAuth1")]
impl DBusAdapter {
    async fn rotate_key(&self, peer_pubkey: String) -> zbus::fdo::Result<String> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            method: "auth.rotateKey",
            params: simd_json::json!({"peer_pubkey": peer_pubkey}),
            id: Some(OwnedValue::from(1)),
        };
        
        let response = self.jsonrpc_client.call(request).await?;
        Ok(response.result["psk"].as_str().unwrap().to_string())
    }
}
```

---

## Dropped: Port References (18789)

All references to port 18789 have been replaced with 8090 in the main document.

**Original configuration:**
```toml
[jsonrpc]
listen_addr = "127.0.0.1:18789"  # Old port
```

**Updated configuration:**
```toml
[jsonrpc]
listen_addr = "127.0.0.1:8090"  # New port
```

---

## Dropped: Performance Benchmarking

### Benchmark Results
```
Messages/second (higher is better)
+-------------------+-----------+------------+
| Operation         | JSON-RPC  | D-Bus      |
+-------------------+-----------+------------+
| Parse Request     | 2.1M msg/s| 45K msg/s  |
| Serialize Response| 1.8M msg/s| 38K msg/s  |
| Round-trip        | 850K ops/s| 22K ops/s  |
+-------------------+-----------+------------+
```

### Criterion Benchmarks
```rust
fn bench_simd_json_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("simd_json_parsing");
    for size in [64, 256, 1024, 4096].iter() {
        group.bench_with_input(BenchmarkId::new("parse_request", size), size, |b, &size| {
            // ... benchmark implementation
        });
    }
}
```

---

## Summary

Dropped sections include:
- wg-auth-service standalone daemon implementation
- JSON-RPC method definitions and handlers
- SIMD-optimized JSON parsing code
- NetworkManager plugin integration
- systemd service units and activation
- Deployment scripts and installation commands
- Client libraries (Rust, Python)
- D-Bus compatibility bridge code
- Port 18789 references (updated to 8090)
- Performance benchmarks comparing JSON-RPC vs D-Bus

These implementation details are superseded by the current D-Bus plugin architecture where WireGuard identity management is integrated into the plugin control plane, not a standalone service.

<!-- Extracted from WG-SESSION-ID.md on 2026-07-20 -->
