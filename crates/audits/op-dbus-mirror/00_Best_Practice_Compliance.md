| Pattern | File:Line | Crate Approach | Corpus Best Practice | Gap | Rating |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `format_json_manual` | `crates/op-dbus-mirror/src/lib.rs:252` | Generates DBus paths ad-hoc via `format!` and parses system diagnostics into unstructured `simd_json::owned::Object` maps. | Define strongly typed contracts using versioned schemas (such as Protocol Buffers) rather than unstructured data models. | **Schema-as-Code Violation**: Data contracts and endpoints are not governed by versioned schemas. | Major Gap |
| `format_json_manual` | `crates/op-dbus-mirror/src/lib.rs:341` | Generates dynamic paths `/org/opdbus/v1/ovsdb/{}/{}` using arbitrary runtime table name strings. | Utilize a static schema registry or strongly typed path builders to prevent dynamic routing injection. | Dynamic routing strings are generated without schema validation. | Major Gap |
| `format_json_manual` | `crates/op-dbus-mirror/src/lib.rs:485` | Intermixing raw strings to publish system services dynamically under `/org/opdbus/v1/system/{}`. | Use schema-driven, typed identifiers and explicit endpoints defined in the interface schema. | Bypasses structured schema boundaries using ad-hoc interpolation. | Major Gap |
| `format_json_manual` | `crates/op-dbus-mirror/src/lib.rs:546` | Uses `replace` logic to clean component IDs for path generation. | Use a unified serialization or slugifying utility mapped directly from schema constraints. | Manual string mutation instead of schema-validated domain primitives. | Minor Gap |
| `format_json_manual` | `crates/op-dbus-mirror/src/lib.rs:635` | Manually constructs a JSON payload via raw string formatting: `format!("{{\"active\":false,\"name\":{:?}}}")`. | Implement structured serialization using robust serializers and schemas. | **Schema-as-Code Violation**: High risk of incorrect escaping and schema validation bypass through raw string interpolation. | Major Gap |
| `std_fs_in_async` | `crates/op-dbus-mirror/src/lib.rs:267` | Uses `tokio::fs::read_to_string` to load dynamic host state. | Leverage asynchronous runtime filesystem APIs to keep the thread pool unblocked. | Compliant (Correctly utilizes asynchronous `tokio::fs`). | Compliant |
| `std_fs_in_async` | `crates/op-dbus-mirror/src/lib.rs:283` | Uses `tokio::fs::read_to_string` to read `/proc/cpuinfo`. | Leverage asynchronous runtime filesystem APIs. | Compliant (Correctly utilizes asynchronous `tokio::fs`). | Compliant |
| `std_fs_in_async` | `crates/op-dbus-mirror/src/lib.rs:305` | Uses `tokio::fs::read_to_string` to read `/proc/loadavg`. | Leverage asynchronous runtime filesystem APIs. | Compliant (Correctly utilizes asynchronous `tokio::fs`). | Compliant |
| `unsafe_block` | `crates/op-dbus-mirror/src/jsonrpc_interface.rs:36` | Utilizes `unsafe { simd_json::from_str(...) }` to parse raw JSON RPC strings. | Deserialize untrusted payloads using safe parsing interfaces to prevent memory unsafety. | Unsafe parsing blocks on untrusted strings can expose memory safety risks if inputs are malformed. | Major Gap |
| `unsafe_block` | `crates/op-dbus-mirror/src/jsonrpc_interface.rs:181` | Utilizes `unsafe { simd_json::from_str(...) }` to parse incoming JSON RPC requests. | Deserialize untrusted payloads using safe parsing interfaces to prevent memory unsafety. | Unsafe parsing blocks on untrusted strings can expose memory safety risks if inputs are malformed. | Major Gap |
| `unsafe_block` | `crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs:237` | Adopts an inherited pipe file descriptor with `std::fs::File::from_raw_fd` under an `unsafe` block. | Document safety invariants when wrapping file descriptors and validate fd state. | Lacks safety documentation explaining fd validation and runtime ownership scope. | Minor Gap |
| `std_fs_in_async` | `crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs:237` | Performs synchronous synchronous file IO writes directly on the inherited pipe file descriptor. | Offload synchronous writes to a blocking pool or write through an asynchronous interface. | Blocking synchronous file IO can stall the reactor if executing within an async context. | Minor Gap |

---

### Actionable Recommendations for Major/Critical Gaps

#### 1. Enforce Schema-as-Code for DBus Paths and Payloads
* **Location:** `crates/op-dbus-mirror/src/lib.rs:252`, `341`, `485`
* **Remediation:** Remove manual format strings used to build `/org/opdbus/v1/...` DBus paths. Implement a declarative scheme using Protocol Buffers or a similar schema representation. Derive path parameters and validate components using typed abstractions instead of raw, unstructured strings.

#### 2. Replace Manual JSON Formatting with Serialization
* **Location:** `crates/op-dbus-mirror/src/lib.rs:635`
* **Remediation:** Completely deprecate manual JSON formatting via `format!`. Define a concrete Rust schema struct for system and component health/active states:
  ```rust
  #[derive(serde::Serialize)]
  struct ComponentStatus {
      active: bool,
      name: String,
  }
  ```
  Serialize instances of this struct using `simd_json::to_string` or `serde_json::to_string` to guarantee structurally valid, schema-compliant JSON payloads.

#### 3. Eliminate Unsafe String Parsing of Untrusted Payloads
* **Location:** `crates/op-dbus-mirror/src/jsonrpc_interface.rs:36`, `181`
* **Remediation:** Since both operations and request parameters originate from DBus messages, they must be treated as untrusted inputs. Remove the `unsafe` block by migrating to safe parsers. If using `simd_json`, utilize its safe parsing interface or replace with `serde_json::from_str` to eliminate memory corruption vectors (such as out-of-bounds reads/writes) during deserialization.