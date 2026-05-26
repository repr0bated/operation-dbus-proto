# Production Security and Quality Audit: op-dbus-mirror

## 1. Dependencies & Feature Inventory

Based on `Cargo.toml` and `Cargo.lock` files, the following direct dependencies and feature profiles are active for `op-dbus-mirror`:

| Dependency Crate | Declared Version | Enabled Features (Explicit/Implicit) | Security / Quality Notes |
| :--- | :--- | :--- | :--- |
| `op-core` | Path dep (`../op-core`) | Default | Internal workspace crate |
| `op-state` | Path dep (`../op-state`) | Default | Internal workspace crate |
| `op-jsonrpc` | Path dep (`../op-jsonrpc`) | Default | Internal workspace crate |
| `op-grpc-bridge` | Path dep (`../op-grpc-bridge`) | Default | Internal workspace crate |
| `op-network` | Path dep (`../op-network`) | Default | Internal workspace crate |
| `anyhow` | `1` | Default | Unpinned patch version |
| `tokio` | `1` | `["full"]` | Unpinned patch version |
| `zbus` | `4.0` | `["tokio"]` | Unpinned patch version |
| `serde` | `1` | `["derive"]` | Unpinned patch version |
| `serde_json` | Workspace (`1`) | Default | Unpinned patch version |
| `simd-json` | `0.13` | `["serde"]` | Unpinned patch version; implements unsafe parsing |
| `tracing` | `0.1` | Default | Unpinned patch version |
| `tracing-subscriber` | `0.3` | `["env-filter"]` | Unpinned patch version |
| `futures` | `0.3` | Default | Unpinned patch version |
| `async-trait` | `0.1` | Default | Unpinned patch version |
| `dashmap` | `5.0` | Default | Unpinned patch version |
| `zbus_xml` | Workspace (`4.0`) | Default | Unpinned patch version |

### Workspace Features
The `op-dbus-mirror` crate defines no custom `[features]` section in its local `Cargo.toml` file.

### Schema-as-Code Dependencies
* **Protocol Buffers**: The workspace includes `prost` and `tonic` but `op-dbus-mirror` has no direct schema definitions of its own. It relies on `op-grpc-bridge` to parse incoming gRPC `ComponentRegistry` structures.
* **JSON Schema / Validation**: Although the workspace defines `jsonschema = "0.29"` as a dependency, `op-dbus-mirror` does not use it to validate any incoming/outgoing OVSDB or NonNet JSON data payloads.
* **OSCAL / Compliance**: There are no OSCAL or compliance validation crates defined as direct dependencies in the `op-dbus-mirror` package.

---

## 2. Storage Backend Check

| Backend | Found at File:Line | Role (KV/Graph/Cache/Queue) | Notes / Architectural Alignment |
| :--- | :--- | :--- | :--- |
| `OvsdbClient` | `crates/op-dbus-mirror/src/lib.rs:48` | Authoritative DB client for network state | Relies on OVSDB client over Unix sockets |
| `NonNetDb` | `crates/op-dbus-mirror/src/lib.rs:49` | Authoritative DB client for non-net database | Interacted with via JSON-RPC requests |
| `cozo` | `Cargo.toml` (Workspace) | Graph / Datalog DB | Relies on `storage-sled` to avoid SQLite link conflicts |
| `sqlx` / `rusqlite` | `Cargo.toml` (Workspace) | Relational database (SQLite) | Used in surrounding workspace crates |

### Architectural Compliance Notes
* **Pure-Rust Sled Strategy**: The workspace configuration specifically uses `cozo` with `storage-sled` to avoid duplicate C-linking conflicts with sqlite3. This aligns with a clean embedded architecture.
* **No Direct Storage Access**: `op-dbus-mirror` does not instantiate raw SQL databases, Sled databases, or Cozo instances locally. Instead, it mirrors OVSDB and NonNet db clients, ensuring it remains a stateless projection plane.

---

## 3. Detailed Security Findings

### CRITICAL: Out-of-Bounds Memory Corruption via Unsafe `simd_json::from_str`
* **File:Line**: `crates/op-dbus-mirror/src/jsonrpc_interface.rs:34`, `crates/op-dbus-mirror/src/jsonrpc_interface.rs:164`
* **Vulnerability Type**: Out-of-Bounds (OOB) Memory Read/Write (CWE-119, CWE-125)
* **Exploatability**: Directly Exploitable. Any unprivileged local D-Bus client can invoke the `transact` method on either the `OvsdbV1` or `NonNetV1` interfaces. 
* **Description**:
  The `transact` methods parse user-supplied JSON strings using `simd_json::from_str` within an `unsafe` block:
  ```rust
  let mut operations_mut = operations.clone();
  let ops: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut operations_mut) }
  ```
  `simd-json` requires that parsed mutable buffers contain at least `simd_json::SIMDJSON_PADDING` bytes (typically 32 or 64 bytes) of trailing garbage allocation. If a standard `String` is cloned and parsed directly without adding this padding, the underlying SIMD instructions will read (and potentially write-back) beyond the bounds of the string's allocation. This causes memory corruption, segmentation faults, or information disclosure on untrusted inputs.
* **Remediation**:
  Avoid using `unsafe simd_json::from_str`. Instead, convert the string to a padded byte vector first, or use a safe parsing engine (such as `serde_json::from_str` or `simd_json::to_owned_value` after ensuring the input vector has explicit padding):
  ```rust
  let mut bytes = operations.into_bytes();
  // Ensure we use the safe parser or provide explicit padding before calling simd_json:
  let ops: simd_json::OwnedValue = simd_json::to_owned_value(&mut bytes)
      .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;
  ```

---

### CRITICAL: Arbitrary File Descriptor Hijacking and Closure via Environment Variables
* **File:Line**: `crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs:248`
* **Vulnerability Type**: File Descriptor Hijacking / Closure (CWE-403, CWE-912)
* **Exploatability**: Directly Exploitable. If `ovs-dbus-init` is run with elevated privileges (typical for OVS startup scripts), any local user who can invoke or pass environment variables to this service can execute a Denial of Service (DoS) or trigger Undefined Behavior.
* **Description**:
  The `signal_dinit_ready` function reads a file descriptor from `DINIT_DBUS_READY_FD` and wraps it in a standard Rust `File` instance using `unsafe { std::fs::File::from_raw_fd(fd) }`:
  ```rust
  fn signal_dinit_ready() {
      let Ok(fd) = env::var("DINIT_DBUS_READY_FD") else { return; };
      let Ok(fd) = fd.parse::<i32>() else { return; };
      let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
      let _ = file.write_all(b"\n");
  }
  ```
  When the `File` object goes out of scope at the end of the function, Rust's `Drop` implementation takes ownership of the raw file descriptor and automatically calls `close(fd)`. If an attacker sets `DINIT_DBUS_READY_FD` to a sensitive file descriptor owned by the parent or daemon process (such as a database stream, unix socket, or `stdout`/`stderr`), that descriptor is closed. Subsequent writes by the application will fail, crash, or write to newly allocated, hijacked file descriptors.
* **Remediation**:
  Ensure that the parsed `fd` is validated to be a designated write-only pipe descriptor belonging to `dinit`, or avoid wrapping it in an owning `File` structure. Use a raw `write` system call or duplicate the descriptor (`dup`) if ownership must be taken, or use `std::mem::forget` / `IntoRawFd` to prevent the socket or descriptor from being closed:
  ```rust
  use std::os::fd::AsRawFd;
  let file = unsafe { std::fs::File::from_raw_fd(fd) };
  let _ = file.write_all(b"\n");
  // Relinquish ownership to prevent dropping/closing:
  let _ = std::os::fd::IntoRawFd::into_raw_fd(file);
  ```

---

### HIGH: Denial of Service via Introspection of Blocked System Services
* **File:Line**: `crates/op-dbus-mirror/src/lib.rs:379`
* **Vulnerability Type**: Unbounded Wait / Denial of Service (CWE-400, CWE-834)
* **Exploatability**: Directly Exploitable. If any third-party service registered on the D-Bus system bus hangs or becomes unresponsive, the mirror synchronization cycle will block indefinitely.
* **Description**:
  During a full tree refresh, the mirror queries all system-level D-Bus services:
  ```rust
  if let Ok(xml) = introspect_proxy.introspect().await {
  ```
  The introspection request `introspect().await` is issued asynchronously but does not declare an explicit timeout. If a service registered on the system bus accepts the connection but never returns an XML reply, this call will hang. Because the update occurs inside the single background thread's tick interval loop, a single hung system service blocks all subsequent synchronization tasks, effectively freezing database updates for OVSDB and NonNet.
* **Remediation**:
  Wrap the introspection future in a `tokio::time::timeout` call to ensure that laggy or malicious D-Bus services do not block the publication engine:
  ```rust
  match tokio::time::timeout(std::time::Duration::from_secs(2), introspect_proxy.introspect()).await {
      Ok(Ok(xml)) => { /* process xml */ },
      _ => { tracing::warn!("Introspection timed out or failed"); }
  }
  ```

---

### MEDIUM: Missing Socket Connection Timeout in `ovs-dbus-init`
* **File:Line**: `crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs:190-213`
* **Vulnerability Type**: Unbounded Socket Read/Connection (CWE-400, CWE-248)
* **Exploatability**: Exploitable by state conditions. If `/var/run/openvswitch/db.sock` is locked, or if OVSDB is deadlocked, the startup binary will hang forever.
* **Description**:
  The `ovsdb_transact` helper attempts to connect to OVSDB and read the JSON response:
  ```rust
  let mut stream = UnixStream::connect(socket_path).await?;
  ...
  loop {
      let read = stream.read(&mut chunk).await?;
      ...
  }
  ```
  Since there is no timeout applied to either `UnixStream::connect` or the continuous `stream.read` loop, a hung socket connection blocks the entire binary initialization thread. Because `ovs-dbus-init` runs during system boot, this can block the init system (`dinit`) from completing service initialization.
* **Remediation**:
  Enforce explicit timeouts using `tokio::time::timeout` on the Unix socket operations:
  ```rust
  let mut stream = tokio::time::timeout(
      std::time::Duration::from_secs(5),
      UnixStream::connect(socket_path)
  ).await.context("OVSDB connection timeout")??;
  ```

---

## 4. Schema-as-Code Gaps

The codebase uses ad-hoc string and hashmap representations for structural properties, bypassing schema-as-code discipline:

### Ad-Hoc `HashMap` Representation for Managed Objects Properties
* **File:Line**: `crates/op-dbus-mirror/src/managed_objects.rs:29-32`, `crates/op-dbus-mirror/src/managed_objects.rs:79`
* **Gap**: Properties are converted to JSON and stored in a untyped `HashMap<String, String>` mapping property names directly to raw JSON payloads:
  ```rust
  pub type PropertyMap = HashMap<String, String>;
  pub type InterfaceMap = HashMap<String, PropertyMap>;
  ```
  This ad-hoc data contract bypasses versioned schemas (such as Protocol Buffers or JSON Schemas). Consumers must query unvalidated keys, increasing the risk of parsing errors upon interface upgrades.

### Unvalidated `Value` Serialization in `MirrorObject`
* **File:Line**: `crates/op-dbus-mirror/src/object.rs:10`, `crates/op-dbus-mirror/src/object.rs:32`
* **Gap**: The database row is stored as an unstructured `simd_json::OwnedValue` and serialized directly on-the-fly to a string representing raw `json_data`:
  ```rust
  async fn json_data(&self) -> String {
      simd_json::to_string(&self.data).unwrap_or_default()
  }
  ```
  Because the data payload is not checked against a versioned schema engine or formal Protocol Buffer specification before publication, upstream D-Bus consumers are vulnerable to schema drift and unvalidated structural changes.

### Unvalidated JSON-RPC Over D-Bus Interfaces
* **File:Line**: `crates/op-dbus-mirror/src/jsonrpc_interface.rs:31`, `crates/op-dbus-mirror/src/jsonrpc_interface.rs:152`
* **Gap**: Method calls such as `transact` accept raw unchecked `String` operations. They pass this directly to either OVSDB or NonNet without validation against formal schemas or protocol schemas, violating the requirement to keep all exposed system interfaces strongly schema-compliant.

---
## ⚠ Citation Warnings
- `crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs:248`: file has 239 lines
