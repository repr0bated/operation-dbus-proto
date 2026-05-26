# D-Bus & IPC Attack Surface Audit

## Registered D-Bus Interfaces, Methods, and Signals

This codebase does **not** register or export any native D-Bus interfaces, methods, or signals in the provided files. No `#[dbus_interface]` attributes or D-Bus XML registration blocks are declared. 

Instead, the `op-projection` system acts exclusively as a D-Bus client via the `SystemDbusReader` struct, which connects to the **system bus** to discover and query other registered system services.

### Bus Connectivity
- **Bus Target**: System Bus (using `zbus::Connection` to introspect system services).
- **Service Dependency**: Targets external objects on the system bus via `IntrospectableProxy` to dynamically map system state.

---

## Security Audit Findings

### [CRITICAL] Public Unauthenticated SSE Stream Exposes All Projections to Network
- **Citation**: `crates/op-projection/src/json_stream.rs:72-88`, `crates/op-projection/src/json_stream.rs:161-197`
- **Vulnerability Type**: Insecure Direct Object Reference / Missing Authentication
- **Description**: 
  The `ProjectionStreamServer` binds to wildcard address `0.0.0.0` (line 78) and hosts a Server-Sent Events (SSE) stream endpoint `/events` (line 69). The `sse_handler` (line 161) performs absolutely no authentication, authorization, or token validation. Upon connection, any client on the network receives a full serialized snapshot of all active system projections (such as active processes, memory configurations, CPU models, and identity sled indices) and continues to receive unredacted updates in real-time.
- **Remediation**: 
  Restrict binding to `127.0.0.1` unless external access is strictly required. Introduce an extraction layer in Axum (e.g., an `Authorization` header extractor) to validate bearer tokens or mutual TLS state before allowing clients to subscribe to `/events`.

---

### [HIGH] Access Control & Redaction Bypassed in Production Event Loop
- **Citation**: `crates/op-projection/src/bin/projection_server.rs:180-245`
- **Vulnerability Type**: Security Logic Bypass
- **Description**: 
  Although `ProjectionAccessController` is instantiated at line 203 of the server entry point, it is never integrated into the actual event-loop update pipeline. The server polls data sources, creates projections, and broadcasts them directly to the `stream_server` at lines 189 and 241 without passing them through `enforce_policy` or performing any identity validation.
- **Remediation**: 
  Route all raw projections through `ProjectionAccessController::enforce_policy` before broadcasting them to the stream server. Redact or filter updates based on client-specific session scopes.

---

### [HIGH] No-Op Sensitive Data Redaction Placeholder
- **Citation**: `crates/op-projection/src/access_control.rs:105-112`
- **Vulnerability Type**: CWE-226: Sensitive Information Left in Memory / Lack of Redaction
- **Description**: 
  The `redact_sensitive` helper, which is meant to scrub PII and secrets (such as Private Keys, Passwords, and Identifiers) based on schema-defined paths, is implemented as a simple placeholder clone (`data.clone()`).
- **Remediation**: 
  Replace the placeholder clone with a JSON-pointer recursion routine that matches paths defined in `PluginSchema::secret_paths` and `PluginSchema::pii_paths` and replaces sensitive nodes with a masked value.

---

### [MEDIUM] Denial of Service via Unvalidated Regex Compilation inside Policy Read Locks
- **Citation**: `crates/op-projection/src/access_control.rs:44`, `crates/op-projection/src/access_control.rs:66`
- **Vulnerability Type**: CWE-400: Uncontrolled Resource Consumption
- **Description**: 
  Every time `enforce_policy` or `validate_permissions` is called, the policy patterns are compiled on the fly using `Regex::new(&policy.resource_pattern)?`. If an operator registers a malformed regular expression (as there is no validation on `add_policy` at line 144), evaluating permissions will return an immediate error, causing the entire permission matching mechanism to fail for all subsequent projections.
- **Remediation**: 
  Compile and store the `Regex` instance directly within the `AccessPolicy` struct when it is added to the controller, validating its syntax ahead of time.

---

### [MEDIUM] Ad-Hoc XML Parser on D-Bus Introspectable Response
- **Citation**: `crates/op-projection/src/dbus_reader.rs:45-60`
- **Vulnerability Type**: CWE-94: Improper Control of Generation of Code / Weak Parsing
- **Description**: 
  The `SystemDbusReader::introspect` method performs ad-hoc string splitting on introspected XML responses (`line.contains("<node name=\"")`) instead of using a structured XML parsing library. A compromised or malicious service on the system bus could supply malformed XML designed to exploit this logic, causing incorrect child path resolution or service panics.
- **Remediation**: 
  Utilize a robust, stream-oriented XML parser (such as `quick-xml` or `zbus_xml`) to parse the introspection tree safely.

---

### [LOW] Unsafe Shared Memory Pointer Dereference
- **Citation**: `crates/op-projection/src/sled_reader.rs:48-51`
- **Vulnerability Type**: Memory Safety Risk
- **Description**: 
  The reader dereferences a raw pointer returned from shared memory (`unsafe { &*ptr }`) without explicitly validating the mapped segment size or memory boundaries of the active memory-mapped file handle (`_mmap`).
- **Remediation**: 
  Implement safe bounds and alignment validation checks on the pointer, ensuring the segment size is equal to or greater than `std::mem::size_of::<IdentitySled>()` before casting.

---

## Schema-as-Code Compliance Audit

The `op-projection` system exhibits multiple deviations from the strict Schema-as-Code discipline. Rather than deriving contracts from versioned schema definitions (such as Protocol Buffers or OSCAL-compliant models), it relies on manually constructed in-memory Rust structures and ad-hoc JSON conversion patterns.

### Violations Identified
1. **Ad-Hoc JSON Value Generation**:
   - `crates/op-projection/src/procfs_reader.rs:144-152` (Memory metrics)
   - `crates/op-projection/src/procfs_reader.rs:180-184` (CPU details)
   - `crates/op-projection/src/sled_reader.rs:53-61` (Identity Sled properties)
   
   Raw properties are constructed manually using the `json!` macro before being ingested into the validator, bypassing structured code generation.

2. **Manual In-Memory Schema Definition**:
   - `crates/op-projection/src/bin/projection_server.rs:20-170`
   
   Versioned schemas are defined line-by-line using manual structural initialization in Rust code (e.g. `PluginSchema { name: ... }`), which increases the risk of schema drifts between distinct system services.

3. **Ad-Hoc Schema Conversion Maps**:
   - `crates/op-projection/src/plugin_reader.rs:270-345`
   
   The engine implements customized mapping functions (`convert_schema`, `convert_field`) to manually translate runtime schemas to projection schemas on-the-fly instead of relying on deterministic compiler generation.