# Quality and Security Audit Report: `op-gateway`

This document contains the production security, quality, and architectural audit of the `op-gateway` crate. 

---

## 1. Documentation & Crate Quality Audit

### Crate-Level Documentation
* **Status**: Partially Compliant.
* **Findings**: The crate-level `//!` documentation in `crates/op-gateway/src/lib.rs:1` is present but extremely brief:
  ```rust
  //! op-gateway: MCP Gateway with WireGuard authentication and smart routing
  ```
  It lacks comprehensive details about architecture, deployment requirements, dependency requirements (such as the SQLite database path or Btrfs subvolumes), and configuration models.

### README.md Presence
* **Status**: **Non-Compliant**.
* **Findings**: No `README.md` file is provided in the `crates/op-gateway/` directory or workspace root to explain deployment invariants, manual setup steps, or security models.

### Public Item Documentation (15 Sample Items Checked)
* **Status**: Partially Compliant. Multiple critical public structures and enums completely lack `/// rustdoc` comments.

| Public Item | File & Line Citation | Status | Details |
| :--- | :--- | :--- | :--- |
| `pub struct EncryptedStorageConfig` | `crates/op-gateway/src/encrypted_storage.rs:17` | Compliant | Has doc comments. |
| `pub struct KdfParams` | `crates/op-gateway/src/encrypted_storage.rs:30` | **Non-Compliant** | Missing `///` rustdoc. |
| `pub struct EncryptedKeyStorage` | `crates/op-gateway/src/encrypted_storage.rs:38` | **Non-Compliant** | Missing `///` rustdoc. |
| `pub struct EncryptedKeyEntry` | `crates/op-gateway/src/encrypted_storage.rs:50` | Compliant | Has doc comments. |
| `pub enum KeyType` | `crates/op-gateway/src/encrypted_storage.rs:61` | **Non-Compliant** | Missing `///` rustdoc. |
| `pub enum GatewayError` | `crates/op-gateway/src/error.rs:4` | **Non-Compliant** | Missing `///` rustdoc. |
| `pub struct RoutingDecision` | `crates/op-gateway/src/mcp_gateway.rs:16` | Compliant | Has doc comments. |
| `pub enum AccessLevel` | `crates/op-gateway/src/mcp_gateway.rs:27` | Compliant | Has doc comments. |
| `pub struct McpClientInfo` | `crates/op-gateway/src/mcp_gateway.rs:38` | Compliant | Has doc comments. |
| `pub struct McpSession` | `crates/op-gateway/src/mcp_gateway.rs:49` | Compliant | Has doc comments. |
| `pub struct McpGatewayManager` | `crates/op-gateway/src/mcp_gateway.rs:60` | Compliant | Has doc comments. |
| `pub struct WireGuardDatabase` | `crates/op-gateway/src/wireguard_auth.rs:25` | Compliant | Has doc comments. |
| `pub struct WireGuardSession` | `crates/op-gateway/src/wireguard_auth.rs:198` | Compliant | Has doc comments. |
| `pub struct WireGuardStats` | `crates/op-gateway/src/wireguard_auth.rs:215` | Compliant | Has doc comments. |
| `pub struct WireGuardAuthManager` | `crates/op-gateway/src/wireguard_auth.rs:230` | Compliant | Has doc comments. |

### Public Unsafe Functions & Invariants
* **Status**: Compliant. There are no public `unsafe fn` declarations within the crate.

---

## 2. Schema-as-Code Discipline Audit

The `op-gateway` system utilizes ad-hoc serialized JSON structures and untyped key-value maps to define critical contracts. This breaks the schema-as-code discipline, which mandates the use of versioned Protocol Buffers or structured schemas like OSCAL for data serialization and interface interchange.

### Ad-Hoc D-Bus JSON Payload Interchange
* **File**: `crates/op-gateway/src/mcp_gateway.rs:293-339`
* **Violations**: The public D-Bus API methods return ad-hoc `simd_json::OwnedValue` payloads constructed dynamically using the `json!` macro rather than versioned schema structs:
  ```rust
  // crates/op-gateway/src/mcp_gateway.rs:308-320
  Ok(json!({
      "endpoint": routing_decision.endpoint,
      "allowed_tools": routing_decision.allowed_tools,
      "capabilities": routing_decision.capabilities,
      "has_full_access": routing_decision.has_full_access,
      "session_id": routing_decision.session_id,
      "access_level": match routing_decision.access_level { ... }
  }))
  ```
  Any change to these dynamic fields risks breaking other control-plane components reading from D-Bus without compilation errors.

### Ad-Hoc Storage Schemas
* **Files**: 
  * `crates/op-gateway/src/encrypted_storage.rs:50` (`EncryptedKeyEntry`)
  * `crates/op-gateway/src/wireguard_auth.rs:198` (`WireGuardSession`)
* **Violations**: WireGuard session options and entry metadata are stored as ad-hoc nested key-value strings (`HashMap<String, String>`) under the `flags` and `metadata` fields rather than being defined in versioned schemas.

---

## 3. Cryptographic and Security Vulnerabilities

### Finding 1: Silent Fallback to Unencrypted Storage & Plaintext Master Key Storage (CRITICAL)
* **File**: `crates/op-gateway/src/encrypted_storage.rs:125-157` (Btrfs), `crates/op-gateway/src/encrypted_storage.rs:197-204` (LUKS), `crates/op-gateway/src/encrypted_storage.rs:284-288` (Plaintext Key)
* **Vulnerability Description**:
  The storage manager is configured to use experimental native Btrfs subvolume encryption by default (`use_native_encryption: true`). If the underlying system does not support this experimental feature (e.g., standard upstream kernels or older toolchains), the initialization silently falls back to an unencrypted subvolume or directory via `create_regular_subvolume()`, logging only a `warn!`.
  
  Furthermore, the fallback for LUKS encryption bypasses LUKS entirely and sets up an unencrypted directory:
  ```rust
  // crates/op-gateway/src/encrypted_storage.rs:197-203
  warn!("LUKS setup requires manual intervention - using test passphrase");
  // Create regular subvolume for now
  self.create_regular_subvolume().await?;
  ```
  
  Crucially, `generate_master_key` writes the master cryptographic key **completely in plaintext** to this potentially unencrypted fallback path:
  ```rust
  // crates/op-gateway/src/encrypted_storage.rs:284-288
  // Store encrypted key (in production, encrypt with user passphrase)
  let mut key_data = Vec::with_capacity(64);
  key_data.extend_from_slice(&key);
  key_data.extend_from_slice(&salt);

  async_fs::write(path, &key_data).await?;
  ```
* **Impact**:
  A local attacker with read capabilities on the filesystem can retrieve `/var/lib/op-dbus/encrypted/wireguard-keys/master.key` (which is stored in plaintext on an unencrypted volume) and immediately decrypt all WireGuard private keys stored within the database.
* **Remediation**:
  1. Abort execution immediately if the requested encryption mechanism (Btrfs Native or LUKS) fails to initialize. Do not perform silent unencrypted fallbacks.
  2. Implement proper user-passphrase key derivation (e.g., PBKDF2/Argon2) to encrypt the master key on disk rather than storing it in raw bytes.

---

### Finding 2: Cryptographic Nonce Reuse in ChaCha20-Poly1305 (CRITICAL)
* **File**: `crates/op-gateway/src/encrypted_storage.rs:259-270` (Counter Reset), `crates/op-gateway/src/encrypted_storage.rs:308-313` (Counter Usage)
* **Vulnerability Description**:
  The application utilizes `ChaCha20Poly1305` for storing sensitive keys. To prevent nonce reuse, it maintains a `nonce_counter` on the `MasterKey` structure. However, this `nonce_counter` is **never persisted to disk**.
  
  Every time the gateway process restarts, the counter is initialized back to `0`:
  ```rust
  // crates/op-gateway/src/encrypted_storage.rs:266-270
  self.master_key = Some(MasterKey {
      key,
      salt,
      nonce_counter: 0,
  });
  ```
* **Impact**:
  Reusing a nonce with the same key in stream ciphers like ChaCha20 completely destroys the confidentiality of the ciphertexts. An attacker who can capture or observe multiple versions of the encrypted key files (e.g., before and after a reboot) can perform a simple multi-ciphertext XOR analysis to recover the plaintexts of the private keys.
* **Remediation**:
  Use a cryptographically secure random number generator (`ring::rand::SecureRandom`) to generate a unique 12-byte random nonce for *every* encryption operation, or persist the state of the monotonic counter safely to storage.

---

### Finding 3: Undefined Behavior in `simd-json` Parsing via Unpadded Buffers
* **File**: `crates/op-gateway/src/encrypted_storage.rs:406`, `crates/op-gateway/src/wireguard_auth.rs:173-174`
* **Vulnerability Description**:
  The parsing of `EncryptedKeyEntry` and `flags` database values relies on `simd_json::from_str`. The input buffer to `simd-json` **must** be mutable and have padding of at least `simd_json::SIMDJSON_PADDING` bytes at the end of the allocation to prevent out-of-bounds reads during vectorized parsing operations. Passing standard cloned strings directly to `simd_json::from_str` violates this safety contract:
  ```rust
  // crates/op-gateway/src/encrypted_storage.rs:405-406
  let mut entry_str = entry_json.clone();
  let entry: EncryptedKeyEntry = unsafe { simd_json::from_str(&mut entry_str) }?;
  ```
  ```rust
  // crates/op-gateway/src/wireguard_auth.rs:172-174
  let mut flags_str = flags_json.clone();
  let flags: std::collections::HashMap<String, String> =
      unsafe { simd_json::from_str(&mut flags_str) }.unwrap_or_default();
  ```
* **Impact**:
  Vectorized read operations can overrun the buffer boundary, resulting in memory segmentation faults, undefined behavior, or information disclosure (leaking surrounding heap memory).
* **Remediation**:
  Use `simd_json::to_vec` or ensure the mutable string is converted to a vector and padded with the necessary padding size using `simd_json::SIMDJSON_PADDING` before invoking the deserializer.

---

### Finding 4: Insecure Command Execution and Path Resolution
* **File**: `crates/op-gateway/src/encrypted_storage.rs:133`, `172`, `215`, `226`, `452`
* **Vulnerability Description**:
  The storage manager runs command-line binaries (`btrfs`, `dd`, `mount`, `df`) via `Command::new` using bare relative names without verifying their absolute paths.
* **Impact**:
  If the application is run in an environment where the `PATH` environment variable is compromised or mutable by unauthorized local users, path-traversal/binary hijacking can lead to arbitrary privilege escalation when executing commands like `mount` as root.
* **Remediation**:
  Define absolute paths for all system utilities (e.g., `/usr/bin/btrfs`, `/bin/dd`, `/bin/mount`, `/bin/df`) and enforce rigorous validation of all executing environments.

---

### Finding 5: Potential Denial of Service / Crash via Subtraction Overflow
* **File**: `crates/op-gateway/src/mcp_gateway.rs:249-253`
* **Vulnerability Description**:
  When cleaning up expired sessions, the manager calculates the elapsed time by subtracting the session's `last_used` timestamp from the current system time:
  ```rust
  // crates/op-gateway/src/mcp_gateway.rs:252-253
  // Sessions expire after 1 hour of inactivity
  if now - session.last_used > 3600 {
  ```
* **Impact**:
  If the host system's clock is synchronized backward (e.g., via NTP adjustment) so that `now` is strictly less than `session.last_used`, this subtraction will cause an **integer underflow**. In debug mode, this immediately panics the program, causing a Denial of Service. In release mode, the wrap-around will create an extremely large value, causing active sessions to be incorrectly classified as expired and immediately dropped.
* **Remediation**:
  Use safe arithmetic operations, such as `checked_sub` or `saturating_sub`:
  ```rust
  if now.saturating_sub(session.last_used) > 3600 {
  ```