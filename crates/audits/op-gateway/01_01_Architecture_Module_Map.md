# OP-DBUS: Production Security & Quality Audit

---

## 1. Overview & Architecture Map

### Module Tree
* **crates/op-gateway/src/lib.rs** (Crate Entry Point)
  * `encrypted_storage` (`crates/op-gateway/src/encrypted_storage.rs`): Handles cryptographic storage of WireGuard private keys and session keys using Btrfs subvolumes and LUKS containers.
  * `error` (`crates/op-gateway/src/error.rs`): Defines internal crate-level error types.
  * `mcp_gateway` (`crates/op-gateway/src/mcp_gateway.rs`): Implements client request routing and D-Bus integration for the Model Context Protocol (MCP) services.
  * `wireguard_auth` (`crates/op-gateway/src/wireguard_auth.rs`): Handles WireGuard session state, SQLite-backed session persistence, and stable PSK/session-key derivations.

### Targets & Entry Points
* **Library Entry Point**: `crates/op-gateway/src/lib.rs`
* **Dependencies**: Uses `ring` for random number generation, `chacha20poly1305` for AEAD encryption, `argon2` for key derivation, `sqlx` for SQLite session storage, and `simd_json` for high-performance JSON operations.

---

## 2. Critical & High Security Findings

### [Finding 1] Cryptographic Nonce Reuse in ChaCha20-Poly1305 on Service Restart
* **File & Line**: `crates/op-gateway/src/encrypted_storage.rs:257`, `crates/op-gateway/src/encrypted_storage.rs:307`
* **Severity**: **Critical** (Directly exploitable)
* **Vulnerability Analysis**:
  The storage manager uses stateful, counter-based nonces for ChaCha20-Poly1305 encryption. In `crates/op-gateway/src/encrypted_storage.rs:307`, a sequential counter is embedded within the nonce:
  ```rust
  let mut nonce = [0u8; 12];
  let nonce_counter = master_key.nonce_counter;
  nonce[4..12].copy_from_slice(&nonce_counter.to_le_bytes());
  master_key.nonce_counter += 1;
  ```
  However, this counter is kept purely in volatile memory. When the gateway service restarts, `load_master_key` is executed during initialization (line 118) and resets the counter back to zero (line 257):
  ```rust
  self.master_key = Some(MasterKey {
      key,
      salt,
      nonce_counter: 0,
  });
  ```
  Consequently, after every service restart, the application reuses the exact same nonces (`0`, `1`, `2`, ...) under the same master key to encrypt new files.
* **Exploit Scenario**: 
  An attacker with read-only access to the backup storage or `/var/lib/op-dbus/encrypted/` can obtain multiple key files encrypted with identical nonces. Since ChaCha20 is a stream cipher, this completely destroys confidentiality. Simple XOR-based cryptanalysis and polynomial evaluation allow the recovery of plaintext keys (including WireGuard private keys and master key elements).
* **Remediation**:
  Generate a unique, cryptographically secure random 96-bit nonce using a CSPRNG for every encryption operation:
  ```rust
  let mut nonce = [0u8; 12];
  ring::rand::SystemRandom::new()
      .fill(&mut nonce)
      .map_err(|_| anyhow::anyhow!("Failed to generate random nonce"))?;
  ```

---

### [Finding 2] Silent Security Bypass via Plaintext Fallback
* **File & Line**: `crates/op-gateway/src/encrypted_storage.rs:143`, `crates/op-gateway/src/encrypted_storage.rs:191`
* **Severity**: **High**
* **Vulnerability Analysis**:
  The system attempts to set up native Btrfs subvolume encryption (which is highly experimental and generally missing from upstream kernels) or LUKS containers. If native encryption fails, the system logs a warning and silently falls back to creating a regular, unencrypted subvolume (line 143):
  ```rust
  if stderr.contains("encryption not supported") || stderr.contains("invalid option") {
      warn!("Native Btrfs encryption not supported, creating regular subvolume");
      self.create_regular_subvolume().await?;
  }
  ```
  Similarly, if LUKS is chosen, the system prints a warning and falls back to an unencrypted subvolume (line 191):
  ```rust
  warn!("LUKS setup requires manual intervention - using test passphrase");
  self.create_regular_subvolume().await?;
  ```
  This creates a false sense of security. The operator believes they have initialized encrypted storage, but the keys are silently stored in plaintext on disk.
* **Remediation**:
  Implement a strict "fail-closed" security policy. If cryptographic isolation or storage encryption cannot be established, the system must abort initialization and return an explicit error instead of performing a silent fallback.

---

### [Finding 3] Plaintext Master Key Storage on Local Disk
* **File & Line**: `crates/op-gateway/src/encrypted_storage.rs:228`, `crates/op-gateway/src/encrypted_storage.rs:277`
* **Severity**: **High**
* **Vulnerability Analysis**:
  In `generate_master_key` (line 277), the system writes the raw, unencrypted master key directly to `master.key` inside the storage subvolume:
  ```rust
  async_fs::write(path, &key_data).await?;
  ```
  Storing the raw master key in plaintext on the same filesystem completely defeats the purpose of the encryption layer. If an attacker gains read access to the directory, they can read the master key and decrypt all stored key entries.
* **Remediation**:
  Do not write the raw master key to disk. Derive the key dynamically from an external Key Management Service (KMS), TPM, or a user-provided passphrase at startup using Argon2.

---

### [Finding 4] High-Frequency Denial of Service (DoS) via Unauthenticated KDF Calculations
* **File & Line**: `crates/op-gateway/src/wireguard_auth.rs:387`, `crates/op-gateway/src/wireguard_auth.rs:661`, `crates/op-gateway/src/wireguard_auth.rs:700`
* **Severity**: **High**
* **Vulnerability Analysis**:
  The `create_session` function (line 387) accepts a `peer_pubkey` from a client and validates its format (line 661). If the format is correct (a 64-character hex string) and no cached session is active, it calls `derive_psk` (line 432), which executes `derive_stable_psk` (line 700). 
  `derive_stable_psk` runs the Argon2 KDF synchronously in the request path:
  ```rust
  let argon2 = Argon2::default();
  let mut psk = [0u8; 32];
  argon2.hash_password_into(&input, salt, &mut psk)
  ```
  Since `create_session` does not authenticate the caller or verify whether the public key is registered before running the KDF, any network client can submit randomized, syntactically valid public keys.
* **Exploit Scenario**:
  An external attacker can flood the session creation endpoint with random 64-character hex strings. The server will run the expensive Argon2 KDF for every request, immediately saturating the CPU, starvating processing resources, and causing a complete Denial of Service (DoS) of the gateway.
* **Remediation**:
  1. Authorize the `peer_pubkey` against a pre-registered database *before* initiating any expensive cryptographic derivations.
  2. Implement strict IP and token-based rate-limiting on the session-creation endpoints.
  3. Offload KDF tasks to a dedicated thread pool with concurrency limits to prevent gateway thread pool starvation.

---

## 3. Schema-as-Code & OSCAL Compliance

The codebase uses ad-hoc serialization structures instead of a versioned schema-as-code discipline.

### Ad-hoc Data Contracts
The following structures are defined directly as Rust structs and serialized/deserialized to JSON without API versioning or schema compilation:
* **Storage Configuration**: `EncryptedStorageConfig` and `KdfParams` (`crates/op-gateway/src/encrypted_storage.rs:20,33`)
* **Key Packaging**: `EncryptedKeyEntry` (`crates/op-gateway/src/encrypted_storage.rs:52`)
* **MCP Routings**: `RoutingDecision` and `McpSession` (`crates/op-gateway/src/mcp_gateway.rs:16,52`)
* **Client Identifiers**: `McpClientInfo` (`crates/op-gateway/src/mcp_gateway.rs:41`) and `ClientInfo` (`crates/op-gateway/src/wireguard_auth.rs:745`)
* **WireGuard Session States**: `WireGuardSession` (`crates/op-gateway/src/wireguard_auth.rs:198`)

### Ad-hoc JSON Payload Construction
In `crates/op-gateway/src/mcp_gateway.rs:318`, the JSON payload returned to D-Bus callers is built dynamically using the `simd_json::json!` macro:
```rust
Ok(json!({
    "endpoint": routing_decision.endpoint,
    "allowed_tools": routing_decision.allowed_tools,
    "capabilities": routing_decision.capabilities,
    "has_full_access": routing_decision.has_full_access,
    "session_id": routing_decision.session_id,
    "access_level": match routing_decision.access_level { ... }
}))
```

### Risk & Compliance Impact
* **OSCAL / FedRAMP Compliance**: This ad-hoc serialization strategy lacks metadata and schema validation. Automated compliance pipelines cannot automatically parse or validate configuration parameters, security settings, or session flags against defined baseline configurations.
* **Backward Compatibility**: Without versioned schemas (e.g. `v1.Session`), changing fields in these structs breaks compatibility with older database files or active clients.

### Remediation
Define all data contracts, configuration models, and RPC payloads using Protocol Buffers (`.proto` files) compiled via `prost` or versioned JSON Schemas. Ensure all configuration parameters are mapped to an OSCAL-compliant component definition schema.

---

## 4. Code Quality & Safety Audit

### [Quality 1] Unsafe `simd_json` Deserialization on Arbitrary Inputs
* **File & Line**: `crates/op-gateway/src/encrypted_storage.rs:356`, `crates/op-gateway/src/wireguard_auth.rs:172`
* **Vulnerability / Risk**:
  The application uses `unsafe` blocks to call `simd_json::from_str` with cloned strings:
  ```rust
  let entry: EncryptedKeyEntry = unsafe { simd_json::from_str(&mut entry_str) }?;
  ```
  `simd_json::from_str` mutates the input string slice in-place. If there are lifetime or buffer boundary assumptions violated in custom data types or if the string is structurally modified concurrently, this can introduce undefined behavior (UB). 
* **Remediation**:
  Use the safe `simd_json::serde::from_str` wrapper instead of unsafe raw blocks, or fall back to standard safe `serde_json` for storage records where performance is not a bottleneck.

---

### [Quality 2] Session ID Collisions on Concurrent Requests
* **File & Line**: `crates/op-gateway/src/wireguard_auth.rs:467`
* **Vulnerability / Risk**:
  The session ID is generated using a second-level timestamp and the public key:
  ```rust
  let input = format!("WG-SESSION-{}-{}", peer_pubkey, Self::current_timestamp());
  ```
  If a peer makes multiple requests within the same second, they will generate identical session IDs. This will trigger a primary key collision in SQLite, causing the database to execute an `INSERT OR REPLACE` (line 72) and silently overwrite the active session. This leads to unexpected disconnection or authorization state loss.
* **Remediation**:
  Include a high-entropy random value (such as a UUIDv4 or 128 bits from `ring::rand`) in the input string before hashing to guarantee global uniqueness.