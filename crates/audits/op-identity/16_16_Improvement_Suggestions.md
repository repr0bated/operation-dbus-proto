1. **Structured Xray Configuration Schema** | Replace raw string formatting for generated configurations with strongly typed structs serialized via `serde_json`. Generating complex JSON strings via ad-hoc formatting is highly fragile, susceptible to escaping/injection errors, and violates the project's schema-as-code discipline. | `crates/op-identity/src/schema_bridge.rs:250`

2. **Unsafe Mmap Alignment and Bounds Validation** | Guard memory-mapped pointers with size and alignment checks before casting. Direct dereferencing of raw pointers mapped from file interfaces assumes the file size matches exactly. If the file is truncated, corrupt, or has incorrect padding, this dereference will trigger undefined behavior or segmentation faults. | `crates/op-identity/src/anna_scribe.rs:60`

3. **Asynchronous Command Execution for Cloud Auth** | Use `tokio::process::Command` instead of synchronous `std::process::Command` in `GCloudAuth` helpers. Invoking blocking process spawns inside the async context of `get_token` blocks Tokio execution threads and halts the runtime reactor loop. | `crates/op-identity/src/gcloud_auth.rs:247`

4. **Schema-Driven Taxonomy Validation** | Model operational categories and the subid taxonomy parsing layout using formal Protocol Buffers or standardized OSCAL schemas, rather than manually parsing delimited strings. Bespoke segment splitting is error-prone and bypasses versioned schema enforcement. | `crates/op-identity/src/schema_bridge.rs:77`

5. **Netlink or State Caching for Peer Lookups** | Query WireGuard handshakes and allowed-IP ranges using netlink sockets or maintain a local in-memory cache, rather than continuously spawning `wg` child processes. Forking/executing external binaries on every lookup degrades performance and limits overall system throughput. | `crates/op-identity/src/wg.rs:19`

6. **Consolidation of Duplicate WireGuard Implementations** | Merge duplicate WireGuard logic and parsing rules between `wg.rs` and `wireguard.rs` into a unified module. The codebase currently splits helper implementations with redundant parsing and hardcoded interface identifiers. | `crates/op-identity/src/wg.rs:15`

7. **Cryptographic Standardisation for Session Ledgers** | Replace the insecure MD5 hash function used in session arrival notarization with a secure standard like SHA-256. MD5 is cryptographically weak, and standardizing on SHA-256 (which is already implemented for NVMe footprints) eliminates algorithmic divergence. | `crates/op-identity/src/anna_scribe.rs:75`

---
## ⚠ Citation Warnings
- `crates/op-identity/src/gcloud_auth.rs:247`: file has 244 lines
