### 1. Workspace Integration & Dependencies

#### Crates Depending on `op-projection`
Based on the workspace `Cargo.toml`, the following crate explicitly depends on `op-projection`:
*   **`op-dbus`** (root package name configured as `"op-dbus"` in `Cargo.toml`) via:
    ```toml
    op-projection.workspace = true
    ```

#### D-Bus Service Names and Object Paths Registered
*   **None.** No D-Bus service names or object paths are registered by the `op-projection` crate itself.
*   The D-Bus interface is accessed solely as a client/consumer via `SystemDbusReader` in `crates/op-projection/src/dbus_reader.rs`, which queries external interfaces dynamically using `IntrospectableProxy` (`crates/op-projection/src/dbus_reader.rs:40-44`).

#### HTTP/gRPC Endpoints Exposed
*   **HTTP (Server-Sent Events) Endpoint:**
    *   **Path:** `GET /events` (`crates/op-projection/src/json_stream.rs:90`)
    *   **Port:** Configured to listen on port `8082` by default in the server binary (`crates/op-projection/src/bin/projection_server.rs:271`).
    *   **Protocol:** Server-Sent Events (SSE) streaming updates via Axum.
*   **gRPC Endpoints:**
    *   **None.** `SystemGrpcReader` (`crates/op-projection/src/grpc_reader.rs:70`) serves as a client-side discovery mechanism and does not bind or listen as a gRPC server.

#### Cross-Crate Circular Dependency Risks
*   **High Structural Coupling:** `op-projection` depends on a large portion of the workspace crates (`op-core`, `op-state`, `op-state-store`, `op-plugins`, `op-dbus-mirror`, `op-grpc-bridge`, `op-blockchain`, `op-identity`) while the main control plane orchestrator `op-dbus` concurrently depends on `op-projection`. 
*   **Circular Architecture Pattern:** `op-projection` consumes `op-plugins` and `op-state-store` directly to project live state (`crates/op-projection/src/plugin_reader.rs:16-24`). However, plugins often require configuration/projections to execute. If a plugin in `op-plugins` attempts to import `op-projection` to query the materialized engine state, a hard circular dependency compile-time error will occur.

---

### 2. Schema-as-Code Discipline Assessment

The codebase claims to enforce a strict "Schema-as-Code Authority" discipline using `PluginSchema` as the single source of truth. However, several critical gaps undermine this:

1.  **Dynamic/Ad-hoc Payload Data Representation:**
    *   The internal payload of all projections (`Projection.data`, `RawEntity.data`, `ValidationError.example`) is defined as `simd_json::OwnedValue` (essentially an untyped dynamic JSON map) in `crates/op-projection/src/data_models.rs:163`.
    *   Data contracts are passed as unstructured, dynamic payloads rather than compile-time versioned serialization models (e.g., Protobuf structs or auto-generated Rust types from OSCAL/JSON Schema documents).
2.  **Hardcoded Bootstrapping in Server Binary:**
    *   In `crates/op-projection/src/bin/projection_server.rs:24-219`, schemas for memory, CPU, network, filesystems, and process entities are defined ad-hoc inside the Rust code itself by manually instantiating `PluginSchema` structs, violating the principle of schemas acting as independent, versioned artifacts stored external to the runtime code.

---

### 3. Production Security & Quality Audit

#### [Critical] Unsafe Shared Memory Raw Pointer Dereference in `IdentitySledReader`
*   **Citations:** `crates/op-projection/src/sled_reader.rs:59-67`
*   **Impact:** Memory Corruption / Arbitrary Code Execution / Privilege Escalation.
*   **Description:** The shared memory reader reads directly from `/dev/shm` (`SHM_SLED_PATH`) using `read_sled()`, returning a raw pointer. The code immediately dereferences this raw pointer without validating alignment, checking for null, or establishing read/write synchronization:
    ```rust
    let (ptr, _mmap) =
        read_sled().map_err(|e| anyhow::anyhow!("Failed to read sled: {}", e))?;
    let sled = unsafe { &*ptr };
    ```
    If another process truncates, modifies, or corrupts the shared memory file, this raw dereference results in undefined behavior (UB), immediate memory access violation crashes, or potential exploitation if an attacker can manipulate the shared memory layout.
*   **Mitigation:** Replace raw unsafe pointer dereferencing with a structured, verified memory mapping library that uses safe casting, boundary validation, and robust read synchronization (e.g., using robust inter-process mutexes or volatile reads).

---

#### [High] Secret and PII Redaction Bypass
*   **Citations:** `crates/op-projection/src/access_control.rs:114-118`
*   **Impact:** Information Disclosure (Leaking of Private Keys, Passwords, and PII).
*   **Description:** The access controller claims to enforce policy-based redaction of sensitive fields. However, `redact_sensitive` is implemented as a silent no-op placeholder:
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
    Any policy configured with `redact_sensitive: true` (`crates/op-projection/src/access_control.rs:51`) will silently fail to redact sensitive data, exporting cleartext credentials and PII to unauthorized requesters.
*   **Mitigation:** Implement the JSON path-based redaction engine immediately, querying `PluginSchema.secret_paths` and `PluginSchema.pii_paths` to mask or purge designated fields before returning the payload.

---

#### [High] Regular Expression Denial of Service (ReDoS) in Hot Paths
*   **Citations:** `crates/op-projection/src/access_control.rs:49`, `crates/op-projection/src/access_control.rs:77`, `crates/op-projection/src/schema_engine.rs:414`
*   **Impact:** Denial of Service / Thread Pool Exhaustion / High CPU Utilization.
*   **Description:** The policy enforcement engine and schema validator compile regular expressions dynamically on *every single request* or *every constraint check* without caching:
    ```rust
    // crates/op-projection/src/access_control.rs:49
    let re = Regex::new(&policy.resource_pattern)?;

    // crates/op-projection/src/schema_engine.rs:414
    let regex = Regex::new(pattern)
        .map_err(|_| anyhow::anyhow!("Invalid regex pattern: '{}'", pattern))?;
    ```
    If a malicious user or plugin registers a schema/policy containing a complex regular expression with nested quantifiers (e.g., `(a+)+`), validating any string against it will cause catastrophic backtracking, locking up the CPU thread indefinitely.
*   **Mitigation:** Compile and cache regular expressions when policies or schemas are registered, rather than compiling them on-the-fly during hot path evaluations. Reject patterns containing dangerous syntax during registration.

---

#### [Medium] Ad-hoc Fragile XML Introspection Parsing
*   **Citations:** `crates/op-projection/src/dbus_reader.rs:48-62`
*   **Impact:** Security Policy Bypass / Logic Subversion.
*   **Description:** The D-Bus introspection logic parses raw XML elements using naive line and substring matching:
    ```rust
    for line in xml.lines() {
        if line.contains("<node name=\"") {
            if let Some(name) = line
                .split("name=\"")
                .nth(1)
                .and_then(|s| s.split('\"').next())
    ```
    An attacker controlling a D-Bus service can return highly malformed or crafted XML containing comments, attribute-scrambling, or nested namespaces that bypass this parser or trick it into mapping incorrect parent-child relationships, subverting system tracking.
*   **Mitigation:** Utilize a robust, safe XML pull-parser (such as `quick-xml`) to cleanly navigate the DOM nodes of the D-Bus introspection schema.

---

#### [Medium] Thread Pool Starvation via Synchronous Blocking of Async Context
*   **Citations:** `crates/op-projection/src/plugin_reader.rs:360-372`
*   **Impact:** Extreme Latency Spikes / Server Hanging.
*   **Description:** The plugin reader utilizes a helper function `block_on` to run asynchronous tasks within synchronous trait implementations:
    ```rust
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
        Err(_) => {
            let runtime = tokio::runtime::Builder::new_current_thread() ...
    ```
    Using `block_on` inside an active Tokio worker pool can cause deadlocks if the thread pool is saturated, or cause severe thread starvation, completely breaking the 50ms latency guarantees expected by the event materializer.
*   **Mitigation:** Re-architect the traits in `crates/op-projection/src/interfaces.rs` to be natively `async trait` (which is already imported and supported in the workspace).

---

#### [Low] In-Memory Only Access Control Policies & Audits
*   **Citations:** `crates/op-projection/src/access_control.rs:18-21`
*   **Impact:** Audit Trail Tampering / Security Configuration Loss on Restart.
*   **Description:** Security policies and access control decision logs are stored in standard in-memory vectors:
    ```rust
    policies: Arc<RwLock<Vec<AccessPolicy>>>,
    audit_trail: Arc<RwLock<Vec<AccessControlAudit>>>,
    ```
    A crash, restart, or administrative refresh of the projection server completely wipes the security logs and any dynamically added policies, leaving no persistent record of historical access or denials.
*   **Mitigation:** Forward audit trails to a structured persistent storage engine (such as `op-state-store` or a dedicated local sqlite/journald endpoint) immediately upon generation.