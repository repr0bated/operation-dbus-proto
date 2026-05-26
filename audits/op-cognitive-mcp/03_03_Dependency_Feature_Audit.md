## Dependencies & Feature Inventory

### Direct Dependencies (from `crates/op-cognitive-mcp/Cargo.toml`)

| Dependency | Version | Explicit Features | Pulled by Default | Notes / Vulnerability Risks |
|---|---|---|---|---|
| `op-core` | Path `../op-core` | None | Yes | Internal crate dependency. |
| `op-mcp` | Path `../op-mcp` | None | Yes | Internal crate dependency. |
| `op-state-store` | Path `../op-state-store` | None | Yes | Internal crate dependency. |
| `op-dynamic-loader` | Path `../op-dynamic-loader` | None | Yes | Internal crate dependency. |
| `op-cache` | Path `../op-cache` | None | Yes | Internal crate dependency. |
| `op-cozo-store` | Workspace | None | Yes | Internal workspace crate. |
| `hex` | Workspace (`0.4`) | None | Yes | Helper crate for hex encoding/decoding. |
| `memmap2` | Workspace (`0.9`) | None | Yes | Used for memory-mapping the identity sled. |
| `serde` | `1.0` | `["derive"]` | Yes | Serialization/Deserialization framework. |
| `serde_json` | Workspace (`1`) | None | Yes | Standard JSON manipulation library. |
| `simd-json` | Workspace (`0.13`) | None | Yes (`"serde"`, `"serde_impl"`) | High-performance JSON parser. |
| `tokio` | `1.0` | `["full"]` | Yes | Asynchronous runtime. Enables all features (file system, network, tasks, timers, etc.). |
| `anyhow` | Workspace (`1`) | None | Yes | Flexible error-handling utility. |
| `qdrant-client` | `1.17` | None | Yes | Vector database client. **Risk:** Workspace version is defined as `1.7`, but overridden here to `1.17`. Mismatches can lead to symbol desynchronization. |
| `reqwest` | Workspace (`0.11`) | None | Yes (`"json"`, `"stream"`) | HTTP client. **Risk:** Workspace uses legacy `0.11` version, which drags in older `rustls` or `native-tls` combinations. |
| `tracing` | `0.1` | None | Yes | Structured diagnostics and logging facade. |
| `tracing-subscriber` | `0.3` | None | Yes (`"env-filter"`, `"json"`) | Formatting and dispatching of tracing spans. |
| `axum` | `0.7` | `["json", "http2"]` | Yes | HTTP server framework. |
| `tower` | `0.4` | None | Yes | Modular service middleware. |
| `tower-http` | `0.5` | `["cors"]` | Yes | Specialized HTTP middlewares. |
| `uuid` | `1.0` | `["v4"]` | Yes | Generates UUIDs. |
| `chrono` | `0.4` | `["serde"]` | Yes | Date and time handling with serialization. |
| `async-trait` | `0.1` | None | Yes | Support for async trait methods. |
| `clap` | Workspace (`4`) | `["env"]` | Yes (`"derive"`) | Command-line argument parsing. |
| `cozo` | Workspace (`0.7.6`) | None | Yes (`"rayon"`, `"storage-sled"`) | Relational-graph-vector Datalog engine. Uses Sled as the storage backend. Sled has known data-corruption edge cases under severe multi-thread contention. |
| `tonic` | Workspace (`0.12`) | None | Yes (`"tls"`, `"tls-roots"`) | gRPC implementation. |
| `prost` | Workspace (`0.13`) | None | Yes | Protocol Buffers implementation. |
| `tonic-reflection` | Workspace (`0.12`) | None | Yes | gRPC reflection services. |
| `tonic-health` | Workspace (`0.12`) | None | Yes | gRPC health-checking protocols. |
| `tonic-web` | Workspace (`0.12`) | None | Yes | Tonic-web gRPC bridge. |
| `zip` | `2` | None | Yes | Compressing and reading zip archives. |
| `sha2` | Workspace (`0.10`) | None | Yes | Cryptographic SHA-256 generation. |
| `regex` | Workspace (`1`) | None | Yes | Regular expression matching. |
| `dashmap` | Workspace (`5.0`) | None | Yes | High-concurrency in-memory concurrent hash maps. |
| `parking_lot` | Workspace (`0.12`) | None | Yes | Lightweight spinlock-fallback synchronization primitives. |
| `zbus` | Workspace (`4.0`) | None | Yes (`"tokio"`) | Native D-Bus bindings. |

### Build Dependencies (from `crates/op-cognitive-mcp/Cargo.toml`)

| Dependency | Version | Features | Notes |
|---|---|---|---|
| `tonic-build` | `0.12` | None | Generates gRPC bindings from Protobuf files. |

### Crate Features Section (`crates/op-cognitive-mcp/Cargo.toml`)

No `[features]` section is defined for `crates/op-cognitive-mcp`.

---

## Storage Backend Table

| Backend | Found at File:Line | Role (KV/Graph/Cache/Queue) | Notes / Compliance Check |
|---|---|---|---|
| **CozoDB (Sled backend)** | `crates/op-cognitive-mcp/src/server.rs:25` | Relational/Graph Persistence | Stores namespaces, entities, users, sessions, compliance graph, and audit trails. Properly utilizes workspace `op-cozo-store` avoiding Rusqlite/SQLite lock contention. |
| **Qdrant** | `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:46` | Vector Store / Semantic retrieval | Stores reasoning episodes with high-dimensional embeddings for accountability tracking. |
| **DashMap** | `crates/op-cognitive-mcp/src/session.rs:37` | In-memory KV / Cache | Serves as Phase 1 conversational history tracking. Comments indicate a plan to introduce SQLite/sqlx backing, which is flagged below as an architectural desynchronization. |

---

## Security Findings & Schema-As-Code Gaps

### [Critical] Buffer Overread / Undefined Behavior in `ghostbridge_interceptor` via Lack of Length Verification on Memory-Mapped File
- **Vulnerability Type**: CWE-125: Out-of-bounds Read / Use of Uninitialized Memory
- **File:Line**: `crates/op-cognitive-mcp/src/interceptor.rs:35-40`

#### Description
In `interceptor.rs`, the `ghostbridge_interceptor` maps the file `/dev/shm/plugin_schema.dat` to memory using the `memmap2` crate:
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
There is no check validating whether the size of the memory-mapped region (`mmap.len()`) is greater than or equal to `std::mem::size_of::<IdentitySled>()` (which is 208 bytes). If the shared memory file is empty or truncated to a size smaller than 208 bytes, dereferencing the pointer `sled_ptr` triggers a buffer overread and undefined behavior. This will result in an immediate segmentation fault, crashing the gRPC and SSE dual-transport servers, causing a complete Denial of Service (DoS) of the cognitive control plane.

#### Remediation
Before casting `mmap.as_ptr()`, strictly enforce that the mapped file size matches or exceeds the struct size, similar to the check implemented in the Qdrant shuttle:
```rust
    if mmap.len() < std::mem::size_of::<IdentitySled>() {
        return Err(Status::failed_precondition("Identity Sled size mismatch / truncated memory."));
    }
```

---

### [Critical] Local File Inclusion (LFI) / Arbitrary Directory Walk and Host File Read via gRPC Ingestion
- **Vulnerability Type**: CWE-22: Improper Limitation of a Pathname to a Restricted Directory
- **File:Line**: `crates/op-cognitive-mcp/src/grpc_service.rs:752-878`

#### Description
The `add_folder` gRPC endpoint walks a directory path provided by the caller (`req.folder_path`) and ingests the contents of every file into the database:
```rust
        let path = std::path::Path::new(&req.folder_path);
        if !path.exists() || !path.is_dir() {
            return Err(Status::invalid_argument(format!(
                "Folder '{}' does not exist or is not a directory",
                req.folder_path
            )));
        }
        ...
        // Walk directory — no shell, pure Rust
        let walker = if req.recursive {
            walkdir(path)
        } else {
            walkdir_shallow(path)
        };

        for entry_path in walker {
            ...
            match std::fs::read_to_string(&entry_path) {
                Ok(content) => {
                    let key = entry_path.file_name()...
                    let value = serde_json::json!({
                        "source_type": "file",
                        "content": content,
                        "path": entry_path.to_string_lossy(),
                    });
                    self.memory_store.store_entry(&namespace, &key, value, vec![], None)...
```
This endpoint implements no path validation, sandboxing, or isolation checks. An attacker (or a compromised, prompt-injected LLM agent authorized to call the MCP tools) can specify sensitive host directories such as `/etc`, `/root`, or `/home/user/.ssh`. The service will recursively read and upload all sensitive host files into the cognitive storage engine (`self.memory_store`). The attacker can then view the contents of these private files via `list_sources` and `get_source_content` gRPC queries.

#### Remediation
Sanitize and restrict the folder paths allowed in `add_folder`. Require paths to reside within a safe, configured, absolute directory root (e.g. `/var/lib/op-cognitive-mcp/workspace/`). Ensure canonicalized sub-paths do not escape this root:
```rust
    let canonical_root = std::fs::canonicalize(root_dir)?;
    let canonical_target = std::fs::canonicalize(path)?;
    if !canonical_target.starts_with(&canonical_root) {
        return Err(Status::permission_denied("Directory traversal detected outside workspace root."));
    }
```

---

### [High] Struct Definition Mismatch (ABI Layout Desynchronization) on Memory-Mapped `IdentitySled`
- **Vulnerability Type**: CWE-436: API / ABI Mismatch
- **File:Line**: `crates/op-cognitive-mcp/src/qdrant_shuttle.rs:20-27` compared with `crates/op-cognitive-mcp/src/interceptor.rs:5-17`

#### Description
The structure `IdentitySled` (which is compiled as `#[repr(C)]` and mapped from `/dev/shm/plugin_schema.dat`) has two contradictory layouts defined inside the exact same codebase:

**interceptor.rs definition:**
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

**qdrant_shuttle.rs definition:**
```rust
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdentitySled {
    pub wireguard_pubkey: [u8; 32],
    pub mutation_index: u64,
    pub is_valid: bool,
    pub hashed_footprint: [u8; 32],
}
```

This ABI mismatch has two major security and operational consequences:
1. **Layout Offset Shift**: In `qdrant_shuttle.rs`, `hashed_footprint` has alignment `1` (as a `[u8; 32]` array). Without the 7-byte padding field `_pad` explicitly added after `is_valid` (as done in `interceptor.rs`), `hashed_footprint` will sit at offset `41` under `#[repr(C)]`. In `interceptor.rs`, `hashed_footprint` sits at offset `48`. Any validation or query based on `hashed_footprint` inside `qdrant_shuttle.rs` will read garbage padding bytes and output a truncated temporal hash, breaking the temporal consistency checking in the accountability loop.
2. **Buffer parsing location mismatch**: `qdrant_shuttle.rs` extracts the `schema_bytes` by skipping past `size_of::<IdentitySled>()`:
   ```rust
   let schema_bytes = mmap[size_of::<IdentitySled>()..]
   ```
   Because `IdentitySled` in `qdrant_shuttle.rs` is only ~73-80 bytes long (vs 208 bytes in `interceptor.rs`), the shuttle begins reading the JSON schema payload from offset `73` or `80`. This offset is actually the middle of the struct in memory mapping, pointing to `schema_uuid`, `subid`, or `control_source` binary structures. This causes `serde_json::from_slice(&schema_bytes)` to fail to parse the schema every time, rendering the semantic accountability loop completely broken.

#### Remediation
Converge on a single canonical definition of `IdentitySled` inside a shared core crate (such as `op-core`), and delete the duplicate local definitions.

---

### [Medium] Bypassable Permissions on Private Chrome Profile Credentials in `setup_auth`
- **Vulnerability Type**: CWE-732: Incorrect Permission Assignment for Critical Resource
- **File:Line**: `crates/op-cognitive-mcp/src/grpc_service.rs:693-706`

#### Description
In `setup_auth`, when the authentication method is `"chrome_profile"`, the server checks whether the targeted path has overly permissive file system permissions on Unix (i.e., readable by other users):
```rust
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if let Ok(metadata) = std::fs::metadata(path) {
                    let mode = metadata.mode() & 0o777;
                    if mode & 0o077 != 0 {
                        warn!(
                            path = %req.credential,
                            mode = format!("{:o}", mode),
                            "Chrome profile has overly permissive permissions; should be 0o600"
                        );
                    }
                }
            }
```
If permissions are overly permissive, the code logs a `warn!` but **continues to run and returns a success response** to the gRPC client. If the credential path contains private session cookies or sensitive profile states, local unprivileged users can read the Chrome profile database and hijack authenticated Google/NotebookLM sessions.

#### Remediation
Strictly abort the request and return `Status::permission_denied` if credentials or profile paths are found with unsafe permissions:
```rust
                    if mode & 0o077 != 0 {
                        return Err(Status::permission_denied(format!(
                            "Chrome profile path '{}' has insecure permissions ({:o}). Must be restricted to 0o600.",
                            req.credential, mode
                        )));
                    }
```

---

### [Medium] Activity Filter Queue Deadlock/Retention Leak via Manipulation of Out-Of-Order Event Timestamps
- **Vulnerability Type**: CWE-400: Uncontrolled Resource Consumption / Logical Flaw
- **File:Line**: `crates/op-cognitive-mcp/src/activity_filter.rs:208-226`

#### Description
In `activity_filter.rs`, `evict_expired` removes elements from the front of the sliding deduplication window if their timestamp is older than `cutoff`:
```rust
    async fn evict_expired(&self, t: &FilterTunables) {
        let cutoff = Utc::now() - Duration::seconds(t.dedup_window_secs);
        let mut w = self.window.write().await;
        while w.front().map_or(false, |e| e.timestamp < cutoff) {
            w.pop_front();
        }
    }
```
The timestamp is derived from the *incoming event* payload (`event.timestamp`), rather than being recorded as the system time of reception. If an event is received with a manipulated timestamp far in the future (e.g., year 2038), it is pushed to the back of the queue.

Once this future-dated event reaches the front of the queue, the condition `e.timestamp < cutoff` will evaluate to `false` for every call of `evict_expired`. This freezes system eviction; no expired elements sitting behind it in the queue will ever be cleaned up by the chronological sliding window. Although bounded by `dedup_window_max` to prevent memory exhaustion, this logical flaw completely bypasses time-based sliding de-duplication rules.

#### Remediation
Use `Utc::now()` to assign the `timestamp` for sliding window eviction entries rather than using the event's client-side reported timestamp:
```rust
        w.push_back(WindowEntry {
            timestamp: Utc::now(), // Use reception time for reliable sliding window arithmetic
            content_hash: event.content_hash.clone(),
        });
```

---

### [Schema-As-Code Gaps] Ad-Hoc Data Contracts Defined via Hardcoded Code Structures & Loose String Formatting

The codebase violates the Schema-as-Code discipline in multiple locations by expressing security-sensitive, cross-component communication contracts as ad-hoc strings, manual JSON maps, or custom structures instead of generating them from versioned, formal schemas (e.g. Protocol Buffers, JSON Schema, or OSCAL definitions).

#### 1. Ad-Hoc Compliance Verification with Strings
- **File:Line**: `crates/op-cognitive-mcp/src/interceptor.rs:56-62`
- **Violation**: The gRPC Temporal Hash Interceptor verifies compliance by reading the `control_source` field of the identity sled and parsing it as a raw string to match against an OSCAL control source header:
  ```rust
  let control_source = unsafe { &(*sled_ptr).control_source };
  let end = control_source.iter().position(|&b| b == 0).unwrap_or(32);
  let oscal_header = std::str::from_utf8(&control_source[..end]).unwrap_or("");
  ```
  Instead of utilizing proper, structured OSCAL compliance schemas, the contract is expressed as a loosely bound, manually indexed `[u8; 32]` array.

#### 2. Ad-Hoc Input Schema Generation for MCP Tools
- **File:Line**: `crates/op-cognitive-mcp/src/cognitive_tools.rs:76-118`
- **Violation**: The `MemoryTool::input_schema()` manually generates JSON validation schemas using inline macro values:
  ```rust
  fn input_schema(&self) -> Value {
      json!({
          "type": "object",
          "properties": {
              "operation": { ... },
              "namespace": { ... },
              ...
  ```
  This is prone to divergence from the actual implementation parameters (such as `EntryQuery` parameters) and fails to enforce centralized OpenAPI or JSON-schema specifications.

#### 3. Unversioned Chatbot Conversational State Contracts
- **File:Line**: `crates/op-cognitive-mcp/src/session.rs:17-36`
- **Violation**: `ConversationSession` and `QueryTurn` are defined as basic Rust structures without schema-as-code integration. There are no accompanying schema files (Protobuf/JSON Schema) to guarantee interoperability or validate historical trace structure changes during state migration or SQLite serialization.

#### 4. Ad-Hoc Event Payload and Operations Types
- **File:Line**: `crates/op-cognitive-mcp/src/activity_filter.rs:115-161`
- **Violation**: `ActivityEvent` defines security significance metrics and event telemetry, yet relies on `payload: serde_json::Value` (line 160). This is a completely unvalidated, untyped blob bypassing the schema enforcement layers of both the agent plane and state stores.

#### Remediation of Schema Gaps
Compile all data transfer objects, logging payloads, and tool input definitions using Protobuf schemas (compiled via `prost-build`) or compile-time generated JSON schemas (`schemars`). Ensure the gRPC service and MCP tools ingest these centralized, versioned schemas as the single source of truth.