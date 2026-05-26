# Production Security and Quality Audit: op-mcp-aggregator

---

## 1. Public API Surface & Dead Code

### Public API Surface Summary

The `op-mcp-aggregator` crate exposes an API designed to coordinate multiple Model Context Protocol (MCP) upstream servers, cache tool schemas, filter capabilities through profiles, and enforce IP-based group entitlements.

- **Total `pub` Items**: ~185 (including structs, enums, impl methods, constants, and re-exports)
- **Glob Re-exports (`pub use *`)**: **0** found. The crate strictly uses explicit re-exports (e.g., in `crates/op-mcp-aggregator/src/lib.rs:80`), which is excellent practice.

### Top 10 Most Impactful Public API Items

| # | Item | Type | Location (File:Line) | Impact Description |
|---|------|------|----------------------|--------------------|
| 1 | `Aggregator` | `struct` | `crates/op-mcp-aggregator/src/aggregator.rs:21` | Main orchestrator managing the lifecycles of server connections, caching tool definitions, and dispatching execution. |
| 2 | `Aggregator::new` | `fn` | `crates/op-mcp-aggregator/src/aggregator.rs:44` | Primary programmatic constructor to initialize the aggregator with customized configuration blocks. |
| 3 | `McpClient` | `struct` | `crates/op-mcp-aggregator/src/client.rs:93` | Low-level client managing protocol-compliant SSE/stdio requests and authentication to upstreams. |
| 4 | `ToolCache` | `struct` | `crates/op-mcp-aggregator/src/cache.rs:46` | High-performance cache wrapper enforcing TTL limits and LRU evictions to mitigate LLM orchestration latency. |
| 5 | `ProfileManager` | `struct` | `crates/op-mcp-aggregator/src/profile.rs:14` | Evaluates and selects subsets of tools to guarantee adherence to system limits (e.g., Cursor's 40-tool ceiling). |
| 6 | `ToolGroups` | `struct` | `crates/op-mcp-aggregator/src/groups.rs:154` | Evaluates IP-based network client metadata to authorize or restrict execution scopes dynamically. |
| 7 | `AggregatorConfig` | `struct` | `crates/op-mcp-aggregator/src/config.rs:17` | Root configuration schema housing profile maps, caching directives, server definitions, and compact modes. |
| 8 | `create_compact_tools` | `fn` | `crates/op-mcp-aggregator/src/compact.rs:74` | Instantiates meta-tools (`list_tools`, `execute_tool`, etc.) allowing up to 95% LLM context savings. |
| 9 | `ConversationContext` | `struct` | `crates/op-mcp-aggregator/src/unused/context.rs:28` | Intended structure tracking user session file paths and intent mappings (currently fully dead/unimported). |
| 10 | `AccessZone` | `enum` | `crates/op-mcp-aggregator/src/lib.rs:88` | Re-exported boundary descriptor establishing the localized authorization tier of incoming requests. |

### Struct Fields with Over-Exposed Visibility

Several internal data structures expose their fields publicly, bypassing validation boundaries:

- **`ToolDefinition` (`crates/op-mcp-aggregator/src/client.rs:78`)**: All fields (`name`, `description`, `input_schema`, `schema_version`, etc.) are `pub`. Modifying these fields directly on cached entries bypasses checks that prevent mismatched schemas.
- **`ToolGroup` (`crates/op-mcp-aggregator/src/groups.rs:33`)**: Exposes all fields (`id`, `name`, `security`, `dependencies`, etc.) as `pub`. Direct mutation of an active group's security level or dependency stack bypasses the state validation enforced inside `ToolGroups::enable`.
- **`ProfileConfig` (`crates/op-mcp-aggregator/src/config.rs:307`)**: Exposes filters (`servers`, `include_tools`, `exclude_tools`) as `pub`, allowing profiles to be modified at runtime in ways that may conflict with active client caches.

---

### Dead Code Analysis

A comprehensive scan of the provided codebase was performed to identify unused structures, functions, modules, and dangling imports. 

- **`#[allow(dead_code)]` Count**: **0** instances of `#[allow(dead_code)]` were found in the audited files.
- **Unimported/Unreferenced Modules**: The entire directory/module `crates/op-mcp-aggregator/src/unused/` is not registered in `lib.rs`, meaning its structures and helper functions are entirely dead.

### Dead Code Table

| Item | Type | Location (File:Line) | Recommendation |
|:---|:---|:---|:---|
| `ConversationContext` | `struct` | `crates/op-mcp-aggregator/src/unused/context.rs:28` | Remove the `unused` directory or register the module in `lib.rs` and integrate. |
| `ContextSuggestion` | `struct` | `crates/op-mcp-aggregator/src/unused/context.rs:191` | Remove or integrate with the main proxy loop. |
| `ContextAwareTools` | `struct` | `crates/op-mcp-aggregator/src/unused/context.rs:207` | Remove or integrate with target server routes. |
| `ContextResponse` | `struct` | `crates/op-mcp-aggregator/src/unused/context.rs:438` | Remove or integrate with outer REST endpoints. |
| `looks_like_path` | `fn` | `crates/op-mcp-aggregator/src/unused/context.rs:114` | Remove (ad-hoc file heuristic is error-prone). |
| `detect_intent` | `fn` | `crates/op-mcp-aggregator/src/unused/context.rs:123` | Remove (NLP intent matching should be offloaded to LLM). |
| `detect_domain` | `fn` | `crates/op-mcp-aggregator/src/unused/context.rs:142` | Remove. |
| `build_file_mappings` | `fn` | `crates/op-mcp-aggregator/src/unused/context.rs:331` | Remove. |
| `build_keyword_mappings` | `fn` | `crates/op-mcp-aggregator/src/unused/context.rs:374` | Remove. |
| `build_intent_mappings` | `fn` | `crates/op-mcp-aggregator/src/unused/context.rs:414` | Remove. |
| `CONTEXT_KEYWORDS` | `const` | `crates/op-mcp-aggregator/src/unused/context.rs:94` | Remove. |
| `Aggregator::clone_arc` | `fn` | `crates/op-mcp-aggregator/src/aggregator.rs:630` | Rewrite. This function panics unconditionally with `unimplemented!()` if `register_with_tool_registry` is invoked. |
| `create_default_profiles` | `fn` | `crates/op-mcp-aggregator/src/profile.rs:229` | Expose to public configuration generators or remove. |
| `builtin_presets` | `fn` | `crates/op-mcp-aggregator/src/groups.rs:649` | Integrate with `ToolGroups::apply_preset` or remove. |
| `GroupPreset` | `struct` | `crates/op-mcp-aggregator/src/groups.rs:729` | Integrate or remove. |

---

## 2. Schema-as-Code Compliance

A schema-as-code discipline dictates that data contracts should be expressed using centralized, versioned schemas (such as Protocol Buffers or OSCAL profiles) rather than ad-hoc maps, unstructured strings, or generic JSON-RPC values.

### Violations of Schema-as-Code Discipline

1. **Ad-Hoc JSON Schema Payload Passing**
   - **Locations**: 
     - `crates/op-mcp-aggregator/src/client.rs:78` (`ToolDefinition::input_schema`)
     - `crates/op-mcp-aggregator/src/aggregator.rs:599` (`McpToolDefinition::input_schema`)
   - **Violation**: The input schema of tools is carried as an untyped `simd_json::OwnedValue` (re-aliased as `Value`). There is no structural model validation or schema serialization layer. Handlers must query nested properties using ad-hoc string indexing (e.g., `obj.get("category").and_then(|c| c.as_str())`), introducing contract drift risks between cached entries, upstream clients, and downstream LLM agents.

2. **Ad-Hoc JSON Response Construction in Compact Meta-Tools**
   - **Locations**:
     - `crates/op-mcp-aggregator/src/compact.rs:173-181` (`ListToolsTool::execute`)
     - `crates/op-mcp-aggregator/src/compact.rs:244-249` (`ExecuteToolTool::execute`)
     - `crates/op-mcp-aggregator/src/compact.rs:305-311` (`GetToolSchemaTool::execute`)
     - `crates/op-mcp-aggregator/src/compact.rs:393-398` (`SearchToolsTool::execute`)
     - `crates/op-mcp-aggregator/src/compact.rs:485-502` (`BatchExecuteTool::execute`)
     - `crates/op-mcp-aggregator/src/compact.rs:514-556` (`compact_mode_summary`)
   - **Violation**: Instead of serializing to standard, versioned structures, all compact mode outputs are generated on-the-fly using the `json!` macro. This creates implicit, undocumented contracts that are easily broken during refactoring.

3. **String-Encoded Domain Group Schemas**
   - **Location**: `crates/op-mcp-aggregator/src/groups.rs:33` (`ToolGroup`)
   - **Violation**: Security categories and domain associations are specified using raw strings (`domain: String`, `patterns: Vec<String>`). These should be driven by versioned security posture documents (e.g., OSCAL component profiles) rather than hardcoded arrays in Rust source files.

---

## 3. Production Security & Quality Analysis

---

### [HIGH] Denial of Service: Guaranteed Panic in `register_with_tool_registry`

#### Technical Overview
The aggregator exposes a public integration method to register proxy tools with external command registries (e.g., `op-tools`):

```rust
// crates/op-mcp-aggregator/src/aggregator.rs:608-628
pub async fn register_with_tool_registry(
    &self,
    registry: &op_tools::ToolRegistry,
    profile_name: &str,
) -> Result<()> {
    let tools = self.list_tools(profile_name).await?;

    for tool_def in tools {
        let aggregator = self.clone_arc(); // <--- TRIGGERS UNCONDITIONAL PANIC
...
```

However, the internal helper `clone_arc` is stubbed out with `unimplemented!()`:

```rust
// crates/op-mcp-aggregator/src/aggregator.rs:630-633
fn clone_arc(&self) -> Arc<Aggregator> {
    // This is a bit awkward - in practice you'd store Arc<Self>
    // For now, return a placeholder
    unimplemented!("Use Arc<Aggregator> directly")
}
```

#### Vulnerability Mechanics
When any external component attempts to integrate aggregated tools via `register_with_tool_registry`, the code executes `self.clone_arc()`, causing a thread-level panic. Because this is an asynchronous context often executed on a shared worker thread, a panic here can disrupt downstream tasks or crash the entire control plane.

#### Remediation
1. Adjust the method signature to accept an already wrapped `Arc<Self>`.
2. Implement safe cloning of `Arc<Self>` internally, or use standard thread-safe initialization wrappers.

```rust
// Recommended Refactoring
pub async fn register_with_tool_registry(
    self: &Arc<Self>,
    registry: &op_tools::ToolRegistry,
    profile_name: &str,
) -> Result<()> {
    let tools = self.list_tools(profile_name).await?;
    for tool_def in tools {
        let aggregator = self.clone(); // Safely clones the Arc
        ...
```

---

### [HIGH] Access Control Bypass: IP-Based Zone Spoofing on Restricted Tool Groups

#### Technical Overview
The security design restricts dangerous system commands (`shell-root`, `system-power`, `disk-format`) to the `Restricted` security tier. It attempts to enforce this by extracting the client IP and mapping it to a local `AccessZone`:

```rust
// crates/op-mcp-aggregator/src/groups.rs:189-195
pub fn from_ip(mut self, ip: &str) -> Self {
    self.access_zone = AccessZone::from_ip_with_config(ip, &self.network_config);
    self.client_ip = Some(ip.to_string());
    info!("🌐 Client IP: {} -> {}", ip, self.access_zone.description());
    self
}
```

```rust
// crates/op-mcp-aggregator/src/groups.rs:214-230
if !self.access_zone.can_access(security) {
    let required = match security {
        SecurityLevel::Restricted => "localhost (127.0.0.1)",
        SecurityLevel::Elevated => "localhost or private network",
        _ => "any",
    };
    return Err(format!(
        "Group '{}' ({:?}) requires {} access. Your zone: {}",
        ...
```

#### Vulnerability Mechanics
IP-based authentication is fundamentally insecure when deployed behind standard HTTP reverse proxies, load balancers, or VPN gateways unless the transport layer explicitly validates connection origins. 

If an attacker sends a request with a spoofed header (e.g., `X-Forwarded-For: 127.0.0.1` or `X-Real-IP: 127.0.0.1`), and the outer web/JSON-RPC server (such as `op-web` or `op-http`) extracts this client IP and passes it blindly to `from_ip()`, the aggregator will grant the remote attacker `AccessZone::Localhost` status. This allows them to enable restricted tool groups and execute arbitrary commands as `root`.

#### Remediation
1. Ensure the web layer sanitizes untrusted proxy headers before determining client IPs.
2. Replace IP-based zone matching with strong cryptographic signatures, JWTs, or client-certificate authentication for any transition to `SecurityLevel::Restricted` or `SecurityLevel::Elevated`.

---

### [MEDIUM] Integrity Risk: Missing Input Schema Validation on Dynamic Tool Calls

#### Technical Overview
The aggregator proxies execution requests to upstream MCP servers using `Value` payloads.

```rust
// crates/op-mcp-aggregator/src/aggregator.rs:145-168
pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<ToolCallResult> {
    self.ensure_initialized().await?;
    ...
    let client = self
        .clients
        .get_client(&server_id)
        .await
        .ok_or_else(|| anyhow!("Server '{}' not connected", server_id))?;

    // Call the tool
    let result = client
        .call_tool(name, arguments.clone())
        .await
        ...
```

#### Vulnerability Mechanics
In `call_tool`, the aggregator forwards the raw user-provided `arguments` to the target client without validating them against the cached JSON schema (`ToolCache`). 

While upstream servers are expected to validate their inputs, failing to validate them at the gateway layer allows malformed JSON-RPC payloads to reach internal services. This increases the attack surface for parser exploits or injection attacks on downstream services.

#### Remediation
Use a JSON Schema validator (e.g., `jsonschema`, which is already declared in the workspace dependencies) to validate incoming `arguments` against the cached `input_schema` prior to dispatching upstream requests.

```rust
// Recommended validation interceptor in call_tool:
let (tool_def, _) = self.cache.get(name).await
    .ok_or_else(|| anyhow!("Tool '{}' not found in cache", name))?;

// Compile and validate the JSON schema
if let Ok(schema) = jsonschema::JSONSchema::compile(&tool_def.input_schema) {
    if let Err(errors) = schema.validate(&arguments) {
        return Err(anyhow!("Input validation failed: {:?}", errors.collect::<Vec<_>>()));
    }
}
```

---

### [MEDIUM] Quality & Reliability: Incomplete Stdio Transport Stubs

#### Technical Overview
The config module permits setting the transport type to `stdio` (`TransportType::Stdio` in `crates/op-mcp-aggregator/src/config.rs:256`). However, the implementation lacks the process execution and pipe handling required for stdio communication:

```rust
// crates/op-mcp-aggregator/src/client.rs:205-209
async fn initialize_stdio(&self) -> Result<()> {
    // For stdio, we'd spawn a child process
    // This is a simplified implementation
    warn!("Stdio transport initialization not fully implemented");
    Ok(())
}
```

```rust
// crates/op-mcp-aggregator/src/client.rs:259-263
async fn send_stdio_request(&self, _request: &McpRequest) -> Result<McpResponse> {
    // Stdio implementation would write to child process stdin
    // and read from stdout
    Err(anyhow!("Stdio transport not fully implemented"))
}
```

#### Quality Mechanics
If a user configures an upstream server to run over `stdio` (e.g., a local CLI binary), initialization will silently succeed, but any subsequent tool calls will fail immediately with `Stdio transport not fully implemented`. This leads to poor diagnostic clarity and unexpected runtime errors.

#### Remediation
Fully implement stdio process spawning using `tokio::process::Command` with standard I/O redirection (`Stdio::piped()`), or remove the `TransportType::Stdio` configuration variant until it is fully supported.