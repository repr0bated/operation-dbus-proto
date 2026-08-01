# Production Security & Quality Audit: op-gateway

---

## 1. Test Audit (ROLE: Tests)

* **Total Test Functions**: 0
* **Representative Tests**: None
* **Property-Based Testing & Fuzzing**: None

### Risk Assessment: High Risk
**No tests found.** The `op-gateway` crate contains zero unit tests, integration tests, or property-based tests. Crucial operations—including session routing, credential validation, cryptographic key derivation, and file permissions enforcement—possess no automated validation, making regressions or silent security failures extremely likely during maintenance.

---

## 2. Schema-As-Code Discipline Violations

This codebase uses ad-hoc serializable structures and hand-crafted serialization mappings rather than versioned, formalized schemas (such as Protocol Buffers or OSCAL documents) for its API boundaries, storage, and D-Bus interfaces. This risks wire-format drift, payload parsing errors, and service incompatibilities.

### Ad-hoc Struct Data Contracts
* **`crates/op-gateway/src/mcp_gateway.rs:36`**: `McpClientInfo` is defined as an ad-hoc Serde/simd-json struct used to represent client identity.
* **`crates/op-gateway/src/mcp_gateway.rs:47`**: `McpSession` is defined as an ad-hoc Rust struct used to track state across boundaries.
* **`crates/op-gateway/src/mcp_gateway.rs:13`**: `RoutingDecision` is defined as an ad-hoc serializable boundary contract for MCP client routing.
* **`crates/op-gateway/src/wireguard_auth.rs:184`**: `WireGuardSession` is an ad-hoc serializable representation of WireGuard identities stored in a local SQLite database.
* **`crates/op-gateway/src/wireguard_auth.rs:569`**: `ClientInfo` is an ad-hoc structure for client environments.
* **`crates/op-gateway/src/wireguard_auth.rs:577`**: `SessionFilter` is an ad-hoc filtering structure.

### Ad-hoc Hand-Crafted JSON & String Serialization
* **`crates/op-gateway/src/mcp_gateway.rs:341`**: The D-Bus routing interface constructs and serializes untyped JSON values manually using `simd_json::json!`.
* **`crates/op-gateway/src/mcp_gateway.rs:356`**: Hand-crafted JSON conversion for D-Bus validation payloads instead of a shared protobuf schema.
* **`crates/op-gateway/src/wireguard_auth.rs:80`**: Hand-crafted serialization of metadata maps (`session.flags`) into untyped JSON strings inside SQLite database commands.
* **`crates/op-gateway/src/wireguard_auth.rs:151`**: Ad-hoc deserialization of `flags` using `unsafe` blocks on database fields: `unsafe { simd_json::from_str(&mut flags_str) }`.

---

## 3. Technical Vulnerability Audit

### [CRITICAL] Catastrophic ChaCha20-Poly1305 Nonce Reuse on Service Restart
* **File:Line**: `crates/op-gateway/src/encrypted_storage.rs:298`, `crates/op-gateway/src/encrypted_storage.rs:327`, `crates/op-gateway/src/encrypted_storage.rs:347`
* **Mechanism**: In `crates/op-gateway/src/encrypted_storage.rs:347`, key storage relies on a stateful `nonce_counter` from the active `MasterKey` object to create 12-byte nonces for ChaCha20-Poly1305 encryption:
  ```rust
  let mut nonce = [0u8; 12];
  let nonce_counter = master_key.nonce_counter;
  nonce[4..12].copy_from_slice(&nonce_counter.to_le_bytes());
  master_key.nonce_counter += 1;
  ```
  However, `nonce_counter` is strictly in-memory. Whenever the service restarts, the `MasterKey` is re-initialized by either `load_master_key` (`crates/op-gateway/src/encrypted_storage.rs:298`) or `generate_master_key` (`crates/op-gateway/src/encrypted_storage.rs:327`), both of which hardcode:
  ```rust
  nonce_counter: 0,
  ```
  Consequently, upon every service restart, the exact same sequence of nonces (starting at `0`) is reused with the same static `master.key` file to encrypt newly saved entries.
* **Exploitability**: **Directly Exploitable.** An attacker with read access to the encrypted subvolume directory can retrieve different ciphertexts that have been encrypted with the exact same nonce and master key. Simple XOR analysis of identical nonces under stream ciphers (like ChaCha20) collapses the cryptographic security entirely, exposing WireGuard private keys in plain text.

---

### [HIGH] Silent Security Degradation / Fail-Open Fallback to Unencrypted Subvolumes
* **File:Line**: `crates/op-gateway/src/encrypted_storage.rs:149`, `crates/op-gateway/src/encrypted_storage.rs:232`
* **Mechanism**: In `setup_native_btrfs_encryption`, if creating an encrypted Btrfs subvolume fails (for instance, because the running kernel does not support experimental Btrfs encryption or rejects the `-e` flag), the code traps the error and silently drops back to an unencrypted, regular subvolume or directory:
  ```rust
  if stderr.contains("encryption not supported") || stderr.contains("invalid option") {
      warn!("Native Btrfs encryption not supported, creating regular subvolume");
      self.create_regular_subvolume().await?;
  }
  ```
  In `setup_luks_encryption` (`crates/op-gateway/src/encrypted_storage.rs:232`), the setup warns that "LUKS setup requires manual intervention" and proceeds to silently fall back to an unencrypted regular subvolume:
  ```rust
  warn!("LUKS setup requires manual intervention - using test passphrase");
  // Create regular subvolume for now
  self.create_regular_subvolume().await?;
  ```
* **Exploitability**: If the host environment lack native Btrfs encryption support or manual LUKS intervention is not provided, the application silently degrades into storing highly sensitive WireGuard private keys and session keys in plain text on standard disks without returning an initialization error. This violates the fail-secure principle.

---

### [HIGH] Plaintext Master Key Persistence on Local File System
* **File:Line**: `crates/op-gateway/src/encrypted_storage.rs:312`
* **Mechanism**: When generating the key storage configuration, the gateway creates a raw cryptographic master key and salt directly from the system entropy pool and writes them *unencrypted* to the local file system:
  ```rust
  let mut key_data = Vec::with_capacity(64);
  key_data.extend_from_slice(&key);
  key_data.extend_from_slice(&salt);

  async_fs::write(path, &key_data).await?;
  ```
* **Exploitability**: Any local adversary or compromised container with access to `/var/lib/op-dbus/encrypted/wireguard-keys/master.key` can immediately extract the decryption key, entirely bypassing the cryptographic confidentiality controls of the underlying storage model. Security relies completely on file system permissions (`0o600`), rendering the secondary encryption layer useless if those permissions are misconfigured or bypassed.

---

### [MEDIUM] Unsafe Use of `simd_json::from_str` on File-System and Database Payloads
* **File:Line**: `crates/op-gateway/src/encrypted_storage.rs:411`, `crates/op-gateway/src/wireguard_auth.rs:151`
* **Mechanism**: The codebase utilizes the `unsafe` variant of `simd_json::from_str` to parse JSON files and database fields:
  ```rust
  // crates/op-gateway/src/encrypted_storage.rs:411
  let entry: EncryptedKeyEntry = unsafe { simd_json::from_str(&mut entry_str) }?;

  // crates/op-gateway/src/wireguard_auth.rs:151
  let flags: std::collections::HashMap<String, String> =
      unsafe { simd_json::from_str(&mut flags_str) }.unwrap_or_default();
  ```
* **Exploitability**: The `unsafe` API in `simd-json` expects that the input string is mutable, padded, and structured in a way that respects SIMD boundary conditions. Using it on strings sourced directly from external files or SQLite rows introduces potential undefined behavior or memory access violations if the serialized data on disk is corrupted, malformed, or intentionally truncated by an attacker. Safe alternatives (`simd_json::from_str` or `serde_json`) should be used on unvalidated disk contents.

---
## ⚠ Citation Warnings
- `crates/op-gateway/src/mcp_gateway.rs:341`: file has 337 lines
- `crates/op-gateway/src/mcp_gateway.rs:356`: file has 337 lines
