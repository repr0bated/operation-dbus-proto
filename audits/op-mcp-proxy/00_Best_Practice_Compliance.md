| Pattern | File:Line | Crate Approach | Corpus Best Practice | Gap | Rating |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `command_new` | `crates/op-mcp-proxy/src/session.rs:95` | Invokes bare `wg` utility via `Command::new` | Use fully qualified absolute executable paths or direct library-level system APIs. | Uses system `PATH` resolution for binary execution; risks local path hijacking. | Minor Gap |
| `command_new` | `crates/op-mcp-proxy/src/session.rs:118` | Invokes bare `wg` command to retrieve network peer allowed-IP configuration | Use netlink interface programmatic bindings instead of shelling out to binary. | Reliance on external tool parsing for routing info instead of native programmatic socket configs. | Minor Gap |
| `format_json_manual` | `crates/op-mcp-proxy/src/session.rs:110` | Formats ad-hoc identifier string `local:{hostname}` | Keep system and process IDs structured within typed data schemas. | Violation of schema-as-code; uses raw dynamic string concatenation. | Minor Gap |
| `std_fs_in_async` | `crates/op-mcp-proxy/src/session.rs:44` | Calls synchronous `std::fs::create_dir_all` within async context | Perform filesystem mutations using asynchronous `tokio::fs` or offload via `spawn_blocking`. | Blocks the async runtime executor threads, reducing execution concurrency. | Major Gap |
| `command_new` | `crates/op-mcp-proxy/src/gcloud_auth.rs:325` | Invokes raw shell command `gcloud` with arbitrary dynamic scopes | Use safe execution sandboxing or official client SDK libraries (e.g. `google-cloud-rust`). | Dynamic argument mapping without formal validation interfaces. | Minor Gap |
| `command_new` | `crates/op-mcp-proxy/src/gcloud_auth.rs:340` | Spawns `gcloud` subprocess without absolute pathing | Resolve path explicitly or restrict Execution Context. | Spawns processes from arbitrary user path environments. | Minor Gap |
| `format_json_manual` | `crates/op-mcp-proxy/src/gcloud_auth.rs:323` | Appends scope values with raw `format!` args builder | Map arguments through structured serializers. | Minor dynamic string mutation. | Compliant |
| `std_fs_in_async` | `crates/op-mcp-proxy/src/gcloud_auth.rs:144` | Blocks async thread to read authentication token using `std::fs::read_to_string` | Read configuration assets asynchronously. | Blocks tokio executor threads when parsing credential assets. | Major Gap |
| `std_fs_in_async` | `crates/op-mcp-proxy/src/gcloud_auth.rs:232` | Blocks async executor running directory traversal `std::fs::read_dir` | Utilize non-blocking, async-native directory iteration. | Blocking FS operations degrade concurrency during asset discovery. | Major Gap |
| `std_fs_in_async` | `crates/op-mcp-proxy/src/gcloud_auth.rs:253` | Synchronously reads credentials mapping JSON files | Read structured files with async IO and deserialize. | Thread-blocking operation in critical async path. | Major Gap |
| `std_fs_in_async` | `crates/op-mcp-proxy/src/gcloud_auth.rs:281` | Reads credentials using `std::fs::read_to_string` inside async execution | Rely on non-blocking task runners or tokio fs modules. | Blocked thread during runtime authentication checks. | Major Gap |
| `unsafe_block` | `crates/op-mcp-proxy/src/sled.rs:37` | Unsafe `MmapOptions` configuration mapping file size | Provide detailed safety invariant explanations for memory mapping. | Lacks clear documentation on concurrent modification safety. | Minor Gap |
| `format_json_manual` | `crates/op-mcp-proxy/src/sled.rs:52` | Formats hexadecimal string IDs on-the-fly | Normal tracing format structure. | Standard string output creation. | Compliant |
| `format_json_manual` | `crates/op-mcp-proxy/src/direct_llm.rs:168` | Creates dynamic error strings manually for system-level errors | Define structured error envelopes mapped to standardized payload types. | Ad-hoc message building instead of using unified schema errors. | Minor Gap |
| `format_json_manual` | `crates/op-mcp-proxy/src/direct_llm.rs:206` | Concatenates manual error messages into generic strings | Rely on strict error enum mapping with structured JSON serialization. | Ad-hoc serialization instead of structured error payloads. | Minor Gap |
| `unsafe_block` | `crates/op-mcp-proxy/src/cloudaicompanion.rs:585` | Unsafe parsing using mutable reference strings with `simd_json` | Add `# Safety` documentation block detailing mutable buffer guarantees. | Undocumented unsafe block for performance-driven parsing. | Minor Gap |
| `unsafe_block` | `crates/op-mcp-proxy/src/cloudaicompanion.rs:607` | Unsafe mutable parsing without static analysis safety bounds | Document mutable state bounds explicitly. | Undocumented unsafe simd-json usage. | Minor Gap |
| `unsafe_block` | `crates/op-mcp-proxy/src/cloudaicompanion.rs:685` | Combines undocumented `unsafe simd_json::from_str` with manual JSON field extraction | Use schema-as-code discipline with strongly-typed structures (Serde) instead of untyped lookups. | Dynamic, ad-hoc payload navigation on an undocumented unsafe interface. | Major Gap |
| `unsafe_block` | `crates/op-mcp-proxy/src/cloudaicompanion.rs:731` | Manual JSON node retrieval on undocumented unsafe deserialized structures | Parse structured schema payloads explicitly into Rust types. | Unsafe schema validation failure; dynamic path queries bypass type safety. | Major Gap |
| `simd_json_from_str` | `crates/op-mcp-proxy/src/cloudaicompanion.rs:585` | Parses configuration into `OwnedValue` | Map incoming configurations to statically structured structs. | Untyped configuration mapping. | Minor Gap |
| `simd_json_from_str` | `crates/op-mcp-proxy/src/cloudaicompanion.rs:607` | Parses JSON files directly to untyped dynamic buffers | Use explicit types rather than generic JSON properties. | Bypasses structured schemas. | Minor Gap |
| `simd_json_from_str` | `crates/op-mcp-proxy/src/cloudaicompanion.rs:685` | Dynamic extraction of quota project IDs from parsed JSON | Parse structurally validated schema payloads. | Manual mapping bypasses type safety validation. | Major Gap |
| `simd_json_from_str` | `crates/op-mcp-proxy/src/cloudaicompanion.rs:731` | Navigates client IDs and secrets dynamically over unsafe parsed objects | Ensure sensitive fields are extracted strictly via typed structs with Serde. | Violates structural schema declarations for confidential fields. | Major Gap |
| `unwrap_expect` | `crates/op-mcp-proxy/src/cloudaicompanion.rs:135` | Uses `.expect("http client")` inside initial builder instantiation | Fail fast on initial configuration validation failure. | Expected builder-level panic for structural setups. | Compliant |
| `simd_json_from_str` | `crates/op-mcp-proxy/src/main.rs:117` | Uses `unsafe` JSON parsing directly on line inputs, navigating fields dynamically | Implement versioned Protocol Buffers or strongly typed JSON-RPC structs. | Violates Schema-as-Code; utilizes dynamic JSON navigation via string literals. | Major Gap |

---

### Actionable Recommendations for Major Gaps

#### 1. Replace Blocking Synchronous Filesystem Access inside Async Runtimes
*   **Gap Location**: 
    *   `crates/op-mcp-proxy/src/session.rs:44` (`std::fs::create_dir_all`)
    *   `crates/op-mcp-proxy/src/gcloud_auth.rs:144` (`std::fs::read_to_string`)
    *   `crates/op-mcp-proxy/src/gcloud_auth.rs:232` (`std::fs::read_dir`)
    *   `crates/op-mcp-proxy/src/gcloud_auth.rs:253` (`std::fs::read_to_string`)
    *   `crates/op-mcp-proxy/src/gcloud_auth.rs:281` (`std::fs::read_to_string`)
*   **Remediation**: Use `tokio::fs` alternatives or wrap standard library synchronous calls inside `tokio::task::spawn_blocking` to prevent blocking the cooperative async executor thread pool.
    *   *Example remediation for `crates/op-mcp-proxy/src/session.rs:44`*:
        ```rust
        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        ```
    *   *Example remediation for `crates/op-mcp-proxy/src/gcloud_auth.rs:144`*:
        ```rust
        let content = tokio::fs::read_to_string(path).await.ok()?;
        ```

#### 2. Enforce Schema-as-Code Discipline and Remediate Undocumented Unsafe JSON Deserialization
*   **Gap Location**: 
    *   `crates/op-mcp-proxy/src/cloudaicompanion.rs:685`
    *   `crates/op-mcp-proxy/src/cloudaicompanion.rs:731`
    *   `crates/op-mcp-proxy/src/main.rs:117`
*   **Remediation**: Remove manual JSON indexing (`val.get("quota_project_id")`, `req["method"]`) and replace it with typed data contracts deserialized via `serde::Deserialize`. If `simd_json` performance is required, still parse into typed structures rather than loose `OwnedValue` types. Every `unsafe` block MUST be documented with a `# Safety` block explaining memory invariants.
    *   *Example remediation for `crates/op-mcp-proxy/src/main.rs:117`*:
        ```rust
        #[derive(serde::Deserialize)]
        struct JsonRpcRequest {
            jsonrpc: String,
            method: String,
            id: serde_json::Value,
        }

        // Safety: `line` is mutated in place during parsing. The memory remains valid as 
        // `line` is pinned to this function scope and is not referenced post-deallocation.
        let req: JsonRpcRequest = unsafe { simd_json::from_str(&mut line) }?;
        let method = req.method;
        ```