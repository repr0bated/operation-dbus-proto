# Production Security & Quality Audit: `op-gateway`

---

### 1. Memory Map Table

Per the memory mapping architecture rules, all occurrences of memory-mapped I/O and large allocations within the audited codebase are documented below. 

| Site | file:line | Type (ro/rw/sled) | Risk |
| :--- | :--- | :--- | :--- |
| `dd` image file allocation | `crates/op-gateway/src/encrypted_storage.rs:186` | Disk Allocation (100MB fallback image) | **Low** - Executed via system command fallback when native partition is unavailable. Potentially triggers synchronous I/O blocks. |

*Note: Sled and direct `memmap2` instances are listed as dependencies in the workspace `Cargo.toml`, but no direct `mmap`, `memmap2`, `MmapMut`, or active Sled databases are instantiated within the provided gateway source files.*

---

### 2. Critical Security Vulnerabilities

#### [CRITICAL] Cryptographic Nonce Reuse via Stateful Reset
- **File & Line**: `crates/op-gateway/src/encrypted_storage.rs:289`, `crates/op-gateway/src/encrypted_storage.rs:323`
- **Vulnerability Type**: Nonce Reuse in Authenticated Encryption (ChaCha20-Poly1305)
- **Description**: 
  The key storage manager uses `ChaCha20Poly1305` to encrypt WireGuard private keys and session keys. It relies on a stateful `nonce_counter` on the `MasterKey` instance to generate unique nonces:
  ```rust
  let mut nonce = [0u8; 12];
  let nonce_counter = master_key.nonce_counter;
  nonce[4..12].copy_from_slice(&nonce_counter.to_le_bytes());
  master_key.nonce_counter += 1;
  ```
  However, in `load_master_key`, every time the service reloads the master key from disk (e.g., during process restart or re-initialization), the `nonce_counter` is hardcoded to reset back to `0`:
  ```rust
  self.master_key = Some(MasterKey {
      key,
      salt,
      nonce_counter: 0,
  });
  ```
- **Exploitability**: 
  Directly exploitable. An attacker who has access to the encrypted key directory (`/var/lib/op-dbus/encrypted`) can observe multiple `.key` files (or updates to the same key) encrypted under the exact same master key and identical nonce values (`0`, `1`, `2`, etc.) after every service restart. Nonce reuse in stream ciphers like ChaCha20 completely breaks confidentiality, enabling ciphertext decryption and the recovery of sensitive WireGuard private keys and pre-shared keys (PSKs).

---

### 3. High Severity Vulnerabilities

#### [HIGH] Undefined Behavior via Unpadded `simd_json::from_str`
- **File & Line**: `crates/op-gateway/src/encrypted_storage.rs:377`, `crates/op-gateway/src/wireguard_auth.rs:177`
- **Vulnerability Type**: Memory Safety / Undefined Behavior
- **Description**: 
  The codebase uses the `unsafe` function `simd_json::from_str` on unpadded buffers:
  ```rust
  // encrypted_storage.rs
  let entry_json = async_fs::read_to_string(&key_file_path).await?;
  let mut entry_str = entry_json.clone();
  let entry: EncryptedKeyEntry = unsafe { simd_json::from_str(&mut entry_str) }?;
  ```
  ```rust
  // wireguard_auth.rs
  let flags_json: String = row.get("flags");
  let mut flags_str = flags_json.clone();
  let flags: std::collections::HashMap<String, String> =
      unsafe { simd_json::from_str(&mut flags_str) }.unwrap_or_default();
  ```
  `simd_json` relies heavily on vector registers (AVX2/SSE) and assumes that input buffers are padded with at least `simd_json::SIMD_JSON_PADDING` (typically 32 or 64 bytes) of extra addressable space at the end to prevent out-of-bounds reads. Passing a standard cloned `String` directly to `simd_json::from_str` violates this invariant.
- **Impact**: 
  If the JSON string ends near a page boundary, SIMD instructions reading past the end of the string buffer will trigger page faults, segmentation faults, or memory leak disclosures.

#### [HIGH] Plaintext Storage of Master Cryptographic Key on Disk
- **File & Line**: `crates/op-gateway/src/encrypted_storage.rs:304`
- **Vulnerability Type**: Cryptographic Material Exposure
- **Description**: 
  The storage manager is described as "encrypted storage for WireGuard keys." However, `generate_master_key` writes the raw, unencrypted master key directly to `/var/lib/op-dbus/encrypted/wireguard-keys/master.key`:
  ```rust
  let mut key_data = Vec::with_capacity(64);
  key_data.extend_from_slice(&key);
  key_data.extend_from_slice(&salt);

  async_fs::write(path, &key_data).await?;
  ```
  Although file permissions are restricted to `0o600`, storing the master key in plaintext on the same filesystem completely defeats the objective of encrypting individual keys.
- **Impact**: 
  If the disk is compromised, or an attacker obtains local read access to the master key file, they can instantly bypass all cryptographic protections applied to the stored WireGuard credentials.

---

### 4. Medium Severity & Performance Hazards

#### [MEDIUM] Denial of Service via Argon2 Password Hashing on Hot Paths
- **File & Line**: `crates/op-gateway/src/wireguard_auth.rs:674`, `crates/op-gateway/src/wireguard_auth.rs:692`
- **Vulnerability Type**: Denial of Service (DoS) / Resource Exhaustion
- **Description**: 
  The application utilizes a Password Hashing Function (`Argon2`) on high-frequency symmetric key derivation paths:
  ```rust
  // Inside derive_stable_psk
  let argon2 = Argon2::default();
  let mut psk = [0u8; 32];
  if argon2.hash_password_into(&input, salt, &mut psk).is_ok() { ... }
  ```
  `Argon2` is designed to be intentionally slow and memory-hard to prevent offline brute-force attacks on weak user passwords. It should never be used to derive session keys or stable PSKs from already cryptographically strong keys (e.g., 256-bit peer public keys).
- **Impact**: 
  When clients establish connections or trigger key rotations, the gateway executes `Argon2::default()` (which utilizes considerable memory and CPU cycles). An external attacker can flood the gateway with connection handshakes or rotation requests, completely exhausting CPU and memory resources, causing a denial of service (DoS) for the entire control plane. Symmetric key derivation should utilize fast algorithms like HKDF-SHA256 or BLAKE2/BLAKE3.

#### [LOW] Redundant Heap Allocations on Hot Paths
- **File & Line**: `crates/op-gateway/src/mcp_gateway.rs:115`, `crates/op-gateway/src/mcp_gateway.rs:251`
- **Vulnerability Type**: Performance Degredation
- **Description**: 
  - On line 115, `route_client` allocates two new `Vec`s (`allowed_tools` and `capabilities`) packed with newly allocated `String`s for every single client routing decision:
    ```rust
    allowed_tools: vec![
        "list_tools".to_string(),
        "search_tools".to_string(),
        "get_tool_schema".to_string(),
        "execute_tool".to_string(),
        "cognitive_reason".to_string(),
        "compact_summarize".to_string(),
    ]
    ```
  - On line 251, `generate_cache_key` allocates an ad-hoc `Vec<String>` of cloned properties to hash client context.
- **Impact**: 
  High frequency allocations and deallocations trigger heap thrashing and garbage collector cycles under heavy traffic. These should be statically defined structures or borrow arrays.

---

### 5. Schema-as-Code Violations

The codebase mandates that all data contracts crossing process boundaries, storage layers, and interfaces be structured using versioned schemas (such as Protocol Buffers or OSCAL-compliant definitions). Ad-hoc structs, raw JSON formatting, and inline SQL schemas represent architecture violations.

#### 1. Ad-hoc Rust Structs for Process and Route State
- **File & Line**: `crates/op-gateway/src/mcp_gateway.rs:18-60`
- **Description**: The structs `RoutingDecision`, `AccessLevel`, `McpClientInfo`, and `McpSession` define the client/server contract for the Compact MCP Gateway. They are formulated as ad-hoc Serde JSON representations rather than versioned Protobuf models.

#### 2. Raw Dynamic JSON Payload Generation over D-Bus
- **File & Line**: `crates/op-gateway/src/mcp_gateway.rs:324`
- **Description**: D-Bus communication is marshaled as raw, unversioned, dynamic JSON values via `simd_json::json!` macro payloads:
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
  Changes to gateway state fields will result in silently broken integration with D-Bus consumers due to the lack of strict schema validation.

#### 3. Embedded Database Schema Definitions
- **File & Line**: `crates/op-gateway/src/wireguard_auth.rs:44`
- **Description**: The schema for storing active sessions is managed as an inline SQL string in raw code:
  ```rust
  sqlx::query(
      r#"
      CREATE TABLE IF NOT EXISTS wireguard_sessions (
          session_id TEXT PRIMARY KEY,
          ...
      )
  "#)
  ```
  Instead of being managed through versioned migration schemas or unified declarative schema-as-code formats, migrations are run procedurally at runtime.