# PRODUCTION SECURITY & QUALITY AUDIT: OBSERVABILITY & SCHEMA-AS-CODE

## 1. Logging Macro Inventory

An audit of the `op-ml` crate's source files was conducted to inventory the logging macros and look for diagnostic patterns.

### Inventory Summary
* **`tracing::` Macros (`info!`, `warn!`, `error!`, `debug!`)**: **0**
* **`println!` / `eprintln!`**: **0**
* **`log::` Macros (`info!`, `warn!`)**: **28**

### Detailed Logging Breakdown
The codebase lists the `tracing` crate as a dependency in `crates/op-ml/Cargo.toml` but relies exclusively on the legacy `log` crate macros. This creates a configuration mismatch where context-propagation, structural spans, and asynchronous tracing features of the workspace's subscriber framework are bypassed.

* **`log::info!` (21 occurrences)**:
  * `crates/op-ml/src/config.rs:183` — Vectorization level set.
  * `crates/op-ml/src/config.rs:199` — Execution provider set.
  * `crates/op-ml/src/config.rs:210` — GPU device ID set.
  * `crates/op-ml/src/downloader.rs:47` — Model availability check.
  * `crates/op-ml/src/downloader.rs:54` — Model cache confirmation.
  * `crates/op-ml/src/downloader.rs:59` — Model download initialization.
  * `crates/op-ml/src/downloader.rs:92` — Count of files being downloaded.
  * `crates/op-ml/src/downloader.rs:96` — File download success.
  * `crates/op-ml/src/downloader.rs:109` — Download completion.
  * `crates/op-ml/src/embedder.rs:26` — Model loading configuration and paths.
  * `crates/op-ml/src/embedder.rs:44` — Thread pool settings for CPU execution.
  * `crates/op-ml/src/embedder.rs:53` — CUDA device assignment.
  * `crates/op-ml/src/embedder.rs:63` — TensorRT device assignment.
  * `crates/op-ml/src/embedder.rs:75` — DirectML device assignment.
  * `crates/op-ml/src/embedder.rs:90` — CoreML execution selection.
  * `crates/op-ml/src/embedder.rs:104` — Successful model initialization.
  * `crates/op-ml/src/model_manager.rs:44` — Global manager instantiation.
  * `crates/op-ml/src/model_manager.rs:131` — On-demand lazy-loading notification.
  * `crates/op-ml/src/model_manager.rs:139` — Successful lazy load.
  * `crates/op-ml/src/model_manager.rs:162` — Local availability verification.
  * `crates/op-ml/src/model_manager.rs:167` — HF Hub fallback download alert.

* **`log::warn!` (7 occurrences)**:
  * `crates/op-ml/src/config.rs:185` — Invalid level configuration fallback.
  * `crates/op-ml/src/config.rs:201` — Invalid execution provider fallback.
  * `crates/op-ml/src/downloader.rs:104` — Missing optional model files.
  * `crates/op-ml/src/embedder.rs:78` — Windows-only execution provider fallback on non-Windows targets.
  * `crates/op-ml/src/embedder.rs:93` — macOS-only execution provider fallback on non-macOS targets.
  * `crates/op-ml/src/model_manager.rs:143` — Primary model load failure warning.
  * `crates/op-ml/src/model_manager.rs:193` — Cascading level fallback alert.

---

## 2. Technical Findings

### CRITICAL: Synchronous Tokio `block_on` Call Inside Active Runtime Causes Immediate Panic (DoS)
* **File & Line**: `crates/op-ml/src/model_manager.rs:134`
* **Vulnerability Class**: Denial of Service (DoS) / Unrecoverable Thread Panic
* **Exploitability**: Directly exploitable via any external call requesting text embeddings if the model is not pre-cached and pre-loaded.
* **Impact**: Critical. This crash will tear down the worker thread, poison the parent global `OnceCell` container, and permanently disable the embedding interface of the control plane.

#### Description
In `crates/op-ml/src/model_manager.rs`, the on-demand lazy initialization of the ML embedder runs within a synchronous context using a thread-safe `OnceCell`. When `get_or_load_embedder` is called at runtime, it checks if the model exists locally and downloads it if missing:

```rust
// Use async runtime to download if needed
let model_dir = tokio::runtime::Handle::current()
    .block_on(async { self.ensure_model_downloaded().await })?;
```

Because the control plane runs inside a parent Tokio runtime (e.g., handling concurrent gRPC or Axum connections), invoking `block_on` on the *current* handle will trigger an immediate, unrecoverable panic:
`"Cannot start a runtime from within a runtime"`

An attacker can easily exploit this by triggering any flow that calls `embed` or `embed_batch` when the model has not been completely loaded, crashing the daemon.

#### Remediation
1. **Async Context Conversion**: Change the lazy initialization method and its call chain to use standard asynchronous mechanics (`async fn` and `.await`) rather than attempting a block-blocking lock on an active thread.
2. **Pre-loading**: Execute the `ensure_model_downloaded` and initialization step sequentially during control plane bootstrapping inside `main.rs` before starting the network listeners.

---

### HIGH: Secondary Fallback Model Loading Error Swallowed without Logging
* **File & Line**: `crates/op-ml/src/model_manager.rs:146`
* **Vulnerability Class**: Diagnostics & Observability Defect
* **Exploitability**: Non-exploitable directly; severely degrades post-mortem crash analysis and live system diagnostics.
* **Impact**: High. Cascading failures in the model load path are masked, leaving operators with partial or misleading failure traces.

#### Description
If the primary ONNX model loading fails (e.g., due to file corruption, driver mismatches, or invalid execution provider configurations), the error is logged as a warning at line 144. The system then attempts to recover by loading a fallback model at a lower semantic level:

```rust
Err(e) => {
    log::warn!("Failed to load {} model: {}", self.config.level, e);
    // Try to fall back to lower level
    self.try_fallback_model().or(Err(e))
}
```

If `try_fallback_model` also fails (e.g., because the fallback directory does not exist or permissions are blocked), the resulting error is silently discarded by `.or(Err(e))`, which returns the *original* error `e` back to the caller. The operator receives an error trace for the *primary* model failure, while the root cause of the *fallback* system failure is completely swallowed without being written to logs or telemetry.

#### Remediation
Capture the error from `try_fallback_model` explicitly. Log it or chain it with the parent error `e` using `anyhow` context propagation before returning the final failure state:

```rust
Err(e) => {
    log::warn!("Failed to load primary {} model: {}", self.config.level, e);
    match self.try_fallback_model() {
        Ok(fallback) => Ok(fallback),
        Err(fallback_err) => {
            log::error!("Secondary fallback model initialization failed: {}", fallback_err);
            Err(e.context(fallback_err))
        }
    }
}
```

---

### MEDIUM: Structural Mismatch on Batch Extraction Silently Swallowed
* **File & Line**: `crates/op-ml/src/embedder.rs:116`
* **Vulnerability Class**: Logic Flaw / Observability Defect
* **Exploitability**: Non-exploitable.
* **Impact**: Medium. This error propagation failure leads to downstream functional anomalies (e.g., dimensional mismatch panics or invalid similarity calculations) with zero diagnostic footprint.

#### Description
The single-text embedding interface `embed` delegates execution to the batched processing interface:

```rust
pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
    self.embed_batch(&[text])
        .map(|mut batch| batch.pop().unwrap_or_default())
}
```

If the ONNX inference session runs successfully but returns an empty list of embeddings (a severe structural or state corruption anomaly, since one input was provided), the pop operation yields `None`. This is silently swallowed by `.unwrap_or_default()`, yielding an empty `Vec<f32>` (zero-dimensional) to the caller. The runtime anomaly is not logged, and no error is returned.

#### Remediation
Enforce structural validation of the batch return count. Return an explicit error if the output vector shape does not match the input shape:

```rust
pub fn embed(&self, text: &str) -> Result<Vec<f32>> {
    let mut batch = self.embed_batch(&[text])?;
    batch.pop().ok_or_else(|| anyhow::anyhow!("ONNX inference returned an empty embedding batch for input"))
}
```

---

## 3. Metrics Instrumentation Note

Telemetry analysis shows that the `op-ml` crate contains **zero** metric collection points. No references to the workspace's imported `prometheus` or `opentelemetry` crates exist in this submodule.

### Missing Critical Telemetry Points
1. **Model Download Latency & State**: Model downloads from the Hugging Face Hub are high-latency I/O operations. The system lacks counters for cache hits/misses and histogram gauges for download duration.
2. **Inference Execution Histograms**: ONNX runtime execution durations in `embed_batch` are highly performance-critical. There are no timers tracking CPU/GPU core computation times.
3. **Execution Provider Fallback Counters**: No metrics exist to track direct fallbacks from CUDA/TensorRT to CPU when initialization fails.
4. **Queue & Batch Gauges**: The system does not record the batch size distribution of incoming vectorization tasks, preventing auto-scaling or batch efficiency optimization.

---

## 4. Security & PII Evaluation

### Payload Sanitization
The system successfully isolates and protects PII by omitting raw payloads from the diagnostic stream. While configuration variables, directories, execution models, and system errors are logged, **the raw text inputs passed to `embed` and `embed_batch` are never printed or logged**. This prevents sensitive corporate data or personally identifiable information (PII) from leaking into log aggregators.

---

## 5. Schema-As-Code Violations

The codebase violates the project's schema-as-code discipline by expressing critical configuration and execution interfaces as ad-hoc Rust structs, raw strings, and raw floating-point arrays instead of versioned Protocol Buffers or OSCAL-compliant schemas.

### Detected Violations
1. **Ad-hoc Configuration Struct**:
   * **File & Line**: `crates/op-ml/src/config.rs:136`
   * **Violation**: `VectorizationConfig` is expressed as an ad-hoc Rust struct parsed from loose environment variables. It lacks a versioned serialization schema, making dynamic configuration validation fragile and breaking programmatic cross-service compatibility.
2. **Untyped Payload String Contracts**:
   * **File & Line**: `crates/op-ml/src/embedder.rs:114`
   * **Violation**: The ingestion contract is modeled as a raw string reference (`text: &str`). It fails to capture metadata, provenance, source ID, or structured properties.
3. **Unversioned Vector Arrays**:
   * **File & Line**: `crates/op-ml/src/embedder.rs:114` and `crates/op-ml/src/model_manager.rs:71`
   * **Violation**: Embeddings are returned as plain floats inside a `Vec<f32>`. Telemetry, model dimensionality constraints, and model version identifiers are completely absent from the returned payload. Downstream vector databases and consumers will silently store or process corrupted vectors if the underlying transformer model changes levels or is swapped out.

### Corrective Action
Define a standardized, versioned Protocol Buffer schema in the model definitions layer:

```protobuf
syntax = "proto3";
package op.ml.v1;

message EmbeddingsRequest {
  string model_version = 1;
  repeated string texts = 2;
}

message Vector {
  repeated float values = 1;
}

message EmbeddingsResponse {
  string model_version = 1;
  uint32 dimensions = 2;
  repeated Vector embeddings = 3;
}
```