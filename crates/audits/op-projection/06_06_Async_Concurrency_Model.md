# Production Security and Quality Audit: op-projection

## 1. Async & Concurrency Analysis

As part of the systems audit, the async runtime usage, reactor blocking patterns, and thread safety bounds have been mapped across the audited `op-projection` codebase.

### Concurrency Metrics
*   **`async fn` count**: **8**
    *   `crates/op-projection/src/dbus_reader.rs:25` (`async fn introspect`)
    *   `crates/op-projection/src/plugin_reader.rs:81` (`pub async fn new`)
    *   `crates/op-projection/src/plugin_reader.rs:163` (`pub async fn read_all_async`)
    *   `crates/op-projection/src/plugin_reader.rs:177` (`pub async fn read_plugin_objects_async`)
    *   `crates/op-projection/src/plugin_reader.rs:189` (`pub async fn read_nested_objects_async`)
    *   `crates/op-projection/src/plugin_reader.rs:208` (`async fn read_loaded_plugin`)
    *   `crates/op-projection/src/json_stream.rs:207` (`async fn sse_handler`)
    *   `crates/op-projection/src/bin/projection_server.rs:15` (`async fn main`)
*   **`tokio::spawn` count**: **1**
    *   `crates/op-projection/src/json_stream.rs:104` (Spawns the background SSE Axum server listener loop)
*   **`spawn_blocking` count**: **0**

### Concurrency and Trait Bound Safety
*   **Reactor-Blocking Operations in Async Contexts**: Multiple instances of synchronous filesystem and hardware reads occur directly inside the primary async runtime threads, creating starvation risks for the reactor. Details are provided in **Finding 11**.
*   **Dropped JoinHandles**: The `tokio::spawn` handle in `json_stream.rs` is discarded, leading to unmanaged server background failures. Details are provided in **Finding 7**.
*   **Send/Sync Bounds on Public Async Traits**: The crate's core async trait interfaces are defined entirely as synchronous traits. No public async traits or `async_trait` macro annotations are declared in `crates/op-projection/src/interfaces.rs`, eliminating public send-bound leak risks for external implementations.

---

## 2. Security and Quality Findings

### Finding 1: Insecure No-Op Redaction Stub in AccessController
*   **Severity**: Critical
*   **Citation**: `crates/op-projection/src/access_control.rs:105-110`
*   **Vulnerability Type**: Sensitive Data Exposure / Missing Access Control

#### Description
The `ProjectionAccessController` claims to enforce data redaction policies for sensitive fields (such as secrets and PII) matching specified paths. However, the internal implementation of `redact_sensitive` is a hardcoded placeholder that returns a clone of the unmodified data payload:

```rust
fn redact_sensitive(
    &self,
    data: &simd_json::OwnedValue,
    _requester: &Requester,
) -> simd_json::OwnedValue {
    // In production, use JSON paths from schema to redact
    data.clone()
}
```

In addition, `is_accessible` dynamically defaults to `true` on line 138.

#### Exploitation
When an administrator configures a policy to restrict or mask fields containing API keys, private keys, or PII (e.g., using `secret_paths` or `pii_paths` defined in `PluginSchema`), the system silently bypasses this requirement. Requesters without permissions will receive the raw, unredacted, and unmasked sensitive payloads.

#### Remediation
Implement deep-path redaction using the `secret_paths` and `pii_paths` specified in the corresponding `PluginSchema`. Parse and traverse the `simd_json::OwnedValue` to nullify or replace matched fields before returning the payload.

```rust
fn redact_sensitive(
    &self,
    data: &simd_json::OwnedValue,
    _requester: &Requester,
) -> simd_json::OwnedValue {
    let mut redacted = data.clone();
    // Retrieve associated schema and iterate over secret_paths / pii_paths 
    // to recursively strip or mask matching JSON pointers.
    redacted
}
```

---

### Finding 2: Unsafe Shared-Memory Dereferencing and Segmentation Fault Risk in IdentitySledReader
*   **Severity**: Critical
*   **Citation**: `crates/op-projection/src/sled_reader.rs:49-55`
*   **Vulnerability Type**: Memory Safety (Undefined Behavior / Torn Reads / Local DoS)

#### Description
The `IdentitySledReader` maps the shared memory file `/dev/shm/...` into its virtual address space and casts the raw pointer directly to a standard Rust shared reference:

```rust
fn read_sled_entity(&self) -> Result<RawEntity> {
    let (ptr, _mmap) =
        read_sled().map_err(|e| anyhow::anyhow!("Failed to read sled: {}", e))?;
    let sled = unsafe { &*ptr };

    let footprint = hex::encode(sled.hashed_footprint);
    let pubkey = hex::encode(sled.wireguard_pubkey);
    // ...
}
```

This dereference violates memory safety on two fronts:
1.  **Aliasing Violation & Undefined Behavior**: The memory behind `ptr` is mutable and owned by a separate process (the Identity service / Sled writer). Accessing it as a standard immutable Rust reference (`&IdentitySled`) is a violation of Rust's safety invariants. The compiler assumes `&T` references are immutable and will optimize reads accordingly, leading to unstable compilation output, torn reads, and potential race conditions.
2.  **Unvalidated Buffer Sizing**: The code does not validate that the mapped shared memory size is equal to or greater than `std::mem::size_of::<IdentitySled>()`. If the shared memory file is truncated or corrupted by another process, dereferencing the pointer triggers an out-of-bounds read.

#### Exploitation
If the shared memory file is truncated to a size smaller than the struct, calling `read_all` or `read_entity` triggers a Segmentation Fault (SIGSEGV), crashing the entire projection control plane. Additionally, concurrent writes by the identity daemon during a read cycle lead to torn reads, returning corrupted WireGuard keys or footprints.

#### Remediation
Never cast shared memory raw pointers directly to `&T`. Use raw pointer reads (`std::ptr::read_volatile`) or volatile accessor wrappers. Validate the size of the underlying memory mapping before attempting any read operation.

```rust
fn read_sled_entity(&self) -> Result<RawEntity> {
    let (ptr, mmap) = read_sled().map_err(|e| anyhow::anyhow!("Failed to read sled: {}", e))?;
    if mmap.len() < std::mem::size_of::<IdentitySled>() {
        return Err(anyhow::anyhow!("Shared memory buffer is truncated"));
    }
    
    // Copy the struct out of the memory-mapped file safely
    let sled: IdentitySled = unsafe { std::ptr::read_volatile(ptr) };
    
    let footprint = hex::encode(sled.hashed_footprint);
    let pubkey = hex::encode(sled.wireguard_pubkey);
    // ...
}
```

---

### Finding 3: Regular Expression Compilation Denial of Service (DoS / ReDoS)
*   **Severity**: High
*   **Citation**: `crates/op-projection/src/access_control.rs:52-53` and `75-76`
*   **Vulnerability Type**: Resource Exhaustion / Denial of Service

#### Description
The `ProjectionAccessController` processes policies by dynamically compiling regex patterns on *every single request* inside the hot-path execution loops of `enforce_policy` and `validate_permissions`:

```rust
let policies = self.policies.read();
for policy in policies.iter() {
    let re = Regex::new(&policy.resource_pattern)?;
    if re.is_match(&projection.id) && policy.redact_sensitive {
        result.data = self.redact_sensitive(&result.data, requester);
    }
}
```

Dynamic compilation of regexes inside a read-lock loop is highly inefficient. Furthermore, there is no validation of the regular expression pattern when a policy is registered via `add_policy`.

#### Exploitation
1.  **System Collapse**: If an administrator registers an invalid regex pattern via `add_policy`, subsequent evaluation requests on *any* projection will fail with `Err(anyhow::Error)`, permanently disabling permission checking and freezing control plane authorization.
2.  **ReDoS**: If the policy source is untrusted or editable by lower-privilege operators, registering a complex regular expression with catastrophic backtracking invariants (e.g., `(a+)+`) will block the thread pool, causing a severe Denial of Service.

#### Remediation
Compile and cache the `Regex` objects inside `AccessPolicy` upon registration, or maintain a pre-compiled thread-safe cache. Validate that all submitted patterns are syntactically sound prior to insertion.

```rust
#[derive(Debug, Clone)]
pub struct AccessPolicy {
    pub id: String,
    pub resource_pattern: String,
    pub compiled_regex: regex::Regex, // Store pre-compiled Regex
    pub required_permissions: Vec<String>,
    pub action: String,
    pub redact_sensitive: bool,
}
```

---

### Finding 4: Ad-Hoc Data Contracts Violating Schema-as-Code Discipline
*   **Severity**: Medium (Code Quality / Architectural Compliance)
*   **Citation**: 
    *   `crates/op-projection/src/procfs_reader.rs:114-118`
    *   `crates/op-projection/src/procfs_reader.rs:141-145`
    *   `crates/op-projection/src/procfs_reader.rs:163-167`
    *   `crates/op-projection/src/procfs_reader.rs:185-189`
    *   `crates/op-projection/src/sled_reader.rs:56-60`
    *   `crates/op-projection/src/dbus_reader.rs:56-62`
*   **Vulnerability Type**: Ad-Hoc Struct/String Data Contracts

#### Description
This codebase is specified to follow a strict Schema-as-Code discipline. However, state extraction layers bypass this architecture by declaring data contracts as ad-hoc, untyped JSON structures constructed inline with raw string literals:

```rust
// crates/op-projection/src/procfs_reader.rs
Ok(RawEntity {
    entity_type: "system.memory".to_string(),
    entity_id: "current".to_string(),
    data: json!({ "total_kb": total_kb, "free_kb": free_kb }).into(),
    source: self.source.clone(),
})
```

By manually defining fields like `"total_kb"`, `"free_kb"`, `"cores"`, `"model"`, `"types"`, `"interfaces"`, `"mutation_index"`, and `"wireguard_pubkey"` as hardcoded keys within `json!` macro calls, the code creates a decentralized set of contracts that cannot be formally versioned, verified at compile time, or exported as declarative schemas.

#### Impact
Field name changes or type changes will silently break downstream projection engines without compiler warnings. This bypasses the schema registry guarantees and diverges from standardized, versioned interfaces.

#### Remediation
Incorporate Protocol Buffers or formalized OSCAL schemas to generate native Rust structs with deterministic serialization traits. Refactor the `SourceReader` implementations to populate code-generated models rather than untyped, manually typed JSON maps.

---

### Finding 5: Fragile Ad-Hoc XML Parsing for D-Bus Introspection
*   **Severity**: High
*   **Citation**: `crates/op-projection/src/dbus_reader.rs:39-51`
*   **Vulnerability Type**: Improper Input Validation / Parser Bypass

#### Description
The `SystemDbusReader` introspects D-Bus services and extracts children nodes by parsing XML via crude string splits and pattern matching:

```rust
// Very basic XML parsing for children
// In production, use a proper XML parser
let mut children = Vec::new();
for line in xml.lines() {
    if line.contains("<node name=\"") {
        if let Some(name) = line
            .split("name=\"")
            .nth(1)
            .and_then(|s| s.split('\"').next())
        {
            if !name.is_empty() {
                children.push(name.to_string());
            }
        }
    }
}
```

This ad-hoc XML parsing is highly fragile and trivially bypassed or exploited.

#### Exploitation
A compromised or malicious D-Bus service can return structured Introspection XML containing false `<node name="..."` definitions embedded inside comments, text blocks, or other unrelated attributes. The parser will extract these as valid node names. 

If a service returns a node name containing path traversal sequences (e.g., `../../target`), line 54:
```rust
let child_path = format!("{}/{}", path, child);
```
concatenates the untrusted string directly, producing arbitrary, spoofed D-Bus object paths which are then pushed into the projection engine as valid objects.

#### Remediation
Replace the ad-hoc line-by-line parsing loop with a structured, robust XML pull parser (such as `quick-xml` already included in the workspace dependencies). Validate that extracted node names conform strictly to D-Bus object path naming rules.

---

### Finding 6: Unbounded In-Memory Audit Trail Growth (Memory Exhaustion / DoS)
*   **Severity**: High
*   **Citation**: `crates/op-projection/src/access_control.rs:133`
*   **Vulnerability Type**: Resource Management / Memory Leak

#### Description
Every access control evaluation records its outcome by appending an `AccessControlAudit` entry to an in-memory audit trail:

```rust
self.audit_trail.write().push(audit);
```

This audit trail vector is owned by a long-lived `Arc` structure on the controller and is never capped, truncated, rotated, or written out to an external storage database.

#### Exploitation
In a production deployment under persistent traffic, thousands of permission checks occur per minute. Because the `audit_trail` vector grows indefinitely, memory consumption of the projection server will increase linearly. Over time, this results in out-of-memory (OOM) situations, crashing the process.

#### Remediation
Introduce an audit rotation policy, use a bounded ring buffer (e.g., a double-ended queue with a fixed max capacity), or write audit events asynchronously to a persistent database or structured log stream using the `tracing` framework instead of keeping them in memory.

---

### Finding 7: Dropped tokio::spawn JoinHandle in SSE Stream Server
*   **Severity**: Medium (Operational Resiliency)
*   **Citation**: `crates/op-projection/src/json_stream.rs:104-115`
*   **Vulnerability Type**: Ignored Runtime Errors / Silent Process Failure

#### Description
In `ProjectionStreamServer::start`, the background Axum listener thread is spawned via `tokio::spawn`, but the returned `JoinHandle` is completely discarded:

```rust
tokio::spawn(async move {
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            warn!(error = %e, "Failed to bind JSON-stream server");
            return;
        }
    };

    if let Err(e) = axum::serve(listener, app).await {
        warn!(error = %e, "JSON-stream server error");
    }
});
```

#### Impact
If binding to the specified port fails (e.g., Address Already In Use) or if the server crashes unexpectedly at runtime, the parent context has no way of detecting the failure. The server terminates silently, and client connections will hang or refuse, while the rest of the daemon continues running in a degraded state without self-healing.

#### Remediation
Retain the `JoinHandle` within `ProjectionStreamServer`, and expose a health-check interface or provide a channel to signal task termination to the main event loop for graceful restarts.

---

### Finding 8: Dynamic Regex Compilation on Every Constraint Validation
*   **Severity**: Medium (Performance Bottleneck)
*   **Citation**: `crates/op-projection/src/schema_engine.rs:418-420`
*   **Vulnerability Type**: Resource Consumption

#### Description
When validating field-level constraints, the `SchemaValidator` compiles the regular expression pattern dynamically:

```rust
(Constraint::Pattern(pattern), Value::String(s)) => {
    let regex = Regex::new(pattern)
        .map_err(|_| anyhow::anyhow!("Invalid regex pattern: '{}'", pattern))?;
    if !regex.is_match(s) { ... }
}
```

#### Impact
Compiling regular expressions is a computationally expensive operation. Executing this step on every entity update and every field match degrades throughput. Furthermore, if a schema is registered with an invalid regex pattern in its constraints, the error is not caught during schema validation or registration. Instead, it is thrown at runtime during hot-path data updates, leading to quarantine actions.

#### Remediation
Validate and pre-compile regular expression constraints during the schema validation and registration phase (`validate_schema`). Store the compiled patterns in the registry to avoid on-the-fly compilation.

---

### Finding 9: Missing Validation During Schema Registration
*   **Severity**: High
*   **Citation**: `crates/op-projection/src/schema_engine.rs:122-126`
*   **Vulnerability Type**: Missing Input Validation

#### Description
The `register_schema` method accepts any `PluginSchema` and directly commits it to the active registry without executing `validate_schema`:

```rust
fn register_schema(&mut self, schema: PluginSchema) -> Result<u64> {
    let schema_name = schema.name.clone();
    let schema_version = schema.version.clone();

    // Check if schema is quarantined
    if let Some(reason) = self.quarantined.get(&schema_name) { ... }
    
    // Counter updates and direct map insertion follow
```

#### Impact
Malformed schemas (e.g., schemas with empty names, duplicate field names, invalid type parameters, or non-compilable regex patterns) can be committed to the engine. This corrupts the schema catalog and causes validation panics when the engine subsequently processes entities matching the malformed schemas.

#### Remediation
Enforce schema validation directly at the registration boundary:

```rust
fn register_schema(&mut self, schema: PluginSchema) -> Result<u64> {
    let validation = self.validate_schema(&schema)?;
    if !validation.valid {
        return Err(anyhow::anyhow!("Invalid schema definition: {:?}", validation.errors));
    }
    // Proceed with registration
}
```

---

### Finding 10: Heavy Thread and Runtime Allocation inside PluginReader
*   **Severity**: Medium (Resource Exhaustion / Thread Leaks)
*   **Citation**: `crates/op-projection/src/plugin_reader.rs:388-396`
*   **Vulnerability Type**: Improper Resource Management

#### Description
To bridge synchronous trait definitions (`SourceReader::read_all`) with asynchronous implementation details, the `SystemPluginReader` invokes a block-on helper:

```rust
fn block_on<F, T>(&self, future: F) -> Result<T>
where
    F: Future<Output = Result<T>>,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
        Err(_) => {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .context("failed to build a tokio runtime for plugin projection")?;
            runtime.block_on(future)
        }
    }
}
```

#### Impact
If called outside an active Tokio runtime, this function builds, starts, and tears down a completely new `current_thread` runtime *for every call*. This is highly resource-intensive and prone to OS resource exhaustion (such as thread and file descriptor limits) if triggered frequently. Conversely, if executed inside a single-threaded Tokio runtime, the `block_in_place` call will panic immediately.

#### Remediation
Refactor the `SourceReader` trait to native async definitions. Avoid spawning nested runtimes or utilizing blocking fallback runtimes.

---

### Finding 11: Blocking Reactor Threads with Sync Filesystem Operations
*   **Severity**: Medium (Reactor Starvation)
*   **Citation**: `crates/op-projection/src/bin/projection_server.rs:259`, `294`, and `299`
*   **Vulnerability Type**: Async Starvation (Reactor Blocking)

#### Description
In the main daemon loop, synchronous calls to `procfs_reader.read_all()` and `sled_reader.read_all()` are executed directly within the context of an async function (`async fn main` annotated with `#[tokio::main]`):

```rust
if procfs_reader.is_available() {
    initial_entities.extend(procfs_reader.read_all()?);
}
```

And inside the periodic update loop:
```rust
loop {
    tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    // ...
    if let Ok(entities) = procfs_reader.read_all() { ... }
}
```

The underlying `procfs` scans perform synchronous directory iteration (`fs::read_dir`) and file reading (`fs::read_to_string`).

#### Impact
Running intensive synchronous system file IO on the executor thread blocks the Tokio worker thread. This delays processing for other concurrent tasks scheduled on that thread (such as serving concurrent SSE requests, handling SSE keep-alives, or receiving incoming event streams).

#### Remediation
Offload synchronous system readers to the blocking pool using `tokio::task::spawn_blocking`:

```rust
let procfs_entities = tokio::task::spawn_blocking(move || {
    procfs_reader.read_all()
}).await??;
```