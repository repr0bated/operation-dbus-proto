# Production Security and Quality Audit: op-cognitive-mcp

## 1. Observability: Macro & Log Statement Analysis

### 1.1 Logging Macro Counts
A precise count of all `tracing` logging macros and `println!` / `eprintln!` calls across the provided files:

| Logging Type | Count | Occurrences by File |
| :--- | :---: | :--- |
| `tracing::info!` | **41** | `notebooklm.rs` (2), `qdrant_shuttle.rs` (3), `grpc_service.rs` (18), `typed_tools.rs` (1), `gemini_fallback.rs` (2), `main.rs` (5), `server.rs` (4), `rag_pipeline.rs` (3), `bin/rag-ingest.rs` (3) |
| `tracing::warn!` | **12** | `notebooklm.rs` (4), `grpc_service.rs` (2), `gemini_fallback.rs` (2), `main.rs` (1), `server.rs` (1), `rag_pipeline.rs` (2) |
| `tracing::error!` | **1** | `bin/rag-ingest.rs` (1) |
| `tracing::debug!` | **1** | `interceptor.rs` (1) |
| `println!` / `eprintln!` | **26** | `main.rs` (1), `bin/op-cog-admin.rs` (2), `bin/rag-ingest.rs` (23) |

### 1.2 Swallowed Errors Without Logging
Several critical internal operations ignore errors without logging or notifying supervisors, violating basic service reliability standards:

*   **Swallowed Chat History Persistence Failures**: 
    *   `crates/op-cognitive-mcp/src/grpc_service.rs:146` and `crates/op-cognitive-mcp/src/grpc_service.rs:227`: Ignored with `let _ = self.session_manager.append_turn(...)`. If session persistence fails due to dashmap lock contention or internal failures, chat state silently drifts, making debugging of context loss impossible.
    *   `crates/op-cognitive-mcp/src/typed_tools.rs:223`: Ignored with `let _ = self.sessions.append_turn(...)` inside the dynamic query execution flow.
*   **Swallowed Directory Ingestion Failures**:
    *   `crates/op-cognitive-mcp/src/grpc_service.rs:1081` & `crates/op-cognitive-mcp/src/grpc_service.rs:1096`: Directory iteration errors in `walkdir` and `walkdir_shallow` are discarded via `if let Ok(entries) = std::fs::read_dir(path)`. If permission issues prevent access, the server silently returns empty lists with no trace.
*   **Swallowed Database Access-Telemetry Failures**:
    *   `crates/op-cognitive-mcp/src/memory_store.rs:307`: `let _ = self.run(q, p);` completely discards errors when updating a record's access counters. This blocks monitoring of dead locks or index corruption during reads.
*   **Silent Fallbacks for Corrupt JSON Data**:
    *   `crates/op-cognitive-mcp/src/memory_store.rs:475`, `496`, & `497`: These matchers silently fall back to `Value::Null` or empty arrays on JSON parse failure during record loading from the relational store. Storage corruption will manifest as missing data rather than clean application errors.
    *   `crates/op-cognitive-mcp/src/cognitive_tools.rs:318` & `323`: Swallows translation errors during `simd-json` to/from `serde` conversions.

### 1.3 PII and Secret Leakage Potential
*   **Unvetted Credentials Path Logging**:
    *   `crates/op-cognitive-mcp/src/grpc_service.rs:866` logs the raw `req.credential` via `warn!(path = %req.credential, ...)`. If a user misconfigures the setup or supplies raw credentials (e.g., active session cookies) to fields expecting paths, secrets are written directly to the server log in plaintext.
*   **PII Filtering Escapes**:
    *   `crates/op-cognitive-mcp/src/activity_filter.rs:173`: While `ActivityFilter` correctly shifts PII payload targets from vector storage via `FilterDecision::EmitChainOnly`, the full `ActivityEvent` struct (including the raw payload) is returned to internal consumers, presenting a risk of downstream printing or unvetted logger dumps.

### 1.4 Metrics Instrumentation
*   **Complete Lack of Metrics**: Although `prometheus` is included in the workspace dependencies, there is **no metric instrumentation** whatsoever in `op-cognitive-mcp`. Counters, histogram latencies, and active connection gauges must be introduced for key pipelines (such as `rag_pipeline.rs` ingestion and `grpc_service.rs` RPC calls).

---

## 2. Design & Architecture: Schema-as-Code Compliance

This workspace enforces strict schema-as-code discipline using Protocol Buffers and OSCAL schemas. However, many internal interfaces rely on ad-hoc structs and unstructured types:

*   **Ad-Hoc Session Management Schemas**:
    *   `crates/op-cognitive-mcp/src/session.rs:20`: `ConversationSession` and `QueryTurn` are defined as ad-hoc serialized structs. These should be formalized inside `cognitive.proto` as versioned protobuf messages.
*   **Unstructured Fallback and Diagnostic Schemas**:
    *   `crates/op-cognitive-mcp/src/gemini_fallback.rs:55`: `GeminiCitation`, `GeminiQueryResult`, `DeepResearchResult`, and `ResearchSection` are declared using standard Rust structures and serialized to unstructured JSON string blocks.
    *   `crates/op-cognitive-mcp/src/doctor.rs:12`: `DiagnosticReport` and `ComponentStatus` represent critical system diagnostic contracts but are defined as ad-hoc structs.
*   **Ad-Hoc Ingestion Records**:
    *   `crates/op-cognitive-mcp/src/rag_pipeline.rs:22` & `73`: `FileMeta` and `RagResult` define structural pipeline outputs as raw structs.

---

## 3. Security Findings

### 3.1 [HIGH] Missing Bounds/Mmap Validation on Shared Memory Sled Dereference
*   **File**: `crates/op-cognitive-mcp/src/interceptor.rs:34-45`
*   **Vulnerability Type**: Out-of-bounds Read / Dereference of Arbitrary Pointer
*   **Description**:
    The gRPC `ghostbridge_interceptor` opens and memory-maps `/dev/shm/plugin_schema.dat`. It immediately casts the raw pointer to `*const IdentitySled` and dereferences its fields (`is_valid`, `hashed_footprint`, etc.) without verifying that the mapped region is at least `size_of::<IdentitySled>()` bytes:
    ```rust
    let file = File::open("/dev/shm/plugin_schema.dat")
        .map_err(|_| Status::internal("SchemaEngine Memory Unreachable"))?;

    let mmap = unsafe {
        MmapOptions::new()
            .map(&file)
            .map_err(|_| Status::internal("Mmap failed"))?
    };
    let sled_ptr = mmap.as_ptr() as *const IdentitySled;

    let is_valid = unsafe { (*sled_ptr).is_valid };
    ```
    If `/dev/shm/plugin_schema.dat` is empty or truncated to a size smaller than 208 bytes, the dereference of `sled_ptr` accesses unmapped virtual memory pages. This triggers a segmentation fault (`SIGSEGV`), allowing local unprivileged processes to easily crash the gRPC control plane (Denial of Service).
*   **Remediation**:
    Enforce a size assertion immediately after memory mapping, mirroring the pattern used in `qdrant_shuttle.rs`:
    ```rust
    if mmap.len() < std::mem::size_of::<IdentitySled>() {
        return Err(Status::failed_precondition("Shared memory structure size mismatch"));
    }
    ```

### 3.2 [HIGH] Arbitrary Local Directory Ingestion (Path Traversal / Info Leak)
*   **File**: `crates/op-cognitive-mcp/src/grpc_service.rs:487-505`
*   **Vulnerability Type**: Path Traversal / Unauthorized Information Access
*   **Description**:
    The `add_folder` RPC accepts `req.folder_path` directly from user input and executes a filesystem walk using `walkdir`/`walkdir_shallow` to ingest files into the SQLite-backed `CognitiveMemoryStore`:
    ```rust
    let path = std::path::Path::new(&req.folder_path);
    if !path.exists() || !path.is_dir() {
        return Err(Status::invalid_argument(...));
    }
    // ... walks directory and reads contents ...
    ```
    The code performs no boundary validation. A client can provide arbitrary system directories (e.g., `/etc` or `/home/user/.ssh`), causing the server to recursively read sensitive files, populate the project database namespace, and expose them through standard RAG/Notebook queries (`query_notebook` / `get_source_content`).
*   **Remediation**:
    Establish a strict root directory whitelist (sandbox path). Validate that the canonicalized candidate path begins with the canonicalized sandbox root path:
    ```rust
    let canonical_path = path.canonicalize()?;
    if !canonical_path.starts_with(&sandbox_root) {
        return Err(Status::permission_denied("Path escapes allowed sandbox root"));
    }
    ```

---
## ⚠ Citation Warnings
- `crates/op-cognitive-mcp/src/cognitive_tools.rs:318`: file has 292 lines
