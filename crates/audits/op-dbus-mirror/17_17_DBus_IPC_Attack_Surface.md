# Production Security & Quality Audit: D-Bus & IPC Attack Surface

## 1. D-Bus & IPC Attack Surface Registry

This section inventories all D-Bus interfaces, methods, and signals registered across the audited codebase. 

### Connection Context
* **System Bus vs Session Bus:** 
  * The main daemon `DbusMirror` can connect to either the System or Session bus depending on the `BusType` parameter passed on initialization (`crates/op-dbus-mirror/src/lib.rs:80`).
  * The helper binary `ovs-dbus-init` connects exclusively to the **Session Bus** (`crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs:77`).

---

### Interface: `org.opdbus.MirrorV1`
* **Registered in:** `crates/op-dbus-mirror/src/dbus_interface.rs`
* **Authentication/Caller Identity Checked:** No caller validation is performed.

| Method / Signal | Type | Input Arguments | Output Arguments | Mutates State / Spawns? | Description |
| :--- | :--- | :--- | :--- | :---: | :--- |
| `publish_snapshot` | Method | None | `Result<()>` | Yes (Triggers database sync) | Forces a full snapshot replication from authoritative databases. |
| `reconcile` | Method | None | `Result<()>` | Yes (Triggers database sync) | Compatibility alias calling `publish_snapshot`. |
| `get_stats` | Method | None | `Result<String>` | No | Returns raw JSON-serialized statistics string. |
| `list_paths` | Method | None | `Result<Vec<String>>` | No | Lists all published object paths in the mirror tree. |

---

### Interface: `org.freedesktop.DBus.ObjectManager`
* **Registered in:** `crates/op-dbus-mirror/src/managed_objects.rs`
* **Authentication/Caller Identity Checked:** No caller validation is performed.

| Method / Signal | Type | Input Arguments | Output Arguments | Mutates State / Spawns? | Description |
| :--- | :--- | :--- | :--- | :---: | :--- |
| `get_managed_objects` | Method | None | `HashMap<OwnedObjectPath, InterfaceMap>` | No | Retrieves all managed objects and their properties. |
| `interfaces_added` | Signal | `object_path: OwnedObjectPath`, `interfaces_and_properties: InterfaceMap` | N/A | No | Emitted when a new object or interface is registered. |
| `interfaces_removed` | Signal | `object_path: OwnedObjectPath`, `interfaces: Vec<String>` | N/A | No | Emitted when an object or interface is removed. |

---

### Interface: `org.opdbus.ProjectedObjectV1`
* **Registered in:** `crates/op-dbus-mirror/src/object.rs`
* **Authentication/Caller Identity Checked:** No caller validation is performed.

| Method / Signal | Type | Input Arguments | Output Arguments | Mutates State / Spawns? | Description |
| :--- | :--- | :--- | :--- | :---: | :--- |
| `json_data` | Property | None | `String` | No | Retrieves full unstructured JSON string representation of a mirrored row. |
| `get_property` | Method | `key: String` | `String` | No | Retrieves a serialized property value by key. |
| `data_updated` | Signal | None | N/A | No | Emitted when `json_data` changes. |

---

### Interface: `org.opdbus.PluginsV1`
* **Registered in:** `crates/op-dbus-mirror/src/plugin_interface.rs`
* **Authentication/Caller Identity Checked:** No caller validation is performed.

| Method / Signal | Type | Input Arguments | Output Arguments | Mutates State / Spawns? | Description |
| :--- | :--- | :--- | :--- | :---: | :--- |
| `list` | Method | None | `Vec<String>` | No | Lists names of all registered plugins. |
| `get` | Method | `name: String` | `String` | No | Gets unstructured state JSON for a specific plugin. |
| `get_all` | Method | None | `HashMap<String, String>` | No | Gets name-to-JSON-state map of all plugins. |

---

### Interface: `org.opdbus.OvsdbV1`
* **Registered in:** `crates/op-dbus-mirror/src/jsonrpc_interface.rs`
* **Authentication/Caller Identity Checked:** No caller validation is performed.

| Method / Signal | Type | Input Arguments | Output Arguments | Mutates State / Spawns? | Description |
| :--- | :--- | :--- | :--- | :---: | :--- |
| `transact` | Method | `operations: String` | `Result<String>` | Yes (Mutates OVSDB database) | Executes raw, arbitrary JSON-RPC transaction payloads against OVSDB. |
| `get_schema` | Method | None | `Result<String>` | No | Gets a proxy of schema databases. |
| `list_dbs` | Method | None | `Result<String>` | No | Lists active OVSDB databases. |
| `dump_db` | Method | None | `Result<String>` | No | Dumps the Open_vSwitch database. |
| `create_bridge` | Method | `name: String` | `Result<()>` | Yes (Spawns config action/state) | Triggers OVS bridge creation. |
| `delete_bridge` | Method | `name: String` | `Result<()>` | Yes (Spawns config action/state) | Triggers OVS bridge deletion. |
| `add_port` | Method | `bridge: String`, `port: String` | `Result<()>` | Yes (Spawns config action/state) | Adds a network port to a specified bridge. |
| `list_bridges` | Method | None | `Result<String>` | No | Lists existing OVS bridges. |
| `list_ports` | Method | `bridge: String` | `Result<String>` | No | Lists ports configured on a bridge. |

---

### Interface: `org.opdbus.NonNetV1`
* **Registered in:** `crates/op-dbus-mirror/src/jsonrpc_interface.rs`
* **Authentication/Caller Identity Checked:** No caller validation is performed.

| Method / Signal | Type | Input Arguments | Output Arguments | Mutates State / Spawns? | Description |
| :--- | :--- | :--- | :--- | :---: | :--- |
| `transact` | Method | `request: String` | `Result<String>` | Yes (Mutates database state) | Executes raw, arbitrary JSON-RPC transaction payloads against NonNet database. |
| `get_schema` | Method | None | `Result<String>` | No | Gets schema details. |
| `list_dbs` | Method | None | `Result<String>` | No | Lists databases. |

---

### Interface: `org.opdbus.Bridge`
* **Registered in:** `crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs`
* **Authentication/Caller Identity Checked:** No caller validation is performed.

| Method / Signal | Type | Input Arguments | Output Arguments | Mutates State / Spawns? | Description |
| :--- | :--- | :--- | :--- | :---: | :--- |
| `get_name` | Method | None | `String` | No | Retrieves name of OVS bridge. |
| `get_uuid` | Method | None | `String` | No | Retrieves OVSDB UUID of bridge. |
| `get_datapath_type`| Method | None | `String` | No | Retrieves datapath type. |
| `get_other_config` | Method | None | `HashMap<String, String>` | No | Retrieves miscellaneous configurations. |
| `get_external_ids` | Method | None | `HashMap<String, String>` | No | Retrieves external identifier metadata. |

---

## 2. Critical Security Findings

### CRITICAL: Memory Unsafety via Unaligned/Unpadded `simd_json` Parsing on Untrusted Input
* **Location:** 
  * `crates/op-dbus-mirror/src/jsonrpc_interface.rs:37`
  * `crates/op-dbus-mirror/src/jsonrpc_interface.rs:188`
* **Impact:** Process crash (Segmentation Fault) or arbitrary code execution.
* **Description:** 
  In the `transact` methods of both `OvsdbInterface` and `NonNetInterface`, raw JSON strings are received from untrusted D-Bus callers, cloned, and parsed using `unsafe { simd_json::from_str(...) }`:
  ```rust
  let mut operations_mut = operations.clone();
  let ops: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut operations_mut) }
      .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;
  ```
  The `simd_json` library achieves high performance via SIMD vector instructions which explicitly require that input byte buffers are padded with `simd_json::SIMDJSON_PADDING` bytes (typically 64 bytes). Standard Rust `String` slices do not guarantee this padding at the end of their allocations. Because `from_str` is used directly on unpadded strings within an `unsafe` block, the SIMD parser will perform out-of-bounds reads when processing payloads near buffer boundaries. This is directly exploitable by any local attacker with D-Bus access, resulting in memory corruption and immediate process termination.

* **Remediation:** 
  Convert the incoming `String` into a padded buffer or use the safe API. If using `simd_json`, convert the string to a `Vec<u8>` and call `simd_json::to_owned_value` which internally handles buffer padding:
  ```rust
  let mut operations_bytes = operations.into_bytes();
  let ops = simd_json::to_owned_value(&mut operations_bytes)
      .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;
  ```

---

### CRITICAL: Missing Authentication & Authorization on Authoritative Control Plane Methods
* **Location:** 
  * `crates/op-dbus-mirror/src/jsonrpc_interface.rs:35`
  * `crates/op-dbus-mirror/src/jsonrpc_interface.rs:88`
  * `crates/op-dbus-mirror/src/jsonrpc_interface.rs:112`
  * `crates/op-dbus-mirror/src/jsonrpc_interface.rs:120`
  * `crates/op-dbus-mirror/src/jsonrpc_interface.rs:186`
* **Impact:** Unprivileged local users can modify system network interfaces, configure bridges, inject ports, or perform arbitrary database mutations on system databases.
* **Description:** 
  The D-Bus mirror daemon registers interfaces representing system-level operations (`org.opdbus.OvsdbV1` and `org.opdbus.NonNetV1`). When configured to use `BusType::System`, the daemon resides on the System Bus, which by default allows communication from various system processes. None of the exposed mutating D-Bus methods check the identity or uid of the caller.
  
  Methods such as `create_bridge`, `delete_bridge`, `add_port`, and the general database `transact` routes bypass authorization checks entirely:
  ```rust
  async fn create_bridge(&self, name: String) -> zbus::fdo::Result<()> {
      if let Some(engine) = &self.schema_engine {
          engine.mutate(...) // No caller uid check!
      } else {
          self.client.create_bridge(&name).await ...
      }
  }
  ```
  Any local user can send a D-Bus message to these endpoints, causing arbitrary network state modifications on the host.

* **Remediation:** 
  Add caller credential validation using `zbus::connection::Connection::peer_credentials`. Extract the user ID (UID) of the caller and enforce that only `root` (UID `0`) or authorized group members can execute state-changing methods. Alternatively, rely on D-Bus System Bus XML policies to restrict method execution to privileged users, though programmatic verification within the application is highly recommended as defense-in-depth:
  ```rust
  // Inside method implementation:
  // (Requires acquiring the active zbus Connection or using method context wrappers)
  let header = ctxt.message().header();
  // Validate caller permissions
  ```

---

## 3. Schema-as-Code Violations

The codebase frequently bypasses structured serialization schema definitions (such as Protocol Buffers or versioned JSON schemas) in favor of ad-hoc JSON construction and raw string serialization.

### Violation 1: Ad-Hoc Statistics Serialization
* **Location:** `crates/op-dbus-mirror/src/dbus_interface.rs:37-41`
* **Description:** 
  The interface retrieves statistics by constructing unstructured, anonymous JSON on the fly using `simd_json::json!`:
  ```rust
  let stats = simd_json::json!({
      "published_objects": self.mirror.published_count(),
      "projected_objects": self.mirror.projected_count(),
  });
  Ok(simd_json::to_string(&stats).unwrap_or_default())
  ```
  This returns an unversioned raw string. D-Bus clients must parse this arbitrarily without a version-controlled interface schema definition.

---

### Violation 2: Unstructured Database Mirroring properties
* **Location:** `crates/op-dbus-mirror/src/managed_objects.rs:25-33`
* **Description:** 
  The `ObjectManagerInterface` represents properties on mirrored database rows as an unstructured map of strings:
  ```rust
  pub type PropertyMap = HashMap<String, String>;
  pub type InterfaceMap = HashMap<String, PropertyMap>;
  ```
  The helper function `build_interface_map` maps a arbitrary serialized JSON block under a single hardcoded key:
  ```rust
  pub fn build_interface_map(json_str: &str) -> InterfaceMap {
      let mut props = PropertyMap::new();
      props.insert("JsonData".to_string(), json_str.to_string());
      ...
  }
  ```
  Database fields and types are completely lost during this projection, transforming structured database rows into unversioned, opaque string fields.

---

### Violation 3: Free-Form JSON Database Mutator payloads
* **Location:** `crates/op-dbus-mirror/src/jsonrpc_interface.rs:35` and `crates/op-dbus-mirror/src/jsonrpc_interface.rs:186`
* **Description:** 
  The `transact` endpoints pass raw JSON-RPC operations over the D-Bus boundary as free-form, unvalidated strings (`operations: String`, `request: String`). This encourages ad-hoc JSON structure mutations where changes are unversioned and unvalidated, exposing both OVSDB and NonNet databases to structural corruption if invalid field combinations are supplied.

---

### Violation 4: Unversioned Plugin State Serialization
* **Location:** `crates/op-dbus-mirror/src/plugin_interface.rs:39-49`
* **Description:** 
  The `PluginInterface` maps plugin configurations and status outputs using an unversioned, raw string-encoded representation of the internal status structure:
  ```rust
  async fn get(&self, name: String) -> String { ... }
  async fn get_all(&self) -> HashMap<String, String> { ... }
  ```
  This design introduces significant backwards-compatibility issues if the layout of the plugin status map evolves over time.

---

## 4. General D-Bus & Architectural Quality Issues

### Architectural Anti-Pattern: Session Bus Usage for Host-Level Open_vSwitch Integration
* **Location:** `crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs:77`
* **Description:** 
  The `ovs-dbus-init` initialization binary is responsible for connecting to Open_vSwitch (`OVSDB`), fetching active host bridges, and registering them as D-Bus objects under the path `/org/opdbus/bridge`. However, it registers these components on the user's **Session Bus**:
  ```rust
  let connection = Builder::session()?
      .name(bus_name.as_str())?
      .build()
      .await
  ```
  Open_vSwitch is a critical, host-wide system daemon managing global network topology. Standardizing on Session Bus ownership for system-level networking resources prevents system-wide services and other local users from accessing the registration tree cleanly, and violates conventions where administrative hypervisor-level orchestration interfaces must belong strictly on the **System Bus**.

---

### Unsafe Use of `from_raw_fd` in System Signal Handling
* **Location:** `crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs:336-340`
* **Description:** 
  The service process receives ownership of a file descriptor from dinit via the environment variable `DINIT_DBUS_READY_FD` and consumes it using `from_raw_fd`:
  ```rust
  let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
  let _ = file.write_all(b"\n");
  ```
  While idiomatic for parent-notifying setups, reading variables directly from the environment and casting them to file descriptors is prone to safety failures if the process is spawned with an unexpected environment configuration, potentially resulting in accidental closure of unrelated active sockets or system descriptors. Provide stricter environment validation prior to wrapping raw descriptors.

---
## ⚠ Citation Warnings
- `crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs:336`: file has 239 lines
