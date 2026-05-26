# LICENSE AUDIT

## 1. License Field Extraction
* **Crate `op-mcp-aggregator`**: Inherits from workspace package specification.
  * Location: `crates/op-mcp-aggregator/Cargo.toml:7` (`license.workspace = true`)
* **Workspace License**: `Apache-2.0`
  * Location: `Cargo.toml:44` (`license = "Apache-2.0"`)

## 2. Dependency License Scan (`Cargo.lock`)
A comprehensive scan of `Cargo.lock` was performed to identify copyleft licenses (GPL, AGPL, SSPL) that could conflict with the permissive `Apache-2.0` license of this codebase.
* **Findings**: No GPL, AGPL, or SSPL-licensed crates were detected in the provided `Cargo.lock`.
* **Incompatibilities**: None found. The dependency tree is compliant with the `Apache-2.0` licensing model.

## 3. Crates Lacking License Fields
No crates within the provided codebase files lack a license specification. Both the root `Cargo.toml` and the sub-crate `Cargo.toml` properly declare their licensing metadata.

---

# SCHEMA-AS-CODE DISCIPLINE AUDIT

The project dictates a Schema-as-Code discipline using versioned serialization contracts (e.g., Protocol Buffers and OSCAL). However, several data contracts, messages, and API structures are defined as ad-hoc Rust structs, bypassing versioned schemas.

## Ad-Hoc Data Contracts and Structs
The following structures define external and internal data serialization boundaries using ad-hoc `serde`/`simd_json` mapping rather than strict Protocol Buffer or OSCAL versioned schemas:

### 1. JSON-RPC & Tool Communication Contracts
* **`McpRequest`** (`crates/op-mcp-aggregator/src/client.rs:53`): Ad-hoc JSON-RPC 2.0 request structure.
* **`McpResponse`** (`crates/op-mcp-aggregator/src/client.rs:75`): Ad-hoc JSON-RPC 2.0 response structure.
* **`McpRpcError`** (`crates/op-mcp-aggregator/src/client.rs:87`): Ad-hoc JSON-RPC error structure.
* **`ToolDefinition`** (`crates/op-mcp-aggregator/src/client.rs:96`): Ad-hoc structure containing loose `Value` properties for input schemas, category, tags, and annotations.
* **`McpToolDefinition`** (`crates/op-mcp-aggregator/src/aggregator.rs:576`): Ad-hoc model for list tool responses with a loosely typed `input_schema` map.
* **`ToolCallResult`** (`crates/op-mcp-aggregator/src/aggregator.rs:542`): Struct binding return types dynamically to `Value`.

### 2. Configuration & State Contracts
* **`AggregatorConfig`** (`crates/op-mcp-aggregator/src/config.rs:13`): Complex, nested configuration document.
* **`UpstreamServer`** (`crates/op-mcp-aggregator/src/config.rs:132`): Server definitions that configure connection transports.
* **`ServerAuth`** (`crates/op-mcp-aggregator/src/config.rs:241`): Ad-hoc enum mapping authentication credentials (bearer tokens, basic, custom headers).
* **`ProfileConfig`** (`crates/op-mcp-aggregator/src/config.rs:275`): Custom profile filters for tools.
* **`CacheConfig`** (`crates/op-mcp-aggregator/src/config.rs:319`): Cache eviction and TTL parameters.
* **`ClientDetectionConfig`** (`crates/op-mcp-aggregator/src/config.rs:334`): User-Agent mapping structures.
* **`CompactModeConfig`** (`crates/op-mcp-aggregator/src/compact.rs:28`): Ad-hoc configuration properties driving tool behaviors.

### 3. Analytics & Internal Integration Contracts
* **`ClientInfo`** (`crates/op-mcp-aggregator/src/aggregator.rs:39`): Information payload derived from initialize payloads.
* **`AggregatorStats`** (`crates/op-mcp-aggregator/src/aggregator.rs:551`): Internal metric tracking payload.
* **`HealthStatus`** (`crates/op-mcp-aggregator/src/aggregator.rs:561`): Control plane status mapping structure.
* **`ServerHealth`** (`crates/op-mcp-aggregator/src/aggregator.rs:568`): Server health definitions.
* **`ProfileStats`** (`crates/op-mcp-aggregator/src/profile.rs:232`): Profile capacity structures.
* **`GroupStatus`** (`crates/op-mcp-aggregator/src/groups.rs:138`): Status object driving control plane authorization.
* **`GroupPreset`** (`crates/op-mcp-aggregator/src/groups.rs:577`): Predetermined collections of groups.

### 4. Context Suggestions Contracts
* **`ConversationContext`** (`crates/op-mcp-aggregator/src/unused/context.rs:28`): Ad-hoc analytical state storing raw string arrays.
* **`ContextSuggestion`** (`crates/op-mcp-aggregator/src/unused/context.rs:163`): Ad-hoc analytical engine suggestion results.
* **`ContextResponse`** (`crates/op-mcp-aggregator/src/unused/context.rs:408`): Communication response payloads for dynamic loader tools.

---

# SECURITY & QUALITY FINDINGS

## CRITICAL: Undefined Behavior via `unsafe` UTF-8 Mutation
### Location: `crates/op-mcp-aggregator/src/config.rs:105`
### Impact: Memory Safety / Undefined Behavior

```rust
let mut content = content;
let mut content_bytes = unsafe { content.as_bytes_mut() };
simd_json::from_slice(&mut content_bytes)
    .with_context(|| "Failed to parse JSON config")?
```

#### Vulnerability Analysis
The configuration loading logic uses `unsafe { content.as_bytes_mut() }` to obtain a mutable byte slice from an owned `String` (`content`). This slice is then passed to `simd_json::from_slice()`. 

1. **In-place Mutation Violation**: `simd_json` is a highly optimized destructive parser that mutates the input slice in-place to perform unescaping, object key indexing, and null-termination (`\0` insertions) of string values.
2. **Invariant Breach**: In Rust, a `String` must contain valid UTF-8 data at all times. Calling `as_bytes_mut()` is only safe if any mutations maintain the UTF-8 invariant of the `String`. Because `simd_json` inserts arbitrary null bytes (`\0`) and modifies the backslash sequences in-place, the UTF-8 validity is broken.
3. **Undefined Behavior (UB)**: Even though `content` goes out of scope after the function returns, having an invalid UTF-8 string alive within the function scope constitutes instant Undefined Behavior in the Rust abstract machine. The compiler's optimizer assumes `String` memory is always valid UTF-8 and can perform incorrect optimizations or generate broken machine code.

#### Remediation
Avoid parsing mutating references of `String` buffers that go out of scope. Convert the string to a mutable vector `Vec<u8>` or read the file directly into a `Vec<u8>`:
```rust
let mut content_bytes = std::fs::read(path)
    .with_context(|| format!("Failed to read config from {}", path.display()))?;
let config: Self = simd_json::from_slice(&mut content_bytes)
    .with_context(|| "Failed to parse JSON config")?;
```

---

## CRITICAL: Guaranteed Panic in Public Registry Integration
### Location: `crates/op-mcp-aggregator/src/aggregator.rs:597-601` (called at `crates/op-mcp-aggregator/src/aggregator.rs:588`)
### Impact: Denial of Service / Crash Loop

```rust
fn clone_arc(&self) -> Arc<Aggregator> {
    // This is a bit awkward - in practice you'd store Arc<Self>
    // For now, return a placeholder
    unimplemented!("Use Arc<Aggregator> directly")
}
```

#### Vulnerability Analysis
The public integration method `register_with_tool_registry` is provided to register aggregated tools with an external `op-tools::ToolRegistry`. This is the primary integration point between the aggregator and the Control Plane execution engine.

Inside `register_with_tool_registry` (lines 583-588):
```rust
pub async fn register_with_tool_registry(
    &self,
    registry: &op_tools::ToolRegistry,
    profile_name: &str,
) -> Result<()> {
    let tools = self.list_tools(profile_name).await?;

    for tool_def in tools {
        let aggregator = self.clone_arc(); // <--- TRIGGERS UNIMPLEMENTED PANIC
```
Calling `register_with_tool_registry` triggers a guaranteed panic whenever a control plane service attempts to register tools. Because this runs inside critical Control Plane worker loops, this will cause the service to crash repeatedly.

#### Remediation
Refactor `Aggregator` to be constructed or wrapped natively inside an `Arc<Aggregator>` and store the `Arc` on the proxy tool rather than calling a fake clone placeholder.

---

## HIGH: Memory Leak & Orphaned Background Thread
### Location: `crates/op-mcp-aggregator/src/aggregator.rs:111-116` & `crates/op-mcp-aggregator/src/cache.rs:241-249`
### Impact: Memory Leak / Resource Exhaustion

```rust
// Start background cache maintenance if configured
if self.config.cache.background_refresh {
    let cache = self.cache.clone();
    tokio::spawn(async move {
        cache_maintenance_loop(cache, Duration::from_secs(60)).await;
    });
}
```

#### Vulnerability Analysis
During aggregator initialization, a background cache maintenance task is spawned via `tokio::spawn` moving a cloned `Arc<ToolCache>` into `cache_maintenance_loop`. 

The maintenance loop is implemented as an infinite cycle:
```rust
pub async fn cache_maintenance_loop(cache: Arc<ToolCache>, interval: Duration) {
    loop {
        tokio::time::sleep(interval).await;
        let evicted = cache.evict_expired().await;
        ...
    }
}
```

Because the spawned task holds a strong `Arc` reference to `ToolCache` and runs an infinite loop with no exit criteria or cancellation token, the cache memory is leaked. Even if the main `Aggregator` struct is dropped, the background task will keep the `ToolCache` alive in memory indefinitely.

#### Remediation
Provide a cancellation token (`tokio_util::sync::CancellationToken`) or use a `Weak<ToolCache>` reference to allow the thread to exit once the aggregator is deallocated:
```rust
let cache = Arc::downgrade(&self.cache);
tokio::spawn(async move {
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
        if let Some(cache) = cache.upgrade() {
            cache.evict_expired().await;
        } else {
            break; // Aggregator dropped, stop loop
        }
    }
});
```

---

## HIGH: Dynamic Initialization Race Condition
### Location: `crates/op-mcp-aggregator/src/aggregator.rs:66-70`
### Impact: Duplicate Connections / Thread Bloat

```rust
pub async fn initialize(&self) -> Result<()> {
    if *self.initialized.read().await {
        return Ok(());
    }
```

#### Vulnerability Analysis
The `initialize` block implements a check-then-act pattern on the `initialized` lock.
The read lock is acquired, checked, and immediately released. If multiple threads call `initialize` concurrently, they will all read `initialized` as `false`, pass the guard statement, and concurrently initiate upstream TCP connections and spawn separate duplicate background cache maintenance tasks (resulting in multiple identical orphaned infinite loops).

#### Remediation
Obtain a write lock immediately and perform the check-then-set pattern atomically within a single transactional write guard:
```rust
pub async fn initialize(&self) -> Result<()> {
    let mut initialized = self.initialized.write().await;
    if *initialized {
        return Ok(());
    }
    
    // ... connect to servers ...
    
    *initialized = true;
    Ok(())
}
```

---

## MEDIUM: Plaintext Authentication Credentials Over Unencrypted SSE HTTP
### Location: `crates/op-mcp-aggregator/src/client.rs:111-158`
### Impact: Information Disclosure / Credential Harvesting

```rust
ServerAuth::Basic { username, password } => {
    let mut headers = reqwest::header::HeaderMap::new();
    use base64::Engine;
    let credentials = base64::engine::general_purpose::STANDARD
        .encode(format!("{}:{}", username, password));
    headers.insert(
        reqwest::header::AUTHORIZATION,
        format!("Basic {}", credentials)
            .parse()
            .map_err(|_| anyhow!("Invalid basic auth"))?,
    );
```

#### Vulnerability Analysis
The `McpClient` supports upstream server configurations using Basic Auth or Bearer tokens. However, the client fails to enforce TLS (`https://`) when transmitting authorization headers. If an upstream server URL is configured with `http://` (e.g., local server or internal development bridge), credentials are sent across the local network in plaintext Base64, facilitating sniffing or extraction via MITM.

#### Remediation
Enforce HTTPS schemes for any upstream servers that configure authentication blocks:
```rust
if config.auth.is_some() && !config.url.starts_with("https://") {
    return Err(anyhow!("SSL/TLS is required for authenticated upstream connections"));
}
```