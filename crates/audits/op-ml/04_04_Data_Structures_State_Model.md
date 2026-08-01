### Data Structures and Concurrency Primitives Audit

| File | `Arc` | `Rc` | `RefCell` | `RwLock` | `Mutex` | `OnceCell` | `.clone()` Count |
| :--- | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| `crates/op-ml/src/config.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-ml/src/downloader.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 1 |
| `crates/op-ml/src/embedder.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-ml/src/lib.rs` | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| `crates/op-ml/src/model_manager.rs` | 6 | 0 | 0 | 0 | 0 | 5 | 2 |

#### `.clone()` Calls Flagged (> 20)
*None.* All files contain $\le 2$ `.clone()` calls.

---

### Key Structural Findings

#### 1. Large Struct Flagged (> 5 Public Fields)
* **File & Line:** `crates/op-ml/src/config.rs:142`
* **Struct:** `pub struct VectorizationConfig`
* **Field Count:** 7 public fields (`level`, `model_dir`, `batch_size`, `load_timeout_secs`, `num_threads`, `execution_provider`, `gpu_device_id`).
* **Impact:** High coupling to the internal representation of config variables, which complicates changes to the configuration mapping layer and violates encapsulation principles.

#### 2. Globally Shared State
* **File & Line:** `crates/op-ml/src/model_manager.rs:17`
* **Definition:** `static MODEL_MANAGER: OnceCell<Arc<ModelManager>> = OnceCell::new();`
* **Type:** Globally shared lazily-initialized singleton context.
* **Impact:** Although thread-safe due to `OnceCell` sync properties, global shared state complicates parallel unit testing and can mask dependency initialization ordering bugs.

---

### Schema-as-Code Violations

#### 1. Ad-Hoc Data Contracts and Hardcoded Configuration Schemas
* **File & Line:** `crates/op-ml/src/config.rs:142`, `crates/op-ml/src/config.rs:15`, `crates/op-ml/src/config.rs:100`
* **Details:** `VectorizationConfig`, `VectorizationLevel`, and `ExecutionProvider` represent configuration schemas and deployment models as ad-hoc Rust structs and enums rather than utilizing a unified, versioned schema definition (e.g., Protocol Buffers or OSCAL-compliant component definitions).
* **Impact:** Changes to deployment contracts require manual Rust code updates and recompilation, hindering interoperability with unified system control planes.

#### 2. Manual Environment and String Parsing Logic
* **File & Line:** `crates/op-ml/src/config.rs:167-217`
* **Details:** Parsing environment configuration (`OP_DBUS_VECTOR_LEVEL`, `OP_DBUS_MODEL_DIR`, etc.) is implemented using ad-hoc string comparisons (`to_lowercase()`, `as_str()`, etc.) instead of declarative schema-driven validation engines.

---

### Technical Security & Reliability Risks

#### 1. Async-in-Sync Runtime Panic Hazard (High Risk)
* **File & Line:** `crates/op-ml/src/model_manager.rs:132-133`
* **Code:**
  ```rust
  let model_dir = tokio::runtime::Handle::current()
      .block_on(async { self.ensure_model_downloaded().await })?;
  ```
* **Vulnerability Analysis:**
  The `get_or_load_embedder()` function uses `tokio::runtime::Handle::current().block_on(...)` to run async download routines synchronously. 
  If this code is executed inside an active multi-threaded Tokio worker thread (which is standard for server applications using axum/grpc as defined in the master `Cargo.toml`), this call **will immediately panic** with:
  `"Cannot start a runtime from within a runtime"`
  This introduces a severe Denial of Service (DoS) vulnerability that can be triggered on-demand by requesting an embedding when the model has not yet been loaded.
* **Remediation:**
  Refactor `ModelManager` to perform model downloading and instantiation during system bootstrap, or utilize a fully asynchronous pipeline instead of wrapping asynchronous blocks in synchronous mutex/cell initializers.