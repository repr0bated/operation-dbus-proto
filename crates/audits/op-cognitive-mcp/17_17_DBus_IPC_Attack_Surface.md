# Production Security and Quality Audit: `op-cognitive-mcp`

---

## 1. D-Bus & IPC Attack Surface Analysis

The D-Bus transport layer in this service presents a highly permissive, unauthenticated IPC attack surface registered directly on the Linux **system bus**.

### Registered D-Bus Metadata
* **Service Name:** `org.opdbus.CognitiveMcp` (registered in `crates/op-cognitive-mcp/src/server.rs:258`)
* **Object Path:** `/org/opdbus/v1/cognitive` (registered in `crates/op-cognitive-mcp/src/server.rs:261`)
* **Interface:** `org.opdbus.CognitiveMcpV1` (declared in `crates/op-cognitive-mcp/src/dbus_interface.rs:24`)

### Exposed Methods (`crates/op-cognitive-mcp/src/dbus_interface.rs:27-51`)
1. **`ListTools() -> s`**
   * **Description:** Lists all registered tools in the workspace.
   * **Identity Verification:** None.
2. **`GetToolSchema(s name) -> s`**
   * **Description:** Retrieves the JSON input schema for a given tool.
   * **Identity Verification:** None.
3. **`CallTool(s name, s args_json) -> s`**
   * **Description:** Executes a selected tool with the provided arguments.
   * **Identity Verification:** None.

### Critical IPC Safety Flaws

* **Complete Lack of Authentication and Caller Verification:**
  The implementation of `CognitiveMcpInterface` in `crates/op-cognitive-mcp/src/dbus_interface.rs:26-51` does not perform any authorization check or caller UID verification. Since the service connects to the **system bus** (`crates/op-cognitive-mcp/src/server.rs:257` via `zbus::Connection::system()`), any local unprivileged process can connect to the system bus and invoke `CallTool`.
* **State Mutation & Sidecar Interactivity:**
  Unauthenticated callers can execute the `cognitive_memory` tool (managed by `MemoryTool` in `crates/op-cognitive-mcp/src/cognitive_tools.rs:29`) with the `store` or `delete` operations, allowing them to overwrite or wipe the CozoDB-backed cognitive memory stores. Additionally, if the NotebookLM bridge is active, invoking a NotebookLM tool via `CallTool` can cause the server to communicate over stdio with external npm-based subprocesses (`crates/op-cognitive-mcp/src/notebooklm.rs:59`), creating a denial of service (DoS) vector through resource exhaustion.
* **Deserialization of Untrusted Input:**
  In `crates/op-cognitive-mcp/src/dbus_interface.rs:43-47`, the method `CallTool` takes `args_json` as a raw string and immediately parses it into a `simd_json::OwnedValue` using `parse_simd`:
  ```rust
  let args = match parse_simd(&args_json) {
      Ok(v) => v,
      Err(e) => return err_json(&e),
  };
  ```
  The parsed unstructured JSON is then passed directly to the executor without schema validation, allowing malformed payload execution.

---

## 2. Critical Exploitable Vulnerabilities

### Vulnerability 1: Out-of-Bounds & Unaligned Read (Memory Safety Violation) in Ghostbridge Interceptor
* **Location:** `crates/op-cognitive-mcp/src/interceptor.rs:25-34`
* **Impact:** Critical (Denial of Service / Undefined Behavior)

#### Analysis
The gRPC interceptor `ghostbridge_interceptor` opens and memory-maps the shared-memory file `/dev/shm/plugin_schema.dat`:
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

There is **zero size validation** on `mmap` before casting `mmap.as_ptr()` to `*const IdentitySled` and dereferencing it. If `/dev/shm/plugin_schema.dat` is empty (0 bytes) or truncated to a size smaller than `size_of::<IdentitySled>()` (238 bytes), dereferencing `sled_ptr` results in an immediate out-of-bounds memory read, leading to a segmentation fault, panic, or undefined behavior. 

Furthermore, `mmap.as_ptr()` returns a pointer aligned to 1 byte, whereas `IdentitySled` contains `u64` fields, necessitating 8-byte alignment on many architectures. Directly dereferencing `(*sled_ptr)` instead of utilizing `std::ptr::read_unaligned` is undefined behavior in Rust.

#### Exploitability
Because `/dev/shm` is traditionally a world-writable directory (with the sticky bit set, permissions `1777` on Linux systems), a local attacker can pre-create `/dev/shm/plugin_schema.dat` as an empty file or truncate it while the server is running. Any incoming gRPC request passing through the interceptor will attempt to process the truncated memory map, causing the server to crash instantly.

---

### Vulnerability 2: Arbitrary File Ingestion & Information Disclosure
* **Location:** `crates/op-cognitive-mcp/src/grpc_service.rs:506-531`
* **Impact:** Critical (Information Disclosure / Privilege Escalation)

#### Analysis
The `add_folder` RPC method accepts an arbitrary, unvalidated path in the `folder_path` parameter:
```rust
let path = std::path::Path::new(&req.folder_path);
if !path.exists() || !path.is_dir() {
    return Err(Status::invalid_argument(format!(
        "Folder '{}' does not exist or is not a directory",
        req.folder_path
    )));
}
```

The server walks the directory recursively (or shallowly) using `walkdir` / `walkdir_shallow` (without a sandbox constraint or path traversal restrictions) and reads all matching file contents into memory:
```rust
for entry_path in walker {
    ...
    match std::fs::read_to_string(&entry_path) {
        Ok(content) => {
            ...
            match self
                .memory_store
                .store_entry(&namespace, &key, value, vec![], None)
                .await
```

These contents are stored as entries inside the cognitive store namespace corresponding to `notebook_id`.

#### Exploitability
An attacker with access to the gRPC service or the unauthenticated system D-Bus interface can call this method with `folder_path` set to sensitive directories (e.g., `/etc/`, private SSH key directories, or another user's home directory). Since the service runs with system-level privileges to interact with the system D-Bus, it has read permissions on sensitive files. The attacker can ingest these private files into the database and subsequently query their contents using the `AskQuestion` or `QueryNotebook` methods, bypassing system DAC/MAC file system restrictions.

---

## 3. Schema-As-Code Compliance Violations

The codebase frequently violates the schema-as-code discipline by utilizing ad-hoc structs, unstructured `serde_json::Value` instances, and raw JSON strings rather than defined Protocol Buffers or versioned schemas.

### Ad-hoc JSON Values as Data Contracts
* **`crates/op-cognitive-mcp/src/activity_filter.rs:191`**
  ```rust
  pub payload: serde_json::Value,
  ```
  The `ActivityEvent` struct stores its primary transactional data inside an unversioned, unstructured `Value` field instead of a protobuf or strongly-typed schema.
* **`crates/op-cognitive-mcp/src/memory_store.rs:69` & `71`**
  ```rust
  pub metadata: serde_json::Value,
  ```
  The namespace context configuration is typed as a generic `Value` blob in the database representation.
* **`crates/op-cognitive-mcp/src/memory_store.rs:81`**
  ```rust
  pub value: serde_json::Value,
  ```
  The `MemoryEntry` key-value store stores values as ad-hoc unstructured JSON.

### Inline/Ad-hoc JSON Schemas
* **`crates/op-cognitive-mcp/src/cognitive_tools.rs:62-97`**
  The input validation contract for the `cognitive_memory` tool is expressed as an inline `json!` macro block representing a JSON Schema, rather than being parsed from a versioned schema file.
* **`crates/op-cognitive-mcp/src/notebooklm.rs:166`**
  The input schema for the NotebookLM external tool bridge is represented as an ad-hoc, untyped `Value` field.

### Raw JSON Strings in API Contracts (JSON-in-Protobuf / JSON-in-D-Bus)
* **`crates/op-cognitive-mcp/src/grpc_service.rs:115`**
  ```rust
  let metadata_json = serde_json::to_string(&ns.metadata).unwrap_or_else(|_| "{}".to_string());
  ```
  Metadata is returned to the client as an ad-hoc serialized JSON string in `GetNotebookResponse` instead of structured protobuf fields.
* **`crates/op-cognitive-mcp/src/grpc_service.rs:698`**
  In `GetHealthResponse`, the system components health details are serialized as an ad-hoc JSON string (`components_json`).
* **`crates/op-cognitive-mcp/src/grpc_service.rs:818`**
  The `GeminiQueryResponse` returns sectional data as a raw string containing serialized JSON (`sections_json`).
* **`crates/op-cognitive-mcp/src/dbus_interface.rs:31`, `37`, and `43`**
  The D-Bus methods `ListTools`, `GetToolSchema`, and `CallTool` handle incoming arguments and return values using raw `String` fields containing unstructured JSON instead of using structured D-Bus types or versioned serialization models.

---

## 4. Code Quality & Security Robustness Findings

### Finding 1: Insecure Credential File Permission Enforcement
* **Location:** `crates/op-cognitive-mcp/src/grpc_service.rs:756-773`
* **Severity:** Medium

#### Description
In the `setup_auth` method, when a Chrome profile path is registered, the system attempts to check for safe permissions (`0o600` on Unix systems). However, if the file is found to have overly permissive permissions (e.g., world-readable `0o777`), it only prints a warning log message:
```rust
if mode & 0o077 != 0 {
    warn!(
        path = %req.credential,
        mode = format!("{:o}", mode),
        "Chrome profile has overly permissive permissions; should be 0o600"
    );
}
```
The registration is allowed to proceed successfully instead of rejecting the credentials. This allows administrators or installers to unknowingly deploy the service in an insecure configuration where other local users can steal authentication cookies.

### Finding 2: Unaligned Memory Pointer Dereferencing
* **Location:** `crates/op-cognitive-mcp/src/interceptor.rs:29-33`
* **Severity:** Low

#### Description
The casting of an arbitrary file pointer to `*const IdentitySled` does not guarantee that the base pointer from the mmap is aligned to the alignment requirements of the `IdentitySled` struct. While x86_64 processors handle unaligned reads transparently with a minor performance penalty, other architectures (like ARM/AArch64) can raise a bus error or experience significant performance degradation. The codebase should use `std::ptr::read_unaligned` consistently, as demonstrated in `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:353`.

---
## ⚠ Citation Warnings
- `crates/op-cognitive-mcp/src/server.rs:258`: file has 217 lines
- `crates/op-cognitive-mcp/src/server.rs:261`: file has 217 lines
- `crates/op-cognitive-mcp/src/server.rs:257`: file has 217 lines
