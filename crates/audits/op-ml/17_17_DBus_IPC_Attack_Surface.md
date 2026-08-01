### D-Bus & IPC Attack Surface

No D-Bus interfaces, methods, or signals are registered in the audited files of the `op-ml` crate. The workspace configuration indicates that D-Bus functionality is managed by sister crates (such as `op-introspection` or `op-identity`), which are not part of the provided source files. 

Within the scope of the audited `op-ml` files, the IPC and system boundaries are limited to:
1. **Outbound Network Connection**: Outbound HTTPS calls to the Hugging Face Hub via the `hf-hub` API client in `crates/op-ml/src/downloader.rs` to download pre-trained model parameters and ONNX runtimes.
2. **File System Boundary**: Creation of and write/read access to model storage directories under `/var/lib/op-dbus/models` (or custom locations specified by `OP_DBUS_MODEL_DIR`).
3. **Environment Boundary**: Parsing of system configuration via local environment variables (`OP_DBUS_VECTOR_LEVEL`, `OP_DBUS_MODEL_DIR`, `OP_DBUS_EXECUTION_PROVIDER`, `OP_DBUS_GPU_DEVICE`).

---

### Security and Quality Findings

#### [Finding 1] Ad-Hoc Serde Serialization Violating Schema-as-Code Discipline
- **Severity**: Low (Quality/Compliance)
- **File:Line**: `crates/op-ml/src/config.rs:159-178`
- **Description**: 
  The configuration and specifications for semantic vectorization models—specifically `VectorizationConfig`, `VectorizationLevel`, and `ExecutionProvider`—are implemented as ad-hoc Rust structures with standard Serde serialization traits rather than being derived from versioned, standardized schemas (such as Protocol Buffers or OSCAL).
- **Impact**: 
  Changes to the vectorization level variants or default model configurations risk breaking compatibility with external state-stores or cache layers without a formal schema evolution path.
- **Remediation**: 
  Define the configuration structures as versioned Protocol Buffer messages in the system's contract repository and generate the corresponding Rust structures to ensure strict schema-as-code discipline.

---

#### [Finding 2] Silent Runtime Dimensionality Mismatch in Model Fallback Logic
- **Severity**: High (Reliability / Denial of Service)
- **File:Line**: `crates/op-ml/src/model_manager.rs:214-237`
- **Description**: 
  If the primary requested model fails to load, `try_fallback_model` attempts to fall back to a lower semantic depth level (e.g., falling back from `High` to `Medium`, or `Medium` to `Low`).
  ```rust
  fn try_fallback_model(&self) -> Result<TextEmbedder> {
      let fallback_level = match self.config.level {
          VectorizationLevel::High => VectorizationLevel::Medium,
          VectorizationLevel::Medium => VectorizationLevel::Low,
          _ => { ... }
      };
  ```
- **Impact**: 
  Falling back from `High` (768 dimensions) to `Medium` (384 dimensions) dynamically alters the dimensionality of the generated vectors. Vector databases (such as Qdrant or Cozo, which are registered in `Cargo.toml` as dependencies of the system) require static dimension definitions for similarity indices. Returning vectors of differing dimensions at runtime will cause downstream databases to reject insertions and queries, resulting in unhandled panics or total database query failures.
- **Remediation**: 
  Avoid silent dynamic dimensional fallbacks. If a configured model fails to load, the embedding process should fail fast and return an explicit initialization error to the caller, preventing index corruption.

---

#### [Finding 3] Insecure Shared Directory Creation and Local Model Hijacking
- **Severity**: High (Local Privilege Escalation / Arbitrary Code Execution)
- **File:Line**: `crates/op-ml/src/downloader.rs:27` and `crates/op-ml/src/downloader.rs:125`
- **Description**: 
  The model downloader initializes cache directories and copies files under the default system directory `/var/lib/op-dbus/models` (via `std::fs::create_dir_all` and `std::fs::copy`) without explicitly specifying restrictive permissions.
- **Impact**: 
  If the control plane daemon runs as a privileged user (e.g., `root`), creating `/var/lib/op-dbus/models` without strict POSIX file permissions (e.g., `0750` or `0700`) allows other local users on the system to read, write, or replace the downloaded ONNX model files (`model.onnx`) or tokenizer settings. A low-privilege attacker can replace the ONNX models with a malicious counterpart, triggering arbitrary code execution or memory exploitation inside the ONNX Runtime context when the daemon performs lazy model-loading and inference.
- **Remediation**: 
  Enforce strict directory permissions upon creation. Use `std::os::unix::fs::DirBuilderExt` to create the cache directories with `0700` or `0750` permissions, and verify that the owner of the model files matches the daemon's user.

---

#### [Finding 4] Threadpool Block-On in Async Execution Context
- **Severity**: Medium (Performance / Resource Exhaustion)
- **File:Line**: `crates/op-ml/src/model_manager.rs:152-154`
- **Description**: 
  The lazy initialization of model files blocks the current runtime executor thread:
  ```rust
  let model_dir = tokio::runtime::Handle::current()
      .block_on(async { self.ensure_model_downloaded().await })?;
  ```
- **Impact**: 
  Running a blocking network download (`ensure_model_downloaded`) inside `tokio::runtime::Handle::current().block_on` in a synchronous calling context suspends the runtime's OS thread. Under concurrent workloads or slow network conditions, this causes immediate tokio threadpool exhaustion, resulting in severe latency spikes and potential deadlocks across the entire system control plane.
- **Remediation**: 
  Refactor the lazy load trigger to be native async (i.e., make `embed` and `embed_batch` async functions), allowing the downloading and ONNX model compilation tasks to yield properly to the async executor. Use `tokio::task::spawn_blocking` for ONNX initialization.