# Production Security and Quality Audit: `op-cognitive-mcp`

## Executive Summary

This audit evaluates the quality, schema discipline, and security posture of the `op-cognitive-mcp` crate. The codebase exhibits a hybrid architecture utilizing gRPC, D-Bus, and HTTP/SSE transports to coordinate local cognitive memory, vector search (Qdrant), and sidecar LLM integrations (Gemini, NotebookLM). 

While the system contains robust error retry mechanisms, multiple **Critical** security and memory-safety vulnerabilities exist in the shared-memory identity verification layer (`IdentitySled`) and the public D-Bus registration. Specifically:
1. **Divergent ABI structures** in memory-mapped shared structures lead to buffer over-reads and parser crashes.
2. **Missing file size validation** during `mmap` parsing allows local unprivileged users to crash the high-privileged gRPC daemon.
3. **Insecure authentication design** relying on world-readable files in `/dev/shm` allows unprivileged local users to bypass gRPC authorization entirely.
4. **Lack of authorization checks** on the system-bus D-Bus interface allows any local peer to execute arbitrary tool payloads.

Additionally, multiple violations of **Schema-as-Code** discipline have been identified where raw, unstructured JSON objects and ad-hoc structures are serialized over interfaces instead of versioned schemas.

---

## Section 1: Code Safety & Memory Audits

### 1.1 Unsafe Blocks and Safety Documentation

There are **six** raw `unsafe` blocks across the provided files. None of them contain a `// SAFETY:` comment explaining why the operation is safe, violating the Rust API guidelines and systems-programming best practices.

#### Unsafe Block 1 & 2
* **File & Line**: `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:368` and `380`
* **Context**:
  ```rust
  let mmap = unsafe { MmapOptions::new().map(&file) }
  ```
  ```rust
  let sled = unsafe { std::ptr::read_unaligned(sled_ptr) };
  ```
* **Flag**: Missing `// SAFETY:` explanation. The function assumes the mapped file size is stable and matches the layout of `IdentitySled` without handling concurrent local truncation.

#### Unsafe Block 3, 4, 5 & 6
* **File & Line**: `crates/op-cognitive-mcp/src/interceptor.rs:31`, `36`, `37`, and `52`
* **Context**:
  ```rust
  let mmap = unsafe {
      MmapOptions::new()
          .map(&file)
          .map_err(|_| Status::internal("Mmap failed"))?
  };
  ```
  ```rust
  let is_valid = unsafe { (*sled_ptr).is_valid };
  ```
  ```rust
  let current_footprint = unsafe { (*sled_ptr).hashed_footprint };
  ```
  ```rust
  let control_source = unsafe { &(*sled_ptr).control_source };
  ```
* **Flag**: Missing `// SAFETY:` explanation. Accessing fields of `IdentitySled` via a raw cast pointer from an unvalidated memory map introduces severe undefined behavior if the backing file `/dev/shm/plugin_schema.dat` is smaller than the struct footprint (see Section 2.1).

### 1.2 Command Invocations

A total of **0** explicit `std::process::Command::new()` calls are directly constructed in the provided source files. 

However, `crates/op-cognitive-mcp/src/notebooklm.rs` acts as an orchestration driver that configures external process execution through `ExternalMcpConfig` (lines 53-70). It pulls the executable command and arguments directly from environment variables `COGNITIVE_MCP_NOTEBOOKLM_COMMAND` (defaulting to `"npx"`) and `COGNITIVE_MCP_NOTEBOOKLM_ARGS` (defaulting to `&["-y", "notebooklm-mcp@latest"]`). 
* **Risk**: Although no forbidden shell commands (`sh`, `bash`, `ovs-*`, etc.) are hardcoded, executing arbitrary commands derived from untrusted environments or unvalidated configs without strict path pinning or binary white-listing poses a path hijacking/injection risk.

---

## Section 2: Critical Vulnerability Findings

### 2.1 Struct Layout Drift / ABI Mismatch (Memory Safety / Denial of Service)
* **File & Line**: `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:28-36` vs `crates/op-cognitive-mcp/src/interceptor.rs:5-17`
* **Impact**: **Critical (Directly Exploitable)**

The `IdentitySled` struct is defined twice with completely different layouts, field orderings, and byte sizes.

**Definition in `qdrant_shuttle.rs`:**
```rust
pub struct IdentitySled {
    pub wireguard_pubkey: [u8; 32],
    pub mutation_index: u64,
    pub is_valid: bool,
    pub hashed_footprint: [u8; 32],
}
```
* **Size**: 32 (pubkey) + 8 (mutation) + 1 (is_valid) + 7 (padding) + 32 (footprint) = **80 bytes**.

**Definition in `interceptor.rs`:**
```rust
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
* **Size**: 80 bytes (base) + 16 (uuid) + 64 (subid) + 32 (control_source) + 16 (nextdns) = **208 bytes**.

#### Exploit Scenario:
1. **Out-of-Bounds Memory Read / SIGSEGV**: If the shared-memory file `/dev/shm/plugin_schema.dat` is populated by an engine using the 80-byte layout (matching `qdrant_shuttle.rs`), its total file size may be smaller than 208 bytes. When `ghostbridge_interceptor` in `interceptor.rs` attempts to access `control_source` at line 52:
   ```rust
   let control_source = unsafe { &(*sled_ptr).control_source };
   ```
   This dereference accesses memory at offset 160. If the mapped memory length is less than 160 bytes, this triggers an immediate out-of-bounds pointer dereference, causing a segmentation fault (SIGSEGV) and crashing the gRPC service process (Denial of Service).
2. **Schema Corruption**: In `qdrant_shuttle.rs:382`, the code calculates the start of the `PluginSchema` JSON bytes as `mmap[size_of::<IdentitySled>()..]`. Because its `size_of` is 80 instead of 208, the shuttle begins parsing the JSON from offset 80, which actually contains binary structure fields (`schema_uuid`, `subid`, etc.). This causes `parse_plugin_schema` (line 391) to fail deterministically with a JSON parsing error, permanently breaking vector pipeline ingestion.

---

### 2.2 Unvalidated Mmap Length (Local Denial of Service)
* **File & Line**: `crates/op-cognitive-mcp/src/interceptor.rs:26-38`
* **Impact**: **Critical (Directly Exploitable)**

In `interceptor.rs`, the `ghostbridge_interceptor` opens and maps `/dev/shm/plugin_schema.dat`:
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
Unlike `qdrant_shuttle.rs:370`, there is **no check** to verify that `mmap.len() >= size_of::<IdentitySled>()`.

#### Exploit Scenario:
Because `/dev/shm` is typically a world-writable temporary filesystem on Linux, any local unprivileged user can execute a simple truncation command (e.g., `> /dev/shm/plugin_schema.dat`) to empty the file. When a gRPC request arrives, the `ghostbridge_interceptor` maps the 0-byte file. The pointer `sled_ptr` is invalid/null. The immediate read of `is_valid` (offset 40) causes a null-pointer dereference or segmentation fault, allowing any unprivileged local user to take down the entire gRPC platform.

---

### 2.3 Authentication Bypass via Shared Memory Information Leak
* **File & Line**: `crates/op-cognitive-mcp/src/interceptor.rs:20-50`
* **Impact**: **Critical (Directly Exploitable)**

The `ghostbridge_interceptor` authenticates incoming gRPC metadata based on whether the request contains a matching temporal hash:
```rust
let request_footprint = footprint_value
    .as_ref()
    .unwrap()
    .to_str()
    .map_err(|_| Status::invalid_argument("Invalid footprint encoding"))?;
let expected_footprint = hex::encode(current_footprint);

if request_footprint != expected_footprint {
    return Err(Status::permission_denied("Temporal Hash Mismatch."));
}
```
The secret token `current_footprint` is read directly from `(*sled_ptr).hashed_footprint` in `/dev/shm/plugin_schema.dat`.

#### Exploit Scenario:
Shared memory files in `/dev/shm/` are generally world-readable unless explicitly tightened. Any local unprivileged user can read `/dev/shm/plugin_schema.dat`, extract the `hashed_footprint` bytes and `trace_id` fields, and generate identical `x-ghostbridge-footprint` and `x-ghostbridge-trace-id` headers. They can then send requests to the local gRPC port (`127.0.0.1:50052`), bypassing the interceptor authentication entirely to call high-privilege gRPC methods.

---

## Section 3: High & Medium Quality & Security Findings

### 3.1 Unauthenticated System D-Bus Interface (High Severity)
* **File & Line**: `crates/op-cognitive-mcp/src/dbus_interface.rs:38-51` and `crates/op-cognitive-mcp/src/server.rs:218-232`
* **Impact**: **High**

The D-Bus transport is registered on the *System Bus* rather than the session bus:
```rust
let conn = zbus::Connection::system().await?;
conn.request_name("org.opdbus.CognitiveMcp").await?;
```
The method `call_tool(name, args_json)` on interface `org.opdbus.CognitiveMcpV1` executes arbitrary tools in the registry:
```rust
async fn call_tool(&self, name: String, args_json: String) -> String { ... }
```
Because no policy credentials, user IDs (UIDs), or `zbus` caller checks are performed inside `dbus_interface.rs`, **any unprivileged process on the host system** can send messages over the system D-Bus to call arbitrary cognitive tools. This allows unprivileged local attackers to:
* Erase cognitive databases by calling `MemoryTool` with `"delete"` or `"reset"` operations.
* Poison the retrieval-augmented generation (RAG) context by inserting raw malicious texts.
* Overuse the Gemini and Voyage APIs by repeatedly calling expensive search operations, creating resource/financial exhaustion.

---

### 3.2 Unrestricted Directory Traversal / File Ingestion in `add_folder` (High Severity)
* **File & Line**: `crates/op-cognitive-mcp/src/grpc_service.rs:651-670`
* **Impact**: **High**

The gRPC method `add_folder` allows recursively walking a path and loading files into the memory store:
```rust
let path = std::path::Path::new(&req.folder_path);
if !path.exists() || !path.is_dir() {
    return Err(Status::invalid_argument(...));
}
...
let walker = if req.recursive {
    walkdir(path)
} else {
    walkdir_shallow(path)
};
```
There is no path normalization, sandboxing, or confinement to verify that the target directory resides within an allowed workspace. A client with access to this endpoint (which includes any local user due to the `/dev/shm` leak in Section 2.3) can pass `folder_path = "/root"` or `folder_path = "/etc"`, prompting the daemon to ingest and expose sensitive configuration files or private keys.

---

### 3.3 Passive Warnings on Insecure Credentials (Medium Severity)
* **File & Line**: `crates/op-cognitive-mcp/src/grpc_service.rs:818-845`
* **Impact**: **Medium**

In `setup_auth`, if the user selects the `"chrome_profile"` auth method, the code checks the UNIX file permissions of the credentials path:
```rust
let mode = metadata.mode() & 0o777;
if mode & 0o077 != 0 {
    warn!(
        path = %req.credential,
        mode = format!("{:o}", mode),
        "Chrome profile has overly permissive permissions; should be 0o600"
    );
}
```
If the profile directory/file has world-readable or group-readable permissions (e.g. `0755` or `0644`), the daemon merely emits a passive log warning (`warn!`) and continues to run. It should actively restrict access or enforce `0o600` permissions by rejecting insecure configurations to protect sensitive cookies and browser sessions.

---

## Section 4: Schema-as-Code Violations

The codebase violates the Schema-as-Code discipline by defining critical interface contracts as ad-hoc, unstructured JSON objects or string values rather than protocol-versioned schemas.

### 4.1 Ad-Hoc JSON Structure Definitions (simd_json)
* **File & Line**: `crates/op-cognitive-mcp/src/cognitive_tools.rs:76-118` and `crates/op-cognitive-mcp/src/typed_tools.rs:107-123`
* **Description**: The input validation schemas for `MemoryTool` and `TypedQueryTool` are defined dynamically as runtime values using `simd_json::json!`. Changes to these properties are not checked at compile time, leading to silent interface breakages between local MCP agents and the server. These should be defined inside declarative JSON Schema or Protocol Buffer definitions.

### 4.2 Raw JSON Passing over D-Bus
* **File & Line**: `crates/op-cognitive-mcp/src/dbus_interface.rs:28-48`
* **Description**: `list_tools` and `call_tool` pass untyped arguments and return values as raw, escaped JSON strings (`String`). D-Bus naturally supports structured variant serialization (`a{sv}` or tuples). Resorting to raw JSON strings over D-Bus bypasses type safety and version compatibility validation.

### 4.3 Unstructured Diagnostic Report Payloads
* **File & Line**: `crates/op-cognitive-mcp/src/grpc_service.rs:799-803`
* **Description**: The `components_json` response in `get_health` is serialized dynamically from an ad-hoc, untyped `serde_json::Value`. Downstream callers must handle unstructured JSON strings instead of receiving a typed, versioned sub-message schema.

### 4.4 Hardcoded State Machine and Metadata Tags
* **File & Line**: `crates/op-cognitive-mcp/src/activity_filter.rs:65-90`
* **Description**: System policies filter metadata based on string-matching hardcoded literals like `"noise"`, `"overkill"`, and `"immutable"`. This logic is tightly coupled to string literals rather than a compiled, schema-driven enumeration.

---

## Corrective Actions & Remediation

1. **Unify `IdentitySled` Layout**: Define a single canonical `IdentitySled` struct within a shared crate (e.g., `op-core`), decorated with `#[repr(C)]` or managed via a versioned protocol layout (like Protobuf or FlatBuffers).
2. **Implement Safe Mmap Checking**:
   In `interceptor.rs`, enforce strict length checking before any pointer cast:
   ```rust
   ensure!(mmap.len() >= size_of::<IdentitySled>(), Status::failed_precondition("Malformed Identity Sled Size"));
   ```
3. **Secure the Temporal Footprint Storage**:
   * Stop writing cryptographic secrets to world-readable paths like `/dev/shm/plugin_schema.dat`.
   * If `/dev/shm` must be used, restrict file creation permissions to `0o600` so only the owner (the high-privilege service) can read the token.
4. **Enforce Policy Enforcement on D-Bus**:
   Integrate peer credential checks (e.g., calling `connection.peer_credentials()` to check the calling process's UID) to ensure only authorized system services can request tool execution on the System Bus.
5. **Implement Path Sanity in `add_folder`**: Use `canonicalize()` and verify that the target path is strictly nested within an authorized workspace root. Reject any paths containing directory traversal patterns (`..`).

---
## ⚠ Citation Warnings
- `crates/op-cognitive-mcp/src/server.rs:218`: file has 217 lines
