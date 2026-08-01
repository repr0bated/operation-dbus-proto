# Production Security and Quality Audit: op-cognitive-mcp

## 1. Executive Summary

This security and quality audit evaluates the `op-cognitive-mcp` crate, focusing on concurrency, asynchronous safety, schema-as-code discipline, and production security vulnerabilities. 

During the audit, **two Critical vulnerabilities** were identified in the shared memory sled mapping and the gRPC interceptor that lead to direct memory unsafety, denial of service, and complete functional collapse. Additionally, **two High-severity issues** were found regarding blocking reactor threads and unauthenticated privilege escalation over the D-Bus System Bus.

---

## 2. Async & Concurrency Analysis

### 2.1 Primitives & Spawn Counts
*   **`async fn` count:** 81
*   **`tokio::spawn` count:** 1 (located in `crates/op-cognitive-mcp/src/server.rs:144`)
*   **`spawn_blocking` count:** 0

### 2.2 Blocking Reactor Violations
Several synchronous blocking filesystem calls are executed directly inside asynchronous contexts, which blocks the Tokio reactor thread pool:

1.  **`crates/op-cognitive-mcp/src/grpc_service.rs`**:
    *   **Line 573:** `path.exists()` and `path.is_dir()` perform blocking filesystem checks.
    *   **Lines 597–603:** `walkdir` synchronously queries directory entries on a worker thread.
    *   **Line 608:** `std::fs::read_to_string(&entry_path)` blocks the worker thread to read file contents into memory.
    *   **Line 877:** `path.exists()` blocks the thread.
    *   **Line 888:** `std::fs::metadata(path)` performs synchronous filesystem metadata reading.

2.  **`crates/op-cognitive-mcp/src/rag_pipeline.rs`**:
    *   **Line 132:** `std::fs::File::open(zip_path)` is executed inside the async function `ingest_repomix_entry`.
    *   **Lines 133–140:** Zip file archive creation and index reading are executed synchronously inside an async context.

### 2.3 JoinHandle Ownership & Lifecycles
*   In `crates/op-cognitive-mcp/src/server.rs` at line 144, `tokio::spawn` is used to run the gRPC server. The returned `grpc_handle` is stored and awaited at line 170.
*   *Vulnerability:* If `self.start_http_server(&http_addr).await?` at line 169 returns an error, the function terminates early, and the `grpc_handle` is dropped before it is awaited. This detaches the spawned task, leaving the gRPC server running in the background while the rest of the application fails to initialize, leading to orphaned threads or zombie servers.

### 2.4 Send & Sync Boundaries
*   `MemoryTool` in `crates/op-cognitive-mcp/src/cognitive_tools.rs` implements `Tool`, a public async trait. The `execute` method correctly takes ownership of `Value` and returns a `Result<Value>`, which satisfies standard `Send` and `Sync` boundaries as required by tonic and tokio frameworks.

---

## 3. Schema-As-Code & Contract Audit

This codebase deviates from the schema-as-code discipline, often relying on ad-hoc structs and freeform stringified JSON payloads rather than versioned, centralized schemas (such as Protocol Buffers or OSCAL schemas).

### 3.1 Ad-Hoc Contracts
1.  **`crates/op-cognitive-mcp/src/activity_filter.rs:125`**: `ActivityEvent` defines its inner payload as `payload: serde_json::Value`. This allows unvalidated, unversioned JSON structures to pass through the filter, violating strict data contracts.
2.  **`crates/op-cognitive-mcp/src/quota.rs:14`**: `QuotaTier` is defined as an ad-hoc Rust struct, serialized using arbitrary JSON rules instead of a versioned configuration schema.
3.  **`crates/op-cognitive-mcp/src/session.rs:18`**: `ConversationSession` is an ad-hoc Rust struct with embedded history turns (`QueryTurn`), which does not reference any versioned contract.
4.  **`crates/op-cognitive-mcp/src/dbus_interface.rs:29–55`**:
    *   **Line 29 (`list_tools`):** Returns a raw JSON-serialized string representing an array of tools instead of utilizing structured, typed D-Bus arguments or protobuf-defined messages.
    *   **Line 37 (`get_tool_schema`):** Returns the input schema as a raw JSON string.
    *   **Line 45 (`call_tool`):** Accepts parameters as a raw JSON string `args_json` and returns raw JSON string output.

---

## 4. Production Vulnerabilities & Security Findings

### Finding 1: Memory Corruption & Out-of-Bounds Read in gRPC Interceptor
*   **Severity:** Critical (Directly Exploitable)
*   **File:** `crates/op-cognitive-mcp/src/interceptor.rs`
*   **Lines:** 31–45
*   **Vulnerability Type:** Missing Bounds Check / Out-of-Bounds Pointer Dereference

#### Description
The `ghostbridge_interceptor` function opens and mmaps the shared identity sled file `/dev/shm/plugin_schema.dat`:
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
There is no validation of the mmap's size before casting the pointer to `*const IdentitySled` and dereferencing its fields. If `/dev/shm/plugin_schema.dat` is empty (0 bytes) or truncated (less than `size_of::<IdentitySled>()`), dereferencing `sled_ptr` accesses unmapped or out-of-bounds memory.

#### Exploit Scenario
Because this interceptor runs on **every incoming gRPC request** (prior to authentication checks), an unauthenticated remote attacker can trigger this endpoint. If the shared memory file is empty or still initializing, the interceptor dereferences an invalid pointer, triggering a segmentation fault (SIGSEGV) and instantly terminating the control plane process.

#### Recommendation
Add an explicit length check before dereferencing:
```rust
if mmap.len() < std::mem::size_of::<IdentitySled>() {
    return Err(Status::failed_precondition("Shared memory sled is truncated."));
}
```

---

### Finding 2: Structural ABI Alignment Divergence on Shared Memory Sled
*   **Severity:** Critical (Directly Exploitable)
*   **Files:** `crates/op-cognitive-mcp/src/qdrant_shuttle.rs` and `crates/op-cognitive-mcp/src/interceptor.rs`
*   **Lines:** `qdrant_shuttle.rs:26–33` and `interceptor.rs:5–16`
*   **Vulnerability Type:** Memory Corruption / ABI Layout Mismatch

#### Description
Two completely different definitions of the `IdentitySled` struct exist in the same crate, both mapping to the exact same shared memory path (`/dev/shm/plugin_schema.dat`):

**`qdrant_shuttle.rs` Layout:**
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

**`interceptor.rs` Layout:**
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

The layout in `interceptor.rs` defines `IdentitySled` with a size of 208 bytes and explicitly inserts `_pad: [u8; 7]` to align `hashed_footprint` to 8-byte boundaries. In contrast, `qdrant_shuttle.rs` omits this padding and has a total struct size of only 73 bytes.

#### Exploit Scenario & Consequences
1.  **Memory corruption of `hashed_footprint`:** When `qdrant_shuttle.rs` attempts to read the footprint (at offset 41), it reads offset 41 to 72, which maps to padding and scrambled fields from the writer's perspective (who wrote it at offset 48). This breaks the integrity of `trace_id` generation.
2.  **Appended Schema Read Failure:** `qdrant_shuttle.rs` extracts the schema bytes starting at `size_of::<IdentitySled>()` (offset 73). However, the interceptor (or writer) appended the schema bytes at offset 208. Therefore, `qdrant_shuttle.rs` parses the interceptor's `schema_uuid`, `subid`, and `control_source` binary values as UTF-8 JSON. This causes `parse_plugin_schema` to fail with parsing errors, completely blocking vector retrieval.

#### Recommendation
Consolidate the definition of `IdentitySled` into a single, versioned module (e.g., `op-state-store` or a common library) and use a unified struct across all modules.

---

### Finding 3: Unauthenticated Local Privilege Escalation via System Bus
*   **Severity:** High
*   **Files:** `crates/op-cognitive-mcp/src/server.rs` and `crates/op-cognitive-mcp/src/dbus_interface.rs`
*   **Lines:** `server.rs:181–188` and `dbus_interface.rs:45–55`
*   **Vulnerability Type:** Broken Access Control / Lack of Authentication

#### Description
In `server.rs`, `start_dbus` registers the service on the system-wide D-Bus System Bus:
```rust
let conn = zbus::Connection::system().await?;
conn.request_name("org.opdbus.CognitiveMcp").await?;
```
The D-Bus interface (`dbus_interface.rs`) exposes a method named `call_tool`:
```rust
async fn call_tool(&self, name: String, args_json: String) -> String
```
There is no verification of the caller's UNIX user ID (UID), GID, or credentials. Any unprivileged local user or process connected to the D-Bus system bus can execute registered cognitive tools.

#### Consequences
If sensitive tools (such as direct file readers, database writers, or commands invoking shell execution via `notebooklm-mcp`) are registered under this registry, any local attacker can escalate their privileges by executing arbitrary administrative tools via the D-Bus system bus.

#### Recommendation
Incorporate a caller credential check inside each D-Bus method using the `zbus::Message` header to verify the caller's UID and enforce strict permissions:
```rust
let header = connection.message_header()?;
let sender_uid = connection.peer_credentials(header.sender()?)?.uid();
if sender_uid != 0 {
    return Err(zbus::fdo::Error::NotSupported("Access denied".to_string()));
}
```

---

### Finding 4: Blocking Thread Pool Denial of Service (DoS) in Directory Ingestion
*   **Severity:** High
*   **File:** `crates/op-cognitive-mcp/src/grpc_service.rs`
*   **Lines:** 597–612
*   **Vulnerability Type:** Async Thread Pool Starvation / Resource Exhaustion

#### Description
The gRPC endpoint `add_folder` allows users to ingest directories. When processing files, the server synchronously iterates through all entries in the directory (lines 597–603) and reads their contents:
```rust
match std::fs::read_to_string(&entry_path) {
    Ok(content) => {
        ...
```
This execution occurs directly within the Tokio worker thread pool. There are no limits enforced on the directory depth, file size, or file count.

#### Consequences
An attacker can specify a path to a huge directory (e.g., `/var/log` or mount loops). Walking this directory and reading all files into memory synchronously blocks the Tokio worker thread pool, rendering the gRPC server unresponsive to all other users.

#### Recommendation
Wrap synchronous file and directory operations inside `tokio::task::spawn_blocking`:
```rust
let content = tokio::task::spawn_blocking(move || {
    std::fs::read_to_string(entry_path)
}).await??;
```