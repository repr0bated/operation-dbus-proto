# Production Security and Quality Audit: `op-cognitive-mcp`

---

## 1. Executive Summary

This document presents a comprehensive production security and quality audit of the `op-cognitive-mcp` crate. The codebase implements a specialized Model Context Protocol (MCP) server that provides a gRPC and D-Bus interface for cognitive memory, session tracking, quota management, and a browser bridge fallback. 

While the architectural design is highly robust (featuring pure-Rust safe file system handling, exponential backoff retries, and strict token budgets), multiple **Critical** security and ABI issues were identified:
* **Out-of-Bounds Memory Read** in the gRPC interceptor, which can cause segmentation faults (Denial of Service) or arbitrary memory exposure.
* **Severe ABI Layout Mismatch** of the `IdentitySled` shared-memory structure between the interceptor and the Qdrant semantic shuttle, resulting in corrupt data offsets.
* **Cryptographic & Operational Hardening Deficiencies**, including world-readable credential checks that fail to block execution, and API key exposure in HTTP query strings.
* **Schema-As-Code Violations**, where core control plane structures rely on ad-hoc, unstructured JSON blobs.

---

## 2. Docs Audit & Standard Checklists

### 2.1 Crate-Level Documentation
* **Crate-level `//!` docs**: **Present** in `crates/op-cognitive-mcp/src/lib.rs:1-13`. It correctly documents the server's purpose, key module structure, and requirements trace (R1-R16).

### 2.2 Pub Items Sample (10 Items)
The following is a sample of 10 public items analyzed for proper `/// rustdoc` documentation:

1. `ActivityFilter` (`crates/op-cognitive-mcp/src/activity_filter.rs:208`)
   * **Result**: **FAIL**. Struct lacks a direct `///` doc comment.
2. `CognitiveToolRegistry` (`crates/op-cognitive-mcp/src/cognitive_tools.rs:16`)
   * **Result**: **FAIL**. Struct lacks a direct `///` doc comment.
3. `MemoryTool` (`crates/op-cognitive-mcp/src/cognitive_tools.rs:29`)
   * **Result**: **FAIL**. Struct lacks a direct `///` doc comment.
4. `register_notebooklm_tools` (`crates/op-cognitive-mcp/src/notebooklm.rs:109`)
   * **Result**: **FAIL**. Public function lacks a `///` doc comment.
5. `QdrantSemanticShuttle` (`crates/op-cognitive-mcp/src/qdrant_shuttle.rs:41`)
   * **Result**: **FAIL**. Struct lacks a direct `///` doc comment.
6. `ToolProfile` (`crates/op-cognitive-mcp/src/tool_profiles.rs:9`)
   * **Result**: **FAIL**. Public enum lacks a `///` doc comment.
7. `DiagnosticReport` (`crates/op-cognitive-mcp/src/doctor.rs:14`)
   * **Result**: **FAIL**. Public struct lacks a `///` doc comment.
8. `ComponentStatus` (`crates/op-cognitive-mcp/src/doctor.rs:22`)
   * **Result**: **FAIL**. Public struct lacks a `///` doc comment.
9. `ghostbridge_interceptor` (`crates/op-cognitive-mcp/src/interceptor.rs:17`)
   * **Result**: **FAIL**. Public interceptor function lacks a `///` doc comment.
10. `NamespaceKind` (`crates/op-cognitive-mcp/src/memory_store.rs:20`)
    * **Result**: **FAIL**. Public enum lacks a `///` doc comment.

### 2.3 README.md Presence
* **Result**: **Absent**. No `README.md` is present in the `op-cognitive-mcp` crate root listed in the FILES section.

### 2.4 Public Unsafe Functions
* **Result**: **None**. There are no `pub unsafe fn` declarations in the provided source files. Unsafe code blocks are utilized internally (e.g., memory mapping), but they are wrapped in safe public functions.

---

## 3. Critical Security Vulnerabilities

### Out-of-Bounds Memory Read & Undefined Behavior in gRPC Interceptor
* **Citation**: `crates/op-cognitive-mcp/src/interceptor.rs:25-36`
* **Impact**: **Critical (Directly Exploitable)**
* **Description**:
  The `ghostbridge_interceptor` opens and memory-maps the active shared-memory identity sled file (`/dev/shm/plugin_schema.dat`):
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
  let current_footprint = unsafe { (*sled_ptr).hashed_footprint };
  ```
  There is **no verification** of `mmap.len()` against `size_of::<IdentitySled>()` before casting and dereferencing the pointer. If `/dev/shm/plugin_schema.dat` is empty (0 bytes) or truncated (less than 208 bytes) due to a crash, restart, or raw system state, dereferencing `sled_ptr` will perform an out-of-bounds memory read. 
  
  Because this interceptor runs on **every incoming gRPC request**, an attacker capable of triggering or timing a truncation of the shared-memory file can instantly crash the Cognitive MCP server with a segmentation fault (SIGSEGV), resulting in a complete Denial of Service (DoS) of the system control plane. Furthermore, direct dereferencing of raw pointers cast from a `*const u8` (without using `std::ptr::read_unaligned`) is Undefined Behavior in Rust if the mapped address is not 8-byte aligned.
* **Remediation**:
  Enforce a strict length check before casting the pointer and use `std::ptr::read_unaligned` to avoid alignment issues:
  ```rust
  if mmap.len() < std::mem::size_of::<IdentitySled>() {
      return Err(Status::failed_precondition("Identity sled file truncated or invalid size."));
  }
  let sled = unsafe { std::ptr::read_unaligned(mmap.as_ptr().cast::<IdentitySled>()) };
  let is_valid = sled.is_valid;
  let current_footprint = sled.hashed_footprint;
  ```

---

### Severe IdentitySled ABI Mismatch / Structural Inconsistency
* **Citations**: 
  * `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:21-30`
  * `crates/op-cognitive-mcp/src/interceptor.rs:5-16`
* **Impact**: **Critical (Data Corruption & Security Bypass)**
* **Description**:
  The `IdentitySled` structure represents a C-style layout mapping directly to the `SchemaEngine` shared memory. However, two radically different definitions of this structure exist within the same crate:

  **Definition A (`qdrant_shuttle.rs`):**
  ```rust
  #[repr(C)]
  pub struct IdentitySled {
      pub wireguard_pubkey: [u8; 32],
      pub mutation_index: u64,
      pub is_valid: bool,
      pub hashed_footprint: [u8; 32],
  }
  ```

  **Definition B (`interceptor.rs`):**
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

  This ABI mismatch has devastating consequences:
  1. **Offset Incoherency**: In `interceptor.rs`, `hashed_footprint` begins at offset `48` (due to the explicit `_pad: [u8; 7]`). In `qdrant_shuttle.rs`, there is no padding; the compiler will pack or pad the struct differently, placing `hashed_footprint` at offset `41` (no alignment constraint on `[u8; 32]`). This means the interceptor and the accountability loop read and compare different segments of shared memory for the footprint, leading to continuous authentication failures or accidental bypasses.
  2. **Zero-Copy Serialization Boundary Drift**: `qdrant_shuttle.rs:341` reads `PluginSchema` JSON bytes by slicing the memory map from `size_of::<IdentitySled>()..`. In `qdrant_shuttle.rs`, the size of `IdentitySled` is `73` (or `80` aligned). In `interceptor.rs`, the size is `208`. If the `SchemaEngine` writes using the 208-byte struct, `qdrant_shuttle.rs` will read starting at offset 80, ingest raw UUID/subid bytes as if they were part of the JSON string, and fail to parse, rendering the semantic indexing completely non-functional.
* **Remediation**:
  Consolidate `IdentitySled` into a single canonical module (or import it from a shared, version-controlled library). Never duplicate `#[repr(C)]` memory layouts.

---

## 4. Medium Security & Quality Findings

### Lack of Hard Enforcement of Credential File Permissions (R13 Violation)
* **Citation**: `crates/op-cognitive-mcp/src/grpc_service.rs:770-781`
* **Impact**: **Medium**
* **Description**:
  Requirement **R13** dictates that credentials must be strictly stored and restricted to `0o600` permissions (owner read/write only) to prevent local privilege escalation or unauthorized local credential extraction. However, the `setup_auth` gRPC implementation merely prints a log warning rather than rejecting the configuration:
  ```rust
  if mode & 0o077 != 0 {
      warn!(
          path = %req.credential,
          mode = format!("{:o}", mode),
          "Chrome profile has overly permissive permissions; should be 0o600"
      );
  }
  ```
  Because this check does not fail-closed, the system will happily boot and execute using world-readable profile/credential files, exposing secrets to any local unprivileged user on the shared system.
* **Remediation**:
  Enforce strict access control. Fail-closed by returning an error status if permissions are too permissive:
  ```rust
  if mode & 0o077 != 0 {
      return Err(Status::permission_denied(format!(
          "Insecure credentials file permissions ({:o}). Permissions must be strictly 0o600.",
          mode
      )));
  }
  ```

---

### Insecure API Key Exposure in Google Gemini Query Parameters
* **Citation**: `crates/op-cognitive-mcp/src/gemini_fallback.rs:364-367`
* **Impact**: **Medium**
* **Description**:
  The Gemini API fallback client construct constructs the target HTTP URL with the API key appended directly in the query parameters:
  ```rust
  let url = format!(
      "{}/models/{}:generateContent?key={}",
      config.api_url, config.model, config.api_key
  );
  ```
  Query parameters are routinely captured and logged in cleartext by reverse proxies, API gateways, load balancers, and local diagnostic clients. Exposing the API key in the URL query string increases the risk of credential leakage.
* **Remediation**:
  Pass the API key using Google's supported standard HTTP header `x-goog-api-key` instead of query parameters:
  ```rust
  let url = format!(
      "{}/models/{}:generateContent",
      config.api_url, config.model
  );
  let response = self
      .client
      .post(&url)
      .header("x-goog-api-key", &config.api_key)
      .json(request)
      .send()
      .await;
  ```

---

### Code Duplication: Multiple Independent Voyage Clients
* **Citations**:
  * `crates/op-cognitive-mcp/src/voyage.rs:8-12`
  * `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:202-208`
* **Impact**: **Low (Code Quality)**
* **Description**:
  The codebase defines a public `VoyageClient` in `voyage.rs` with `client`, `api_key`, and `model` fields. However, `qdrant_shuttle.rs` implements a completely separate, private, duplicated `VoyageClient` struct with different fields (`api_url`, `output_dimension`). This leads to severe API inconsistency and double maintenance overhead when modifying embedding client behavior.
* **Remediation**:
  Delete the duplicate struct in `qdrant_shuttle.rs` and utilize the canonical `VoyageClient` in `voyage.rs`, extending it to accept configurable options if required.

---

## 5. Schema-As-Code Violations

The codebase enforces a schema-as-code discipline using Protocol Buffers and OSCAL, yet multiple critical interfaces fallback to ad-hoc, raw, or weakly typed JSON values and strings:

### 5.1 Ad-Hoc Diagnostic Payloads
* **Citation**: `crates/op-cognitive-mcp/src/doctor.rs:22-26`
* **Violation**: `ComponentStatus` uses an untyped, arbitrary `serde_json::Value` named `details`. Diagnostic reports are critical for health monitors and automated orchestrators; their structures should be explicitly governed by a versioned schema to prevent upstream decoding panic.

### 5.2 Untyped Telemetry Event Payloads
* **Citation**: `crates/op-cognitive-mcp/src/activity_filter.rs:197`
* **Violation**: `ActivityEvent::payload` is typed as a raw `serde_json::Value`. Because these events are stored in the vector database and eventually emitted to the snowball audit trail, they must be represented as structured, schema-validated elements (such as versioned Protocol Buffer messages).

### 5.3 Weakly Typed Memory Store Values
* **Citation**: `crates/op-cognitive-mcp/src/memory_store.rs:53`, `crates/op-cognitive-mcp/src/memory_store.rs:64`
* **Violation**: `MemoryNamespace::metadata` and `MemoryEntry::value` are stored as raw `serde_json::Value` objects. Since this state is shared globally across DBus, agent runtimes, and local workflows, persisting control plane parameters as unstructured blobs poses an immediate risk of system state corruption.

### 5.4 String-Embedded JSON in gRPC Responses
* **Citation**: `crates/op-cognitive-mcp/src/grpc_service.rs:728`, `crates/op-cognitive-mcp/src/grpc_service.rs:796`
* **Violation**: `GetHealthResponse` and `DoctorResponse` transmit system diagnostic states as raw JSON strings inside the protobuf field `components_json: String`. This bypasses Protocol Buffers serialization entirely, requiring callers to perform ad-hoc JSON parsing on top of the gRPC layer. These diagnostics should be defined as formal, nested gRPC fields.