# Production Security & Quality Audit: op-projection Control Plane

## 1. Schema-as-Code Audit

This codebase implements a metadata-driven projection system but relies entirely on ad-hoc, manually serialized Rust structs to exchange payloads across event materialization boundaries, real-time UI streams, and system interfaces. Untyped JSON is utilized as a primary data carrier.

| Item | Type | file:line | Has .proto? | Gap / Violation | Severity |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `Value` | Type Alias | `crates/op-projection/src/data_models.rs:10` | No | Uses `simd_json::OwnedValue` as a catch-all type for projection data, configuration schemas, and metadata. Bypasses schema-as-code validation. | Major |
| `Projection` | Struct | `crates/op-projection/src/data_models.rs:125` | No | Authoritative base representation of state transformations is modeled strictly as an ad-hoc Rust structure with serialized JSON fields. | Major |
| `ProjectionEvent` | Struct | `crates/op-projection/src/data_models.rs:335` | No | Event-driven payloads ingested for projection materialization are typed in native Rust; no corresponding cross-language versioned schema exists. | Major |
| `ProjectionUpdate` | Struct | `crates/op-projection/src/data_models.rs:356` | No | Data frame broadcasted over WebSockets/SSE to external user interfaces. Bypasses versioned schemas. | Major |
| `Requester` | Struct | `crates/op-projection/src/data_models.rs:380` | No | Principal metadata and permissions structure utilized for access control evaluations is an ad-hoc struct. | Major |
| `AccessPolicy` | Struct | `crates/op-projection/src/data_models.rs:393` | No | Access control policies are defined as Rust structs and parsed via manual regex without a versioned declarative schema. | Major |

---

## 2. OSCAL Coverage Audit

The security control enforcement (authentication context, authorization checks, state mutation logging, data sanitization) within this control plane lacks machine-readable representation in OSCAL artifacts. 

| Control Area | Implemented at file:line | OSCAL Artifact | Gap | Severity |
| :--- | :--- | :--- | :--- | :--- |
| **Authentication & Principal Identification (IA-2, IA-8)** | `crates/op-projection/src/data_models.rs:380` | Component Definition | `Requester` identities and trust anchors are processed inside the projection engine but are not documented or mapped to an OSCAL component definition. | Moderate |
| **Access Control & Authorization (AC-2, AC-3, AC-6)** | `crates/op-projection/src/access_control.rs:35` | Component Definition | Dynamic access enforcement is hardcoded and executed without reference to machine-readable authorization schemas or OSCAL control files. | Major |
| **Dynamic Policy Enforcement Bypass (AC-3, AC-3(3))** | `crates/op-projection/src/bin/projection_server.rs:360` | System Security Plan (SSP) / Component Definition | A hardcoded fallback policy (`allow-all-read`) is inserted directly into the runtime access controller. Bypasses OSCAL compliance definitions. | Critical |
| **Audit Trail and Logging (AU-2, AU-3, AU-12)** | `crates/op-projection/src/access_control.rs:114` | Assessment Results / SSP | Access decisions are logged to an in-memory `audit_trail` and standard console logs, but lack formal telemetry bindings or OSCAL assurance audits. | Moderate |
| **PII & Secret Protection (SC-28, MP-6)** | `crates/op-projection/src/access_control.rs:107` | Component Definition | Redaction rules defined in `PluginSchema` are evaluated but utilize a dummy implementation that does not perform redaction. | Critical |
| **System Interface Mapping (CA-3, SC-7)** | `crates/op-projection/src/dbus_reader.rs:15` | Component Definition | System-level D-Bus interfaces scanned and projected by `SystemDbusReader` are undocumented in any machine-readable OSCAL artifact. | Moderate |
| **Service Endpoint Identification (CA-3, SC-7)** | `crates/op-projection/src/grpc_reader.rs:14` | Component Definition | gRPC endpoints scanned, mapped, and monitored by the system lack corresponding system boundary descriptions in OSCAL. | Moderate |

---

## 3. Vulnerability & Quality Findings

### CRITICAL: Dummy Sensitive Data Redaction Leads to PII and Secrets Exposure
*   **Citations**: `crates/op-projection/src/access_control.rs:107-112`, `crates/op-projection/src/access_control.rs:41-43`
*   **Vulnerability Impact**: The `ProjectionAccessController` evaluates access control policies. If a policy has `redact_sensitive` set to `true`, it invokes the `redact_sensitive` method. However, this method is a dummy stub:
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
    This directly exposes highly sensitive PII, passwords, private keys, or system secrets (configured in `pii_paths` and `secret_paths` in `PluginSchema`) to unauthorized read clients, completely defeating the purpose of the security controller. This is directly exploitable given the active validation code.

### CRITICAL: Hardcoded Global Policy Bypasses Access Control Verification
*   **Citations**: `crates/op-projection/src/bin/projection_server.rs:360-367`
*   **Vulnerability Impact**: During server bootstrap, a global allow-all read policy is appended directly to the active access controller:
    ```rust
    access_controller.add_policy(AccessPolicy {
        id: "allow-all-read".to_string(),
        resource_pattern: ".*".to_string(),
        required_permissions: vec![],
        action: "read".to_string(),
        redact_sensitive: false,
    });
    ```
    Because `validate_permissions` loops over all policies and permits access if *any* policy allows the action, this hardcoded entry causes `validate_permissions` to always succeed for any requester trying to read any resource. Combined with the dummy redaction code, this allows unauthenticated, unprivileged actors to access all raw system state without restriction.

### MAJOR: Dynamic Regular Expression Compilation inside Security Loops
*   **Citations**: `crates/op-projection/src/access_control.rs:38`, `crates/op-projection/src/access_control.rs:56`, `crates/op-projection/src/access_control.rs:69`
*   **Vulnerability Impact**: Within `enforce_policy` and `validate_permissions`, the system compiles regexes on the fly using `Regex::new(&policy.resource_pattern)`. This compilation is executed inside critical execution paths on every query. 
    1.  **Denial of Service (CPU Exhaustion)**: An attacker who can register or submit a customized access policy can supply a complex, catastrophic regular expression, inducing catastrophic backtracking and triggering CPU exhaustion (ReDoS).
    2.  **Runtime Failures**: If an invalid regex pattern is stored, compiling it triggers a runtime error (`anyhow::Error`), which is propagated up, potentially causing API endpoint failures and localized DOS.

### MAJOR: Synchronous `block_on` Execution Over Tokio Runtime Threads
*   **Citations**: `crates/op-projection/src/plugin_reader.rs:374-388`, `crates/op-projection/src/plugin_reader.rs:395-397`
*   **Vulnerability Impact**: The `SystemPluginReader` is a synchronous structure that must resolve asynchronous plugin database queries. It uses a helper method `block_on` which attempts to grab the current runtime and call `block_in_place`:
    ```rust
    fn block_on<F, T>(&self, future: F) -> Result<T>
    where
        F: Future<Output = Result<T>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
            Err(_) => { ... }
        }
    }
    ```
    This synchronous blocking pattern can easily cause thread starvation, deadlock, or thread-pool exhaustion when executed inside active multi-threaded asynchronous contexts (such as the Axum HTTP routing server). If the Tokio current-thread runtime is active, calling `block_on` on a handle where a runtime is already running will panic immediately.

---

## 4. Remediation Recommendations

### Schema-as-Code Implementation
1.  **Declare Protobuf Definitions**: Define `Projection`, `ProjectionEvent`, `ProjectionUpdate`, and `Requester` payloads in versioned Protocol Buffers (`.proto`) files.
2.  **Codegen Integration**: Use `prost-build` or `tonic-build` in a `build.rs` script to generate high-performance Rust structs directly from the `.proto` schemas.
3.  **Strict Type Conversions**: Replace unstructured `simd_json::OwnedValue` inside internal structures with strongly-typed Protobuf structures or Well-Known Types (e.g., `google.protobuf.Struct`).

### OSCAL Compliance Implementation
1.  **Export Declarative Policies**: Migrate hardcoded rules (like `allow-all-read`) into machine-readable policy definitions (e.g., OPA Rego or OSCAL Component Definitions) containing strict, auditable criteria.
2.  **Control Traceability**: Add OSCAL component mapping files linking `ProjectionAccessController` specifically to AC-3, AC-6, SC-28, and AU-12 compliance requirements.

### Security Defect Remediation
1.  **Implement Real Redaction**: Replace the dummy `redact_sensitive` implementation with a recursive JSON processing filter that navigates the `pii_paths` and `secret_paths` vectors, replacing matching target values with masked strings (e.g., `"[REDACTED]"`).
2.  **Pre-Compile Access Policies**: Compile `resource_pattern` regular expressions exactly once when an `AccessPolicy` is registered. Cache the compiled `regex::Regex` structure in memory (using `once_cell` or thread-safe reference wrappers) to eliminate dynamic CPU overhead and prevent ReDoS patterns.
3.  **Remove Blocking Runtime Calls**: Refactor `SourceReader` and `PluginReader` to be async-native traits (utilizing `#[async_trait]`), removing `block_on` and `block_in_place` operations.

---
## ⚠ Citation Warnings
- `crates/op-projection/src/bin/projection_server.rs:360`: file has 322 lines
- `crates/op-projection/src/bin/projection_server.rs:360`: file has 322 lines
