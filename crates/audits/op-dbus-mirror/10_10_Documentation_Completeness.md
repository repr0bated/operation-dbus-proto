# Production Security & Quality Audit: `op-dbus-mirror`

---

## 1. Docs (ROLE: Docs Compliance)

### 1.1 Crate-Level Documentation
* **Status**: **Passed**
* **Location**: `crates/op-dbus-mirror/src/lib.rs:1-5`
* **Comment**: Crate-level `//!` rustdoc is present in `lib.rs`, explaining the purpose of the 1:1 D-Bus publication service.

### 1.2 README.md Presence
* **Status**: **Not Found**
* **Comment**: No `README.md` was provided in the audited FILES section.

### 1.3 Public Unsafe Functions
* **Status**: **Passed**
* **Comment**: No `pub unsafe fn` declarations were found in the provided files. All `unsafe` blocks are contained inside safe/async functions.

### 1.4 Sampled `pub` Items Rustdoc Review
A sample of 10 `pub` items was verified for `///` rustdoc coverage:

| Item | File & Line | Item Signature | Has `///` Doc |
| :--- | :--- | :--- | :--- |
| **1** | `dbus_interface.rs:7` | `pub struct DbusMirrorInterface` | **No** (Failed) |
| **2** | `managed_objects.rs:40` | `pub struct ObjectManagerInterface` | **No** (Failed) |
| **3** | `plugin_interface.rs:16` | `pub struct PluginInterface` | **No** (Failed) |
| **4** | `managed_objects.rs:80` | `pub fn build_interface_map(...)` | **Yes** (Passed) |
| **5** | `object.rs:8` | `pub struct MirrorObject` | **Yes** (Passed) |
| **6** | `object.rs:16` | `pub fn update_data(...)` | **No** (Failed) |
| **7** | `lib.rs:104` | `pub fn published_count(...)` | **No** (Failed) |
| **8** | `lib.rs:596` | `pub fn list_published_paths(...)` | **No** (Failed) |
| **9** | `jsonrpc_interface.rs:18` | `pub struct OvsdbInterface` | **Yes** (Passed) |
| **10** | `jsonrpc_interface.rs:141` | `pub struct NonNetInterface` | **Yes** (Passed) |

* **Audit Flag**: **6 out of 10** sampled public items are missing `///` rustdoc comments.

---

## 2. Security Vulnerabilities

### Critical Findings

#### Heap Buffer Overread in `transact` Methods via Unpadded `simd-json` Parsing
* **Vulnerability Class**: Memory Corruption (Heap Buffer Overread / Segmentation Fault)
* **Severity**: **Critical** (Directly exploitable by unprivileged D-Bus clients)
* **Location**: `crates/op-dbus-mirror/src/jsonrpc_interface.rs:46-47` and `crates/op-dbus-mirror/src/jsonrpc_interface.rs:191-192`
* **Description**:
  The `simd-json` parser relies on SIMD vector instructions (AVX2/SSE/NEON) that read memory in chunks of 32 or 64 bytes. For these vector reads to be memory-safe, `simd-json` explicitly requires that the input buffer contain an allocation padding of `simd_json::PADDING` bytes beyond the string length. Passing a standard unpadded slice/string to unsafe parsing APIs causes undefined behavior and out-of-bounds reads.
  
  In both `OvsdbInterface::transact` and `NonNetInterface::transact`, input parameters are cloned into standard strings and parsed directly inside unsafe blocks:
  ```rust
  let mut operations_mut = operations.clone();
  let ops: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut operations_mut) }
  ```
  Because `operations_mut` is a standard cloned string, it does **not** have padding allocated at the end of its buffer. Calling `simd_json::from_str` on it allows an external caller invoking these D-Bus methods to trigger a denial-of-service crash (SIGSEGV) or potentially leak out-of-bounds heap memory.
* **Remediation**:
  Ensure the input string is converted into a padded binary buffer before parsing using `simd_json::to_padded_bin` or route it through the safe `simd_json::to_owned_value` after copying into a `Vec<u8>`:
  ```rust
  let mut padded_bytes = operations.into_bytes();
  let ops = simd_json::to_owned_value(&mut padded_bytes)
      .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;
  ```

---

### High Severity Findings

#### Arbitrary File Descriptor Closure/Hijack in `signal_dinit_ready`
* **Vulnerability Class**: Resource Management (File Descriptor Corruption & Hijacking)
* **Severity**: **High** (Exploitable if an attacker can control process environment variables)
* **Location**: `crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs:284-297`
* **Description**:
  The binary parses the environment variable `DINIT_DBUS_READY_FD` and passes it directly to `std::fs::File::from_raw_fd(fd)`:
  ```rust
  let Ok(fd) = env::var("DINIT_DBUS_READY_FD") else { ... }
  // ...
  let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
  let _ = file.write_all(b"\n");
  ```
  `from_raw_fd` takes absolute ownership of the specified file descriptor. When `file` goes out of scope at the end of the function, the underlying raw descriptor is **closed**.
  
  If a local attacker can control the execution environment of this binary, they can supply any valid descriptor (such as fd 0, 1, 2, or socket fds owned by parent processes). The program will corrupt that file descriptor by writing a newline to it and then instantly close it. This can bypass validation checks, disable logging, or crash core process streams.
* **Remediation**:
  Verify the validity of the descriptor prior to wrapping it, or do not automatically close/drop the descriptor. Instead, write to the file descriptor without taking ownership, or use `std::mem::forget(file)` after writing to prevent `drop()` from closing it.

---

## 3. Schema-As-Code Violations

The codebase consistently violates the schema-as-code discipline by utilizing ad-hoc serialized JSON strings and unstructured maps inside D-Bus contracts, instead of structured versioned types (such as Protobuf or OSCAL schemas).

### 3.1 Ad-Hoc Statistics Serialization
* **Location**: `crates/op-dbus-mirror/src/dbus_interface.rs:27-33`
* **Violation**: `get_stats` formats raw JSON objects on-the-fly and returns an untyped `String`:
  ```rust
  let stats = simd_json::json!({
      "published_objects": self.mirror.published_count(),
      "projected_objects": self.mirror.projected_count(),
  });
  Ok(simd_json::to_string(&stats).unwrap_or_default())
  ```

### 3.2 Unstructured Map Contracts
* **Location**: `crates/op-dbus-mirror/src/managed_objects.rs:23-27`
* **Violation**: Property maps are typed as raw string-to-string mappings:
  ```rust
  pub type PropertyMap = HashMap<String, String>;
  pub type InterfaceMap = HashMap<String, PropertyMap>;
  ```
  Rather than transmitting typed schemas, properties are flattened into string pairs where the value represents raw nested JSON payloads (as shown in `build_interface_map` at line 79).

### 3.3 Raw JSON String snapshots
* **Location**: `crates/op-dbus-mirror/src/plugin_interface.rs:14`
* **Violation**: `PluginSnapshot` maps plugin IDs directly to unstructured state strings (`HashMap<String, String>`). Returning untyped JSON through D-Bus methods like `get` and `get_all` breaks compiler-enforced data contracts.

### 3.4 Untyped JSON-RPC Over D-Bus Method Calls
* **Location**: `crates/op-dbus-mirror/src/jsonrpc_interface.rs:33` and `crates/op-dbus-mirror/src/jsonrpc_interface.rs:157`
* **Violation**: Both `OvsdbInterface::transact` and `NonNetInterface::transact` accept arbitrary untyped query parameters as raw strings (`operations: String`, `request: String`). This circumvents compile-time protocol boundaries.

### 3.5 Ad-Hoc Database Representation Structs
* **Location**: `crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs:22-30`
* **Violation**: `BridgeRow` represents bridge structures using ad-hoc fields and free-form string maps (`HashMap<String, String>`) instead of referencing versioned protobuf schemas.

---

## 4. Performance & Quality Warnings

### 4.1 Redundant Triple-Serialization in Snapshot Generation
* **Location**: `crates/op-dbus-mirror/src/lib.rs:379-388`
* **Description**:
  When publishing OVSDB snapshots, the code performs highly inefficient redundant serialization steps:
  1. `self.ovsdb.dump_db` is called, deserializing OVSDB structures into a standard type.
  2. The database dump is serialized to a `String` via `serde_json::to_string`.
  3. The string is converted into a vector of bytes (`into_bytes`).
  4. It is then deserialized a second time into `simd_json::OwnedValue`:
  ```rust
  let dump_serde = self.ovsdb.dump_db("Open_vSwitch").await?;
  let dump: Value = {
      let s = serde_json::to_string(&dump_serde)...
      let mut b = s.into_bytes();
      simd_json::to_owned_value(&mut b)...
  };
  ```
  On larger databases, this triple-conversion introduces significant CPU overhead and garbage collection/allocation spikes.

### 4.2 Blocking Sequential D-Bus Introspection Loop
* **Location**: `crates/op-dbus-mirror/src/lib.rs:432-475`
* **Description**:
  In `publish_system_services`, the mirror queries all system D-Bus names and calls `introspect().await` on `/` for each service sequentially in a single `for` loop. If any registered service is unresponsive or slow to reply, the background task will block the entire mirror refresh cycle. Since this cycle runs periodically, slow introspection calls will stall updates for all other databases (OVSDB/NonNet). Introspections should be executed concurrently or cached.

---
## ⚠ Citation Warnings
- `crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs:284`: file has 239 lines
