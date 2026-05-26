# Architecture & Module Map

### Overview
The `op-dynamic-loader` crate provides intelligent, dynamic tool loading capabilities for the OP-DBUS control plane. It acts as an enhancement layer over the base `op-tools` registry by implementing LRU caching, usage pattern analysis via `op-execution-tracker`, and execution-aware load strategies. This ensures frequently executed or critical tools are cached in memory, while cold tools are dynamically loaded on-demand.

### Module Tree
```
op-dynamic-loader (lib.rs)
 ├── dynamic_registry (DynamicToolRegistry, EnhancedToolRegistry trait)
 ├── error (DynamicLoaderError enum)
 ├── execution_aware_loader (ExecutionAwareLoader, ExecutionAwareToolRegistry trait)
 └── loading_strategy (LoadingStrategy trait, SmartLoadingStrategy)
```

### Entry Points
- **Library Entry Point**: `crates/op-dynamic-loader/src/lib.rs`
  - Re-exports core structures: `DynamicToolRegistry`, `DynamicLoaderError`, `ExecutionAwareLoader`, `LoadingStrategy`, and `SmartLoadingStrategy`.

### Notes
- The architecture relies heavily on asynchronous locking (`tokio::sync::RwLock`) to coordinate concurrent access to the underlying LRU cache (`lru::LruCache`).
- Integrates directly with the `op-execution-tracker` crate to inspect completed historical runs and make predictive loading choices.

---

# Security & Quality Audit Findings

## Critical Severity

### DoS via Unvalidated Non-Zero Cache Size Panic
- **File & Line**: `crates/op-dynamic-loader/src/dynamic_registry.rs:45`
- **Exploitability**: Directly exploitable. An administrator or system configuration passing a value of `0` for `max_cache_size` will trigger an immediate, unhandled runtime panic during the initialization of the registry, crashing the application thread or system daemon.
- **Mechanism**:
  ```rust
  tool_cache: Arc<RwLock<LruCache::new(
      NonZeroUsize::new(max_cache_size).unwrap(),
  ))),
  ```
  `NonZeroUsize::new(0)` returns `None`. Calling `.unwrap()` on a `None` variant in Rust causes a thread panic. Since `ExecutionAwareLoader::new` propagates this initialization directly, any deployment with an unvalidated config file setting `max_cache_size = 0` will crash on startup.
- **Remediation**:
  Replace `.unwrap()` with a safe default or return an explicit validation error:
  ```rust
  let cache_size = NonZeroUsize::new(max_cache_size)
      .ok_or_else(|| DynamicLoaderError::CacheError("max_cache_size must be greater than zero".to_string()))?;
  ```
  Alternatively, default a size of `0` to a minimum size of `1` or return a `Result<Self, DynamicLoaderError>` from the constructor.

---

## Medium Severity

### Lock Guard Held Across `.await` Points (Deadlock / Resource Exhaustion)
- **File & Line**: `crates/op-dynamic-loader/src/dynamic_registry.rs:60-64`
- **Exploitability**: High concurrency required, causing performance degradation or lock starvation.
- **Mechanism**:
  ```rust
  // Check cache first
  {
      // LruCache::get requires &mut self to update LRU order
      let mut cache = self.tool_cache.write().await;
      if let Some(tool) = cache.get(name) {
          *self.cache_hits.write().await += 1;
          return Ok(Arc::clone(tool));
      }
  }
  ```
  The code acquires a write lock on the `tool_cache` RwLock (`let mut cache = self.tool_cache.write().await;`). While holding this write lock, it executes another asynchronous write lock operation (`*self.cache_hits.write().await += 1;`). 
  
  Holding a write guard across an `.await` boundary in Tokio is a severe anti-pattern:
  1. It blocks any other concurrent task trying to read or write from `self.tool_cache`.
  2. If the executor thread yields during the statistics lock acquisition, other threads could experience starvation.
- **Remediation**:
  Release the write lock guard *before* performing any asynchronous actions, or use an atomic type (`std::sync::atomic::AtomicU64`) for statistics, which avoids asynchronous locks entirely and has negligible CPU overhead:
  ```rust
  // Replace RwLock<u64> with AtomicU64
  self.cache_hits.fetch_add(1, Ordering::SeqCst);
  ```

---

## Low Severity

### Resource Overload / Query Storm on Cache Misses
- **File & Line**: `crates/op-dynamic-loader/src/loading_strategy.rs:46`
- **Exploitability**: Medium under high-load uncached tool lookups.
- **Mechanism**:
  On every cache miss for an unregistered tool, `should_load` is triggered:
  ```rust
  // Check recent execution history
  let recent_executions = self.execution_tracker.list_recent_completed(10).await;
  ```
  This queries the execution tracker for history. If the execution tracker is backed by a database, network call, or disk file, querying it repeatedly on cache misses will degrade performance, creating an "N+1 query" equivalent performance storm.
- **Remediation**:
  Cache the query history of the execution tracker, throttle queries using a cooldown mechanism, or pass aggregated metrics from the tracker to the loader rather than executing fresh database queries per dynamic tool resolution.

---

# Schema-As-Code Violations

### Ad-hoc Tool Configuration & Magic String Comparisons
- **File & Lines**: `crates/op-dynamic-loader/src/loading_strategy.rs:89-95`
- **Compliance Defect**: The control plane uses ad-hoc hardcoded string slices to determine if a tool is "critical":
  ```rust
  let critical_tools = [
      "respond_to_user",
      "cannot_perform",
      "systemd_status",
      "file_read",
      "agent_status",
  ];
  ```
- **Consequence**: This bypasses versioned schema enforcement. Tool criticality, component structures, and capabilities should be validated against versioned schemas (such as OSCAL Component Definitions or Protobuf validation constraints) rather than maintaining hardcoded magic strings in logic files.

### Ad-hoc Context and Model Instantiation
- **File & Lines**: `crates/op-dynamic-loader/src/execution_aware_loader.rs:51-54`
- **Compliance Defect**:
  ```rust
  pub async fn get_tool(&self, tool_name: &str) -> Result<op_tools::BoxedTool> {
      let context = ExecutionContext::new(tool_name);
      self.get_tool_with_context(tool_name, &context).await
  }
  ```
  The context contracts and tool references are passed as unstructured types (`&str` for names, ad-hoc `ExecutionContext` structs). Under a strict schema-as-code discipline, tool execution contexts, registry definitions, and component properties must be defined as versioned schemas (e.g., Protocol Buffers) to ensure interoperability and cryptographic integrity of execution tracking records.