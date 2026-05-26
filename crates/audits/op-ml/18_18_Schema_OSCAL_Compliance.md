# Production Security and Quality Audit: `op-ml` Crate

## 1. Schema-as-Code Compliance

The codebase uses ad-hoc Rust structs with Serde annotations to manage configurations, semantic levels, and execution providers. These structures are not derived from versioned schemas (such as Protocol Buffers), preventing multi-language interoperability and robust schema evolution guarantees for the control plane.

| Item | Type | file:line | Has .proto? | Gap |
| :--- | :--- | :--- | :--- | :--- |
| `VectorizationLevel` | Enum | `crates/op-ml/src/config.rs:16` | No | Defined as an ad-hoc Serde representation. This enum dictates model selections and output dimensions, but has no machine-readable schema definition. |
| `ExecutionProvider` | Enum | `crates/op-ml/src/config.rs:88` | No | Hardcoded hardware acceleration configurations. Lacks a language-agnostic interface representation. |
| `VectorizationConfig` | Struct | `crates/op-ml/src/config.rs:131` | No | Defined strictly as a Rust struct. It cannot be easily ingested, validated, or sent across language boundaries without manual translation layers. |
| Model Embeddings | Data Contract | `crates/op-ml/src/embedder.rs:131` | No | Raw `Vec<Vec<f32>>` representations are returned without a versioned schema envelope containing metadata (dimensions, model hash, distance metric). |

---

## 2. OSCAL Compliance Coverage

The automated downloading and loading of machine learning models (`.onnx` binaries) from external repositories introduces supply chain vulnerabilities that must be codified in system compliance artifacts.

| Control Area | Implemented at file:line | OSCAL Artifact | Gap |
| :--- | :--- | :--- | :--- |
| **Software, Firmware, and Information Integrity** (NIST SP 800-53 SI-7) | `crates/op-ml/src/downloader.rs:72` | None | External ONNX model binaries and configuration files are downloaded from Hugging Face Hub without local cryptographic hash (SHA-256) enforcement or signature validation. |
| **Mobile Code / Execution of External Binary Objects** (NIST SP 800-53 SC-18) | `crates/op-ml/src/embedder.rs:114` | None | The crate executes dynamic model graphs using ONNX Runtime (`ort`) directly from the local filesystem path `/var/lib/op-dbus/models` without code-signing verifications. |
| **Least Privilege & File System Protections** (NIST SP 800-53 AC-3) | `crates/op-ml/src/config.rs:151` | None | Default model directory is hardcoded to `/var/lib/op-dbus/models`. System isolation boundaries for this writable path are undocumented in an OSCAL Component Definition. |

---

## 3. Vulnerability Findings and Quality Gaps

### [CRITICAL] Runtime Panic via Async-in-Sync Blocking (`Handle::block_on`)
* **Location:** `crates/op-ml/src/model_manager.rs:135-137`
* **Impact:** High Availability Loss / Denial of Service (DoS)
* **Description:** 
  In `ModelManager::get_or_load_embedder`, the lazy-initialization block uses `OnceCell::get_or_try_init` (which is synchronous). Inside this synchronous closure, the code attempts to download models by calling:
  ```rust
  let model_dir = tokio::runtime::Handle::current()
      .block_on(async { self.ensure_model_downloaded().await })?;
  ```
  If `ModelManager::embed` (or `embed_batch`) is invoked from an existing asynchronous context (e.g., inside an Axum handler, a Tonic gRPC service, or a Zbus DBus handler, all of which run on the Tokio thread pool), this call will retrieve the active Tokio handle and invoke `block_on`.
  
  Tokio strictly prohibits nested runtime execution. Calling `Handle::block_on` from within an active asynchronous worker thread immediately panics with:
  `"Cannot start a runtime from within a runtime. This happens because block_on was called from within an async context."`
  
  Because the control plane depends on these embeddings, any incoming requests that trigger lazy-loading of the model will immediately crash the entire control plane process.

---

### [MAJOR] Remote Code Execution (RCE) / Supply Chain Integrity Risk via Unverified Model Execution
* **Location:** `crates/op-ml/src/downloader.rs:72-111` and `crates/op-ml/src/embedder.rs:114`
* **Impact:** Integrity Compromise / Remote Code Execution
* **Description:** 
  The `ModelDownloader` downloads `.onnx` model files on-demand directly from Hugging Face Hub. Once downloaded, `TextEmbedder::load` commits the file directly to ONNX Runtime:
  ```rust
  let session = builder
      .commit_from_file(&model_path)
      .context(format!("Failed to load ONNX model from {:?}", model_path))?;
  ```
  ONNX Runtime parses complex binary computational graphs in C++. Historically, parsing untrusted ONNX files has led to heap buffer overflows, out-of-bounds reads, and arbitrary code execution.
  
  Since the downloader does not pin the cryptographic hash (SHA-256) of the target model or verify an authenticity signature, a compromised Hugging Face repository or a Man-in-the-Middle (MitM) attacker could deliver a malformed or poisoned model file that exploits ONNX Runtime parser vulnerabilities to execute code in the context of the control plane (often running as root or with elevated capabilities).

---

### [MAJOR] Privilege Escalation via Environment Variable Hijacking
* **Location:** `crates/op-ml/src/config.rs:179-181`
* **Impact:** Local Privilege Escalation
* **Description:** 
  The configuration loader resolves the model directory dynamically from environment variables:
  ```rust
  if let Ok(model_dir) = std::env::var("OP_DBUS_MODEL_DIR") {
      config.model_dir = std::path::PathBuf::from(model_dir);
  }
  ```
  If the control plane daemon executes with high privileges, a local attacker with the ability to modify the environment of the service (or inject environment parameters through systemd service overrides or process creation vectors) can rewrite `OP_DBUS_MODEL_DIR` to point to a globally writable directory (e.g., `/tmp`). They can pre-stage a malicious `model.onnx` file in that directory. When the control plane generates embeddings, it will load and execute the attacker's model file, resulting in elevated privilege execution.

---

## 4. Recommendations

### Recommendation 1: Fix the Critical Runtime Panic (Async-in-Sync)
Refactor the model manager and embedder to use fully asynchronous paths. Replace the synchronous `once_cell::sync::OnceCell` with an asynchronous lock or initializer such as `tokio::sync::OnceCell`, ensuring that `get_or_load_embedder` and `embed` are non-blocking `async fn` signatures.

```rust
// In crates/op-ml/src/model_manager.rs:
use tokio::sync::OnceCell;

pub struct ModelManager {
    config: VectorizationConfig,
    embedder: OnceCell<TextEmbedder>,
}

impl ModelManager {
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        if !self.is_enabled() {
            return Ok(Vec::new());
        }
        let embedder = self.get_or_load_embedder().await?;
        embedder.embed(text)
    }

    async fn get_or_load_embedder(&self) -> Result<&TextEmbedder> {
        self.embedder.get_or_try_init(|| async {
            log::info!("Loading {} model on-demand...", self.config.level);
            let model_dir = self.ensure_model_downloaded().await?;
            TextEmbedder::load(&model_dir, &self.config)
        }).await
    }
}
```

### Recommendation 2: Implement Cryptographic Signature and Checksum Pinning
Extend the `VectorizationConfig` or introduce a metadata file mapping each `VectorizationLevel` to its expected SHA-256 hash. Verify this hash before committing the model to ONNX Runtime.

```rust
// In crates/op-ml/src/downloader.rs:
use sha2::{Digest, Sha256};
use std::io::Read;

fn verify_model_hash(path: &Path, expected_hex_hash: &str) -> Result<()> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 { break; }
        hasher.update(&buffer[..count]);
    }
    let result = hasher.finalize();
    let actual_hex = hex::encode(result);
    if actual_hex != expected_hex_hash {
        return Err(anyhow::anyhow!("Cryptographic hash mismatch! Refusing to load untrusted model."));
    }
    Ok(())
}
```

### Recommendation 3: Enforce Schema-as-Code
Define a structured config and response envelope in a protocol buffer file (e.g., `vector_service.proto`) to represent the configuration, models, and embeddings:

```protobuf
syntax = "proto3";
package op.ml.v1;

enum VectorizationLevel {
  VECTORIZATION_LEVEL_UNSPECIFIED = 0;
  VECTORIZATION_LEVEL_NONE = 1;
  VECTORIZATION_LEVEL_LOW = 2;
  VECTORIZATION_LEVEL_MEDIUM = 3;
  VECTORIZATION_LEVEL_HIGH = 4;
}

message VectorizationConfig {
  VectorizationLevel level = 1;
  string model_dir = 2;
  int32 batch_size = 3;
  int64 load_timeout_secs = 4;
}
```
Incorporate this `.proto` file into the gRPC build pipeline (`op-grpc-bridge`) and implement the `prost::Message` trait for model parsing.