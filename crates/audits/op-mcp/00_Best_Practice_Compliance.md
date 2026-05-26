| Pattern | File:Line | Crate Approach | Corpus Best Practice | Gap | Rating |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `unsafe_block` | `crates/op-mcp/src/agents_main.rs:781` | Parses mutable stdin strings with `unsafe { simd_json::from_str }`. | Avoid `unsafe` parsing on standard types; use safe deserialization crates like `serde_json`. | Undefined Behavior (UB) risk: `simd_json::from_str` requires the input buffer to be padded with `simd_json::PADDING` bytes, which standard stdin lines do not guarantee. | Critical Gap |
| `simd_json_from_str` | `crates/op-mcp/src/agents_main.rs:781` | Uses `simd_json::from_str` directly in unsafe blocks. | Prioritize memory-safe parsing libraries unless extreme performance is measured and padding invariants are met. | Memory-safety vulnerability due to missing padding guarantees. | Critical Gap |
| `format_json_manual` | `crates/op-mcp/src/agents_main.rs:467` | Employs ad-hoc `json!` construction with inline formatting. | Define structured, versioned schemas (such as Protobuf/OSCAL) rather than manual JSON maps. | Non-compliance with schema-as-code discipline; makes contracts prone to schema drift. | Major Gap |
| `format_json_manual` | `crates/op-mcp/src/agents_main.rs:468` | Direct formatted inline strings for JSON values. | Model protocol states as versioned structs. | High risk of serialization errors and schema drift. | Major Gap |
| `format_json_manual` | `crates/op-mcp/src/agents_main.rs:554` | Manual formatting of code analysis response fields. | Define structured data transfer objects. | Ad-hoc serialization bypasses formal quality contracts. | Major Gap |
| `format_json_manual` | `crates/op-mcp/src/agents_main.rs:571` | Ad-hoc string formatting for expert messages. | Use versioned schemas to represent tool outputs. | Ad-hoc serialization bypasses formal quality contracts. | Major Gap |
| `format_json_manual` | `crates/op-mcp/src/agents_main.rs:592` | Dynamic formatting of troubleshooter issue outputs. | Use strictly versioned schemas. | Ad-hoc serialization bypasses formal quality contracts. | Major Gap |
| `unwrap_expect` | `crates/op-mcp/src/agents_main.rs:787` | Calls `.unwrap()` on JSON serialization of error structures. | Bubble up errors gracefully using `?`. | Panics on serialization failure will crash the entire agent process. | Minor Gap |
| `unwrap_expect` | `crates/op-mcp/src/agents_main.rs:799` | Uses `.unwrap()` when writing responses back to standard output. | Propagate errors gracefully using `?` or `match`. | High panic risk if stdout stream is disrupted. | Minor Gap |
| `unsafe_block` | `crates/op-mcp/src/agents_server.rs:282` | Clones a `String` and parses with `unsafe { simd_json::from_str }`. | Avoid unsafe in-place mutation of string slices. | Unpadded mutable cloned strings violate `simd_json` alignment/padding requirements, causing UB. | Critical Gap |
| `simd_json_from_str` | `crates/op-mcp/src/agents_server.rs:282` | Unsafe parsing of output string variables. | Use standard safe parsing (`serde_json`). | Out-of-bounds reads during SIMD processing. | Critical Gap |
| `unwrap_expect` | `crates/op-mcp/src/compact.rs:204` | Unwraps JSON serialized value builder. | Use fallible construction or handle formatting errors. | Panics on serialization failure. | Minor Gap |
| `unwrap_expect` | `crates/op-mcp/src/compact.rs:256` | Unwraps serialized JSON array outputs. | Propagate errors via the call stack. | Panics on serialization failure. | Minor Gap |
| `unwrap_expect` | `crates/op-mcp/src/compact.rs:301` | Unwraps schema serialization. | Use safe error handling pattern. | Panics on serialization failure. | Minor Gap |
| `unsafe_block` | `crates/op-mcp/src/external_client.rs:363` | Parses `response_line` using `unsafe { simd_json::from_str }`. | Use safe deserializers for dynamic external inputs. | Out-of-bounds reads if external daemon outputs aren't padded. | Critical Gap |
| `unsafe_block` | `crates/op-mcp/src/external_client.rs:427` | Parses loaded configuration files with unsafe `simd_json`. | Use safe loaders for untrusted file content. | Lack of padding on file-to-string buffer triggers UB. | Critical Gap |
| `simd_json_from_str` | `crates/op-mcp/src/external_client.rs:363` | In-place parsing of raw lines without padding. | Use safe `serde_json`. | Security vulnerability via out-of-bounds memory reading. | Critical Gap |
| `simd_json_from_str` | `crates/op-mcp/src/external_client.rs:427` | Parsing of file content with unsafe `simd_json`. | Standardize on safe serializing structures. | Memory corruption on malformed config file. | Critical Gap |
| `command_new` | `crates/op-mcp/src/external_client.rs:101` | Spawns binaries from arbitrary configuration commands. | Use absolute path resolution and sanitize inputs. | Execution of arbitrary unvalidated command binaries. | Major Gap |
| `std_fs_in_async` | `crates/op-mcp/src/external_client.rs:421` | Uses `tokio::fs::read_to_string`. | Run non-blocking filesystem calls in async contexts. | Compliant. | Compliant |
| `unsafe_block` | `crates/op-mcp/src/protocol.rs:179` | Unsafe deserialization in test assertions. | Keep tests safe and predictable. | Potential UB during test suite executions. | Minor Gap |
| `simd_json_from_str` | `crates/op-mcp/src/protocol.rs:179` | SIMD deserialization in test environment. | Use standard safe parsers. | Test suite memory safety violation. | Minor Gap |
| `command_new` | `crates/op-mcp/src/http_server.rs:364` | Spawns commands directly from HTTP configuration. | Sanitize path arguments and enforce sandboxing. | Privilege escalation / Command Injection via HTTP server configuration. | Major Gap |
| `std_fs_in_async` | `crates/op-mcp/src/tools/filesystem.rs:44` | Exposes `tokio::fs::read_to_string` directly as an MCP tool. | Confine paths to a validated sandbox directory. | Critical Directory Traversal: Exposes arbitrary host file disclosure to the client (LLM). | Critical Gap |
| `std_fs_in_async` | `crates/op-mcp/src/tools/filesystem.rs:82` | Exposes `tokio::fs::write` directly as an MCP tool. | Prevent arbitrary writing by validating paths. | Critical Path Traversal / Arbitrary File Write: Allows arbitrary host compromise. | Critical Gap |
| `std_fs_in_async` | `crates/op-mcp/src/tools/filesystem.rs:113` | Uses `tokio::fs::read_dir` directly with raw user path. | Validate and confine relative paths. | Information disclosure via unsanitized host dir scanning. | Critical Gap |
| `command_new` | `crates/op-mcp/src/tools/shell.rs:73` | Spawns arbitrary shells with user-controlled input. | Eliminate generic shell execution in automated agents. | Arbitrary Remote Code Execution (RCE) on the host system. | Critical Gap |
| `std_fs_in_async` | `crates/op-mcp/src/tools/system.rs:29` | Uses async `tokio::fs::read_dir`. | Prevent thread-blocking IO. | Compliant. | Compliant |
| `command_new` | `crates/op-mcp/src/tools/ovs.rs:40` | Spawns `ovs-vsctl` using relative system lookup. | Execute commands with absolute paths (`/usr/bin/...`). | PATH hijacking vulnerabilities on host environment. | Major Gap |
| `command_new` | `crates/op-mcp/src/tools/ovs.rs:49` | Spawns `ovs-ofctl` using relative PATH lookups. | Enforce absolute path binaries. | PATH hijacking vulnerability. | Major Gap |

---

### Actionable Recommendations

#### 1. Eliminate Undefined Behavior from `simd_json::from_str`
* **Applicable Files**: `crates/op-mcp/src/agents_main.rs:781`, `crates/op-mcp/src/agents_server.rs:282`, `crates/op-mcp/src/external_client.rs:363`, `crates/op-mcp/src/external_client.rs:427`
* **Vulnerability Analysis**: `simd_json::from_str` requires the target string to be mutable and padded with `simd_json::PADDING` bytes at the end of the buffer. Standard `String` variables cloned or read line-by-line do not guarantee this padding, resulting in out-of-bounds memory reads and segmentation faults.
* **Resolution**: Replace the unsafe imports of `simd_json` with standard, safe `serde_json::from_str`. If SIMD-accelerated JSON deserialization is strictly necessary for your performance budget, allocate a padded `Vec<u8>` and use the safe API `simd_json::from_slice`:
  ```rust
  let mut padded_bytes = content.into_bytes();
  padded_bytes.resize(padded_bytes.len() + simd_json::PADDING, 0);
  let configs: Vec<ExternalMcpConfig> = simd_json::from_slice(&mut padded_bytes)
      .context("Failed to parse MCP config")?;
  ```

#### 2. Prevent Host Compromise via Path Traversal Sanitization
* **Applicable Files**: `crates/op-mcp/src/tools/filesystem.rs:44`, `82`, `113`
* **Vulnerability Analysis**: The filesystem tools directly consume raw paths from the MCP client (often an LLM) and execute operations on the host filesystem. This permits arbitrary read/write actions on any host file, including private SSH keys, configuration targets, and system credentials.
* **Resolution**: Canonicalize all target paths and enforce containment within a strictly defined workspace/sandbox directory. Reject any path resolving outside the designated root:
  ```rust
  use std::path::{Path, Component};

  pub fn safe_sandbox_resolve(base: &Path, user_path: &Path) -> Result<std::path::PathBuf, String> {
      let resolved = base.join(user_path);
      let canonical_resolved = std::fs::canonicalize(&resolved)
          .map_err(|e| format!("Invalid path: {}", e))?;
      let canonical_base = std::fs::canonicalize(base)
          .map_err(|e| format!("Invalid base path: {}", e))?;

      if canonical_resolved.starts_with(&canonical_base) {
          Ok(canonical_resolved)
      } else {
          Err("Path traversal attempt detected. Target is outside the sandbox root.".to_string())
      }
  }
  ```

#### 3. Eradicate Remote Code Execution (RCE) Vulnerability in Shell Tool
* **Applicable Files**: `crates/op-mcp/src/tools/shell.rs:73`
* **Vulnerability Analysis**: Exposing a tool that accepts arbitrary command strings and executes them through a shell interface allows untrusted/adversarial clients (or compromised LLMs) to run any arbitrary terminal command under the privileges of the MCP process.
* **Resolution**: Completely disable generic shell execution. Restrict execution patterns to a predefined, immutable whitelist of target executables with strictly validated, non-dynamic positional parameters.

#### 4. Establish Schema-as-Code Contracts
* **Applicable Files**: `crates/op-mcp/src/agents_main.rs:467`, `468`, `554`, `571`, `592`
* **Vulnerability Analysis**: Dynamic JSON construction using the `json!` macro with ad-hoc formatted fields violates schema-as-code principles. It makes the system fragile to protocol drift, parsing errors, and payload corruption.
* **Resolution**: Define strongly-typed structs decorated with `#[derive(Serialize, Deserialize)]` to represent your protocol models. Use versioned types or code-generated structures (like Protocol Buffers) to ensure stable data-contracts across the service boundaries.

#### 5. Eliminate PATH Hijacking via Absolute Command Paths
* **Applicable Files**: `crates/op-mcp/src/external_client.rs:101`, `crates/op-mcp/src/http_server.rs:364`, `crates/op-mcp/src/tools/ovs.rs:40`, `49`
* **Vulnerability Analysis**: Spawning system utilities like `ovs-vsctl` using relative lookups depends entirely on the caller's environment `PATH` variable, making the application vulnerable to binary hijacking if `PATH` is manipulated or contaminated.
* **Resolution**: Replace relative lookups with absolute filesystem paths (e.g., `/usr/bin/ovs-vsctl` or `/usr/sbin/ovs-ofctl`) and validate that configuration-defined binaries reside within permitted execution directories.