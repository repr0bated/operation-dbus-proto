# 1. License & Compliance Audit

## 1.1 License Field Extraction
- **Crate:** `op-gateway` (`crates/op-gateway/Cargo.toml`)
  - **License Field:** **None**. The local `Cargo.toml` package declaration does not specify a `license` field or inherit from the workspace via `license.workspace = true`. 
- **Workspace Default:** `Cargo.toml:46` under `[workspace.package]` specifies `license = "Apache-2.0"`. However, because `crates/op-gateway/Cargo.toml` does not specify `license.workspace = true`, the license property is not automatically inherited by the `op-gateway` crate when processed independently.

## 1.2 Cargo.lock Scanning for GPL/AGPL/SSPL Crates
A comprehensive scan of `Cargo.lock` was performed. No GPL, AGPL, or SSPL licensed crates were identified. 
- **Note on Copyleft:** The workspace depends on `cozo` (`Cargo.lock:297`), which is licensed under the Mozilla Public License 2.0 (MPL-2.0). MPL-2.0 is a weak copyleft license. It is generally compatible with Apache-2.0 and does not trigger the viral copyleft requirements of GPL-family licenses, provided that Cozo remains unmodified or any modifications to Cozo source code are made public under MPL-2.0.

## 1.3 Crates with Missing License Fields
- **`op-gateway`** (`crates/op-gateway/Cargo.toml`): Contains no `license` or `license.workspace` field.

---

# 2. Schema-as-Code Evaluation

This codebase purports to follow a strict schema-as-code discipline utilizing Protocol Buffers and OSCAL. However, the audited `op-gateway` crate contains multiple violations where external interface data contracts, internal states, and database storage are defined using ad-hoc structs, raw JSON objects, or serialized strings.

### Ad-Hoc Data Contracts instead of Versioned Schemas
- **`crates/op-gateway/src/encrypted_storage.rs:16`**: Config contracts (`EncryptedStorageConfig`, `KdfParams`) are represented using ad-hoc serde-derived structs.
- **`crates/op-gateway/src/encrypted_storage.rs:49`**: The key metadata and storage entry contracts (`EncryptedKeyEntry`, `KeyType`) are expressed as ad-hoc serializable structures instead of structured, versioned schemas.
- **`crates/op-gateway/src/mcp_gateway.rs:17`**: Client-facing routing structures (`RoutingDecision`, `AccessLevel`, `McpClientInfo`, `McpSession`) are defined as ad-hoc structures.
- **`crates/op-gateway/src/wireguard_auth.rs:197`**: Key metadata and session structures (`WireGuardSession`, `ClientInfo`, `SessionFilter`) are defined as ad-hoc Rust structs.

### Ad-Hoc D-Bus Payloads Bypassing Schema Definitions
- **`crates/op-gateway/src/mcp_gateway.rs:240` (`dbus_route_client`)**: Returns raw, untyped `simd_json::OwnedValue` objects mapped directly using the `json!` macro.
- **`crates/op-gateway/src/mcp_gateway.rs:266` (`dbus_validate_session`)**: Returns ad-hoc untyped JSON values representing validation states.
- **`crates/op-gateway/src/mcp_gateway.rs:275` (`dbus_get_capabilities`)**: Returns ad-hoc untyped JSON arrays representing client capabilities over D-Bus.

These D-Bus interfaces should be defined using structured, versioned Protocol Buffer schemas or strongly typed, code-generated D-Bus interfaces rather than arbitrary JSON blobs.

### Ad-Hoc Database Columns
- **`crates/op-gateway/src/wireguard_auth.rs:91`**: The database persistence layer serializes session properties as unstructured JSON strings inside SQLite text columns (`flags` field), completely bypassing database schema typing and versioning.

---

# 3. Production Security & Quality Audit

## Critical Vulnerabilities (Directly Exploitable)

### 3.1 Catastrophic ChaCha20Poly1305 Nonce Reuse on Service Restart
- **File:** `crates/op-gateway/src/encrypted_storage.rs:275` (and `341`)
- **Impact:** **Critical** (Directly Exploitable)
- **Description:** 
  In `load_master_key` (`encrypted_storage.rs:275`), when the master key is loaded from the filesystem, the `nonce_counter` inside the `MasterKey` struct is hardcoded to reset to `0`:
  ```rust
  self.master_key = Some(MasterKey {
      key,
      salt,
      nonce_counter: 0,
  });
  ```
  During the `store_key` execution (`encrypted_storage.rs:341`), the AEAD nonce is derived entirely from this in-memory counter:
  ```rust
  let mut nonce = [0u8; 12];
  let nonce_counter = master_key.nonce_counter;
  nonce[4..12].copy_from_slice(&nonce_counter.to_le_bytes());
  master_key.nonce_counter += 1;
  ```
  Every time the `op-gateway` service restarts, the `nonce_counter` resets to `0`. Consequently, writing or updating any keys after a restart reuses the exact same nonces (`0`, `1`, `2`, etc.) under the same static master key. 
  
  Nonce reuse in ChaCha20Poly1305 is cryptographically fatal. It allows an attacker with read access to the encrypted storage to perform a "forbidden attack," breaking ciphertext confidentiality (allowing plaintext recovery of the stored WireGuard private keys and session tokens) and potentially allowing message forgery.

---

### 3.2 Predictable Session ID Generation (Zero Cryptographic Entropy)
- **File:** `crates/op-gateway/src/wireguard_auth.rs:511`
- **Impact:** **Critical** (Directly Exploitable)
- **Description:** 
  The function `generate_session_id` constructs session identifiers using a purely deterministic string hashing approach:
  ```rust
  let input = format!("WG-SESSION-{}-{}", peer_pubkey, Self::current_timestamp());
  let session_ids = self
      .crypto_engine
      .generate_session_ids_batch(&[input.as_bytes()]);
  ```
  The input consists only of a static string, the public key of the client (`peer_pubkey`), and the low-resolution system timestamp in seconds. It contains no random entropy from a cryptographically secure random number generator (CSPRNG). 
  
  Because peer public keys are transmitted in plaintext during handshakes and are publicly known, an attacker can easily brute-force or compute active session IDs by guessing recent timestamps (which have a low search space of 1 second intervals). Since session IDs are validated to grant API access (`mcp_gateway.rs:193`), this allows an attacker to hijack active client sessions and query restricted MCP tools.

---

### 3.3 Cryptographic Key Derivation of "Secret" PSK from Public Keys
- **File:** `crates/op-gateway/src/wireguard_auth.rs:662` (and `525`)
- **Impact:** **Critical** (Directly Exploitable)
- **Description:** 
  The system attempts to automatically derive a "pre-shared key" (PSK) for clients:
  ```rust
  pub fn derive_stable_psk(&self, peer_key: &[u8; 32]) -> Vec<[u8; 32]> {
      ...
      let salt = b"WG-STABLE-PSK-2024";
      let mut input = Vec::with_capacity(39);
      input.extend_from_slice(b"WG-PSK-");
      input.extend_from_slice(peer_key);

      let argon2 = Argon2::default();
      let mut psk = [0u8; 32];
      if argon2.hash_password_into(&input, salt, &mut psk).is_ok() {
          results.push(psk);
      }
      results
  }
  ```
  The input used to derive the "secret" PSK consists solely of the public `peer_key` (the client's WireGuard public key) and a static salt. There are no server-side secrets or private key material incorporated into this KDF.
  
  Because a client's public key is public information, any network eavesdropper or malicious client can run this identical Argon2 derivation using the known public key and the hardcoded salt. This completely defeats the security guarantees of the WireGuard PSK (which relies on mutual pre-shared secrecy), allowing any unauthorized entity to compute the PSK of any legitimate client and bypass authentication.

---

## High and Medium Security Risks

### 3.4 Plaintext Fallback of Sensitive Key Storage on Initialization Failures
- **File:** `crates/op-gateway/src/encrypted_storage.rs:149` (and `234`)
- **Impact:** **High**
- **Description:** 
  The storage manager is designed to fallback to unencrypted regular subvolumes/directories if experimental Btrfs native encryption is unsupported or if LUKS setup is not manually configured:
  ```rust
  // Fallback to regular subvolume if encryption not supported
  if stderr.contains("encryption not supported") || stderr.contains("invalid option") {
      warn!("Native Btrfs encryption not supported, creating regular subvolume");
      self.create_regular_subvolume().await?;
  }
  ```
  If this silent fallback triggers (which will be the case on almost all standard production kernels where experimental Btrfs encryption is disabled), the system proceeds to function without encryption. Crucially, the master key generation (`encrypted_storage.rs:296`) writes the raw master key and salt directly to disk at `self.storage_path/master.key` without any password-based derivation or key-wrap mechanism:
  ```rust
  let mut key_data = Vec::with_capacity(64);
  key_data.extend_from_slice(&key);
  key_data.extend_from_slice(&salt);
  async_fs::write(path, &key_data).await?;
  ```
  This results in highly sensitive WireGuard private keys and session encryption keys being stored on disk in plaintext.

---

### 3.5 Lack of Memory Zeroization for Temporary Buffers and Decrypted Keys
- **File:** `crates/op-gateway/src/encrypted_storage.rs:260` (and `291`, `wireguard_auth.rs:434`)
- **Impact:** **Medium**
- **Description:** 
  While the `MasterKey` struct correctly implements `Zeroize` and `ZeroizeOnDrop`, the temporary variables containing raw key material and salts during load/generation operations (`key`, `salt`, and `key_data` buffers) do not implement `Zeroize`. 
  Furthermore, `retrieve_private_key` in `wireguard_auth.rs:434` returns the raw WireGuard private key as a plain, unzeroized `[u8; 32]` array. This leaves sensitive cryptographic key material residing in dirty heap and stack pages long after the operations have completed, making them vulnerable to extraction via memory dumping or compromise of co-located processes.

---

### 3.6 TOCTOU Race Condition in File Permission Hardening
- **File:** `crates/op-gateway/src/encrypted_storage.rs:159` (and `224`)
- **Impact:** **Medium**
- **Description:** 
  In `setup_native_btrfs_encryption` and `create_regular_subvolume`, the directory/subvolume is created using default system creation masks first, and then tightened afterward using `set_secure_permissions()`:
  ```rust
  // Fallback to regular directory
  warn!("Btrfs not available, using regular directory");
  async_fs::create_dir_all(&self.storage_path).await?;
  ...
  self.set_secure_permissions().await?;
  ```
  This introduces a classic Time-of-Check to Time-of-Use (TOCTOU) race condition. For a brief window, the directory holding master keys and session tokens is created with default permissions (such as `0755` or `0777`), allowing unauthorized local users or co-located containers to open or read files before the permission mask is updated to `0700`.

---

### 3.7 Non-Cryptographic Session Key Derivation Without Server-Side Secrecy
- **File:** `crates/op-gateway/src/wireguard_auth.rs:680`
- **Impact:** **Medium**
- **Description:** 
  In `derive_session_keys`, the session keys are derived using Argon2 from the client's public key (`peer_key`) and the generated `server_nonce`:
  ```rust
  let mut input = Vec::with_capacity(71);
  input.extend_from_slice(b"WG-SESSION-");
  input.extend_from_slice(peer_key);
  input.extend_from_slice(server_nonce);
  ```
  There is no static server-side private key (such as the server's private WireGuard key) incorporated into this derivation. If the `server_nonce` is communicated to the client over an unauthenticated channel, any passive adversary observing the network can capture the nonce, combine it with the client's public key, and compute the identical "session key", fully compromising the confidentiality of the session-key encrypted transport.

---

## Code Quality and Reliability Findings

### 3.8 Highly Fragile Parsing of Shell Output for Filesystem Metrics
- **File:** `crates/op-gateway/src/encrypted_storage.rs:440`
- **Impact:** **Low / Quality**
- **Description:** 
  The system executes a system process calling `df -T` and attempts to parse the whitespace-separated output lines to determine disk statistics:
  ```rust
  let output = Command::new("df")
      .args(["-T", self.storage_path.to_str().unwrap()])
      .output()
  ...
  let fields: Vec<&str> = lines[1].split_whitespace().collect();
  ```
  This parsing mechanism is highly fragile and prone to failure in environments where filesystem mount paths contain spaces, where customized localized headers are configured, or within containerized/chrooted systems where the `df` command-line utility is missing or restricted. It is highly recommended to use native system calls (e.g., `statfs` via the `libc` or `nix` crates) rather than shell command execution.