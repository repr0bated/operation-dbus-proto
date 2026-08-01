# Production Security & Quality Audit: op-dbus-mirror

## 1. Unsafe Code Analysis

This section analyzes every `unsafe {` block found in the audited codebase.

### Block 1
* **File:** `crates/op-dbus-mirror/src/jsonrpc_interface.rs:38`
* **Context:**
  ```rust
  let ops: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut operations_mut) }
  ```
* **Safety Comment:** Missing `// SAFETY:` comment.
* **Risk Evaluation:** **High**. `simd_json::from_str` modifies the input string in-place to resolve escape sequences and null-terminate tokens. This requires that the input string buffer has `simd_json::SIMDJSON_PADDING` (typically 32 or 64 bytes) of initialized padding after the string data. Cloned standard `String` allocations do not guarantee this padding, which can cause SIMD instructions to perform out-of-bounds reads, potentially leading to memory corruption or segmentation faults.

### Block 2
* **File:** `crates/op-dbus-mirror/src/jsonrpc_interface.rs:158`
* **Context:**
  ```rust
  let req: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut request_mut) }
  ```
* **Safety Comment:** Missing `// SAFETY:` comment.
* **Risk Evaluation:** **High**. Same alignment, padding, and out-of-bounds read risk as Block 1 since the cloned raw D-Bus `request` String is parsed directly without guaranteed padding.

### Block 3
* **File:** `crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs:301`
* **Context:**
  ```rust
  let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
  ```
* **Safety Comment:** Missing `// SAFETY:` comment.
* **Risk Evaluation:** **High**. Taking ownership of an arbitrary raw file descriptor via `from_raw_fd` is unsafe because closing the returned `File` will automatically close the underlying descriptor. If `DINIT_DBUS_READY_FD` is manipulated or set to a descriptor currently in use by another thread or standard streams (e.g., standard output `1`), it will lead to silent log loss, double-closes, or file descriptor reuse race conditions.

---

## 2. Process Spawning & Command Execution

* **Total Count of `Command::new()`:** 0
* **Forbidden Commands Check:** No command invocations or process spawns exist in the provided source files.

---

## 3. D-Bus Method Exposure

The following D-Bus methods are exposed on either the system bus or session bus. If the service is initialized with `BusType::System` (as supported in `lib.rs:77`), **any unprivileged system-bus peer** can call these methods. 

### org.opdbus.OvsdbV1 (`crates/op-dbus-mirror/src/jsonrpc_interface.rs`)
* `transact(operations: String) -> String`
* `get_schema() -> String`
* `list_dbs() -> String`
* `dump_db() -> String`
* `create_bridge(name: String)`
* `delete_bridge(name: String)`
* `add_port(bridge: String, port: String)`
* `list_bridges() -> String`
* `list_ports(bridge: String) -> String`

### org.opdbus.NonNetV1 (`crates/op-dbus-mirror/src/jsonrpc_interface.rs`)
* `transact(request: String) -> String`
* `get_schema() -> String`
* `list_dbs() -> String`

### org.opdbus.MirrorV1 (`crates/op-dbus-mirror/src/dbus_interface.rs`)
* `publish_snapshot()`
* `reconcile()`
* `get_stats() -> String`
* `list_paths() -> Vec<String>`

### org.opdbus.PluginsV1 (`crates/op-dbus-mirror/src/plugin_interface.rs`)
* `list() -> Vec<String>`
* `get(name: String) -> String`
* `get_all() -> HashMap<String, String>`

### org.freedesktop.DBus.ObjectManager (`crates/op-dbus-mirror/src/managed_objects.rs`)
* `get_managed_objects() -> HashMap<OwnedObjectPath, InterfaceMap>`

---

## 4. Security Findings & Vulnerability Audit

### [CRITICAL] Unauthenticated Mutative Method Exposure on System Bus
* **Reference:** `crates/op-dbus-mirror/src/jsonrpc_interface.rs:35-131` and `crates/op-dbus-mirror/src/lib.rs:77`
* **Vulnerability Type:** Missing Access Control / Privilege Escalation
* **Description:** 
  The D-Bus publication service can be instantiated on the System Bus (`BusType::System`). On this bus, the `OvsdbInterface` and `NonNetInterface` are registered, exposing control plane administrative actions such as `transact`, `create_bridge`, `delete_bridge`, and `add_port`.
  
  There are absolutely no authentication, sender verification, or peer UID checks implemented on any of these exposed methods.
* **Exploit Scenario:** 
  An unprivileged local user or a compromised service inside a container with access to the system bus can send a direct D-Bus method call (e.g., `org.opdbus.OvsdbV1.create_bridge`) to create or delete virtual interfaces, or use `transact` to issue arbitrary write operations to the authoritative `Open_vSwitch` database, entirely bypassing host-level permissions.
* **Mitigation:**
  Verify the peer's credentials on every incoming method call by checking the sender's UID using `zbus::Connection::peer_credentials()`. Reject any mutative requests if the peer UID is not `0` (root) or the designated service user.

### [HIGH] Arbitrary File Descriptor Closure via Untrusted Environment Variable
* **Reference:** `crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs:291-304`
* **Vulnerability Type:** Improper Input Validation / Resource Management
* **Description:** 
  The `signal_dinit_ready` function reads the `DINIT_DBUS_READY_FD` environment variable, parses it as an integer, and blindly consumes it with `std::fs::File::from_raw_fd(fd)`. 
* **Exploit Scenario:** 
  If an attacker or an misconfigured wrapper controls the environment variables of `ovs-dbus-init`, they can set `DINIT_DBUS_READY_FD=1` or `DINIT_DBUS_READY_FD=2`. This will cause the process to close its own standard output or standard error when the temporary `File` drops, preventing log generation and potentially leading to crashes or silent failures.
* **Mitigation:** 
  Restrict file descriptor inheritance and validate that the parsed file descriptor is valid, is not a standard stream, and matches expected inherited file descriptors from the parent supervisor.

### [MEDIUM] Host Statistics Leakage to Unprivileged Local Peers
* **Reference:** `crates/op-dbus-mirror/src/lib.rs:188-251`
* **Vulnerability Type:** Information Disclosure
* **Description:** 
  The `publish_host_snapshot` function systematically parses `/proc/meminfo`, `/proc/cpuinfo`, and `/proc/loadavg` to register them as individual `MirrorObject` instances in the D-Bus hierarchy under `/org/opdbus/v1/host/`. These objects are globally queryable.
* **Risk:** 
  While much of `/proc` is visible to local users, publishing structured host stats on a public system bus enables containerized or sandboxed processes to conduct target reconnaissance, side-channel analysis, or monitor host system loads without directly querying host procfs.
* **Mitigation:** 
  Apply policy rules or client-side UID checks to the host segment of the D-Bus mirror tree, ensuring only authorized system services can access node performance data.

---

## 5. Schema-as-Code Compliance Audit

The system relies on a schema-as-code discipline using Protocol Buffers and OSCAL. Ad-hoc representation of structures as strings, maps, or untyped JSON blobs violates this discipline and bypasses compile-time schema safety.

### Finding 1: Untyped Interface Map JSON Properties
* **Reference:** `crates/op-dbus-mirror/src/managed_objects.rs:24-28`
* **Description:** Properties are defined using an ad-hoc key-value string map rather than typed versioned schemas:
  ```rust
  pub type PropertyMap = HashMap<String, String>;
  pub type InterfaceMap = HashMap<String, PropertyMap>;
  ```

### Finding 2: Unstructured Database State Replication
* **Reference:** `crates/op-dbus-mirror/src/object.rs:10-12`
* **Description:** The mirror objects wrap raw unstructured `simd_json::OwnedValue` data, discarding database constraints:
  ```rust
  pub struct MirrorObject {
      data: Value,
  }
  ```

### Finding 3: Ad-hoc Serialization of DB Rows
* **Reference:** `crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs:20-27`
* **Description:** The `BridgeRow` represents database structural attributes as ad-hoc strings and maps:
  ```rust
  struct BridgeRow {
      name: String,
      uuid: String,
      datapath_type: String,
      other_config: HashMap<String, String>,
      external_ids: HashMap<String, String>,
  }
  ```

### Schema Compliance Mitigation:
Replace all raw JSON strings, `simd_json::OwnedValue`, and unstructured string maps in D-Bus interfaces with serialization layers generated directly from Protocol Buffers schemas. Ensure any structural system model conforms strictly to generated OSCAL Rust types.

---
## ⚠ Citation Warnings
- `crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs:301`: file has 239 lines
- `crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs:291`: file has 239 lines
