# Production Quality & Security Audit: op-state-store

## 1. Observability Macro & Statement Counts

A complete scan of the provided source files was conducted to tally tracing/logging invocations against standard print statements.

### Macro / Statement Tally
* **`tracing::info!` / `info!`**: 15 occurrences
* **`tracing::warn!` / `warn!`**: 8 occurrences
* **`tracing::error!` / `error!`**: 0 occurrences
* **`tracing::debug!` / `debug!`**: 14 occurrences
* **`println!`**: 2 occurrences

### Total Ttracing Calls: 37
### Total Println Calls: 2

---

## 2. Observability & Quality Findings

### Finding 1 (Critical): Plaintext Secret Leakage in Connection Log
* **Location**: `crates/op-state-store/src/redis_stream.rs:41`
* **Code**:
  ```rust
  info!("Connecting to Redis at {}", url);
  ```
* **Impact**: The connection URL format explicitly supports credentials (e.g., `redis://:password@localhost:6379`), as highlighted in `redis_stream.rs:35-36`. Logging the raw `url` at `info!` level exposes the plaintext Redis password directly to system logs, standard output, and aggregate monitoring tools.
* **Remediation**: Parse the connection string prior to logging and redact the password field from the printed output.

---

### Finding 2 (Medium): Silent Error Swallowing in Dependency Verification
* **Location**: `crates/op-state-store/src/disaster_recovery.rs:405`
* **Code**:
  ```rust
  // Search for installed packages (filter: INSTALLED=2)
  let result: std::result::Result<(), zbus::Error> = tx_proxy
      .call("SearchNames", &(2u64, vec![package_name.to_string()]))
      .await;

  // If we get a result without error, package exists
  Ok(result.is_ok())
  ```
* **Impact**: If the zbus D-Bus connection is severed, timeouts, or permission constraints are violated, `result` will evaluate to `Err(zbus::Error)`. The code maps any error variant strictly to `result.is_ok() == false` and returns `Ok(false)` (interpreting query failure as "package is not installed"). This swallows crucial connectivity/permission errors and forces false-positive install operations.
* **Remediation**: Propagate the D-Bus `zbus::Error` up the stack or log it as a warning before returning `false`.

---

### Finding 3 (Minor): Ignored Fallback Executions
* **Location**: `crates/op-state-store/src/schema_shuttle.rs:126`
* **Code**:
  ```rust
  Command::new("sh")
      .arg("-c")
      .arg(format!(
          "export X_GHOSTBRIDGE_FOOTPRINT='{}' && export X_GHOSTBRIDGE_TRACE_ID='{}' && systemctl reload xray", 
          new_footprint_hex, trace_id
      ))
      .spawn()?;
  ```
* **Impact**: Spawns `systemctl reload xray` asynchronously but does not wait for execution to complete or check the exit status. If the command fails to reload the system service, the error is swallowed without logging or state tracking.
* **Remediation**: Wait for command completion via `.status()` or handle the exit code, logging any non-zero results.

---

### Finding 4 (Minor): Swallowed Serialization Failures
* **Locations**: 
  * `crates/op-state-store/src/event_chain.rs:183`
  * `crates/op-state-store/src/event_chain.rs:539`
  * `crates/op-state-store/src/disaster_recovery.rs:173`
* **Code**:
  ```rust
  let canonical = simd_json::serde::to_owned_value(&payload).unwrap_or_default();
  ```
  ```rust
  let canonical_str = simd_json::to_string(value).unwrap_or_default();
  ```
  ```rust
  let state_json = simd_json::to_string(&state).unwrap_or_default();
  ```
* **Impact**: Under unexpected data constraints (e.g. infinite recursion, invalid types), serialization fails. Utilizing `unwrap_or_default()` causes these failures to occur silently, outputting empty JSON representations or empty hashes instead of raising errors.
* **Remediation**: Bubble up the `simd_json::Error` or log the failure to trace invalid payloads.

---

### Finding 5 (Minor): Missing/Swallowed Redis Deserialization Errors
* **Locations**: 
  * `crates/op-state-store/src/redis_stream.rs:318`
  * `crates/op-state-store/src/redis_stream.rs:338`
* **Code**:
  ```rust
  if let Ok(event) = unsafe { simd_json::from_str::<JobEvent>(&mut value) } {
      events.push(event);
  }
  ```
* **Impact**: Malformed event structures or outdated schema payloads present in the Redis stream are dropped silently. The application does not log warning details when elements fail to parse.
* **Remediation**: Add a warning/debug log within an `else` block to trace deserialization issues.

---

## 3. Metrics Instrumentation Summary

Metrics are implemented via the `prometheus` crate in `crates/op-state-store/src/metrics.rs`. The following counters, gauges, and histograms are declared:

### Job Execution Metrics
* `op_state_jobs_created_total` (Counter): Cumulative count of jobs created.
* `op_state_jobs_by_status` (GaugeVec): Dynamic count of current jobs labeled by `status` (`pending`, `running`, `completed`, `failed`).
* `op_state_job_transitions_total` (CounterVec): Transitions tracked by `from_status` and `to_status`.
* `op_state_job_duration_seconds` (HistogramVec): Job completion latencies labeled by `tool_name`.

### Operations & Storage Metrics
* `op_state_store_operation_seconds` (HistogramVec): Measures latency for `operation` and `store_type` keys.
* `op_state_store_errors_total` (CounterVec): Tallies database/cache level errors by `operation`, `store_type`, and `error_type`.
* `op_state_sqlite_pool_size` (Gauge): Displays current SQLx SQLite pool size.
* `op_state_sqlite_db_size_bytes` (Gauge): File size of the SQLite database.

### Redis Stream Metrics
* `op_state_redis_connected` (Gauge): Connection state flag (1 = connected, 0 = disconnected).
* `op_state_redis_stream_length` (GaugeVec): Monitored lengths of Redis streams (`jobs`, `plugins`).
* `op_state_redis_operations_total` (CounterVec): Cumulative Redis operations tracked by action type.

---

## 4. Schema-as-Code Compliance Review

The codebase contains strict compliance directives to use versioned schemas (such as JSON Schema 2026, Protobuf, or OSCAL) for data definitions. However, several internal integration structures violate this discipline by defining ad-hoc structs and unstructured types.

### Non-Compliant Data Contracts
1. **Ad-hoc Serialization Structs**:
   * **Location**: `crates/op-state-store/src/disaster_recovery.rs:15-84`
   * **Structures**: `SystemDependency`, `PluginStateExport`, `DisasterRecoveryExport`, `HostInfo`, `RestoreResult`
   * **Violation**: These public-facing disaster recovery data contracts are written as ad-hoc Rust structs serialized directly to JSON via `simd_json`/`serde` rather than using OSCAL or Protobuf definitions.
2. **Untyped JSON Blobs**:
   * **Location**: `crates/op-state-store/src/execution_job.rs:21-29`
   * **Structure**: `ExecutionJob`
   * **Violation**: Utilizes `simd_json::OwnedValue` as a generic argument payload placeholder rather than strict versioned schema representations.
3. **Ad-hoc Audit Logs**:
   * **Location**: `crates/op-state-store/src/event_chain.rs:114-154`
   * **Structure**: `ChainEvent`
   * **Violation**: Serializes metadata components dynamically to JSON payload schemas rather than employing standardized versioned protobuf contracts.