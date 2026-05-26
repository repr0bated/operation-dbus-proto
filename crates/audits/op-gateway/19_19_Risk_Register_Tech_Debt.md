| Severity | Issue | Evidence | Recommendation |
| :--- | :--- | :--- | :--- |
| **Critical** | Cryptographic Key Disclosure via Stateful Nonce Reuse in ChaCha20-Poly1305 | `crates/op-gateway/src/encrypted_storage.rs:307`<br>`crates/op-gateway/src/encrypted_storage.rs:368` | Persist the nonce counter securely to disk, or generate random 96-bit nonces using a cryptographically secure random number generator (CBRNG). |
| **Critical** | Predictable Session ID / Auth Token Generation | `crates/op-gateway/src/wireguard_auth.rs:442` | Utilize a secure, high-entropy random value (e.g., UUID v4 or 256-bit random token) for session tokens rather than hashing public/predictable metadata. |
| **Critical** | Zero-Security WireGuard PSK Derived from Public Metadata | `crates/op-gateway/src/wireguard_auth.rs:545` | Generate true cryptographically secure random preshared keys and distribute them out-of-band; do not derive them from public keys. |
| **High** | Master Key Plaintext Exposure in Unencrypted Fallback Directory | `crates/op-gateway/src/encrypted_storage.rs:156`<br>`crates/op-gateway/src/encrypted_storage.rs:318` | Never store the master key in plaintext on disk. If fallback to unencrypted storage occurs, require manual passphrase entry or integrate with a kernel keyring (e.g., `keyring` crate). |
| **High** | Unbounded Memory Leak in Routing Cache (Denial of Service) | `crates/op-gateway/src/mcp_gateway.rs:128` | Implement cache lookup logic in `route_client` and enforce a size-bounded eviction policy (e.g., LRU cache) or time-to-live (TTL) limits. |
| **High** | Schema-as-Code Violation: Ad-hoc SQLite and JSON SerDe contracts | `crates/op-gateway/src/wireguard_auth.rs:51`<br>`crates/op-gateway/src/wireguard_auth.rs:158` | Refactor persistence schemas to use versioned Protocol Buffers and generate SQLite data bindings from compiled schemas. |
| **Medium** | Potential Undefined Behavior via Unsafe `simd_json::from_str` | `crates/op-gateway/src/encrypted_storage.rs:394` | Replace the unsafe `simd_json::from_str` with its safe counterpart or use standard safe JSON parsing libraries to prevent memory corruption. |

---

### Detailed Findings & Technical Remediation

#### 1. State Desynchronization and Nonce Reuse in ChaCha20-Poly1305 (Critical)
* **Impact**: Total loss of confidentiality and integrity of stored WireGuard keys.
* **Mechanism**: 
  In `crates/op-gateway/src/encrypted_storage.rs`, the `MasterKey` struct initializes its stateful `nonce_counter` to `0` when loaded from disk:
  ```rust
  // Line 307
  self.master_key = Some(MasterKey {
      key,
      salt,
      nonce_counter: 0,
  });
  ```
  During the `store_key` flow (line 368), the stateful counter is used to construct the nonce:
  ```rust
  let nonce_counter = master_key.nonce_counter;
  nonce[4..12].copy_from_slice(&nonce_counter.to_le_bytes());
  master_key.nonce_counter += 1;
  ```
  Every time the gateway daemon restarts, the `nonce_counter` resets to `0`. Consequently, writing new keys after a restart reuses previous nonces under the identical master key. In ChaCha20-Poly1305, encrypting two different messages with the same key and nonce allows an attacker to XOR the ciphertexts together to recover the plaintexts and forge valid authentication tags.
* **Remediation**:
  Switch to generating standard 96-bit random nonces for every encryption operation:
  ```rust
  let mut nonce = [0u8; 12];
  SystemRandom::new().fill(&mut nonce)
      .map_err(|_| anyhow::anyhow!("Entropy failure"))?;
  ```

---

#### 2. Predictable Session ID / Auth Token Generation (Critical)
* **Impact**: Authentication bypass. Any network attacker can hijack active authenticated sessions.
* **Mechanism**: 
  The `session_id` acts as the primary authentication token when routing client requests through the MCP Gateway. However, `generate_session_id` constructs the token deterministically using public values:
  ```rust
  // Line 442
  let input = format!("WG-SESSION-{}-{}", peer_pubkey, Self::current_timestamp());
  let session_ids = self
      .crypto_engine
      .generate_session_ids_batch(&[input.as_bytes()]);
  ```
  Because `peer_pubkey` is public and `current_timestamp()` is evaluated in seconds, there are only 3,600 possible session ID combinations per hour for any given peer. An attacker can precalculate these IDs and query the D-Bus or gRPC interface to hijack active client sessions.
* **Remediation**:
  Inject cryptographically secure random entropy into the session token:
  ```rust
  let mut token = [0u8; 32];
  SystemRandom::new().fill(&mut token)
      .map_err(|_| anyhow::anyhow!("Entropy failure"))?;
  let session_id = hex::encode(token);
  ```

---

#### 3. Zero-Security WireGuard PSK Derived from Public Metadata (Critical)
* **Impact**: Total compromise of WireGuard transport security. Any passive eavesdropper can decrypt VPN payloads.
* **Mechanism**: 
  The preshared key (PSK) used for WireGuard connections is derived using the client's public key as the sole high-entropy input:
  ```rust
  // Line 545
  let salt = b"WG-STABLE-PSK-2024";
  let mut input = Vec::with_capacity(39);
  input.extend_from_slice(b"WG-PSK-");
  input.extend_from_slice(peer_key);
  ```
  Because the peer public key is public (by design and transmitted in cleartext during handshakes), the "preshared key" is not actually a secret. It can be derived by any entity observing the connection.
* **Remediation**:
  Use standard cryptographic random values for PSKs and store them within a secure database table accessible only by the gateway process.

---

#### 4. Master Key Plaintext Exposure in Unencrypted Fallback Directory (High)
* **Impact**: Local privilege escalation. Any local process with read access to the directory can read the master key.
* **Mechanism**: 
  In `encrypted_storage.rs:156`, if native experimental Btrfs encryption is not supported, the code falls back silently to an unencrypted subvolume or standard directory:
  ```rust
  warn!("Native Btrfs encryption not supported, creating regular subvolume");
  self.create_regular_subvolume().await?;
  ```
  It then writes the raw master key directly to `master.key` in plaintext (line 318):
  ```rust
  async_fs::write(path, &key_data).await?;
  ```
  This creates a false sense of security; the master key is stored in plaintext on unencrypted storage, defeating the purpose of utilizing Btrfs native encryption.
* **Remediation**:
  Do not write raw master key files to disk. Utilize the Linux kernel keyring API (via `keyring` crate) or prompt for an operator-provided passphrase at startup.

---

#### 5. Unbounded Memory Leak in Routing Cache (High)
* **Impact**: Denial of Service (DoS) via Out-Of-Memory (OOM) crash.
* **Mechanism**: 
  `McpGatewayManager` defines `routing_cache` to store routing decisions. During `route_client`, decisions are inserted into the cache:
  ```rust
  // Line 128
  let mut cache = self.routing_cache.write().await;
  let cache_key = self.generate_cache_key(&client_info);
  cache.insert(cache_key, routing_decision.clone());
  ```
  However, `self.routing_cache` is **never queried or evicted**. Every client routing request results in a permanent entry inside the hashmap, leaking memory continuously.
* **Remediation**:
  Integrate an LRU or TTL-based cache structure such as `lru::LruCache` to limit maximum capacity.

---

#### 6. Schema-as-Code Violation: Ad-hoc SQLite and JSON Blobs (High)
* **Impact**: Schema fragility, incompatibility with compliance standards (e.g. FedRAMP/OSCAL), and lack of data validation controls.
* **Mechanism**: 
  In `wireguard_auth.rs:51`, the database schema is defined as an ad-hoc, manual SQL string. Additionally, the `flags` column (line 158) is read and parsed as an ad-hoc, nested JSON structure. This bypasses structural type validation and versioning controls.
* **Remediation**:
  Define data contracts using Protocol Buffers (Proto3) files. Compile these schemas to generate safe Rust types, and leverage `prost` to serialize structured, versioned blobs for SQLite storage.

---

#### 7. Potential Undefined Behavior via Unsafe `simd_json::from_str` (Medium)
* **Impact**: Memory safety violations or segmentation faults on corrupt disk storage.
* **Mechanism**: 
  The codebase uses the unsafe API of `simd_json` on read file contents:
  ```rust
  // Line 394
  let entry: EncryptedKeyEntry = unsafe { simd_json::from_str(&mut entry_str) }?;
  ```
  The unsafe `from_str` function expects strict structural padding and alignment guarantees. If a local file is truncated or corrupted, this call will trigger undefined memory access.
* **Remediation**:
  Replace `unsafe simd_json::from_str` with the safe `simd_json::from_slice` or standard `serde_json::from_str`.