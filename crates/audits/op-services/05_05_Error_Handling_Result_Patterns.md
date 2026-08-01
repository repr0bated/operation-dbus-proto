# Quality & Security Audit Report: `op-services`

## 1. Error Handling Metrics

| Metric / Operator | Count | Notes / Observations |
| :--- | :--- | :--- |
| `.unwrap()` | 0 | **Excellent**: No direct panic-inducing unwraps exist in the codebase. |
| `.expect()` | 0 | **Excellent**: No panic-inducing expect calls exist in the codebase. |
| `.unwrap_or()` | 6 | Used safely for local fallback defaults (e.g., ports, state-defaults, delays). |
| `.unwrap_or_else()` | 2 | Used safely for environment variable defaults and default struct instances. |
| `.unwrap_or_default()`| 5 | Used for serializing values to fallback JSON/strings. |
| `?` operator | 76 | Broadly used across all modules to propagate errors cleanly via `anyhow::Result` or `tonic::Status`. |
| `todo!()` | 0 | No active placeholders or stubbed-out logic in production. |
| `unimplemented!()` | 0 | No unimplemented pathways. |
| `panic!()` | 0 | No manual panic aborts in the codebase. |

---

## 2. Analysis of Safe Fallback (Unwrap Variant) Sites

Since there are **zero** raw `.unwrap()` or `.expect()` calls in the provided source files, the codebase is free of immediate unwrap-induced panic risks. To provide a comprehensive review, the first 5 occurrences of safe fallback unwrap variants (`unwrap_or`, `unwrap_or_else`, and `unwrap_or_default`) are audited below:

### Site 1: `crates/op-services/src/bin/op-services.rs:44-46`
```rust
    let addr = std::env::var("OP_SERVICES_GRPC_ADDR")
        .unwrap_or_else(|_| "[::]:50053".to_string())
        .parse()?;
```
* **Context**: Parsing the gRPC listening address with a default fallback if the environment variable is missing.
* **Result vs Panic Recommendation**: **Result/Safe Fallback Appropriate**. Using `unwrap_or_else` is the correct Rust pattern here. A panic is avoided, and fallback to localhost/all-interfaces port 50053 is a standard, safe default.

### Site 2: `crates/op-services/src/bin/systemctl.rs:30-33`
```rust
            if let Some(status) = resp.into_inner().status {
                println!(
                    "State: {:?}",
                    ServiceState::try_from(status.state).unwrap_or(ServiceState::StateStopped)
                );
            }
```
* **Context**: Converting a gRPC response state integer into a structured `ServiceState` enum.
* **Result vs Panic Recommendation**: **Result/Safe Fallback Appropriate**. Falling back to `StateStopped` is highly resilient. However, if the protocol receives an undefined/corrupt state integer, silently converting it to `StateStopped` could mask server-side communication issues. 
* **Refinement**: Consider printing a warning or erroring out if `try_from` fails, rather than silently defaulting to `StateStopped`.

### Site 3: `crates/op-services/src/bin/systemctl.rs:59-61`
```rust
                let state =
                    ServiceState::try_from(status.state).unwrap_or(ServiceState::StateStopped);
```
* **Context**: Similar to Site 2, parsing service state for display.
* **Result vs Panic Recommendation**: **Result/Safe Fallback Appropriate**. See recommendation for Site 2.

### Site 4: `crates/op-services/src/dbus/interface.rs:33`
```rust
        Ok(serde_json::to_string(&status).unwrap_or_default())
```
* **Context**: Serializing status structures to JSON strings for D-Bus returns.
* **Result vs Panic Recommendation**: **Result Recommended**. Returning an empty string `""` (the default for `String`) when serialization fails hides critical system-state failures.
* **Refinement**: Map serialization errors to a D-Bus error (`zbus::fdo::Error::Failed`) using `map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?` instead of returning a silently empty string.

### Site 5: `crates/op-services/src/dbus/interface.rs:46`
```rust
        Ok(serde_json::to_string(&status).unwrap_or_default())
```
* **Context**: Serializing stop command status to JSON strings for D-Bus returns.
* **Result vs Panic Recommendation**: **Result Recommended**. See recommendation for Site 4. Map serialization errors to a formal D-Bus error payload.

---

## 3. Lock Poisoning Risk Analysis

A common failure mode in multi-threaded Rust systems is lock poisoning. When a thread panics while holding a standard library lock (`std::sync::Mutex` or `std::sync::RwLock`), the lock becomes poisoned. Subsequent attempts to acquire the lock return an `Err(PoisonError)`, which developers frequently discard via `.lock().unwrap()`, cascading the panic across threads.

### Audit Findings:
* `ProcessManager` in `crates/op-services/src/manager/process.rs:14` uses `tokio::sync::RwLock` to guard the active process map:
  ```rust
  processes: RwLock<HashMap<ServiceName, u32>>,
  ```
* `ServiceManager` in `crates/op-services/src/manager/service_manager.rs:18` uses `tokio::sync::RwLock` to guard service statuses:
  ```rust
  statuses: Arc<RwLock<HashMap<ServiceName, ServiceStatus>>>,
  ```

### Assessment:
**Immune to Lock Poisoning**. The codebase uses `tokio::sync::RwLock` exclusively. Unlike `std::sync::RwLock`, Tokio’s asynchronous lock implementation **does not implement lock poisoning**. The `.read().await` and `.write().await` calls return the guard directly, rather than returning a `Result`. This guarantees that if a task panics while holding a lock, the lock is freed normally on task cleanup, and subsequent tasks can acquire the lock safely without risk of lock-poisoning panics.

---

## 4. Schema-as-Code Compliance Review

The system-wide manager relies on versioned data contracts to coordinate system-state operations. A strict **schema-as-code** discipline requires all data boundaries to be modeled using strongly versioned schemas (such as Protocol Buffers or OSCAL JSON schemas) rather than ad-hoc Rust structs or raw JSON strings.

Three non-compliant boundaries were identified where ad-hoc JSON serialization bypasses the schema-as-code discipline:

### Finding 1: Ad-hoc JSON Serialization Over D-Bus Interfaces
* **Location**: `crates/op-services/src/dbus/interface.rs:33, 46, 59, 72`
* **Violined Contract**:
  ```rust
  async fn start(&self, name: &str) -> zbus::fdo::Result<String> {
      ...
      Ok(serde_json::to_string(&status).unwrap_or_default())
  }
  ```
* **Risk**: High architectural drift. D-Bus clients expect strongly typed IPC boundaries. Serializing a dynamic Rust struct to a raw JSON string and returning it as a generic D-Bus `String` is an ad-hoc protocol contract. If the daemon’s internal `ServiceStatus` struct evolves, the D-Bus interface contract silently breaks without compile-time validation for external clients.
* **Remediation**: Use native D-Bus structures/dictionaries mapped via `zbus::zvariant::Type` and `serde::Serialize`, or compile versioned Protocol Buffer payloads to send as binary arrays (`ay`) over D-Bus.

### Finding 2: Unvalidated D-Bus Response Binding in Native Clients
* **Location**: `crates/op-services/src/bin/systemctl-native.rs:33, 40, 47, 54`
* **Violined Contract**:
  ```rust
  let result: String = proxy.call("Start", &(name.as_str(),)).await?;
  ```
* **Risk**: Medium. The native systemctl utility consumes the raw JSON string and outputs it directly to the console. This introduces loose interface typing, rendering the client unable to perform structured verification or schema validation of systemctl status fields without resorting to ad-hoc parsing.
* **Remediation**: Integrate the D-Bus client with a versioned Protobuf or JSON schema validation layer before consuming or printing server payloads.

### Finding 3: Raw JSON Text Fields in SQL Storage (Database-as-Code Violation)
* **Location**: `crates/op-services/src/store/mod.rs:64, 70`
* **Violined Contract**:
  ```rust
  // SQLite Migration Schema:
  // definition TEXT NOT NULL

  // Serialization:
  let json = serde_json::to_string(service)?;

  // Deserialization:
  Ok(Some(serde_json::from_str(&json)?))
  ```
* **Risk**: Medium. Storing the primary service configuration definition as an unvalidated JSON string (`TEXT`) in SQLite bypasses relational database constraints. The database has no awareness of the layout of `ServiceDef`, making migrations, schema enforcement, and structural indexing impossible. It is highly vulnerable to silent configuration corruption.
* **Remediation**: Store the configuration as a validated document. Either:
  1. Define a JSON Schema for the SQLite column using SQLite's JSON1 extension functions, validating the text layout on write.
  2. Map the data structure to database tables with distinct, typed columns matching the versioned Protobuf schemas.
  3. Store the configuration as a compiled, versioned Protobuf binary blob (`BLOB`) instead of an ad-hoc JSON text string.