### Data Structures & State Audit

#### 1. Concurrency & Reference-Counting Primitive Counts

| File | `Arc` | `Rc` | `RefCell` | `RwLock` | `Mutex` | `OnceCell` |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: |
| `crates/op-identity/src/anna_scribe.rs` | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-identity/src/gcloud_auth.rs` | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-identity/src/registration.rs` | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-identity/src/session.rs` | 4 | 0 | 0 | 0 | 1 | 0 |
| `crates/op-identity/src/token.rs` | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-identity/src/wg.rs` | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-identity/src/wireguard.rs` | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-identity/src/lib.rs` | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-identity/src/schema_bridge.rs` | 0 | 0 | 0 | 0 | 0 | 0 |

#### 2. Clone Call Counts (Flagged if > 20)
*   **`crates/op-identity/src/session.rs`**: 11 `.clone()` / `.cloned()` calls.
*   All other files contain $\le$ 1 `.clone()` call. No files exceed the threshold of 20.

#### 3. Large Structs (> 5 Public Fields)
*   **`crates/op-identity/src/session.rs:21`**: Struct `Session` has 7 public fields:
    ```rust
    pub struct Session {
        pub session_id: String,
        pub pubkey: String,
        pub user_email: Option<String>,
        pub oauth_token: Option<String>,
        pub token_expires_at: Option<DateTime<Utc>>,
        pub created_at: DateTime<Utc>,
        pub last_seen_at: DateTime<Utc>,
    }
    ```
*   **`crates/op-identity/src/schema_bridge.rs:50`**: Struct `SubidTaxonomy` has 6 public fields:
    ```rust
    pub struct SubidTaxonomy {
        pub category: SubidCategory,
        pub component_type: String,
        pub subject: String,
        pub verb: String,
        pub facet: Option<String>,
        pub version: u8,
    }
    ```
*   **`crates/op-identity/src/schema_bridge.rs:136`**: Struct `IdentitySled` has 18 public fields:
    ```rust
    pub struct IdentitySled {
        pub wireguard_pubkey: [u8; 32],
        pub mutation_index: u64,
        pub is_valid: bool,
        pub _pad: [u8; 7],
        pub hashed_footprint: [u8; 32],
        pub schema_uuid: [u8; 16],
        pub subid: [u8; 64],
        pub subid_category: [u8; 8],
        pub subid_component_type: [u8; 32],
        pub subid_subject: [u8; 64],
        pub subid_verb: [u8; 32],
        pub subid_facet: [u8; 32],
        pub subid_version: u8,
        pub _pad2: [u8; 7],
        pub control_source: [u8; 32],
        pub control_refs: [u8; 128],
        pub statement_refs: [u8; 128],
        pub nextdns_profile: [u8; 16],
    }
    ```

#### 4. Globally Mutable State
*   **`crates/op-identity/src/schema_bridge.rs:333`**: Global atomic counter:
    ```rust
    static MUTATION_INDEX: AtomicU64 = AtomicU64::new(0);
    ```

---

### Schema-as-Code Compliance Audit

The system uses a mixed design of shared C-representations and ad-hoc mappings:
*   **Violations (Ad-hoc Data Contracts)**:
    *   `SessionLedger` in `crates/op-identity/src/anna_scribe.rs:27` is declared as an ad-hoc Rust struct and uses pure strings rather than versioned schema definitions.
    *   `Session` and `UserMapping` in `crates/op-identity/src/session.rs:21` and `crates/op-identity/src/session.rs:33` are ad-hoc domain structures.
    *   `CachedToken` in `crates/op-identity/src/token.rs:11` is defined as an ad-hoc contract serialized via JSON.
*   **Partial Compliance**:
    *   `IdentitySled` in `crates/op-identity/src/schema_bridge.rs:136` and `PluginSchema` in `crates/op-identity/src/anna_scribe.rs:18` use a fixed byte layout mapping directly to system shared memory, capturing compliance attributes (e.g., OSCAL UUIDs and NIST control references). However, these are managed as raw Rust structs rather than generated from schema engines (e.g., `prost` / Protocol Buffers).

---

### Security & Quality Findings

#### CRITICAL: Memory Safety & UB in Shared Memory Zero-Copy Reads
*   **Location**: `crates/op-identity/src/anna_scribe.rs:44-51`
*   **Description**: In `notarize_arrival`, a memory map is created over `/dev/shm/plugin_schema.dat` and cast directly to `PluginSchema` via raw pointers:
    ```rust
    let mmap = unsafe {
        MmapOptions::new()
            .map(&file)
            .map_err(|_| "Memory map failed".to_string())?
    };
    let schema_ptr = mmap.as_ptr() as *const PluginSchema;
    let is_valid = unsafe { (*schema_ptr).is_valid };
    ```
    There is no validation of the mapped file's size before casting and dereferencing the pointer. If the file is smaller than `std::mem::size_of::<PluginSchema>()` (e.g., due to truncation by another process, premature crash, or malicious local payload), dereferencing `schema_ptr` leads to an Out-Of-Bounds memory read and undefined behavior.
*   **Aliasing Soundness Violation**: In `crates/op-identity/src/schema_bridge.rs:533`, `run_schema_shuttle` converts a raw pointer from the mmap directly to a shared reference `&IdentitySled`:
    ```rust
    let (ptr, _mmap) = read_sled()?;
    let sled = unsafe { &*(ptr) };
    ```
    Because `/dev/shm/plugin_schema.dat` is located in tmpfs and can be mutated concurrently by the writing process (`SchemaEngine`), creating a shared reference `&IdentitySled` violates Rust's aliasing rules (which guarantee that a reference `&T` points to immutable data for its lifetime). This is a critical soundness bug that can result in undefined compiler optimizations and unstable behavior.

#### CRITICAL: Command Hijacking via Path Manipulation
*   **Location**:
    *   `crates/op-identity/src/gcloud_auth.rs:282`: `Command::new("gcloud")`
    *   `crates/op-identity/src/token.rs:68`: `Command::new("gcloud")`
    *   `crates/op-identity/src/wg.rs:20`: `Command::new("wg")`
    *   `crates/op-identity/src/wireguard.rs:31`: `Command::new("wg")`
    *   `crates/op-identity/src/schema_bridge.rs:512`: `Command::new("incus")`
    *   `crates/op-identity/src/schema_bridge.rs:567`: `Command::new("xray")`
*   **Description**: Command execution is performed using relative binary names without absolute paths. The system relies entirely on the executing process's `PATH` environment variable to resolve the binaries.
*   **Exploitability**: Because the WireGuard (`wg`), Linux Container (`incus`), and proxy (`xray`) management routines are highly privileged operations likely running as `root` or within highly-privileged system groups, a local unprivileged attacker who can manipulate the environment variables of these processes can point the `PATH` to a directory under their control. When `Command::new` executes, it will invoke the attacker's malicious binary under the context of the privileged identity-state service, resulting in privilege escalation.

#### HIGH: Unsafe SIMD-JSON Deserialization of Keyring Entries
*   **Location**: `crates/op-identity/src/token.rs:81-83`
*   **Description**:
    ```rust
    async fn read_from_keyring(&self) -> Result<CachedToken> {
        let entry = keyring::Entry::new("mcp-identity", "gcloud-token")?;
        let mut json = entry.get_password()?;
        Ok(unsafe { simd_json::from_str(&mut json) }?)
    }
    ```
    The `simd_json::from_str` function has strict allocation, padding, and alignment requirements. Calling the `unsafe` variant of `from_str` directly on a mutable string fetched from the OS keyring is extremely dangerous. If an attacker can inject a payload or if the keyring data gets corrupted/malformed, this unsafe call will cause memory corruption, segmentation faults, or buffer overflows.

#### HIGH: Stack Memory Leak to Shared Filesystem (Information Disclosure)
*   **Location**: `crates/op-identity/src/schema_bridge.rs:175-181`
*   **Description**:
    ```rust
    let bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(sled as *const IdentitySled as *const u8, IdentitySled::SIZE)
    };
    let mut f = File::create(&tmp)?;
    f.write_all(bytes)?;
    ```
    The struct `IdentitySled` contains alignment padding fields (`_pad: [u8; 7]` and `_pad2: [u8; 7]`). If the compiler leaves uninitialized data inside padding bytes during struct instantiation on the stack, casting the entire struct to a byte slice and writing it directly to a shared memory file in `/dev/shm` makes those uninitialized stack bytes visible to any local unprivileged process. This can leak stack variables, cryptographic keys, or sensitive heap addresses.

#### MEDIUM: Cryptographic Continuity with Broken Hash Function (MD5)
*   **Location**: `crates/op-identity/src/anna_scribe.rs:73-75`
*   **Description**: `AnnaScribe::notarize_arrival` uses MD5 to calculate the footprint for the session identity:
    ```rust
    let payload = format!("{}:{}", wg_pubkey, current_mutation);
    let genesis_hash = format!("{:x}", md5::compute(payload.as_bytes()));
    ```
    MD5 is cryptographically broken and vulnerable to collision attacks. While the footprint is described as an "accountability loop", using MD5 undermines security guarantees if peer identities are meant to be cryptographically validated. Use SHA-256 or SHA-3 for all footprint generations.

---
## ⚠ Citation Warnings
- `crates/op-identity/src/gcloud_auth.rs:282`: file has 244 lines
