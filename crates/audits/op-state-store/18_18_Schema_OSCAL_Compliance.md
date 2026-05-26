# OP-STATE-STORE SECURITY & QUALITY AUDIT

## 1. Schema-as-Code Table

The codebase defines multiple critical data models, ledger transactions, and boundary-crossing payloads using ad-hoc Rust structs and untyped JSON objects (`simd_json::OwnedValue`), violating the schema-as-code discipline.

| Item | Type | file:line | Has .proto? | Gap |
| :--- | :--- | :--- | :--- | :--- |
| `SystemDependency` | Struct | `disaster_recovery.rs:19` | No | Defined as an ad-hoc Rust struct with no Protocol Buffer schema. |
| `PluginStateExport` | Struct | `disaster_recovery.rs:33` | No | Uses untyped `simd_json::OwnedValue` (`Value`) for the `state` field. |
| `DisasterRecoveryExport` | Struct | `disaster_recovery.rs:48` | No | Serialized directly to JSON in an ad-hoc manner; no proto contract. |
| `HostInfo` | Struct | `disaster_recovery.rs:69` | No | Ad-hoc system information struct without a schema. |
| `RestoreResult` | Struct | `disaster_recovery.rs:78` | No | Ad-hoc system feedback structure. |
| `ChainEvent` | Struct | `event_chain.rs:136` | No | Ledger record representing audit logs, lacks a versioned proto definition. |
| `ActionOrigin` | Enum | `event_chain.rs:90` | No | Action provenance structure modeled dynamically in Rust. |
| `DenyReason` | Enum | `event_chain.rs:115` | No | Dynamic error types with no schema-as-code representation. |
| `EventBatch` | Struct | `event_chain.rs:321` | No | Batch verification metadata with no schema. |
| `MerkleProof` | Struct | `event_chain.rs:356` | No | Merkle verification proof model defined purely in Rust. |
| `StateSnapshot` | Struct | `event_chain.rs:388` | No | State storage record using untyped JSON (`simd_json::OwnedValue`). |
| `ExecutionJob` | Struct | `execution_job.rs:19` | No | Core job ledger record; uses untyped JSON for `arguments`. |
| `ExecutionResult` | Struct | `execution_job.rs:11` | No | Contains untyped JSON `output` field. |
| `StoredObject` | Struct | `lib.rs:41` | No | Storage model using untyped JSON `data`. |
| `CanonicalDbExport` | Struct | `lib.rs:50` | No | Ad-hoc structural arrays of untyped JSON values. |
| `ToolRecord` | Struct | `state_store.rs:7` | No | Persistent tool record using serialized JSON strings rather than versioned proto fields. |
| `PluginSchema` | Struct | `plugin_schema.rs:88` | No | Hand-rolled schema catalog implementation instead of standard protobuf metadata. |
| `FieldSchema` | Struct | `plugin_schema.rs:41` | No | Part of the hand-rolled schema catalog. |
| `IdentitySled` | Struct | `schema_shuttle.rs:9` | No | Shared-memory layout defined as a raw C-repr struct. |

---

## 2. OSCAL Coverage Table

The following table flags system security controls implemented in the control plane that lack traceability to machine-readable OSCAL policy or documentation artifacts.

| Control Area | Implemented at file:line | OSCAL Artifact | Gap |
| :--- | :--- | :--- | :--- |
| **Audit Record Retention / Generation** (NIST 800-53 AU-2, AU-3, AU-12) | `sqlite_store.rs:260`<br>`event_chain.rs:136` | None | Audit log collection, format, and storage are hardcoded in SQLite database schemas and Rust models with no machine-readable `component-definition` or `system-security-plan` (SSP) mapping. |
| **Protection of Audit Information** (NIST 800-53 AU-9, AU-10) | `event_chain.rs:1`<br>`event_chain.rs:321` | None | Cryptographic event chaining and Merkle root verification provide log integrity verification, but the verification controls are missing OSCAL representation. |
| **Software Installation & System Change Control** (NIST 800-53 CM-5, SI-7) | `disaster_recovery.rs:275` | None | Privileged software dependency installation via D-Bus PackageKit lacks trace mapping to authorized installer profiles in OSCAL. |
| **Information Flow Enforcement & Access Control** (NIST 800-53 AC-3, AC-4) | `event_chain.rs:90`<br>`event_chain.rs:115` | None | Provenance checks (human-instructed vs autonomous actions) are hardcoded in the codebase and cannot be inspected/updated via machine-readable OSCAL policies. |
| **Identification and Authentication** (NIST 800-53 IA family) | `schema_shuttle.rs:9` | None | Base64 WireGuard key extraction and `IdentitySled` validation lack trace mapping to identity assurance profiles in OSCAL. |

---

## 3. Detailed Findings & Recommendations

### [CRITICAL] Memory Safety Violation: Out-of-Bounds Memory Corruption via Unpadded `simd_json::from_str`

#### Vulnerability Analysis
The codebase makes extensive use of the `simd_json` crate to perform JSON parsing. By design, `simd_json` uses highly optimized SIMD instructions (such as AVX2, NEON, SSE4.2) that read and write memory in 32-byte vectors. To maintain memory safety, `simd_json` requires that any input string buffer must be padded with at least `simd_json::PADDING` (typically 32 bytes) of scratch space beyond the logical length of the string. 

Calling `simd_json::from_str` on unpadded, standard Rust `String` buffers causes SIMD memory operations to overshoot the buffer bounds, leading to undefined behavior (UB), heap metadata corruption, or instant segmentation faults (Denial of Service). 

This unsafe, unpadded invocation occurs in multiple critical paths processing external/untrusted data:
- **Disaster Recovery Imports**: `disaster_recovery.rs:118`
  ```rust
  pub fn from_json(json: &str) -> Result<Self> {
      let mut json_mut = json.to_string();
      Ok(unsafe { simd_json::from_str(&mut json_mut) }?)
  }
  ```
- **Real-Time Redis Stream Consumers**: `redis_stream.rs:219`, `252`, `273`
  ```rust
  let mut json_mut = json;
  Ok(Some(unsafe { simd_json::from_str(&mut json_mut)? }))
  ```
- **SQLite Database Object Storage**: `sqlite_store.rs:212`, `242`, `272`, `322`, `452`, `500`, `592`
  ```rust
  let mut state_json: String = row.get("state_json");
  let state: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut state_json)? };
  ```

#### Remediation
Ensure that all parsed string buffers are explicitly padded using `simd_json::to_padded_string` before invoking the parser, or switch to the safe, standard `serde_json::from_str` function for any inputs that cannot be strictly verified as padded.

*Example safe implementation with `simd_json`:*
```rust
pub fn from_json(json: &str) -> Result<Self> {
    let mut padded_bytes = simd_json::to_padded_string(json);
    Ok(unsafe { simd_json::from_slice(&mut padded_bytes) }?)
}
```

---

### [MAJOR] Cryptographic Vulnerability: Tamper-Evident Ledger Integrity Compromised by MD5 Usage

#### Vulnerability Analysis
The "tamper-evident" compliance ledger and Event Chain verification engine rely entirely on MD5 hashing to generate event signatures, compute Merkle batch roots, and prove the immutability of system states:
- `event_chain.rs:430` uses `md5::compute` to hash canonical state transitions.
- `event_chain.rs:436` uses `md5` to build concatenated sibling hashes in the Merkle Tree.
- `disaster_recovery.rs:101`, `161` uses MD5 to generate checksums and verify plugin integrity.
- `sqlite_store.rs:188` uses MD5 for state snapshots.

MD5 is a cryptographically broken hash function vulnerable to collision attacks (including chosen-prefix collisions). Since this ledger acts as a NIST 800-53 compliance layer to assure auditors of action origins, system state transitions, and authorization (human vs. autonomous), an attacker who can generate MD5 collisions can forge unauthorized system transitions and modify origin parameters (e.g. rewriting an `Autonomous` action to appear as `Instructed`) without breaking the chain's cryptographic checksum or Merkle proofs.

#### Remediation
Replace all occurrences of MD5 with a secure, cryptographically robust hash function such as SHA-256 (via the `sha2` crate, which is already present in the workspace dependencies).
- Update `compute_hash` in `event_chain.rs:430` to compute SHA-256 signatures.
- Update the Merkle node computation in `event_chain.rs:441` to perform SHA-256 pair concatenation.

---

### [MAJOR] Design Gap: Ad-hoc Data Serialization & Lack of Protocol Buffer Schemas

#### Vulnerability Analysis
The `op-state-store` crate serves as the system state and job ledger for `op-dbus`. However, there are zero `.proto` schema definitions present. All data exchanges (such as disaster recovery exports, tool execution logs, and audit logs) are represented as ad-hoc Rust structs with untyped JSON fields (`simd_json::OwnedValue`). 

This violates the workspace-wide schema-as-code discipline and increases the risk of schema drift. Field changes inside the Rust struct (e.g., adding/modifying fields in `SystemDependency` or `ChainEvent`) will silently break backward compatibility with stored SQLite databases, Redis events, or disaster recovery exports, leading to parse failures and system lockouts.

#### Remediation
1. Define all ledger transactions, disaster recovery schemas, and state objects in Protocol Buffer (`.proto`) files.
2. Use `prost` and `tonic-build` to compile these schemas into Rust data models.
3. Replace untyped JSON structures (`simd_json::OwnedValue`) with strongly typed protobuf messages.
4. For fields that must hold arbitrary structured payloads, use standard `protobuf.Any` types rather than raw JSON strings.

---

### [MEDIUM] Quality Gap: Unnecessary Shell Process Spawning in Schema Shuttle

#### Vulnerability Analysis
In `schema_shuttle.rs:95-101`, the `run_shuttle` background loop reloads the Xray daemon by spawning an interactive shell:
```rust
Command::new("sh")
    .arg("-c")
    .arg(format!(
        "export X_GHOSTBRIDGE_FOOTPRINT='{}' && export X_GHOSTBRIDGE_TRACE_ID='{}' && systemctl reload xray", 
        new_footprint_hex, trace_id
    ))
    .spawn()?;
```
Spawning an intermediate shell process (`sh`) is highly inefficient and creates an unnecessary command execution surface. While the variables formatted into the command string (`new_footprint_hex`, `trace_id`) are hex-encoded strings, any changes to upstream code that permit arbitrary characters in these identifiers could lead to shell command injection.

#### Remediation
Avoid invoking shell wrappers. Execute the `systemctl` command directly and pass the required parameters in the environment block of the `Command` builder:
```rust
Command::new("systemctl")
    .arg("reload")
    .arg("xray")
    .env("X_GHOSTBRIDGE_FOOTPRINT", new_footprint_hex)
    .env("X_GHOSTBRIDGE_TRACE_ID", trace_id)
    .spawn()?;
```