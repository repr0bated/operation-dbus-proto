### ROLE: Build Check

*   **Edition**: `2021` is defined both in the workspace root `Cargo.toml` and in `crates/op-cognitive-mcp/Cargo.toml`.
*   **Rust Version**: No `rust-version` field is explicitly specified in either `Cargo.toml` or `crates/op-cognitive-mcp/Cargo.toml`.
*   **Bins**: 
    *   `rag-ingest` (path: `crates/op-cognitive-mcp/src/bin/rag-ingest.rs`)
    *   `op-cog-admin` (path: `crates/op-cognitive-mcp/src/bin/op-cog-admin.rs`)
    *   Implicit binary: `main.rs` (compiles to `op-cognitive-mcp` daemon).
*   **Examples**: None defined in the crate.
*   **Workspace Inheritance**: 
    *   `crates/op-cognitive-mcp/Cargo.toml` inherits dependencies from the workspace using `{ workspace = true }` for: `op-cozo-store`, `hex`, `memmap2`, `serde_json`, `simd-json`, `anyhow`, `reqwest`, `clap`, `cozo`, `tonic`, `prost`, `tonic-reflection`, `tonic-health`, `tonic-web`, `sha2`, `regex`, `dashmap`, `parking_lot`, and `zbus`.
    *   **Local Overrides**:
        *   `tokio` is declared locally as `{ version = "1.0", features = ["full"] }` instead of inheriting from the workspace `{ version = "1", features = ["full"] }`.
        *   `serde` is declared locally as `{ version = "1.0", features = ["derive"] }` instead of inheriting from the workspace `{ version = "1", features = ["derive"] }`.
        *   `qdrant-client` is declared locally as `"1.17"` in `crates/op-cognitive-mcp/Cargo.toml`. However, the workspace `Cargo.toml` defines `qdrant-client = "1.7"`. This is a critical version mismatch (v1.7 vs v1.17) that will force dual compilation of completely distinct client APIs and potentially trigger linker conflicts or duplicate symbols if types are shared across the crate boundaries.

---

### SCHEMA-AS-CODE BUILD CHECK

*   **Protocol Buffer Compilation**:
    *   `crates/op-cognitive-mcp` specifies a build-dependency on `tonic-build = { version = "0.12" }`.
    *   `crates/op-cognitive-mcp/src/lib.rs:25-33` invokes `tonic::include_proto!("operation.cognitive.v1")` and `tonic::include_file_descriptor_set!("cognitive_descriptor")`.
    *   This indicates `build.rs` compiles `proto/cognitive.proto` into Rust files at **build time**.
*   **Source of Truth**:
    *   The Protocol Buffer `.proto` file (`proto/cognitive.proto`) acts as the source of truth for the gRPC service contracts.
*   **Findings**:
    *   **Ad-Hoc Schema Violation**: Multiple data contracts in the crate bypass versioned schema-as-code discipline. Ad-hoc JSON shapes are constructed directly via `serde_json::json!` or represented as untyped `serde_json::Value` objects inside structured stores:
        *   `crates/op-cognitive-mcp/src/memory_store.rs:59-69`: `MemoryEntry` and `MemoryNamespace` define their primary data payloads using `serde_json::Value` (untyped JSON objects).
        *   `crates/op-cognitive-mcp/src/grpc_service.rs:360-364` and `crates/op-cognitive-mcp/src/grpc_service.rs:412-416`: Ingested file contents and source metadata are defined as inline ad-hoc JSON structures (`serde_json::json!({ "source_type": ..., "content": ... })`) rather than strongly-typed, versioned schema definitions.
        *   `crates/op-cognitive-mcp/src/dbus_interface.rs:35-39`: D-Bus tool list responses are serialized from raw, ad-hoc JSON values constructed dynamically on each query.

---

### VULNERABILITY & QUALITY AUDIT

#### 1. CRITICAL: Memory Layout & Size Discrepancy on `IdentitySled` Shared Memory Mapping
*   **Vulnerability Type**: ABI Mismatch / Out-of-Bounds Read / Memory Corruption
*   **Citations**:
    *   `crates/op-cognitive-mcp/src/interceptor.rs:5-15`
    *   `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:24-30`
    *   `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:312-329`
*   **Description**:
    The system maps a shared memory file `/dev/shm/plugin_schema.dat` to coordinate state between the gRPC interceptor and the Qdrant semantic shuttle. However, the `IdentitySled` struct is defined with completely different structures, padding, and sizes in the two modules:
    *   In `interceptor.rs:5-15`, `IdentitySled` is defined as a large 208-byte struct featuring explicit alignment padding (`_pad: [u8; 7]`) and downstream fields like `schema_uuid`, `subid`, `control_source`, and `nextdns_profile`.
    *   In `qdrant_shuttle.rs:24-30`, `IdentitySled` is a small 80-byte struct that completely lacks the `_pad` field and all downstream metadata fields.
    
    This structural mismatch results in two critical failures:
    1.  **Offset Shift**: Because `qdrant_shuttle.rs` omits `_pad: [u8; 7]` after `is_valid: bool`, the `hashed_footprint` array is read starting at offset 41 instead of offset 48. This leads to continuous `Temporal Hash Mismatch` errors inside `ghostbridge_interceptor` whenever it tries to validate the client's cryptographic footprint.
    2.  **Offset Deserialization Corruption**: In `qdrant_shuttle.rs:326-328`, the shuttle attempts to read appended `PluginSchema` JSON bytes immediately after the sled header by slicing the mmap using `size_of::<IdentitySled>()` as the offset. Because `size_of::<IdentitySled>()` in `qdrant_shuttle.rs` evaluates to 80 bytes instead of the actual 208-byte writer struct boundary, the parser will treat raw header fields (such as `subid` and `control_source`) as raw JSON bytes. This results in guaranteed JSON deserialization failures at runtime, completely breaking the accountability loop.

---

#### 2. HIGH: TOCTOU Race Condition in Activity Filter Deduplication Window
*   **Vulnerability Type**: Time-of-Check to Time-of-Use (TOCTOU) / Race Condition
*   **Citations**:
    *   `crates/op-cognitive-mcp/src/activity_filter.rs:260-281`
*   **Description**:
    The deduplication check in `ActivityFilter::evaluate` splits its lock acquisitions across an asynchronous gap. 
    1.  First, it acquires a read lock on the sliding window to verify if the event's content hash is already registered:
        ```rust
        let is_dup = self.window.read().await.iter().any(|e| e.content_hash == event.content_hash);
        ```
    2.  If `is_dup` is false, it drops the read lock and executes downstream logic before finally acquiring a write lock to append the new hash:
        ```rust
        let mut w = self.window.write().await;
        w.push_back(WindowEntry { ... });
        ```
    
    Because the read lock is dropped before the write lock is acquired, two concurrent, identical events processed in parallel threads can both successfully pass the duplicate check. This allows duplicate events to bypass the filter and pollute downstream consensus systems (blockchain audit logs and Qdrant vector spaces). To fix this, the check and insertion must be performed atomically within a single write lock block.

---

#### 3. HIGH: Broken Pagination/Filtering Logic via Post-Fetch Memory Filtering in `query_entries`
*   **Vulnerability Type**: Logic Error / Query Bypass
*   **Citations**:
    *   `crates/op-cognitive-mcp/src/memory_store.rs:510-520`
*   **Description**:
    In `CognitiveMemoryStore::query_entries`, parameters like `key_pattern` and `tags` are not passed down to the database query engine (CozoDB). Instead, they are filtered *in memory* after fetching a truncated set of records from the database using the query limit (defaulting to 100):
    ```rust
    let rows = self.run(script, params).context("query entries")?;
    let mut entries: Vec<MemoryEntry> = rows.rows.iter().map(row_to_entry).collect();

    // Apply key_pattern (substring match) post-fetch.
    if let Some(pat) = &q.key_pattern {
        entries.retain(|e| e.key.contains(pat));
    }
    ```
    If a namespace contains more entries than the fetch limit, and the matching records occur beyond the first 100 entries, the function will return an empty vector or an incomplete dataset. This breaks database query safety and results in intermittent search failures as the database grows. All filtering parameters must be translated into the native Datalog execution script.

---

#### 4. HIGH: Path Traversal and Arbitrary File Leakage via gRPC `AddFolder` Endpoint
*   **Vulnerability Type**: Arbitrary File Read / Path Traversal
*   **Citations**:
    *   `crates/op-cognitive-mcp/src/grpc_service.rs:608-622`
    *   `crates/op-cognitive-mcp/src/grpc_service.rs:629-654`
*   **Description**:
    The `add_folder` gRPC endpoint accepts a user-provided string `folder_path` and walks the specified directory using standard filesystem utilities without any validation or chroot restrictions:
    ```rust
    let path = std::path::Path::new(&req.folder_path);
    if !path.exists() || !path.is_dir() { ... }
    ...
    for entry_path in walker {
        match std::fs::read_to_string(&entry_path) {
            Ok(content) => { ... }
        }
    }
    ```
    This allows any client with gRPC access (or an agent dynamically invoking the equivalent underlying tool) to pass arbitrary absolute paths (e.g. `/etc` or `~/.ssh`) and dump their entire contents directly into the public memory store namespace.

---

#### 5. MEDIUM: API Key Leakage via HTTP Query Parameters in Gemini Fallback
*   **Vulnerability Type**: Cryptographic Credential Exposure
*   **Citations**:
    *   `crates/op-cognitive-mcp/src/gemini_fallback.rs:388-393`
*   **Description**:
    When the primary NotebookLM bridge fails, the fallback layer queries the Gemini API. However, the client formats the private Gemini API key directly into the query string parameter of the HTTP target URL:
    ```rust
    let url = format!(
        "{}/models/{}:generateContent?key={}",
        config.api_url, config.model, config.api_key
    );
    ```
    URL query parameters are commonly captured in plain text by intermediate reverse proxies, router logs, HTTP request tracing systems, and diagnostic dumps. API credentials must be transmitted via secure request headers (such as `x-goog-api-key`) rather than query parameters.

---

#### 6. MEDIUM: Unaligned Pointer Dereference (Undefined Behavior) in gRPC Interceptor
*   **Vulnerability Type**: Undefined Behavior (UB)
*   **Citations**:
    *   `crates/op-cognitive-mcp/src/interceptor.rs:27-32`
*   **Description**:
    Inside the `ghostbridge_interceptor`, the shared memory sled is directly dereferenced from a raw pointer obtained from a memory-mapped file:
    ```rust
    let mmap = unsafe { MmapOptions::new().map(&file).map_err(|_| Status::internal("Mmap failed"))? };
    let sled_ptr = mmap.as_ptr() as *const IdentitySled;
    let is_valid = unsafe { (*sled_ptr).is_valid };
    ```
    The memory address returned by `mmap.as_ptr()` is only guaranteed to be page-aligned (often 4096 bytes), which aligns with the struct's requirements. However, Rust's memory model strictly prohibits dereferencing raw pointers to types requiring higher alignment (such as `IdentitySled`, which aligns to 8 bytes due to `mutation_index: u64`) unless alignment has been formally verified or `std::ptr::read_unaligned` is used. Directly dereferencing `*sled_ptr` without alignment checks constitutes undefined behavior, which can cause compiler optimization anomalies or CPU exceptions on strict-alignment architectures.

---

#### 7. LOW: Duplicate Client Implementations for Voyage AI Embedding Services
*   **Vulnerability Type**: Code Quality / Maintainability
*   **Citations**:
    *   `crates/op-cognitive-mcp/src/voyage.rs:11-23`
    *   `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:173-200`
*   **Description**:
    The repository implements two completely separate and redundant `VoyageClient` structures to generate text embeddings. One is declared globally as `crates/op-cognitive-mcp/src/voyage.rs`, while another identical structure is re-declared inside `crates/op-cognitive-mcp/src/qdrant_shuttle.rs`. This duplication increases code maintenance overhead and can lead to diverging configuration parsing logic and inconsistent runtime behaviors. Use the shared implementation in `voyage.rs` across all modules.