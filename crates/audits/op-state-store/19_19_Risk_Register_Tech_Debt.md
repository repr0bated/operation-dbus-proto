# Security and Quality Audit: Risk Register

| Severity | Issue | Evidence (file:line) | Recommendation |
| :--- | :--- | :--- | :--- |
| **Critical** | Unauthenticated Host-Level Package Installation via Unsigned DR Imports | `crates/op-state-store/src/disaster_recovery.rs:480`<br>`crates/op-state-store/src/disaster_recovery.rs:556` | Implement asymmetric cryptographic signatures (e.g., Ed25519) on DR exports. Reject any DR import during restore if the signature is missing or fails verification. |
| **High** | Cryptographic Integrity Failure Due to MD5 in Compliance Event Chain & DR State | `crates/op-state-store/src/event_chain.rs:649`<br>`crates/op-state-store/src/disaster_recovery.rs:115`<br>`crates/op-state-store/src/disaster_recovery.rs:214` | Replace all occurrences of MD5 with a secure, collision-resistant hash function like SHA-256 or BLAKE3. |
| **High** | Command Injection Risk and Reload Race Conditions via Asynchronous Shell Spawn | `crates/op-state-store/src/schema_shuttle.rs:108` | Avoid invoking system commands via `sh -c`. Use direct execution of `/usr/bin/systemctl` with parameterized arguments and block/synchronize reloads to prevent race conditions. |
| **High** | Gaps in Schema-as-Code Discipline and OSCAL Compliance | `crates/op-state-store/src/plugin_schema.rs:94`<br>`crates/op-state-store/src/disaster_recovery.rs:39`<br>`crates/op-state-store/src/lib.rs:48` | Refactor untyped `simd_json::OwnedValue` states to strongly-typed versioned Protobuf contracts. Translate custom schema structures into OSCAL Component Definition models to meet FedRAMP standards. |
| **Medium** | Isolated Stack-Only Allocation of theoretically Shared "Identity Sled" | `crates/op-state-store/src/schema_shuttle.rs:80` | Map `IdentitySled` to a persistent shared memory region (e.g., using `memmap2` or a POSIX shared memory file descriptor) to ensure it can be accessed by the Xray process. |
| **Low** | Insecure Status Parsing Fallback Leading to Unintended Job Double-Execution | `crates/op-state-store/src/sqlite_store.rs:623` | Return an explicit `Result::Err` or transition to a designated `Unknown/Corrupted` status rather than defaulting to `Pending` when encountering malformed status strings. |

---

### Detailed Findings & Technical Context

#### 1. Unauthenticated Host-Level Package Installation via Unsigned DR Imports
The disaster recovery restore process imports configuration files containing host dependencies and immediately triggers installation via D-Bus PackageKit (which executes with system-level root privileges). 

Because the `DisasterRecoveryExport` file is unsigned, its `checksum` field (calculated via an MD5 hash loop) is easily forgeable. An attacker who is able to supply a modified DR JSON file can alter the package names in `global_dependencies` or `plugin.dependencies` to trigger the root-level download, installation, or upgrading of arbitrary packages, or manipulate dependency versions.

#### 2. Weak Cryptographic Hash (MD5) in Event Chain and Compliance Layers
The compliance engine utilizes MD5 to calculate "tamper-evident" transition hashes, database state hashes, and Merkle tree roots. 

MD5 is highly vulnerable to cryptographic collision attacks. Attackers with database access can alter the transition logs or insert malicious transitions (e.g., executing a command or applying an unauthorized state) and easily generate matching MD5 footprints. This undermines the security guarantees of the event chain compliance layers.

#### 3. Command Injection Risk and Reload Race Conditions in Xray Integration
In `schema_shuttle.rs:108`, a system reload is triggered by passing formatted string properties (`new_footprint_hex` and `trace_id`) into `/bin/sh -c`. 

If upstream input filtering on those variables fails or is bypassed (e.g., if `new_footprint_hex` contains shell metacharacters), this pattern introduces a shell injection vulnerability. Additionally, calling `.spawn()` launches the shell asynchronously without checking for concurrency locks, allowing rapid state changes to spark overlapping reloads of `xray`, causing performance degradation or race conditions.

#### 4. Gaps in Schema-as-Code and OSCAL Compliance
Data contracts such as `StoredObject` and `PluginStateExport` utilize `simd_json::OwnedValue` (ad-hoc untyped JSON objects) instead of strongly-typed versioned schemas. 

Furthermore, `PluginSchema` and `FieldSchema` rely on a custom in-memory validation engine written in Rust rather than an industry-standard OSCAL (Open Security Controls Assessment Language) format, posing compatibility and compliance gaps for FedRAMP environments requiring versioned machine-readable system security plans.

#### 5. Local Stack Isolation of "Identity Sled"
The `IdentitySled` struct is declared with `#[repr(C)]` for zero-copy memory layout. However, in `schema_shuttle.rs:80`, `session_sled` is allocated entirely as a local variable on the execution stack of `run_shuttle()`. 

Since no file descriptor, shared memory mapping (`mmap`), or IPC mechanism is initialized to share this memory segment, other system processes (like Xray) cannot access it. The zero-copy memory segment exists only theoretically in comments, making the feature ineffective.