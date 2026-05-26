| Pattern | File:Line | Crate Approach | Corpus Best Practice | Gap | Rating |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `format_json_manual` | `crates/op-mcp-aggregator/src/aggregator.rs:176` | Manual error string formatting with context | Structured error context / schema-defined errors | Uses ad-hoc string formatting for system tool call failures instead of a structured, versioned error schema. | Minor Gap |
| `format_json_manual` | `crates/op-mcp-aggregator/src/aggregator.rs:281` | Manual connection error string formatting | Structured status reporting | Manual ad-hoc string formatting for connection errors instead of typed error states. | Minor Gap |
| `unwrap_expect` | `crates/op-mcp-aggregator/src/aggregator.rs:803` | Using `.unwrap()` in async initialization within a test context | Return `Result` or use `.expect()` with descriptive message | Use of raw `.unwrap()` makes test failures harder to debug; returning `Result` or using `.expect` is cleaner. | Compliant |
| `unwrap_expect` | `crates/op-mcp-aggregator/src/aggregator.rs:812` | Using `.unwrap()` in async test setup | Return `Result` or use `.expect()` with descriptive message | Use of raw `.unwrap()` in test setup. | Compliant |
| `unwrap_expect` | `crates/op-mcp-aggregator/src/aggregator.rs:815` | Using `.unwrap()` in async test verification | Return `Result` or use `.expect()` | Use of raw `.unwrap()` in test execution. | Compliant |
| `unwrap_expect` | `crates/op-mcp-aggregator/src/cache.rs:66` | `.unwrap_or(NonZeroUsize::new(1000).unwrap())` | Safe, statically-known defaults | Nested raw `.unwrap()` for a statically known non-zero default. Can be replaced with safe constant or `expect`. | Minor Gap |
| `unwrap_expect` | `crates/op-mcp-aggregator/src/cache.rs:256` | Using `.unwrap()` in unit test assertions | Native assertions or test `Result` | Standard test assertion pattern, safe in test code. | Compliant |
| `format_json_manual` | `crates/op-mcp-aggregator/src/client.rs:26` | Ad-hoc URL construction `format!("{}/mcp", ...)` | Schema-defined, strongly-typed endpoints or URI templates | Violates Schema-as-Code by using ad-hoc string concatenation to build critical API endpoints instead of a typed API router or client schema. | Minor Gap |
| `format_json_manual` | `crates/op-mcp-aggregator/src/client.rs:30` | Ad-hoc URL construction `format!("{}/message", ...)` | Schema-defined, strongly-typed endpoints or URI templates | Violates Schema-as-Code by manually constructing endpoints via string interpolation. | Minor Gap |
| `format_json_manual` | `crates/op-mcp-aggregator/src/client.rs:118` | Manual auth header formatting | Strong-typed auth token structs or middleware | Formatting sensitive auth headers manually instead of using safe `HeaderValue` constructor or middleware. | Minor Gap |
| `unsafe_block` | `crates/op-mcp-aggregator/src/config.rs:87` | `unsafe { content.as_bytes_mut() }` to parse JSON with `simd_json` | Safe conversion or using byte-level APIs directly | Severe risk of Undefined Behavior (UB). Mutating a `String`'s underlying bytes can violate UTF-8 invariants. | Major Gap |
| `std_fs_in_async` | `crates/op-mcp-aggregator/src/config.rs:76` | `std::fs::read_to_string(path)` in async function | Use asynchronous fs APIs (e.g., `tokio::fs`) | Blocking standard I/O in an async context. Blocks the runtime's executor thread. | Minor Gap |

### Actionable Recommendations for Major/Critical Gaps

#### 1. Eliminate Undefined Behavior Risk in JSON Configuration Parsing
* **File:** `crates/op-mcp-aggregator/src/config.rs:87`
* **Issue:** The use of `unsafe { content.as_bytes_mut() }` is highly dangerous. In Rust, it is a strict invariant that `str` and `String` contain valid UTF-8. If `simd_json::from_slice` mutates the byte slice (such as inserting null terminators or unescaping characters in-place) and temporarily violates UTF-8 constraints, it results in immediate Undefined Behavior (UB) when the compiler optimizes or drops the `String`.
* **Remediation:** Avoid reading the configuration file as a string altogether. Read it directly into a raw byte vector (`Vec<u8>`), which can be safely mutated by `simd_json` without any unsafe blocks or UTF-8 invariant violations. Combine this with asynchronous file I/O to resolve the async blocking issue in `crates/op-mcp-aggregator/src/config.rs:76`.

```rust
// Replace crates/op-mcp-aggregator/src/config.rs:76-88 with:
let path = path.as_ref();
let mut content_bytes = tokio::fs::read(path)
    .await
    .with_context(|| format!("Failed to read config from {}", path.display()))?;

let config: AggregatorConfig = simd_json::from_slice(&mut content_bytes)
    .with_context(|| "Failed to parse JSON config")?;
```