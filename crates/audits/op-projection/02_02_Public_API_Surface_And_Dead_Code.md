# Security and Quality Audit Report: `op-projection`

---

## 1. Schema-as-Code and OSCAL Compliance Checklist

The `op-projection` crate claims to follow a **Schema-as-Code** discipline, but the implementation relies heavily on ad-hoc Rust structs, manual runtime conversions, and hardcoded structures instead of single-source-of-truth Protocol Buffers or OSCAL compliance profiles.

### Violations Identified
*   **Duplicate and Ad-Hoc Data Contracts**: In `crates/op-projection/src/data_models.rs:14-478`, the entire data model suite (such as `PluginSchema`, `FieldSchema`, `Projection`, and `Requester`) is expressed as ad-hoc, manually written Rust structures annotated with `serde` deserialization attributes instead of compiled schema definitions (e.g., `.proto` or JSON schema files).
*   **Manual Schema Translation layers**: In `crates/op-projection/src/plugin_reader.rs:452-526`, manual conversion helpers (`convert_schema`, `convert_field`, etc.) are implemented to translate `op-state-store`'s `RuntimePluginSchema` into the local `PluginSchema`. This duplication introduces maintenance overhead and risks semantic desynchronization between runtime state and projected representations.
*   **Hardcoded Configuration Schemas**: In `crates/op-projection/src/bin/projection_server.rs:28-232`, schemas for memory, CPU, network, process, filesystems, and the identity sled are defined as hardcoded Rust structures instantiated at startup. This prevents centralized security policy updates and configuration versioning.
*   **Zero OSCAL Integration**: The codebase lacks OSCAL catalog, profile, or component definition mapping, making automated compliance validation against FedRAMP/NIST frameworks impossible.

### Remediation Strategy
1.  **Consolidate Data Contracts**: Define all projection contracts, event structures, and schemas in shared Protocol Buffers (`.proto`) files. Generate the Rust data models automatically using `prost` or `tonic`.
2.  **Unify the Schema Registry**: Reference a single `PluginSchema` type across `op-state-store`, `op-plugins`, and `op-projection` rather than implementing conversion layers.
3.  **OSCAL Mapping**: Author OSCAL Component Definitions for the Projection Engine detailing how controls like "Data in Transit Cryptography" and "Boundary Protection" are inherited and validated.

---

## 2. Vulnerability Audit and Threat Analysis

### Finding 1: Stub/No-op Redaction of Sensitive Data and PII
*   **Location**: `crates/op-projection/src/access_control.rs:97-103`
*   **Severity**: **Critical**
*   **Exploitability**: Directly exploitable. Projections that match security policies requiring redaction of passwords, private keys, or PII will leak the sensitive values in their entirety to unauthorized clients.
*   **Analysis**:
    ```rust
    fn redact_sensitive(
        &self,
        data: &simd_json::OwnedValue,
        _requester: &Requester,
    ) -> simd_json::OwnedValue {
        // In production, use JSON paths from schema to redact
        data.clone()
    }
    ```
    This method is used in the main access enforcement loop (`crates/op-projection/src/access_control.rs:44`). If a policy matches a projection containing PII or secrets (identified by `pii_paths` or `secret_paths` in `PluginSchema`), the engine calls `redact_sensitive` and updates the projection's data with the result. Because the implementation simply returns `data.clone()`, **no sensitive data is ever redacted**. 

### Finding 2: Denial of Service (DoS) via Hot-Loop Regular Expression Compilation
*   **Location**: `crates/op-projection/src/access_control.rs:42`, `64` and `crates/op-projection/src/schema_engine.rs:446`
*   **Severity**: **High**
*   **Exploitability**: Highly exploitable by sending updates to the engine or querying permissions under moderate client load, resulting in 100% CPU utilization.
*   **Analysis**:
    In `access_control.rs`:
    ```rust
    let re = Regex::new(&policy.resource_pattern)?;
    ```
    In `schema_engine.rs`:
    ```rust
    let regex = Regex::new(pattern).map_err(...)?;
    ```
    Regular expression compilation in the `regex` crate is a heavy computational task. The engine performs these compilations inside hot loops for *every single* permission check (`validate_permissions`), redaction enforcement (`enforce_policy`), and field constraint validation (`validate_constraints`). This is a severe CPU exhaustion vector.

### Finding 3: Fragile XML Parsing Vulnerable to Malicious D-Bus Payload Injection
*   **Location**: `crates/op-projection/src/dbus_reader.rs:51-68`
*   **Severity**: **High**
*   **Exploitability**: Exploitable by a compromised local D-Bus service returning crafted XML during introspection to inject false entities, overwrite sibling node representations, or trigger logical path traversal.
*   **Analysis**:
    ```rust
    // Very basic XML parsing for children
    // In production, use a proper XML parser
    let mut children = Vec::new();
    for line in xml.lines() {
        if line.contains("<node name=\"") {
            if let Some(name) = line
                .split("name=\"")
                .nth(1)
                .and_then(|s| s.split('\"').next())
            {
                if !name.is_empty() {
                    children.push(name.to_string());
                }
            }
        }
    }
    ```
    The manual parsing of XML using raw string splits is highly unsafe. If a node name contains special characters or directory traversal sequences (e.g. `../inject`), the string extraction splits it as a valid node name, resulting in a target path of `/../inject`. When projected, this violates D-Bus object hierarchy boundaries.

### Finding 4: Fragile Memory-Map Pointer Lifetime Contract
*   **Location**: `crates/op-projection/src/sled_reader.rs:73-83`
*   **Severity**: **Medium**
*   **Exploitability**: High risk of Use-After-Free (UAF) or segmentation faults during development or subsequent refactoring.
*   **Analysis**:
    ```rust
    fn read_sled_entity(&self) -> Result<RawEntity> {
        let (ptr, _mmap) =
            read_sled().map_err(|e| anyhow::anyhow!("Failed to read sled: {}", e))?;
        let sled = unsafe { &*ptr };
    ```
    The variable `_mmap` holds the underlying mapped memory segment. Because of the leading underscore, `_mmap` is kept alive until the end of the `read_sled_entity` block, allowing reads from `sled` to succeed safely *for now*. However, because the raw pointer `ptr` is untethered from Rust's borrow checker lifetime system, any future code optimization (such as converting to `let (ptr, _) = read_sled()`) will drop the memory mapping immediately, rendering `sled` a dangling pointer and causing immediate segmentation faults or undefined memory access on dereference.

---

## 3. Public API Surface & Dead Code Analysis

### Public API Surface Summary
*   **Total Public Items**: 118
*   **Glob Re-exports (`pub use *`)**: 2 occurrences (identified in `lib.rs`)

#### Top 10 Most Impactful Public Items
| Item | Type | file:line | Impact |
| --- | --- | --- | --- |
| `PluginSchema` | Struct | `crates/op-projection/src/data_models.rs:15` | Defines the entire structure and parsing strategy for plugin state validation. |
| `Projection` | Struct | `crates/op-projection/src/data_models.rs:127` | The primary payload encapsulating raw data, validation results, and lifecycle state. |
| `SchemaRegistry` | Trait | `crates/op-projection/src/interfaces.rs:14` | The contract defining register, fetch, and quarantine operations on schemas. |
| `ProjectionEngine` | Trait | `crates/op-projection/src/interfaces.rs:74` | The core mutation pipeline contract that converts RawEntities into Projections. |
| `EventMaterializer` | Trait | `crates/op-projection/src/interfaces.rs:114` | Responsible for consuming event-fed updates under performance constraints. |
| `AccessController` | Trait | `crates/op-projection/src/interfaces.rs:170` | Mandates permissions validation and selective fields redaction before egress. |
| `ProjectionSystemEngine` | Struct | `crates/op-projection/src/projection_engine.rs:20` | Authoritative database integration mapping in-memory state changes. |
| `SchemaEngine` | Struct | `crates/op-projection/src/schema_engine.rs:36` | Memory-backed realization of the SchemaRegistry trait. |
| `ProjectionStore` | Struct | `crates/op-projection/src/projection_store.rs:18` | In-memory concurrent store utilizing DashMap for zero-copy lookups. |
| `ProjectionStreamServer` | Struct | `crates/op-projection/src/json_stream.rs:31` | Implements Server-Sent Events (SSE) via Axum for UI distribution. |

#### Glob Re-exports
*   `crates/op-projection/src/lib.rs:34`: `pub use data_models::*;` — Pollution of the root namespace with over 30 auxiliary structs/enums.
*   `crates/op-projection/src/lib.rs:39`: `pub use interfaces::*;` — Exposes all trait declarations indiscriminately, complicating dependency tracking.

#### Structs with Exposed Public Fields (Should be Private)
The following structures in `crates/op-projection/src/data_models.rs` expose internal state directly:
*   `PluginSchema` (lines 14-31) — Fields like `fields` and `secret_paths` are fully public and mutable, allowing downstream code to alter rules post-registration.
*   `Projection` (lines 126-151) — Fields like `state` (ProjectionState) and `validation_errors` can be manipulated externally, bypassing the validation checks of `ProjectionSystemEngine`.

---

### Dead Code Analysis

The following table lists items that are compiled but never invoked or are intentionally suppressed via compiler hacks:

| Item | Type | file:line | Recommendation |
| --- | --- | --- | --- |
| `AuditTrail` | Trait | `crates/op-projection/src/interfaces.rs:198` | **Remove**: Unimplemented interface that has no concrete backing struct. |
| `HistoricalStore` | Trait | `crates/op-projection/src/interfaces.rs:216` | **Remove**: Defined but completely unused. Historical logic should be integrated into `ProjectionStore`. |
| `_dbus_reader` | Variable | `crates/op-projection/src/bin/projection_server.rs:309` | **Expose/Integrate**: Initialized but never read from or wired into the event loop. |
| `_grpc_reader` | Variable | `crates/op-projection/src/bin/projection_server.rs:310` | **Expose/Integrate**: Unused placeholder instantiation. |
| `OvsdbMirrorProjection` | Trait | `crates/op-projection/src/interfaces.rs:293` | **Test/Remove**: Interface defined for Mirror projections but not utilized by the server. |
| `OvsdbMirrorProjectionImpl` | Struct | `crates/op-projection/src/ovsdb_mirror.rs:13` | **Expose/Test**: Stub representation with no operational server references. |
| `read_nested_objects` | Function | `crates/op-projection/src/interfaces.rs:281` | **Remove**: Trait function that remains uncalled by the event processor. |