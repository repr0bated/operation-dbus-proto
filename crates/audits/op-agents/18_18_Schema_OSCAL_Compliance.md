### Schema-as-Code Compliance Audit

This codebase heavily relies on ad-hoc Rust structs, manual JSON string serialization, and untyped JSON values (`simd_json::OwnedValue`) to define its message schemas and API boundaries. There are no `.proto` definitions or schema-driven validation contracts, presenting significant maintenance and schema evolution risks.

| Item | Type | file:line | Has .proto? | Gap |
| :--- | :--- | :--- | :--- | :--- |
| `AgentDescriptor` | Rust Struct | `crates/op-agents/src/agent_catalog.rs:41` | **No** | Tool registration payload uses ad-hoc Rust structure with no versioned schema definition. |
| `AgentSpec` | Rust Struct | `crates/op-agents/src/agent_registry.rs:18` | **No** | Complex agent launch configuration is parsed directly from un-versioned JSON. |
| `AgentInstance` | Rust Struct | `crates/op-agents/src/agent_registry.rs:83` | **No** | Dynamic instance state schema defined purely as Rust data structures. |
| `AgentTask` | Rust Struct | `crates/op-agents/src/agents/base.rs:11` | **No** | Task request structure contains an untyped `HashMap<String, simd_json::OwnedValue>` config map. |
| `TaskResult` | Rust Struct | `crates/op-agents/src/agents/base.rs:51` | **No** | Execution result uses an untyped `HashMap<String, simd_json::OwnedValue>` metadata map. |
| `org.dbusmcp.Agent` RPC | D-Bus Interface | `crates/op-agents/src/dbus_service.rs:95` | **No** | D-Bus interface passes messages as untyped raw JSON `String` payloads. |
| `/api/agents` payload | REST Endpoint | `crates/op-agents/src/router.rs:131` | **No** | Axum REST handlers consume and produce untyped, unstructured JSON payloads (`simd_json::OwnedValue`). |
| `serialize_memory_entries` | Hand-rolled Serializer | `crates/op-agents/src/agents/orchestration/memory.rs:232` | **No** | String-formatting `format!` macro used for manual JSON construction instead of a schema or standard serializer. |

---

### OSCAL Control Coverage Audit

There is no machine-readable OSCAL (Open Security Controls Assessment Language) mapping or System Security Plan (SSP) documenting this codebase. Security critical controls—including system privilege escalation, process isolation, boundary enforcement, and authentication—are implemented in-code as hardcoded logic with zero external configuration or policy tracing.

| Control Area | Implemented at file:line | OSCAL Artifact | Gap |
| :--- | :--- | :--- | :--- |
| **Access Enforcement (AC-3)** | `crates/op-agents/src/router.rs:64` | *None* | HTTP endpoints allow dynamic spawning of system agents over REST with no authentication or role checking. |
| **Access Enforcement (AC-3)** | `crates/op-agents/src/dbus_service.rs:95` | *None* | D-Bus methods on `org.dbusmcp.Agent` allow arbitrary clients to run unchecked system operations. |
| **Process Isolation (SC-39)** | `crates/op-agents/src/security/sandbox.rs:125` | *None* | Subprocess sandboxing logic and resource limits are hardcoded without mapping to an OSCAL System Security Plan (SSP). |
| **Boundary Protection (SC-7)** | `crates/op-agents/src/security/profiles.rs:97` | *None* | File system path whitelists and command constraints are compiled directly into the binary as hardcoded vectors. |
| **Least Privilege (AC-6)** | `crates/op-agents/src/agent_registry.rs:142` | *None* | The registry spawns processes with root privileges (`requires_root`) without verifying contextual authorization policies. |

---

### Recommendations for Major & Critical Gaps

#### 1. CRITICAL: Directly Exploitable JSON Injection via Hand-rolled Serialization
* **Location:** `crates/op-agents/src/agents/orchestration/memory.rs:232`
* **Finding:** The `serialize_memory_entries` function manually formats JSON strings using the `format!` macro:
  ```rust
  let entry_json = format!(
      "\"{}\":{{\"value\":\"{}\",\"memory_type\":\"{}\",\"tags\":[{}],\"created_at\":{},\"updated_at\":{},\"access_count\":{},\"last_accessed\":{}{}}}",
      key, entry.value, memory_type_str, tags_json, entry.created_at, entry.updated_at, 
      entry.access_count, entry.last_accessed, expires_json
  );
  ```
  Since `key` and `entry.value` are populated from user-controlled inputs via tool executions, an attacker can input unescaped double-quotes (`"`) or backslashes (`\`). This allows an attacker to inject arbitrary JSON elements, overwrite internal state, hijack the persistent memory store, or crash the agent registry during deserialization.
* **Remediation:** Remove the custom string formatting completely. Derive `serde::Serialize` on `MemoryEntry` and use a standard JSON library to safely serialize the complete `HashMap`:
  ```rust
  fn serialize_memory_entries(cache: &HashMap<String, MemoryEntry>) -> Result<String, String> {
      serde_json::to_string(cache).map_err(|e| e.to_string())
  }
  ```

#### 2. MAJOR: Untyped, Schema-less Communication over D-Bus and HTTP
* **Location:** `crates/op-agents/src/dbus_service.rs:104` and `crates/op-agents/src/router.rs:131`
* **Finding:** Core agent operations receive tasks as raw JSON strings (`task_json: String`) and deserialize them dynamically. This bypasses structured type validation, exposes the endpoints to malformed payload processing vulnerabilities, and breaks interface compatibility.
* **Remediation:** Define all agent payloads (`AgentTask`, `TaskResult`, `AgentDescriptor`) as versioned Protocol Buffers inside `.proto` files. Compile them using `prost` or `tonic-build` as part of the workspace build pipeline. Enforce strict type constraints and structural verification before any execution or routing logic occurs.

#### 3. MAJOR: Lack of OSCAL Component Definitions for Privileged Spawning
* **Location:** `crates/op-agents/src/agent_registry.rs:142` and `crates/op-agents/src/security/sandbox.rs:125`
* **Finding:** The control plane spawns system commands, some requiring elevated or root permissions, without referencing a system-wide security plan or component definition file. Hardcoded file system whitelists are prone to configuration drift and security audits cannot dynamically verify compliance.
* **Remediation:** Author an OSCAL `component-definition` YAML mapping the codebase's sandboxing mechanism (`SandboxExecutor`), network restrictions, and path whitelists to NIST SP 800-53 Rev. 5 controls (primarily AC-3, AC-6, SC-7, and SC-39). Implement an access-control check that cross-references requests against this machine-readable policy before executing commands.