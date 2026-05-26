# Production Security and Quality Audit: op-ml

---

## 1. Executive Summary

This document presents a production-grade security, allocation, and quality audit of the `op-ml` crate. The crate implements machine learning utility routines, local cache downloading from the Hugging Face Hub, and embedding generation using the ONNX Runtime (`ort`). 

Our analysis revealed **no directly exploitable Critical vulnerabilities** in the isolated source files (e.g., remote code execution or authorization bypass), but identified several **High and Medium-risk architectural, performance, and compliance defects**:
1. **High Risk (Runtime Panic / DoS):** Synchronous execution of `block_on` inside the active Tokio runtime context during on-demand lazy loading of models.
2. **Medium Risk (Supply Chain / Integrity Bypass):** Complete lack of cryptographic hash verification on downloaded ONNX binaries and tokenizers, relying blindly on the upstream cache and local copies.
3. **High Performance overhead:** Repeated hot-path heap allocations inside ONNX tensor packing and vector L2 normalization loops.
4. **Schema-as-Code Compliance Defect:** Use of ad-hoc Serde serialization structs rather than structured, versioned Protocol Buffers or OSCAL validation definitions.

---

## 2. Memory Map & Allocation Analysis

This section analyzes memory mapping behavior, sled storage usage, and heap allocation metrics within the `op-ml` crate.

### Memory Mapping Context
While the provided files in `crates/op-ml` do not explicitly invoke `memmap2::Mmap` directly in their Rust source, the crate depends on the ONNX Runtime session binder (`ort`). `Session::commit_from_file` (invoked at `crates/op-ml/src/embedder.rs:117`) internally instructs the underlying C/C++ ONNX Runtime library to memory-map the model file (`model.onnx`) read-only (`ro`) to prevent reading the entire binary into physical RAM. 

### Sled Storage Warning
In the workspace `Cargo.toml`, the `cozo` database engine is configured with `storage-sled`. If `sled` databases are opened on a directory residing on a `tmpfs` or `noexec` mount, write operations or internal memory maps will fail silently, leading to data loss or instant process crashes under strict Linux security profiles.

### Large Heap Allocations
The system allocates large buffers for model caching and execution:
* **Model Binaries on Heap/Disk:** MiniLM-L3 (~61MB), MiniLM-L6 (~80MB), and MPNet-base-v2 (~420MB) are loaded into virtual memory space during session initialization (`crates/op-ml/src/embedder.rs:117`).
* **Input Tensor Arrays:** During batch embeddings, double-precision tensors (`Array2`) are allocated on the heap based on `batch_size * max_len` (`crates/op-ml/src/embedder.rs:183-184`).

### Memory Map Table

| Site | File:Line | Type | Risk |
| :--- | :--- | :--- | :--- |
| `Session::commit_from_file` | `crates/op-ml/src/embedder.rs:117` | `ro` (via `ort` library) | Low. High-capacity read-only mapping of `model.onnx` can exhaust virtual memory addresses on 32-bit archs. |
| `std::fs::copy` | `crates/op-ml/src/downloader.rs:132` | Heap buffer copy | Medium. Large model files are copied sequentially, causing transient allocation spikes. |
| Sled Storage (Workspace) | `Cargo.toml` | `sled` (implicit map) | High if instantiated on `tmpfs` or `noexec` mounts; triggers system violation faults. |

---

## 3. Schema-as-Code Compliance Review

The codebase fails to adhere to the schema-as-code discipline:

* **Ad-hoc Serialization Structs (`crates/op-ml/src/config.rs:142-162`):** The vectorization control state `VectorizationConfig` and its inner properties `VectorizationLevel` and `ExecutionProvider` are specified as ad-hoc, raw Rust structures using native `serde` macros. 
* **Lack of Versioned Schema Definition:** There are no versioned Protocol Buffers (`.proto`) or OSCAL-compliant component definitions specifying the parameters for vectorization depth, batch boundaries, or execution provider backends. 
* **Impact:** Any integration changes to configuration parameters require recompilation of the Rust binary. This limits declarative compliance validations and cross-language control plane configurations.

---

## 4. Detailed Findings & Vulnerability Analysis

### [Finding 1] High - Runtime Panic / Denial of Service via `block_on` in Active Async Context
* **Citation:** `crates/op-ml/src/model_manager.rs:120`
* **Defect:** Synchronous blocking call inside active runtime thread.
* **Impact:** 
  The lazy initialization routine `get_or_load_embedder` calls `ensure_model_downloaded` within `tokio::runtime::Handle::current().block_on(...)`:
  ```rust
  let model_dir = tokio::runtime::Handle::current()
      .block_on(async { self.ensure_model_downloaded().await })?;
  ```
  If `embed` or `embed_batch` is called from within an active asynchronous task running on the same Tokio runtime (which is the standard architecture of the `op-dbus` gateway/services), invoking `block_on` on the current handle will trigger an immediate panic:
  `"Cannot start a runtime from within a runtime. This happens because block_on was called from within an async context."`
* **Exploitability:** An attacker submitting a request that lazily triggers model loading (if the model is not yet cached or loaded) can consistently crash the control plane daemon, creating a trivial remote Denial of Service (DoS) path.
* **Remediation:** Remove `block_on` from the synchronous execution path. Perform the model download and session loading during the asynchronous bootstrap phase of the control plane daemon, or use a dedicated blocking thread pool (e.g., `tokio::task::spawn_blocking`) to safely initialize the model.

---

### [Finding 2] Medium - Insecure Supply Chain: No Cryptographic Integrity Verification of ONNX Models
* **Citation:** `crates/op-ml/src/downloader.rs:120-134`
* **Defect:** Model binaries are downloaded and copied into system paths with zero cryptographic validation.
* **Impact:** 
  The `ModelDownloader::download_file` function pulls models from Hugging Face Hub, retrieves the file path, and copies it directly into the production model cache:
  ```rust
  let file_path = repo.get(file_name).await.context(...)?;
  let target_path = target_dir.join(file_name);
  std::fs::copy(&file_path, &target_path).context(...)?;
  ```
  No validation (such as checking SHA-256 digests against a set of trusted hardcoded hashes) is performed on the binary. 
* **Exploitability:** If an attacker intercepts the download mirror, compromises the upstream Hugging Face repository, or achieves write access to the local cache directory, they can substitute `model.onnx` with a malicious payload. Loading a modified ONNX model into `ort` / ONNX Runtime can trigger known buffer overflows or arbitrary code execution vulnerabilities within native C++ runtimes.
* **Remediation:** Introduce a strict SHA-256 integrity-checking matrix for all valid models. Prior to loading the session in `TextEmbedder::load`, verify the payload digest.

---

### [Finding 3] Low - Weak Permissions on Hardcoded Default Directory
* **Citation:** `crates/op-ml/src/config.rs:163`
* **Defect:** Default model directory resides in a shared global directory.
* **Impact:**
  The default configuration points to `/var/lib/op-dbus/models`:
  ```rust
  model_dir: std::path::PathBuf::from("/var/lib/op-dbus/models")
  ```
  If the directory permissions are loosely set or if the service runs with root permissions while maintaining fallback permissions, a low-privileged system user can dump a custom `model.onnx` inside the folder to hijack model logic or exploit path parsing.
* **Remediation:** Enforce runtime directory validation. Assert that `/var/lib/op-dbus/models` is strictly owned by the service user (e.g., `op-dbus:op-dbus`) and has `0750` or `0700` permissions.

---

### [Finding 4] Informational - Extreme Performance Degradation in Hot-Path Tensor Loop
* **Citation:** `crates/op-ml/src/embedder.rs:162-177`, `crates/op-ml/src/embedder.rs:201-206`
* **Defect:** Repeated heap allocations and conversions inside high-throughput loops.
* **Impact:**
  During batch embedding operations, the system allocates fresh arrays and vectors continually:
  1. No pre-allocation for input vectors (lines 162-163):
     ```rust
     let mut input_ids_vec = Vec::new();
     let mut attention_mask_vec = Vec::new();
     ```
  2. Sequential vector allocation and resizing inside loop (lines 170-174):
     ```rust
     let mut padded_ids = ids.to_vec(); // Allocates on every iter
     let mut padded_mask = mask.to_vec(); // Allocates on every iter
     padded_ids.resize(max_len, 0); // Triggers reallocation
     padded_mask.resize(max_len, 0); // Triggers reallocation
     ```
  3. L2 Norm allocation on slice slices (lines 201-205 & 213):
     ```rust
     let row = embeddings.slice(ndarray::s![i, ..]).to_vec(); // Allocates
     let normalized = self.l2_normalize(&row); // Allocates map & collect
     ```
* **Remediation:** 
  Optimize memory allocations in `embed_batch`:
  * Pre-allocate `input_ids_vec` and `attention_mask_vec` using `Vec::with_capacity(batch_size * max_len)`.
  * Calculate padded offsets inline or write to a single pre-allocated flat vector instead of creating auxiliary `padded_ids` vectors.
  * Implement `l2_normalize_in_place` to normalize model output vectors directly inside the allocated tensor buffer without allocating secondary rows.