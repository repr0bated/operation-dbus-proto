| Pattern | File:Line | Crate Approach | Corpus Best Practice | Gap | Rating |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `unsafe_block` | `crates/op-jsonrpc/src/nonnet.rs:278` | Invokes `unsafe { simd_json::from_str }` on mutable strings without documented safety invariants. | Safely wrap parsing or use robust safe parsers to ensure memory and UTF-8 safety invariants. | Lack of explicit safety documentation for in-place string mutation. | Minor Gap |
| `simd_json_from_str` | `crates/op-jsonrpc/src/nonnet.rs:278` | Performs in-place parsing utilizing raw mutable string slices. | Use standard safe parsing (`serde_json` or safe `simd_json` abstractions) unless performance is heavily profiled. | Risky mutable string parsing API used without explicit guardrails. | Minor Gap |
| `format_json_manual` | `crates/op-jsonrpc/src/nonnet.rs:286` | Formats dynamic JSON-RPC error responses using ad-hoc strings via `format!`. | Use versioned schemas (Protobuf or structured error structs) to guarantee schema-as-code compliance. | Violates schema-as-code discipline; error payloads are ad-hoc strings. | Major Gap |
| `format_json_manual` | `crates/op-jsonrpc/src/nonnet.rs:293` | Formats parse errors dynamically using ad-hoc `format!`. | Use versioned schemas (Protobuf or structured error structs) to guarantee schema-as-code compliance. | Violates schema-as-code discipline; error payloads are ad-hoc strings. | Major Gap |
| `format_json_manual` | `crates/op-jsonrpc/src/nonnet.rs:324` | Generates database-not-found errors via dynamic dynamic string formatting. | Use versioned schemas (Protobuf or structured error structs) to guarantee schema-as-code compliance. | Violates schema-as-code discipline; error payloads are ad-hoc strings. | Major Gap |
| `format_json_manual` | `crates/op-jsonrpc/src/nonnet.rs:348` | Generates error messages as manually formatted dynamic strings. | Use versioned schemas (Protobuf or structured error structs) to guarantee schema-as-code compliance. | Violates schema-as-code discipline; error payloads are ad-hoc strings. | Major Gap |
| `format_json_manual` | `crates/op-jsonrpc/src/nonnet.rs:371` | Constructs ad-hoc dynamic JSON payloads inline with `json!({"error": ...})`. | Define response data contracts as schema-as-code targets (Protobuf/OSCAL specs). | Violates schema-as-code; constructs JSON objects using raw macros and string literals. | Major Gap |
| `std_fs_in_async` | `crates/op-jsonrpc/src/nonnet.rs:234` | Invokes asynchronous `tokio::fs::create_dir_all`. | Use async runtime file-system operators. | None. Correct async FS utilization. | Compliant |
| `std_fs_in_async` | `crates/op-jsonrpc/src/nonnet.rs:239` | Invokes blocking synchronous `path.exists()` inside an async task. | Avoid blocking calls in async executor tasks; use asynchronous metadata queries. | Synchronous `Path::exists` blocks tokio executor threads. | Major Gap |
| `unwrap_expect` | `crates/op-jsonrpc/src/protocol.rs:133` | Uses `.unwrap()` on serialization results in test contexts. | Unwraps and panics are standard and correct in unit tests. | None. Test-only usage. | Compliant |
| `unwrap_expect` | `crates/op-jsonrpc/src/protocol.rs:140` | Uses `.unwrap()` on JSON generation in test contexts. | Unwraps and panics are standard and correct in unit tests. | None. Test-only usage. | Compliant |
| `unsafe_block` | `crates/op-jsonrpc/src/server.rs:225` | Employs unsafe `simd_json::from_str` with in-place mutation of incoming lines. | Document safety invariants or utilize safe fallback parsing methods. | Lacks clear documentation on safety guarantees of the incoming buffer slice. | Minor Gap |
| `simd_json_from_str` | `crates/op-jsonrpc/src/server.rs:225` | Parses input stream lines destructively in-place. | Use standard safe parsing alternatives. | In-place destructive parsing increases complexity of memory state verification. | Minor Gap |
| `std_fs_in_async` | `crates/op-jsonrpc/src/server.rs:128` | Invokes asynchronous `tokio::fs::create_dir_all`. | Use async runtime file-system operators. | None. Correct async FS utilization. | Compliant |
| `std_fs_in_async` | `crates/op-jsonrpc/src/server.rs:132` | Uses blocking `path.exists()` in an async thread context. | Query file existence asynchronously to avoid blocking the reactor thread. | Synchronous filesystem probe blocks tokio executor threads. | Major Gap |
| `simd_json_from_str` | `crates/op-jsonrpc/src/nonnet_staging.rs:40` | Parses JSON-RPC inputs using the safe `simd_json::from_str` interface. | Rely on safe parsing methods to preserve memory safety properties. | None. Safe API variant utilized. | Compliant |
| `unsafe_block` | `crates/op-jsonrpc/src/ovsdb.rs:94` | Calls unsafe `simd_json::from_str` on dynamic strings. | Ensure safety invariants are met and explicitly commented. | Undocumented unsafe block for dynamic deserialization. | Minor Gap |
| `unsafe_block` | `crates/op-jsonrpc/src/ovsdb.rs:106` | Employs unsafe parsing on cloned heap strings. | Fallback to safe parser variants. | Undocumented unsafe block on mutating strings. | Minor Gap |
| `unsafe_block` | `crates/op-jsonrpc/src/ovsdb.rs:493` | Performs unsafe mutating parsing of cloned transaction rows. | Use safe JSON parsing interfaces. | Undocumented unsafe block in transaction parser loop. | Minor Gap |
| `simd_json_from_str` | `crates/op-jsonrpc/src/ovsdb.rs:94` | Mutates string buffers in-place during parsing. | Prefer safe non-destructive parsers. | Non-destructive safe alternative should be favored unless microbenchmarked. | Minor Gap |
| `simd_json_from_str` | `crates/op-jsonrpc/src/ovsdb.rs:106` | Clones strings to support destructive parsing. | Prefer safe immutable parsing. | Allocates extra strings specifically to support destructive parser API. | Minor Gap |
| `unwrap_expect` | `crates/op-jsonrpc/src/ovsdb.rs:523` | Calls `.unwrap()` on a parsing operation inside production transaction logic. | Propagate errors via the `Result` type using `?` or construct meaningful context errors. | Panics production thread if JSON payload is malformed. | Major Gap |
| `unwrap_expect` | `crates/op-jsonrpc/src/ovsdb.rs:600` | Calls `.expect` on JSON parsing inside test module. | standard usage of panics in test environments. | None. Test-only context. | Compliant |
| `unwrap_expect` | `crates/op-jsonrpc/src/ovsdb.rs:610` | Calls `.expect` on test assertions. | standard usage of panics in test environments. | None. Test-only context. | Compliant |
| `std_fs_in_async` | `crates/op-jsonrpc/src/ovsdb.rs:631` | Uses `std::fs::remove_file` in test bootstrap context. | Standard practice accepts simple blocking operations in test setups. | None. Test-only context. | Compliant |

---

### Recommendations for Major/Critical Gaps

#### 1. Eliminate Schema-as-Code Violations (Ad-hoc String Formatting)
*   **Affected Files:** 
    * `crates/op-jsonrpc/src/nonnet.rs:286`
    * `crates/op-jsonrpc/src/nonnet.rs:293`
    * `crates/op-jsonrpc/src/nonnet.rs:324`
    * `crates/op-jsonrpc/src/nonnet.rs:348`
    * `crates/op-jsonrpc/src/nonnet.rs:371`
*   **Problem:** String generation using `format!` and ad-hoc nested structures under `json!` violates the discipline of schema-as-code. This increases serialization error rates, fragments error contract definitions, and hinders automated verification against OSCAL/Protobuf schemas.
*   **Remedy:**
    *   Define formal error and response contracts using the project's Protocol Buffers or versioned JSON Schema targets.
    *   Implement structural error serialization via strongly-typed structures generated directly from schema source definitions:
        ```rust
        // Example schema-backed structure representation
        #[derive(serde::Serialize)]
        pub struct JsonRpcErrorPayload {
            pub code: i32,
            pub message: std::borrow::Cow<'static, str>,
            pub details: Option<serde_json::Value>,
        }
        ```

#### 2. Resolve Blocking Filesystem Calls in Async Contexts
*   **Affected Files:** 
    * `crates/op-jsonrpc/src/nonnet.rs:239`
    * `crates/op-jsonrpc/src/server.rs:132`
*   **Problem:** `path.exists()` is a synchronous system call. Invoking this directly on an async executor thread blocks the entire reactor worker thread, degrading performance and throughput.
*   **Remedy:**
    *   Avoid calling `path.exists()` before `tokio::fs::remove_file`. Directly call `tokio::fs::remove_file` and ignore or gracefully handle any resulting `std::io::ErrorKind::NotFound` errors.
    *   Alternatively, use `tokio::fs::metadata(path).await.is_ok()` to check file existence asynchronously:
        ```rust
        // Correct asynchronous execution pattern
        if tokio::fs::metadata(&path).await.is_ok() {
            let _ = tokio::fs::remove_file(&path).await;
        }
        ```

#### 3. Eradicate Unsafe unwraps in Production Parsing Logic
*   **Affected Files:** 
    * `crates/op-jsonrpc/src/ovsdb.rs:523`
*   **Problem:** Calling `.unwrap()` on values parsed from external network messages (such as OVSDB responses) is a vector for Denial of Service (DoS) attacks. If an external service returns an unexpected structure or null string value, the production thread panics.
*   **Remedy:**
    *   Safely resolve options using `ok_or` or conditional `if let` blocks, and map errors cleanly to return RPC failures instead of crashing:
        ```rust
        let uuid_str = uuid_array[1]
            .as_str()
            .ok_or_else(|| Error::new(error_codes::PARSE_ERROR, "Invalid UUID format"))?;
        return Ok(uuid_str.to_string());
        ```