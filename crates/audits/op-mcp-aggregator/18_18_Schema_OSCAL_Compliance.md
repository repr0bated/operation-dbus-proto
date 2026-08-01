# Production Security & Quality Audit: op-mcp-aggregator

---

### 1. Schema-as-Code Table

| Item | Type | file:line | Has .proto? | Gap |
| :--- | :--- | :--- | :--- | :--- |
| `ClientInfo` | Struct | `crates/op-mcp-aggregator/src/aggregator.rs:43` | No | Ad-hoc Rust struct with manual serde parsing. |
| `call_tool` arguments | Method Parameter | `crates/op-mcp-aggregator/src/aggregator.rs:188` | No | Uses untyped `simd_json::OwnedValue` (Value) for client/server IPC data. |
| `ToolCallResult` | Struct | `crates/op-mcp-aggregator/src/aggregator.rs:444` | No | Ad-hoc Rust struct wrapping untyped JSON payload. |
| `AggregatorStats` | Struct | `crates/op-mcp-aggregator/src/aggregator.rs:453` | No | Ad-hoc Rust struct for statistics. |
| `HealthStatus` / `ServerHealth` | Struct | `crates/op-mcp-aggregator/src/aggregator.rs:463` | No | Ad-hoc representation of target health status. |
| `McpToolDefinition` | Struct | `crates/op-mcp-aggregator/src/aggregator.rs:476` | No | Untyped input schemas represented as ad-hoc JSON elements. |
| `McpRequest` | Struct | `crates/op-mcp-aggregator/src/client.rs:44` | No | Hand-rolled representation of JSON-RPC protocol requests. |
| `McpResponse` | Struct | `crates/op-mcp-aggregator/src/client.rs:67` | No | Hand-rolled representation of JSON-RPC protocol responses. |
| `McpRpcError` | Struct | `crates/op-mcp-aggregator/src/client.rs:77` | No | Ad-hoc representation of protocol-level errors. |
| `ToolDefinition` | Struct | `crates/op-mcp-aggregator/src/client.rs:85` | No | Ad-hoc data structure for downstream tool contracts. |
| `AggregatorConfig` | Struct | `crates/op-mcp-aggregator/src/config.rs:17` | No | Ad-hoc configuration parsed using file extensions and raw serde mappings. |
| `ToolGroup` | Struct | `crates/op-mcp-aggregator/src/groups.rs:37` | No | Hardcoded Rust struct grouping tools with unstructured string fields. |
| `ConversationContext` | Struct | `crates/op-mcp-aggregator/src/unused/context.rs:25` | No | Ad-hoc unstructured analysis object utilizing plain-text fields. |
| `ContextSuggestion` | Struct | `crates/op-mcp-aggregator/src/unused/context.rs:196` | No | Ad-hoc representation of context suggestions. |
| `ContextResponse` | Struct | `crates/op-mcp-aggregator/src/unused/context.rs:360` | No | Untyped metadata response wrapped in ad-hoc JSON layout. |

---

### 2. OSCAL Coverage Table

| Control Area | Implemented at file:line | OSCAL Artifact | Gap |
| :--- | :--- | :--- | :--- |
| **Identification & Authentication** (IA-2, IA-8) | `crates/op-mcp-aggregator/src/client.rs:105-144` | None | Upstream server authentication mechanisms (Bearer, Basic, Custom Headers) are configured in code/config files with no corresponding OSCAL control mapping or automated credential management strategy. |
| **System & Communications Protection** (SC-7) | `crates/op-mcp-aggregator/src/client.rs:241-269` | None | Network pathing and endpoint normalization (`/mcp` vs `/message`) are entirely governed by code logic without architectural mappings in an OSCAL Component Definition. |
| **Access Control / Least Privilege** (AC-3, AC-6) | `crates/op-mcp-aggregator/src/profile.rs:107` | None | Named user profiles (e.g. `minimal`, `sysadmin`, `dev`) restricting accessible tools are implemented locally, with no dynamic OSCAL mapping to verify boundary adherence. |
| **Authorization Bypass in Compact Mode** (AC-3, AC-6) | `crates/op-mcp-aggregator/src/compact.rs:262` | None | **CRITICAL**: The meta-tool execution mechanism permits calling any cached tool directly, bypassing profile-based boundary controls. This design defect cannot be audited or represented securely in OSCAL. |
| **Information Flow Enforcement** (AC-4) | `crates/op-mcp-aggregator/src/groups.rs:172-224` | None | IP-to-AccessZone logic (mapping clients to `Public`, `Standard`, `Elevated`, or `Restricted` tools) is hardcoded in Rust, rather than validated against machine-readable OSCAL SSP system boundaries. |
| **Hardcoded Security Categorization** (RA-3) | `crates/op-mcp-aggregator/src/groups.rs:328-601` | None | Classification of tools (e.g. labeling `shell-root` or `disk-format` as `Restricted`) is hardcoded in the codebase, preventing dynamic verification of system security posture. |
| **Adaptive Flow Control / Context Extraction** (AC-4) | `crates/op-mcp-aggregator/src/unused/context.rs:74` | None | Heuristic context mining (extracting domains from user messages and auto-enabling toolsets) is implemented implicitly, violating explicit flow enforcement requirements. |
| **Audit Logging & Review** (AU-2, AU-12) | `crates/op-mcp-aggregator/src/aggregator.rs:254` | None | Connection events, auto-detected client shifts, and execution results are logged via `tracing::info!` without mapping to OSCAL audit policies. |

---

### 3. Detailed Findings & Recommendations

#### CRITICAL: Authorization & Security Profile Bypass in Compact Mode Meta-Tools
*   **Vulnerability Description**: The compact mode defines a series of meta-tools designed to reduce context window consumption by lazy-loading schemas. One of these meta-tools, `execute_tool` (`crates/op-mcp-aggregator/src/compact.rs:262`), allows clients to execute any tool by name. However, `ExecuteToolTool::execute` directly invokes `self.aggregator.call_tool(tool_name, arguments)` (`crates/op-mcp-aggregator/src/compact.rs:262`) rather than validating the client's current profile through `call_tool_in_profile` (`crates/op-mcp-aggregator/src/aggregator.rs:219`). 
    Furthermore, `call_tool` contains no validation checking the client's `AccessZone` (IP classification) or security clearance (`SecurityLevel::Restricted` vs `Public`) for the target tool. Because any client that can call the `execute_tool` meta-tool (which is categorized as `SecurityLevel::Elevated`) can instruct it to execute *any* tool cached from the upstream servers (such as `shell-root` or `disk-format`), a client assigned to a highly restricted profile (e.g., `minimal`) can escalate privileges and run arbitrary restricted system commands.
*   **Proof of Concept Flow**:
    1. A client connects from an untrusted zone and is assigned the `minimal` profile (which contains only safe, read-only tools like `respond`).
    2. Because they are in compact mode, they are exposed to the `execute_tool` meta-tool.
    3. The client calls `execute_tool` passing `"tool_name": "shell_root"` and `"arguments": {"cmd": "rm -rf /"}`.
    4. `ExecuteToolTool::execute` extracts `"shell_root"` and executes `self.aggregator.call_tool("shell_root", arguments)`.
    5. `Aggregator::call_tool` resolves the target tool directly from the cache, retrieves the client connection, and executes the command on the target server. The profile check in `call_tool_in_profile` is completely bypassed, and no `AccessZone` validation occurs.
*   **Remediation Recommendation**: 
    1. Modify `ExecuteToolTool` to track the user's active profile and restrict tool execution by calling `self.aggregator.call_tool_in_profile(...)` instead of `call_tool(...)`.
    2. Enforce dynamic IP-based security checks inside `call_tool` directly. Query the active `AccessZone` of the calling client and cross-reference the matched tool's security level prior to dispatching the RPC to the upstream server.

#### MAJOR: Use of Untyped JSON representation for Critical Data Contracts (Schema-as-Code Violation)
*   **Vulnerability Description**: The system heavily relies on untyped structured JSON value objects (`simd_json::OwnedValue` / `Value`) across critical boundaries:
    *   Tool parameters are accepted as raw `Value` objects in `call_tool` (`crates/op-mcp-aggregator/src/aggregator.rs:188`).
    *   Downstream schemas are stored as unstructured `Value` schemas in `ToolDefinition` (`crates/op-mcp-aggregator/src/client.rs:85`).
    *   The JSON-RPC communication layer uses `Value` fields for parameters, results, and errors (`crates/op-mcp-aggregator/src/client.rs:44`, `crates/op-mcp-aggregator/src/client.rs:67`).
    This reliance on untyped models violates the core principles of schema-as-code. It bypasses compile-time type validation, making the interface highly susceptible to format injection, null pointer assumptions, and type-confusion vulnerabilities during deserialization.
*   **Remediation Recommendation**: Refactor the RPC contract using strongly typed structures. Express the JSON-RPC interface and the Model Context Protocol (MCP) specifications as Protocol Buffer schemas (.proto files) and generate Rust structures using `prost`/`tonic` (already included in the workspace dependencies). Validate all unstructured JSON input payload envelopes against strongly typed Rust definitions at the entry point of the aggregator.

#### MAJOR: Code-Hardcoded Security Controls and Policies (OSCAL Non-Compliance)
*   **Vulnerability Description**: Security decisions, tool classifications, and client routing choices are tightly coupled to code execution:
    *   Client auto-detection defaults are mapped to hardcoded string matches (e.g., `"claude"`, `"anthropic"`, `"cursor"`) in `ClientDetectionConfig` (`crates/op-mcp-aggregator/src/config.rs:434-526`).
    *   System security classifications mapping tools to Public, Standard, Elevated, or Restricted categories are entirely hardcoded in `builtin_groups` (`crates/op-mcp-aggregator/src/groups.rs:328-601`).
    This architecture makes it impossible to dynamically assess, audit, or reconfigure the security posture of the platform without recompiling and redeploying the code. It directly conflicts with OSCAL requirements, which mandate machine-readable, externalized control declarations (such as System Security Plans) that can be dynamically verified.
*   **Remediation Recommendation**: Externalize all authorization policies, client rules, and tool classification profiles. Define these boundaries in standardized schema documents (e.g., XML/JSON representing OSCAL Component Definitions and Profiles). Implement a runtime policy engine inside the aggregator that ingests these schemas, rather than compiling hardcoded strings inside `groups.rs` and `config.rs`.