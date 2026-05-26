# OP-PROJECTION: Security & Quality Audit Report

## Architecture & Module Map

### Overview
`op-projection` is a schema-validated state transformation engine designed to ingest live, authoritative state from diverse sources (such as D-Bus, gRPC, procfs, and dynamic runtime plugins) and project this state into optimized, queryable, and version-controlled data models. It is built to serve dashboards, orchestration tools, audit trails, and topology mapping with a strict materialization constraint (50ms processing latency).

### Module Tree
The crate is structured into the following hierarchy:
* **`lib.rs`**: Core library entry point that exposes the public APIs, interfaces, and concrete implementations.
* **`data_models.rs`**: Defines the in-memory representation of schemas (`PluginSchema`), fields, states, relationships, historical logs, events, and updates.
* **`interfaces.rs`**: Specifies the abstraction traits for the registry, validator, projection engine, SSE server, access controller, audit trail, and various source readers.
* **`schema_engine.rs` / `schema_validator.rs`**: Implements the authoritative schema registry, verification checks, and type/constraint enforcement engines.
* **`projection_engine.rs`**: Orchestrates state conversion, transforming raw entities into validated projections.
* **`projection_store.rs`**: Direct memory management layer using `DashMap` for zero-copy concurrent lookup and version tracking.
* **`access_control.rs`**: Implements basic role-based permissions, pattern-matching rules, and data redaction hooks.
* **`json_stream.rs`**: SSE (Server-Sent Events) delivery mechanism implemented via Axum to stream live state updates to connected clients.
* **Source Readers**:
  * `dbus_reader.rs`: zbus-based system bus introspection.
  * `grpc_reader.rs`: tonic-based gRPC reflection scanner.
  * `procfs_reader.rs`: Direct `/proc` state scraper.
  * `sled_reader.rs`: Direct zero-copy reader scanning shared memory `/dev/shm/sled`.
  * `plugin_reader.rs`: Dynamic schema converter interfacing with the `op-state-store` runtime SQLite database.

### Entry Points
* **Library**: `crates/op-projection/src/lib.rs`
* **Binary Target**: `crates/op-projection/src/bin/projection_server.rs` — Wires together the engines, begins periodic polling, and hosts the real-time SSE server on port `8082`.

---

## Critical Production Security Findings

### 1. Unauthenticated Axum SSE Endpoint Bypasses Access Control & Leaks All Secrets
* **File**: `crates/op-projection/src/json_stream.rs`
* **Lines**: 114–116 (`Router` definition) & 181–224 (`sse_handler`)
* **Vulnerability Type**: Broken Authentication and Access Control (CWE-284 / CWE-306)
* **Exploitability**: **Directly Exploitable**

#### Analysis
In `projection_server.rs` (line 214), the SSE streaming server is initialized and bound to the wildcard address:
```rust
stream_server.start(8082)?;
```
Looking at `json_stream.rs` (lines 114–116), the Axum router registers the endpoint `/events` with absolutely no security middleware:
```rust
let app = Router::new()
    .route("/events", get(sse_handler))
    .with_state(state);
```
Inside the `sse_handler` (lines 181–224), any client connecting to the route is immediately served the complete, unredacted snapshot of all active projections in memory:
```rust
let mut snapshot = state
    .snapshot
    .iter()
    .map(|projection| projection.value().clone())
    .collect::<Vec<_>>();
```
The client is then subscribed to the broadcast channel to receive real-time state updates. 

This completely bypasses the security controls defined in `access_control.rs`. The `AccessController::enforce_policy` and `validate_permissions` logic is never invoked when clients connect or when updates are broadcasted. Any network attacker can simply connect to `http://<server-ip>:8082/events` and scrape all sensitive projections, including the WireGuard private/public keys, hashed footprints, and system process structures.

#### Remediation
1. Apply an authentication middleware (e.g., Bearer JWT or mutual TLS checks) to the Axum route in `json_stream.rs`.
2. Do not broadcast raw projections directly to the raw socket. For each connected client, the stream must yield updates that have been passed through `AccessController::enforce_policy` tailored to that specific client's authenticated `Requester` context.

---

### 2. Sensitive Data Redaction is a No-Op
* **File**: `crates/op-projection/src/access_control.rs`
* **Lines**: 125–131 (`redact_sensitive`)
* **Vulnerability Type**: Sensitive Data Exposure (CWE-200)
* **Exploitability**: **Directly Exploitable**

#### Analysis
The `ProjectionAccessController` is responsible for evaluating policies and removing sensitive fields (secrets, PII) when the matching policy dictates `policy.redact_sensitive == true` (line 55). However, the implementation of `redact_sensitive` is a hardcoded placeholder that simply clones and returns the input data:
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
If any component relies on `AccessController::enforce_policy` to strip sensitive information before exporting projections, the secrets will remain entirely intact and visible, leading to critical exposure of credentials and PII.

#### Remediation
Implement recursive redaction in `redact_sensitive` using the `secret_paths` and `pii_paths` arrays declared on the authoritative `PluginSchema` (defined in `data_models.rs`). Walk the `simd_json::OwnedValue` object tree and replace matching JSON pointer paths with masked or null values.

---

## High & Medium Severity Security Findings

### 3. Unbounded Memory Growth on Audit Logs & Event History (Denial of Service)
* **File**: `crates/op-projection/src/access_control.rs` & `crates/op-projection/src/projection_store.rs`
* **Lines**: `access_control.rs:149` & `projection_store.rs:43–46`
* **Vulnerability Type**: Resource Exhaustion (CWE-400)
* **Exploitability**: High (Requires prolonged system activity)

#### Analysis
The projection server maintains an active memory footprint of all access control decisions and historic state transitions:
1. In `access_control.rs` (line 149), every authorization decision appends directly to an unrestricted vector:
   ```rust
   self.audit_trail.write().push(audit);
   ```
2. In `projection_store.rs` (lines 43–46), every call to `upsert` appends a copy of the old projection to a vector tracking state history:
   ```rust
   self.history
       .entry(id.clone())
       .or_insert_with(Vec::new)
       .push(historical);
   ```
Neither of these structures has a maximum capacity limit, time-to-live (TTL) eviction, or persistent database offloading. In high-velocity environments where system stats (CPU, memory, processes) are polled and updated frequently, the heap will steadily leak memory until the process is terminated by the kernel's Out-Of-Memory (OOM) killer.

#### Remediation
* Implement a sliding-window ring buffer or bounded queue for the `audit_trail` and `history` collections.
* Periodically flush old entries to the persistent `SqliteStore` or a dedicated log file, and prune the in-memory cache.

---

### 4. Unsafe Raw Pointer Dereference of Shared Memory Without Validation
* **File**: `crates/op-projection/src/sled_reader.rs`
* **Lines**: 57–61 (`read_sled_entity`)
* **Vulnerability Type**: Unvalidated Memory Access / Buffer Overflow (CWE-119 / CWE-822)
* **Exploitability**: Medium (Requires localized privilege to modify shared memory)

#### Analysis
The `IdentitySledReader` extracts state from a shared memory sled hosted in `/dev/shm`:
```rust
let (ptr, _mmap) =
    read_sled().map_err(|e| anyhow::anyhow!("Failed to read sled: {}", e))?;
let sled = unsafe { &*ptr };
```
This dereference is wrapped inside an `unsafe` block but performs no structural verification on the memory pointed to by `ptr`. If the shared memory segment is corrupted, truncated by another process, or malformed, dereferencing this pointer will cause a segmentation fault (crashing the server) or lead to undefined memory behavior.

#### Remediation
Introduce structural validation prior to dereferencing the pointer. Ensure the shared memory segment contains a magic byte header, a structural checksum (e.g., CRC32 or Blake3), and verify that the mapped length matches the expected memory size of `IdentitySled`.

---

### 5. High Lock Contention on Hot Path Violating Latency Guarantees
* **File**: `crates/op-projection/src/ovsdb_mirror.rs`
* **Lines**: 43–52 (`handle_disconnect`)
* **Vulnerability Type**: Thread Block / Performance Bottleneck (CWE-400 / CWE-833)
* **Exploitability**: Medium

#### Analysis
The `ProjectionMaterializer` is designed with a strict processing SLA: event processing latency must remain under 50ms, with warnings logged if this is exceeded (`event_materializer.rs` line 124). 
However, in `ovsdb_mirror.rs` (lines 43–52), `handle_disconnect` locks the entire projection engine:
```rust
let mut engine = self.engine.lock();
for p in engine.get_all_projections() {
    if p.entity_type.starts_with("ovsdb.") {
        engine.degrade_projection(&p.id, "OVSDB connection lost", Vec::new());
    }
}
```
This loop performs a synchronous `get_all_projections()`, clones the entire set, and sequentially invokes `degrade_projection` (which internally executes write-locks on the underlying `DashMap` storage). If the system is managing large environments (e.g., thousands of virtual switches/ports), this operation will completely block the projection engine, starving the `ProjectionMaterializer` and causing it to violate its 50ms real-time latency guarantee.

#### Remediation
Avoid holding the heavy global lock during long loops. Instead, retrieve only the IDs of the relevant projections, release the lock, and then update their states in chunks, or process state degradation asynchronously using background tasks.

---

### 6. Fragile XML Parsing of Introspection Data
* **File**: `crates/op-projection/src/dbus_reader.rs`
* **Lines**: 45–66 (`introspect`)
* **Vulnerability Type**: Improper Input Validation (CWE-20)
* **Exploitability**: Medium (Can result in missing or malformed entities)

#### Analysis
The `SystemDbusReader` parses the XML string returned from D-Bus introspection using an ad-hoc line-by-line split mechanism:
```rust
let mut children = Vec::new();
for line in xml.lines() {
    if line.contains("<node name=\"") {
        if let Some(name) = line
            .split("name=\"")
            .nth(1)
            .and_then(|s| s.split('\"').next())
        { ... }
    }
}
```
This parsing strategy is highly fragile. XML does not guarantee element formatting on a single line, and attributes can be declared in arbitrary orders, with variable whitespace, or using single quotes (`'`) instead of double quotes (`"`). Under any of these non-standard formatting scenarios, the parser will fail to discover nodes, leaving child entities silently unprojected.

#### Remediation
Refactor the parsing logic to use a proper, streaming XML pull parser. The workspace already includes a dependency on `quick-xml` (as seen in `Cargo.toml`), which should be leveraged here to robustly parse the D-Bus introspection tags.

---

### 7. Inefficient Re-compilation of Regular Expressions on Hot Path
* **File**: `crates/op-projection/src/access_control.rs`
* **Lines**: 52 & 70 (`Regex::new`)
* **Vulnerability Type**: Inefficient Resource Management / CPU Exhaustion (CWE-400)
* **Exploitability**: Medium (High performance overhead)

#### Analysis
In the access controller's permission checker, the policy resource patterns are compiled on every single access request:
```rust
let re = Regex::new(&policy.resource_pattern)?;
```
This occurs inside `validate_permissions` and `enforce_policy`. For a high-frequency system validating hundreds of projections per second against multiple access control policies, repeatedly compiling identical regular expressions is a severe performance bottleneck.

#### Remediation
Compile the `Regex` instance once when the `AccessPolicy` is registered or added to the controller via `add_policy` (line 159). Store the compiled `Regex` directly inside the `AccessPolicy` struct to avoid any runtime compilation overhead.

---

## Schema-as-Code Violations

The codebase is governed by a strict "Schema-as-Code" discipline using Protocol Buffers and OSCAL. All data contracts and entity definitions must be expressed via versioned, declarative schemas. The following instances violate this discipline by defining contracts as ad-hoc Rust structs, hardcoded configurations, or imperatively built models.

### Violation 1: Ad-Hoc Struct Definition of Core Schemas & Events
* **File**: `crates/op-projection/src/data_models.rs`
* **Lines**: 16–33 (`PluginSchema`), 36–52 (`FieldSchema`), 115–144 (`Projection`), 207–228 (`AuditProjection`), 284–303 (`ProjectionEvent`)

#### Description
Instead of compiling schemas and data contracts from unified Protocol Buffers (`.proto` files), the workspace defines its metadata and transfer models as hand-written Rust structures decorated with ad-hoc `serde` serialization annotations. This deviates from a single-source-of-truth approach, leading to schema drift when shared across platform boundaries or utilized by non-Rust microservices.

#### Remediation
Define `PluginSchema`, `FieldSchema`, `Projection`, and `ProjectionEvent` structures inside a Protobuf schema file. Use `prost` and `tonic-build` to generate compile-time Rust structures automatically.

---

### Violation 2: Imperative In-Code Schema Construction
* **File**: `crates/op-projection/src/bin/projection_server.rs`
* **Lines**: 27–210

#### Description
The server's bootstrap sequence registers several foundational system schemas (e.g., `system.memory`, `system.cpu`, `system.network`, `identity.sled`, `system.process`, and `system.filesystems`) by manually constructing the structures inside Rust code:
```rust
let memory_schema = PluginSchema {
    name: "system.memory".to_string(),
    version: "1.0.0".to_string(),
    fields: vec![ ... ],
    ...
};
```
Defining data contracts procedurally in the binary initialization logic violates the Schema-as-Code principle. Schemas must be maintained in versioned file catalogs rather than embedded inside executable code.

#### Remediation
Extract all schemas out of `projection_server.rs` and save them as declarative JSON Schema or OSCAL catalog files. Update the server bootstrap to load, validate, and register these schemas from a designated configuration directory (e.g., `/etc/op-projection/schemas/`) at startup.

---

### Violation 3: Manual Schema Conversion and Translation Layers
* **File**: `crates/op-projection/src/plugin_reader.rs`
* **Lines**: 426–476 (`convert_schema`, `convert_field`, `convert_field_type`, `convert_constraint`)

#### Description
Because there is no unified schema representation between the runtime plugins (`op-state-store`) and the projection system, the codebase relies on custom, hand-coded mapping logic to translate fields, types, and constraints between the two definitions. This ad-hoc translation logic is error-prone, hard to maintain, and bypasses formal schema-checking tools.

#### Remediation
Unify the schema definitions across the `op-state-store` and `op-projection` crates using a single shared Proto-derived schema model, eliminating the need for custom translation logic.