# Security & Quality Audit Report

## 1. Security Assessment & Critical Findings

### CRITICAL: Memory Corruption & Out-of-Bounds Read via `simd_json::from_str` on Unpadded Strings
*   **File**: `crates/op-dbus-mirror/src/jsonrpc_interface.rs`
*   **Lines**: 36, 176
*   **Impact**: Memory corruption, Undefined Behavior (UB), and Process Crash (Denial of Service).
*   **Description**: 
    The `simd-json` crate achieves extreme performance by using SIMD vector instructions that read memory in 32-byte or 64-byte chunks. Because of this, `simd-json` explicitly requires that the input buffer has `simd_json::PADDING` bytes of allocated, initialized padding at the end. Calling `simd_json::from_str` on a standard Rust string slice that lacks this padding is highly unsafe and results in out-of-bounds reads.
    
    In both the OVSDB and NonNet interfaces, the transaction functions clone the input D-Bus string and pass it directly to `simd_json::from_str` inside an `unsafe` block:
    
    ```rust
    // Line 34-37
    async fn transact(&self, operations: String) -> zbus::fdo::Result<String> {
        let mut operations_mut = operations.clone();
        let ops: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut operations_mut) }
    ```
    
    A standard `String` cloned from the input parameters is not guaranteed to be allocated with `simd_json::PADDING` trailing bytes. If an attacker sends a D-Bus transaction payload of a specific size, the SIMD engine will read past the allocated string heap boundary, potentially reading uninitialized memory, hitting unmapped pages (SIGSEGV), and crashing the daemon.
*   **Remediation**: 
    Do not use the unsafe `simd_json::from_str` directly on unpadded strings. Instead, convert the string to a mutable byte vector, extend it with padding bytes, and use `simd_json::to_owned_value`, or parse via safe interfaces like `simd_json::serde::from_slice` after ensuring padding.

---

### HIGH: Arbitrary File Descriptor Overwrite and Close via Environment Injection
*   **File**: `crates/op-dbus-mirror/src/bin/ovs-dbus-init.rs`
*   **Lines**: 335
*   **Impact**: Privilege Escalation / Internal Socket Exhaustion / Arbitrary File Corruption.
*   **Description**:
    The `signal_dinit_ready` helper reads an environment variable `DINIT_DBUS_READY_FD`, parses it as an integer, and blindly wraps it in a raw file descriptor:
    
    ```rust
    let Ok(fd) = env::var("DINIT_DBUS_READY_FD") else { return; };
    let Ok(fd) = fd.parse::<i32>() else { return; };
    let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
    let _ = file.write_all(b"\n");
    ```
    
    When `file` goes out of scope at the end of `signal_dinit_ready()`, Rust's `Drop` implementation automatically calls `close(fd)` on that file descriptor. 
    
    If this binary is run in a context where environment variables can be manipulated by a local attacker, the attacker can set `DINIT_DBUS_READY_FD` to the file descriptor of a critical system resource (such as stdin, stdout, standard error, or a vital socket/database descriptor). The process will write a newline character into that resource and then *forcibly close* it, triggering a Denial of Service or hijacking standard input/output.
*   **Remediation**:
    Avoid using unsafe `from_raw_fd` without strict ownership verification. If ownership must be assumed, wrap the file in `std::mem::ManuallyDrop` so that Rust does not trigger an automatic `close()` on a shared descriptor, or validate that the FD matches expected initialization conditions.

---

### MEDIUM: Synchronous Blocking D-Bus Introspection in Background Sync Loop
*   **File**: `crates/op-dbus-mirror/src/lib.rs`
*   **Lines**: 509-514
*   **Impact**: Thread Starvation / Denial of Service of the Sync Loop.
*   **Description**:
    In the `publish_system_services` function (called inside `refresh_full_tree` every 30 seconds), the service retrieves a list of all active system D-Bus names and sequentially triggers an `introspect()` call on every single one of them:
    
    ```rust
    for name in names {
        ...
        let introspect_proxy = zbus::fdo::IntrospectableProxy::builder(&system_conn)
            .destination(name_str)?
            .path("/")?
            .build()
            .await?;
        ...
        if let Ok(xml) = introspect_proxy.introspect().await {
    ```
    
    Introspecting dozens of system-level services one-by-one sequentially over D-Bus is an incredibly slow operation. If any single service on the system bus lags or stalls, it will wait for the default timeout (typically 25 seconds). Because these calls are sequential and blocking within the sync loop, a single unresponsive D-Bus daemon can stall the entire `op-dbus-mirror` publication service indefinitely, preventing updates to OVSDB, NonNet, and plugins.
*   **Remediation**:
    Perform introspection calls concurrently using `futures::stream::FuturesUnordered` with a strict per-request timeout configured on the proxy builder, rather than sequentially awaiting each introspection.

---

### MEDIUM: ObjectPath Validation Failure Stalls DBus Mirror Syncer
*   **File**: `crates/op-dbus-mirror/src/lib.rs`
*   **Lines**: 458-459, 498-501
*   **Impact**: Loss of Mirror Synchronization.
*   **Description**:
    D-Bus object paths have strict grammatical rules (e.g. no hyphens, must be separated by slashes, alphanumeric/underscores only). When projecting NonNet database and table names, `db_name` and `table_name` are injected directly into the path format string:
    
    ```rust
    let path = format!(
        "/org/opdbus/v1/nonnet/{}/{}/{}",
        db_name, table_name, id
    );
    ```
    
    If a database name contains invalid characters (e.g. `non-net-db`), `ObjectPath::try_from` inside `publish_object` will fail and return an `Err`. Because these functions bubble errors up with the `?` operator (e.g., `self.publish_object(...).await?;`), a single invalid database or table name will abort the entire snapshot process for that run, leaving the mirror in an inconsistent state and preventing other valid databases from being synchronized.
*   **Remediation**:
    Sanitize `db_name` and `table_name` using `sanitize_path_segment` before compiling the path, or handle path-creation errors gracefully inside the loops instead of short-circuiting the entire synchronization pass.

---

## 2. Schema-As-Code Compliance Check

The `op-dbus-mirror` crate violates the codebase's strict *Schema-as-Code* discipline in multiple locations by expressing data contracts as ad-hoc dynamically created JSON structures and returning them as raw, untyped strings over D-Bus instead of using versioned Protocol Buffers or OSCAL models.

### Violations List:
1.  **Ad-hoc Stats JSON Generation**:
    *   **File**: `crates/op-dbus-mirror/src/dbus_interface.rs`
    *   **Lines**: 33-37
    *   **Violation**: Publication statistics are generated using an ad-hoc `simd_json::json!` macro and returned as a raw serialized string, rather than using a compiled Protobuf struct.
2.  **Ad-hoc Plugin Properties Map**:
    *   **File**: `crates/op-dbus-mirror/src/managed_objects.rs`
    *   **Lines**: 81-86
    *   **Violation**: The plugin data contract is expressed via an ad-hoc `InterfaceMap` holding an arbitrary `"JsonData"` key containing stringified JSON, bypassing formal schemas.
3.  **Untyped Database Row Projections**:
    *   **File**: `crates/op-dbus-mirror/src/object.rs`
    *   **Lines**: 32-41
    *   **Violation**: Database rows are returned as raw JSON values (`simd_json::to_string(&self.data)`) or string-queried properties (`get_property(&self, key: String)`), failing to utilize schema-defined structures.
4.  **Ad-hoc Plugin Snapshot Handlers**:
    *   **File**: `crates/op-dbus-mirror/src/plugin_interface.rs`
    *   **Lines**: 41-47, 50-52
    *   **Violation**: Returns raw, undocumented JSON string schemas (`"{\\"active\\":false}"`) for plugins, bypassing protobuf-defined states.
5.  **Ad-hoc JSON-RPC Transact Payloads**:
    *   **File**: `crates/op-dbus-mirror/src/jsonrpc_interface.rs`
    *   **Lines**: 34, 174
    *   **Violation**: Raw JSON-RPC requests and responses are passed back and forth as raw `String` arguments instead of validated Protobuf payloads.

---

## 3. Test Coverage & Quality Audit

### Test Finding: HIGH RISK - Zero Test Coverage
*   **Crate**: `op-dbus-mirror`
*   **Status**: **No tests found**

### Complete Audit Metrics:
*   **Total unit test functions (`#[test]`)**: 0
*   **Total test modules (`#[cfg(test)]`)**: 0
*   **Integration tests (`tests/`)**: None found inside the `op-dbus-mirror` workspace.
*   **Fuzzing & Property Testing (proptest, quickcheck)**: None configured for this crate.

### Risk Mitigation Notice:
The complete absence of test coverage for a low-level systems component handling unsafe pointer operations (`simd_json::from_str`) and direct file descriptor control plane integration represents an extremely high operational risk. No regression safety rails exist to prevent future security vulnerabilities, lockups, or memory corruption.