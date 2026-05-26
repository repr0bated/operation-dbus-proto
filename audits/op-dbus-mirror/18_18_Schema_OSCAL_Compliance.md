# Production Security and Quality Audit: op-dbus-mirror

## 1. Schema-as-Code Compliance

| Item | Type | file:line | Has .proto? | Gap |
| :--- | :--- | :--- | :--- | :--- |
| `MirrorObject::data` | Dynamic Struct Property | `crates/op-dbus-mirror/src/object.rs:9` | No | Uses untyped `simd_json::OwnedValue` to represent database rows. Structural properties cannot be compiled or validated against a versioned schema. |
| `DbusMirrorInterface::get_stats` | D-Bus Method Response | `crates/op-dbus-mirror/src/dbus_interface.rs:31` | No | Returns ad-hoc JSON generated via `simd_json::json!` stringified at runtime rather than a versioned schema-defined payload. |
| `build_interface_map` / `JsonData` | D-Bus Property Mapping | `crates/op-dbus-mirror/src/managed_objects.rs:88` | No | Packages internal states as arbitrary stringified JSON blobs under the `JsonData` property instead of mapping fields to structured protobuf fields. |
| `PluginInterface::get` / `get_all` | D-Bus Methods | `crates/op-dbus-mirror/src/plugin_interface.rs:43` | No | Exposes raw plugin database maps directly as stringified JSON keys/values with no static contract or serialization validation. |
| `OvsdbInterface::transact` | D-Bus Method Input | `crates/op-dbus-mirror/src/jsonrpc_interface.rs:35` | No | Accepts an arbitrary raw `String` representing JSON-RPC mutations. Bypasses structured type checks entirely. |
| `NonNetInterface::transact` | D-Bus Method Input | `crates/op-dbus-mirror/src/jsonrpc_interface.rs:168` | No | Accepts an untyped JSON string request parsed dynamically at runtime, creating a massive schema gap for external mutation endpoints. |
| `component_info_to_value` | Hand-rolled Mapper | `crates/op-dbus-mirror/src/lib.rs:434` | Yes (partially) | Hand-rolls serialization logic mapping versioned Protobuf `ComponentInfo` fields manually into dynamic `simd_json::OwnedValue` fields, violating schema-driven serialization. |

---

## 2. OSCAL Coverage & Security Control Mapping

| Control Area | Implemented at file:line | OSCAL Artifact | Gap |
| :--- | :--- | :--- | :--- |
| **AC-3 (Access Enforcement)** | `crates/op-dbus-mirror/src/lib.rs:77-83` | None | D-Bus connections are initialized on the `System` or `Session` bus under `org.opdbus.v1` with absolutely no authorization policies or peer credential checks (`zbus::connection::Builder` is used without custom policies). |
| **AC-3 (Access Enforcement)** / **SC-7 (Boundary Protection)** | `crates/op-dbus-mirror/src/lib.rs:232` | None | Host environment characteristics (parsing raw `/proc/meminfo`, `/proc/cpuinfo`, and `/proc/loadavg`) are published to the public D-Bus namespace `/org/opdbus/v1/host/*` without validating caller privilege levels. |
| **AU-2 (Event Logging)** / **AU-12 (Audit Generation)** | `crates/op-dbus-mirror/src/jsonrpc_interface.rs:35`, `168` | None | Transactions that modify state (such as `OvsdbInterface::transact` and `NonNetInterface::transact`) bypass standard security audit logging. Actions are routed directly to the database without generating verifiable audit events. |
| **CM-6 (Configuration Settings)** / **Hardcoded Policies** | `crates/op-dbus-mirror/src/lib.rs:326` | None | Security boundary filters (`SKIP_SERVICES`) are defined as a hardcoded static array of strings rather than being managed and ingested as machine-readable policy configuration (e.g., an OSCAL Component Definition). |

---

## 3. Vulnerability Findings & Code Quality Violations

### [CRITICAL] Memory Safety Violation (Heap Out-of-Bounds Read) in JSON-RPC Interfaces
*   **Location:** `crates/op-dbus-mirror/src/jsonrpc_interface.rs:37` and `crates/op-dbus-mirror/src/jsonrpc_interface.rs:171`
*   **Vulnerability Type:** Memory Safety / Out-of-Bounds Read / Undefined Behavior
*   **Description:**
    The implementation uses `unsafe` blocks to parse cloned string buffers with `simd_json::from_str`:
    ```rust
    // crates/op-dbus-mirror/src/jsonrpc_interface.rs:35-37
    async fn transact(&self, operations: String) -> zbus::fdo::Result<String> {
        let mut operations_mut = operations.clone();
        let ops: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut operations_mut) }
    ```
    `simd_json::from_str` relies on **unaligned SIMD instructions** that process data in wide (32-byte or 64-byte) registers. Because of this, `simd_json` strictly requires input buffers to have a trailing padding of `simd_json::SIMDJSON_PADDING` bytes. 
    Standard Rust string allocation (via `operations.clone()`) does **not** allocate padding bytes at the end of the backing array. When an unpadded string slice is forced through `unsafe { simd_json::from_str }`, the SIMD registers will read past the allocated boundary of the `operations_mut` string on the heap.
*   **Exploitability:**
    Directly exploitable by any local/sandboxed application that has permissions to send a method call message to the D-Bus interface. Sending a payload whose length ends near a page boundary will trigger a **Segmentation Fault** (Denial of Service) or result in **uncontrolled heap information exposure** (reading adjacent heap metadata into the parsed JSON structure).
*   **Remediation:**
    Convert the raw string into a padded byte vector, or use safe APIs like `simd_json::to_owned_value` which automatically copy the data into a padded buffer:
    ```rust
    let mut bytes = operations.into_bytes();
    let ops = simd_json::to_owned_value(&mut bytes)
        .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;
    ```

### [MAJOR] Host Information Leakage via `/proc` Mirroring over D-Bus
*   **Location:** `crates/op-dbus-mirror/src/lib.rs:232-251`
*   **Vulnerability Type:** Information Exposure
*   **Description:**
    The D-Bus mirror reads from `/proc/meminfo`, `/proc/cpuinfo`, and `/proc/loadavg` at every sync interval and publishes the parsed results globally on the system bus. This exposes kernel-level host configurations and hardware performance structures to low-privileged sandboxed applications on the system without requiring physical read permissions to `/proc`.
*   **Remediation:**
    Implement strict client credential checks in the D-Bus object server. Validate the user ID (UID) of the caller before returning data from `/org/opdbus/v1/host/` endpoints.

---

## 4. Architectural Recommendations

1.  **Enforce Schema-as-Code for D-Bus Payloads:**
    Replace the usage of raw `String` responses carrying stringified JSON (e.g., in `dbus_interface.rs` and `plugin_interface.rs`) with well-defined structural schemas. If D-Bus structures are required, use `zbus`'s native support for serializing complex Rust structs implementing `zvariant::Type` and `serde::Serialize` instead of escape-hatching via untyped strings.
2.  **Externalize Ignore / Skip Lists as OSCAL Policies:**
    Refactor the hardcoded `SKIP_SERVICES` static array out of `lib.rs` and load system boundary rules from a centralized machine-readable policy descriptor mapped to an OSCAL component definition.
3.  **Establish Audit Trail for State Mutations:**
    Every transaction routed through `OvsdbInterface::transact` or `NonNetInterface::transact` should generate a structured audit trail event correlating the D-Bus caller's UID and the mutating payload to comply with AU-2 and AU-12 control areas.