# License and Quality Audit: op-dbus-mirror

## 1. License Audit

### 1.1. Package License Field Extraction
* **File audited:** `crates/op-dbus-mirror/Cargo.toml`
* **Finding:** The package `op-dbus-mirror` **does not define** a `license` or `license.workspace = true` field within its package definition block. While the root workspace `Cargo.toml` defines `license = "Apache-2.0"` in `[workspace.package]`, the crate `op-dbus-mirror` does not inherit it because it has no `license.workspace = true` declaration.

### 1.2. Copyleft/Incompatible License Scan
* **File audited:** `Cargo.lock`
* **Finding:** No GPL/AGPL/SSPL copyleft dependencies are declared in the `Cargo.lock` file.
* **Note on MPL-2.0:** The crate `cozo` (version `0.7.6`) is licensed under **MPL-2.0** (Mozilla Public License 2.0). MPL-2.0 is a weak copyleft license. It is legally compatible with the workspace's Apache-2.0 license, provided any modifications to the Cozo source code itself are distributed under MPL-2.0. Since `cozo` is consumed as an unmodified upstream crate, this does not introduce any license incompatibility or proprietary contamination risk.

### 1.3. Crates with No License Field
* **Crate:** `op-dbus-mirror`
* **File reference:** `crates/op-dbus-mirror/Cargo.toml`
* **Details:** This crate misses the workspace inheritance field `license.workspace = true` or any explicit `license` field.

---

## 2. Schema-as-Code Violations

The codebase does not follow a strict schema-as-code discipline using Protocol Buffers or OSCAL at its boundaries. Instead, database rows, plugins, and metadata states are serialized and transferred across D-Bus interfaces as ad-hoc unstructured JSON strings, custom HashMaps, or native Rust structures.

### 2.1. Ad-Hoc Properties and Interfaces in `ObjectManager`
* **File & Line:** `crates/op-dbus-mirror/src/managed_objects.rs:27-31`
```rust
pub type PropertyMap = HashMap<String, String>;
pub type InterfaceMap = HashMap<String, PropertyMap>;
```
* **Violation:** Instead of returning versioned schema structures, properties are typed as an ad-hoc `HashMap<String, String>` where the property names are arbitrary keys and values are raw JSON-encoded strings.

### 2.2. Unstructured Statistics Serialized as Raw JSON
* **File & Line:** `crates/op-dbus-mirror/src/dbus_interface.rs:34`
```rust
    async fn get_stats(&self) -> zbus::fdo::Result<String> {
        let stats = simd_json::json!({
            "published_objects": self.mirror.published_count(),
            "projected_objects": self.mirror.projected_count(),
        });
        Ok(simd_json::to_string(&stats).unwrap_or_default())
    }
```
* **Violation:** Rather than publishing metrics using structured schemas, the service constructs an ad-hoc JSON value and returns it as an untyped `String` to callers.

### 2.3. Unversioned Row Data Property
* **File & Line:** `crates/op-dbus-mirror/src/object.rs:33-46`
```rust
    #[zbus(property)]
    async fn json_data(&self) -> String {
        simd_json::to_string(&self.data).unwrap_or_default()
    }

    async fn get_property(&self, key: String) -> String {
        self.data
            .get(&key)
            .map(|v| simd_json::to_string(v).unwrap_or_default())
            .unwrap_or_default()
    }
```
* **Violation:** The core database row data is queried via D-Bus as a raw JSON blob (`String`) or key-value lookups returning untyped strings, rather than as a versioned Protobuf object.

### 2.4. Ad-Hoc Plugin Snapshot Maps
* **File & Line:** `crates/op-dbus-mirror/src/plugin_interface.rs:13`
```rust
pub type PluginSnapshot = Arc<RwLock<HashMap<String, String>>>;
```
* **Violation:** Represents registered plugin metadata and active state using an ad-hoc mapping of names to raw, untyped JSON strings.

### 2.5. Raw String Payloads in Mutate Interfaces
* **File & Lines:** `crates/op-dbus-mirror/src/jsonrpc_interface.rs:33` and `crates/op-dbus-mirror/src/jsonrpc_interface.rs:172`
* **Violation:** The `transact` endpoints on both `OvsdbInterface` and `NonNetInterface` receive transaction payloads as unvalidated D-Bus `String` arguments. Input validation and parsing are handled dynamically at runtime rather than enforced by an interface-level schema contract.

### 2.6. Local Definition of Database Row Layouts
* **File & Line:** `crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs:18-25`
```rust
struct BridgeRow {
    name: String,
    uuid: String,
    datapath_type: String,
    other_config: HashMap<String, String>,
    external_ids: HashMap<String, String>,
}
```
* **Violation:** This local struct is defined ad-hoc inside a binary target without generating it from a centralized, versioned workspace database schema registry.

---

## 3. Security and Quality Findings

### 3.1. [Critical] Arbitrary File Descriptor Closure & Reassignment Vulnerability
* **File & Line:** `crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs:228-238`
* **Function:** `signal_dinit_ready()`

```rust
fn signal_dinit_ready() {
    let Ok(fd) = env::var("DINIT_DBUS_READY_FD") else {
        return;
    };
    let Ok(fd) = fd.parse::<i32>() else {
        return;
    };

    // Dinit passes ownership of this pipe fd to the service process.
    let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
    let _ = file.write_all(b"\n");
}
```

#### Description
The function `signal_dinit_ready()` parses the `DINIT_DBUS_READY_FD` environment variable and takes complete ownership of that file descriptor using `std::fs::File::from_raw_fd(fd)`. 

When the local variable `file` goes out of scope at the end of the function, the destructor of `std::fs::File` is called, which immediately **closes the file descriptor**.

If `DINIT_DBUS_READY_FD` is manipulated (e.g. by an unprivileged process spawning this service or via inherited environment manipulation) to target an active descriptor used by the application—such as `1` (stdout), `2` (stderr), or an active socket FD or event-loop descriptor (epoll/kqueue) managed by `tokio`—the descriptor will be closed silently.

#### Impact
This vulnerability can lead to two critical security failures:
1. **Denial of Service (DoS):** Closing `tokio`'s internal event-loop descriptors or logging output pipes will cause the service to panic, hang, or crash.
2. **Silent Data Hijacking / Use-After-Close FD Reassignment:** If a critical communication file descriptor (such as a database socket or a D-Bus socket) is closed, a subsequent file-opening or connection operation on another thread can be assigned the same descriptor index by the OS. The thread that originally owned that closed descriptor index may continue to perform I/O, unknowingly reading from or writing sensitive control-plane data to the newly reassigned socket or file.

#### Remediation
Do not close the file descriptor inside this process. Instead of wrapping the raw file descriptor in `std::fs::File` (which enforces drop-based closure), write to it using low-level OS primitives (e.g., `libc::write` on Unix) without taking ownership, or use `std::mem::forget(file)` to prevent Rust from invoking the close destructor on drop:

```rust
use std::io::Write;
use std::os::fd::FromRawFd;

fn signal_dinit_ready() {
    let Ok(fd_str) = env::var("DINIT_DBUS_READY_FD") else { return; };
    let Ok(fd) = fd_str.parse::<i32>() else { return; };

    unsafe {
        let mut file = std::fs::File::from_raw_fd(fd);
        let _ = file.write_all(b"\n");
        // Prevent the destructor from closing the FD, returning ownership to dinit/parent
        let _ = std::os::fd::IntoRawFd::into_raw_fd(file);
    }
}
```

---

### 3.2. [High] Unbounded Memory and CPU Exhaustion via `ovsdb_transact` Socket Loop
* **File & Line:** `crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs:166-175`
* **Function:** `ovsdb_transact()`

```rust
    let mut buffer = Vec::new();
    let mut chunk = vec![0u8; 65536];
    loop {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Ok(value) = serde_json::from_slice::<Value>(&buffer) {
            return Ok(value);
        }
    }
```

#### Description
This loop reads chunks of data from a Unix stream connected to the OVSDB socket and appends them to a buffer. It tries to deserialize the entire accumulated `buffer` on every iteration. This pattern has two significant flaws:
1. **Unbounded Memory Growth:** There is no limit placed on the size of the accumulated `buffer`. If the peer socket continues to write bytes that do not form a complete, valid JSON payload, the buffer will grow indefinitely until the process is terminated by the OS Out-Of-Memory (OOM) killer.
2. **Quadratic CPU Complexity ($O(N^2)$):** In each loop iteration, the function attempts to parse the entire accumulated buffer starting from the beginning. For large database transfers (such as heavy OVS configurations), this creates a severe CPU bottleneck due to repeated redundant parsing.

#### Impact
An attacker with write access to the OVSDB Unix socket (or a compromised/buggy database socket peer) can exhaust system memory or CPU, bringing down the bridge initialization subsystem and halting system bridge discovery.

#### Remediation
Implement a maximum buffer size limit and use a streaming parser (such as `serde_json::Deserializer::from_reader`) to parse JSON stream elements sequentially without re-evaluating the entire history from scratch:

```rust
    let mut buffer = Vec::new();
    let mut chunk = vec![0u8; 4096];
    const MAX_ALLOWED_RESPONSE_SIZE: usize = 10 * 1024 * 1024; // 10 MB limit

    loop {
        let read = stream.read(&mut chunk).await?;
        if read == 0 {
            break;
        }
        if buffer.len() + read > MAX_ALLOWED_RESPONSE_SIZE {
            return Err(anyhow!("Response size exceeded limits"));
        }
        buffer.extend_from_slice(&chunk[..read]);
        // Consider utilizing a more efficient/incremental state-machine parsing approach
        if let Ok(value) = serde_json::from_slice::<Value>(&buffer) {
            return Ok(value);
        }
    }
```

---

### 3.3. [High] Unvalidated System D-Bus Names Causing Service-Wide Denial of Service
* **File & Line:** `crates/op-dbus-mirror/src/lib.rs:502-530`
* **Function:** `publish_system_services()`

```rust
            // Sanitize for D-Bus object path: replace dots and hyphens with underscores
            let safe_name = name_str.replace('.', "/").replace('-', "_");
...
            let path = format!("/org/opdbus/v1/system/{}", safe_name);
            self.publish_object(&path, service_data).await?;
```

#### Description
The service converts registered system D-Bus service names into D-Bus object paths by replacing `.` with `/` and `-` with `_`. However, it does not sanitize other characters, nor does it check for structural anomalies like consecutive dots (`..`), leading/trailing dots (`.name` / `name.`), or characters outside the alphanumeric, underscore, and slash ranges.

If a local or system service registers with an irregular name (for example, containing special characters or consecutive dots like `org..foo`), `safe_name` will contain an invalid sequence (e.g. `org//foo` or a trailing slash).

When `publish_object` calls `ObjectPath::try_from(path)`, it will fail with an error because the path violates D-Bus specification rules. This error propagates up through the `?` operator in `publish_object(&path, service_data).await?`, aborting the execution of the entire `publish_system_services` function.

#### Impact
An unprivileged local daemon registering a specifically crafted D-Bus service name can crash or block the entire system service mirroring routine, causing a Denial of Service (DoS) of the control plane mirror.

#### Remediation
Exhaustively sanitize and validate the constructed D-Bus path segments to guarantee compliance with the D-Bus specification, and log errors locally on a per-service basis instead of aborting the entire update cycle:

```rust
    // Ensure safe_name consists strictly of valid D-Bus path segments
    let safe_name: String = name_str
        .split('.')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            segment.chars().map(|ch| if ch.is_ascii_alphanumeric() || ch == '_' { ch } else { '_' }).collect::<String>()
        })
        .collect::<Vec<String>>()
        .join("/");

    if !safe_name.is_empty() {
        let path = format!("/org/opdbus/v1/system/{}", safe_name);
        if let Err(e) = self.publish_object(&path, service_data).await {
            tracing::warn!("Failed to publish system service {}: {}", name_str, e);
        }
    }
```

---

### 3.4. [Medium] Synchronous `GetManagedObjects` Call Blocks Executor Thread
* **File & Line:** `crates/op-dbus-mirror/src/managed_objects.rs:59-66`
* **Function:** `get_managed_objects()`

```rust
#[interface(name = "org.freedesktop.DBus.ObjectManager")]
impl ObjectManagerInterface {
    /// Return every managed object with all their interface properties.
    fn get_managed_objects(&self) -> HashMap<OwnedObjectPath, InterfaceMap> {
        self.registry
            .iter()
            .map(|e| (e.key().clone(), e.value().clone()))
            .collect()
    }
}
```

#### Description
The implementation of `GetManagedObjects` is fully synchronous (`fn` instead of `async fn`) and iterates over the entire `ManagedObjectRegistry` (implemented via `DashMap`), cloning every single registered path, interface map, and key-value pair. 

Under heavy loads (for instance, the 16,000 objects tested in the performance test binary), this collection clone locks the registry and blocks the D-Bus executor thread for a significant duration.

#### Impact
Any local client on the D-Bus can send repeated `GetManagedObjects` requests, inducing high latency and blocking the asynchronous executor thread pool. This causes timeouts in database synchronization, OVSDB communication, and heartbeat tasks.

#### Remediation
Convert `get_managed_objects` to an asynchronous method and consider executing the database cloning work in a separate thread pool using `tokio::task::spawn_blocking` to avoid stalling the cooperative async executor thread:

```rust
    async fn get_managed_objects(&self) -> HashMap<OwnedObjectPath, InterfaceMap> {
        let registry = self.registry.clone();
        tokio::task::spawn_blocking(move || {
            registry
                .iter()
                .map(|e| (e.key().clone(), e.value().clone()))
                .collect()
        })
        .await
        .unwrap_or_default()
    }
```

---

### 3.5. [Medium] Parsing Incomplete `/proc/cpuinfo` Excludes Last Processor Core
* **File & Line:** `crates/op-dbus-mirror/src/lib.rs:360-379`
* **Function:** `gather_cpuinfo()`

```rust
        for line in content.lines() {
            if line.is_empty() {
                if !current_core.is_empty() {
                    cores.push(Value::Object(Box::new(current_core.clone())));
                    current_core.clear();
                }
                continue;
            }
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() == 2 {
                let key = parts[0].trim().replace(' ', "_");
                let val = parts[1].trim();
                current_core.insert(key.into(), Value::from(val.to_string()));
            }
        }
```

#### Description
In Linux, `/proc/cpuinfo` groups information about each core, separating them with a blank line. The parser in `gather_cpuinfo()` relies on finding an empty line (`line.is_empty()`) to commit and push the active `current_core` object to the `cores` list.

However, if `/proc/cpuinfo` terminates on a non-empty line (without a trailing trailing newline/empty line block), the loop will exit and leave the last parsed core in the `current_core` buffer. It is never committed to the `cores` vector.

#### Impact
On systems where the `/proc/cpuinfo` output does not terminate with an empty line, the last core is completely omitted from the reported state. This leads to inaccurate hardware configuration reporting within the database mirror.

#### Remediation
Add a final check after the loop to push any remaining core data in the buffer:

```rust
        for line in content.lines() {
            // ... [parsing loop] ...
        }
        // Commit the final core if it wasn't followed by an empty line
        if !current_core.is_empty() {
            cores.push(Value::Object(Box::new(current_core)));
        }
```

---

### 3.6. [Low] Unused Tree Walking Module (Dead Code)
* **File & Line:** `crates/op-dbus-mirror/src/tree.rs:1-50`
* **Module:** `tree`

```rust
pub struct MirrorNode {
    pub name: String,
    pub children: HashMap<String, MirrorNode>,
    pub data: Option<Value>,
}
```

#### Description
The module `tree` is declared in `crates/op-dbus-mirror/src/lib.rs` using `pub mod tree;`, but `MirrorNode` is never imported, referenced, or constructed in any logic within the entire codebase.

#### Impact
Unused code blocks increase binary size, add maintainer cognitive load, and risk compiling outdated dependencies or logic.

#### Remediation
Remove `pub mod tree;` from `lib.rs` and delete the unused file `crates/op-dbus-mirror/src/tree.rs`.

---

### 3.7. [Low] Unnecessary `unsafe` Blocks for Safe `simd_json::from_str` Parsing
* **File & Lines:** `crates/op-dbus-mirror/src/jsonrpc_interface.rs:36` and `crates/op-dbus-mirror/src/jsonrpc_interface.rs:175`

```rust
let ops: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut operations_mut) }
```

#### Description
The codebase wraps calls to `simd_json::from_str` inside `unsafe` blocks. In `simd-json` version 0.13, parsing string slices using `from_str` is a **safe** function (internally taking care of padding requirements safely or through appropriate safe wrappers). 

#### Impact
Using `unsafe` blocks for functions that do not actually have an unsafe signature violates Rust's quality principles, dilutes the auditability of actual dangerous blocks, and triggers compiler warnings or linter errors under strict codebases.

#### Remediation
Remove the `unsafe` block wrappers. Simply call `simd_json::from_str` directly:

```rust
let ops: simd_json::OwnedValue = simd_json::from_str(&mut operations_mut)
    .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;
```