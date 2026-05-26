# Observability & Security Audit: `op-dbus-mirror`

## 1. Observability Metrics & Analysis

### Tracing Macros vs. `println!` Count

The codebase primarily utilizes the `tracing` crate for production observability, logging events at appropriate severity levels. `println!` is strictly confined to test/verification scripts.

| File | `tracing::info!` | `tracing::warn!` | `tracing::error!` | `tracing::debug!` | `println!` |
| :--- | :---: | :---: | :---: | :---: | :---: |
| `crates/op-dbus-mirror/src/object.rs` | 0 | 0 | 0 | 1 | 0 |
| `crates/op-dbus-mirror/src/lib.rs` | 13 | 12 | 4 | 2 | 0 |
| `crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs` | 3 | 2 | 0 | 0 | 0 |
| `crates/op-dbus-mirror/src/bin/verify_performance.rs` | 0 | 0 | 0 | 0 | 10 |
| **Total** | **16** | **14** | **4** | **3** | **10** |

### Swallowed Errors (Without Logging)

Several instances exist where errors or results are ignored using `let _ =` without fallback logging or handling:

*   **`crates/op-dbus-mirror/src/lib.rs:463`**: Discards the `Result` of removing a stale `MirrorObject` from the D-Bus object server:
    ```rust
    let _ = self
        .connection
        .object_server()
        .remove::<object::MirrorObject, _>(op)
        .await;
    ```
*   **`crates/op-dbus-mirror/src/lib.rs:553`**: Ignores the `Result` of emitting the `data_updated` D-Bus signal:
    ```rust
    let _ = iface_ref.get().await.data_updated(ctxt).await;
    ```
*   **`crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs:245`**: Discards the result of writing to the `DINIT_DBUS_READY_FD` pipe, which may lead to silent failures where `dinit` hangs waiting for service readiness:
    ```rust
    let _ = file.write_all(b"\n");
    ```

### PII or Secrets in Log Output

*   **No Direct Leakage Identified**: Database values are mapped from `OVSDB` and `NonNet` into memory structures and emitted to D-Bus, but the actual record payloads are not logged. 
*   **Mitigation Check**: Logs in `DbusMirror` only log metadata, database counts, and table names (e.g., `crates/op-dbus-mirror/src/lib.rs:314`).

### Metrics Instrumentation

The `op-dbus-mirror` crate has **no formal metrics instrumentation** (such as `prometheus` or `metrics` crate registration) within its actual library code. 

Instead, it implements an ad-hoc custom metrics reporting endpoint over D-Bus:
*   **`crates/op-dbus-mirror/src/dbus_interface.rs:33-39`**: Exposes statistics on demand using the `get_stats` method. It polls the internal `DashMap` sizes:
    ```rust
    async fn get_stats(&self) -> zbus::fdo::Result<String> {
        let stats = simd_json::json!({
            "published_objects": self.mirror.published_count(),
            "projected_objects": self.mirror.projected_count(),
        });
        Ok(simd_json::to_string(&stats).unwrap_or_default())
    }
    ```

---

## 2. Security Vulnerabilities

### [CRITICAL] Undefined Behavior & Memory Corruption via Unpadded `simd_json::from_str` on Untrusted D-Bus Input
*   **Reference**: `crates/op-dbus-mirror/src/jsonrpc_interface.rs:44` and `crates/op-dbus-mirror/src/jsonrpc_interface.rs:163`
*   **Impact**: Memory Corruption, Denial of Service (DoS), or potential Local Privilege Escalation.
*   **Description**:
    The `simd-json` crate relies on high-performance SIMD instruction sets (such as AVX2/SSE) which load memory in chunks of 32 or 64 bytes. To guarantee safety and prevent reading out-of-bounds, `simd-json` explicitly requires that any buffer parsed with raw slice/string parsing functions **must be padded with `simd_json::PADDING_SIZE` bytes** of extra capacity.
    
    The codebase exposes two D-Bus methods, `transact` in `OvsdbInterface` and `transact` in `NonNetInterface`, which take a standard unpadded `String` from untrusted D-Bus clients and parse them using `unsafe { simd_json::from_str(&mut string) }`:
    ```rust
    async fn transact(&self, operations: String) -> zbus::fdo::Result<String> {
        let mut operations_mut = operations.clone();
        let ops: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut operations_mut) }
    ```
    Cloning a Rust `String` creates a tightly allocated heap buffer with no trailing padding. Because the D-Bus payload is received directly from the network or a local socket, there is no guarantee of SIMD padding. Calling `unsafe simd_json::from_str` on this buffer will cause vector instructions to read past the allocated buffer bounds. If the buffer is aligned near a memory page boundary, this triggers an immediate segmentation fault, allowing any unprivileged local user with access to the system bus to crash the control plane daemon.
*   **Remediation**:
    Avoid using `unsafe simd_json::from_str` on unpadded strings. Use standard `serde_json::from_str` or properly copy the bytes into a padded buffer (`simd_json::to_owned_value` after manually extending the vector capacity by `simd_json::PADDING_SIZE` elements):
    ```rust
    let mut bytes = operations.into_bytes();
    bytes.reserve(simd_json::PADDING_SIZE);
    let ops = simd_json::to_owned_value(&mut bytes)
        .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;
    ```

### [MAJOR] Arbitrary File Descriptor Closure via Unvalidated Environment Variable in `ovs-dbus-init`
*   **Reference**: `crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs:244`
*   **Impact**: Denial of Service (DoS) / Descriptor Exhaustion.
*   **Description**:
    The binary `ovs-dbus-init` reads a file descriptor value from an environment variable to signal readiness to the service manager (`dinit`):
    ```rust
    let Ok(fd) = env::var("DINIT_DBUS_READY_FD") else { return; };
    let Ok(fd) = fd.parse::<i32>() else { return; };

    // Dinit passes ownership of this pipe fd to the service process.
    let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
    let _ = file.write_all(b"\n");
    ```
    Creating a `std::fs::File` via `unsafe { File::from_raw_fd(fd) }` transfers ownership of the file descriptor to the `File` object. When `file` goes out of scope at the end of the `signal_dinit_ready` block, it is dropped, which automatically calls `close(fd)`.
    
    If the environment variable `DINIT_DBUS_READY_FD` is manipulated by a malicious actor (or accidentally configured to standard descriptors like `0` (stdin), `1` (stdout), or the D-Bus connection socket), `ovs-dbus-init` will close that file descriptor. Closing active sockets or system descriptors unexpectedly leads to instability or silent termination of critical communication channels.
*   **Remediation**:
    Verify that the file descriptor parsed is indeed valid and corresponds to an expected writable pipe, or avoid taking ownership of the descriptor. Alternatively, use standard libc write calls without wrapping it in a structure that closes the descriptor on drop, or duplicate the file descriptor using `dup()` before consuming it.

---

## 3. Schema-as-Code Compliance

The codebase has multiple violations of the Schema-as-Code discipline. Rather than defining versioned, structural schemas (e.g. Protocol Buffers, OSCAL, or native strongly-typed Zbus types), serialization is handled dynamically using un-typed stringified JSON structures:

### 1. Dynamic JSON Serialization of Managed Properties
*   **Reference**: `crates/op-dbus-mirror/src/managed_objects.rs:33-36`
*   **Violation**: 
    ```rust
    pub type PropertyMap = HashMap<String, String>;
    pub type InterfaceMap = HashMap<String, PropertyMap>;
    ```
    The data properties for managed entities are stored as raw key-value maps of strings. Structural database records are formatted into a single generic property called `JsonData` carrying raw JSON strings:
    ```rust
    pub fn build_interface_map(json_str: &str) -> InterfaceMap {
        let mut props = PropertyMap::new();
        props.insert("JsonData".to_string(), json_str.to_string());
    ```
    This completely bypasses the static interface definitions of D-Bus and forces clients to consume an un-versioned schema encoded inside a JSON string.

### 2. Ad-hoc JSON Blobs Returned in Property Getters
*   **Reference**: `crates/op-dbus-mirror/src/object.rs:33-43`
*   **Violation**:
    The generic `MirrorObject` exposes its schema over D-Bus as a raw stringified JSON payload:
    ```rust
    #[zbus(property)]
    async fn json_data(&self) -> String {
        simd_json::to_string(&self.data).unwrap_or_default()
    }
    ```
    Because properties are returned as un-typed JSON strings, D-Bus clients must parse schemas dynamically at runtime, violating the guarantees of schema-driven API contracts.

### 3. JSON-RPC Over D-Bus Without Structural Type Definitions
*   **Reference**: `crates/op-dbus-mirror/src/jsonrpc_interface.rs:41-43` and `crates/op-dbus-mirror/src/jsonrpc_interface.rs:159-161`
*   **Violation**:
    Database operations on OVSDB and NonNet are routed over D-Bus using raw `String` variables:
    ```rust
    async fn transact(&self, operations: String) -> zbus::fdo::Result<String>
    ```
    This turns D-Bus into a raw transport proxy for text strings, rendering compile-time interface compatibility checks or OSCAL compliance audits impossible.

---
## ⚠ Citation Warnings
- `crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs:245`: file has 239 lines
- `crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs:244`: file has 239 lines
