# Production Security and Quality Audit: op-cognitive-mcp

## 1. Critical Findings

### 1.1. Unsanitized gRPC Input Path Traversal & Arbitrary File Read
*   **File/Line**: `crates/op-cognitive-mcp/src/grpc_service.rs:440`
*   **Type**: Path Traversal / Arbitrary File Read
*   **Impact**: Critical (Directly Exploitable)
*   **Description**:
    The gRPC endpoint method `add_folder` accepts an arbitrary `folder_path` parameter directly from the client request. The path is immediately evaluated and passed to filesystem traversal operations (`walkdir` and `walkdir_shallow`) on line 480 without sandbox restriction, boundary verification, or canonicalization checks:
    ```rust
    let path = std::path::Path::new(&req.folder_path);
    if !path.exists() || !path.is_dir() { ... }
    ```
    An attacker can supply sensitive system paths (e.g., `/etc` or `/var/lib`) to read host files. These files are ingested into the database as source documents, which the attacker can then extract using `ask_question`, `list_sources`, or `get_source_content`. Since this service registers on the D-Bus system bus, it is likely running with elevated permissions, making system-wide compromise trivial.

---

### 1.2. Missing Bounds Validation on Shared Memory Map Leading to Local Denial of Service (SIGSEGV/SIGBUS)
*   **File/Line**: `crates/op-cognitive-mcp/src/interceptor.rs:32`
*   **Type**: Unsafe Memory Access / Out-of-bounds Read
*   **Impact**: Critical (Directly Exploitable)
*   **Description**:
    The `ghostbridge_interceptor` maps `/dev/shm/plugin_schema.dat` into memory using the `memmap2` crate:
    ```rust
    let mmap = unsafe {
        MmapOptions::new()
            .map(&file)
            .map_err(|_| Status::internal("Mmap failed"))?
    };
    ```
    On line 30, it immediately casts the pointer to an `IdentitySled` struct:
    ```rust
    let sled_ptr = mmap.as_ptr() as *const IdentitySled;
    ```
    There is no length check verifying that the mapped region is at least `size_of::<IdentitySled>()` bytes. Because `/dev/shm` is world-writable on standard Linux environments, any local unprivileged user can create a 0-byte or 1-byte file at `/dev/shm/plugin_schema.dat`. When a gRPC request triggers the interceptor, the pointer dereference at `(*sled_ptr).is_valid` triggers an out-of-bounds read and causes an immediate segmentation fault (SIGSEGV or SIGBUS), permanently crashing the service.

---

## 2. Non-Critical / Design Quality Findings

### 2.1. Silent ABI Drift and Struct Layout Mismatch on Shared Memory Sled
*   **File/Line**: `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:26` and `crates/op-cognitive-mcp/src/interceptor.rs:5`
*   **Type**: ABI Incompatibility / Memory Corruption Risk
*   **Impact**: High
*   **Description**:
    The shared memory mapping `/dev/shm/plugin_schema.dat` is parsed using two different struct definitions of `IdentitySled`:
    *   In `src/qdrant_shuttle.rs:26`, `IdentitySled` does not contain explicit padding between `is_valid` (bool) and `hashed_footprint`:
        ```rust
        pub struct IdentitySled {
            pub wireguard_pubkey: [u8; 32],
            pub mutation_index: u64,
            pub is_valid: bool,
            pub hashed_footprint: [u8; 32],
        }
        ```
    *   In `src/interceptor.rs:5`, the struct contains a 7-byte padding array to align fields to 8 bytes, along with extra metadata fields:
        ```rust
        pub struct IdentitySled {
            pub wireguard_pubkey: [u8; 32],
            pub mutation_index: u64,
            pub is_valid: bool,
            pub _pad: [u8; 7],
            pub hashed_footprint: [u8; 32],
            ...
        }
        ```
    Because of this layout mismatch, `qdrant_shuttle.rs` evaluates `hashed_footprint` starting at offset 41, whereas `interceptor.rs` evaluates it from offset 48. This leads to broken trace comparisons and authentication failures.

---

### 2.2. Unchecked Recursive Directory Walking (Memory & Disk Exhaustion DoS)
*   **File/Line**: `crates/op-cognitive-mcp/src/grpc_service.rs:480`
*   **Type**: Resource Exhaustion
*   **Impact**: Medium
*   **Description**:
    When calling `add_folder` with `recursive = true`, the walk operation does not place limits on maximum recursion depth, maximum directory entries, or individual file sizes. If directed to walk massive directories, the server will block worker threads, exhaust RAM, and potentially saturate the CozoDB database on disk, causing a denial of service.

---

### 2.3. Time-of-Check to Time-of-Use (TOCTOU) Race Condition in Deduplication Filter
*   **File/Line**: `crates/op-cognitive-mcp/src/activity_filter.rs:319`
*   **Type**: Concurrency / Race Condition
*   **Impact**: Low
*   **Description**:
    `evaluate` determines if an event is a duplicate under a read-lock on the sliding window, releases that lock, and then acquires a write-lock to insert the new entry on line 328. In high-concurrency environments, identical duplicate events can bypass the read-lock check simultaneously and both be added, causing redundant emissions.

---

### 2.4. Credential Visibility in Process Environment
*   **File/Line**: `crates/op-cognitive-mcp/src/notebooklm.rs:52` and `crates/op-cognitive-mcp/src/gemini_fallback.rs:28`
*   **Type**: Insecure Credential Storage
*   **Impact**: Low
*   **Description**:
    The server reads sensitive authorization tokens (such as `NOTEBOOKLM_COOKIE` and `GEMINI_API_KEY`) from environment variables. On shared Linux systems, environment variables of running processes can be read by other users through `/proc/<pid>/environ`.

---

## 3. Schema-as-Code Violations

The codebase is expected to maintain strict schema-as-code discipline using Protocol Buffers and OSCAL. The following areas bypass versioned schemas in favor of ad-hoc JSON structures or unstructured strings:

| File:Line | Description | Contract Type | Resolution Recommendation |
| :--- | :--- | :--- | :--- |
| `src/activity_filter.rs:129` | `ActivityEvent::payload` is represented as an unstructured `serde_json::Value` rather than a compiled protobuf or schema-validated type. | Unstructured JSON | Define a compiled protobuf schema containing structured event payload types. |
| `src/cognitive_tools.rs:62` | `MemoryTool::input_schema()` constructs an ad-hoc JSON schema definition using the `json!` macro at runtime. | Ad-hoc Schema | Define tools and their input schemas using standard declarative Protocol Buffers. |
| `src/typed_tools.rs:112` | `TypedQueryTool::input_schema()` registers hardcoded JSON structures in memory. | Ad-hoc Schema | Replace with versioned declarative registry models. |
| `src/gemini_fallback.rs:78` | Communication with Gemini API is typed via ad-hoc serialization structs like `GeminiRequest` and `GeminiResponse`. | Ad-hoc JSON API | Model the internal API boundaries using unified OpenAPI or Protobuf specs. |
| `src/doctor.rs:13` | `DiagnosticReport` structures use ad-hoc string and nested JSON arrays rather than structured OSCAL compliance schemas. | Compliance Report | Align diagnostic output structures with standard OSCAL-compliant schema objects. |

---

## 4. Public API Surface & Dead Code

### 4.1. Public API Surface Summary
*   **Total `pub` items found**: 143 items.

#### Top 10 Most Impactful Public API Surface Items:
1.  **`CognitiveMcpServer`** (`crates/op-cognitive-mcp/src/server.rs:19`): Main runtime manager coordinating dual servers (SSE & gRPC) and DBus bindings.
2.  **`CognitiveGrpcService`** (`crates/op-cognitive-mcp/src/grpc_service.rs:35`): Ingress service executing NotebookLM grounded queries.
3.  **`ActivityFilter`** (`crates/op-cognitive-mcp/src/activity_filter.rs:254`): Pipeline component enforcing event significance filtering and PII scrubbing.
4.  **`CognitiveMemoryStore`** (`crates/op-cognitive-mcp/src/memory_store.rs:104`): SQLite-free CRUD boundary over CozoDB memory spaces.
5.  **`SessionManager`** (`crates/op-cognitive-mcp/src/session.rs:43`): Conversation context persistence layer.
6.  **`QuotaManager`** (`crates/op-cognitive-mcp/src/quota.rs:28`): Thread-safe rate limiter checking daily user usage tiers.
7.  **`RagPipeline`** (`crates/op-cognitive-mcp/src/rag_pipeline.rs:122`): Extraction-embedding pipeline connecting files to vector DB layers.
8.  **`GeminiFallback`** (`crates/op-cognitive-mcp/src/gemini_fallback.rs:127`): API client providing resilience when NotebookLM sidecar bridges fail.
9.  **`ghostbridge_interceptor`** (`crates/op-cognitive-mcp/src/interceptor.rs:19`): System security boundary validating gRPC request metadata against sled signatures.
10. **`CognitiveMcpInterface`** (`crates/op-cognitive-mcp/src/dbus_interface.rs:17`): DBus messaging registry.

#### Glob Re-exports (`pub use *`):
*   No glob re-exports (`pub use *`) were found in any files. All re-exports list specific items (e.g. `pub use activity_filter::{...}`).

#### Structs with exposed `pub` fields:
*   `ActivityEvent` (`activity_filter.rs:129`): Exposes raw operational data fields directly. Should be refactored to use getter methods to preserve immutability.
*   `GeminiConfig` (`gemini_fallback.rs:21`): Exposes raw config fields, including `pub api_key`.
*   `IdentitySled` (`qdrant_shuttle.rs:26`): Exposes byte arrays directly, increasing risk of byte alignment corruption.

---

### 4.2. Dead Code Review

| Item | Type | File:Line | Recommendation |
| :--- | :--- | :--- | :--- |
| `VoyageClient` | Struct | `crates/op-cognitive-mcp/src/voyage.rs:7` | **Remove**: This struct is entirely redundant with the local `VoyageClient` definition in `src/qdrant_shuttle.rs:180` and is never imported or used. |
| `OpKind::WorkflowStep` | Enum Variant | `crates/op-cognitive-mcp/src/activity_filter.rs:113` | **Remove or Implement**: Never constructed or matched. |
| `OpKind::SessionLifecycle` | Enum Variant | `crates/op-cognitive-mcp/src/activity_filter.rs:114` | **Remove or Implement**: Never constructed or matched. |
| `FileType::Other` | Enum Variant | `crates/op-cognitive-mcp/src/rag_pipeline.rs:67` | **Remove**: Unused as all classifications fall back to `FileType::Source` as a catch-all. |
| `CognitiveMemoryStore::cleanup_expired` | Method | `crates/op-cognitive-mcp/src/memory_store.rs:395` | **Expose or Integrate**: This cleanup function is defined but never called by any active gRPC endpoint, DBus command, or background daemon. |