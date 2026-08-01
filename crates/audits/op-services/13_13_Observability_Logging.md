### 1. Observability Metric: Tracing Macros vs. Println

A complete count of the `tracing` logging macros compared to `println!` and `eprintln!` statements across the evaluated codebase shows a clean separation between the service daemon (which uses structured tracing) and the CLI wrappers (which use print statements).

#### Tracing Macros Count (Total: 15)
*   **`tracing::info!` (8 occurrences)**
    *   `crates/op-services/src/bin/op-services.rs:22`
    *   `crates/op-services/src/bin/op-services.rs:42`
    *   `crates/op-services/src/dbus/interface.rs:104`
    *   `crates/op-services/src/manager/dinit_proxy.rs:57`
    *   `crates/op-services/src/manager/process.rs:44`
    *   `crates/op-services/src/manager/process.rs:57`
    *   `crates/op-services/src/manager/service_manager.rs:26`
    *   `crates/op-services/src/store/mod.rs:59`
*   **`tracing::warn!` (5 occurrences)**
    *   `crates/op-services/src/manager/service_manager.rs:30`
    *   `crates/op-services/src/manager/service_manager.rs:120`
    *   `crates/op-services/src/manager/service_manager.rs:131`
    *   `crates/op-services/src/manager/service_manager.rs:142`
*   **`tracing::error!` (2 occurrences)**
    *   `crates/op-services/src/bin/op-services.rs:32`
    *   `crates/op-services/src/manager/process.rs:59`
*   **`tracing::debug!` (0 occurrences)**

#### Standard Output/Error Print Count (Total: 24)
*   **`println!` (14 occurrences)**
    *   `crates/op-services/src/bin/systemctl-native.rs:31`
    *   `crates/op-services/src/bin/systemctl-native.rs:37`
    *   `crates/op-services/src/bin/systemctl-native.rs:43`
    *   `crates/op-services/src/bin/systemctl-native.rs:49`
    *   `crates/op-services/src/bin/systemctl-native.rs:50`
    *   `crates/op-services/src/bin/systemctl-native.rs:55`
    *   `crates/op-services/src/bin/systemctl.rs:27`
    *   `crates/op-services/src/bin/systemctl.rs:28`
    *   `crates/op-services/src/bin/systemctl.rs:38`
    *   `crates/op-services/src/bin/systemctl.rs:47`
    *   `crates/op-services/src/bin/systemctl.rs:56`
    *   `crates/op-services/src/bin/systemctl.rs:58`
    *   `crates/op-services/src/bin/systemctl.rs:61`
    *   `crates/op-services/src/bin/systemctl.rs:68`
*   **`eprintln!` (10 occurrences)**
    *   `crates/op-services/src/bin/systemctl-native.rs:72` to `77` (5 lines)
    *   `crates/op-services/src/bin/systemctl.rs:81` to `86` (5 lines)

---

### 2. Observability & Quality: Swallowed Errors

#### Lagged Stream Shutdown in gRPC Watcher (High Severity)
*   **Citation**: `crates/op-services/src/grpc/server.rs:219-224`
*   **Impact**: When clients watch status changes using `watch_status`, a background task polls events from a broadcast channel:
    ```rust
    tokio::spawn(async move {
        while let Ok(event) = sub.recv().await {
            if tx.send(Ok(event.into())).await.is_err() {
                break;
            }
        }
    });
    ```
    The `sub.recv().await` returns a `Result<ServiceEvent, RecvError>`. `RecvError` has two variants: `Closed` and `Lagged(u64)`. Under high-throughput conditions (e.g. many rapid service state changes), if the consumer lags behind, `sub.recv()` will yield `Err(RecvError::Lagged)`. 
    Because the loop terminates immediately on any non-`Ok` value (`while let Ok(event)`), the stream silently exits and shuts down for that client. The client is never notified of the lag, no warning is written to the logs, and the client ceases to receive any future status transitions.
*   **Remediation**: Match on the error explicitly. Log a `warn!` message if `RecvError::Lagged` occurs, but continue processing subsequent events.

#### Silent Failure of D-Bus JSON Serialization (Low Severity)
*   **Citation**: `crates/op-services/src/dbus/interface.rs:38`, `51`, `64`, `77`
*   **Impact**: Inside the D-Bus interface implementation, serialization failures are discarded using `.unwrap_or_default()`:
    ```rust
    Ok(serde_json::to_string(&status).unwrap_or_default())
    ```
    If `status` serialization fails, an empty string `""` is silently sent over D-Bus instead of returning a valid D-Bus error descriptor. This prevents diagnosis of serialization issues.
*   **Remediation**: Propagate the serialization error as a `zbus::fdo::Error::Failed`.

---

### 3. Security Audit: PII & Secrets Leakage

#### Audit of Log Output for Sensitive Material
*   **Findings**: The process environment parameters are retrieved from the store and set on the spawned command in `crates/op-services/src/manager/process.rs:37-39`:
    ```rust
    for (k, v) in &service.environment {
        cmd.env(k, v);
    }
    ```
    Crucially, the code does **not** write the environment hashmap to the tracing logger, avoiding accidental leakage of API keys, tokens, or credential variables stored within the service environment definitions.
*   **Audit Database Status**: In `crates/op-services/src/store/mod.rs:120-134`, an `audit` function is declared to write to the `audit_log` table. However, there are no calls to this audit logger anywhere in the provided service manager codebase, meaning no administrative details are currently recorded or leaked via this subsystem.

---

### 4. Observability: Metrics Instrumentation

#### Missing Metrics Omission
*   **Finding**: Although the workspace workspace dependencies list `prometheus` and `opentelemetry` as available, **no metrics instrumentation exists** within the `op-services` crate.
*   There are zero counters, gauges, or histograms tracking:
    *   D-Bus/gRPC RPC request metrics.
    *   Active/running process counts (`ProcessManager` tracked processes).
    *   Service startup/shutdown latency or execution success/failure rates.
    *   Dinit proxy connection status and message processing errors.

---

### 5. Quality Discipline: Schema-As-Code Compliance

The codebase bypasses standard schema-as-code validation patterns in multiple runtime boundaries, resorting to unstructured strings and serialization workarounds.

#### Ad-Hoc JSON Strings over D-Bus
*   **Citation**: `crates/op-services/src/dbus/interface.rs:38`, `51`, `64`, `77`
*   **Impact**: Rather than representing complex structures natively as versioned D-Bus interface types or mapped Protobuf contracts, the system serializes internal data schemas down to ad-hoc JSON strings before sending them across the D-Bus message bus:
    ```rust
    async fn start(&self, name: &str) -> zbus::fdo::Result<String> { ... }
    ```
    This completely invalidates compile-time schema-as-code guarantees for non-Rust clients relying on the D-Bus system bus.

#### Unstructured SQLite Storage Schema
*   **Citation**: `crates/op-services/src/store/mod.rs:31-39`
*   **Impact**: The SQLite schema relies on an unstructured `TEXT` field for definitions:
    ```sql
    CREATE TABLE IF NOT EXISTS services (
        name TEXT PRIMARY KEY,
        definition TEXT NOT NULL,
        ...
    )
    ```
    Instead of using versioned tables or SQL schema migrations to track structural modifications of the `ServiceDef` over time, the system writes raw JSON documents to the database. If fields are added, renamed, or deleted in the external `op-plugins` dependency, parsing database rows will fail catastrophically during startup/migration without a path for deterministic database translation or recovery.

---
## ⚠ Citation Warnings
- `crates/op-services/src/store/mod.rs:120`: file has 117 lines
