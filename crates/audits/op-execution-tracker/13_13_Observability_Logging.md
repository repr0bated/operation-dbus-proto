### Observability Macro Count

| Macro Type | Count | Location |
| :--- | :--- | :--- |
| `tracing::info!` | 6 | `crates/op-execution-tracker/src/execution_tracker.rs:98`, `131`<br>`crates/op-execution-tracker/src/telemetry.rs:28`, `42`, `47`, `59` |
| `tracing::warn!` | 1 | `crates/op-execution-tracker/src/execution_tracker.rs:169` |
| `tracing::error!` | 0 | None |
| `tracing::debug!` | 0 | None |
| `println!` | 0 | None |

---

### Swallowed Errors Without Logging

#### 1. Ignored Event Broadcast Failures
* **File & Line**: `crates/op-execution-tracker/src/execution_tracker.rs:94`, `125`, `163`
* **Risk**: High
* **Details**: The `event_sender.send(...)` method returns a `Result<usize, SendError<ExecutionEvent>>`. The codebase explicitly discards these results using `let _ = ...`. If the broadcast queue is full or if downstream receivers lag/fail, execution status changes (Started, Completed, Failed) will be silently dropped without any error logging or trace generation.

#### 2. Silently Coerced Clock Drift
* **File & Line**: `crates/op-execution-tracker/src/record.rs:79`
* **Risk**: Low
* **Details**: When capturing execution start timing, the duration since the UNIX epoch is calculated via `SystemTime::now().duration_since(...)`. On system clock drift or backward clock adjustments, this returns an `Err`. This error is silently swallowed using `unwrap_or_default()`, resetting the start time to `0` nanoseconds without emitting a warning.

#### 3. Dropped Serialization Failures in Hash & Output Summaries
* **File & Line**: `crates/op-execution-tracker/src/record.rs:268`, `296`, `297`
* **Risk**: Medium
* **Details**: 
  * At line 268, JSON serialization errors are swallowed with `unwrap_or_default()`, falling back to an empty string.
  * In `hash_execution` (lines 296–297), `simd_json::to_vec` failures are silently swallowed with `unwrap_or_default()`. If input/output values fail to serialize, they compute as empty vectors, leading to silent hash collisions and breaking chain validation/audit-trail integrity.

---

### PII and Secret Logging Risks

#### 1. Plaintext Logging of Error Payloads
* **File & Line**: `crates/op-execution-tracker/src/execution_tracker.rs:169` and `crates/op-execution-tracker/src/telemetry.rs:47`
* **Risk**: High
* **Details**: Both statements log raw error strings directly (`error = %error` and `error = ?result.error`). If tracked agent/tool executions fail due to database authentication failures, external API authorization issues, or process failures containing user variables, sensitive credentials or PII will be logged in plaintext.

#### 2. Broad Event Details Logged at Info Level
* **File & Line**: `crates/op-execution-tracker/src/telemetry.rs:59`
* **Risk**: High
* **Details**: `record_event` logs arbitrary `details` strings at `info!` level. If the details of an execution event contain personal identity attributes, query parameters, or authorization tokens used by the tools, they are propagated directly to the application logging sink.

---

### Metrics Instrumentation

The codebase relies on the `prometheus` crate to define and record custom operational performance metrics in `crates/op-execution-tracker/src/metrics.rs`.

* **Crate Used**: `prometheus` (version `0.13.4` in workspace)
* **Metrics Exposed**:
  * `mcp_executions_started_total` (IntCounter): Total number of executions started.
  * `mcp_active_executions` (IntGauge): Number of currently active executions.
  * `mcp_executions_succeeded_total` (IntCounter): Total number of successfully completed executions.
  * `mcp_executions_failed_total` (IntCounter): Total number of failed executions.
  * `mcp_execution_duration_seconds` (Histogram): Execution duration in seconds with buckets `[0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0, 60.0]`.
  * `mcp_status_transitions_total` (IntCounter): Total number of execution status transitions.
* **Scraping Interface**: 
  * `get_registry()` retrieves the custom `Registry` for scraping (line 119).
  * `get_metrics_json()` returns a simplified JSON format of the gathered metric families (line 124).

---

### Schema-as-Code Compliance

The architecture defines its internal and external data contracts using ad-hoc, manually derived Rust structures with generic unstructured JSON containers rather than formal, versioned Protocol Buffer or OSCAL schemas.

#### Ad-Hoc Data Contracts Defined in Code:
* **File & Line**: `crates/op-execution-tracker/src/execution_context.rs:9`
  * Struct `ExecutionContext` represents the runtime contract for tracing tools. It relies on `simd_json::OwnedValue` for generic unstructured metadata.
* **File & Line**: `crates/op-execution-tracker/src/execution_context.rs:68`
  * Struct `ExecutionResult` is an ad-hoc contract returning execution outputs as generic JSON values along with unformatted errors.
* **File & Line**: `crates/op-execution-tracker/src/record.rs:108`
  * Struct `ExecutionRecord` defines the auditing database record contract using unstructured fields (`input: Value`, `output: Value`, `metadata: HashMap<String, String>`).

#### Recommendation:
Align with the schema-as-code discipline by defining these contracts as versioned Protocol Buffer schemas (`.proto` files) or OSCAL Assessment Results/Metadata definitions. This ensures compile-time contract enforcement across control planes, preventing serialization incompatibilities during system updates.