# Production Security and Quality Audit Report

This document presents a production security and quality audit of the `op-tools` crate. The audit focuses on **Schema-as-Code Compliance**, **OSCAL Compliance**, and **Directly Exploitable Vulnerabilities** identified within the source files.

---

## 1. Schema-as-Code Compliance

This codebase utilizes dynamic, untyped JSON structures (`simd_json::OwnedValue` or `serde_json::Value`) for core execution payloads, API endpoints, telemetry events, and configuration states. This represents a significant gap in the "schema-as-code" discipline, which mandates that all data contracts are defined as versioned, strongly typed Protocol Buffer schemas (`.proto`). 

### Schema-as-Code Analysis

| Item | Type | file:line | Has .proto? | Gap |
| :--- | :--- | :--- | :--- | :--- |
| **Tool Execution Payload** | Data Contract | `crates/op-tools/src/tool.rs:43` | No | Uses untyped `simd_json::OwnedValue` for both tool execution inputs and results, making type enforcement and interface evolution impossible to verify at compile time. |
| **Tool Definition Metadata** | Metadata Struct | `crates/op-tools/src/registry.rs:17` | No | Defines input schemas via dynamic JSON values instead of versioned Protocol Buffer schema descriptors. |
| **HTTP Execute Tool Endpoint** | API Contract | `crates/op-tools/src/router.rs:121` | No | The HTTP POST endpoint `/api/tools/:name/execute` accepts untyped JSON (`Json<Value>`) directly from the web boundary, leading to an unversioned, unsafe data contract. |
| **D-Bus Projected Method Tool** | Dynamic Schema | `crates/op-tools/src/builtin/dbus_hybrid.rs:136` | No | Dynamically reconstructs ad-hoc JSON schemas from raw D-Bus signature strings at runtime rather than relying on deterministic, pre-compiled schemas. |
| **Orchestration Telemetry Events** | Event Model | `crates/op-tools/src/orchestration_plugin.rs:46` | No | `ToolExecutedEvent` embeds arguments and metadata as arbitrary JSON `Value` objects, violating audit-trail schemas and complicating append-only snowball storage. |
| **Plugin Capabilities & States** | Config Schema | `crates/op-tools/src/builtin/plugin_state_tool.rs:102` | No | Uses ad-hoc dynamic JSON maps for state queries, diffs, and actions across state plugins without a centralized, versioned schema repository. |

---

## 2. OSCAL Compliance

The codebase implements critical system boundary controls—such as authorization checking, path validation, shell execution limits, and auditing—directly within the procedural Rust code. There are no machine-readable OSCAL (Open Security Controls Assessment Language) mappings or component definitions to document these security capabilities.

### OSCAL Coverage Analysis

| Control Area (NIST SP 800-53) | Implemented at file:line | OSCAL Artifact | Gap |
| :--- | :--- | :--- | :--- |
| **AC-3 / AC-6 (Least Privilege / Access Enforcement)** | `crates/op-tools/src/security.rs:136` | `component-definition` | The definition of access profiles (`Unrestricted`, `Restricted`, `Custom`) is hardcoded in Rust. This capability is not exported as an OSCAL Component Definition, preventing automated compliance auditing of system privileges. |
| **SI-10 (Information Input Validation)** | `crates/op-tools/src/validation.rs:19` | `component-definition` | Forbidden character filters, whitelisted commands, and system path rules are hardcoded, bypassing machine-readable compliance profiles (such as System Security Plans). |
| **AU-2 / AU-12 (Audit Event Generation / Audit Record Association)** | `crates/op-tools/src/orchestration_plugin.rs:163` | `system-security-plan` | The telemetry/event registry that dispatches tool execution events and LLM decisions lacks alignment with OSCAL Assessment Plans (SAPs) or System Security Plans (SSPs). |
| **CM-7 (Least Functionality / Remote Desktop Control)** | `crates/op-tools/src/builtin/anydesk.rs:45` | `component-definition` | Tools for starting, stopping, and checking AnyDesk remote control sessions have no corresponding OSCAL control implementations documenting remote access authorization boundaries. |
| **AC-17 (Remote Access / D-Bus Projection Boundary)** | `crates/op-tools/src/discovery/projection_engine.rs:88` | `component-definition` | Dynamic projection of local D-Bus interfaces to remote-accessible LLM tools has no machine-readable documentation describing interface-level system boundaries. |

---

## 3. High and Critical Vulnerabilities

### [CRITICAL] Path Traversal & Arbitrary File Write Bypass in `self_tools.rs`
* **Location:** `crates/op-tools/src/builtin/self_tools.rs:46` (`validate_self_path`) & `crates/op-tools/src/builtin/self_tools.rs:197` (`SelfWriteFileTool::execute`)
* **Impact:** **Remote Code Execution (RCE) / System Compromise**
* **Mechanism:** 
  The codebase exposes tools designed to let an LLM modify its own source code repository. To prevent escaping the repository root, `validate_self_path` performs the following validation:
  ```rust
  let canonical = full_path.canonicalize().unwrap_or_else(|_| full_path.clone());
  if !canonical.starts_with(&repo_path) { ... }
  ```
  However, `Path::canonicalize()` fails with an `Err` if the target file or any parent directory in the path does not exist. If the path contains directory traversal sequences (`..`) and points to a non-existent file, `canonicalize()` fails and falls back to `full_path.clone()`. Because Rust's `Path::starts_with()` evaluates paths **lexically** without resolving symbolic links or `..` segments, it falsely returns `true` for a path like `/home/user/repo/crates/op-tools/src/../../../../../../etc/cron.d/malicious`.
  
  Furthermore, inside `SelfWriteFileTool::execute`, when `create_dirs` is `true` (the default) and the parent directory of the target file does not exist, the validation block is skipped completely:
  ```rust
  if p.exists() {
      // Validation occurs here
  } else if !create_dirs {
      return Err(...);
  }
  ```
  Because the validation is skipped when `p.exists()` is false, an attacker can specify a non-existent parent directory with directory traversal segments. The application will then call `tokio::fs::create_dir_all(parent)` (which resolves the `..` segments and creates the directory structure) and write arbitrary payload content anywhere on the filesystem.

---

### [CRITICAL] Command Injection Sandbox Escape in Restricted Shell Tools
* **Location:** `crates/op-tools/src/security.rs:271` (`check_command`) & `crates/op-tools/src/builtin_old.rs:133` (`ShellTool::validate`)
* **Impact:** **Privilege Escalation / Arbitrary Command Execution**
* **Mechanism:** 
  To restrict command execution for guest or untrusted sessions, the `security.rs` and `builtin_old.rs` modules parse the first word of the command to check it against an allowlist:
  ```rust
  let base_cmd = command
      .split_whitespace()
      .next()
      .ok_or_else(|| SecurityError::ValidationFailed("Empty command".to_string()))?;
  ```
  If `base_cmd` matches a whitelisted utility (such as `ls` or `cat`), the validation passes. However, the command string is executed by spawning a raw shell:
  ```rust
  let mut child = Command::new("bash")
      .arg("-c")
      .arg(command)
  ```
  Because the check only evaluates the first token and does not validate the rest of the string for shell metacharacters, an attacker can append command chain operators (`&&`, `;`, `|`, backticks, or `$()`) to bypass the filter entirely. For example, passing the command `ls -la && rm -rf /` yields `base_cmd = "ls"`, which is approved, resulting in the execution of the entire malicious command chain with system privileges.

---

### [HIGH] Bypassed Security Validation (Dead Code)
* **Location:** `crates/op-tools/src/validation.rs:136` (`InputValidator`)
* **Impact:** **Lack of Defense-in-Depth**
* **Mechanism:** 
  The codebase defines an `InputValidator` in `validation.rs` containing rules for filtering forbidden characters, restricting directory access, and scanning shell command patterns. However, this validator is **never instantiated or called** anywhere within the tool execution pipeline (`crates/op-tools/src/router.rs` or `crates/op-tools/src/executor.rs`).
  
  All client requests to `/api/tools/:name/execute` execute commands directly via `tool.execute(params)`, completely bypassing the validation and sanitization logic.

---

### [HIGH] Privilege Escalation via Headless X11 Display Control
* **Location:** `crates/op-tools/src/builtin/anydesk.rs:595` (`diagnose_x11_access_issues`)
* **Impact:** **Arbitrary File Read/Write and System Access**
* **Mechanism:**
  The diagnostic tools for AnyDesk check and provide "fixes" for X11 display issues. One of the automatic fix commands executes raw shell strings using root permissions:
  ```rust
  fix_commands.push("sudo cp /home/jeremy/.Xauthority /root/.Xauthority && sudo chown root:root /root/.Xauthority && sudo chmod 600 /root/.Xauthority".to_string());
  ```
  This command references hardcoded user paths (`/home/jeremy`). If an attacker can manipulate or symlink the `.Xauthority` file in the user's directory, they can force the privileged process to overwrite files or gain root access to the X11 server authority structure, enabling screen-scraping and input injection across sessions.

---

## 4. Recommendations

### 1. Remediate `self_tools.rs` Path Traversal
* **Action:** Never rely on lexical checks (`starts_with`) for un-canonicalized paths. Modify `validate_self_path` to verify the parent directory’s canonical path, and explicitly reject any paths containing `..` or symbolic links before proceeding.
* **Refined Code:**
  ```rust
  fn validate_self_path(relative_path: &str) -> Result<PathBuf> {
      let repo_path = get_self_repo_path()
          .ok_or_else(|| anyhow::anyhow!("OP_SELF_REPO_PATH is not set"))?
          .canonicalize()?;
      
      if relative_path.contains("..") {
          return Err(anyhow::anyhow!("Directory traversal detected"));
      }
      
      let full_path = repo_path.join(relative_path.trim_start_matches('/'));
      
      // Ensure the parent directory is valid and nested within the repository
      if let Some(parent) = full_path.parent() {
          if parent.exists() {
              let canonical_parent = parent.canonicalize()?;
              if !canonical_parent.starts_with(&repo_path) {
                  return Err(anyhow::anyhow!("Path escapes repository root"));
              }
          } else {
              // Recurse parents to find the closest existing ancestor
              let mut ancestor = parent;
              while !ancestor.exists() {
                  if let Some(next_ancestor) = ancestor.parent() {
                      ancestor = next_ancestor;
                  } else {
                      break;
                  }
              }
              if !ancestor.canonicalize()?.starts_with(&repo_path) {
                  return Err(anyhow::anyhow!("Path escapes repository root"));
              }
          }
      }
      Ok(full_path)
  }
  ```

### 2. Secure Restricted-Mode Command Execution
* **Action:** Never execute commands using a shell launcher (`bash -c`) when executing allowlisted utilities. Instead, parse commands into a structured vector of arguments, and execute the executable directly using `tokio::process::Command::new(base_cmd).args(safe_args)`. This prevents shell metacharacters from being interpreted.

### 3. Integrate `InputValidator` into the Execution Pipeline
* **Action:** Instantiate `InputValidator` within `ToolExecutor` (or the HTTP router) and enforce input validation and null-byte sanitization on all incoming payloads before forwarding them to `Tool::execute`.

### 4. Transition to Versioned Protocol Buffer Schemas
* **Action:** Replace the dynamic `simd_json::OwnedValue` parameters inside the `Tool` trait with strongly typed, versioned Protocol Buffer structures. Generate Rust structs from `.proto` definitions using `tonic-build` and `prost` to establish structured, verifiable data contracts across the API boundary.

### 5. Generate OSCAL Component Definitions
* **Action:** Create machine-readable OSCAL `component-definition` files to document the system boundaries, least privilege boundaries, and input validation controls. Link the procedurally implemented security profiles in `security.rs` and the sanitization rules in `validation.rs` to explicit NIST SP 800-53 security control IDs (e.g., AC-3, CM-7, SI-10).