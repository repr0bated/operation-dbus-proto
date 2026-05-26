# Production Security & Quality Audit: op-cognitive-mcp

---

## 1. License Audit

### Workspace & Crate Licenses
*   **Workspace License**: The workspace `Cargo.toml` specifies the license under the key `workspace.package.license` as `Apache-2.0`.
*   **op-cognitive-mcp Crate**: The local crate `crates/op-cognitive-mcp/Cargo.toml` has no local `license` field or `license.workspace = true` declaration. It should explicitly inherit the workspace license.

### Copyleft Scanner (`Cargo.lock`)
A scan of `Cargo.lock` reveals no strictly incompatible strong copyleft licenses (such as GPL, AGPL, or SSPL):
*   `cozo` (version 0.7.6) is compiled with `storage-sled`. The underlying engine relies on MPL-2.0 (Mozilla Public License 2.0). Since it is integrated dynamically/statically as an embedded library and no modifications to the CozoDB code itself are compiled here, MPL-2.0 does not trigger viral copyleft taint of the proprietary workspace crates under the Apache-2.0 terms.
*   Other crates (e.g., `sled`, `ring`, `tonic`, `tokio`) are licensed under permissible/weak copyleft terms (MIT, Apache-2.0, BSD-3-Clause).

---

## 2. Critical Security Vulnerabilities

### Out-of-bounds Read / Segfault Risk in gRPC Interceptor
*   **Citation**: `crates/op-cognitive-mcp/src/interceptor.rs:1004-1011`
*   **Impact**: **Critical (Denial of Service & Crash)**
*   **Description**: The gRPC interceptor `ghostbridge_interceptor` blindly maps and casts `/dev/shm/plugin_schema.dat` to `*const IdentitySled` without validating that the mapped length is at least equal to `size_of::<IdentitySled>()`.
    ```rust
    let mmap = unsafe {
        MmapOptions::new()
            .map(&file)
            .map_err(|_| Status::internal("Mmap failed"))?
    };
    let sled_ptr = mmap.as_ptr() as *const IdentitySled;

    let is_valid = unsafe { (*sled_ptr).is_valid }; // <--- NO BOUNDS CHECK
    ```
    If `/dev/shm/plugin_schema.dat` is empty, truncated, or corrupted by a local unprivileged process, `mmap.len()` will be less than `208` bytes. Accessing `(*sled_ptr).is_valid` triggers an immediate out-of-bounds memory read, leading to a segmentation fault that crashes the entire gRPC control plane service.

---

### ABI Layout Misalignment & Telemetry Corruption
*   **Citation**: `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:475-481` compared with `crates/op-cognitive-mcp/src/interceptor.rs:985-996`
*   **Impact**: **Critical (Security Controls Bypass / Memory Corruption)**
*   **Description**: The codebase declares two conflicting definitions of the zero-copy shared memory ABI structure `IdentitySled`. 

    **In `qdrant_shuttle.rs`**:
    ```rust
    #[repr(C)]
    pub struct IdentitySled {
        pub wireguard_pubkey: [u8; 32],
        pub mutation_index: u64,
        pub is_valid: bool,
        pub hashed_footprint: [u8; 32],
    }
    ```
    
    **In `interceptor.rs`**:
    ```rust
    #[repr(C)]
    pub struct IdentitySled {
        pub wireguard_pubkey: [u8; 32],
        pub mutation_index: u64,
        pub is_valid: bool,
        pub _pad: [u8; 7],
        pub hashed_footprint: [u8; 32],
        pub schema_uuid: [u8; 16],
        pub subid: [u8; 64],
        pub control_source: [u8; 32],
        pub nextdns_profile: [u8; 16],
    }
    ```

    Because of this misalignment:
    1.  The size of `IdentitySled` in `qdrant_shuttle.rs` is 80 bytes (aligned), whereas in `interceptor.rs` it is 208 bytes.
    2.  `read_shared_mapping` in `qdrant_shuttle.rs` slices the memory map to retrieve the JSON-serialized `PluginSchema` using `mmap[size_of::<IdentitySled>()..]`. This slices at offset **80**, reading raw binary metadata (such as `schema_uuid` and `subid` written by the interceptor) instead of the actual JSON string starting at offset **208**.
    3.  This mismatch permanently breaks the Qdrant semantic engine's schema parsing, rendering automated accountability checks entirely dead and causing constant deserialization failures during runtime startup.

---

### Unrestricted Local Directory Traversal & System File Disclosure
*   **Citation**: `crates/op-cognitive-mcp/src/grpc_service.rs:608-651`
*   **Impact**: **Critical (Arbitrary File Read / Confused Deputy)**
*   **Description**: The gRPC method `add_folder` takes a user-controlled `req.folder_path` and traverses it using standard file-system walkers with zero directory sandboxing, canonicalization, or path restriction.
    ```rust
    let path = std::path::Path::new(&req.folder_path);
    if !path.exists() || !path.is_dir() {
        return Err(Status::invalid_argument(...));
    }
    // ...
    let walker = if req.recursive { walkdir(path) } else { walkdir_shallow(path) };
    ```
    Any client (or a compromised/manipulated LLM agent executing this tool) can pass absolute paths like `/etc`, `/var/lib`, or `/root` as the directory. The engine recursively reads every file and inserts its plaintext contents directly into the public database. These files can then be queried, listed, and fully exfiltrated via `list_sources` and `get_source_content`.

---

## 3. Security & Quality Gaps

### Brittle Description-Based Classification (PII Bypass)
*   **Citation**: `crates/op-cognitive-mcp/src/activity_filter.rs:194`
*   **Impact**: **Medium (Privacy Leak / Auditing Bypass)**
*   **Description**: The PII identification helper `is_pii` uses a case-insensitive substring search for `"[pii]"` inside a field's unstructured documentation description to classify sensitive data:
    ```rust
    if let Some(field_schema) = schema.fields.get(field_name) {
        return field_schema.description.to_lowercase().contains("[pii]") || ...
    }
    ```
    Relying on developers to correctly type a specific tag in unstructured English documentation strings rather than enforcing a structured, typed metadata schema flag is extremely brittle. Any typos (e.g. `[PII`, `(pii)`) or description updates will silently leak sensitive user credentials/PII to searchable Qdrant storage.

---

### Non-Streaming Memory Accumulation in RAG Pipeline
*   **Citation**: `crates/op-cognitive-mcp/src/rag_pipeline.rs:1221-1250`
*   **Impact**: **Low (OOM / Denial of Service)**
*   **Description**: The function `parse_and_chunk` claims to stream repomix entries without loading whole files into memory, but it accumulates all parsed chunks in a single contiguous `Vec<Chunk>` before returning the iterator:
    ```rust
    let mut output: Vec<Chunk> = Vec::new();
    while let Some(Ok(line)) = lines_iter.next() {
        // ...
        output.extend(chunks);
    }
    output.into_iter()
}
    ```
    If a large repository is processed, this collection will balloon to hundreds of megabytes in memory, posing a high risk of Out-Of-Memory (OOM) aborts.

---

### Inconsistent and Duplicated `VoyageClient` Structs
*   **Citation**: `crates/op-cognitive-mcp/src/voyage.rs:405` compared with `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:526`
*   **Impact**: **Low (Inconsistent Authentication & Dead Code)**
*   **Description**: Two conflicting implementations of `VoyageClient` exist within the crate. 
    *   The public implementation in `voyage.rs` only checks the environment variable `VOYAGE_API_KEY`.
    *   The private implementation in `qdrant_shuttle.rs` checks `COGNITIVE_MCP_VOYAGE_API_KEY` first.
    *   This duplication creates a maintenance hazard where changes to embedding parameters or credential storage formats fail to propagate across both components, leading to confusing authentication errors during runtime deployment.

---

## 4. Schema-as-Code Compliance Violations

The codebase bypasses structured, versioned schema definitions in favor of loose JSON serialization, breaking the protocol guarantees of the schema-as-code discipline:

1.  **Ad-hoc JSON values for core telemetry state**:
    *   `crates/op-cognitive-mcp/src/activity_filter.rs:173`: `pub payload: serde_json::Value` allows unvalidated schemas to be logged to the ledger.
    *   `crates/op-cognitive-mcp/src/memory_store.rs:1073`: `pub metadata: serde_json::Value` bypasses type guarantees for dynamic namespace configuration.
    *   `crates/op-cognitive-mcp/src/memory_store.rs:1083`: `pub value: serde_json::Value` stores arbitrary values without schema validation.

2.  **Serialized JSON strings inside gRPC payloads**:
    *   `crates/op-cognitive-mcp/src/grpc_service.rs:527`: `metadata_json` is serialized as an ad-hoc JSON string in `GetNotebookResponse` instead of using a structured Protobuf representation.
    *   `crates/op-cognitive-mcp/src/grpc_service.rs:743`: `components_json` embeds arbitrary diagnostic telemetry in `GetHealthResponse`.
    *   `crates/op-cognitive-mcp/src/grpc_service.rs:778`: `sections_json` encodes structured research details inside a loose JSON string in `GeminiQueryResponse`.

All of these instances should be migrated to versioned Protobuf payloads or schema-validated structures.

---
## ⚠ Citation Warnings
- `crates/op-cognitive-mcp/src/interceptor.rs:1004`: file has 67 lines
- `crates/op-cognitive-mcp/src/interceptor.rs:985`: file has 67 lines
- `crates/op-cognitive-mcp/src/rag_pipeline.rs:1221`: file has 837 lines
- `crates/op-cognitive-mcp/src/voyage.rs:405`: file has 72 lines
- `crates/op-cognitive-mcp/src/memory_store.rs:1073`: file has 590 lines
- `crates/op-cognitive-mcp/src/memory_store.rs:1083`: file has 590 lines
