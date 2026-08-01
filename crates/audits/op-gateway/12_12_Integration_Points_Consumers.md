# Integration & Security Audit Report: `op-gateway`

This report provides a production security and quality audit of the `op-gateway` crate based strictly on the provided workspace configuration and source files.

---

## 1. Workspace Dependency Analysis

### Workspace Crates Depending on `op-gateway`
Based on the workspace `Cargo.toml` and `Cargo.lock` configurations, there are currently **0 internal crates** that list `op-gateway` as a dependency. 
- `op-gateway` is defined as a workspace member in `Cargo.toml` but is not listed in `[workspace.dependencies]`.
- No other workspace crate (e.g., `op-dbus`, `op-mcp`, `op-identity`) specifies `op-gateway` in its package dependencies in `Cargo.lock`. 

### Cross-Crate Circular Dependency Risk
Because `op-gateway` does not currently import or depend on any other internal workspace `op-*` crates (as shown in `crates/op-gateway/Cargo.toml`), and no workspace crates depend on it, there is **no circular dependency risk** at present. It functions as a completely isolated leaf node.

---

## 2. D-Bus & Network Exposure Audit

### Registered D-Bus Services & Object Paths
- **Service Name**: None registered.
- **Object Path**: None registered.
- **Audit Detail**: While `crates/op-gateway/src/mcp_gateway.rs` implements public helper functions designed for D-Bus integration (`dbus_route_client`, `dbus_validate_session`, and `dbus_get_capabilities`), the crate itself does not depend on `zbus` (see `crates/op-gateway/Cargo.toml`) and does not register any interfaces, services, or object paths with a D-Bus daemon. Any actual registration is deferred to external coordinator packages (e.g. `op-dbus`).

### Exposed HTTP/gRPC Endpoints
- **Endpoints Exposed**: None.
- **Audit Detail**: The `op-gateway` crate acts purely as a library and does not spin up any native HTTP or gRPC listeners. It references mock target backend endpoints (e.g., `grpc://localhost:50051` and `grpc://localhost:50052` in `crates/op-gateway/src/mcp_gateway.rs:114`), but these represent the destinations of client routing decisions, not servers hosted by `op-gateway` itself.

---

## 3. Schema-as-Code Compliance Audit

The system architecture discipline mandates that all data contracts be expressed as versioned schemas (such as Protocol Buffers or OSCAL) rather than ad-hoc structs or strings. The following ad-hoc data contracts violate this discipline:

### Ad-hoc Persistent Configs & State Structs
* **`EncryptedStorageConfig`** [`crates/op-gateway/src/encrypted_storage.rs:19`] — Ad-hoc Rust struct with Serde serialization used for persisting storage configurations.
* **`KdfParams`** [`crates/op-gateway/src/encrypted_storage.rs:32`] — Ad-hoc configuration contract.
* **`EncryptedKeyEntry`** [`crates/op-gateway/src/encrypted_storage.rs:47`] — Persisted key metadata format serialized to JSON on disk without a versioned schema.
* **`KeyType`** [`crates/op-gateway/src/encrypted_storage.rs:58`] — Ad-hoc enumeration.
* **`StorageStats`** [`crates/op-gateway/src/encrypted_storage.rs:439`] — Ad-hoc status reporting schema.

### Ad-hoc Routing & Session Management Contracts
* **`RoutingDecision`** [`crates/op-gateway/src/mcp_gateway.rs:15`] — Ad-hoc contract returned over D-Bus/JSON-RPC representing routing state.
* **`AccessLevel`** [`crates/op-gateway/src/mcp_gateway.rs:26`] — Ad-hoc authorization level contract.
* **`McpClientInfo`** [`crates/op-gateway/src/mcp_gateway.rs:36`] — Ad-hoc client payload metadata contract.
* **`McpSession`** [`crates/op-gateway/src/mcp_gateway.rs:47`] — Ad-hoc session data contract.
* **`WireGuardSession`** [`crates/op-gateway/src/wireguard_auth.rs:194`] — Session schema persisted directly to SQLite and serialized via Serde without versioning.
* **`WireGuardStats`** [`crates/op-gateway/src/wireguard_auth.rs:211`] — Ad-hoc JSON-RPC/D-Bus diagnostic payload contract.
* **`ClientInfo`** [`crates/op-gateway/src/wireguard_auth.rs:578`] — Ad-hoc transaction metadata payload.
* **`SessionFilter`** [`crates/op-gateway/src/wireguard_auth.rs:585`] — Ad-hoc query interface contract.

### Ad-hoc D-Bus Serialization
* **`McpGatewayManager` Method Returns** [`crates/op-gateway/src/mcp_gateway.rs:304-338`] — The D-Bus methods return raw, untyped `simd_json::OwnedValue` JSON values generated via the `json!` macro on the fly, entirely bypassing structured and versioned schema contracts.

---

## 4. Production Security & Quality Findings

### [CRITICAL] Cryptographic Nonce Reuse via Non-Persisted State
* **Reference**: `crates/op-gateway/src/encrypted_storage.rs:308` and `crates/op-gateway/src/encrypted_storage.rs:341-344`
* **Impact**: Total compromise of key confidentiality.
* **Mechanism**: When storing keys, the storage engine uses `ChaCha20Poly1305` and relies on a stateful `nonce_counter` in the `MasterKey` struct to prevent nonce reuse:
  ```rust
  let mut nonce = [0u8; 12];
  let nonce_counter = master_key.nonce_counter;
  nonce[4..12].copy_from_slice(&nonce_counter.to_le_bytes());
  master_key.nonce_counter += 1;
  ```
  However, `nonce_counter` is kept purely in volatile memory. Every time `load_master_key` is called (which occurs on every service initialization/restart), the counter is hardcoded back to `0`:
  ```rust
  self.master_key = Some(MasterKey {
      key,
      salt,
      nonce_counter: 0,
  });
  ```
  Because this counter is never persisted to disk, process restarts force the reuse of identical nonces under the same master key (e.g., `nonce_counter = 0, 1, 2...`). Nonce reuse in stream ciphers like ChaCha20 completely destroys the security guarantees, enabling passive attackers to XOR ciphertexts and recover raw WireGuard private keys.

---

### [CRITICAL] Cryptographic Bypass: Publicly Derivable Pre-Shared Key (PSK)
* **Reference**: `crates/op-gateway/src/wireguard_auth.rs:538-557`
* **Impact**: Subversion of the WireGuard Pre-Shared Key security layer.
* **Mechanism**: WireGuard's PSK is designed as a symmetric secret to provide additional authorization and post-quantum security. However, the system derives the stable PSK deterministically from the client's public key using a static, hardcoded salt:
  ```rust
  pub fn derive_stable_psk(&self, peer_key: &[u8; 32]) -> Vec<[u8; 32]> {
      ...
      let salt = b"WG-STABLE-PSK-2024";
      let mut input = Vec::with_capacity(39);
      input.extend_from_slice(b"WG-PSK-");
      input.extend_from_slice(peer_key);
      ...
      let argon2 = Argon2::default();
      argon2.hash_password_into(&input, salt, &mut psk);
  ```
  Because the client's public key is sent unencrypted over the network during the handshake, and the salt and prefix are static constants in the open source binary, **any network eavesdropper can derive the client's PSK**. This completely nullifies the security benefits of the PSK.

---

### [HIGH] Plaintext Storage of Master Key adjacent to Ciphertexts
* **Reference**: `crates/op-gateway/src/encrypted_storage.rs:291-314` and `crates/op-gateway/src/encrypted_storage.rs:323-333`
* **Impact**: Encryption-at-rest bypass.
* **Mechanism**: The master key used to encrypt all WireGuard keys is written completely in plaintext to `/var/lib/op-dbus/encrypted/wireguard-keys/master.key` on disk:
  ```rust
  let mut key_data = Vec::with_capacity(64);
  key_data.extend_from_slice(&key);
  key_data.extend_from_slice(&salt);
  async_fs::write(path, &key_data).await?;
  ```
  The system attempts to compensate for this by applying `0o600` permissions. However, storing the raw decryption key alongside the encrypted data files in the same directory defeats the purpose of "at-rest" cryptographic separation, as any attacker achieving local file read privileges can immediately read the master key and decrypt all WireGuard secrets.

---

### [HIGH] Undefined Behavior & Memory Corruption via Unpadded `simd_json` Deserialization
* **Reference**: `crates/op-gateway/src/encrypted_storage.rs:390` and `crates/op-gateway/src/wireguard_auth.rs:163`
* **Impact**: Segfaults, denial-of-service, or undefined memory access in production.
* **Mechanism**: The code invokes `unsafe { simd_json::from_str(&mut entry_str) }` on standard Rust strings cloned from files or database fields:
  ```rust
  let entry_json = async_fs::read_to_string(&key_file_path).await?;
  let mut entry_str = entry_json.clone();
  let entry: EncryptedKeyEntry = unsafe { simd_json::from_str(&mut entry_str) }?;
  ```
  The `simd-json` crate requires that any parsed string buffer must have `simd_json::PADDING` (usually 32 or 64 bytes) of zeroed, allocated padding at the end of the buffer. Cloning a standard `String` provides no such capacity guarantees. Passing unpadded string buffers to `simd_json` causes SIMD registers to perform out-of-bounds reads, leading to segmentation faults or unpredictable process memory corruption.

---

### [MEDIUM] Silent Fallback to Plaintext Storage on Encryption Failure
* **Reference**: `crates/op-gateway/src/encrypted_storage.rs:136-146` and `crates/op-gateway/src/encrypted_storage.rs:194-206`
* **Impact**: Unintended plaintext storage of private keys.
* **Mechanism**: If `use_native_encryption` is set to `true`, the system executes `btrfs subvolume create -e` to create an encrypted subvolume. If this fails due to a lack of kernel or filesystem tool support (e.g., experimental features not compiled), the system emits a warning and silently falls back to a regular unencrypted subvolume. If Btrfs is unavailable altogether, it falls back to a standard unencrypted directory:
  ```rust
  if stderr.contains("encryption not supported") || stderr.contains("invalid option") {
      warn!("Native Btrfs encryption not supported, creating regular subvolume");
      self.create_regular_subvolume().await?;
  }
  ```
  This creates a silent security failure where administrators are led to believe native filesystem encryption is active, when the system has actually degraded to storing keys in an unencrypted directory tree.