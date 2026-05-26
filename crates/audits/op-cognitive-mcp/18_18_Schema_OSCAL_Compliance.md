### Schema-as-Code Audit

| Item | Type | file:line | Has .proto? | Gap |
| :--- | :--- | :--- | :--- | :--- |
| `IdentitySled` | ABI Struct | `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:32` | No | Shared-memory layout mapped directly from `/dev/shm/plugin_schema.dat` with no machine-readable IDL or Protobuf schema. |
| `IdentitySled` | ABI Struct | `crates/op-cognitive-mcp/src/interceptor.rs:5` | No | Secondary mismatched definition of the shared-memory ABI mapping the same file path with different offsets and padding fields. |
| `ActivityEvent` | Event Struct | `crates/op-cognitive-mcp/src/activity_filter.rs:136` | No | Data contract containing untyped payload (`serde_json::Value` at line 186) rather than a versioned schema. |
| `ListTools` / `GetToolSchema` / `CallTool` | D-Bus RPC Interface | `crates/op-cognitive-mcp/src/dbus_interface.rs:30` | No | Untyped string serialization boundaries returning ad-hoc JSON payload strings instead of strongly typed Protobuf/gRPC message structures. |
| `MemoryTool` / `TypedQueryTool` / `TypedStoreTool` | MCP Tool Definitions | `crates/op-cognitive-mcp/src/cognitive_tools.rs:55`, `crates/op-cognitive-mcp/src/typed_tools.rs:114`, `crates/op-cognitive-mcp/src/typed_tools.rs:250` | No | Input schemas dynamically built using `simd_json::OwnedValue` instead of referencing generated Protobuf message contracts. |
| `GeminiRequest` / `GeminiResponse` | API Structs | `crates/op-cognitive-mcp/src/gemini_fallback.rs:71`, `crates/op-cognitive-mcp/src/gemini_fallback.rs:103` | No | Hand-rolled external HTTP payload mapping structures bypassing versioned OpenAPI or Protobuf schemas. |

---

### OSCAL Coverage

| Control Area | Implemented at file:line | OSCAL Artifact | Gap |
| :--- | :--- | :--- | :--- |
| **System and Information Integrity (SI-4 / Information System Monitoring)** | `crates/op-cognitive-mcp/src/activity_filter.rs:38` | None | Event significance gate derives classifications dynamically from schema tags (`noise`, `overkill`, `immutable`) but lacks mapping in an OSCAL `component-definition`. |
| **Media Protection (MP-6 / Media Sanitization)** | `crates/op-cognitive-mcp/src/activity_filter.rs:198` | None | Dynamic PII detection and redaction algorithm strips payload metrics before vector store ingestion, but lacks explicit tracking under an OSCAL `system-security-plan` (SSP). |
| **Access Control (AC-3 / Access Enforcement)** | `crates/op-cognitive-mcp/src/interceptor.rs:17` | None | The `ghostbridge_interceptor` enforces gRPC request authentication using temporal footprint hash validation, but this gate has no mapping to an OSCAL control or systemic audit record. |
| **System and Communications Protection (SC-5 / Denial of Service Protection)** | `crates/op-cognitive-mcp/src/quota.rs:41` | None | Usage rate-limiting and tier quota boundaries are hardcoded in Rust structs with no machine-readable policy linkage to compliance definitions. |
| **Access Control (AC-6 / Least Privilege)** | `crates/op-cognitive-mcp/src/grpc_service.rs:1040` | None | Verifies that Chrome profiles/credentials directories are restricted with standard UNIX directory mask `0o600` but contains no declarative policy linkage. |

---

### Recommendations

#### 1. CRITICAL: Resolve ABI Mismatch and Add Bounds Verification to Shared Memory Interceptor
* **Location:** `crates/op-cognitive-mcp/src/interceptor.rs:5` vs. `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:32`
* **Vulnerability:** The `IdentitySled` struct is defined twice inside the same crate with incompatible sizes, fields, and byte alignments. In `interceptor.rs`, the struct is 208 bytes long and includes an explicit `_pad` field and several supplementary tail fields. In `qdrant_shuttle.rs`, the struct is approximately 80 bytes long. 
* **Impact:** 
  1. The `ghostbridge_interceptor` (at `interceptor.rs:27`) memory-maps `/dev/shm/plugin_schema.dat` and immediately casts the raw pointer without validating that `mmap.len() >= size_of::<IdentitySled>()`. If the file was written by a component using the smaller size layout (approx 80 bytes), dereferencing fields like `control_source` (which offset past 160 bytes) will read out-of-bounds of the mapped memory region, triggering an immediate `SIGSEGV` and crashing the gRPC server.
  2. The temporal hash check (`request_footprint != expected_footprint`) will fail or read garbage memory because of offset desynchronization of `hashed_footprint`.
* **Remediation:** 
  1. Consolidate `IdentitySled` into a single, canonical, shared definition inside an interface module (e.g., `op-core` or a common library).
  2. In `interceptor.rs:27`, insert a hard bounds check before dereferencing:
     ```rust
     ensure!(mmap.len() >= std::mem::size_of::<IdentitySled>(), Status::failed_precondition("Shared memory structure size mismatch."));
     ```

#### 2. CRITICAL: Prevent Arbitrary File Ingest and Directory Traversal in `add_folder`
* **Location:** `crates/op-cognitive-mcp/src/grpc_service.rs:555`
* **Vulnerability:** The `add_folder` RPC accepts an unvalidated `folder_path` argument from the client. It converts this string directly to a path block (`let path = std::path::Path::new(&req.folder_path);`), verifies its existence, and recursively traverses the filesystem via synchronous directory walks, loading the target file streams into memory (`std::fs::read_to_string`).
* **Impact:** Any authenticated caller of the gRPC interface or local LLM agent triggering this tool can bypass workspace boundaries. By supplying parameters like `/etc`, `/root`, or relative directory traversal arguments, they can force the server to ingest sensitive system configurations (e.g., system shadow files, SSH keyrings, internal credentials) directly into the database index, allowing simple subsequent exfiltration through search queries.
* **Remediation:** 
  1. Canonicalize the input path and validate it against an explicit, strict directory allowlist (such as `/var/lib/op-cognitive-mcp` or a designated workspace folder):
     ```rust
     let canonical_target = std::fs::canonicalize(path)?;
     let allowed_root = std::fs::canonicalize("/var/lib/op-cognitive-mcp/workspace")?;
     if !canonical_target.starts_with(&allowed_root) {
         return Err(Status::permission_denied("Directory path is outside allowed workspace boundaries."));
     }
     ```
  2. Implement an explicit lock to prevent symlink traversal during recursive folder walking by ignoring symlinks during directory iterations.

#### 3. HIGH: Remediate In-Memory Session Leak in `SessionManager`
* **Location:** `crates/op-cognitive-mcp/src/session.rs:43`
* **Vulnerability:** `SessionManager` utilizes a persistent in-memory `DashMap<String, ConversationSession>` with no eviction policy or maximum storage capacity. Although the historical turns within an individual session are capped via `max_history`, the master session keys are never cleaned up or expired.
* **Impact:** Attackers can generate high numbers of dummy requests with randomized, fresh `conversation_id` values. This will continuously spawn active entries inside the `DashMap`, exhausting available RAM and causing a Denial of Service (DoS) crash via Out-Of-Memory (OOM).
* **Remediation:** Implement an active Least-Recently-Used (LRU) eviction cache or apply a standard Time-To-Live (TTL) expiration loop using `tokio::time` to drop old session maps.

#### 4. HIGH: Align Schema-as-Code Contracts to Protobuf definitions
* **Location:** `crates/op-cognitive-mcp/src/activity_filter.rs:186`, `crates/op-cognitive-mcp/src/dbus_interface.rs:30`
* **Gap:** Public interfaces (such as the D-Bus registry methods and structural filtering events) return unstructured payload maps and serialized JSON strings (`serde_json::Value` and `simd_json::OwnedValue`). This breaks standard schema-as-code discipline, bypassing static contract compilation checks.
* **Remediation:** Define standard schema payloads for dynamic tool metadata and event structures inside a centralized protocol buffer definition (e.g., `cognitive.proto`), using Tonic and Prost code generation blocks to enforce typed verification.

#### 5. MEDIUM: Author OSCAL Machine-Readable Declarative Compliance Mappings
* **Location:** `crates/op-cognitive-mcp/src/server.rs`
* **Gap:** The codebase implements several distinct security compliance features—such as PII masking (`activity_filter.rs`), role/authorization gates (`interceptor.rs`), and denial-of-service prevention boundaries (`quota.rs`)—but contains no declarative compliance artifacts linking these to NIST SP 800-53 or FedRAMP frameworks.
* **Remediation:** Create an OSCAL `component-definition` in JSON/YAML format. Map the gRPC and D-Bus ingress routes to specific NIST SP 800-53 controls (e.g., **SC-5** for the quota limit, **MP-6** for the PII stripping gates, and **AU-2/AU-3** for the audit filter engine). This bridges active software implementation constraints directly to machine-readable audit configurations.