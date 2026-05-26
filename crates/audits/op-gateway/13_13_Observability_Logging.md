# Production Security and Quality Audit: op-gateway

## I. Observability & Logging Audit

### 1. Tracing Macros vs. `println!` Count

A total scan of the `op-gateway` crate reveals that the codebase strictly uses the structured `tracing` crate for observability. No raw `println!` or `eprintln!` statements are present. 

| File | `tracing::info!` | `tracing::debug!` | `tracing::warn!` | `tracing::error!` | `println!` |
| :--- | :---: | :---: | :---: | :---: | :---: |
| `crates/op-gateway/src/encrypted_storage.rs` | 10 | 8 | 3 | 0 | 0 |
| `crates/op-gateway/src/mcp_gateway.rs` | 4 | 1 | 0 | 0 | 0 |
| `crates/op-gateway/src/wireguard_auth.rs` | 7 | 4 | 5 | 0 | 0 |
| **Total** | **21** | **13** | **8** | **0** | **0** |

---

### 2. Silent Error Swallowing

Multiple occurrences of suppressed, unlogged, or silently handled errors were identified:

*   **`crates/op-gateway/src/wireguard_auth.rs:470`**: 
    ```rust
    if let Ok(stored_psk) = key_storage.retrieve_key(&psk_key_id).await {
    ```
    If `retrieve_key` fails due to filesystem corruption, decryption failure, or permission issues, the error is discarded silently. The system falls back to generating and storing a new PSK. This can cause silent desynchronization between the gateway and its clients without alerting operations.

*   **`crates/op-gateway/src/wireguard_auth.rs:146-148`**:
    ```rust
    let flags_json: String = row.get("flags");
    let mut flags_str = flags_json.clone();
    let flags: std::collections::HashMap<String, String> =
        unsafe { simd_json::from_str(&mut flags_str) }.unwrap_or_default();
    ```
    If database corruption or ad-hoc JSON schema changes make `flags` unparseable, `simd_json::from_str` fails and the error is silently swallowed via `.unwrap_or_default()`, yielding an empty hash map without logging a diagnostic warning or error.

*   **`crates/op-gateway/src/encrypted_storage.rs:500-505`**:
    ```rust
    total_space: fields[2].parse().unwrap_or(0),
    used_space: fields[3].parse().unwrap_or(0),
    available_space: fields[4].parse().unwrap_or(0),
    ```
    If the output of `df` cannot be parsed due to localization changes or OS format differences, values default to `0` silently.

---

### 3. Potential PII or Secrets in Log Output

*   **`crates/op-gateway/src/wireguard_auth.rs:214`**:
    `debug!("Creating WireGuard session for peer: {}", peer_pubkey);`
*   **`crates/op-gateway/src/wireguard_auth.rs:282`**:
    `info!("Created WireGuard session {} for peer {}", session.session_id, peer_pubkey);`
*   **`crates/op-gateway/src/wireguard_auth.rs:344`**:
    `info!("Rotating session key for peer: {} (force: {})", peer_pubkey, force);`
*   **`crates/op-gateway/src/wireguard_auth.rs:389`**:
    `info!("Session key rotated successfully for peer: {}", peer_pubkey);`

**Impact**: While public keys are not secrets, they represent unique hardware/client identifiers. In strict compliance contexts (e.g., GDPR, CCPA), logging raw peer public keys in correlation with session IDs and client names (e.g., `client_info.name`) constitutes logging tracking identifiers (PII). These should be obfuscated or hashed (e.g., displaying only the first 8 characters) in `info!` and `debug!` statements.

---

### 4. Metrics Instrumentation Gaps

Although the workspace dependencies include `prometheus` and `opentelemetry`, the `op-gateway` crate contains **zero metrics instrumentation**. 
*   **Ad-hoc State Tracking**: In `crates/op-gateway/src/wireguard_auth.rs:168`, statistics are collected inside an ad-hoc in-memory struct `WireGuardStats`.
*   **No Exporters**: There are no telemetry endpoints, prometheus registries, or `metrics` crate macros to expose these parameters to production scrapers.

---

## II. Data Contract & Schema Discipline Audit

The `op-gateway` crate bypasses the unified schema-as-code discipline (Protocol Buffers / OSCAL) established in the workspace. Data contracts are represented using ad-hoc `serde`-serializable structs and unstructured JSON values:

*   **Ad-hoc D-Bus Serialization (`crates/op-gateway/src/mcp_gateway.rs:268-295`)**:
    The `dbus_route_client`, `dbus_validate_session`, and `dbus_get_capabilities` methods construct loose JSON payloads inline using the `simd_json::json!` macro rather than returning typed, schema-versioned Protocol Buffer contracts.
*   **Ad-hoc Struct Definitions**:
    *   `McpClientInfo` (`crates/op-gateway/src/mcp_gateway.rs:50`)
    *   `McpSession` (`crates/op-gateway/src/mcp_gateway.rs:61`)
    *   `RoutingDecision` (`crates/op-gateway/src/mcp_gateway.rs:17`)
    *   `WireGuardSession` (`crates/op-gateway/src/wireguard_auth.rs:124`)
    *   `EncryptedKeyEntry` (`crates/op-gateway/src/encrypted_storage.rs:51`)
    *   `EncryptedStorageConfig` (`crates/op-gateway/src/encrypted_storage.rs:18`)

These structs rely on ad-hoc JSON serialization directly to local files or SQLite tables. Any change in fields will cause silent database and storage loading failures across upgrades.

---

## III. Cryptographic and Security Vulnerability Audit

### 1. [CRITICAL] Authentication Bypass via Public Key Impersonation
*   **File/Line**: `crates/op-gateway/src/mcp_gateway.rs:220-238`

#### Vulnerability Analysis
The authentication routing system relies on `check_authentication` to verify if a client is authorized. The logic is defined as:
```rust
async fn check_authentication(&self, client_info: &McpClientInfo) -> Result<bool> {
    // Check auth token first
    if let Some(ref auth_token) = client_info.auth_token {
        return self.wireguard_auth.validate_session(auth_token).await;
    }

    // Check peer public key
    if let Some(ref peer_pubkey) = client_info.peer_pubkey {
        let filter = SessionFilter {
            active_only: Some(true),
            peer_pubkey: Some(peer_pubkey.clone()),
            created_after: None,
            created_before: None,
        };

        let sessions = self.wireguard_auth.list_sessions(Some(filter)).await?;
        return Ok(!sessions.is_empty());
    }

    // No authentication information provided
    Ok(false)
}
```

If a client fails to provide an `auth_token` but provides a `peer_pubkey`, the gateway queries the active sessions list. If *any* active session exists for that public key, the gateway returns `Ok(true)`, granting the client `AccessLevel::Full` privileges.

There is no cryptographic signature check, challenge-response phase, or proof of private key ownership matching `peer_pubkey`. Because peer public keys are inherently public and transmitted in cleartext during WireGuard handshakes or public configurations, any malicious local D-Bus user or unauthenticated client can impersonate any active WireGuard node simply by providing its target public key in the `peer_pubkey` payload.

#### Remediation
Remove peer public key checks as a valid standalone authentication option. Require all requests to present a cryptographically verifiable session token (`auth_token`) signed by the server, or establish a proper challenge-response authentication handshake.

---

### 2. [CRITICAL] Cryptographic Plaintext Master Key Storage on Unencrypted Fallbacks
*   **File/Line**: `crates/op-gateway/src/encrypted_storage.rs:328`, `crates/op-gateway/src/encrypted_storage.rs:158`, and `crates/op-gateway/src/encrypted_storage.rs:222`

#### Vulnerability Analysis
The master encryption key is generated securely via Ring's `SystemRandom` but is written to the filesystem in **plaintext**:
```rust
async fn generate_master_key(&mut self, path: &Path) -> Result<()> {
    let rng = SystemRandom::new();
    let mut key = [0u8; 32];
    let mut salt = [0u8; 32];

    rng.fill(&mut key)
        .map_err(|_| anyhow::anyhow!("Failed to generate key"))?;
    rng.fill(&mut salt)
        .map_err(|_| anyhow::anyhow!("Failed to generate salt"))?;

    // Store encrypted key (in production, encrypt with user passphrase)
    let mut key_data = Vec::with_capacity(64);
    key_data.extend_from_slice(&key);
    key_data.extend_from_slice(&salt);

    async_fs::write(path, &key_data).await?;
    ...
```

The system assumes the underlying storage is encrypted by Btrfs or LUKS. However:
1.  **Btrfs native encryption fallback (`crates/op-gateway/src/encrypted_storage.rs:158-161`)**: If native Btrfs encryption is not supported (which is standard, as native encryption is experimental and unmerged), the code silently falls back to an unencrypted subvolume or standard unencrypted directory (`crates/op-gateway/src/encrypted_storage.rs:239`).
2.  **LUKS encryption fallback (`crates/op-gateway/src/encrypted_storage.rs:222-225`)**: The LUKS setup is hardcoded to fail and directly falls back to an unencrypted subvolume:
    ```rust
    warn!("LUKS setup requires manual intervention - using test passphrase");

    // Create regular subvolume for now
    self.create_regular_subvolume().await?;
    ```

As a result, on standard production systems, the cryptographic master key `master.key` is stored in raw plaintext inside a standard, unencrypted directory next to the database and encrypted key files. Any user or attacker with access to `/var/lib/op-dbus/` can read `master.key` and immediately decrypt all WireGuard private keys, PSKs, and session credentials.

#### Remediation
Encrypt the master key with a key derived from a strong passphrase using Argon2id before writing it to disk. Do not perform silent unencrypted fallbacks for sensitive asset storage.

---

### 3. [CRITICAL] ChaCha20-Poly1305 Nonce Reuse Across System Restarts
*   **File/Line**: `crates/op-gateway/src/encrypted_storage.rs:365-370`

#### Vulnerability Analysis
The file encryption implementation utilizes `ChaCha20Poly1305` and derives the nonce from an in-memory counter on `MasterKey`:
```rust
// Generate nonce
let mut nonce = [0u8; 12];
let nonce_counter = master_key.nonce_counter;
nonce[4..12].copy_from_slice(&nonce_counter.to_le_bytes());
master_key.nonce_counter += 1;
```

The `MasterKey` struct and its `nonce_counter` are purely transient and initialized in memory to `0` whenever `EncryptedKeyStorage` starts:
```rust
self.master_key = Some(MasterKey {
    key,
    salt,
    nonce_counter: 0,
});
```

Because `nonce_counter` always starts at `0` upon service initialization, restarting the service causes the system to reuse the exact same nonces (`0`, `1`, `2`...) with the same master key to encrypt new files or updates to old keys.

In stream ciphers like ChaCha20, reusing a nonce (a "two-time pad" condition) completely breaks confidentiality. An attacker who can read the encrypted `{key_id}.key` files (which are on unencrypted fallbacks) can perform xor operations on different ciphertexts to recover the plaintext key secrets.

#### Remediation
Never use a state-dependent sequence counter for nonces in transient storage structures. Utilize a cryptographically secure pseudo-random number generator (CSPRNG) to populate the entire 96-bit (12-byte) nonce vector for each encryption operation.

---

## IV. Summary of Recommendations

1.  **Enforce Cryptographic Authentication**: Modify `McpGatewayManager::check_authentication` to completely drop peer public key validation in favor of signature checks.
2.  **Ensure Cryptographic Confidentiality of Master Key**: Implement a key-derivation function (KDF) to encrypt the master key with a passphrase.
3.  **Remediate Nonce Reuse**: Use random nonces via Ring's CSPRNG for all AES or ChaCha20Poly1305 operations.
4.  **Adopt Schema Discipline**: Discontinue the use of ad-hoc structs and unstructured `serde_json` or `simd_json` values for D-Bus messages. Define all inter-service and persistence schemas in formal, versioned Protocol Buffer definitions.
5.  **Expose Metrics**: Connect the internal `WireGuardStats` struct to the workspace's Prometheus registry to enable proper infrastructure observability.