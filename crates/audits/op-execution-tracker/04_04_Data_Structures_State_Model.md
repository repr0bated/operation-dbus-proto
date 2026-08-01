### Data Structure & Concurrency Primitive Counts

#### `crates/op-execution-tracker/src/execution_context.rs`
* **`Arc` count**: 0
* **`Rc` count**: 0
* **`RefCell` count**: 0
* **`RwLock` count**: 0
* **`Mutex` count**: 0
* **`OnceCell` count**: 0
* **`.clone()` count**: 2
  * `parent.trace_id.clone()` (line 115)
  * `parent.execution_id.clone()` (line 116)
* **Globally mutable state**: None
* **Large Structs (> 5 public fields)**: 
  * `ExecutionContext` (lines 8–32): Contains 8 public fields (`execution_id`, `trace_id`, `parent_id`, `tool_name`, `status`, `created_at`, `updated_at`, `metadata`).

---

#### `crates/op-execution-tracker/src/execution_tracker.rs`
* **`Arc` count**: 4
  * `records: Arc<RwLock<Vec<ExecutionRecord>>>` (line 55)
  * `stats: Arc<RwLock<ExecutionStats>>` (line 59)
  * `records: Arc::new(...)` (line 69)
  * `stats: Arc::new(...)` (line 71)
* **`Rc` count**: 0
* **`RefCell` count**: 0
* **`RwLock` count**: 5
  * `use tokio::sync::{broadcast, RwLock};` (line 9)
  * `records: Arc<RwLock<Vec<ExecutionRecord>>>` (line 55)
  * `stats: Arc<RwLock<ExecutionStats>>` (line 59)
  * `RwLock::new(Vec::with_capacity(max_history))` (line 69)
  * `RwLock::new(ExecutionStats::default())` (line 71)
* **`Mutex` count**: 0
* **`OnceCell` count**: 0
* **`.clone()` count**: 7
  * `records.push(record.clone());` (line 98)
  * `ExecutionEvent::Started(Box::new(record.clone()))` (line 107)
  * `record.tool_name.clone()` (line 126)
  * `error.clone()` (line 146)
  * `record.tool_name.clone()` (line 158)
  * `record.tool_name.clone()` (line 162)
  * `self.stats.read().await.clone()` (line 223)
* **Globally mutable state**: None
* **Large Structs (> 5 public fields)**:
  * `ExecutionStats` (lines 11–19): Contains 6 public fields (`total_executions`, `successful_executions`, `failed_executions`, `total_duration_ms`, `executions_by_tool`, `failures_by_tool`).

---

#### `crates/op-execution-tracker/src/lib.rs`
* **`Arc` count**: 0
* **`Rc` count**: 0
* **`RefCell` count**: 0
* **`RwLock` count**: 0
* **`Mutex` count**: 0
* **`OnceCell` count**: 0
* **`.clone()` count**: 0
* **Globally mutable state**: None
* **Large Structs (> 5 public fields)**: None

---

#### `crates/op-execution-tracker/src/metrics.rs`
* **`Arc` count**: 2
  * `registry: Arc<RwLock<Registry>>` (line 31)
  * `registry: Arc::new(...)` (line 89)
* **`Rc` count**: 0
* **`RefCell` count**: 0
* **`RwLock` count**: 2
  * `registry: Arc<RwLock<Registry>>` (line 31)
  * `RwLock::new(registry)` (line 89)
* **`Mutex` count**: 0
* **`OnceCell` count**: 0
* **`.clone()` count**: 7
  * `executions_started.clone()` (line 41)
  * `active_executions.clone()` (line 47)
  * `executions_succeeded.clone()` (line 53)
  * `executions_failed.clone()` (line 59)
  * `execution_duration.clone()` (line 69)
  * `status_transitions.clone()` (line 75)
  * `self.registry.read().await.clone()` (line 114)
* **Globally mutable state**: None
* **Large Structs (> 5 public fields)**: None

---

#### `crates/op-execution-tracker/src/record.rs`
* **`Arc` count**: 0
* **`Rc` count**: 0
* **`RefCell` count**: 0
* **`RwLock` count**: 0
* **`Mutex` count**: 0
* **`OnceCell` count**: 0
* **`.clone()` count**: 4
  * `id: id.clone(),` (line 196)
  * `id.clone()` (line 197)
  * `id: id.clone(),` (line 324)
  * `output: self.output.clone(),` (line 328)
* **Globally mutable state**: None
* **Large Structs (> 5 public fields)**:
  * `ExecutionTiming` (lines 45–58): Contains 6 public fields (`started_at`, `ended_at`, `monotonic_ns`, `duration_ms`, `duration_ns`, `wallclock_ns`).
  * `ExecutionRecord` (lines 92–128): Contains 17 public fields (`id`, `trace_id`, `tool_name`, `input`, `output`, `status`, `timing`, `policy_id`, `plugin_core_hash`, `tunable_hash`, `prev_hash`, `exec_hash`, `output_summary`, `error`, `success`, `initiated_by`, `metadata`).

---

#### `crates/op-execution-tracker/src/telemetry.rs`
* **`Arc` count**: 0
* **`Rc` count**: 0
* **`RefCell` count**: 0
* **`RwLock` count**: 0
* **`Mutex` count**: 0
* **`OnceCell` count**: 0
* **`.clone()` count**: 0
* **Globally mutable state**: None
* **Large Structs (> 5 public fields)**: None

---

### Schema-as-Code Violations

#### 1. Ad-Hoc Dynamic Data Contracts in Trace Tracking
* **Citation**: `crates/op-execution-tracker/src/execution_context.rs:31`
* **Defect**: The `metadata` field is typed as `simd_json::OwnedValue` instead of referencing a schema-defined configuration or standard OSCAL object metadata struct. Ad-hoc representation of metadata properties across the control plane limits compile-time validations and version compatibility guarantees between services.

#### 2. Ad-Hoc Audit Log Tracking Struct
* **Citation**: `crates/op-execution-tracker/src/record.rs:92`
* **Defect**: The crate acts as a system accountability and audit layer, yet the main interface `ExecutionRecord` is implemented as an ad-hoc Rust struct with unstructured dynamic JSON inputs and outputs (`input: Value`, `output: Value`) and a loose `metadata: HashMap<String, String>`. It does not conform to standardized, versioned machine-readable system event schema contracts such as OSCAL Assessment Results, nor does it serialize using Protocol Buffers despite gRPC/Protobuf dependencies being pulled in at the workspace level.

---

### Security & Quality Findings

#### 1. CRITICAL: Unchecked UTF-8 Character Boundary Slicing Leading to Denial-of-Service (DoS)
* **Citation**: `crates/op-execution-tracker/src/record.rs:353`
* **Severity**: Critical
* **Impact**: Unconditional panic (thread/task abort) on tool execution output parsing.
* **Description**:
  The `truncate_string` utility slices a raw byte length of `max_len` from a `&str` without checking char boundaries:
  ```rust
  fn truncate_string(s: &str, max_len: usize) -> String {
      if s.len() <= max_len {
          s.to_string()
      } else {
          format!("{}... (truncated)", &s[..max_len])
      }
  }
  ```
  If `max_len` falls inside a multi-byte UTF-8 character boundary (such as an emoji, Chinese character, or non-ASCII symbol commonly output by automated developer tools), the slicing operation `&s[..max_len]` will crash with a panic.
  This function is executed under write locks when finishing executions:
  * In `ExecutionRecord::complete` (line 217), on arbitrary system process output.
  * In `ExecutionRecordBuilder::build` (line 331), on the serialized representation of `self.output`.
  An attacker or a crafted tool output containing a multi-byte character straddling the 1000-byte boundary will panic the thread executing the tracker, allowing complete crash of core control loops or DBus interfaces integrating with this crate.
* **Remediation**:
  Use Char-boundary safe truncating or iterate safely over character boundaries:
  ```rust
  fn truncate_string(s: &str, max_len: usize) -> String {
      if s.len() <= max_len {
          s.to_string()
      } else {
          // Find the nearest char boundary <= max_len
          let mut boundary = max_len;
          while !s.is_char_boundary(boundary) {
              boundary -= 1;
          }
          format!("{}... (truncated)", &s[..boundary])
      }
  }
  ```

#### 2. MEDIUM: Lock Contention and O(N) Shifting in Core Active Ring Buffer
* **Citation**: `crates/op-execution-tracker/src/execution_tracker.rs:99`
* **Severity**: Medium
* **Impact**: Significant performance degradation and blocking latency on concurrent execution tracking.
* **Description**:
  The `records` list is an `Arc<RwLock<Vec<ExecutionRecord>>>`.
  1. In `start_execution` (line 98), when the collection grows past `max_history`, it calls `records.remove(0)`. In a standard `Vec`, this requires shifting up to `max_history` (default 1000) elements in memory. This copy operation runs under a write lock on `records`.
  2. On completion (`complete_execution`, line 113) or failure (`fail_execution`, line 145), the code acquires a write lock and executes a linear `iter_mut().find(...)` to locate the record by ID. This O(N) search under a write lock completely serializes execution completions. If many tools complete simultaneously, lock acquisition times will block all threads attempting to log metrics or starting tools.
* **Remediation**:
  Replace `Vec<ExecutionRecord>` with a combination of a `VecDeque` for ring-buffer index retention and a `HashMap<String, ExecutionRecord>` (or a concurrent map like `DashMap` if appropriate) to ensure updates are O(1) without requiring linear search and memory shifts under an exclusive write lock.

#### 3. LOW: Non-Monotonic Clock Used for Time Tracking and Verification Hash Ordering
* **Citation**: `crates/op-execution-tracker/src/record.rs:60`
* **Severity**: Low
* **Impact**: Out-of-order execution recording and clock drift causing verification metrics mismatch.
* **Description**:
  The `ExecutionTiming::capture_start` is documented as providing "Monotonic nanoseconds (for ordering)", yet it obtains the timestamp via `SystemTime::now()`:
  ```rust
  let monotonic = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap_or_default()
      .as_nanos();
  ```
  `SystemTime` is not monotonic and can jump backwards or forwards due to manual configuration changes or NTP synchronization drift. This violates the monotonic ordering guarantees asserted in the `ExecutionTiming` struct definition.
* **Remediation**:
  To measure actual durations or order metrics monotonically, store absolute sequence numbers or fetch true monotonic nanoseconds via specialized monotonic platforms APIs or keep sorting relative purely to the returned `Instant` where possible. Alternatively, rename the field to reflect its dependency on Wall-clock times.