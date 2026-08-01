| Pattern | File:Line | Crate Approach | Corpus Best Practice | Gap | Rating |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `format_json_manual` | `crates/op-plugins/src/auto_create.rs:91` | Computes MD5 hash on dynamically serialized JSON strings for state validation. | Canonicalized schemas with secure, collision-resistant deterministic hashing (e.g., SHA-256). | Use of weak MD5 hashing on potentially non-deterministic dynamic JSON string serialization. | Major Gap |
| `format_json_manual` | `crates/op-plugins/src/auto_create.rs:92` | Dynamic in-memory JSON state representation serialized on the fly to calculate hashes. | Strict versioned schema-as-code (e.g., Protobuf/OSCAL) serialization to guarantee byte ordering. | Ad-hoc serialization is vulnerable to object key ordering differences causing false state mismatch. | Major Gap |
| `command_new` | `crates/op-plugins/src/dynamic_loading.rs:133` | Runs external CLI process `btrfs subvolume list` via shell command runner. | Direct programmatic APIs/library bindings or securely isolated wrapper executions. | Tight coupling to system-dependent binaries and fragile command output parsing. | Minor Gap |
| `command_new` | `crates/op-plugins/src/dynamic_loading.rs:141` | Spawns external CLI process `btrfs subvolume create` with direct path interpolation. | OS/filesystem level bindings or sandboxed subprocess managers. | Hardcoded external binary execution dependencies. | Minor Gap |
| `command_new` | `crates/op-plugins/src/dynamic_loading.rs:155` | Executing external CLI process `btrfs subvolume show` via `Command::new`. | Directly integrated file-system APIs. | Brittle parsing of raw external CLI command output. | Minor Gap |
| `format_json_manual` | `crates/op-plugins/src/dynamic_loading.rs:242` | Generates ad-hoc notification structures with manually formatted strings. | Schema-defined, structured application events. | Lacks a formalized event/error contract, causing unstructured logging. | Minor Gap |
| `format_json_manual` | `crates/op-plugins/src/dynamic_loading.rs:349` | Serializes dynamic state object to string via `.to_string()` for structural hashing. | Stable deterministic binary serialization (e.g. Protobuf byte representation). | `.to_string()` can yield non-deterministic results if internal struct ordering/formatting changes. | Major Gap |
| `format_json_manual` | `crates/op-plugins/src/plugin.rs:190` | Hashes dynamic version text string directly to confirm plugin authenticity. | Static cryptographic checksums verified against a secure registry. | Brittle comparison method prone to format modification errors. | Minor Gap |
| `std_fs_in_async` | `crates/op-plugins/src/registry.rs:84` | Calls asynchronous `tokio::fs::create_dir_all().await` inside async routine. | Utilize non-blocking async filesystem operations. | None (Fully compliant with async FS paradigm). | Compliant |
| `command_new` | `crates/op-plugins/src/service_def.rs:474` | Executes asynchronous shell-out to `systemctl` CLI binary. | Interface directly with DBus or specialized IPC interfaces (e.g., using `zbus`). | Shell-out execution of shell utilities introduces runtime dependencies. | Minor Gap |
| `command_new` | `crates/op-plugins/src/service_def.rs:519` | Invokes `systemctl` for daemon control using parameter execution. | Direct DBus interaction. | Vulnerable to system configuration changes and missing executable dependencies. | Minor Gap |
| `std_fs_in_async` | `crates/op-plugins/src/service_def.rs:367` | Writes a file to `/etc/dinit.d/` synchronously using `std::fs::write` inside async context. | Use non-blocking IO `tokio::fs::write` or wrap sync disk operations in `spawn_blocking`. | Blocking synchronous write operations executed directly inside async execution contexts. | Major Gap |
| `unwrap_expect` | `crates/op-plugins/src/default_registry.rs:222` | Uses `.unwrap()` to set up memory database store in testing module. | Return `Result` or use explicit assertion macros. | None (Test routines safely leverage panic-on-failure wrappers). | Compliant |
| `unwrap_expect` | `crates/op-plugins/src/default_registry.rs:232` | Uses `.unwrap()` to handle loader outcome within testing suite. | Leverage `?` propagation in tests or descriptive assertion messages. | None (Permitted in unit tests). | Compliant |
| `unwrap_expect` | `crates/op-plugins/src/default_registry.rs:238` | Uses `.unwrap()` for unit testing execution logic. | Safe test error execution. | None (Permitted in unit tests). | Compliant |
| `unwrap_expect` | `crates/op-plugins/src/default_registry.rs:241` | Validates plugin default registry schema output with `.unwrap()`. | Assert-on-error patterns. | None (Permitted in unit tests). | Compliant |
| `unwrap_expect` | `crates/op-plugins/src/default_registry.rs:257` | Uses `.unwrap()` for schema publication tests. | Strict test assertions. | None (Permitted in unit tests). | Compliant |
| `unsafe_block` | `crates/op-plugins/src/state_plugins/config.rs:42` | Parses `ConfigStoreState` JSON using `unsafe simd_json::from_str(&mut content)`. | Explicit safety documentation with proven invariants, backed by versioned schema-as-code contracts. | Undocumented unsafe block parsing ad-hoc JSON without structural schema contract. | Major Gap |
| `simd_json_from_str` | `crates/op-plugins/src/state_plugins/config.rs:42` | Uses in-place parsing string mutation using `simd_json` unsafe functions. | Use safe parsing interfaces or formal schemas ensuring data format constraints. | Directly modifying buffer inputs via unsafe mutations without safety proof. | Major Gap |
| `std_fs_in_async` | `crates/op-plugins/src/state_plugins/config.rs:39` | Reads data via asynchronous file system wrapper `tokio::fs::read_to_string`. | Non-blocking async reads. | None (Fully compliant with async FS paradigm). | Compliant |
| `std_fs_in_async` | `crates/op-plugins/src/state_plugins/config.rs:53` | Generates paths using `tokio::fs::create_dir_all`. | Non-blocking directory management. | None (Fully compliant with async FS paradigm). | Compliant |
| `std_fs_in_async` | `crates/op-plugins/src/state_plugins/config.rs:59` | Serializes configuration schema changes directly via `tokio::fs::write`. | Non-blocking async disk write. | None (Fully compliant with async FS paradigm). | Compliant |
| `unsafe_block` | `crates/op-plugins/src/state_plugins/mcp.rs:164` | Mutates slice content inside `unsafe simd_json::from_str(&mut c_mut)`. | Unsafe operations must be documented with a `// SAFETY:` explaining invariants. | Missing safety documentation and formal contract definitions. | Major Gap |
| `simd_json_from_str` | `crates/op-plugins/src/state_plugins/mcp.rs:164` | In-place dynamic parsing on raw mutable input variables. | Type-safe structured schema verification. | Ad-hoc mutation structure relies on raw memory layouts with no explicit boundaries. | Major Gap |
| `unsafe_block` | `crates/op-plugins/src/state_plugins/ovsdb_bridge.rs:159` | Performs in-place parsing of OVSDB dynamic bridge details via unsafe block. | Explicitly validate buffer ownership and document safety reasons. | Utilizes unsafe interfaces for standard formatting, though documented with basic comment. | Minor Gap |
| `simd_json_from_str` | `crates/op-plugins/src/state_plugins/ovsdb_bridge.rs:159` | Unsafely mutates character buffers for local parsing optimizations. | Safe fallback parsers or guaranteed memory-safe structures. | Safety relies entirely on the library's assumptions of valid UTF-8 and correct buffer lengths. | Minor Gap |
| `unsafe_block` | `crates/op-plugins/src/state_plugins/privacy_routes.rs:57` | Parses network routing layout files using `unsafe simd_json::from_str(&mut content)`. | Maintain invariant verification and explicitly detail safety guarantees. | Undocumented unsafe execution parsing unvalidated ad-hoc schemas. | Major Gap |
| `simd_json_from_str` | `crates/op-plugins/src/state_plugins/privacy_routes.rs:57` | Modifies raw routing content variables directly inside parsing routines. | Define concrete schema-as-code models (e.g., Protobuf structs) to guarantee safety bounds. | Undocumented unsafe mutations on dynamic external inputs. | Major Gap |
| `unsafe_block` | `crates/op-plugins/src/state_plugins/net.rs:259` | Deserializes raw bridge payload strings using unsafe in-place operations. | Isolate unsafe structures and utilize strict interface contracts. | Ad-hoc network layout representations parsed inside unsafe context without proof of safety. | Major Gap |
| `simd_json_from_str` | `crates/op-plugins/src/state_plugins/net.rs:261` | Mutates underlying JSON string buffers in-place during parsing. | Safe deserializers combined with formalized protocols. | Risk of undefined behavior (UB) on malformed network configurations without explicit constraints. | Major Gap |

---

### Actionable Recommendations for Major/Critical Gaps

#### 1. Eliminate Blocking Synchronous IO inside Async Runtimes
*   **Gap Location:** `crates/op-plugins/src/service_def.rs:367`
*   **Recommendation:**
    Replace the direct synchronous `std::fs::write` call with its asynchronous equivalent from the `tokio` crate. If direct synchronous integration is required, spawn the task using `tokio::task::spawn_blocking` to prevent CPU starvation on the async executor thread pool.
    ```rust
    // Recommended non-blocking write
    let path = format!("/etc/dinit.d/{}", self.name);
    tokio::fs::write(&path, self.to_dinit()).await?;
    ```

#### 2. Implement Deterministic State Hashing and Secure Cryptographic Functions
*   **Gap Locations:** `crates/op-plugins/src/auto_create.rs:91`, `crates/op-plugins/src/auto_create.rs:92`, and `crates/op-plugins/src/dynamic_loading.rs:349`
*   **Recommendation:**
    *   **Replace MD5:** Deprecate the use of MD5 in favor of a cryptographically secure hash function like SHA-256 (via the `sha2` crate).
    *   **Avoid `.to_string()` for Hashing:** Do not rely on ad-hoc string formatting or serializing un-ordered dynamic objects (e.g., JSON maps parsed using standard methods) to produce state comparison hashes.
    *   **Adopt Deterministic Schemas:** Compile states into versioned Protocol Buffer bytes or canonicalized JSON schemas. Hash the serialized output of those structured contracts to ensure exact matching:
    ```rust
    use sha2::{Sha256, Digest};
    // Ensure `desired` is defined as a structured, versioned schema object
    let mut hasher = Sha256::new();
    hasher.update(desired.to_protobuf_bytes()?); // Deterministic representation
    let hash_result = format!("{:x}", hasher.finalize());
    ```

#### 3. Strict Safety Verification and Migration to Schema-as-Code Contracts
*   **Gap Locations:**
    *   `crates/op-plugins/src/state_plugins/config.rs:42`
    *   `crates/op-plugins/src/state_plugins/mcp.rs:164`
    *   `crates/op-plugins/src/state_plugins/privacy_routes.rs:57`
    *   `crates/op-plugins/src/state_plugins/net.rs:259-261`
*   **Recommendation:**
    *   **Annotate Unsafe blocks:** Each `unsafe` block must be accompanied by an explicit `// SAFETY:` comment proving that all preconditions of the underlying library (e.g. `simd_json`'s allocation padding and UTF-8 requirements) are strictly met.
    *   **Replace Unsafe with Safe Alternatives:** Unless parsing performance is a verified production bottleneck, prefer safe deserialization via `serde_json` or `simd_json`'s safe execution branches.
    *   **Adopt Schema-as-Code:** Migrate unstructured objects (like `HashMap<String, Value>` and unstructured config files) to versioned schemas (such as Protocol Buffers or OSCAL-compliant structs) to establish a strictly defined contract for external inputs, avoiding undefined behavior on malformed payloads.