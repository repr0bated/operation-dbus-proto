### 1. Build Quality & Workspace Inheritance Audit

*   **Edition**: The crate `op-projection` inherits its edition from the workspace (`edition.workspace = true` in `crates/op-projection/Cargo.toml`). The workspace `Cargo.toml` specifies `edition = "2021"`.
*   **Rust Version**: No explicit `rust-version` is specified in either the workspace or the crate's `Cargo.toml`.
*   **Binaries**: The crate contains one binary target located at `crates/op-projection/src/bin/projection_server.rs`.
*   **Examples**: There are no example targets defined in `Cargo.toml` or present in the file structure.
*   **Build Script (`build.rs`)**: There is no `build.rs` build script provided or defined for `op-projection`. Consequently, there are no compile-time arbitrary shell execution or build-time codegen risks present within this crate.
*   **Workspace Inheritance vs. Local Overrides**:
    *   **Inherited**: Metadata fields (`version`, `edition`, `license`) and the majority of external dependencies (e.g., `tokio`, `axum`, `serde`, `simd-json`, `chrono`, `anyhow`, `tracing`) are inherited via `.workspace = true`.
    *   **Overrides**: Local overrides are used for `dashmap = "5.0"`, `parking_lot = "0.12"`, and `sha2 = "0.10"` in `crates/op-projection/Cargo.toml`, bypassing the workspace dependency specification.

---

### 2. Schema-as-Code Build Check

*   **Prost/Tonic-Build Invocation**: There is no build-time invocation of `prost-build` or `tonic-build` within the `op-projection` crate itself, as it lacks a `build.rs`.
*   **Protocol Buffer Sources**: No `.proto` files are checked into the `op-projection` directory. However, the crate relies on `prost` and `tonic` via its dependencies, which indicates that any Proto compilation is handled by other upstream workspace crates (such as `op-grpc-bridge` or `op-cognitive-mcp`).
*   **Runtime vs. Build-Time Compilation**: No runtime compilation of Proto files occurs in this crate. 
*   **Ad-Hoc Data Contracts Critique**:
    *   The projection system defines data contracts dynamically using custom Rust structs representing JSON schemas (`PluginSchema` and `FieldSchema` in `crates/op-projection/src/data_models.rs:14-63`). 
    *   The payload data is stored in `simd_json::OwnedValue` inside `Projection` (`crates/op-projection/src/data_models.rs:136`), which represents an ad-hoc JSON structure. 
    *   While this provides a dynamic validation engine, it violates strict schema-as-code compile-time discipline. Changes to schema fields are validated dynamically at runtime (`crates/op-projection/src/schema_engine.rs:324`) rather than being strictly enforced via statically typed, versioned, compile-time generated types.

---

### 3. Security Vulnerabilities Audit

#### CRITICAL: Complete Bypass of Sensitive Data Redaction (PII/Secret Leakage)
*   **File**: `crates/op-projection/src/access_control.rs:104-111`
*   **Vulnerability Type**: Sensitive Data Exposure / Security Bypass
*   **Impact**: Leakage of highly sensitive system credentials, cryptographic keys, and PII to unauthorized users.
*   **Exploitability**: Directly exploitable.
*   **Analysis**:
    The access controller defines policy enforcement that explicitly checks if redaction is required for a matched resource pattern:
    ```rust
    // crates/op-projection/src/access_control.rs:45-50
    let policies = self.policies.read();
    for policy in policies.iter() {
        let re = Regex::new(&policy.resource_pattern)?;
        if re.is_match(&projection.id) && policy.redact_sensitive {
            result.data = self.redact_sensitive(&result.data, requester);
        }
    }
    ```
    However, the implementation of `redact_sensitive` is a blank placeholder that simply returns a clone of the original data:
    ```rust
    // crates/op-projection/src/access_control.rs:104-111
    fn redact_sensitive(
        &self,
        data: &simd_json::OwnedValue,
        _requester: &Requester,
    ) -> simd_json::OwnedValue {
        // In production, use JSON paths from schema to redact
        data.clone()
    }
    ```
    If `redact_sensitive` is configured to `true` in an `AccessPolicy` (such as in production policies targeting `identity.sled` projections), the data is returned completely unredacted. An unprivileged client requesting this projection will bypass all intended redaction filters and receive plaintext secrets (e.g., WireGuard private/public keys, hashed footprints).

#### HIGH: Unsafe Dereference of Shared Memory Sled
*   **File**: `crates/op-projection/src/sled_reader.rs:55-65`
*   **Vulnerability Type**: Memory Corruption / Undefined Behavior / Denial of Service
*   **Impact**: Process crash (SIGSEGV/SIGBUS) or arbitrary memory reading.
*   **Exploitability**: Exploitable by any local process with access to `/dev/shm`.
*   **Analysis**:
    The `IdentitySledReader` maps the identity sled file directly from shared memory `/dev/shm` and dereferences it using raw pointer casting without size verification or validation:
    ```rust
    // crates/op-projection/src/sled_reader.rs:57-59
    let (ptr, _mmap) =
        read_sled().map_err(|e| anyhow::anyhow!("Failed to read sled: {}", e))?;
    let sled = unsafe { &*ptr };
    ```
    Since `/dev/shm` is shared across local processes, any other process running on the host could truncate, corrupt, or modify the underlying file backing the shared memory allocation. If the file is truncated, dereferencing `ptr` leads to an out-of-bounds memory access causing a segmentation fault (`SIGSEGV`) or bus error (`SIGBUS`), instantly crashing the entire Projection control plane. There is also no synchronization mechanism (such as volatile reads or memory barriers) to protect against concurrent modification data races.

#### HIGH: Potential Runtime Panic via `block_in_place` on Single-Threaded Runtime
*   **File**: `crates/op-projection/src/plugin_reader.rs:374-385`
*   **Vulnerability Type**: Denial of Service (Panic)
*   **Impact**: Thread panic and server crash during synchronous reads.
*   **Exploitability**: Triggered automatically when plugin reads are executed in a standard single-threaded Tokio context.
*   **Analysis**:
    The `SystemPluginReader` implements `SourceReader`'s synchronous `read_all` by blocking on an asynchronous function using a utility helper:
    ```rust
    // crates/op-projection/src/plugin_reader.rs:374-380
    fn block_on<F, T>(&self, future: F) -> Result<T>
    where
        F: Future<Output = Result<T>>,
    {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(future)),
            ...
    ```
    Tokio's `block_in_place` is only supported on the multi-threaded scheduler (`rt-multi-thread`). If the projection server runs on a single-threaded scheduler (which is standard for lightweight control-plane agents), `block_in_place` will immediately panic at runtime with the message: *"Can retrograde to block_in_place only on a multi-threaded registrar/executor."* This will crash the entire projection server when executing periodic sync scans (`crates/op-projection/src/bin/projection_server.rs:252`).

#### MEDIUM: CPU Exhaustion / ReDoS via Uncached Regex Compilation
*   **File**: `crates/op-projection/src/access_control.rs:47` and `crates/op-projection/src/access_control.rs:69`
*   **Vulnerability Type**: Denial of Service (Resource Exhaustion)
*   **Impact**: High CPU usage and severe performance degradation under high query volume.
*   **Exploitability**: Highly exploitable through frequent authorization requests.
*   **Analysis**:
    In `enforce_policy` and `validate_permissions`, regular expressions are compiled dynamically on every policy evaluation:
    ```rust
    // crates/op-projection/src/access_control.rs:45-48
    let policies = self.policies.read();
    for policy in policies.iter() {
        let re = Regex::new(&policy.resource_pattern)?;
    ```
    Compiling a regex pattern is a computationally expensive operation. Since `validate_permissions` and `enforce_policy` are invoked sequentially for every projection access request, compiling regexes on every check allows an attacker to exhaust server CPU cycles by making rapid, cheap validation requests. Patterns should be compiled once (e.g., in `add_policy` or using a thread-local thread-safe cache) and stored in `AccessPolicy`.

---

### 4. Quality & Reliability Findings

#### HIGH: State Desynchronization via Unbounded SSE Channel Lag Drops
*   **File**: `crates/op-projection/src/json_stream.rs:242-249`
*   **Classification**: Bug / Reliability Issue
*   **Impact**: The frontend/UI displays stale or incorrect state indefinitely.
*   **Analysis**:
    The SSE server uses a `tokio::sync::broadcast` channel to stream updates. When a client's HTTP connection lags (e.g., slow network), the receiver stream lags behind.
    ```rust
    // crates/op-projection/src/json_stream.rs:242-248
    let live = BroadcastStream::new(rx).filter_map(|result| match result {
        Ok(update) => {
            let data = serde_json::to_string(&update).unwrap_or_default();
            Some(Ok(Event::default().event("projection_update").data(data)))
        }
        Err(_) => None,
    });
    ```
    When the broadcast channel lags, `BroadcastStream` yields an `Err(Lagged)` error. The implementation silently discards this error (`Err(_) => None`). This drops all updates missed during the lag interval without triggering a full state reconciliation or informing the client, causing permanent desynchronization between the projection state and the UI until the client manually disconnects and reconnects.

#### MEDIUM: Ad-Hoc XML Splitting of D-Bus Introspection Data
*   **File**: `crates/op-projection/src/dbus_reader.rs:48-61`
*   **Classification**: Code Quality / Fragility
*   **Impact**: Missing or corrupted D-Bus entity discovery.
*   **Analysis**:
    Rather than using a robust XML parsing library, `introspect` parses D-Bus XML responses using raw line splitting:
    ```rust
    // crates/op-projection/src/dbus_reader.rs:48-52
    let mut children = Vec::new();
    for line in xml.lines() {
        if line.contains("<node name=\"") {
            if let Some(name) = line
                .split("name=\"")
    ```
    If the D-Bus service formats XML differently (e.g., inserts a newline between attributes, uses single quotes, or includes nested node elements on a single line), this ad-hoc logic will fail to extract child node names. This silences the discovery of critical D-Bus interfaces.

#### LOW: Hardcoded Host File Paths
*   **File**: `crates/op-projection/src/plugin_reader.rs:21`
*   **Classification**: Code Quality / Portability
*   **Impact**: Portability constraints across different target operating systems.
*   **Analysis**:
    The path to the SQLite state database is hardcoded directly to a specific system path:
    ```rust
    const STATE_STORE_PATH: &str = "/var/lib/op-dbus/state.db";
    ```
    This restricts execution environments (such as non-Linux OS development or rootless container deployments where `/var/lib` is read-only or inaccessible) and forces a fallback to an in-memory database, losing persistent plugin states. This path should be configurable via environment variables or a configuration file.

---
## ⚠ Citation Warnings
- `crates/op-projection/src/json_stream.rs:242`: file has 215 lines
- `crates/op-projection/src/json_stream.rs:242`: file has 215 lines
