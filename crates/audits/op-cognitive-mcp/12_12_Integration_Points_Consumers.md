# Workspace Integration Overview

## Crates Depending on `op-cognitive-mcp`
Based on the workspace `Cargo.toml` and the package dependency declarations in `Cargo.lock`:
* **`op-dbus`** (Root workspace package): Declared in `Cargo.toml` dependencies block.
* **`op-grpc-bridge`**: Declared as depending on `op-cognitive-mcp` in the package registry listing in `Cargo.lock`.

---

## Registered D-Bus Configurations
The following D-Bus service names, object paths, and interfaces are registered by the server:
* **Service Name:** `org.opdbus.CognitiveMcp` (Registered in `crates/op-cognitive-mcp/src/server.rs:210` and `crates/op-cognitive-mcp/src/main.rs:94`)
* **Object Path:** `/org/opdbus/v1/cognitive` (Registered in `crates/op-cognitive-mcp/src/server.rs:214` and `crates/op-cognitive-mcp/src/main.rs:95`)
* **Interface Name:** `org.opdbus.CognitiveMcpV1` (Declared in `crates/op-cognitive-mcp/src/dbus_interface.rs:24`)

### Exposed D-Bus Methods:
* `ListTools() -> s`
* `GetToolSchema(s name) -> s`
* `CallTool(s name, s args_json) -> s`

---

## Exposed HTTP & gRPC Endpoints

### 1. HTTP / Server-Sent Events (SSE) Transport
* **Default Socket:** `0.0.0.0:3003` (CLI arg parsing in `crates/op-cognitive-mcp/src/main.rs:14`)
* **Protocol:** Model Context Protocol (MCP) over SSE. Routes are served by `op_mcp::HttpSseTransport` (`crates/op-cognitive-mcp/src/server.rs:94-96`).

### 2. gRPC Endpoint (`CognitiveToolService`)
* **Default Socket:** `0.0.0.0:50052` (CLI arg parsing in `crates/op-cognitive-mcp/src/main.rs:18`)
* **Service Identifier:** `operation.cognitive.v1.CognitiveToolService`
* **RPC Methods:**
  * `AskQuestion`
  * `QueryNotebook`
  * `ListNotebooks`
  * `GetNotebook`
  * `CreateNotebook`
  * `BatchCreateNotebooks`
  * `AddSource`
  * `AddFolder`
  * `ListSources`
  * `GetSourceContent`
  * `GenerateDataTable`
  * `GetHealth`
  * `SetupAuth`
  * `RemoveSource`
  * `GeminiQuery`
  * `GetToolProfile`
  * `Doctor`
  * `GetQueryHistory`

---

## Workspace Circular Dependency Risks
While there is no direct dependency cycle declared inside `Cargo.toml` files, there is an implicit **data-level circular dependency** and tight coupling risk with `op-grpc-bridge`. 

Both `op-cognitive-mcp` and `op-grpc-bridge` share a dependency on the zero-copy memory layout of the shared-memory file (`/dev/shm/plugin_schema.dat`). Because the shared-memory file layout behaves as an unversioned contract across crate boundaries, modifying the state engine structures in one crate without synchronized recompilation of the interceptors and shuttles will lead to severe data misalignment and runtime faults.

---

# Production Security & Quality Findings

### Critical Finding 1: Mismatched ABI Layouts for `IdentitySled` Leading to Out-of-Bounds Memory Read and Process Crash
* **File & Line Citation:** `crates/op-cognitive-mcp/src/interceptor.rs:5-15` and `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:24-31`
* **Severity:** Critical
* **Description:** 
  The codebase defines two completely different memory layouts for the `IdentitySled` struct, both of which are used to cast raw bytes mapped from the same shared memory location `/dev/shm/plugin_schema.dat`:
  * In `qdrant_shuttle.rs:24-31`, `IdentitySled` is defined as:
    ```rust
    #[repr(C)]
    pub struct IdentitySled {
        pub wireguard_pubkey: [u8; 32],
        pub mutation_index: u64,
        pub is_valid: bool,
        pub hashed_footprint: [u8; 32],
    }
    ```
    Its layout is unpadded (offset of `hashed_footprint` is `41` bytes).
  * In `interceptor.rs:5-15`, `IdentitySled` is defined as:
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
    Its layout is padded, forcing `hashed_footprint` to reside at offset `48`.
* **Exploitability & Impact:**
  1. **Authentication Bypass or Denial of Service:** When the shuttle reads the current `hashed_footprint` at offset `41`, and the `ghostbridge_interceptor` reads `hashed_footprint` from offset `48` of the exact same memory-mapped page, the temporal hash checks in `interceptor.rs:41-43` will fail. This causes all legitimate incoming gRPC requests to be blocked due to a `Temporal Hash Mismatch`.
  2. **Segmentation Fault / Denial of Service:** In `interceptor.rs:26-30`, the raw file `/dev/shm/plugin_schema.dat` is mapped via `mmap` with **zero validation of file length**. If the shared-memory file has been created with a size matching only the smaller `IdentitySled` definition (73 bytes), accessing the fields `control_source` (offset 160) or `nextdns_profile` (offset 192) in `interceptor.rs:46` or `interceptor.rs:47` triggers an out-of-bounds read. This results in an immediate segmentation fault (`SIGSEGV`) and crashes the entire gRPC server.

---

### Critical Finding 2: Arbitrary File Read and Information Disclosure via `AddFolder` Path Traversal
* **File & Line Citation:** `crates/op-cognitive-mcp/src/grpc_service.rs:328-393`
* **Severity:** Critical
* **Description:**
  The `AddFolder` gRPC endpoint accepts a user-provided `folder_path` parameter and resolves it using pure filesystem actions:
  ```rust
  let path = std::path::Path::new(&req.folder_path);
  if !path.exists() || !path.is_dir() { ... }
  ```
  The logic then crawls the designated folder, reads the content of all files, and ingests them into the database memory namespace:
  ```rust
  match std::fs::read_to_string(&entry_path) {
      Ok(content) => {
          ...
          match self.memory_store.store_entry(&namespace, &key, value, vec![], None).await
  ```
* **Exploitability & Impact:**
  There is no path sanitization, jail, or root-directory verification in the endpoint. Any unauthenticated or compromised client calling this gRPC service can provide arbitrary sensitive absolute paths (e.g. `/etc` or `/var/lib/op-dbus`) to `AddFolder`. The server will traverse those paths, ingest all readable files into the cognitive database, and allow the attacker to retrieve their contents via subsequents queries to `ListSources` or `GetSourceContent`.

---

### Schema-as-Code Violations
* **File & Line Citation:** 
  * `crates/op-cognitive-mcp/src/memory_store.rs:81`
  * `crates/op-cognitive-mcp/src/memory_store.rs:97`
  * `crates/op-cognitive-mcp/src/grpc_service.rs:434`
  * `crates/op-cognitive-mcp/src/grpc_service.rs:487`
  * `crates/op-cognitive-mcp/src/grpc_service.rs:565`
  * `crates/op-cognitive-mcp/src/dbus_interface.rs:30-47`
* **Severity:** Medium
* **Description:**
  The codebase bypasses the workspace's schema-as-code discipline in multiple places by representing structured data contracts as raw JSON strings or dynamic `serde_json::Value` structures:
  * In `memory_store.rs:81`, namespace `metadata` uses raw `serde_json::Value` without schema verification.
  * In `memory_store.rs:97`, the cognitive entries are typed as raw `serde_json::Value` rather than versioned schema primitives.
  * In `grpc_service.rs:434`, health diagnostics are generated and returned as an ad-hoc JSON serialized string (`components_json`).
  * In `grpc_service.rs:487`, authentication responses are returned as an ad-hoc string (`SetupAuthResponse`).
  * In `dbus_interface.rs:30-47`, tool structures and input schemas are returned to callers as raw ad-hoc JSON arrays and strings.

---

### Quality / Security Issue: Fragile Ad-Hoc String Matching for PII Detection
* **File & Line Citation:** `crates/op-cognitive-mcp/src/activity_filter.rs:163-176`
* **Severity:** Low
* **Description:**
  The PII filter determines whether an event should have its payload stripped before vector database ingestion using an ad-hoc string search on the field's description:
  ```rust
  if let Some(field_schema) = schema.fields.get(field_name) {
      return field_schema.description.to_lowercase().contains("[pii]")
          || field_schema.constraints.iter().any(|c| {
              matches!(c, op_state_store::plugin_schema::Constraint::Custom { validator }
                  if validator == "pii")
          });
  }
  ```
* **Impact:**
  Relying on developers to correctly type a string substring like `[pii]` in user-facing descriptions is fragile. If descriptions contain phrases like "This field does not contain pii", or if `[pii]` is misspelled, the filter will fail to flag the PII content. This results in sensitive user data being permanently indexed in Qdrant vector databases, bypassing auditing pipelines and exposing sensitive user secrets to semantic queries. PII flags must be expressed as strict, structured boolean markers or metadata tags in the schema engine definitions.