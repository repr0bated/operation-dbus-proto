| Pattern | File:Line | Crate Approach | Corpus Best Practice | Gap | Rating |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `format_json_manual` | `crates/op-cognitive-mcp/src/activity_filter.rs:82` | Creates string paths ad-hoc using `format!` and validates against schemas. | Use versioned schemas or strongly-typed path structures rather than manual formatting. | Manual string parsing/formatting instead of type-safe path representation. | Minor Gap |
| `unwrap_expect` | `crates/op-cognitive-mcp/src/activity_filter.rs:476` | Uses `.unwrap()` in testing environments to handle asynchronous output. | Propagate errors using `?` or add descriptive context with `.expect()`. | Raw `.unwrap()` reduces code legibility and context during test failures. | Minor Gap |
| `unwrap_expect` | `crates/op-cognitive-mcp/src/activity_filter.rs:502` | Uses raw `.unwrap()` to evaluate test filter outcomes. | Propagate errors or use standard assertions. | Raw `.unwrap()` inside tests doesn't bubble up readable context. | Minor Gap |
| `format_json_manual` | `crates/op-cognitive-mcp/src/notebooklm.rs:164` | Constructs description fields manually via `format!`. | Derive fields dynamically from versioned Proto/JSON schema generators. | Violates Schema-as-Code discipline by building unstructured interface descriptions. | Minor Gap |
| `unsafe_block` | `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:313` | Maps file memory using an unannotated `unsafe` block. | Map memory after validating file attributes; include explicit safety contracts. | Lack of explicit safety contract documentation (`// SAFETY:`). | Minor Gap |
| `unsafe_block` | `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:324` | Dereferences unaligned raw pointers mapped directly from a file. | Deserialize structured binary data safely using protocol schemas (e.g. Protocol Buffers, FlatBuffers). | Dangerous cast of disk bytes directly to struct types without safety boundaries. | Major Gap |
| `format_json_manual` | `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:75` | Contextualizes builder results via `format!`. | Use standard format wrapper strings inside structured context blocks. | None (compliant context usage). | Compliant |
| `format_json_manual` | `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:78` | Appends error descriptions with `format!`. | Standard error formatting blocks. | None (compliant error handling context). | Compliant |
| `format_json_manual` | `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:271` | Contextualizes Voyage API errors with manual `format!`. | Standard contextual blocks. | None. | Compliant |
| `unwrap_expect` | `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:517` | Unwraps thread join/channel operations abruptly. | Gracefully handle channel terminations using standard error propagation. | Potential for unexpected panics cascading to the parent executor thread. | Minor Gap |
| `unwrap_expect` | `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:527` | Raw `.unwrap()` on runtime operations. | Propagate task errors or use `.context()`. | Potential unhandled task execution panic. | Minor Gap |
| `std_fs_in_async` | `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:1` | Imports synchronous standard filesystem APIs. | Use asynchronous equivalents or offload blocking operations. | Blocks Tokio's async worker threads with synchronous file operations. | Major Gap |
| `unwrap_expect` | `crates/op-cognitive-mcp/src/session.rs:190` | Uses raw `.unwrap()` on test variables. | Graceful test failure validation or explicit expectation messaging. | Manual unwrapping without contextual messages. | Minor Gap |
| `std_fs_in_async` | `crates/op-cognitive-mcp/src/grpc_service.rs:558` | Reads files using synchronous blockages (`std::fs::read_to_string`). | Use `tokio::fs::read_to_string` to avoid thread starvation. | Blocks asynchronous tasks synchronously, crippling service performance. | Major Gap |
| `std_fs_in_async` | `crates/op-cognitive-mcp/src/grpc_service.rs:865` | Inspects system file metadata using synchronous calls. | Execute file metadata operations asynchronously. | Synchronous standard filesystem access blocks asynchronous worker thread pools. | Major Gap |
| `std_fs_in_async` | `crates/op-cognitive-mcp/src/grpc_service.rs:1077` | Reads directory nodes synchronously in a recursive function. | Use asynchronous streams or offload blocking execution tasks. | Blocks async thread runtime during execution directory searches. | Major Gap |
| `std_fs_in_async` | `crates/op-cognitive-mcp/src/grpc_service.rs:1093` | Recurses directory structure synchronously. | Use `tokio::fs::read_dir` within async tasks. | Synchronous path traversal blocks the scheduler pool of the service. | Major Gap |
| `unsafe_block` | `crates/op-cognitive-mcp/src/interceptor.rs:29` | Maps persistent file blocks directly to memory maps inside unsafe blocks. | Validate mappings and verify target boundary size before reading. | Lack of size checks or safety annotations. | Minor Gap |
| `unsafe_block` | `crates/op-cognitive-mcp/src/interceptor.rs:36` | Dereferences mapped pointer fields without type constraints or alignment checks. | Rely on robust versioned protocols or strictly check the byte array bounds first. | Violates Schema-as-Code; risks memory corruption if files are malformed. | Major Gap |
| `unsafe_block` | `crates/op-cognitive-mcp/src/interceptor.rs:37` | Unsafely accesses fields of memory mapped raw structure objects. | Use structured serialization frameworks to extract data safely. | Memory-mapped out-of-bounds reading potential from corrupted headers. | Major Gap |
| `process_exit` | `crates/op-cognitive-mcp/src/main.rs:96` | Exits the process during argument validation mismatch using `process::exit`. | Elevate errors out of `main` to propagate stack unwinding cleanly. | None (standard CLI bootstrap error validation). | Compliant |

---

### Actionable Recommendations for Major Gaps

#### 1. Replace Synchronous File Systems in Asynchronous Contexts
* **Gaps Identified**: `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:1`, `crates/op-cognitive-mcp/src/grpc_service.rs:558`, `crates/op-cognitive-mcp/src/grpc_service.rs:865`, `crates/op-cognitive-mcp/src/grpc_service.rs:1077`, `crates/op-cognitive-mcp/src/grpc_service.rs:1093`
* **Root Cause**: The async runtime (such as Tokio) runs multiple state machines concurrently on a limited thread pool. Blocking these threads with synchronous functions (e.g., `std::fs::read_to_string`, `std::fs::read_dir`) degrades throughput, raises tail latency, and can lead to thread starvation.
* **Remediation**:
  * Replace the imports of `std::fs` with `tokio::fs` in async modules.
  * Update directory iteration loops to use `tokio::fs::read_dir` combined with asynchronous streams:
    ```rust
    let mut entries = tokio::fs::read_dir(path).await?;
    while let Some(entry) = entries.next_entry().await? {
        let p = entry.path();
        // Process entry asynchronously
    }
    ```
  * For metadata verification and string file loading, swap standard library calls to their non-blocking asynchronous counterparts:
    ```rust
    let content = tokio::fs::read_to_string(&entry_path).await?;
    ```

#### 2. Eliminate Unsafe Pointer Dereferencing of Memory-Mapped Files
* **Gaps Identified**: `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:324`, `crates/op-cognitive-mcp/src/interceptor.rs:36`, `crates/op-cognitive-mcp/src/interceptor.rs:37`
* **Root Cause**: Memory mapping a file directly on disk to read data contracts without checking file size, structural boundaries, or binary layout constraints can result in alignment violations, out-of-bounds pointer dereferencing, and execution faults. This violates the Schema-as-Code discipline.
* **Remediation**:
  * Implement safe structural serialization with versioned Protocol Buffers or FlatBuffers. This avoids mapping direct binary layouts that may change across systems or compiler updates.
  * If memory-mapped file casting is absolutely required, validate that the mapped slice's length is at least `std::mem::size_of::<IdentitySled>()` before performing pointer manipulation:
    ```rust
    if mmap.len() < std::mem::size_of::<IdentitySled>() {
        return Err(anyhow::anyhow!("File is too small to contain safe structural data"));
    }
    ```
  * Implement a magic byte header check and write an explicit `// SAFETY:` block documenting the alignment constraints and safety invariants of the structure layout.