# Production Quality & Security Audit: `op-ml`

## 1. Dependencies & Feature Inventory

### Direct Dependencies of `op-ml` (from `crates/op-ml/Cargo.toml`)

| Dependency | Specified Version | Features Explicitly Enabled | Features Pulled by Default / Inherited | Risk / Note |
| :--- | :--- | :--- | :--- | :--- |
| `tokio` | `workspace = true` | None (in crate) | Inherits `["full"]` from workspace | Robust, but used unsafely in sync-to-async bridges |
| `serde` | `workspace = true` | None (in crate) | Inherits `["derive"]` from workspace | Standard serialization |
| `simd-json` | `workspace = true` | None (in crate) | Inherits `["serde", "serde_impl"]` from workspace | Highly optimized JSON parsing |
| `anyhow` | `workspace = true` | None | Statically defined as `"1"` | Generic error handling |
| `thiserror` | `workspace = true` | None | Statically defined as `"1"` | Domain error definition |
| `tracing` | `workspace = true` | None | Statically defined as `"0.1"` | Structured instrumentation |
| `reqwest` | `workspace = true` | None (in crate) | Inherits `["json", "stream"]` from workspace | HTTP Client |
| `log` | `workspace = true` | None | Statically defined as `"0.4"` | Legacy logging support |
| `num_cpus` | `workspace = true` | None | Statically defined as `"1.16"` | Thread pool sizing |
| `sha2` | `workspace = true` | None | Statically defined as `"0.10"` | Cryptographic hashing |
| `hf-hub` | `"0.5.0"` | `["tokio"]` | None | Model downloader from Hugging Face Hub |

### Workspace Feature Gating
* **`default = []`**: Builds a stubbed-out ML implementation where embedding generation immediately fails unless the `ml` feature is enabled.
* **`ml = []`**: Gates actual model downloading and local execution. It enables `hf_hub`, `tokenizers`, `ort`, and `ndarray` imports. 
  * *Code gate locations:* Used extensively with `# [cfg(feature = "ml")]` block headers in:
    * `crates/op-ml/src/downloader.rs`
    * `crates/op-ml/src/embedder.rs`
    * `crates/op-ml/src/model_manager.rs`

### Schema-as-Code Evaluation
* **Gap Identified**: The models configurations and data contracts inside `crates/op-ml/src/config.rs` (e.g., `VectorizationConfig`, `VectorizationLevel`, `ExecutionProvider`) are defined as ad-hoc Rust structures with standard `serde` attributes. 
* There are **no** dependencies on `prost`, `tonic-build`, or `jsonschema` inside the `op-ml` crate itself, even though the workspace dependencies (such as `op-compliance` and `op-state-store`) make heavy use of versioned Protocol Buffers and schema validation. Model execution parameters and configurations passed over DBus or RPC should be defined as versioned schemas rather than ad-hoc formats to maintain systemic compliance and avoid parser mismatches.

---

## 2. Storage Backend Inventory

The following database engines and cache storage backends are declared in the root workspace configuration:

| Backend | Found at file:line | Role (KV / Graph / Cache / Queue) | Notes / Violations |
| :--- | :--- | :--- | :--- |
| `cozo` | `Cargo.toml:59` | Relational-Graph-Vector | Relational-graph storage using Datalog engine. Uses `storage-sled` backend to avoid SQLite engine-link conflicts with `rusqlite`. |
| `sqlx` | `Cargo.toml:101` | Relational Storage | Configured with `sqlite` runtime for local control-plane relational schemas. |
| `rusqlite` | `Cargo.toml:102` | Relational Storage | Bundled engine for local lightweight database interactions. |
| `redis` | `Cargo.toml:103` | KV / Cache | Shared key-value caching layer. |
| `op-cozo-store` | `Cargo.toml:38` | Workspace Crate dependency | Relational database adapter layer. |
| `op-cache` | `Cargo.toml:21` | Workspace Crate dependency | Cache orchestration layer. |

### Storage Engine Architecture Violations
* **No Database Storage in `op-ml`**: The `op-ml` crate stores ML models directly in the file system (defaulting to `/var/lib/op-dbus/models` as defined in `crates/op-ml/src/config.rs:158`). However, vector embeddings produced by `op-ml` are consumed downstream by other workspace crates like `op-cognitive-mcp` (which integrates with `cozo` and `qdrant-client`). 
* No architectural storage violations are directly present in `op-ml` files, as it delegating persistent storing duties entirely to calling crates.

---

## 3. Security & Quality Issues Table

| Issue ID | Severity | File : Line | Description | Exploitable? |
| :--- | :--- | :--- | :--- | :--- |
| **OP-ML-01** | **Critical** | `crates/op-ml/src/model_manager.rs:149` | Sync-to-Async Deadlock and Panic via `Handle::block_on` inside Tokio worker context. | **Yes** |
| **OP-ML-02** | **High** | `crates/op-ml/src/downloader.rs:136` | Thread starvation due to synchronous blocking IO (`std::fs::copy`) inside async context. | **Yes** |
| **OP-ML-03** | **Medium** | `crates/op-ml/src/downloader.rs:63` | Incomplete/Corrupted model bypass due to lack of hash/integrity checks in `is_model_complete`. | **Yes** |
| **OP-ML-04** | **Medium** | `crates/op-ml/src/embedder.rs:141` | Denial of Service (uncontrolled panic) when slicing unexpected rank/shape tensors. | **Yes** |
| **OP-ML-05** | **Low** | `crates/op-ml/src/model_manager.rs:172` | Failed Fallback execution path due to lack of asset availability verification. | No |

---

## 4. Detailed Technical Findings

### OP-ML-01: Sync-to-Async Deadlock/Panic on On-Demand Model Loading (CRITICAL)

#### Description
In `crates/op-ml/src/model_manager.rs`, the global model manager implements lazy initialization of the ONNX models. When `embed()` or `embed_batch()` is called, they invoke `get_or_load_embedder()`.

At `crates/op-ml/src/model_manager.rs:149`, the code attempts to transition from a synchronous context to an asynchronous one by getting the current Tokio handle and calling `block_on` to run the model download logic:

```rust
// Use async runtime to download if needed
let model_dir = tokio::runtime::Handle::current()
    .block_on(async { self.ensure_model_downloaded().await })?;
```

#### Exploitability & Impact
This is **directly exploitable** to cause an immediate Denial of Service (Application Crash). 

In an asynchronous application such as the `op-dbus` control plane (which runs on `axum` and `zbus`), any handler calling `ModelManager::global().embed(text)` is already running on a Tokio worker thread. Calling `block_on` from within an active Tokio worker context is strictly forbidden by Tokio and results in an immediate, uncatchable runtime panic:

```
Cannot start a runtime from within a runtime. This happens because a function (like `block_on`) attempted to block the current thread while the color context of the thread is already in a runtime.
```

Any client sending a DBus message or HTTP request that triggers an on-demand model download/load will instantly crash the entire system daemon.

#### Remediation
Refactor `ModelManager` to expose a native `async fn embed()` and `async fn get_or_load_embedder()` interface, propagating async/await up to callers rather than blocking worker threads:

```rust
// In crates/op-ml/src/model_manager.rs
pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
    if !self.is_enabled() {
        return Ok(Vec::new());
    }
    let embedder = self.get_or_load_embedder().await?;
    embedder.embed(text)
}
```

---

### OP-ML-02: Async Thread Starvation via Synchronous File Operations (HIGH)

#### Description
In `crates/op-ml/src/downloader.rs:136`, when copying downloaded models from the Hugging Face cache to the target models directory, the code calls `std::fs::copy`:

```rust
// Copy to target directory
let target_path = target_dir.join(file_name);
std::fs::copy(&file_path, &target_path)
    .context(format!("Failed to copy {} to {:?}", file_name, target_path))?;
```

Similarly, on line 31 and 85, `std::fs::create_dir_all` is called synchronously.

#### Exploitability & Impact
ML models such as `MPNet-base-v2` (`~420MB` as defined in `crates/op-ml/src/config.rs:58`) are large files. Performing synchronous I/O operations of this size on a Tokio thread blocks the executor. It prevents other cooperative tasks from executing, causing severe latency spikes, missed DBus heartbeat signals, and HTTP client connection timeouts on the gateway.

#### Remediation
Use Tokio's non-blocking filesystem utilities (`tokio::fs`) or offload synchronous filesystem copying to a dedicated blocking thread pool using `tokio::task::spawn_blocking`:

```rust
// In crates/op-ml/src/downloader.rs
let target_path = target_dir.join(file_name.to_string());
tokio::fs::copy(&file_path, &target_path)
    .await
    .context(format!("Failed to copy {} to {:?}", file_name, target_path))?;
```

---

### OP-ML-03: Incomplete/Corrupted Model Bypass via Weak Completion Checks (MEDIUM)

#### Description
In `crates/op-ml/src/downloader.rs:63`, the system determines whether a model is cached and ready to load using `is_model_complete`:

```rust
fn is_model_complete(&self, model_dir: &Path) -> bool {
    // Check for required files
    let model_file = model_dir.join("model.onnx");
    let tokenizer_file = model_dir.join("tokenizer.json");

    model_file.exists() && tokenizer_file.exists()
}
```

#### Exploitability & Impact
If the download or file copying process is interrupted (e.g., via a daemon restart, container termination, or out-of-disk space event), the target files `model.onnx` or `tokenizer.json` may exist as truncated or empty (`0-byte`) files. 

Because `is_model_complete` only asserts file *existence*, the manager will consider the model complete, skip the download step, and attempt to load it. This results in a permanent loading panic or deserialization failure inside the ONNX/tokenizers parser every time the service boots up. This is a persistent Denial of Service that requires manual sysadmin intervention to clear directories under `/var/lib/op-dbus/models/`.

#### Remediation
Perform verification of size/hashes, or write model files to a temporary `.tmp` directory or file name first, renaming them atomically to their final target path only once the transfer is fully completed:

```rust
// In crates/op-ml/src/downloader.rs
async fn download_file(...) -> Result<()> {
    let tmp_path = target_dir.join(format!("{}.tmp", file_name));
    let target_path = target_dir.join(file_name);
    
    tokio::fs::copy(&file_path, &tmp_path).await?;
    tokio::fs::rename(&tmp_path, &target_path).await?;
    Ok(())
}
```

---

### OP-ML-04: Denial of Service via Uncontrolled Tensor Shape Slicing Panics (MEDIUM)

#### Description
In `crates/op-ml/src/embedder.rs:141`, the embedder extracts the tensor from ONNX Runtime outputs and directly slices it:

```rust
// Extract embeddings (typically from "last_hidden_state" or "sentence_embedding")
let embeddings = outputs["sentence_embedding"]
    .try_extract_tensor::<f32>()?
    .view()
    .to_owned();

// Convert to Vec<Vec<f32>>
let dim = self.level.dimensions();
let mut result = Vec::new();

for i in 0..batch_size {
    let row = embeddings.slice(ndarray::s![i, ..]).to_vec();
    ...
```

#### Exploitability & Impact
If an operator loads an alternative model, or if the repository's tensor output structure changes (e.g., shape rank is different from expected, or output dimensions do not align with `[batch_size, dim]`), the `.slice()` call using `ndarray::s![i, ..]` will panic due to an out-of-bounds array access. Since Rust panics do not cross FFI boundaries elegantly and often terminate processes when not caught, this crash will take down the entire executable.

#### Remediation
Validate the shape of the retrieved tensor against expected rank and batch size before performing slicing operations:

```rust
let shape = embeddings.shape();
if shape.len() != 2 || shape[0] != batch_size {
    return Err(anyhow::anyhow!(
        "Invalid output tensor shape. Expected [{}, {}], got {:?}", 
        batch_size, dim, shape
    ));
}
```

---

### OP-ML-05: Defunctional Fallback Strategy for Missing Assets (LOW)

#### Description
In `crates/op-ml/src/model_manager.rs:172`, `try_fallback_model` attempts to downgrade the vectorization level (e.g., from `High` to `Medium`) if the initial loading of the model fails:

```rust
fn try_fallback_model(&self) -> Result<TextEmbedder> {
    let fallback_level = match self.config.level { ... };
    let model_dir = self.get_model_path_for_level(fallback_level)?;
    
    // Create fallback config with same execution provider
    let mut fallback_config = self.config.clone();
    fallback_config.level = fallback_level;

    TextEmbedder::load(&model_dir, &fallback_config)
        .context(format!("Fallback to {} failed", fallback_level))
}
```

#### Impact
This fallback assumes that the lower-level model files are already downloaded and complete on disk. If they have not been downloaded previously, `TextEmbedder::load` will fail immediately with a `FileNotFound` error. The fallback strategy is functionally inert for on-demand use cases and fails to resolve loading faults.

#### Remediation
Perform an explicit `ensure_model_downloaded` check for the chosen fallback model level before attempting to call `TextEmbedder::load`.