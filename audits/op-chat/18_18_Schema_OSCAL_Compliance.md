# Production Security & Quality Audit: op-chat

## 1. Schema-as-Code Registry

The following table documents instances where data contracts, interface schemas, or state machines are expressed as ad-hoc Rust structs, strings, or untyped JSON rather than versioned, centralized schemas.

| Item | Type | File:Line | Has .proto? | Gap |
| :--- | :--- | :--- | :--- | :--- |
| `RpcRequest` / `RpcResponse` | Message Type | `crates/op-chat/src/actor.rs:62`, `crates/op-chat/src/actor.rs:123` | **No** | Declared as ad-hoc Rust enums/structs with manual Serde tagging, bypassing versioned Protocol Buffer contracts. |
| Untyped JSON parameters (`Value`) | Parameter Type | `crates/op-chat/src/actor.rs:76`, `crates/op-chat/src/grpc_client.rs:191` | **No** | Uses `simd_json::OwnedValue` as a generic argument bucket instead of strongly-typed Protobuf fields. |
| Built-in Tool Definitions | Schema Definition | `crates/op-chat/src/chat_loop.rs:81`, `crates/op-chat/src/tool_loader.rs:110` | **No** | Tool inputs are described using inline JSON schemas compiled inside the binaries instead of schema definitions. |
| Hand-rolled Translators | Serialization | `crates/op-chat/src/mcp_server.rs:70-131`, `crates/op-chat/src/grpc_client.rs:270-344` | **No** | Uses custom nested conversion loops between `prost_types::Value` and untyped raw `simd_json::OwnedValue` rather than compiled structs. |
| Built-in Workstacks | Process Definition | `crates/op-chat/src/orchestration/mod.rs:62` | **No** | Expresses multi-phase plans and dependencies via ad-hoc Rust builder patterns instead of a versioned declarative schema. |
| Context Configuration | State Definition | `crates/op-chat/src/agent_tools.rs:267-349` | **No** | Hardcodes a list of agent properties, capabilities, and arguments inside Rust code. |

---

## 2. OSCAL Compliance & Control Mapping

This system handles operating-system level changes, OVS network changes, and systemd administration. Security controls implemented to safeguard these boundaries must be declared in machine-readable OSCAL profiles. 

The following table maps implemented security mechanisms to NIST SP 800-53 security controls, flagging gaps where code-enforced boundaries lack corresponding OSCAL artifacts.

| Control Area | Implemented at File:Line | OSCAL Artifact | Gap |
| :--- | :--- | :--- | :--- |
| **AC-3 / AC-6** (Access Control & Least Privilege) | `crates/op-chat/src/tool_loader.rs:264`, `crates/op-chat/src/tool_loader.rs:343` | SSP (System Security Plan) | Hardcoded file blocklists in Rust code instead of machine-readable security boundaries mapped via OPA or OSCAL component definitions. |
| **AC-6 / SC-7** (Least Privilege & Boundary Protection) | `crates/op-chat/src/tool_loader.rs:400` | Component Definition | Arbitrary execution allowed via whitelisted interpreter binaries (`python`, `cargo`, `npm`) without OSCAL enforcement mapping. |
| **AC-4 / SC-7** (Information Flow Enforcement) | `crates/op-chat/src/system_prompt.rs:102` | Component Definition | Network topology limits, subnets, and VLAN mappings are defined inside LLM system prompt strings instead of OSCAL boundary rules. |
| **CM-6 / SC-7** (Prohibited Command Controls) | `crates/op-chat/src/chat_loop.rs:172`, `crates/op-chat/src/nl_admin.rs:195` | System Security Plan (SSP) | Ad-hoc text-based CLI filtering with no machine-readable policy artifact alignment or control verification. |

---

## 3. Vulnerability Findings & Recommendations

### Critical Finding 1: Remote Code Execution (RCE) via Argument Injection in `ShellExecuteTool`
* **File:Line:** `crates/op-chat/src/tool_loader.rs:400-502`
* **Impact:** Critical (Directly Exploitable)
* **Description:** 
The `ShellExecuteTool` checks if a requested binary is in the `allowed_commands` whitelist (`ls`, `cat`, `python`, `cargo`, `npm`, etc.). However, it accepts arbitrary user-supplied arguments via `args: Vec<String>` and forwards them unchecked to `tokio::process::Command` at `crates/op-chat/src/tool_loader.rs:479`.
Because powerful interpreter binaries like `python`, `cargo`, and `npm` are whitelisted, an attacker can pass `command: "python3"` with `args: ["-c", "import os; os.system('malicious-payload')"]` or `command: "cargo"` with custom build-scripts. This allows an attacker to execute arbitrary shell code on the host machine.
* **Remediation:** 
Remove interpreters and developer tools (`python`, `cargo`, `npm`, `bash`) from the whitelist entirely. If arbitrary commands must be run, enforce strict validation of the argument vector using a regular expression that forbids command chaining characters, file descriptors, or code execution flags.

---

### Critical Finding 2: Path Traversal Security Bypass in `ReadFileTool` and `WriteFileTool`
* **File:Line:** `crates/op-chat/src/tool_loader.rs:264-283`, `crates/op-chat/src/tool_loader.rs:343-363`
* **Impact:** Critical (Directly Exploitable)
* **Description:** 
Both `ReadFileTool` and `WriteFileTool` attempt to restrict access to sensitive system paths by verifying if the path begins with a restricted prefix (e.g., `/etc/shadow`, `/etc/sudoers` for reading; `/etc/`, `/boot/` for writing) using `path.starts_with(...)`.
However, the input path is never canonicalized. An attacker can bypass these prefix checks entirely using path traversal. For example, providing `path: "/tmp/../../etc/shadow"` bypasses the `starts_with("/etc/shadow")` check, yet resolves to `/etc/shadow` when read by `tokio::fs::read_to_string` on line 273.
* **Remediation:** 
Enforce path canonicalization using `std::fs::canonicalize` or `tokio::fs::canonicalize` to resolve all relative symlinks and traversal sequences *before* performing any blocklist prefix checks:
```rust
let canonical_path = tokio::fs::canonicalize(path).await?;
if forbidden_paths.iter().any(|&p| canonical_path.starts_with(p)) {
    // Return access denied
}
```

---

### Major Finding 3: Insecure Serialization Mapping and Type Safety Deficiencies
* **File:Line:** `crates/op-chat/src/mcp_server.rs:70-131`, `crates/op-chat/src/grpc_client.rs:270-344`
* **Impact:** Medium/High (Reliability and Code Quality)
* **Description:** 
The codebase heavily relies on untyped `simd_json::OwnedValue` to represent complex structured data. In `mcp_server.rs` and `grpc_client.rs`, manual recursive mappings are written to translate `prost_types::Value` kinds to `simd_json::StaticNode` variants. Bypassing typed structures leads to silent desynchronization when internal Protocol Buffer schemas are updated. A change to a `.proto` contract will compile successfully but fail at runtime due to manual string-based field lookups (`get("message")`, etc.) or incorrect enum-to-integer mappings.
* **Remediation:** 
Utilize strongly-typed Rust structs generated from Protocol Buffers (`op_chat.orchestration.rs`) across the entire pipeline. Avoid translating to generic, untyped `simd_json::OwnedValue` except at the absolute boundary layer of external inputs. Ensure all tool arguments are validated against strict JSON schemas or Protobuf descriptors.

---

### Major Finding 4: Hardcoded Policy Baseline Violations (OSCAL CM-6 / SC-7 Alignment)
* **File:Line:** `crates/op-chat/src/system_prompt.rs:102-192`, `crates/op-chat/src/chat_loop.rs:172-214`
* **Impact:** Medium (Compliance & Governance)
* **Description:** 
The target network topology configuration, VLAN assignments, MTUs, routing policies, and prohibited CLI commands are defined within string prompts and inline regex filters. In a federal or high-compliance production environment, defining system boundaries and policy baselines directly in code strings violates configuration management (CM-6) and boundary protection (SC-7) auditing rules.
* **Remediation:** 
1. Export the network topology limits and command blocklists to an external, versioned schema-backed JSON or YAML policy file.
2. Generate an OSCAL Component Definition mapping these rules to NIST 800-53 security controls.
3. Validate tool calls at runtime against this externalized policy using an engine such as Open Policy Agent (OPA) instead of hardcoded strings in system prompts.