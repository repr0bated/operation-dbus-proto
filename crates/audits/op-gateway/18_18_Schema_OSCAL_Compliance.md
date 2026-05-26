| Item | Type | file:line | Has .proto? | Gap |
| :--- | :--- | :--- | :--- | :--- |
| `EncryptedStorageConfig` | Data Contract | `crates/op-gateway/src/encrypted_storage.rs:18` | No | Defined as an ad-hoc Rust struct with Serde attributes. Lacks a versioned, language-neutral schema. |
| `KdfParams` | Data Contract | `crates/op-gateway/src/encrypted_storage.rs:31` | No | Defined as an ad-hoc Rust struct. No versioned Protobuf definition. |
| `EncryptedKeyEntry` | Data Contract | `crates/op-gateway/src/encrypted_storage.rs:49` | No | Defined as an ad-hoc Rust struct. Serialized as raw JSON, causing schema-evolution risk. |
| `KeyType` | Enum | `crates/op-gateway/src/encrypted_storage.rs:59` | No | Explicit type representation lacking a central versioned schema mapping. |
| `StorageStats` | Data Contract | `crates/op-gateway/src/encrypted_storage.rs:550` | No | Untyped system contract with no shared schema format. |
| `RoutingDecision` | RPC/D-Bus Payload | `crates/op-gateway/src/mcp_gateway.rs:17` | No | Internal ad-hoc representation returned over D-Bus with no formalized schema. |
| `AccessLevel` | Enum | `crates/op-gateway/src/mcp_gateway.rs:28` | No | Ad-hoc serialization enum mapping. |
| `McpClientInfo` | Client Request Contract | `crates/op-gateway/src/mcp_gateway.rs:39` | No | Ad-hoc request layout; lacks strict validation rules or field-number assignments. |
| `McpSession` | Session Management State | `crates/op-gateway/src/mcp_gateway.rs:50` | No | In-memory session layout without a formalized state schema. |
| `dbus_route_client` | D-Bus Endpoint | `crates/op-gateway/src/mcp_gateway.rs:271` | No | Returns untyped `simd_json::OwnedValue` using `json!` instead of a structured Protobuf/OSCAL schema. |
| `dbus_validate_session` | D-Bus Endpoint | `crates/op-gateway/src/mcp_gateway.rs:294` | No | Returns ad-hoc schema-less JSON (`simd_json::OwnedValue`). |
| `dbus_get_capabilities` | D-Bus Endpoint | `crates/op-gateway/src/mcp_gateway.rs:304` | No | Returns ad-hoc schema-less JSON (`simd_json::OwnedValue`). |
| `WireGuardSession` | Database/State Model | `crates/op-gateway/src/wireguard_auth.rs:179` | No | Hand-rolled schema structure utilizing JSON columns for flexible metadata fields without a contract. |
| `WireGuardStats` | Monitoring Contract | `crates/op-gateway/src/wireguard_auth.rs:197` | No | System metric representation. |
| `ClientInfo` | Data Contract | `crates/op-gateway/src/wireguard_auth.rs:801` | No | Ad-hoc struct serialization contract. |
| `SessionFilter` | Query Contract | `crates/op-gateway/src/wireguard_auth.rs:809` | No | Ad-hoc query filtering parameters. |

### OSCAL Coverage Audit

| Control Area | Implemented at file:line | OSCAL Artifact | Gap |
| :--- | :--- | :--- | :--- |
| **AC-3: Access Enforcement** (D-Bus endpoint protection & routing capabilities) | `crates/op-gateway/src/mcp_gateway.rs:90-120` | None | Capabilities and allowed tools are hardcoded in compilation-level arrays. No mapping to an OSCAL Component Definition policy. |
| **IA-2: Identification and Authentication** (WireGuard session authentication) | `crates/op-gateway/src/mcp_gateway.rs:211-232` | None | Authentication state verified by raw parameters with no machine-readable proof of identity requirement mapped to OSCAL. |
| **SC-13: Cryptographic Protection** (LUKS / Btrfs Native Encryption) | `crates/op-gateway/src/encrypted_storage.rs:114`, `191` | None | Stated as "LUKS fallback" or "Native Btrfs" but lacks verification mappings to OSCAL system security plans or cryptographic validation artifacts. |
| **SC-28: Protection of Information at Rest** (Plaintext Storage of Keys) | `crates/op-gateway/src/encrypted_storage.rs:340-378` | None | Master key generated and written in plaintext to standard persistent media with no cryptographic protection (e.g. envelope encryption) mapped to OSCAL. |

---

### Detailed Findings & Recommendations

#### 1. [CRITICAL] Authentication Bypass via Spoofed Client Public Keys
- **Citation**: `crates/op-gateway/src/mcp_gateway.rs:211-232`
- **Impact**: Any unauthenticated client or entity communicating over the control plane can masquerade as an authenticated user. An attacker merely has to provide the `peer_pubkey` of any actively connected WireGuard client (which is public information in WireGuard topologies) without an `auth_token`.
- **Vulnerability Analysis**:
  In `check_authentication`, if an `auth_token` is missing, the gateway falls back to checking the client's `peer_pubkey`:
  ```rust
  if let Some(ref peer_pubkey) = client_info.peer_pubkey {
      let filter = SessionFilter {
          active_only: Some(true),
          peer_pubkey: Some(peer_pubkey.clone()),
          ..
      };
      let sessions = self.wireguard_auth.list_sessions(Some(filter)).await?;
      return Ok(!sessions.is_empty());
  }
  ```
  The logic returns `Ok(true)` purely if an active session associated with that public key exists in the database. There is no cryptographic verification (such as requiring a signature from that public key or verification of a secure session cookie generated using that public key's private counterpart). 
- **Remediation**:
  Remove the fallback authentication path on raw public key lookup. All requests must present a validated cryptographic token (`auth_token`) linked to the active session key, rather than relying on a self-reported public identifier.

#### 2. [CRITICAL] Cryptographic Key-Reuse & Cryptographic Failure via Nonce-Counter Reset
- **Citation**: `crates/op-gateway/src/encrypted_storage.rs:354-358` and `crates/op-gateway/src/encrypted_storage.rs:407-411`
- **Impact**: Multi-message nonce reuse under the same encryption key in ChaCha20-Poly1305. An attacker with read access to the encrypted key database can decrypt all encrypted credentials, exposing WireGuard private keys and session keys.
- **Vulnerability Analysis**:
  In `store_key`, the 12-byte nonce is constructed strictly using the sequential `nonce_counter`:
  ```rust
  let mut nonce = [0u8; 12];
  let nonce_counter = master_key.nonce_counter;
  nonce[4..12].copy_from_slice(&nonce_counter.to_le_bytes());
  master_key.nonce_counter += 1;
  ```
  However, during storage initialization (`load_master_key`), the counter is hardcoded to reset to `0` every time the service starts:
  ```rust
  self.master_key = Some(MasterKey {
      key,
      salt,
      nonce_counter: 0,
  });
  ```
  Upon reboot or restart of the gateway daemon, the exact same sequence of nonces (0, 1, 2, etc.) will be reused to encrypt new keys or key modifications using the same master key. Nonce reuse in stream ciphers like ChaCha20 completely breaks confidentiality, allowing recovery of plaintexts via XOR analysis of matching keystreams.
- **Remediation**:
  Persist the `nonce_counter` in a cryptographically secure, monotonic, non-volatile state store, or switch to using standard cryptographically secure random 96-bit nonces (using `ring::rand::SecureRandom`) for each encryption operation.

#### 3. [CRITICAL] Plaintext Persistence of Cryptographic Master Key
- **Citation**: `crates/op-gateway/src/encrypted_storage.rs:340-352` and `crates/op-gateway/src/encrypted_storage.rs:364-378`
- **Impact**: Complete failure of encryption-at-rest. An attacker with access to `/var/lib/op-dbus/encrypted` can directly read the cryptographic master key and compromise all encrypted WireGuard keys and sessions.
- **Vulnerability Analysis**:
  Although labeled as storing an "encrypted key", the implementation of `generate_master_key` writes the raw generated 32-byte key and 32-byte salt directly to the filesystem in plaintext:
  ```rust
  let mut key_data = Vec::with_capacity(64);
  key_data.extend_from_slice(&key);
  key_data.extend_from_slice(&salt);
  async_fs::write(path, &key_data).await?;
  ```
  Similarly, `load_master_key` simply reads the raw 64 bytes directly from disk without performing any key derivation or passphrase decryption:
  ```rust
  let encrypted_data = async_fs::read(path).await?;
  ...
  key.copy_from_slice(&encrypted_data[0..32]);
  ```
  Setting file permissions to `0o600` is insufficient to protect sensitive keys from disk extraction, backup exposure, or lateral privilege escalation.
- **Remediation**:
  Do not store the raw master key in plaintext on the same media. Implement native envelope encryption utilizing a hardware security module (HSM), TPM, or prompt for a user-derived passphrase processed through a hardened Key Derivation Function (KDF) like Argon2id.

#### 4. [MAJOR] Silent Cryptographic Bypass in Fallback LUKS Setup
- **Citation**: `crates/op-gateway/src/encrypted_storage.rs:229-234`
- **Impact**: Security controls are silently disabled. When experimental native Btrfs encryption is unavailable or LUKS setup fallback is reached, the system silently writes confidential keys to unencrypted persistent storage.
- **Vulnerability Analysis**:
  In `setup_luks_encryption`, rather than setting up a standard LUKS partition loop device or throwing a fatal initialization error, the function logs a warning and falls back to storing data on a standard, unencrypted subvolume:
  ```rust
  warn!("LUKS setup requires manual intervention - using test passphrase");
  // Create regular subvolume for now
  self.create_regular_subvolume().await?;
  ```
  This violates the principle of secure fail-safe defaults (fail-secure). If the operator expects LUKS-level protection, the code proceeds without it.
- **Remediation**:
  Refactor `setup_luks_encryption` to fail loudly and return an error if the container cannot be securely mounted or formatted as LUKS. Do not fallback to unencrypted subvolumes when storing critical identities.

#### 5. [MAJOR] Synchronous CPU-Bound Argon2 execution inside Async Tokio Thread Pool
- **Citation**: `crates/op-gateway/src/wireguard_auth.rs:754-758` and `crates/op-gateway/src/wireguard_auth.rs:779-784`
- **Impact**: Complete blocking of the Tokio async executor, causing high latency, starvation of concurrent tasks, and vulnerability to low-bandwidth Denial of Service (DoS) attacks.
- **Vulnerability Analysis**:
  `derive_stable_psk` and `derive_session_keys` are executed within async workflows but perform intensive, synchronous, CPU-bound password hashing operations via `Argon2::default()`. Hashing functions of this nature block the executing thread for tens of milliseconds or more. When called under load or in batches, this blocks the entire Tokio thread pool.
- **Remediation**:
  Wrap the synchronous Argon2 processing logic in a `tokio::task::spawn_blocking` call to hand off CPU-heavy calculations to a dedicated blocking-thread pool:
  ```rust
  let psk = tokio::task::spawn_blocking(move || {
      let argon2 = Argon2::default();
      let mut psk = [0u8; 32];
      argon2.hash_password_into(&input, salt, &mut psk).map(|_| psk)
  }).await??;
  ```

#### 6. [MEDIUM] D-Bus Untyped Serialization / Ad-hoc Contract Schema Violations
- **Citation**: `crates/op-gateway/src/mcp_gateway.rs:271-291`, `294-301`, `304-311`
- **Impact**: Violation of the Schema-as-Code discipline. Schema-less serialization models risk runtime decoding failures, version drift, and parsing security bugs.
- **Vulnerability Analysis**:
  The gateway endpoints returned over the D-Bus interface are built as ad-hoc, untyped JSON structures utilizing `json!`:
  ```rust
  Ok(json!({
      "endpoint": routing_decision.endpoint,
      "allowed_tools": routing_decision.allowed_tools,
      ...
  }))
  ```
  These structures have no formalized, versioned, or neutral definitions (such as Protocol Buffer schemas).
- **Remediation**:
  Model all externalized control contracts as versioned Protocol Buffers (.proto files) or structured Rust types generated using versioned API models, ensuring zero reliance on raw untyped JSON blobs over control interfaces. Map these components and endpoints to an OSCAL Component Definition to satisfy continuous security monitoring standards.