### Dependencies & Feature Inventory

The following table lists all direct dependencies specified in `crates/op-gateway/Cargo.toml` alongside their version requirements, explicitly enabled features, and security/architectural notes.

| Dependency | Version | Explicitly Enabled Features | Pulled in by Default | Notes / Vulnerability Risks |
|---|---|---|---|---|
| `tokio` | `1` | `["full"]` | Yes | Heavily used async runtime. Large attack surface due to `full` feature set. |
| `serde` | `1` | `["derive"]` | Yes | Standard serialization; used for ad-hoc JSON structs. |
| `simd-json` | `0.13` | None | Yes | High-performance JSON parser. Uses unsafe bindings; sensitive to input padding (see findings). |
| `ring` | `0.17` | None | Yes | Cryptographic assembly primitives. Used for secure random generation. |
| `x25519-dalek`| `2.0` | None | Yes | Diffie-Hellman key exchange primitive. |
| `chacha20poly1305` | `0.10` | None | Yes | AEAD engine used for local file encryption. |
| `argon2` | `0.5` | `["std"]` | Yes | Password hashing / KDF. Used for stable PSK derivation. |
| `blake2` | `0.10` | None | Yes | Hashing algorithm. Used in the SIMD engine. |
| `zeroize` | `1.6` | `["zeroize_derive"]` | Yes | Memory zeroization on drop for key material. |
| `base64` | `0.22` | None | Yes | Base64 representation. |
| `hex` | `0.4` | None | Yes | Hexadecimal representation. |
| `sqlx` | `0.8` | `["runtime-tokio", "sqlite"]` | Yes | SQLite client. Used for storing WireGuard sessions. |
| `tracing` | `0.1` | None | Yes | System logging. |
| `uuid` | `1` | `["v4", "serde"]` | Yes | UUID generation for active sessions. |
| `thiserror` | `1` | None | Yes | Error derive macro. |
| `anyhow` | `1` | None | Yes | Ad-hoc error propagation. |
| `chrono` | `0.4` | `["serde"]` | Yes | Time utilities. |

#### Crate Features Gating
The `op-gateway` crate defines **no custom features** within its `Cargo.toml`. There are no `cfg(feature = ...)` blocks gating internal implementation details.

#### Schema-as-Code Analysis
Although workspace dependencies like `prost`, `tonic-build`, and `prost-build` are defined in the parent workspace to support schema-as-code patterns, `op-gateway` **bypasses the schema-as-code discipline entirely**. 
* All public interfaces, D-Bus data payloads, and session tracking states (such as `RoutingDecision`, `McpClientInfo`, and `WireGuardSession`) are implemented as ad-hoc, unversioned Rust structs annotated with raw Serde macros.
* D-Bus serialization in `crates/op-gateway/src/mcp_gateway.rs` constructs unstructured JSON dynamically using the `json!` macro rather than generating API contracts from static schemas (e.g., Protocol Buffers, JSON Schema, or OSCAL definitions).

---

### Storage Backend Check

| Backend | Found at File:Line | Role | Architectural Evaluation |
|---|---|---|---|
| **SQLite (via `sqlx`)** | `crates/op-gateway/src/wireguard_auth.rs:37` | Relational Session Database | Used to store WireGuard session meta-information, peer configuration, and timestamps. |
| **Btrfs Subvolumes / Filesystem** | `crates/op-gateway/src/encrypted_storage.rs:188` | Key-Value Key Storage | Flat-file JSON structure representing encrypted WireGuard private keys and PSKs. |

#### Architectural Violations
1. **Bypassing Central Storage Standards:** The workspace defines a central database interface and a central cache mechanism (`op-state-store`, `op-cozo-store`, and `op-cache`). The `op-gateway` crate bypasses this entire infrastructure, initializing its own isolated SQLite engine (`sqlite:///var/lib/op-dbus/wireguard.db`) and managing isolated filesystem states directly.
2. **Ad-hoc SQLite Key Storage:** Using standard SQLite databases for secure gating parameters and raw flat files for key material introduces state synchronization overhead and ignores the high-performance Sled/CozoDB architecture validated elsewhere in the workspace.

---

### Security & Quality Audit Findings

#### CRITICAL: Stateful Nonce Reset on Restart (Nonce Reuse in ChaCha20-Poly1305)
* **Citation:** `crates/op-gateway/src/encrypted_storage.rs:320`, `350`, and `365-368`
* **Impact:** Direct cryptographic break of file encryption. Allows an attacker with access to stored `.key` files to decrypt sensitive WireGuard private keys and PSKs.
* **Analysis:**
  The `store_key` method implements ChaCha20-Poly1305 file encryption using a stateful `nonce_counter` attached to the `MasterKey` in memory:
  ```rust
  let mut nonce = [0u8; 12];
  let nonce_counter = master_key.nonce_counter;
  nonce[4..12].copy_from_slice(&nonce_counter.to_le_bytes());
  master_key.nonce_counter += 1;
  ```
  However, this counter is never persisted to disk. Upon gateway restart, the master key is reloaded from disk, and `nonce_counter` is hardcoded to reset back to `0`:
  ```rust
  // crates/op-gateway/src/encrypted_storage.rs:320
  self.master_key = Some(MasterKey {
      key,
      salt,
      nonce_counter: 0, // Reset to zero
  });
  ```
  Consequently, every system reboot resets the encryption sequence. If keys are updated or newly created after a reboot, the cipher nonces are reused (`0`, `1`, `2`, etc.) with the exact same master key. Nonce reuse in stream ciphers like ChaCha20 completely breaks confidentiality, allowing trivial mathematical derivation of the keystream and exposure of raw private keys.
* **Remediation:** Use a cryptographically secure random number generator (such as `ring::rand`) to generate a unique 12-byte initialization vector (nonce) for every single call to `store_key`, and store that nonce in the `EncryptedKeyEntry` metadata. Never rely on an unpersisted counter for stream cipher nonces.

---

#### CRITICAL: Plaintext Storage of System Cryptographic Master Key
* **Citation:** `crates/op-gateway/src/encrypted_storage.rs:337-347`
* **Impact:** Complete bypass of local encryption. Any user or local process with read access to `/var/lib/op-dbus/encrypted/wireguard-keys/` can extract the master key and decrypt all WireGuard credentials.
* **Analysis:**
  The `generate_master_key` function generates a cryptographically secure key and salt, but proceeds to write them directly to disk in plaintext:
  ```rust
  // Store encrypted key (in production, encrypt with user passphrase)
  let mut key_data = Vec::with_capacity(64);
  key_data.extend_from_slice(&key);
  key_data.extend_from_slice(&salt);

  async_fs::write(path, &key_data).await?;
  ```
  The code attempts to mitigate this by setting permissions to `0o600`, but storing the master key next to the encrypted payloads in raw plaintext completely invalidates the security boundary of local file encryption. If Btrfs native encryption or LUKS fallback fails, the keys are completely unprotected.
* **Remediation:** Integrate with a standard system keyring (such as standard Linux `keyring` or `systemd-creds`) or use a true Key Derivation Function (KDF) fed by an external passphrase input to derive the master key in-memory only.

---

#### HIGH: Unpadded and Unsafe `simd_json` Deserialization (Memory Safety Violation)
* **Citation:** `crates/op-gateway/src/encrypted_storage.rs:408` and `crates/op-gateway/src/wireguard_auth.rs:175`
* **Impact:** Potential undefined behavior, buffer overreads, or segmentation faults when parsing local metadata and D-Bus flags.
* **Analysis:**
  Both files utilize `unsafe` blocks to perform fast deserialization using `simd_json::from_str`:
  ```rust
  // crates/op-gateway/src/encrypted_storage.rs:408
  let entry_json = async_fs::read_to_string(&key_file_path).await?;
  let mut entry_str = entry_json.clone();
  let entry: EncryptedKeyEntry = unsafe { simd_json::from_str(&mut entry_str) }?;
  ```
  and:
  ```rust
  // crates/op-gateway/src/wireguard_auth.rs:175
  let flags_json: String = row.get("flags");
  let mut flags_str = flags_json.clone();
  let flags: std::collections::HashMap<String, String> =
      unsafe { simd_json::from_str(&mut flags_str) }.unwrap_or_default();
  ```
  The `simd-json` crate relies on vector instructions that process strings in 32-byte or 64-byte chunks. Because of this, it strictly mandates that any input string buffer passed to its parser *must* be allocated with `simd_json::SIMDJSON_PADDING` trailing bytes. Standard Rust `String` allocations do not guarantee this padding. Passing a standard `String` directly to `simd_json::from_str` via `unsafe` bypasses these safety checks and can lead to out-of-bounds vector reads if the JSON payload ends near a page boundary or lacks padding.
* **Remediation:** Use `simd_json::to_vec` and parse from a vector allocated with padding, or utilize safe parsing alternatives such as standard `serde_json::from_str` for metadata where microsecond performance is not a bottleneck.

---

#### HIGH: Silent Storage Security Degradation on Platform Feature Mismatch
* **Citation:** `crates/op-gateway/src/encrypted_storage.rs:149-155` and `227-230`
* **Impact:** System administrators are led to believe the system is operating on an encrypted Btrfs partition, when it has silently degraded to writing plaintext keys directly to the physical storage device.
* **Analysis:**
  If the platform lacks experimental Btrfs native encryption kernel support, the gateway catches the shell error and falls back to a normal unencrypted subvolume or directory:
  ```rust
  if stderr.contains("encryption not supported") || stderr.contains("invalid option") {
      warn!("Native Btrfs encryption not supported, creating regular subvolume");
      self.create_regular_subvolume().await?;
  }
  ```
  The function returns `Ok(())` despite failing to establish an encrypted boundary. In a system configured with `use_native_encryption: true`, the gateway silently degrades to plaintext key storage without halting or throwing a high-priority system error.
* **Remediation:** If the configured cryptographic boundary cannot be safely established, the initialization sequence must return a hard error (`GatewayError::Storage`) and abort execution immediately.

---

#### HIGH: Hardcoded Static Salts for Peer Credential Derivation
* **Citation:** `crates/op-gateway/src/wireguard_auth.rs:913` and `938`
* **Impact:** Decreased resistance against precomputation and offline brute-force attacks on stable WireGuard PSKs and rotated session keys.
* **Analysis:**
  When deriving stable WireGuard PSKs, the cryptographic engine utilizes a static hardcoded salt:
  ```rust
  // crates/op-gateway/src/wireguard_auth.rs:913
  let salt = b"WG-STABLE-PSK-2024";
  ```
  Similarly, session key derivation relies on:
  ```rust
  // crates/op-gateway/src/wireguard_auth.rs:938
  let salt = b"WG-SESSION-KEY-2024";
  ```
  Using identical, hardcoded salts across all systems and peers enables adversaries to build precomputed rainbow tables or dictionary attack maps specific to the `op-gateway` software, stripping away the entropy protection normally provided by unique salts.
* **Remediation:** Generate a random salt during system deployment (or per peer configuration), persist it securely alongside the local gateway identity, and supply this dynamic salt during Argon2 derivation.

---

#### MEDIUM: Potential Command Argument Injection on Btrfs/DF Execution
* **Citation:** `crates/op-gateway/src/encrypted_storage.rs:136-146` and `481-484`
* **Impact:** Host command manipulation or unintended filesystem traversal if configuration inputs are modified by an unauthorized user.
* **Analysis:**
  The gateway invokes system binaries using parameters constructed directly from raw configurations:
  ```rust
  let output = Command::new("btrfs")
      .args([
          "subvolume",
          "create",
          "-e",
          self.storage_path.to_str().unwrap(),
      ])
  ```
  And:
  ```rust
  let output = Command::new("df")
      .args(["-T", self.storage_path.to_str().unwrap()])
  ```
  While `Command::new` does not spawn a raw shell (preventing traditional `; malicious_cmd` shell injection), a carefully constructed directory path (e.g., beginning with leading dashes like `--help` or pointing to a symbolic link) can force target utilities to behave unexpectedly or fail.
* **Remediation:** Clean and sanitize `storage_path` inputs before execution. Verify that the configured path does not contain unexpected flags, control characters, or non-printable UTF-8 sequences. Ensure that `self.storage_path` is resolved to its canonical form prior to execution.

---
## ⚠ Citation Warnings
- `crates/op-gateway/src/wireguard_auth.rs:938`: file has 915 lines
