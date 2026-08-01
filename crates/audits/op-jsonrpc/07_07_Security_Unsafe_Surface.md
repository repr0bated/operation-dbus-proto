# Production Security and Quality Audit: op-jsonrpc

## 1. Memory Safety & Unsafe Analysis

All identified `unsafe` blocks in the codebase have been audited. Every instance involves the use of `simd_json::from_str` on standard `String` references without safety documentation or validation of memory alignment and padding invariants.

### Directly Exploitable Memory Vulnerability (Critical)

In multiple files, the codebase performs in-place parsing of standard JSON strings using `simd_json::from_str` inside an `unsafe` block. 

* **The Vulnerability**: `simd-json` relies on SIMD hardware vectorization (AVX2/SSE4.2/NEON). Its parser requires that the input string buffer be mutable and padded with at least `simd_json::SIMDJSON_PADDING` (typically 32 or 64) bytes beyond the logical string end. Passing an unpadded string slice (`as_mut_str()`) obtained from a standard `std::string::String` (such as one populated by `tokio::io::BufReader::read_line`) will trigger **out-of-bounds memory reads** when the vectorized instructions execute past the buffer boundary.
* **Exploitability**: This is directly exploitable by system peers or TCP clients sending JSON payloads. Triggering out-of-bounds memory access leads to immediate segmentation faults (Denial of Service) or potential information disclosure of adjacent heap memory.
* **Remediation**: Use `simd_json::to_padded_bin` to guarantee safety padding, or utilize standard `serde_json` for processing line-delimited streams from network sockets where allocation-overhead is acceptable.

### Unsafe Call Map

#### `crates/op-jsonrpc/src/nonnet.rs:312`
```rust
        let response = match unsafe { simd_json::from_str::<Value>(line.as_mut_str()) } {
```
* **Vulnerability**: Critical. Standard `line: String` from `read_line` has no guarantee of SIMD padding.
* **Missing Comment**: `// SAFETY:` is completely missing.

---

#### `crates/op-jsonrpc/src/server.rs:257`
```rust
        match unsafe { simd_json::from_str::<Value>(line.as_mut_str()) } {
```
* **Vulnerability**: Critical. Input from TCP socket reader is parsed directly from unpadded mutable string slice.
* **Missing Comment**: `// SAFETY:` is completely missing.

---

#### `crates/op-jsonrpc/src/ovsdb.rs:92`
```rust
        if let Ok(value) = unsafe { simd_json::from_str::<Value>(payload.as_mut_str()) } {
```
* **Vulnerability**: Critical. `payload` is created via `.to_string()`, which allocates an exact-fit string buffer with zero trailing padding.
* **Missing Comment**: `// SAFETY:` is completely missing.

---

#### `crates/op-jsonrpc/src/ovsdb.rs:104`
```rust
            if let Ok(value) = unsafe { simd_json::from_str::<Value>(owned.as_mut_str()) } {
```
* **Vulnerability**: Critical. Standard `owned: String` buffer passed to `simd_json` without alignment/padding validation.
* **Missing Comment**: `// SAFETY:` is completely missing.

---

#### `crates/op-jsonrpc/src/ovsdb.rs:479`
```rust
                if let Ok(update) = unsafe { simd_json::from_str::<Value>(line_clone.as_mut_str()) }
```
* **Vulnerability**: Critical. Buffer parsed from asynchronous socket reader chunk lacking SIMD-compliant bounds padding.
* **Missing Comment**: `// SAFETY:` is completely missing.

---

#### `crates/op-jsonrpc/src/ovsdb_rpc_call.rs:24`
```rust
        let response: Value = unsafe { simd_json::from_str(&mut response_str)? };
```
* **Vulnerability**: Critical. Invocation with raw mutable reference of dynamic socket read buffer containing zero tail padding.
* **Missing Comment**: `// SAFETY:` is completely missing.

---

## 2. Command Execution & Process Security

### Process Spawning Analysis
* **Command Spawning Count**: `0` (Zero instances of `Command::new` or similar subprocess execution interfaces exist in this crate).
* **Forbidden Utilities**: No instances of forbidden shells (`bash`, `sh`), network exfiltration tools (`curl`, `wget`), or OpenFlow/OVS command-line executables (`ovs-vsctl`, etc.) are spawned as processes.

### Direct Protocol Communications
Rather than shelling out to forbidden OVS executable binaries, the crate interacts directly with OVSDB over local Unix domain sockets (e.g. `/var/run/openvswitch/db.sock`) using the native JSON-RPC protocol. While this avoids OS command injection vectors, it bypasses authorization mechanisms if the socket permissions are weakly configured.

---

## 3. Credentials and Secrets Audit

No hardcoded cryptographic tokens, API passwords, private keys, or static IP addresses were detected within the source code of this crate. 

Socket paths and local addresses default to standard runtime values:
* `/var/run/op-dbus/jsonrpc.sock` (`crates/op-jsonrpc/src/server.rs:46`)
* `/var/run/openvswitch/db.sock` (`crates/op-jsonrpc/src/ovsdb.rs:25`)

---

## 4. D-Bus Method Exposure

As audited strictly from the provided source tree, this crate does not use or register any direct D-Bus bindings (`zbus` interface attributes or system-bus peer registries). Although the overarching workspace target is named `op-dbus`, all network and state synchronization procedures in `crates/op-jsonrpc` are exposed strictly over Unix domain sockets and configureable TCP socket listeners rather than D-Bus system-bus pathways.

---

## 5. Schema-as-Code Discipline

This repository violates the strict Schema-as-Code discipline. Data contracts, interface methods, and database schemas are handled as ad-hoc, untyped strings and loosely validated dynamic JSON values instead of versioned ProtoBuf or OSCAL schemas.

### Ad-Hoc Data Contracts & Schemas

#### `crates/op-jsonrpc/src/protocol.rs:11-17`
```rust
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
    pub id: Value,
}
```
* **Violation**: The outer structure uses raw serialization, while the inner request variables (`params`, `id`) are processed as raw `simd_json::OwnedValue` elements. This bypasses static contract compilation, rendering api changes un-versioned and highly brittle to dynamic modifications.

---

#### `crates/op-jsonrpc/src/nonnet.rs:117-142`
```rust
        for (name, value) in plugins {
            // Skip network plugin
            if name == "net" {
                continue;
            }

            // Infer columns from the value structure
            let columns = infer_columns(value);
            schema_tables.insert(name.clone(), json!({"columns": columns}));

            // Convert value to rows
            let rows = value_to_rows(value);
...
```
* **Violation**: Tables, column constraints, and schemas are dynamically "inferred" at runtime from arbitrary plugin values via `infer_columns` and `infer_type`. Changes to core plugin structures will silently break non-network interface structures without compile-time contract resolution or version gating.

---

#### `crates/op-jsonrpc/src/ovsdb.rs:145-184`
```rust
        let operations = json!([
            {
                "op": "insert",
                "table": "Bridge",
                "row": {
                    "name": name,
                    "ports": ["set", [["named-uuid", port_uuid]]]
                },
                "uuid-name": bridge_uuid
            },
...
```
* **Violation**: Transaction schemas (e.g., `Bridge`, `Port`, `Interface`) are declared via dynamic string key lookups in nested dynamic structures. A typo in a property name or value format will bypass compile-time detection, resulting in runtime transaction failures inside the database driver.