# License and Dependency Compliance Audit

## 1. License Field Extraction
- **Crate Name**: `op-projection`
- **Cargo.toml Location**: `crates/op-projection/Cargo.toml`
- **Extracted License**: `Apache-2.0` (Inherited from the workspace root `Cargo.toml` via `license.workspace = true`)

## 2. Cargo.lock Scan for Incompatible Crates (GPL/AGPL/SSPL)
A comprehensive scan of `Cargo.lock` was performed. No GPL, AGPL, or SSPL licensed crates were found in the dependency tree. All external dependencies conform to permissive licenses (such as MIT, Apache-2.0, or BSD).

## 3. Crates with No License Field
All local crates defined in the workspace (`op-dbus` and `op-projection`) specify a valid license field. There are no crates in the workspace lacking a license declaration.

---

# Security & Quality Findings

## 1. Critical Vulnerabilities

### Bypass of Sensitive Data Redaction (PII/Secrets Exposure)
- **Reference**: `crates/op-projection/src/access_control.rs:105-113`
- **Vulnerability Type**: CWE-201 (Insertion of Sensitive Information Into Sent Data), CWE-312 (Cleartext Storage/Transmission of Sensitive Information)
- **Description**: The `ProjectionAccessController` is designed to enforce access control and redact sensitive data (such as secrets and PII) based on configured schemas and policy rules. However, the `redact_sensitive` function is implemented as a stub that simply returns `data.clone()` without performing any modification:
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
- **Exploitability**: Directly exploitable. Any policy that specifies `redact_sensitive = true` (designed to filter passwords, private keys, or PII) will silently fail to redact anything. Any client querying these projections will receive unredacted sensitive values, completely bypassing intended security controls.

---

## 2. Schema-as-Code Discipline Violations

### Ad-Hoc Inline Schema Construction
- **Reference**: `crates/op-projection/src/bin/projection_server.rs:24-174`
- **Description**: Several system contracts (`system.memory`, `system.cpu`, `system.network`, `identity.sled`, `system.process`, `system.filesystems`) are defined programmatically as ad-hoc, inline Rust struct initializations rather than being loaded from versioned, declarative schema files (such as Protocol Buffers or versioned JSON schemas/OSCAL components).
- **Remediation**: Transition these hardcoded schemas into versioned, declarative schemas managed within a centralized registry, loading them dynamically from a schema-as-code repository.

### Hardcoded Ad-Hoc Nested Object Schema
- **Reference**: `crates/op-projection/src/plugin_reader.rs:131-177`
- **Description**: The schema for validating nested plugin object projections (`nested_object_projection_schema`) is defined directly as a hardcoded Rust function returning an ad-hoc `PluginSchema` struct:
  ```rust
  pub fn nested_object_projection_schema() -> PluginSchema { ... }
  ```
- **Remediation**: Migrate the schema to an external versioned schema file matching the system's schema-as-code guidelines.

---

## 3. High & Medium Risk Findings

### Shared Memory Concurrent Access & Data Race (Undefined Behavior)
- **Reference**: `crates/op-projection/src/sled_reader.rs:66-70`
- **Risk Level**: High
- **Description**: The `IdentitySledReader` accesses the shared memory pointer returned by `read_sled()` and casts it directly into a shared reference `&*ptr` without any atomic synchronization, locking, or memory barriers:
  ```rust
  let (ptr, _mmap) =
      read_sled().map_err(|e| anyhow::anyhow!("Failed to read sled: {}", e))?;
  let sled = unsafe { &*ptr };
  ```
  If another process or thread concurrently mutates the shared memory space while this process reads `sled.hashed_footprint` or `sled.wireguard_pubkey`, it constitutes a data race on non-atomic fields. In Rust, this violates the aliasing model and triggers Undefined Behavior, which can cause torn reads, silent corruption, or crashes.
- **Remediation**: Wrap shared memory structures in atomic fields or use robust cross-process synchronization (such as shared mutexes or semaphores) to guarantee memory safety.

### Unbounded Recursion on External Inputs (Potential Stack Overflow DoS)
- **Reference**: `crates/op-projection/src/plugin_reader.rs:224-282`
- **Risk Level**: Medium / High
- **Description**: The helper function `collect_nested_entities_recursive` parses plugin states recursively to extract nested entities. There is no limit on recursion depth:
  ```rust
  fn collect_nested_entities_recursive(
      entities: &mut Vec<RawEntity>,
      plugin_id: &str,
      parent_id: &str,
      path: &str,
      value: &Value,
      source: &str,
  ) { ... }
  ```
  If a plugin is fed with deeply nested JSON (either maliciously crafted or from a corrupted state source), this will consume the call stack and crash the entire process via a stack overflow.
- **Remediation**: Implement a maximum depth limit check (e.g., maximum depth of 16 or 32 levels) to abort recursion safely.

### Metric Leak: Infinite Increment of `client_count` in SSE Handler
- **Reference**: `crates/op-projection/src/json_stream.rs:133-138`
- **Risk Level**: Medium
- **Description**: In the `sse_handler` function, `client_count` and `total_clients` are incremented upon connection:
  ```rust
  state
      .client_count
      .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
  ```
  However, there is no corresponding `fetch_sub` when the client stream terminates or drops. While `ProjectionStreamServer::disconnect_client` exists to decrement the count, it is never invoked by the Axum SSE handler. This causes the reported client count metric to monotonically increase forever, leaking resources/metrics.
- **Remediation**: Wrap the client connection lifecycle in a custom stream wrapper or a drop guard that decrements `client_count` when the future or stream is dropped.

---

## 4. Quality & Performance Issues

### Dynamic Regex Compilation in Hot Loops
- **Reference**: `crates/op-projection/src/access_control.rs:48`, `crates/op-projection/src/access_control.rs:72`, and `crates/op-projection/src/schema_engine.rs:251`
- **Risk Level**: Low / Quality
- **Description**: Regular expressions are compiled dynamically on every invocation of `enforce_policy`, `validate_permissions`, and `validate_constraints`:
  ```rust
  let re = Regex::new(&policy.resource_pattern)?; // access_control.rs
  ```
  ```rust
  let regex = Regex::new(pattern).map_err(...); // schema_engine.rs
  ```
  Compiling a regular expression is a computationally heavy operation. Doing this dynamically on every permission check or constraint validation dramatically reduces throughput and can easily be abused to cause high CPU usage (Denial of Service).
- **Remediation**: Compile regular expressions once when policies or schemas are loaded, and store the compiled `Regex` object in memory.

### Fragile XML Parsing of Introspection Output
- **Reference**: `crates/op-projection/src/dbus_reader.rs:49-62`
- **Risk Level**: Low / Quality
- **Description**: Introspection results from D-Bus are processed using a naive string-line scanning technique:
  ```rust
  for line in xml.lines() {
      if line.contains("<node name=\"") { ... }
  }
  ```
  This ad-hoc parsing is highly fragile. Variations in whitespace, attribute ordering, or namespaces in valid XML will cause the parser to fail to discover D-Bus paths or produce corrupted entity IDs.
- **Remediation**: Use a compliant, lightweight XML pull parser (such as `quick-xml`) to parse the introspection nodes safely.

### Non-Monotonic Clock Used for Latency Measurement
- **Reference**: `crates/op-projection/src/event_materializer.rs:47` and `crates/op-projection/src/event_materializer.rs:118`
- **Risk Level**: Low / Quality
- **Description**: The event materializer measures latency using `Utc::now()` (system wall clock time):
  ```rust
  let start_time = Utc::now();
  ...
  self.last_latency = Utc::now().signed_duration_since(start_time);
  ```
  System clocks are non-monotonic and can jump backwards or forwards due to NTP synchronization or manual time adjustments. This can cause negative latency values or false trigger warnings.
- **Remediation**: Replace `Utc::now()` with `tokio::time::Instant::now()` or `std::time::Instant::now()`, which are guaranteed to be monotonic.

### Unreliable Source Filtering by Substring Matching
- **Reference**: `crates/op-projection/src/projection_engine.rs:195-202`
- **Risk Level**: Low / Quality
- **Description**: Filtering projections by source relies on a simple substring match on the projection's ID:
  ```rust
  self.store
      .get_all()
      .into_iter()
      .filter(|p| p.id.contains(source))
      .collect()
  ```
  If `source` is "dbus", it will match `entity_type:entity_id` values like `plugin:my-dbus-device`, resulting in incorrect filtering and projection leakage across sources.
- **Remediation**: Add a dedicated `source` string field to the `Projection` model, and filter strictly on that field.