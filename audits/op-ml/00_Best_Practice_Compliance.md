| Pattern | File:Line | Crate Approach | Corpus Best Practice | Gap | Rating |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `unwrap_expect` | `crates/op-ml/src/config.rs:113` | Doctest uses `.unwrap()` to verify parsing of `"cpu"`. | Use `Result` returning doctests/tests with `?`, or use `.expect()` to provide clear failure context. | Doctest panics directly on failure instead of propagating errors cleanly. | Minor Gap |
| `unwrap_expect` | `crates/op-ml/src/config.rs:274`, `278`, `282`, `286` | Unit tests assert parsing success via `.unwrap()`. | Use `assert!(matches!(...))` or test helper assertions that display structured failures. | Repetitive `.unwrap()` pattern without custom assertion panic messages makes debugging test failures harder. | Minor Gap |
| `format_json_manual` | `crates/op-ml/src/downloader.rs:27`, `102`, `130`, `crates/op-ml/src/embedder.rs:109`, `crates/op-ml/src/model_manager.rs:191` | Eager string formatting inside `.context(format!(...))` for error propagation. | Use lazy evaluation via `.with_context(|| format!(...))` to avoid allocating strings on the success path. | Eager evaluation of `format!` strings creates heap allocations even when operations succeed. | Major Gap |
| `std_fs_in_async` | `crates/op-ml/src/downloader.rs:26`, `75`, `129` | Synchronous blocking I/O calls (`std::fs::create_dir_all`, `std::fs::copy`) inside an async context. | Use asynchronous I/O (`tokio::fs`) or wrap heavy blocking filesystem tasks in `tokio::task::spawn_blocking`. | Synchronous filesystem calls block the async reactor execution thread, degrading system concurrency. | Major Gap |
| `schema_as_code` | `crates/op-ml/src/config.rs:113`, `274` | Configuration options like `ExecutionProvider` and `VectorizationLevel` parsed from ad-hoc strings. | Declare versioned system configuration contracts using formal schemas (e.g., Protobuf or OSCAL component definitions) serialized via Serde. | Lack of formalized data contracts leads to fragile, string-based, hand-maintained configuration parsing. | Minor Gap |

---

### Actionable Recommendations

#### 1. Eliminate Blocking I/O in Asynchronous Contexts (`std_fs_in_async`)
* **Location:** `crates/op-ml/src/downloader.rs:26`, `75`, `129`
* **Resolution:** Replace blocking filesystem calls with non-blocking alternatives or run them inside a blocking pool. If using `tokio`, replace `std::fs` calls with their asynchronous equivalents from `tokio::fs`.
  * *Example fix for `crates/op-ml/src/downloader.rs:26`:*
    ```rust
    // Instead of: std::fs::create_dir_all(&cache_dir)
    tokio::fs::create_dir_all(&cache_dir)
        .await
        .with_context(|| format!("Failed to create cache directory: {:?}", cache_dir))?;
    ```
  * Alternatively, wrap the entire file-copy loop block inside a `spawn_blocking` call if using a synchronous download architecture wrapped in async.

#### 2. Prevent Happy-Path Allocations in Error Contexts (`format_json_manual`)
* **Location:** `crates/op-ml/src/downloader.rs:27`, `102`, `130`, `crates/op-ml/src/embedder.rs:109`, `crates/op-ml/src/model_manager.rs:191`
* **Resolution:** Change all instances of `.context(format!(...))` to `.with_context(|| format!(...))`. This guarantees that string formatting and its associated memory allocations only execute when an error actually occurs.
  * *Example fix for `crates/op-ml/src/downloader.rs:130`:*
    ```rust
    // Instead of: .context(format!("Failed to copy {} to {:?}", file_name, target_path))
    .with_context(|| format!("Failed to copy {} to {:?}", file_name, target_path))
    ```