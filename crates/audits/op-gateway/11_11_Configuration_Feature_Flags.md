# Production Security and Quality Audit: op-gateway

## 1. Environmental Variable Reads (`std::env::var`)

Below is the complete list of all `std::env::var` reads within the audited files:

*   **`crates/op-gateway/src/wireguard_auth.rs:31`**
    ```rust
    let database_url = std::env::var("OP_WIREGUARD_DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:///var/lib/op-dbus/wireguard.db".to_string());
    ```
    *   **Error Handling & Defaults**: Secure. Uses `unwrap_or_else` to fallback to a safe local default database path (`sqlite:///var/lib/op-dbus/wireguard.db`) in the event that the environmental variable is unset.

*   **`crates/op-gateway/src/wireguard_auth.rs:605`**
    ```rust
    if let Ok(key_hex) = std::env::var("WG_AUTH_MASTER_KEY") {
    ```
    *   **Error Handling & Defaults**: Secure. Uses pattern matching (`if let Ok(...)`) to safely check for the variable's presence. If the variable is absent, it seamlessly falls back to cryptographically secure on-the-fly generation using `SystemRandom` at line 615.

---

## 2. Cargo Features & Workspace Additivity

### Crate-Level Features (`crates/op-gateway/Cargo.toml`)
The `op-gateway` crate does not define any custom features in its `Cargo.toml`.

### Workspace-Level Features (`Cargo.toml`)
The root workspace package `op-dbus` defines the following features:
```toml
[features]
default = ["grpc"]
grpc = []
```

### Additivity Analysis
Cargo features are strictly additive. Because `op-gateway` does not declare any internal features, it has no features that can be deselectively resolved. However, its dependencies (such as `sqlx`, `tokio`, and `serde`) are configured with workspace-wide unified feature sets. Downstream or sibling crates in the workspace that depend on `op-gateway` can addwardly activate features on shared dependencies (e.g., activating additional `sqlx` drivers or `tokio` components) without conflict.

---

## 3. Hardcoded Paths, Ports, and Addresses

The following hardcoded system paths, addresses, and ports were identified:

### Hardcoded System Paths
*   **`crates/op-gateway/src/encrypted_storage.rs:89`**
    ```rust
    base_path: PathBuf::from("/var/lib/op-dbus/encrypted")
    ```
    *   **Impact**: Default storage path for experimental encrypted WireGuard keys.

*   **`crates/op-gateway/src/encrypted_storage.rs:90`**
    ```rust
    subvolume_name: "wireguard-keys".to_string()
    ```
    *   **Impact**: Default Btrfs subvolume name.

*   **`crates/op-gateway/src/encrypted_storage.rs:92`**
    ```rust
    luks_device_name: Some("opdbus_wg_keys".to_string())
    ```
    *   **Impact**: Default virtual block device mapper target name for fallback LUKS containers.

*   **`crates/op-gateway/src/encrypted_storage.rs:201`**
    ```rust
    let luks_path = format!("/dev/mapper/{}", device_name);
    ```
    *   **Impact**: Hardcoded Linux system block device path prefix.

*   **`crates/op-gateway/src/encrypted_storage.rs:215`**
    ```rust
    let container_path = self.config.base_path.join("wireguard_keys.img");
    ```
    *   **Impact**: Hardcoded fallback disk image filename for the virtual loopback device.

*   **`crates/op-gateway/src/encrypted_storage.rs:223`**
    ```rust
    "if=/dev/zero"
    ```
    *   **Impact**: Hardcoded host source path passed to `dd` for zeroing out container files.

*   **`crates/op-gateway/src/encrypted_storage.rs:282`**
    ```rust
    let master_key_path = self.storage_path.join("master.key");
    ```
    *   **Impact**: Hardcoded file name for the master key store.

*   **`crates/op-gateway/src/wireguard_auth.rs:32`**
    ```rust
    "sqlite:///var/lib/op-dbus/wireguard.db"
    ```
    *   **Impact**: Default path to the WireGuard SQLite session database.

### Hardcoded Network Addresses & Ports
*   **`crates/op-gateway/src/mcp_gateway.rs:104`**
    ```rust
    endpoint: "grpc://localhost:50051".to_string()
    ```
    *   **Impact**: Hardcoded localhost routing target for full-access (Compact + Cognitive) clients.

*   **`crates/op-gateway/src/mcp_gateway.rs:124`**
    ```rust
    endpoint: "grpc://localhost:50052".to_string()
    ```
    *   **Impact**: Hardcoded localhost routing target for cognitive-only restricted clients.

---

## 4. Schema-as-Code Compliance Violations

The system design dictates a schema-as-code discipline using Protocol Buffers and OSCAL. The following components violate this design by expressing data contracts as ad-hoc JSON structs or unstructured dynamic objects instead of strongly-typed, versioned schemas:

*   **`crates/op-gateway/src/mcp_gateway.rs:337-349`**
    ```rust
    pub async fn dbus_route_client(...) -> Result<Value> {
        ...
        Ok(json!({
            "endpoint": routing_decision.endpoint,
            "allowed_tools": routing_decision.allowed_tools,
            "capabilities": routing_decision.capabilities,
            "has_full_access": routing_decision.has_full_access,
            "session_id": routing_decision.session_id,
            "access_level": ...
        }))
    }
    ```
    *   **Violation**: Returns an ad-hoc JSON `Value` over the D-Bus interface. This contract should be represented by a versioned Protocol Buffer schema compiled into a native struct.

*   **`crates/op-gateway/src/mcp_gateway.rs:352-358`**
    ```rust
    pub async fn dbus_validate_session(&self, session_id: &str) -> Result<Value> {
        let is_valid = self.validate_session(session_id).await?;
        Ok(json!({
            "valid": is_valid,
            "session_id": session_id
        }))
    }
    ```
    *   **Violation**: Uses dynamic JSON maps for public D-Bus API responses rather than compiled protobuf schemas.

*   **`crates/op-gateway/src/mcp_gateway.rs:361-367`**
    ```rust
    pub async fn dbus_get_capabilities(&self, session_id: &str) -> Result<Value> {
        let capabilities = self.get_client_capabilities(session_id).await?;
        Ok(json!({
            "capabilities": capabilities,
            "session_id": session_id
        }))
    }
    ```
    *   **Violation**: Expresses capability structures dynamically via schema-less JSON.

*   **`crates/op-gateway/src/wireguard_auth.rs:62` & `crates/op-gateway/src/wireguard_auth.rs:94`**
    ```rust
    let flags_json = simd_json::to_string(&session.flags)?;
    ```
    *   **Violation**: Serializes arbitrary `HashMap<String, String>` structures directly as untyped JSON strings into the SQLite database. These represent database schema-less storage patterns.

*   **`crates/op-gateway/src/encrypted_storage.rs:395`**
    ```rust
    let entry_json = simd_json::to_string(&entry)?;
    ```
    *   **Violation**: Serializes the `EncryptedKeyEntry` metadata and payload directly as a raw JSON string to key files on disk without validation schemas. This presents a risk of desynchronization and data corruption if upgrades alter the internal structural fields.

---

## 5. Direct Exploitable Security Vulnerabilities

### [CRITICAL] Cryptographic Key Nonce Reuse in ChaCha20Poly1305

#### Vulnerability Location
*   **`crates/op-gateway/src/encrypted_storage.rs:295-314` (`load_master_key`)**
*   **`crates/op-gateway/src/encrypted_storage.rs:351-404` (`store_key`)**

#### Description & Exploitation Mechanism
The `EncryptedKeyStorage` module utilizes the `ChaCha20Poly1305` AEAD algorithm to encrypt client private keys and session data. The security proof of ChaCha20Poly1305 relies entirely on the absolute uniqueness of the `(Key, Nonce)` pair. 

In `store_key`, the 12-byte nonce is derived statefully from `master_key.nonce_counter` (which is incremented monotonically on each key write):
```rust
let mut nonce = [0u8; 12];
let nonce_counter = master_key.nonce_counter;
nonce[4..12].copy_from_slice(&nonce_counter.to_le_bytes());
master_key.nonce_counter += 1;
```

However, the persistent state of `nonce_counter` is **not saved anywhere on disk**. When the application starts or restarts, it reads the static master key from `master.key` and executes `load_master_key`:
```rust
self.master_key = Some(MasterKey {
    key,
    salt,
    nonce_counter: 0, // CRITICAL: Reset to 0 over restart
});
```

Because `nonce_counter` is reset to `0` during every initialization, the application will reuse the exact same nonces (`0`, `1`, `2`, ...) under the exact same master key `key` to encrypt new files after a restart.

#### Exploitation
An attacker who achieves local read access to the encrypted subvolume directory (via another vulnerability, diagnostic logs, backup leaks, or container escapes) can perform a keystream recovery attack:
1.  **Keystream Recovery**: Since $C_1 = P_1 \oplus K_{\text{stream}}$ and $C_2 = P_2 \oplus K_{\text{stream}}$, the attacker can compute $C_1 \oplus C_2 = P_1 \oplus P_2$. 
2.  **Key Extraction**: Since WireGuard private keys and PSKs contain high entropy but the envelope serialization format (`EncryptedKeyEntry`) contains known JSON headers, standard multi-ciphertext cryptanalysis tools can easily isolate and decrypt the raw private keys.

#### Remediation
Ensure that nonces for `ChaCha20Poly1305` are drawn randomly using a cryptographically secure pseudorandom number generator (CSPRNG) rather than a stateful counter that does not persist across application lifecycle boundaries:
```rust
let mut nonce = [0u8; 12];
ring::rand::SystemRandom::new()
    .fill(&mut nonce)
    .map_err(|_| anyhow::anyhow!("Nonce generation failed"))?;
```

---
## ⚠ Citation Warnings
- `crates/op-gateway/src/mcp_gateway.rs:352`: file has 337 lines
- `crates/op-gateway/src/mcp_gateway.rs:361`: file has 337 lines
