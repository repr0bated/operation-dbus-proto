### 1. Critical Cryptographic Failure: Reused Nonces in ChaCha20Poly1305 Across Restarts
* **Suggestion**: Use a cryptographically secure random 96-bit nonce for each encryption operation instead of a stateful sequential counter that resets on service restarts.
* **Rationale**: The current implementation initializes the `nonce_counter` to `0` whenever `load_master_key` is called (which occurs on service startup). Because `store_key` uses this counter to generate the ChaCha20Poly1305 nonce and increments it sequentially, restarting the gateway guarantees that the identical sequence of nonces (starting at `0`) is reused to encrypt different keys. Nonce reuse under the same key in a stream-cipher-based AEAD like ChaCha20Poly1305 is catastrophic; it destroys confidentiality by allowing an attacker who captures the ciphertexts to XOR them and recover the plaintext keys.
* **Example**: `crates/op-gateway/src/encrypted_storage.rs:292`

---

### 2. Plaintext Master Key Storage on the Filesystem
* **Suggestion**: Derive the master key dynamically at runtime using a robust Key Derivation Function (KDF) from a user-provided passphrase, or integrate with a hardware-backed security module (e.g., TPM 2.0 or an external KMS) instead of persisting it in plaintext.
* **Rationale**: In `generate_master_key`, the generated key material and salt are written to disk completely in plaintext. This defeats the entire threat model of encrypting individual keys; any local compromise of the base path `/var/lib/op-dbus/encrypted` allows an adversary to read `master.key`, obtain the key/salt, and decrypt all stored WireGuard private keys and session secrets.
* **Example**: `crates/op-gateway/src/encrypted_storage.rs:315`

---

### 3. Severe Denial of Service (DoS) Risk via Argon2 KDF on Session Creation Path
* **Suggestion**: Replace Argon2 with HKDF-SHA256 (`hkdf` crate) for deriving stable PSKs and session keys from base key material.
* **Rationale**: Argon2 is an intentionally slow, memory-hard algorithm designed for password hashing. Using `Argon2::default()` inside `derive_stable_psk` and `derive_session_keys` creates a critical performance bottleneck on the active session establishment and key rotation path. An unauthenticated remote peer can easily trigger CPU and memory exhaustion (causing a Denial of Service) by repeatedly hitting the gateway with new/random public keys, forcing the server to spin up multiple Argon2 hashes concurrently.
* **Example**: `crates/op-gateway/src/wireguard_auth.rs:608`

---

### 4. Violation of Schema-as-Code: Ad-Hoc JSON Payloads over D-Bus
* **Suggestion**: Define all gateway payloads and contracts (e.g., routing decisions, session validations, client capabilities) using versioned Protocol Buffers schemas, compile them with `prost`, and enforce structured serialization rather than untyped, ad-hoc JSON.
* **Rationale**: Methods like `dbus_route_client` construct raw, dynamically typed JSON structures using the `json!` macro and return them as raw `Value`s. This circumvents the project's schema-as-code discipline, creating brittle, unversioned contracts that are difficult to update, type-check, and maintain across the microservices boundary.
* **Example**: `crates/op-gateway/src/mcp_gateway.rs:317`

---

### 5. Brittle and Insecure Failbacks to Unencrypted Storage
* **Suggestion**: Enforce strict error boundaries and fail loudly if native Btrfs encryption cannot be enabled, or require explicit, non-default configuration parameters to allow unencrypted fallbacks.
* **Rationale**: When native Btrfs encryption fails (e.g., due to lack of kernel support), `setup_native_btrfs_encryption` logs a warning and silently falls back to creating an unencrypted subvolume or regular directory. Similarly, the LUKS fallback falls back to an unencrypted subvolume. In production control planes, silently violating security constraints by storing private cryptographic keys in the clear is extremely dangerous.
* **Example**: `crates/op-gateway/src/encrypted_storage.rs:163`

---

### 6. Performance Overhead: Ad-Hoc Heap Allocation of String Cache Keys
* **Suggestion**: Avoid allocating a formatted heap `String` on every client routing request. Use a numeric key format (e.g., `u64` raw hash) or construct a stack-allocated/borrowed composite key for cache lookups.
* **Rationale**: `generate_cache_key` calls `format!("mcp_route_{:x}", hasher.finish())` which forces a heap allocation for every single routing decision. Under high client traffic, these transient string allocations degrade throughput and increase garbage collection overhead on the allocator.
* **Example**: `crates/op-gateway/src/mcp_gateway.rs:290`

---

### 7. Storage Bottleneck: Ad-Hoc Directory Storage (One File Per Key)
* **Suggestion**: Transition from writing individual files to the filesystem to using an embedded database designed for transactional key-value access (such as the workspace's CozoDB or Sled) wrapped with an AEAD layer.
* **Rationale**: Every stored key creates a new `{key_id}.key` file on raw disk. This design does not scale; high session volume causes severe inode exhaustion, directory indexing overhead, and high latencies during directory traversals or batch cleanups.
* **Example**: `crates/op-gateway/src/encrypted_storage.rs:351`

---

### 8. Brittle CLI Parsing and Potential Panics on System Output
* **Suggestion**: Implement structured parsing with strict boundary checking or regex instead of unvalidated whitespace split-indexing.
* **Rationale**: `get_filesystem_info` parses the output of `df` by splitting lines on whitespace and hard-indexing into the slice (`fields[1]`, `fields[2]`). If the system's `df` output format slightly differs (e.g., localized systems, or different system coreutils), the indices can be out of bounds, triggering a runtime panic in a background daemon task.
* **Example**: `crates/op-gateway/src/encrypted_storage.rs:467`

---

### 9. Lack of Structured Logging Fields and Tracing Spans
* **Suggestion**: Annotate all public cryptographic storage and session management functions with structured `#[tracing::instrument]` attributes to capture calling contexts, sessions, and error flows without exposing raw key data.
* **Rationale**: Critical flows like `store_key`, `retrieve_key`, and `create_session` lack structured spans, relying instead on flat text log messages. In production environments, identifying which user session triggered a cryptographic failure or filesystem write is nearly impossible without correlation IDs attached to structured tracing spans.
* **Example**: `crates/op-gateway/src/encrypted_storage.rs:324`