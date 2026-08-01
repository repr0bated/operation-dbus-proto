### 1. BUILD ROLE AUDIT FINDINGS

*   **Edition & Rust Version:** 
    *   The `crates/op-ml/Cargo.toml` crate inherits its edition from the workspace package definition via `edition.workspace = true`. 
    *   In the root `Cargo.toml` (lines 35-40), the inherited workspace edition is defined as `"2021"`. 
    *   No specific minimum supported Rust version (`rust-version`) is specified in either `Cargo.toml` or `crates/op-ml/Cargo.toml`.
*   **Binaries & Examples:** 
    *   There are no binary targets (`[[bin]]`) or examples (`[[example]]`) configured within `crates/op-ml/Cargo.toml`.
*   **Build Scripts (`build.rs`):** 
    *   No `build.rs` build script is present in the `crates/op-ml` directory. Thus, there are no compile-time arbitrary shell execution or unsafe code-generation risks in this crate.
*   **Workspace Inheritance vs. Local Overrides:** 
    *   The crate heavily utilizes workspace inheritance. The fields `version`, `edition`, `authors`, `license`, and `description` are inherited from the root workspace using `.workspace = true`.
    *   All core dependencies—including `tokio`, `serde`, `simd-json`, `anyhow`, `thiserror`, `tracing`, `reqwest`, `log`, `num_cpus`, and `sha2`—are inherited from the workspace block.
    *   `hf-hub` is declared as a local override with version `"0.5.0"` and the `"tokio"` feature enabled locally (crates/op-ml/Cargo.toml:19).

---

### 2. SCHEMA-AS-CODE BUILD CHECK

*   **Runtime/Build Protobuf Generation:** 
    *   The `op-ml` crate does not invoke `prost-build` or `tonic-build` to compile `.proto` files, nor are any Protocol Buffer schemas or OSCAL schemas declared inside this crate.
*   **Data Contract Violations (Schema-as-Code Discipline):**
    *   **Ad-hoc Struct Serialization:** In `crates/op-ml/src/config.rs:154-171`, the configuration parameter contract (`VectorizationConfig`) is defined as an ad-hoc Rust struct with Serde attributes. This contract controls critical operational metrics (execution providers, GPU device IDs, directory locations) but lacks a declarative, versioned schema (e.g., Protocol Buffers or OSCAL templates).

---

### 3. SECURITY & QUALITY AUDIT REPORT

#### [CRITICAL] Denial of Service (DoS) via Sync-over-Async Runtime Panic
*   **Location:** `crates/op-ml/src/model_manager.rs:148-149`
*   **Vulnerability Type:** Thread Blocking & Runtime Panic / Unhandled Exception
*   **Description:** 
    The synchronous method `get_or_load_embedder` calls `tokio::runtime::Handle::current().block_on(...)` to download the model on-demand when `embed` or `embed_batch` is invoked:
    ```rust
    let model_dir = tokio::runtime::Handle::current()
        .block_on(async { self.ensure_model_downloaded().await })?;
    ```
    If this library is integrated into an asynchronous daemon (e.g., an Axum endpoint or a tonic gRPC service running on a Tokio worker thread), calling `block_on` from within an active asynchronous execution context will trigger an immediate, unhandled runtime panic:
    `"cannot start a runtime from within a runtime"` or `"block_on panics when called from an async context"`.
    This allows any request that triggers a lazy-load model vectorization to instantly terminate the control plane process.
*   **Remediation:** 
    Remove runtime blocking calls from the synchronous hot-path. Initialize the `ModelManager` and download all required models asynchronously during application startup (`main` bootstrap), or make `embed` and `embed_batch` fully asynchronous API calls.

---

#### [MEDIUM] Denial of Service (DoS) via Panicking Map Indexing on Missing Model Output
*   **Location:** `crates/op-ml/src/embedder.rs:182-185`
*   **Vulnerability Type:** Unhandled Panic / DoS
*   **Description:** 
    During model inference, the code extracts the tensor output using direct index notation:
    ```rust
    let embeddings = outputs["sentence_embedding"]
        .try_extract_tensor::<f32>()?
        .view()
        .to_owned();
    ```
    If a downloaded model or a fallback model (e.g., a custom model hosted under one of the Hugging Face IDs) does not output a node with the exact name `"sentence_embedding"`, the indexing operator `outputs[...]` will panic. This unhandled panic propagates up the stack and crashes the thread or process.
*   **Remediation:** 
    Use the safe lookup method `.get()` instead of direct index notation:
    ```rust
    let embedding_tensor = outputs.get("sentence_embedding")
        .ok_or_else(|| anyhow::anyhow!("Model output 'sentence_embedding' missing from inference response"))?;
    ```

---

#### [LOW] Broken Test Suite via Immediate Panic on Construction
*   **Location:** `crates/op-ml/src/embedder.rs:246-253`
*   **Vulnerability Type:** Quality / Fragile Test Code
*   **Description:** 
    The unit test `test_l2_normalize` constructs `TextEmbedder` using `todo!()` macros for `session` and `tokenizer`:
    ```rust
    let embedder = TextEmbedder {
        session: todo!(), // Mock for test
        tokenizer: todo!(),
        level: VectorizationLevel::Medium,
    };
    ```
    Since `todo!()` expands to an immediate panic expression at runtime, constructing this struct causes the test to fail immediately when `cargo test` is executed, preventing the test suite from running successfully.
*   **Remediation:** 
    Refactor `l2_normalize` to be a pure function or associated method on `TextEmbedder` that does not require instantiation of complex fields (`Session` and `Tokenizer`), allowing it to be tested in isolation.

---

#### [LOW] Incomplete Download Bypass via Exist-Only File Verification
*   **Location:** `crates/op-ml/src/downloader.rs:64-70`
*   **Vulnerability Type:** Robustness / Cache Corruption
*   **Description:** 
    The function `is_model_complete` determines if a model has been successfully downloaded by checking if the files exist on disk:
    ```rust
    fn is_model_complete(&self, model_dir: &Path) -> bool {
        let model_file = model_dir.join("model.onnx");
        let tokenizer_file = model_dir.join("tokenizer.json");

        model_file.exists() && tokenizer_file.exists()
    }
    ```
    If a download is interrupted or the process is terminated while writing `model.onnx` or `tokenizer.json`, the empty or truncated files will remain on disk. The system will falsely treat the cache as valid, skip future downloads, and persistently fail to load the model on subsequent starts.
*   **Remediation:** 
    Verify that the files are non-empty (e.g., size > 0) and check their cryptographic integrity, or attempt to parse the JSON and ONNX headers before concluding that the cache is valid.