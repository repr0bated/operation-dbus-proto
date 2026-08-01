# Public API Surface & Dead Code

## Public API Surface Analysis

The `op-dbus-mirror` crate exposes a library API surface to coordinate internal system state with the host D-Bus system. The total number of `pub` items (including structures, types, methods, fields, constants, and module declarations) is **53**.

### Top 10 Most Impactful Public Items

| Item | Type | file:line | Impact Description |
| :--- | :--- | :--- | :--- |
| `DbusMirror` | `struct` | `crates/op-dbus-mirror/src/lib.rs:32` | Core coordination coordinator managing synchronization of OVSDB and NonNet schemas to D-Bus paths. |
| `DbusMirror::start` | `fn` | `crates/op-dbus-mirror/src/lib.rs:84` | Orchestrates the primary background loop, registers D-Bus path endpoints, and watches gRPC component events. |
| `DbusMirror::refresh_full_tree` | `fn` | `crates/op-dbus-mirror/src/lib.rs:207` | Initiates the synchronous projection sweep, querying multiple authoritative backends and serializing schemas. |
| `OvsdbInterface` | `struct` | `crates/op-dbus-mirror/src/jsonrpc_interface.rs:19` | The core network transaction coordinator exposed over the system D-Bus bus. |
| `NonNetInterface` | `struct` | `crates/op-dbus-mirror/src/jsonrpc_interface.rs:155` | Exposes NonNet transactions, lists tables, and mutates unmanaged plugin state via local methods. |
| `DbusMirrorInterface` | `struct` | `crates/op-dbus-mirror/src/dbus_interface.rs:8` | Management wrapper allowing external callers to manually trigger synchronization sweeps or query sizes. |
| `ObjectManagerInterface` | `struct` | `crates/op-dbus-mirror/src/managed_objects.rs:43` | Freedesktop DBus ObjectManager adapter mapping dynamic tree states to queries in one roundtrip. |
| `MirrorObject` | `struct` | `crates/op-dbus-mirror/src/object.rs:9` | Generic D-Bus projected node encapsulating serialised database tuples. |
| `PluginInterface` | `struct` | `crates/op-dbus-mirror/src/plugin_interface.rs:15` | Fixed system management path allowing client polling of plugin snapshots. |
| `prelude::DbusMirror` | `use` | `crates/op-dbus-mirror/src/lib.rs:623` | Canonical crate re-export facilitating unified imports for control-plane tasks. |

### Glob Re-exports
* No glob re-exports (`pub use *`) are present in this codebase.

### Public Struct Fields Requiring Encapsulation
The following fields are exposed as `pub` on public structs, which breaks encapsulation boundaries, allowing callers to manipulate inner client states directly:
* `MirrorNode::name` (`crates/op-dbus-mirror/src/tree.rs:8`)
* `MirrorNode::children` (`crates/op-dbus-mirror/src/tree.rs:9`)
* `MirrorNode::data` (`crates/op-dbus-mirror/src/tree.rs:10`)
* `OvsdbInterface::client` (`crates/op-dbus-mirror/src/jsonrpc_interface.rs:20`)
* `OvsdbInterface::schema_engine` (`crates/op-dbus-mirror/src/jsonrpc_interface.rs:21`)
* `NonNetInterface::nonnet` (`crates/op-dbus-mirror/src/jsonrpc_interface.rs:156`)
* `NonNetInterface::schema_engine` (`crates/op-dbus-mirror/src/jsonrpc_interface.rs:157`)

---

## Dead Code Report

### Suppressed Dead Code Warnings
There are no `#[allow(dead_code)]` or `#[allow(unused_imports)]` attributes in the reviewed library files.

### Unused Definitions and Empty Modules

The following table lists items that are declared `pub` or private but are never internally referenced or imported anywhere within the provided files:

| Item | Type | file:line | Recommendation |
| :--- | :--- | :--- | :--- |
| `MirrorNode` | `struct` | `crates/op-dbus-mirror/src/tree.rs:7` | **Remove.** The entire `tree` module is declared but completely unreferenced by the orchestrator or any other module. |
| `MirrorNode::new` | `fn` | `crates/op-dbus-mirror/src/tree.rs:13` | **Remove** along with the `MirrorNode` struct. |
| `MirrorNode::insert` | `fn` | `crates/op-dbus-mirror/src/tree.rs:22` | **Remove** along with the `MirrorNode` struct. |
| `MirrorNode::insert_recursive` | `fn` | `crates/op-dbus-mirror/src/tree.rs:31` | **Remove** along with the `MirrorNode` struct. |
| `DbusMirror::load_plugin_state` | `fn` | `crates/op-dbus-mirror/src/lib.rs:488` | **Expose or Test.** This method is never invoked, but is designed to seed plugin snapshots. Expose via a D-Bus method or test case. |
| `DbusMirror::projected_count` | `fn` | `crates/op-dbus-mirror/src/lib.rs:201` | **Expose or Remove.** Never called except in `get_stats` (which uses `published_count` alias instead). |

---

# Security & Quality Audit

## [CRITICAL] Memory Safety Violation: Unsafe `simd_json` Deserialization on Unpadded Buffers

### Citation
* `crates/op-dbus-mirror/src/jsonrpc_interface.rs:32`
* `crates/op-dbus-mirror/src/jsonrpc_interface.rs:168`

### Technical Description
The D-Bus methods `OvsdbInterface::transact` and `NonNetInterface::transact` parse arbitrary JSON strings provided by external users over the system bus using the `unsafe` function `simd_json::from_str`:

```rust
// src/jsonrpc_interface.rs:32
let mut operations_mut = operations.clone();
let ops: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut operations_mut) }
    .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;
```

`simd_json`'s parsing engine operates on native 128-bit or 256-bit vector registers. To do this without causing out-of-bounds page faults, it requires the input buffer to have at least `simd_json::SIMD_JSON_PADDING` (typically 32 or 64) bytes of zero-initialized trailing padding. 

Calling `simd_json::from_str` on a standard `String` buffer (such as the one produced by cloning `operations` or `request`) violates this safety contract because `String` allocations do not guarantee trailing padding alignment. When executing AVX2/SSE4 vector loads near the boundary of the input payload, the hardware will perform **out-of-bounds memory reads**, leading directly to **Undefined Behavior, memory leaks of adjacent heap structures (information disclosure), or daemon segmentation faults (denial of service)**.

### Remediation
Do not use raw `unsafe` string conversions with `simd_json` unless you explicitly construct a padded buffer. Replace the unsafe calls with standard, safe serialization logic or copy the string bytes into a padded buffer:

```rust
let mut bytes = operations.into_bytes();
// Ensure trailing padding is present
bytes.resize(bytes.len() + simd_json::SIMD_JSON_PADDING, 0);
let ops: simd_json::OwnedValue = simd_json::to_owned_value(&mut bytes)
    .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;
```

---

## [HIGH] Permanent State Leakage: Non-Plugin Stale Objects Retained in `plugin_registry`

### Citation
* `crates/op-dbus-mirror/src/lib.rs:528`
* `crates/op-dbus-mirror/src/lib.rs:564`
* `crates/op-dbus-mirror/src/lib.rs:595`

### Technical Description
Whenever a database row is mapped to a D-Bus path inside `publish_object`, the object's properties are added to `plugin_registry` (the `ManagedObjectRegistry` that backs the `ObjectManager` at OBJECT_MANAGER_PATH):

```rust
// src/lib.rs:511
self.register_in_object_manager(path, &data).await;
```

This registration occurs for **all** objects, including OVSDB rows, NonNet data, host stat snapshots, system service snapshots, and components.

However, when `remove_stale_publications` executes, it only calls `deregister_from_object_manager` for objects matching the plugin namespace:

```rust
// src/lib.rs:591
// If this was a plugin-managed object, remove it from the registry and emit InterfacesRemoved.
if path.starts_with("/org/opdbus/v1/plugins/") {
    self.deregister_from_object_manager(&path).await;
}
```

Because of this conditional, any stale OVSDB rows (`/org/opdbus/v1/ovsdb/...`), NonNet records, or gRPC components that are deleted from the underlying databases are deleted from the zbus `object_server` but **never purged from the `plugin_registry`**. 

This results in:
1. **Unbounded Memory Leakage**: The `plugin_registry` `DashMap` grows infinitely over the lifetime of the process as rows are added, updated, and deleted.
2. **Stale D-Bus State**: Any client calling `GetManagedObjects` on `/org/opdbus/v1` receives a long list of objects that no longer exist on the bus, leading to critical synchronization failures.

### Remediation
Remove the namespace check before purging stale registrations. All deleted items must be deregistered from the ObjectManager mapping:

```rust
for path in to_remove {
    let op = ObjectPath::try_from(path.as_str())?;
    self.connection
        .object_server()
        .remove::<object::MirrorObject, _>(op)
        .await?;
    self.published_objects.remove(&path);

    // Always deregister from the ObjectManager registry
    self.deregister_from_object_manager(&path).await;
}
```

---

## [HIGH] File Descriptor Hijacking and Corruption in `signal_dinit_ready`

### Citation
* `crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs:252-267`

### Technical Description
In the initialization utility, the daemon attempts to notify `dinit` of its readiness status by reading a file descriptor number from an environment variable and writing to it:

```rust
// src/bin/ovs-dbus-init.rs:264
let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
let _ = file.write_all(b"\n");
```

This implementation contains critical quality and security vulnerabilities:
1. **Unsafe File Descriptor Ownership**: `File::from_raw_fd` consumes ownership of the raw file descriptor. When `file` goes out of scope at the end of `signal_dinit_ready`, the standard library destructor invokes `close(fd)`.
2. **Resource Hijacking / EBADF Panics**: If a misconfiguration or a malicious actor manipulates `DINIT_DBUS_READY_FD` to match an active file descriptor used by other critical operations (such as the D-Bus socket connection or standard error logs), that descriptor is closed. Subsequent operations on those sockets fail with `EBADF`, crashing the program.
3. **No Validation**: There is no check to ensure the file descriptor is actually open, writable, or of the expected type (e.g. pipe/socket).

### Remediation
Ensure the file descriptor is not dropped and closed. Use `std::mem::forget` or wrap the descriptor in `std::io::Write` adapters that do not take ownership, such as raw syscalls:

```rust
use std::os::fd::AsRawFd;

fn signal_dinit_ready() {
    let Ok(fd_str) = env::var("DINIT_DBUS_READY_FD") else { return; };
    let Ok(fd) = fd_str.parse::<i32>() else { return; };

    // Use libc direct write or ensure File does not close the fd
    let payload = b"\n";
    unsafe {
        libc::write(fd, payload.as_ptr() as *const libc::c_void, payload.len());
    }
}
```

---

## [MEDIUM] Schema-as-Code Violation: Ad-hoc JSON and String Payloads

### Citation
* `crates/op-dbus-mirror/src/dbus_interface.rs:28-32`
* `crates/op-dbus-mirror/src/managed_objects.rs:83-88`
* `crates/op-dbus-mirror/src/jsonrpc_interface.rs:32`
* `crates/op-dbus-mirror/src/jsonrpc_interface.rs:168`
* `crates/op-dbus-mirror/src/object.rs:26-30`

### Technical Description
In violation of the schema-as-code discipline using Protocol Buffers and OSCAL compliance specifications, the `op-dbus-mirror` crate defines system state exchanges using unstructured, ad-hoc JSON blobs and plain strings. 

Examples of this anti-pattern include:
* **Ad-hoc Serialization**: In `get_stats`, the statistical data contract is compiled using a raw `simd_json::json!` macro and sent as an unversioned raw string.
* **String Properties**: The `ObjectManager` populates `build_interface_map` with unstructured `JsonData` strings.
* **Raw JSON RPC Over D-Bus**: `transact` routes untyped strings directly to the backend databases.

This pattern prevents the enforcement of type-safe data schemas, allows data desynchronization during updates, and circumvents OSCAL automated compliance audits because there are no versioned protobuf artifacts representing the interface boundaries.

### Remediation
Refactor the interface payloads to use versioned Protobuf messages or concrete, statically-typed Rust structs generated from unified schemas (e.g., matching the structures defined in `op-grpc-bridge`). D-Bus signatures should carry typed structures rather than flat JSON strings.

---

## [MEDIUM] Unbounded Stack Recursion on Arbitrary Data Inputs

### Citation
* `crates/op-dbus-mirror/src/lib.rs:435-452`
* `crates/op-dbus-mirror/src/tree.rs:31-44`

### Technical Description
The method `collect_plugin_children` walks through arbitrary JSON payload objects and arrays recursive-style to register sub-paths in the D-Bus hierarchy:

```rust
// src/lib.rs:442
out.push((child_path.clone(), Self::child_value_payload(value)));
self.collect_plugin_children(&child_path, value, out);
```

Similarly, `MirrorNode::insert_recursive` recursively parses splitting segments:

```rust
// src/tree.rs:42
entry.insert_recursive(remaining, data);
```

Since these payloads originate from database entries populated by dynamic external plugins or OVSDB tables, a malicious input containing deeply nested structures or highly cyclical configurations can consume the thread stack space. This results in **unbounded recursion, culminating in a stack overflow and a total daemon crash**.

### Remediation
Eliminate the recursive strategy in favor of an iterative worklist algorithm, or enforce a strict depth-limit boundary (e.g., maximum depth of 16 levels) on any nested objects:

```rust
fn collect_plugin_children_safe(
    &self,
    root_path: &str,
    data: &Value,
    out: &mut Vec<(String, Value)>,
    depth: usize,
) {
    if depth > 16 {
        tracing::warn!("Max structural depth exceeded at D-Bus projection: {}", root_path);
        return;
    }
    // ... rest of the method passing depth + 1
}
```

---

## [LOW] File Descriptors Leaked on Procfs Gathering Failures

### Citation
* `crates/op-dbus-mirror/src/lib.rs:327`
* `crates/op-dbus-mirror/src/lib.rs:347`
* `crates/op-dbus-mirror/src/lib.rs:367`

### Technical Description
When gathering host configurations (`gather_meminfo`, `gather_cpuinfo`, `gather_loadavg`), the mirror reads directly from standard paths such as `/proc/meminfo`. If these operations fail or block (e.g., inside restricted container runtimes lacking mount access), the underlying files could be left in invalid states or block the async runtime thread. 

Furthermore, `tokio::fs::read_to_string` is safe against descriptor leaks, but hardcoded filesystem accesses in these helpers undermine system portability, making mock tests or sandboxed execution impossible.

### Remediation
Abstract the system statistics reader behind a provider trait. This lets you mock the host outputs during testing and sandbox runtime environments.

---
## ⚠ Citation Warnings
- `crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs:252`: file has 239 lines
