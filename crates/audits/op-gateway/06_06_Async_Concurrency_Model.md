# Production Quality & Security Audit: Crate `op-gateway`

## Async & Concurrency Analysis

*   **`async fn` count**: 53
*   **`tokio::spawn` count**: 2
*   **`spawn_blocking` count**: 0

---

## Findings

### [CRITICAL] ChaCha20-Poly1305 Nonce Reuse Catastrophe on Service Restart
*   **File/Line**: `crates/op-gateway/src/encrypted_storage.rs:271-299`, `crates/op-gateway/src/encrypted_storage.rs:317-320`, `crates/op-gateway/src/encrypted_storage.rs:77-87`
*   **Description**: Every time `load_master_key` is called (during service startup or initialization), the `MasterKey`'s `nonce_counter` is reset to `0`:
    ```rust
    self.master_key = Some(MasterKey {
        key,
        salt,
        nonce_counter: 0,
    });
    ```
    However, the `master_key.key` is persistently loaded from `master.key` (which is written to disk in plaintext, which is itself a physical security hazard). 
    
    When `store_key` is called:
    ```rust
    let mut nonce = [0u8; 12];
    let nonce_counter = master_key.nonce_counter;
    nonce[4..12].copy_from_slice(&nonce_counter.to_le_bytes());
    master_key.nonce_counter += 1;
    ```
    The stream cipher `ChaCha20` is initialized with the static key, and a nonce derived from this zero-initialized `nonce_counter`. This means that on **every single reboot**, the gateway will reuse the exact same nonces (e.g., `0`, `1`, `2`) with the exact same master key to encrypt new files on disk. 
    
    In stream ciphers like ChaCha20, reusing a nonce with the same key to encrypt different plaintexts is a catastrophic cryptographic failure. An attacker who has read access to the directory (especially when falling back to regular unencrypted Btrfs directories, as described below) can XOR two ciphertexts encrypted with the same nonce, eliminating the key stream entirely and trivially recovering both plaintexts (including WireGuard private keys).
*   **Remediation**: Use a secure random 96-bit nonce generated via a cryptographically secure random number generator (e.g., `ring::rand::SecureRandom`) for every encrypted key entry, and store the nonce alongside the ciphertext. Never use a deterministic stateful counter that resets upon process restarts.

---

### [CRITICAL] Publicly-Derivable Cryptographic Secrets (Stable PSK & Session Key)
*   **File/Line**: `crates/op-gateway/src/wireguard_auth.rs:619-637`, `crates/op-gateway/src/wireguard_auth.rs:639-674`
*   **Description**: The gateway derives "stable preshared keys" (PSKs) and "session keys" using a purely public derivation process:
    1.  `derive_stable_psk` hashes a static string, a static hardcoded salt `b"WG-STABLE-PSK-2024"`, and the client's **public** key (`peer_key`) using Argon2.
    2.  `derive_session_keys` hashes a static string, a static hardcoded salt `b"WG-SESSION-KEY-2024"`, the client's public key, and a server nonce (which must be communicated to the client).
    
    Because there are **no server private keys, master secrets, or client private keys** mixed into the Argon2 key derivation function, the derived keys are entirely a function of publicly observable data (client public keys and public salts). 
    
    Any passive eavesdropper on the network or any local user who knows the client's WireGuard public key can trivially calculate both the client's stable PSK and session keys. This renders the WireGuard PSK entirely useless for its intended security properties (e.g., post-quantum resistance, mutual authentication).
*   **Remediation**: Integrate a cryptographically secure, server-side private master key (e.g., a high-entropy secret loaded from a secure HSM or environment variable) into the input key material of the Argon2 derivation function.

---

### [HIGH] Blocking Subprocesses and Synchronous IO inside Async Executor
*   **File/Line**: `crates/op-gateway/src/encrypted_storage.rs:135`, `crates/op-gateway/src/encrypted_storage.rs:188`, `crates/op-gateway/src/encrypted_storage.rs:219`, `crates/op-gateway/src/encrypted_storage.rs:238`, `crates/op-gateway/src/encrypted_storage.rs:372`
*   **Description**: The gateway uses `std::process::Command` synchronous subprocess invocation (`Command::new(...).output()`) directly inside several `async fn`s:
    *   `setup_native_btrfs_encryption`: Invokes `btrfs subvolume create` synchronously.
    *   `setup_luks_encryption`: Invokes `dd if=/dev/zero ... bs=1M count=100` synchronously (blocks the thread while writing a 100MB file).
    *   `create_regular_subvolume`: Invokes `btrfs subvolume create` synchronously.
    *   `mount_luks_device`: Invokes `mount` synchronously.
    *   `get_filesystem_info`: Invokes `df -T` synchronously.
    
    Calling blocking synchronous processes inside async tasks stalls the tokio threadpool executor. Under load, this causes massive scheduling latency, stalls the network reactor, drops connections, and triggers health check timeouts. Furthermore, the code contains numerous synchronous filesystem checks (`self.storage_path.exists()`, `Path::new(...).exists()`) inside async contexts.
*   **Remediation**: Use `tokio::process::Command` instead of `std::process::Command` for async process execution, and replace blocking `std::fs` operations with `tokio::fs` or wrap them inside `tokio::task::spawn_blocking`.

---

### [HIGH] Extreme Write-Lock Deadlock and Starvation Hazard holding locks across database `.await` calls
*   **File/Line**: `crates/op-gateway/src/wireguard_auth.rs:434-449`, `crates/op-gateway/src/wireguard_auth.rs:581-591`
*   **Description**: The control plane locks `self.sessions` and `self.peer_sessions` with asynchronous write locks (`write().await`), and then performs database transaction operations using `sqlx` *while holding these write locks*:
    *   In `rotate_session_key` (line 434), `sessions` write lock is acquired, and inside the lock, `self.database.update_wireguard_session(session).await` is executed (line 447).
    *   In `start_background_tasks` (line 581), both `sessions` and `peer_sessions` write locks are acquired and held inside a loop while awaiting `database.remove_wireguard_session(&session_id).await` for *every* expired session.
    
    This is an anti-pattern. SQLite disk synchronization and write lock contentions can take tens to hundreds of milliseconds. Holding memory-cache write locks across SQLite `.await` writes blocks all incoming validation requests and handshake routing attempts globally, causing major starvation and latency spikes.
*   **Remediation**: Perform the database writes *before* or *after* acquiring the write locks on the in-memory cache, or release the locks prior to initiating disk IO. Update the database first, then quickly update the cache inside a short, synchronous memory-only write lock window.

---

### [MEDIUM] Unsafe in-place deserialization of untrusted payloads via `simd_json`
*   **File/Line**: `crates/op-gateway/src/encrypted_storage.rs:360`, `crates/op-gateway/src/wireguard_auth.rs:242`
*   **Description**: The gateway reads serializations from the filesystem and database, clones them, and uses `unsafe { simd_json::from_str(&mut entry_str) }` to parse the payload in-place.
    Using `unsafe` blocks directly on data loaded from external files or the database bypassed memory-safety analysis. If the file on disk is corrupted, partially written due to an abrupt shutdown, or modified by a malicious local user, mutating the string buffer inside `unsafe simd_json` parser implementations can cause memory unsafety, segmentation faults, or buffer overflows.
*   **Remediation**: Use the safe parsing API provided by `simd_json::from_slice` or standard `serde_json::from_str` for data loaded from external, potentially unstable sources.

---

### [MEDIUM] Ad-hoc Data Contracts Expressed as Rust Structs instead of Versioned Schemas
*   **File/Line**: 
    *   `crates/op-gateway/src/encrypted_storage.rs:20` (`EncryptedStorageConfig`)
    *   `crates/op-gateway/src/encrypted_storage.rs:50` (`EncryptedKeyEntry`)
    *   `crates/op-gateway/src/mcp_gateway.rs:16` (`RoutingDecision`)
    *   `crates/op-gateway/src/mcp_gateway.rs:40` (`McpClientInfo`)
    *   `crates/op-gateway/src/mcp_gateway.rs:51` (`McpSession`)
    *   `crates/op-gateway/src/wireguard_auth.rs:172` (`WireGuardSession`)
    *   `crates/op-gateway/src/wireguard_auth.rs:189` (`WireGuardStats`)
*   **Description**: Contrary to the strict schema-as-code discipline using Protocol Buffers and OSCAL compliance profiles, the data contracts representing configuration, key entries, routing decisions, client info, and system statistics are expressed as ad-hoc Rust structs with generic Serde derive attributes. This lacks versioning, backward-compatibility validation, and platform-agnostic definition files.
*   **Remediation**: Migrate these ad-hoc structures into versioned Protocol Buffer (`.proto`) schemas. Integrate automatic code generation into the build pipeline using `prost-build` and use versioned schemas for storage and D-Bus IPC message structures.

---

### [LOW] Deterministic Session ID Collision and SQL Primary Key Violations
*   **File/Line**: `crates/op-gateway/src/wireguard_auth.rs:411`, `crates/op-gateway/src/wireguard_auth.rs:604-610`
*   **Description**: `generate_session_id` constructs the session ID entirely based on the peer's public key and the current Unix epoch timestamp in seconds:
    ```rust
    let input = format!("WG-SESSION-{}-{}", peer_pubkey, Self::current_timestamp());
    ```
    If a client attempts to re-authenticate or establish concurrent sessions within the same second, the generated session ID will be identical. When writing this to the SQLite database (which has a `session_id TEXT PRIMARY KEY` constraint), this deterministic collision will trigger an `INSERT OR REPLACE` or database lock contention, causing session hijacking or sudden termination of the previous active session.
*   **Remediation**: Include high-entropy random bytes (generated via `ring::rand`) inside the session ID input string before hashing to ensure cryptographic uniqueness.