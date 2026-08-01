# Production Security & Quality Audit: op-cognitive-mcp

---

## 1. Executive Summary

This production-grade security and quality audit evaluates the `op-cognitive-mcp` crate. The crate implements a cognitive Model Context Protocol (MCP) server that handles memory namespaces, a NotebookLM sidecar bridge, gRPC service layers, and D-Bus interfaces. 

While the system design is highly modern, integrating both vector retrieval (Qdrant) and relational-graph-vector logic (CozoDB/Sled), several critical security and stability issues have been identified:
1. **Critical Out-of-Bounds Memory Read & UB**: The gRPC interceptor casts a memory-mapped file to a struct pointer and dereferences it without verifying whether the mapped buffer is of sufficient size, and without ensuring pointer alignment.
2. **Critical SIMD Memory Safety Violations**: The crate parses JSON using `simd_json` on standard Rust vectors that lack the necessary end padding (`simd_json::PADDING_SIZE`), which can trigger segmentation faults or out-of-bounds heap reads.
3. **Severe ABI Layout Mismatch**: Two distinct implementations of the `IdentitySled` memory layout are used to access the same shared-memory file (`/dev/shm/plugin_schema.dat`). The mismatch shifts field offsets (specifically `hashed_footprint`), corrupting temporal hash validation.

---

## 2. Critical Security Findings

### 2.1. Out-of-Bounds Memory Read & Direct Pointer Cast UB
* **File & Line**: `crates/op-cognitive-mcp/src/interceptor.rs:31-36`
* **Impact**: Critical (Exploitable Denial of Service / Undefined Behavior)
* **Description**: 
  The gRPC interceptor maps `/dev/shm/plugin_schema.dat` to validate the client's Ghostbridge identity sled:
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
  This implementation contains two severe memory safety violations:
  1. **No Length Check**: The code dereferences `sled_ptr` without checking whether `mmap.len()` is at least `size_of::<IdentitySled>()` (which is 208 bytes). If another local process or a failed write truncates `/dev/shm/plugin_schema.dat` to a size smaller than 208 bytes (e.g., 0 bytes), dereferencing `sled_ptr` reads out-of-bounds memory, leading to an immediate segmentation fault and crashing the gRPC server (Denial of Service).
  2. **Alignment Violation**: Casting a raw byte pointer `*const u8` directly to `*const IdentitySled` violates Rust's alignment requirements. The `IdentitySled` struct contains `u64` fields, which require 8-byte alignment on most architectures. Direct dereferencing of an unaligned pointer is undefined behavior.
* **Remediation**: 
  Validate the file size before mapping, and use `std::ptr::read_unaligned` to load the struct safely without assuming correct alignment:
  ```rust
  if mmap.len() < std::mem::size_of::<IdentitySled>() {
      return Err(Status::failed_precondition("Sled size mismatch."));
  }
  let sled = unsafe { std::ptr::read_unaligned(mmap.as_ptr() as *const IdentitySled) };
  let is_valid = sled.is_valid;
  ```

### 2.2. Unpadded Buffers Passed to `simd_json` (Memory Safety Violations)
* **File & Line**: 
  * `crates/op-cognitive-mcp/src/dbus_interface.rs:50-53`
  * `crates/op-cognitive-mcp/src/cognitive_tools.rs:256-260`
* **Impact**: Critical (Exploitable Memory Safety / Crash / Leak)
* **Description**: 
  `simd_json` relies on the parsed buffer having a trailing padding of `simd_json::PADDING_SIZE` bytes to perform vector registers operations safely. If the buffer is not padded, the parser will perform out-of-bounds reads.
  In `dbus_interface.rs`:
  ```rust
  fn parse_simd(s: &str) -> Result<simd_json::OwnedValue, String> {
      let mut buf = s.as_bytes().to_vec();
      simd_json::from_slice(&mut buf).map_err(|e| e.to_string())
  }
  ```
  `s.as_bytes().to_vec()` creates a vector allocated with the exact size of the input string, offering no padding. Since `parse_simd` parses arbitrary arguments supplied over the D-Bus system bus, a remote or local caller can pass custom payloads to trigger segmentation faults or out-of-bounds heap reads.
  The same issue is present in `cognitive_tools.rs` within `serde_to_simd_json`:
  ```rust
  fn serde_to_simd_json(v: serde_json::Value) -> Value {
      let s = serde_json::to_string(&v).unwrap_or_default();
      let mut buf = s.into_bytes();
      simd_json::from_slice(&mut buf).unwrap_or(Value::Static(simd_json::StaticNode::Null))
  }
  ```
* **Remediation**: 
  Always use `simd_json::to_padded_container` or allocate additional padding bytes manually before parsing:
  ```rust
  let mut buf = s.as_bytes().to_vec();
  buf.resize(buf.len() + simd_json::PADDING_SIZE, 0);
  simd_json::from_slice(&mut buf)
  ```

---

## 3. High Security & System Integrity Findings

### 3.1. Sled Shared-Memory Race Conditions & TOCTOU
* **File & Line**: `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:315-325`
* **Impact**: High (Temporal Identity Mismatch / State Corruption)
* **Description**: 
  The function `read_shared_mapping` accesses the shared memory file `/dev/shm/plugin_schema.dat` which resides in the globally writable `/dev/shm` tmpfs. 
  Because this file is opened and mapped concurrently without filesystem locking (`flock`) or atomic memory state operations, a race condition exists. A local unprivileged attacker could modify or rewrite `/dev/shm/plugin_schema.dat` between the time the client reads the footprint and the time the interceptor validates it.
* **Remediation**: 
  Protect the shared memory file with exclusive permissions (`0o600`), and enforce shared-read/write file locks using the `fs2` crate when mapping state.

### 3.2. Unchecked Zip Archive Extraction (Zip Bomb Denial of Service)
* **File & Line**: `crates/op-cognitive-mcp/src/rag_pipeline.rs:232-237`
* **Impact**: High (Denial of Service via Resource Exhaustion)
* **Description**: 
  The ingestion pipeline extracts repomix source files from a ZIP archive:
  ```rust
  let file = std::fs::File::open(zip_path)?;
  let mut archive = zip::ZipArchive::new(file)?;
  ...
  let entry = archive.by_index(entry_idx)?;
  let reader = BufReader::new(entry);
  ```
  The pipeline reads and parses the target archive entries without validating the compressed vs. uncompressed size ratios or restricting maximum memory consumption. An attacker could supply a constructed "Zip Bomb" (a highly compressed archive of massive uncompressed size) causing the host machine to run out of memory (OOM), leading to a kernel-level process termination of the control plane.
* **Remediation**: 
  Implement explicit limits on decompressed entry sizes:
  ```rust
  let size_limit = 50 * 1024 * 1024; // 50 MB limit
  if entry.size() > size_limit {
      anyhow::bail!("Decompressed ZIP size exceeds safety limit.");
  }
  ```

---

## 4. Medium Quality & Compatibility Mismatch Findings

### 4.1. Struct Layout Mismatch on `IdentitySled` (Severe ABI Corruption)
* **File & Line**: 
  * `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:26-33`
  * `crates/op-cognitive-mcp/src/interceptor.rs:5-17`
* **Impact**: Medium (Authentication Failure & Logic Errors)
* **Description**: 
  The structure of the Ghostbridge `IdentitySled` differs severely between files:
  
  **`qdrant_shuttle.rs` layout:**
  ```rust
  #[repr(C)]
  pub struct IdentitySled {
      pub wireguard_pubkey: [u8; 32], // Offset 0
      pub mutation_index: u64,        // Offset 32
      pub is_valid: bool,             // Offset 40
      pub hashed_footprint: [u8; 32], // Offset 41
  }
  ```
  **`interceptor.rs` layout:**
  ```rust
  #[repr(C)]
  pub struct IdentitySled {
      pub wireguard_pubkey: [u8; 32], // Offset 0
      pub mutation_index: u64,        // Offset 32
      pub is_valid: bool,             // Offset 40
      pub _pad: [u8; 7],              // Offset 41
      pub hashed_footprint: [u8; 32], // Offset 48
      pub schema_uuid: [u8; 16],
      ...
  }
  ```
  In `qdrant_shuttle.rs`, the field `hashed_footprint` starts at offset **41**. In `interceptor.rs`, because of the manual `_pad: [u8; 7]` field, it starts at offset **48**. This mismatch shifts the fields in memory, meaning that any footprint written by the Qdrant Semantic Shuttle will be read at the wrong offset by the Ghostbridge interceptor, resulting in a persistent `Temporal Hash Mismatch` and broken gRPC authentication.
* **Remediation**: 
  Consolidate `IdentitySled` into a single shared definition in `op-core` or `op-state-store`, and reference it across all crates to guarantee binary layout parity.

### 4.2. Local Path Exposure & Directory Oracle
* **File & Line**: `crates/op-cognitive-mcp/src/grpc_service.rs:396-403`
* **Impact**: Medium (Information Disclosure)
* **Description**: 
  The gRPC `add_folder` method permits checking if arbitrary directories exist on the host file system:
  ```rust
  let path = std::path::Path::new(&req.folder_path);
  if !path.exists() || !path.is_dir() {
      return Err(Status::invalid_argument(format!(
          "Folder '{}' does not exist or is not a directory",
          req.folder_path
      )));
  }
  ```
  By submitting varied paths to this endpoint, an attacker with access to the gRPC service can map the filesystem directory structure of the host, identifying installed applications, mounted drives, or specific user accounts.
* **Remediation**: 
  Do not return detailed existence error messages to the client. Limit filesystem access to a sandboxed directory root (e.g., `/var/lib/op-cognitive-mcp/allowed_sources`).

---

## 5. Performance, Allocation & Memory Map Audit

### 5.1. Hot-Path Allocations
* **Un-allocated Vectors in Loops**:
  * `crates/op-cognitive-mcp/src/grpc_service.rs:527`: In `walkdir`, `let mut result = Vec::new()` is instantiated recursively across all directories without pre-allocating capacity. This leads to heavy allocation fragmentation.
  * `crates/op-cognitive-mcp/src/rag_pipeline.rs:394-397`: Loops in `extract_rust` construct `symbols`, `doc_comments`, `imports`, and `pending_doc` vectors dynamically for each file parsed without reserving bounds.
* **`format!()` overhead**:
  * `crates/op-cognitive-mcp/src/rag_pipeline.rs:508`: `format!("{repo}:{file_path}:{chunk_index}:{content}")` is executed for every code block chunk to calculate the SHA256. This should be refactored to stream the bytes directly into the SHA256 context using `Hasher::update`.

### 5.2. Memory Map Table

| Site | file:line | Type | Size | Risk |
|---|---|---|---|---|
| `QdrantSemanticShuttle` | `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:318` | Read-Only | `>= 73` bytes | TOCTOU / symlink attacks if the sled path is manipulated. |
| `Ghostbridge Interceptor` | `crates/op-cognitive-mcp/src/interceptor.rs:31` | Read-Only | `>= 208` bytes | **CRITICAL**: Undefined behavior and segmentation faults if `/dev/shm/plugin_schema.dat` is truncated or corrupted. |
| Cozo Sled Backend | `crates/op-cognitive-mcp/src/server.rs:38` | Sled (Internal mmap) | Managed by Sled | Database corruption if the DB is written to highly volatile mounts (e.g., `tmpfs` under memory pressure). |

---

## 6. Schema-as-Code Compliance Review

The codebase currently violates the schema-as-code discipline in multiple places by representing structured data contracts as arbitrary strings, ad-hoc JSON elements, or manually built structs:

1. **Ad-hoc Input Schemas**:
   * **File & Line**: `crates/op-cognitive-mcp/src/cognitive_tools.rs:104-142`
   * **Violation**: The input parameters for cognitive memory storage and deletion are expressed as inline JSON structures (`json!({...})`) rather than being loaded from versioned protobuf schemas.
2. **Untyped Payload Fields**:
   * **File & Line**: `crates/op-cognitive-mcp/src/activity_filter.rs:132`
   * **Violation**: `ActivityEvent` defines `payload` as `serde_json::Value`, allowing arbitrary unvalidated JSON objects to flow through the filter.
3. **Ad-hoc Relational Memory Values**:
   * **File & Line**: `crates/op-cognitive-mcp/src/memory_store.rs:82`
   * **Violation**: `MemoryEntry` uses `serde_json::Value` for its value field. Data validation is completely absent at the database layer.
4. **Dynamic Metadata Maps**:
   * **File & Line**: `crates/op-cognitive-mcp/src/rag_pipeline.rs:253-270`
   * **Violation**: Hover metadata fields such as `symbols`, `imports`, and `tags` are packed on the fly into an ad-hoc JSON structure before insertion into Qdrant, rather than conforming to a strict schema definitions contract.

**Remediation**: 
Replace untyped `serde_json::Value` properties with formal Protobuf messages, compile them using `prost-build`, and perform schema checks using the generated message definitions.