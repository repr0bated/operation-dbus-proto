# Production Security and Quality Audit: `op-tools`

This document provides a production security and quality audit of the `op-tools` crate based on the provided source code files. 

---

## 1. Overview & Module Map

### Total `.rs` Files Audited
- `crates/op-tools/src/lib.rs` (Primary library entry point)
- `crates/op-tools/src/builtin_old.rs` (Legacy tool definitions)
- `crates/op-tools/src/dynamic_tool.rs` (D-Bus projection wrapper)
- `crates/op-tools/src/executor.rs` (Execution concurrency and timeout management)
- `crates/op-tools/src/mcptools.rs` (Model Context Protocol client integration)
- `crates/op-tools/src/orchestration_plugin.rs` (Audit logging and activity tracking)
- `crates/op-tools/src/registry.rs` (Tool registration and metadata storage)
- `crates/op-tools/src/router.rs` (Axum HTTP handler and route mapping)
- `crates/op-tools/src/security.rs` (Access level enforcement and security validator)
- `crates/op-tools/src/tool.rs` (Core `Tool` trait and implementations)
- `crates/op-tools/src/validation.rs` (Schema and input validation)
- `crates/op-tools/src/validation_tests.rs` (Input validator unit tests)
- `crates/op-tools/src/bin/op-packagekit-install.rs` (Stand-alone PackageKit client)
- `crates/op-tools/src/builtin/agent_tool.rs` (D-Bus Agent interface)
- `crates/op-tools/src/builtin/anydesk.rs` (Remote desktop helpers)
- `crates/op-tools/src/builtin/code_search.rs` (Semantic code snippet retrieval)
- `crates/op-tools/src/builtin/dbus.rs` (Systemd D-Bus interface)
- `crates/op-tools/src/builtin/dbus_hybrid.rs` (Hybrid D-Bus generic caller)
- `crates/op-tools/src/builtin/dbus_introspection.rs` (D-Bus structural exploration)
- `crates/op-tools/src/builtin/dbus_search_tool.rs` (Full-text search on D-Bus interfaces)
- `crates/op-tools/src/builtin/dbus_tool.rs` (Dynamic D-Bus RPC tools)
- `crates/op-tools/src/builtin/dinit.rs` (Dinit service manager interface)
- `crates/op-tools/src/builtin/error_reporting_tool.rs` (Internal diagnostic logger)
- `crates/op-tools/src/builtin/file.rs` (Secure file read/write operations)
- `crates/op-tools/src/builtin/gcloud_tools.rs` (GCloud CLI introspector)
- `crates/op-tools/src/builtin/incus_tools.rs` (Incus container/VM operations)
- `crates/op-tools/src/builtin/lxc_tools.rs` (Proxmox LXC container manager)
- `crates/op-tools/src/builtin/ovs.rs` (Legacy OVS helper)
- `crates/op-tools/src/builtin/ovs_tools.rs` (OVSDB & Netlink controller)
- `crates/op-tools/src/builtin/ovsdb.rs` (OVSDB JSON-RPC socket client)
- `crates/op-tools/src/builtin/packagekit.rs` (PackageKit D-Bus client)
- `crates/op-tools/src/builtin/plugin.rs` (Dynamic configuration state manager)
- `crates/op-tools/src/builtin/plugin_state_tool.rs` (State plugin adapter)
- `crates/op-tools/src/builtin/procfs.rs` (Procfs/Sysfs read-write controller)
- `crates/op-tools/src/builtin/respond_tool.rs` (Orchestrator response interface)
- `crates/op-tools/src/builtin/response_tools.rs` (LLM response aggregator)
- `crates/op-tools/src/builtin/rtnetlink_tools.rs` (Native rtnetlink network manager)
- `crates/op-tools/src/builtin/self_tools.rs` (Self-modification & Git code repository tools)
- `crates/op-tools/src/builtin/shell_tool.rs` (Shell execution mechanics)
- `crates/op-tools/src/builtin/system.rs` (System metrics retriever)
- `crates/op-tools/src/builtin/indexer_tools.rs` (Semantic code search client)
- `crates/op-tools/src/builtin/mod.rs` (Eager tool registration mapping)
- `crates/op-tools/src/builtin/openflow_tools.rs` (OpenFlow rule injector)
- `crates/op-tools/src/builtin/plugin_projection.rs` (Plugin D-Bus projection readers)
- `crates/op-tools/src/discovery/mod.rs` (Source registry & cache manager)
- `crates/op-tools/src/discovery/projection_engine.rs` (Dynamic D-Bus introspection engine)
- `crates/op-tools/src/discovery/sources/agent.rs` (Agent directory metadata scanner)
- `crates/op-tools/src/discovery/sources/dbus.rs` (D-Bus service scanner)
- `crates/op-tools/src/discovery/sources/mod.rs` (Source index)
- `crates/op-tools/src/discovery/sources/plugin.rs` (State plugin scanner)

### Entry Points
- **Library**: `crates/op-tools/src/lib.rs` (Handles built-in registration, re-exports common types)
- **Binary**: `crates/op-tools/src/bin/op-packagekit-install.rs` (Standalone tool to install packages via PackageKit over system D-Bus)

---

## 2. Critical Security Vulnerabilities (Directly Exploitable)

### [CRITICAL] Unauthenticated Arbitrary Code Execution and Execution Validator Bypass
- **Files**: `crates/op-tools/src/router.rs:105-125`, `crates/op-tools/src/builtin/shell.rs:98`
- **Impact**: Arbitrary system command execution as `root` (or the executing system user) by any unauthenticated remote attacker.
- **Description**: 
  The Axum router exposes the `POST /api/tools/:name/execute` HTTP endpoint, which handles execution requests via `execute_tool_handler` in `router.rs`. 
  This endpoint does not perform any session verification, cryptographic token validation, or authentication. When executing `shell_execute`, the parameter `session_id` is supplied by the caller as a JSON parameter.
  Furthermore, `execute_tool_handler` completely bypasses `InputValidator::validate_input` from `crates/op-tools/src/validation.rs`. Instead, it directly calls `tool.execute(params)`.
  When executing `ShellExecuteTool`, the code queries the security validator via `get_security_validator()`. By default, the global `SecurityValidator` is initialized using `ToolSecurityProfile::admin()`, setting the default access level to `AccessLevel::Unrestricted`. In `check_command` (`crates/op-tools/src/security.rs:252`), if the access level is `Unrestricted`, the validation passes automatically.
  An attacker can send a raw HTTP request to execute the `shell_execute` tool with an arbitrary command (e.g., `rm -rf /` or starting a reverse shell), which will be executed instantly by `execute_command` via `Command::new("bash")`.

---

### [CRITICAL] Arbitrary File Write/Overwrite Outside Repository via Directory Traversal in `SelfWriteFileTool`
- **File**: `crates/op-tools/src/builtin/self_tools.rs:171-209`
- **Impact**: Arbitrary file creation or system-wide file overwriting (including `/etc/shadow`, ssh authorized keys, or crontabs) leading to host compromise.
- **Description**: 
  The `SelfWriteFileTool::execute` function attempts to restrict file writes to paths within the repository specified by the `OP_SELF_REPO_PATH` environment variable. 
  It implements a security check to ensure that the canonicalized path starts with the repository path:
  ```rust
  let parent = full_path.parent();
  if let Some(p) = parent {
      if p.exists() {
          let canonical_parent = p.canonicalize().unwrap_or(p.to_path_buf());
          if !canonical_parent.starts_with(&canonical_repo) {
              return Err(anyhow::anyhow!("Path '{}' would escape the self-repository. Access denied.", path));
          }
      } else if !create_dirs {
          return Err(anyhow::anyhow!("Parent directory does not exist: {:?}", p));
      }
  }
  ```
  If the parent directory of the targeted path does *not* exist, `p.exists()` returns `false`. As long as `create_dirs` is `true` (which is the default), the code skips the `starts_with` validation check entirely and reaches the directory creation and write phase:
  ```rust
  if create_dirs {
      if let Some(parent) = full_path.parent() {
          tokio::fs::create_dir_all(parent).await?;
      }
  }
  tokio::fs::write(&full_path, content).await?;
  ```
  An attacker can bypass the path validation simply by targeting a non-existent parent directory (e.g., `../../../../etc/cron.d/malicious_cron`). The tool will recursively create the directory path outside of the repository and write the arbitrary payload with no restrictions.

---

### [CRITICAL] Unchecked Directory Traversal in `/proc` File Reader Tool
- **File**: `crates/op-tools/src/builtin/procfs.rs:16-21`, `crates/op-tools/src/builtin/procfs.rs:158-181`
- **Impact**: Arbitrary file read of any system file outside of `/proc` (including private keys, credentials, and configuration files).
- **Description**: 
  The `ProcFsReadTool` reads files relative to `/proc`. It uses `validate_relative_path` to prevent path traversal:
  ```rust
  fn validate_relative_path(path: &str) -> anyhow::Result<()> {
      if path.is_empty() || path.starts_with('/') || path.contains("..") || path.contains('\\') {
          return Err(anyhow::anyhow!("Invalid path"));
      }
      Ok(())
  }
  ```
  However, this validation is insufficient because it does not resolve symbolic links. On Linux, `/proc/self/root` is a symbolic link that points to the system root `/`.
  If an attacker requests the path `self/root/etc/shadow`:
  1. The string does not start with `/`.
  2. The string does not contain `..` or `\\`.
  `validate_relative_path` accepts this path as valid. The tool then constructs the full path using `make_full_path("/proc", "self/root/etc/shadow")`, resulting in `/proc/self/root/etc/shadow`.
  When `tokio::fs::read_to_string` is called on this path, Linux resolves the `self/root` symlink to `/`, causing the system to read `/etc/shadow` and bypass the directory restriction entirely.

---

### [CRITICAL] Insecure File-Write Symlink Exploitation in `SecureFileTool`
- **Files**: `crates/op-tools/src/builtin/file.rs:184-250`, `crates/op-tools/src/security.rs:356-382`
- **Impact**: Arbitrary file creation or modification on the host system, bypassing restricted access level policies.
- **Description**: 
  When a session has the `AccessLevel::Restricted` profile, file writes are restricted to paths starting with `/tmp` (enforced in `validate_write_path`, `security.rs:377`). 
  However, the security validator only performs a string-level `starts_with` check on the input path string and does not canonicalize the destination before writing. 
  An attacker can create a symbolic link in `/tmp` pointing to `/etc/shadow` (or any other sensitive target) and then execute a `file_write` operation with the path `/tmp/exploit_link`. 
  Because `/tmp/exploit_link` starts with `/tmp`, the validation passes. `tokio::fs::write` resolves the symlink, writing the attacker-controlled payload directly into the protected file.

---

## 3. Adherence to Schema-as-Code Discipline

This codebase **violates** the schema-as-code discipline throughout the tool registration and validation layers:

| Crate / File | Line Range | Ad-Hoc Implementation Details |
| :--- | :--- | :--- |
| `crates/op-tools/src/builtin_old.rs` | 24-35 | Inline schema definition using `simd_json::json!` for `EchoTool` inputs. |
| `crates/op-tools/src/builtin_old.rs` | 134-150 | Ad-hoc inline JSON structure for `ShellTool` inputs. |
| `crates/op-tools/src/builtin/anydesk.rs` | 51-54 | Inline empty object schema definition for `AnyDeskGetIdTool`. |
| `crates/op-tools/src/builtin/dbus.rs` | 32-48 | Hardcoded inline JSON definition for `DbusSystemdRestartTool`. |
| `crates/op-tools/src/builtin/file.rs` | 100-164 | Hardcoded schema validation constraints for `SecureFileTool` using raw JSON strings and maps. |
| `crates/op-tools/src/builtin/rtnetlink_tools.rs` | 28-37 | Ad-hoc JSON array constraints mapped inline for rtnetlink interfaces. |
| `crates/op-tools/src/builtin/shell_tool.rs` | 196-213 | Inline schema definition with manually constructed types for `ReadFileTool`. |

### Architectural Risks
- **Data Contract Fragility**: Modifying the signature or types of a system tool requires editing hardcoded `simd_json::json!` structures across dozens of files. This leads to drift between client definitions and execution contracts.
- **Lack of Centralized Serialization/Deserialization**: Instead of using unified, version-controlled Protocol Buffers or structured OSCAL schemas, variables are extracted manually (e.g., `.get("args").and_then(|v| v.as_str())`), resulting in runtime parsing errors and high maintenance overhead.

---

## 4. Quality & Design Findings

### [HIGH] Global Security Validator Bypass and Validation-State Incoherence
- **File**: `crates/op-tools/src/validation.rs:188-218`
- **Description**: 
  The `InputValidator::validate_input` function determines whether a session is trusted by matching its ID against `config.trusted_sessions` (which defaults to `"chatbot"`, `"orchestrator"`, and `"system"`). 
  If a session is trusted, any schema validation, string sanitization, and security validations (e.g., shell injection patterns, directory constraints) that fail are logged as warnings but **do not block execution**:
  ```rust
  if let Err(e) = self.validate_schema(tool_name, &sanitized_input, schema).await {
      if is_trusted && !self.config.strict_validation {
          warn!(... "Schema validation bypassed for trusted session");
      } else {
          validation_errors.push(format!("Schema validation failed: {}", e));
      }
  }
  ```
  If `is_trusted` is `true`, `ValidatedInput::should_proceed` returns `true` even if the payload is dangerous. 
  Since the LLM or chat orchestrator runs within the `chatbot` session, it has unrestricted access to execute raw commands or write files outside the repository. 
  If the LLM suffers from prompt injection or hallucinates a path/command, the input validator will log a warning and let the dangerous input proceed directly to execution.

---

### [MEDIUM] Hardcoded User Directory Paths and Fragile System Dependencies
- **Files**: `crates/op-tools/src/builtin/anydesk.rs:592`, `crates/op-tools/src/discovery/sources/agent.rs:25`
- **Description**: 
  The codebase contains several hardcoded host-level home directories belonging to a specific user (`jeremy`):
  - `anydesk.rs`: `/home/jeremy/.anydesk/anydesk.conf` and `/home/jeremy/.anydesk/user.conf` are scanned to retrieve the AnyDesk connection ID.
  - `agent.rs`: `agents_dir` falls back to `/home/jeremy/agents` if `dirs::home_dir()` is unavailable.
- **Impact**: 
  If the application is deployed on a system where the user `jeremy` does not exist or has a different home directory, the tool discovery and AnyDesk ID retrieval operations will fail silently. Configurable environment variables or standard sysfs directories must be used instead.

---

### [MEDIUM] Resource Exhaustion via Connection Churn in Dynamic D-Bus Tools
- **File**: `crates/op-tools/src/dynamic_tool.rs:114-124`
- **Description**: 
  The `DynamicDbusTool::execute` function connects to the system D-Bus on every invocation:
  ```rust
  let connection = zbus::Connection::system()
      .await
      .map_err(|e| anyhow::anyhow!("Failed to connect to system bus: {}", e))?;
  ```
- **Impact**: 
  Under high concurrency (e.g., executing multiple tool pipelines concurrently), spawning a new zbus connection for every single RPC call will exhaust file descriptors, trigger systemd limits on bus clients, and degrade execution times. 
  The engine should reuse a shared connection from a centralized registry (similar to the `AgentConnectionRegistry` pattern).

---

### [MEDIUM] Redundant Re-implementation of Base64 Encoding
- **File**: `crates/op-tools/src/builtin_old.rs:293-324`
- **Description**: 
  The codebase implements a manual Base64 encoder from scratch within `builtin_old.rs` under `mod base64` with the comment `// Simple base64 encoding (to avoid additional dependency)`.
- **Impact**: 
  The workspace already has a dependency on the robust, optimized `base64` crate (specified in the root `Cargo.toml`). Re-implementing cryptographic or formatting standards manually is an anti-pattern that introduces performance overhead and potential edge-case validation issues.

---

### [LOW] Insecure Shell Escape Fallbacks and System Leakage
- **File**: `crates/op-tools/src/builtin/rtnetlink_tools.rs:65-103`
- **Description**: 
  If the native `rtnetlink` interface fails to retrieve network details, the code falls back to spawning a shell process to run `ip -j addr show` and parses the output. 
  It then returns the native error string (`e.to_string()`) inside the JSON response back to the client.
- **Impact**: 
  The fallback relies on the `ip` binary being present in the host's path. Furthermore, leaking low-level system error strings directly to unauthenticated consumers exposes internal operating system parameters and helps attackers fingerprint host systems.

---

## 5. Security Action Plan

To secure this system for a production deployment, the following changes must be implemented immediately:

1. **Implement HTTP Authentication Middleware**: Integrate an authentication and authorization layer (such as Bearer tokens or mTLS) inside `crates/op-tools/src/router.rs` to protect `/api/tools/:name/execute`.
2. **Mandate Input Validation**: Update `execute_tool_handler` to instantiate the `InputValidator` and reject requests that fail validation before invoking the targeted tool.
3. **Fix Directory Traversal in `SelfWriteFileTool`**: Always canonicalize the target file path and verify that it starts with `canonical_repo`, regardless of whether the parent directory exists:
   ```rust
   let full_path = repo_path.join(clean_path);
   let canonical_target = full_path.canonicalize().unwrap_or(full_path.clone());
   if !canonical_target.starts_with(&canonical_repo) {
       return Err(anyhow::anyhow!("Access denied: path escapes repository"));
   }
   ```
4. **Prevent Symlink Dereferencing in Restricted File Tools**: Use `tokio::fs::symlink_metadata` to check if a file path is a symbolic link before writing to it under restricted access level profiles.
5. **Secure `/proc` Relative Reader**: Use canonicalization onconstructed paths before invoking file reads, and ensure the resolved path does not leave `/proc`. Do not allow structural path segments like `self/root` to be passed as relative segments.