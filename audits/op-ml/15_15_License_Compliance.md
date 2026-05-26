# Production Security, Quality, and License Audit Report

---

## 1. License Audit

### 1.1 License Field Extraction
* **Workspace License**: `Apache-2.0` (defined in `Cargo.toml:43` under `[workspace.package]`).
* **Crate `op-ml` License**: `Apache-2.0` (inherited from workspace via `crates/op-ml/Cargo.toml:7`).
* **Crate `op-dbus` License**: `Apache-2.0` (inherited from workspace via `Cargo.toml:122`).

### 1.2 License Compatibility Scan (`Cargo.lock`)
The following license incompatibility was identified during the scan of `Cargo.lock`:

* **Conflict Found**: `cozo` version `0.7.6` (`Cargo.lock:396`) is licensed under the **GNU General Public License v3.0 (GPL-3.0)**. 
* **Impact**: Under the copyleft provisions of GPL-3.0, any combined work or derivative work compiled with `cozo` must be licensed as a whole under GPL-3.0. This directly conflicts with the permissive `Apache-2.0` license claimed in `Cargo.toml:43`. Sublicensing GPL-3.0 covered code under the Apache-2.0 license is legally invalid and creates a major compliance violation for downstream distribution of the combined binary.

### 1.3 Crates with No License Field
* All internal workspace crates defined in `Cargo.toml` inherit the workspace license field correctly. No custom/ad-hoc internal crates were found lacking a license declaration.
* Note that `Cargo.lock` does not natively store licensing metadata for third-party dependencies; registry license verification must be performed out-of-band for untrusted external crates.

---

## 2. Schema-as-Code Compliance Audit

The codebase follows a strict schema-as-code discipline utilizing Protocol Buffers and OSCAL. The following components violate this policy by expressing data contracts as ad-hoc Rust structs or strings rather than versioned, validated schemas:

* **Ad-Hoc Config Struct**: `VectorizationConfig` (`crates/op-ml/src/config.rs:109`) defines the serialized configurations for the machine learning engine using ad-hoc native Rust types.
* **Ad-Hoc Config Enums**: `VectorizationLevel` (`crates/op-ml/src/config.rs:14`) and `ExecutionProvider` (`crates/op-ml/src/config.rs:92`) express configuration states as standard Serde-serializable enums rather than utilizing versioned Protobuf messages.
* **Ad-Hoc Model Naming**: Model identifiers (`crates/op-ml/src/config.rs:28`) are expressed as hardcoded static strings rather than schema-governed registry entries.

---

## 3. Security and Quality Findings

### CRITICAL: Runtime Panic / Denial of Service via `block_on` inside Active Tokio Runtime
* **File & Line**: `crates/op-ml/src/model_manager.rs:144`
* **Vulnerability Class**: Thread State Violation / Denial of Service (DoS)
* **Description**: The global `ModelManager` initializes and downloads the transformer model on-demand inside `get_or_load_embedder` using a synchronous block-on call:
  ```rust
  let model_dir = tokio::runtime::Handle::current()
      .block_on(async { self.ensure_model_downloaded().await })?;
  ```
  Because the web gateway (`op-gateway`), web server (`op-web`), and cognitive store (`op-cognitive-mcp`) operate entirely within an active Tokio runtime, calling `block_on` from any of their async task threads (e.g., during an incoming API request that triggers `ModelManager::global().embed()`) will result in an immediate runtime panic: `"Cannot start a runtime from within a runtime."`
* **Exploitability**: **Directly Exploitable**. An unauthenticated remote user can send any request requiring embedding/vectorization. When the service lazily loads the model for the first time on the Tokio worker thread, the thread panics. This allows trivial unauthenticated remote Denial of Service (DoS).
* **Remediation**: Avoid blocking the async executor thread. Convert `embed` and `get_or_load_embedder` into asynchronous functions (`async fn`) and natively `.await` the model download process instead of utilizing synchronous `block_on`.

---

### MAJOR: Broken Unit Test via `todo!` Macro Evaluation
* **File & Line**: `crates/op-ml/src/embedder.rs:258`
* **Vulnerability Class**: Quality Bug / Broken CI Pipeline
* **Description**: The unit test `test_l2_normalize` instantiates `TextEmbedder` with placeholder `todo!()` macros for `session` and `tokenizer`:
  ```rust
  let embedder = TextEmbedder {
      session: todo!(), // Mock for test
      tokenizer: todo!(),
      level: VectorizationLevel::Medium,
  };
  ```
  Although the test only exercises `l2_normalize`, which does not read the `session` or `tokenizer` fields, the construction of the struct eagerly evaluates the `todo!()` macros. This results in an immediate unconditional panic when the unit test runs.
* **Impact**: The unit test suite is permanently broken and cannot pass in CI, masking potential regressions.
* **Remediation**: Implement a safe mock or change `l2_normalize` to be an associated function (`fn l2_normalize(vec: &[f32]) -> Vec<f32>`) or a standalone utility function that does not require instantiating `TextEmbedder`.

---

### MEDIUM: Insecure Default Model Storage Directory Permissions
* **File & Line**: `crates/op-ml/src/config.rs:119`
* **Vulnerability Class**: Insecure Default / Privilege Escalation Risk
* **Description**: The default storage directory for downloaded ONNX models is set to a global system directory:
  ```rust
  model_dir: std::path::PathBuf::from("/var/lib/op-dbus/models"),
  ```
  If this directory is created with insecure permissions (or is not restricted to `0700` owned by the service user), a local attacker on a multi-user system could pre-create or modify the directory structure. They could plant a malicious `model.onnx` file, which is then loaded by `Session::builder().commit_from_file(&model_path)` (`crates/op-ml/src/embedder.rs:135`), leading to local privilege escalation or arbitrary code execution within the context of the service.
* **Remediation**: Ensure that `create_dir_all` enforces strict permissions (POSIX permissions `0700`) and validate that the model directory is owned exclusively by the service user before attempting to load ONNX binaries.

---

### MEDIUM: Predictable Temporary Directory in Test Environment
* **File & Line**: `crates/op-ml/src/downloader.rs:152`
* **Vulnerability Class**: Temporary File Vulnerability (Symlink Attack)
* **Description**: The test block utilizes a highly predictable path in the shared system temporary directory:
  ```rust
  let temp_dir = std::env::temp_dir().join("op-dbus-test-models");
  ```
  On shared systems, any local attacker can pre-create `/tmp/op-dbus-test-models` or create a symlink pointing to sensitive system files. When the test suite runs and calls `create_dir_all` or writes files, it could overwrite sensitive files owned by the testing user.
* **Remediation**: Use the `tempfile` crate (already present in `Cargo.toml` workspace dependencies) to generate cryptographically secure, unpredictable temporary directory paths for unit tests.