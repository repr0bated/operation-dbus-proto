This file is a merged representation of a subset of the codebase, containing specifically included files, combined into a single document by Repomix.

<file_summary>
This section contains a summary of this file.

<purpose>
This file contains a packed representation of a subset of the repository's contents that is considered the most important context.
It is designed to be easily consumable by AI systems for analysis, code review,
or other automated processes.
</purpose>

<file_format>
The content is organized as follows:
1. This summary section
2. Repository information
3. Directory structure
4. Repository files (if enabled)
5. Multiple file entries, each consisting of:
  - File path as an attribute
  - Full contents of the file
</file_format>

<usage_guidelines>
- This file should be treated as read-only. Any changes should be made to the
  original repository files, not this packed version.
- When processing this file, use the file path to distinguish
  between different files in the repository.
- Be aware that this file may contain sensitive information. Handle it with
  the same level of security as you would the original repository.
</usage_guidelines>

<notes>
- Some files may have been excluded based on .gitignore rules and Repomix's configuration
- Binary files are not included in this packed representation. Please refer to the Repository Structure section for a complete list of file paths, including binary files
- Only files matching these patterns are included: /home/jeremy/git/operation-dbus-proto/crates/op-mcp/**
- Files matching patterns in .gitignore are excluded
- Files matching default ignore patterns are excluded
- Files are sorted by Git change count (files with more changes are at the bottom)
</notes>

</file_summary>

<directory_structure>
/
  home/
    jeremy/
      git/
        operation-dbus-proto/
          crates/
            op-mcp/
              docs/
                ARCHITECTURE.md
              proto/
                internal_agents.proto
                mcp.proto
              src/
                grpc/
                  generated/
                    .gitkeep
                    op.mcp.v1.rs
                  client.rs
                  mod.rs
                  server.rs
                  service.rs
                tools/
                  filesystem.rs
                  mod.rs
                  ovs.rs
                  plugin.rs
                  qdrant.rs
                  response.rs
                  shell.rs
                  system.rs
                  systemd.rs
                transport/
                  http.rs
                  mod.rs
                  stdio.rs
                  websocket.rs
                agents_main.rs
                agents_server.rs
                agents_server.rs.patch
                builtin_trait_agents.rs
                compact_main.rs
                compact.rs
                config.rs
                external_client.rs
                http_server.rs
                lib.rs
                lib.rs.grpc-additions
                main.rs
                mod.rs
                mod.rs.patch
                protocol.rs
                request_context.rs
                request_handler.rs
                resources.rs
                router.rs
                server.rs
                sse.rs
                tool_adapter_orchestrated.rs
                tool_adapter.rs
                tool_adapter.rs.backup
                tool_registry.rs
                trait_agent_executor.rs
              build.rs
              Cargo.toml
              Cargo.toml.grpc-additions
              compare-op-mcp.md
              README.md
              SPEC.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/docs/ARCHITECTURE.md">
# op-mcp Architecture

## Overview

op-mcp is a clean MCP (Model Context Protocol) server that provides:

1. **MCP JSON-RPC 2.0 Protocol** - Standard MCP protocol over stdio
2. **Lazy Tool Loading** - Tools loaded on-demand with LRU caching
3. **Discovery System** - Multiple sources for tool discovery
4. **External MCP Aggregation** - Connect to other MCP servers

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                        MCP Client                                │
│                    (Claude Desktop, etc.)                        │
└───────────────────────────┬─────────────────────────────────────┘
                            │ stdio (JSON-RPC 2.0)
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│                        op-mcp Server                             │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                    McpServer                             │   │
│  │  - JSON-RPC protocol handling                            │   │
│  │  - Request routing                                       │   │
│  │  - Response formatting                                   │   │
│  └────────────────────────┬────────────────────────────────┘   │
│                           │                                      │
│  ┌────────────────────────▼────────────────────────────────┐   │
│  │                 LazyToolManager                          │   │
│  │  - On-demand tool loading                                │   │
│  │  - Context-based filtering                               │   │
│  │  - LRU cache management                                  │   │
│  └────────────────────────┬────────────────────────────────┘   │
│                           │                                      │
│           ┌───────────────┼───────────────┐                     │
│           ▼               ▼               ▼                     │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐            │
│  │ ToolRegistry │ │  Discovery   │ │   External   │            │
│  │  (LRU Cache) │ │   System     │ │  MCP Clients │            │
│  └──────────────┘ └──────────────┘ └──────────────┘            │
└─────────────────────────────────────────────────────────────────┘
                            │
        ┌───────────────────┼───────────────────┐
        ▼                   ▼                   ▼
┌──────────────┐   ┌──────────────┐   ┌──────────────┐
│   D-Bus      │   │   Plugins    │   │   Agents     │
│   Services   │   │   (op-state) │   │  (op-agents) │
└──────────────┘   └──────────────┘   └──────────────┘
```

## Key Components

### 1. McpServer

The main server component that:
- Handles MCP JSON-RPC 2.0 protocol
- Routes requests to appropriate handlers
- Manages server lifecycle

### 2. LazyToolManager

Manages tool loading with:
- **On-demand loading**: Tools loaded when first requested
- **LRU caching**: Evicts unused tools to save memory
- **Context filtering**: Returns relevant tools based on context
- **Multiple sources**: D-Bus, plugins, agents, external MCP

### 3. ToolRegistry (from op-tools)

Provides:
- Tool storage with usage tracking
- LRU eviction policy
- Factory-based lazy loading
- Statistics and monitoring

### 4. ToolDiscoverySystem (from op-tools)

Manages tool discovery from:
- **BuiltinToolSource**: Compiled-in tools
- **DbusDiscoverySource**: Runtime D-Bus introspection
- **PluginDiscoverySource**: State management plugins
- **AgentDiscoverySource**: Agent-based tools

## Data Flow

### Tool Listing

```
Client → tools/list → LazyToolManager → DiscoverySystem → Definitions
                                      ↓
                              Apply context filter
                                      ↓
                              Return paginated list
```

### Tool Execution

```
Client → tools/call → LazyToolManager → Registry.get()
                                       ↓
                            ┌──────────┴──────────┐
                            │ Tool loaded?        │
                            ├─────────────────────┤
                            │ Yes: Return cached  │
                            │ No: Load via factory│
                            └─────────────────────┘
                                       ↓
                              Tool.execute(args)
                                       ↓
                              Return result
```

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `MCP_MAX_TOOLS` | 50 | Max tools to keep loaded |
| `MCP_IDLE_SECS` | 300 | Idle time before eviction |
| `MCP_DBUS_DISCOVERY` | true | Enable D-Bus discovery |
| `MCP_PLUGIN_DISCOVERY` | true | Enable plugin discovery |
| `MCP_AGENT_DISCOVERY` | true | Enable agent discovery |
| `MCP_PRELOAD` | true | Preload essential tools |

## Benefits

1. **Memory Efficient**: Only loads tools when needed
2. **Fast Startup**: No upfront tool loading
3. **Scalable**: Supports thousands of tools
4. **Context-Aware**: Returns relevant tools based on context
5. **Extensible**: Easy to add new discovery sources
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/proto/internal_agents.proto">
// Internal Agent Communication Protocol
//
// This is for INTERNAL use between MCP gateway and agents.
// External clients still use HTTP/SSE/stdio.

syntax = "proto3";

package op_agents;

// Agent Lifecycle Service
// Used by MCP server to manage run-on-connection agents
service AgentLifecycle {
    // Start an agent (called on client connect)
    rpc Start(StartRequest) returns (StartResponse);
    
    // Stop an agent (called on client disconnect)
    rpc Stop(StopRequest) returns (StopResponse);
    
    // Health check
    rpc Health(HealthRequest) returns (HealthResponse);
    
    // Watch agent status (bidirectional streaming)
    rpc WatchStatus(WatchRequest) returns (stream AgentStatus);
}

// Agent Execution Service
// High-performance tool execution
service AgentExecution {
    // Execute single operation
    rpc Execute(ExecuteRequest) returns (ExecuteResponse);
    
    // Batch execute multiple operations
    rpc BatchExecute(stream ExecuteRequest) returns (stream ExecuteResponse);
    
    // Streaming execution (for long-running operations)
    rpc StreamExecute(ExecuteRequest) returns (stream ExecuteChunk);
}

// Memory Service
// Dedicated service for memory agent (high frequency)
service MemoryService {
    rpc Remember(RememberRequest) returns (RememberResponse);
    rpc Recall(RecallRequest) returns (RecallResponse);
    rpc Forget(ForgetRequest) returns (ForgetResponse);
    rpc List(ListRequest) returns (ListResponse);
    rpc Search(SearchRequest) returns (SearchResponse);
    
    // Bulk operations
    rpc BulkRemember(stream RememberRequest) returns (BulkResponse);
}

// Sequential Thinking Service
// Dedicated for cognitive chain operations
service SequentialThinkingService {
    // Start a thinking chain
    rpc StartChain(ChainRequest) returns (ChainResponse);
    
    // Add thought to chain
    rpc AddThought(ThoughtRequest) returns (ThoughtResponse);
    
    // Stream thinking process
    rpc StreamThinking(ChainRequest) returns (stream ThoughtResponse);
    
    // Conclude chain
    rpc Conclude(ConcludeRequest) returns (ConcludeResponse);
}

// Context Manager Service
service ContextManagerService {
    rpc Save(SaveContextRequest) returns (SaveContextResponse);
    rpc Load(LoadContextRequest) returns (LoadContextResponse);
    rpc List(ListContextRequest) returns (ListContextResponse);
    rpc Delete(DeleteContextRequest) returns (DeleteContextResponse);
    rpc Export(ExportRequest) returns (stream ExportChunk);
    rpc Import(stream ImportChunk) returns (ImportResponse);
}

// Rust Pro Service
// Dedicated for cargo operations
service RustProService {
    rpc Check(CargoRequest) returns (CargoResponse);
    rpc Build(CargoRequest) returns (stream CargoOutput);
    rpc Test(CargoRequest) returns (stream CargoOutput);
    rpc Clippy(CargoRequest) returns (stream CargoOutput);
    rpc Format(CargoRequest) returns (CargoResponse);
    rpc Doc(CargoRequest) returns (stream CargoOutput);
}

// ============ Messages ============

message StartRequest {
    string agent_id = 1;
    map<string, string> config = 2;
}

message StartResponse {
    bool success = 1;
    string message = 2;
    int64 started_at = 3;
}

message StopRequest {
    string agent_id = 1;
    bool force = 2;
}

message StopResponse {
    bool success = 1;
    string message = 2;
}

message HealthRequest {
    string agent_id = 1;
}

message HealthResponse {
    bool healthy = 1;
    string status = 2;
    int64 uptime_secs = 3;
    map<string, string> metrics = 4;
}

message WatchRequest {
    repeated string agent_ids = 1;
}

message AgentStatus {
    string agent_id = 1;
    string status = 2;  // starting, running, stopping, stopped, error
    int64 timestamp = 3;
    map<string, string> metadata = 4;
}

message ExecuteRequest {
    string agent_id = 1;
    string operation = 2;
    string arguments_json = 3;
    int64 timeout_ms = 4;
    string correlation_id = 5;
}

message ExecuteResponse {
    bool success = 1;
    string result_json = 2;
    string error = 3;
    int64 execution_time_ms = 4;
    string correlation_id = 5;
}

message ExecuteChunk {
    string correlation_id = 1;
    string chunk = 2;
    bool is_final = 3;
    bool is_error = 4;
}

// Memory messages
message RememberRequest {
    string key = 1;
    string value = 2;
    int64 ttl_secs = 3;  // 0 = no expiry
    repeated string tags = 4;
}

message RememberResponse {
    bool success = 1;
    string key = 2;
}

message RecallRequest {
    string key = 1;
}

message RecallResponse {
    bool found = 1;
    string key = 2;
    string value = 3;
    int64 created_at = 4;
    int64 accessed_at = 5;
}

message ForgetRequest {
    string key = 1;
}

message ForgetResponse {
    bool success = 1;
}

message ListRequest {
    string pattern = 1;  // glob pattern
    int32 limit = 2;
}

message ListResponse {
    repeated string keys = 1;
    int32 total = 2;
}

message SearchRequest {
    string query = 1;
    int32 limit = 2;
}

message SearchResponse {
    repeated MemoryEntry entries = 1;
}

message MemoryEntry {
    string key = 1;
    string value = 2;
    float score = 3;
}

message BulkResponse {
    int32 success_count = 1;
    int32 failure_count = 2;
    repeated string errors = 3;
}

// Sequential thinking messages
message ChainRequest {
    string problem = 1;
    int32 max_steps = 2;
    string context = 3;
}

message ChainResponse {
    string chain_id = 1;
    bool started = 2;
}

message ThoughtRequest {
    string chain_id = 1;
    string thought = 2;
    int32 step = 3;
}

message ThoughtResponse {
    string chain_id = 1;
    int32 step = 2;
    string thought = 3;
    string status = 4;  // thinking, analyzing, concluding, complete
    bool is_final = 5;
}

message ConcludeRequest {
    string chain_id = 1;
}

message ConcludeResponse {
    string chain_id = 1;
    string conclusion = 2;
    repeated string steps = 3;
    int32 total_steps = 4;
}

// Context messages
message SaveContextRequest {
    string name = 1;
    string content = 2;
    repeated string tags = 3;
}

message SaveContextResponse {
    bool success = 1;
    string name = 2;
    int64 size_bytes = 3;
}

message LoadContextRequest {
    string name = 1;
}

message LoadContextResponse {
    bool found = 1;
    string name = 2;
    string content = 3;
    repeated string tags = 4;
    int64 created_at = 5;
    int64 updated_at = 6;
}

message ListContextRequest {
    string tag_filter = 1;
}

message ListContextResponse {
    repeated ContextInfo contexts = 1;
}

message ContextInfo {
    string name = 1;
    int64 size_bytes = 2;
    repeated string tags = 3;
    int64 updated_at = 4;
}

message DeleteContextRequest {
    string name = 1;
}

message DeleteContextResponse {
    bool success = 1;
}

message ExportRequest {
    repeated string names = 1;  // empty = all
    string format = 2;  // json, yaml
}

message ExportChunk {
    bytes data = 1;
    bool is_final = 2;
}

message ImportChunk {
    bytes data = 1;
    bool is_final = 2;
    string format = 3;
}

message ImportResponse {
    int32 imported_count = 1;
    repeated string errors = 2;
}

// Cargo/Rust messages
message CargoRequest {
    string path = 1;
    bool release = 2;
    repeated string features = 3;
    string filter = 4;  // for tests
    bool fix = 5;  // for clippy/fmt
    int64 timeout_secs = 6;
}

message CargoResponse {
    bool success = 1;
    string stdout = 2;
    string stderr = 3;
    int32 exit_code = 4;
    int64 duration_ms = 5;
}

message CargoOutput {
    string line = 1;
    string stream = 2;  // stdout, stderr
    bool is_final = 3;
    int32 exit_code = 4;  // only set when is_final
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/proto/mcp.proto">
syntax = "proto3";

package op.mcp.v1;

import "google/protobuf/struct.proto";
import "google/protobuf/empty.proto";

// MCP Service - Structured gRPC
service McpService {
  // Unary request/response (standard MCP calls)
  rpc Call(McpRequest) returns (McpResponse);
  
  // Server streaming for SSE-like behavior
  rpc Subscribe(SubscribeRequest) returns (stream McpEvent);
  
  // Bidirectional streaming for full duplex
  rpc Stream(stream McpRequest) returns (stream McpResponse);
  
  // Health check
  rpc Health(google.protobuf.Empty) returns (HealthResponse);
  
  // Initialize session (run-on-connection agents)
  rpc Initialize(InitializeRequest) returns (InitializeResponse);
  
  // Tool operations
  rpc ListTools(ListToolsRequest) returns (ListToolsResponse);
  rpc GetToolSchema(GetToolSchemaRequest) returns (GetToolSchemaResponse);
  rpc CallTool(CallToolRequest) returns (CallToolResponse);
  rpc CallToolStreaming(CallToolRequest) returns (stream ToolOutput);
}

// Generic MCP request with structured params
message McpRequest {
  string jsonrpc = 1;  // Always "2.0"
  optional string id = 2;
  string method = 3;
  optional google.protobuf.Struct params = 4;  // Structured params
}

// Generic MCP response with structured result
message McpResponse {
  string jsonrpc = 1;
  optional string id = 2;
  optional google.protobuf.Struct result = 3;  // Structured result
  optional McpError error = 4;
}

message McpError {
  int32 code = 1;
  string message = 2;
  optional google.protobuf.Struct data = 3;  // Structured error data
}

// Subscribe to server events
message SubscribeRequest {
  repeated string event_types = 1;  // "tools", "agents", "resources"
  optional string session_id = 2;
}

message McpEvent {
  string event_type = 1;
  string data_json = 2;
  int64 timestamp = 3;
  uint32 sequence = 4;
}

// Health check

message HealthResponse {
  bool healthy = 1;
  string version = 2;
  string server_name = 3;
  ServerMode mode = 4;
  repeated string connected_agents = 5;
  uint64 uptime_secs = 6;
}

enum ServerMode {
  SERVER_MODE_UNKNOWN = 0;
  SERVER_MODE_COMPACT = 1;
  SERVER_MODE_AGENTS = 2;
  SERVER_MODE_FULL = 3;
}

// Initialize session
message InitializeRequest {
  string client_name = 1;
  optional string client_version = 2;
  optional string session_id = 3;
  repeated string capabilities = 4;
}

message InitializeResponse {
  string protocol_version = 1;
  string server_name = 2;
  string server_version = 3;
  repeated string capabilities = 4;
  repeated string started_agents = 5;  // Run-on-connection agents
  string session_id = 6;
}

// Tool operations with structured arguments
message ListToolsRequest {
  optional string category = 1;
  optional string query = 2;
  uint32 limit = 3;
  uint32 offset = 4;
}

message ListToolsResponse {
  repeated ToolInfo tools = 1;
  uint32 total = 2;
  bool has_more = 3;
}

message ToolInfo {
  string name = 1;
  string description = 2;
  ToolSchema input_schema = 3;  // Structured schema
  optional string category = 4;
  repeated string tags = 5;
}

message ToolSchema {
  repeated ToolParameter parameters = 1;
  repeated string required = 2;
}

message ToolParameter {
  string name = 1;
  ParameterType type = 2;
  string description = 3;
  optional google.protobuf.Value default_value = 4;
  repeated string enum_values = 5;
}

enum ParameterType {
  PARAMETER_TYPE_STRING = 0;
  PARAMETER_TYPE_INTEGER = 1;
  PARAMETER_TYPE_NUMBER = 2;
  PARAMETER_TYPE_BOOLEAN = 3;
  PARAMETER_TYPE_ARRAY = 4;
  PARAMETER_TYPE_OBJECT = 5;
}

message GetToolSchemaRequest {
  string tool_name = 1;
}

message GetToolSchemaResponse {
  ToolSchema schema = 1;
}

// Tool execution with structured arguments
message CallToolRequest {
  string tool_name = 1;
  ToolArguments arguments = 2;  // Structured arguments
  optional string session_id = 3;
  optional uint32 timeout_ms = 4;
}

message ToolArguments {
  oneof args {
    FileSystemArgs filesystem = 1;
    NetworkArgs network = 2;
    DatabaseArgs database = 3;
    ShellArgs shell = 4;
    google.protobuf.Struct generic = 99;  // Fallback for unknown tools
  }
}

message FileSystemArgs {
  string path = 1;
  optional string content = 2;
  FileOperation operation = 3;
  optional FileMode mode = 4;
}

enum FileOperation {
  FILE_OPERATION_READ = 0;
  FILE_OPERATION_WRITE = 1;
  FILE_OPERATION_DELETE = 2;
  FILE_OPERATION_LIST = 3;
}

message FileMode {
  uint32 mode = 1;
}

message NetworkArgs {
  string url = 1;
  string method = 2;
  map<string, string> headers = 3;
  optional bytes body = 4;
}

message DatabaseArgs {
  string query = 1;
  map<string, string> parameters = 2;
}

message ShellArgs {
  string command = 1;
  repeated string args = 2;
  map<string, string> env = 3;
  optional string working_dir = 4;
}

message CallToolResponse {
  bool success = 1;
  google.protobuf.Struct result = 2;  // Structured result
  optional string error = 3;
  uint64 duration_ms = 4;
}

// Streaming tool output
message ToolOutput {
  OutputType output_type = 1;
  string content = 2;
  uint32 sequence = 3;
  bool is_final = 4;
  optional int32 exit_code = 5;
}

enum OutputType {
  OUTPUT_TYPE_UNKNOWN = 0;
  OUTPUT_TYPE_STDOUT = 1;
  OUTPUT_TYPE_STDERR = 2;
  OUTPUT_TYPE_PROGRESS = 3;
  OUTPUT_TYPE_RESULT = 4;
  OUTPUT_TYPE_ERROR = 5;
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/grpc/generated/.gitkeep">
# Generated protobuf code goes here
# Run `cargo build --features grpc` to generate
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/grpc/generated/op.mcp.v1.rs">
// This file is @generated by prost-build.
/// Generic MCP request with structured params
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct McpRequest {
    /// Always "2.0"
    #[prost(string, tag = "1")]
    pub jsonrpc: ::prost::alloc::string::String,
    #[prost(string, optional, tag = "2")]
    pub id: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, tag = "3")]
    pub method: ::prost::alloc::string::String,
    /// Structured params
    #[prost(message, optional, tag = "4")]
    pub params: ::core::option::Option<::prost_types::Struct>,
}
/// Generic MCP response with structured result
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct McpResponse {
    #[prost(string, tag = "1")]
    pub jsonrpc: ::prost::alloc::string::String,
    #[prost(string, optional, tag = "2")]
    pub id: ::core::option::Option<::prost::alloc::string::String>,
    /// Structured result
    #[prost(message, optional, tag = "3")]
    pub result: ::core::option::Option<::prost_types::Struct>,
    #[prost(message, optional, tag = "4")]
    pub error: ::core::option::Option<McpError>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct McpError {
    #[prost(int32, tag = "1")]
    pub code: i32,
    #[prost(string, tag = "2")]
    pub message: ::prost::alloc::string::String,
    /// Structured error data
    #[prost(message, optional, tag = "3")]
    pub data: ::core::option::Option<::prost_types::Struct>,
}
/// Subscribe to server events
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct SubscribeRequest {
    /// "tools", "agents", "resources"
    #[prost(string, repeated, tag = "1")]
    pub event_types: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, optional, tag = "2")]
    pub session_id: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct McpEvent {
    #[prost(string, tag = "1")]
    pub event_type: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub data_json: ::prost::alloc::string::String,
    #[prost(int64, tag = "3")]
    pub timestamp: i64,
    #[prost(uint32, tag = "4")]
    pub sequence: u32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct HealthResponse {
    #[prost(bool, tag = "1")]
    pub healthy: bool,
    #[prost(string, tag = "2")]
    pub version: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub server_name: ::prost::alloc::string::String,
    #[prost(enumeration = "ServerMode", tag = "4")]
    pub mode: i32,
    #[prost(string, repeated, tag = "5")]
    pub connected_agents: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(uint64, tag = "6")]
    pub uptime_secs: u64,
}
/// Initialize session
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InitializeRequest {
    #[prost(string, tag = "1")]
    pub client_name: ::prost::alloc::string::String,
    #[prost(string, optional, tag = "2")]
    pub client_version: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, optional, tag = "3")]
    pub session_id: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, repeated, tag = "4")]
    pub capabilities: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct InitializeResponse {
    #[prost(string, tag = "1")]
    pub protocol_version: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub server_name: ::prost::alloc::string::String,
    #[prost(string, tag = "3")]
    pub server_version: ::prost::alloc::string::String,
    #[prost(string, repeated, tag = "4")]
    pub capabilities: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    /// Run-on-connection agents
    #[prost(string, repeated, tag = "5")]
    pub started_agents: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(string, tag = "6")]
    pub session_id: ::prost::alloc::string::String,
}
/// Tool operations with structured arguments
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListToolsRequest {
    #[prost(string, optional, tag = "1")]
    pub category: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, optional, tag = "2")]
    pub query: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(uint32, tag = "3")]
    pub limit: u32,
    #[prost(uint32, tag = "4")]
    pub offset: u32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ListToolsResponse {
    #[prost(message, repeated, tag = "1")]
    pub tools: ::prost::alloc::vec::Vec<ToolInfo>,
    #[prost(uint32, tag = "2")]
    pub total: u32,
    #[prost(bool, tag = "3")]
    pub has_more: bool,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ToolInfo {
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub description: ::prost::alloc::string::String,
    /// Structured schema
    #[prost(message, optional, tag = "3")]
    pub input_schema: ::core::option::Option<ToolSchema>,
    #[prost(string, optional, tag = "4")]
    pub category: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(string, repeated, tag = "5")]
    pub tags: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ToolSchema {
    #[prost(message, repeated, tag = "1")]
    pub parameters: ::prost::alloc::vec::Vec<ToolParameter>,
    #[prost(string, repeated, tag = "2")]
    pub required: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ToolParameter {
    #[prost(string, tag = "1")]
    pub name: ::prost::alloc::string::String,
    #[prost(enumeration = "ParameterType", tag = "2")]
    pub r#type: i32,
    #[prost(string, tag = "3")]
    pub description: ::prost::alloc::string::String,
    #[prost(message, optional, tag = "4")]
    pub default_value: ::core::option::Option<::prost_types::Value>,
    #[prost(string, repeated, tag = "5")]
    pub enum_values: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetToolSchemaRequest {
    #[prost(string, tag = "1")]
    pub tool_name: ::prost::alloc::string::String,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct GetToolSchemaResponse {
    #[prost(message, optional, tag = "1")]
    pub schema: ::core::option::Option<ToolSchema>,
}
/// Tool execution with structured arguments
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CallToolRequest {
    #[prost(string, tag = "1")]
    pub tool_name: ::prost::alloc::string::String,
    /// Structured arguments
    #[prost(message, optional, tag = "2")]
    pub arguments: ::core::option::Option<ToolArguments>,
    #[prost(string, optional, tag = "3")]
    pub session_id: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(uint32, optional, tag = "4")]
    pub timeout_ms: ::core::option::Option<u32>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ToolArguments {
    #[prost(oneof = "tool_arguments::Args", tags = "1, 2, 3, 4, 99")]
    pub args: ::core::option::Option<tool_arguments::Args>,
}
/// Nested message and enum types in `ToolArguments`.
pub mod tool_arguments {
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Args {
        #[prost(message, tag = "1")]
        Filesystem(super::FileSystemArgs),
        #[prost(message, tag = "2")]
        Network(super::NetworkArgs),
        #[prost(message, tag = "3")]
        Database(super::DatabaseArgs),
        #[prost(message, tag = "4")]
        Shell(super::ShellArgs),
        /// Fallback for unknown tools
        #[prost(message, tag = "99")]
        Generic(::prost_types::Struct),
    }
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FileSystemArgs {
    #[prost(string, tag = "1")]
    pub path: ::prost::alloc::string::String,
    #[prost(string, optional, tag = "2")]
    pub content: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(enumeration = "FileOperation", tag = "3")]
    pub operation: i32,
    #[prost(message, optional, tag = "4")]
    pub mode: ::core::option::Option<FileMode>,
}
#[derive(Clone, Copy, PartialEq, ::prost::Message)]
pub struct FileMode {
    #[prost(uint32, tag = "1")]
    pub mode: u32,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct NetworkArgs {
    #[prost(string, tag = "1")]
    pub url: ::prost::alloc::string::String,
    #[prost(string, tag = "2")]
    pub method: ::prost::alloc::string::String,
    #[prost(map = "string, string", tag = "3")]
    pub headers: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        ::prost::alloc::string::String,
    >,
    #[prost(bytes = "vec", optional, tag = "4")]
    pub body: ::core::option::Option<::prost::alloc::vec::Vec<u8>>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct DatabaseArgs {
    #[prost(string, tag = "1")]
    pub query: ::prost::alloc::string::String,
    #[prost(map = "string, string", tag = "2")]
    pub parameters: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        ::prost::alloc::string::String,
    >,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ShellArgs {
    #[prost(string, tag = "1")]
    pub command: ::prost::alloc::string::String,
    #[prost(string, repeated, tag = "2")]
    pub args: ::prost::alloc::vec::Vec<::prost::alloc::string::String>,
    #[prost(map = "string, string", tag = "3")]
    pub env: ::std::collections::HashMap<
        ::prost::alloc::string::String,
        ::prost::alloc::string::String,
    >,
    #[prost(string, optional, tag = "4")]
    pub working_dir: ::core::option::Option<::prost::alloc::string::String>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CallToolResponse {
    #[prost(bool, tag = "1")]
    pub success: bool,
    /// Structured result
    #[prost(message, optional, tag = "2")]
    pub result: ::core::option::Option<::prost_types::Struct>,
    #[prost(string, optional, tag = "3")]
    pub error: ::core::option::Option<::prost::alloc::string::String>,
    #[prost(uint64, tag = "4")]
    pub duration_ms: u64,
}
/// Streaming tool output
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ToolOutput {
    #[prost(enumeration = "OutputType", tag = "1")]
    pub output_type: i32,
    #[prost(string, tag = "2")]
    pub content: ::prost::alloc::string::String,
    #[prost(uint32, tag = "3")]
    pub sequence: u32,
    #[prost(bool, tag = "4")]
    pub is_final: bool,
    #[prost(int32, optional, tag = "5")]
    pub exit_code: ::core::option::Option<i32>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ServerMode {
    Unknown = 0,
    Compact = 1,
    Agents = 2,
    Full = 3,
}
impl ServerMode {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::Unknown => "SERVER_MODE_UNKNOWN",
            Self::Compact => "SERVER_MODE_COMPACT",
            Self::Agents => "SERVER_MODE_AGENTS",
            Self::Full => "SERVER_MODE_FULL",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "SERVER_MODE_UNKNOWN" => Some(Self::Unknown),
            "SERVER_MODE_COMPACT" => Some(Self::Compact),
            "SERVER_MODE_AGENTS" => Some(Self::Agents),
            "SERVER_MODE_FULL" => Some(Self::Full),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum ParameterType {
    String = 0,
    Integer = 1,
    Number = 2,
    Boolean = 3,
    Array = 4,
    Object = 5,
}
impl ParameterType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::String => "PARAMETER_TYPE_STRING",
            Self::Integer => "PARAMETER_TYPE_INTEGER",
            Self::Number => "PARAMETER_TYPE_NUMBER",
            Self::Boolean => "PARAMETER_TYPE_BOOLEAN",
            Self::Array => "PARAMETER_TYPE_ARRAY",
            Self::Object => "PARAMETER_TYPE_OBJECT",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "PARAMETER_TYPE_STRING" => Some(Self::String),
            "PARAMETER_TYPE_INTEGER" => Some(Self::Integer),
            "PARAMETER_TYPE_NUMBER" => Some(Self::Number),
            "PARAMETER_TYPE_BOOLEAN" => Some(Self::Boolean),
            "PARAMETER_TYPE_ARRAY" => Some(Self::Array),
            "PARAMETER_TYPE_OBJECT" => Some(Self::Object),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum FileOperation {
    Read = 0,
    Write = 1,
    Delete = 2,
    List = 3,
}
impl FileOperation {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::Read => "FILE_OPERATION_READ",
            Self::Write => "FILE_OPERATION_WRITE",
            Self::Delete => "FILE_OPERATION_DELETE",
            Self::List => "FILE_OPERATION_LIST",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "FILE_OPERATION_READ" => Some(Self::Read),
            "FILE_OPERATION_WRITE" => Some(Self::Write),
            "FILE_OPERATION_DELETE" => Some(Self::Delete),
            "FILE_OPERATION_LIST" => Some(Self::List),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ::prost::Enumeration)]
#[repr(i32)]
pub enum OutputType {
    Unknown = 0,
    Stdout = 1,
    Stderr = 2,
    Progress = 3,
    Result = 4,
    Error = 5,
}
impl OutputType {
    /// String value of the enum field names used in the ProtoBuf definition.
    ///
    /// The values are not transformed in any way and thus are considered stable
    /// (if the ProtoBuf definition does not change) and safe for programmatic use.
    pub fn as_str_name(&self) -> &'static str {
        match self {
            Self::Unknown => "OUTPUT_TYPE_UNKNOWN",
            Self::Stdout => "OUTPUT_TYPE_STDOUT",
            Self::Stderr => "OUTPUT_TYPE_STDERR",
            Self::Progress => "OUTPUT_TYPE_PROGRESS",
            Self::Result => "OUTPUT_TYPE_RESULT",
            Self::Error => "OUTPUT_TYPE_ERROR",
        }
    }
    /// Creates an enum from field names used in the ProtoBuf definition.
    pub fn from_str_name(value: &str) -> ::core::option::Option<Self> {
        match value {
            "OUTPUT_TYPE_UNKNOWN" => Some(Self::Unknown),
            "OUTPUT_TYPE_STDOUT" => Some(Self::Stdout),
            "OUTPUT_TYPE_STDERR" => Some(Self::Stderr),
            "OUTPUT_TYPE_PROGRESS" => Some(Self::Progress),
            "OUTPUT_TYPE_RESULT" => Some(Self::Result),
            "OUTPUT_TYPE_ERROR" => Some(Self::Error),
            _ => None,
        }
    }
}
/// Generated client implementations.
pub mod mcp_service_client {
    #![allow(
        unused_variables,
        dead_code,
        missing_docs,
        clippy::wildcard_imports,
        clippy::let_unit_value,
    )]
    use tonic::codegen::*;
    use tonic::codegen::http::Uri;
    /// MCP Service - Structured gRPC
    #[derive(Debug, Clone)]
    pub struct McpServiceClient<T> {
        inner: tonic::client::Grpc<T>,
    }
    impl McpServiceClient<tonic::transport::Channel> {
        /// Attempt to create a new client by connecting to a given endpoint.
        pub async fn connect<D>(dst: D) -> Result<Self, tonic::transport::Error>
        where
            D: TryInto<tonic::transport::Endpoint>,
            D::Error: Into<StdError>,
        {
            let conn = tonic::transport::Endpoint::new(dst)?.connect().await?;
            Ok(Self::new(conn))
        }
    }
    impl<T> McpServiceClient<T>
    where
        T: tonic::client::GrpcService<tonic::body::BoxBody>,
        T::Error: Into<StdError>,
        T::ResponseBody: Body<Data = Bytes> + std::marker::Send + 'static,
        <T::ResponseBody as Body>::Error: Into<StdError> + std::marker::Send,
    {
        pub fn new(inner: T) -> Self {
            let inner = tonic::client::Grpc::new(inner);
            Self { inner }
        }
        pub fn with_origin(inner: T, origin: Uri) -> Self {
            let inner = tonic::client::Grpc::with_origin(inner, origin);
            Self { inner }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> McpServiceClient<InterceptedService<T, F>>
        where
            F: tonic::service::Interceptor,
            T::ResponseBody: Default,
            T: tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
                Response = http::Response<
                    <T as tonic::client::GrpcService<tonic::body::BoxBody>>::ResponseBody,
                >,
            >,
            <T as tonic::codegen::Service<
                http::Request<tonic::body::BoxBody>,
            >>::Error: Into<StdError> + std::marker::Send + std::marker::Sync,
        {
            McpServiceClient::new(InterceptedService::new(inner, interceptor))
        }
        /// Compress requests with the given encoding.
        ///
        /// This requires the server to support it otherwise it might respond with an
        /// error.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.send_compressed(encoding);
            self
        }
        /// Enable decompressing responses.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.inner = self.inner.accept_compressed(encoding);
            self
        }
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_decoding_message_size(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.inner = self.inner.max_encoding_message_size(limit);
            self
        }
        /// Unary request/response (standard MCP calls)
        pub async fn call(
            &mut self,
            request: impl tonic::IntoRequest<super::McpRequest>,
        ) -> std::result::Result<tonic::Response<super::McpResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::unknown(
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/op.mcp.v1.McpService/Call",
            );
            let mut req = request.into_request();
            req.extensions_mut().insert(GrpcMethod::new("op.mcp.v1.McpService", "Call"));
            self.inner.unary(req, path, codec).await
        }
        /// Server streaming for SSE-like behavior
        pub async fn subscribe(
            &mut self,
            request: impl tonic::IntoRequest<super::SubscribeRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::McpEvent>>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::unknown(
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/op.mcp.v1.McpService/Subscribe",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("op.mcp.v1.McpService", "Subscribe"));
            self.inner.server_streaming(req, path, codec).await
        }
        /// Bidirectional streaming for full duplex
        pub async fn stream(
            &mut self,
            request: impl tonic::IntoStreamingRequest<Message = super::McpRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::McpResponse>>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::unknown(
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/op.mcp.v1.McpService/Stream",
            );
            let mut req = request.into_streaming_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("op.mcp.v1.McpService", "Stream"));
            self.inner.streaming(req, path, codec).await
        }
        /// Health check
        pub async fn health(
            &mut self,
            request: impl tonic::IntoRequest<()>,
        ) -> std::result::Result<tonic::Response<super::HealthResponse>, tonic::Status> {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::unknown(
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/op.mcp.v1.McpService/Health",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("op.mcp.v1.McpService", "Health"));
            self.inner.unary(req, path, codec).await
        }
        /// Initialize session (run-on-connection agents)
        pub async fn initialize(
            &mut self,
            request: impl tonic::IntoRequest<super::InitializeRequest>,
        ) -> std::result::Result<
            tonic::Response<super::InitializeResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::unknown(
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/op.mcp.v1.McpService/Initialize",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("op.mcp.v1.McpService", "Initialize"));
            self.inner.unary(req, path, codec).await
        }
        /// Tool operations
        pub async fn list_tools(
            &mut self,
            request: impl tonic::IntoRequest<super::ListToolsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ListToolsResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::unknown(
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/op.mcp.v1.McpService/ListTools",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("op.mcp.v1.McpService", "ListTools"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn get_tool_schema(
            &mut self,
            request: impl tonic::IntoRequest<super::GetToolSchemaRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetToolSchemaResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::unknown(
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/op.mcp.v1.McpService/GetToolSchema",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("op.mcp.v1.McpService", "GetToolSchema"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn call_tool(
            &mut self,
            request: impl tonic::IntoRequest<super::CallToolRequest>,
        ) -> std::result::Result<
            tonic::Response<super::CallToolResponse>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::unknown(
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/op.mcp.v1.McpService/CallTool",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("op.mcp.v1.McpService", "CallTool"));
            self.inner.unary(req, path, codec).await
        }
        pub async fn call_tool_streaming(
            &mut self,
            request: impl tonic::IntoRequest<super::CallToolRequest>,
        ) -> std::result::Result<
            tonic::Response<tonic::codec::Streaming<super::ToolOutput>>,
            tonic::Status,
        > {
            self.inner
                .ready()
                .await
                .map_err(|e| {
                    tonic::Status::unknown(
                        format!("Service was not ready: {}", e.into()),
                    )
                })?;
            let codec = tonic::codec::ProstCodec::default();
            let path = http::uri::PathAndQuery::from_static(
                "/op.mcp.v1.McpService/CallToolStreaming",
            );
            let mut req = request.into_request();
            req.extensions_mut()
                .insert(GrpcMethod::new("op.mcp.v1.McpService", "CallToolStreaming"));
            self.inner.server_streaming(req, path, codec).await
        }
    }
}
/// Generated server implementations.
pub mod mcp_service_server {
    #![allow(
        unused_variables,
        dead_code,
        missing_docs,
        clippy::wildcard_imports,
        clippy::let_unit_value,
    )]
    use tonic::codegen::*;
    /// Generated trait containing gRPC methods that should be implemented for use with McpServiceServer.
    #[async_trait]
    pub trait McpService: std::marker::Send + std::marker::Sync + 'static {
        /// Unary request/response (standard MCP calls)
        async fn call(
            &self,
            request: tonic::Request<super::McpRequest>,
        ) -> std::result::Result<tonic::Response<super::McpResponse>, tonic::Status>;
        /// Server streaming response type for the Subscribe method.
        type SubscribeStream: tonic::codegen::tokio_stream::Stream<
                Item = std::result::Result<super::McpEvent, tonic::Status>,
            >
            + std::marker::Send
            + 'static;
        /// Server streaming for SSE-like behavior
        async fn subscribe(
            &self,
            request: tonic::Request<super::SubscribeRequest>,
        ) -> std::result::Result<tonic::Response<Self::SubscribeStream>, tonic::Status>;
        /// Server streaming response type for the Stream method.
        type StreamStream: tonic::codegen::tokio_stream::Stream<
                Item = std::result::Result<super::McpResponse, tonic::Status>,
            >
            + std::marker::Send
            + 'static;
        /// Bidirectional streaming for full duplex
        async fn stream(
            &self,
            request: tonic::Request<tonic::Streaming<super::McpRequest>>,
        ) -> std::result::Result<tonic::Response<Self::StreamStream>, tonic::Status>;
        /// Health check
        async fn health(
            &self,
            request: tonic::Request<()>,
        ) -> std::result::Result<tonic::Response<super::HealthResponse>, tonic::Status>;
        /// Initialize session (run-on-connection agents)
        async fn initialize(
            &self,
            request: tonic::Request<super::InitializeRequest>,
        ) -> std::result::Result<
            tonic::Response<super::InitializeResponse>,
            tonic::Status,
        >;
        /// Tool operations
        async fn list_tools(
            &self,
            request: tonic::Request<super::ListToolsRequest>,
        ) -> std::result::Result<
            tonic::Response<super::ListToolsResponse>,
            tonic::Status,
        >;
        async fn get_tool_schema(
            &self,
            request: tonic::Request<super::GetToolSchemaRequest>,
        ) -> std::result::Result<
            tonic::Response<super::GetToolSchemaResponse>,
            tonic::Status,
        >;
        async fn call_tool(
            &self,
            request: tonic::Request<super::CallToolRequest>,
        ) -> std::result::Result<
            tonic::Response<super::CallToolResponse>,
            tonic::Status,
        >;
        /// Server streaming response type for the CallToolStreaming method.
        type CallToolStreamingStream: tonic::codegen::tokio_stream::Stream<
                Item = std::result::Result<super::ToolOutput, tonic::Status>,
            >
            + std::marker::Send
            + 'static;
        async fn call_tool_streaming(
            &self,
            request: tonic::Request<super::CallToolRequest>,
        ) -> std::result::Result<
            tonic::Response<Self::CallToolStreamingStream>,
            tonic::Status,
        >;
    }
    /// MCP Service - Structured gRPC
    #[derive(Debug)]
    pub struct McpServiceServer<T> {
        inner: Arc<T>,
        accept_compression_encodings: EnabledCompressionEncodings,
        send_compression_encodings: EnabledCompressionEncodings,
        max_decoding_message_size: Option<usize>,
        max_encoding_message_size: Option<usize>,
    }
    impl<T> McpServiceServer<T> {
        pub fn new(inner: T) -> Self {
            Self::from_arc(Arc::new(inner))
        }
        pub fn from_arc(inner: Arc<T>) -> Self {
            Self {
                inner,
                accept_compression_encodings: Default::default(),
                send_compression_encodings: Default::default(),
                max_decoding_message_size: None,
                max_encoding_message_size: None,
            }
        }
        pub fn with_interceptor<F>(
            inner: T,
            interceptor: F,
        ) -> InterceptedService<Self, F>
        where
            F: tonic::service::Interceptor,
        {
            InterceptedService::new(Self::new(inner), interceptor)
        }
        /// Enable decompressing requests with the given encoding.
        #[must_use]
        pub fn accept_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.accept_compression_encodings.enable(encoding);
            self
        }
        /// Compress responses with the given encoding, if the client supports it.
        #[must_use]
        pub fn send_compressed(mut self, encoding: CompressionEncoding) -> Self {
            self.send_compression_encodings.enable(encoding);
            self
        }
        /// Limits the maximum size of a decoded message.
        ///
        /// Default: `4MB`
        #[must_use]
        pub fn max_decoding_message_size(mut self, limit: usize) -> Self {
            self.max_decoding_message_size = Some(limit);
            self
        }
        /// Limits the maximum size of an encoded message.
        ///
        /// Default: `usize::MAX`
        #[must_use]
        pub fn max_encoding_message_size(mut self, limit: usize) -> Self {
            self.max_encoding_message_size = Some(limit);
            self
        }
    }
    impl<T, B> tonic::codegen::Service<http::Request<B>> for McpServiceServer<T>
    where
        T: McpService,
        B: Body + std::marker::Send + 'static,
        B::Error: Into<StdError> + std::marker::Send + 'static,
    {
        type Response = http::Response<tonic::body::BoxBody>;
        type Error = std::convert::Infallible;
        type Future = BoxFuture<Self::Response, Self::Error>;
        fn poll_ready(
            &mut self,
            _cx: &mut Context<'_>,
        ) -> Poll<std::result::Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, req: http::Request<B>) -> Self::Future {
            match req.uri().path() {
                "/op.mcp.v1.McpService/Call" => {
                    #[allow(non_camel_case_types)]
                    struct CallSvc<T: McpService>(pub Arc<T>);
                    impl<T: McpService> tonic::server::UnaryService<super::McpRequest>
                    for CallSvc<T> {
                        type Response = super::McpResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::McpRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as McpService>::call(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let method = CallSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/op.mcp.v1.McpService/Subscribe" => {
                    #[allow(non_camel_case_types)]
                    struct SubscribeSvc<T: McpService>(pub Arc<T>);
                    impl<
                        T: McpService,
                    > tonic::server::ServerStreamingService<super::SubscribeRequest>
                    for SubscribeSvc<T> {
                        type Response = super::McpEvent;
                        type ResponseStream = T::SubscribeStream;
                        type Future = BoxFuture<
                            tonic::Response<Self::ResponseStream>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::SubscribeRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as McpService>::subscribe(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let method = SubscribeSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.server_streaming(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/op.mcp.v1.McpService/Stream" => {
                    #[allow(non_camel_case_types)]
                    struct StreamSvc<T: McpService>(pub Arc<T>);
                    impl<
                        T: McpService,
                    > tonic::server::StreamingService<super::McpRequest>
                    for StreamSvc<T> {
                        type Response = super::McpResponse;
                        type ResponseStream = T::StreamStream;
                        type Future = BoxFuture<
                            tonic::Response<Self::ResponseStream>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<tonic::Streaming<super::McpRequest>>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as McpService>::stream(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let method = StreamSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.streaming(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/op.mcp.v1.McpService/Health" => {
                    #[allow(non_camel_case_types)]
                    struct HealthSvc<T: McpService>(pub Arc<T>);
                    impl<T: McpService> tonic::server::UnaryService<()>
                    for HealthSvc<T> {
                        type Response = super::HealthResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(&mut self, request: tonic::Request<()>) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as McpService>::health(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let method = HealthSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/op.mcp.v1.McpService/Initialize" => {
                    #[allow(non_camel_case_types)]
                    struct InitializeSvc<T: McpService>(pub Arc<T>);
                    impl<
                        T: McpService,
                    > tonic::server::UnaryService<super::InitializeRequest>
                    for InitializeSvc<T> {
                        type Response = super::InitializeResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::InitializeRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as McpService>::initialize(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let method = InitializeSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/op.mcp.v1.McpService/ListTools" => {
                    #[allow(non_camel_case_types)]
                    struct ListToolsSvc<T: McpService>(pub Arc<T>);
                    impl<
                        T: McpService,
                    > tonic::server::UnaryService<super::ListToolsRequest>
                    for ListToolsSvc<T> {
                        type Response = super::ListToolsResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::ListToolsRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as McpService>::list_tools(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let method = ListToolsSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/op.mcp.v1.McpService/GetToolSchema" => {
                    #[allow(non_camel_case_types)]
                    struct GetToolSchemaSvc<T: McpService>(pub Arc<T>);
                    impl<
                        T: McpService,
                    > tonic::server::UnaryService<super::GetToolSchemaRequest>
                    for GetToolSchemaSvc<T> {
                        type Response = super::GetToolSchemaResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::GetToolSchemaRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as McpService>::get_tool_schema(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let method = GetToolSchemaSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/op.mcp.v1.McpService/CallTool" => {
                    #[allow(non_camel_case_types)]
                    struct CallToolSvc<T: McpService>(pub Arc<T>);
                    impl<
                        T: McpService,
                    > tonic::server::UnaryService<super::CallToolRequest>
                    for CallToolSvc<T> {
                        type Response = super::CallToolResponse;
                        type Future = BoxFuture<
                            tonic::Response<Self::Response>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::CallToolRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as McpService>::call_tool(&inner, request).await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let method = CallToolSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.unary(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                "/op.mcp.v1.McpService/CallToolStreaming" => {
                    #[allow(non_camel_case_types)]
                    struct CallToolStreamingSvc<T: McpService>(pub Arc<T>);
                    impl<
                        T: McpService,
                    > tonic::server::ServerStreamingService<super::CallToolRequest>
                    for CallToolStreamingSvc<T> {
                        type Response = super::ToolOutput;
                        type ResponseStream = T::CallToolStreamingStream;
                        type Future = BoxFuture<
                            tonic::Response<Self::ResponseStream>,
                            tonic::Status,
                        >;
                        fn call(
                            &mut self,
                            request: tonic::Request<super::CallToolRequest>,
                        ) -> Self::Future {
                            let inner = Arc::clone(&self.0);
                            let fut = async move {
                                <T as McpService>::call_tool_streaming(&inner, request)
                                    .await
                            };
                            Box::pin(fut)
                        }
                    }
                    let accept_compression_encodings = self.accept_compression_encodings;
                    let send_compression_encodings = self.send_compression_encodings;
                    let max_decoding_message_size = self.max_decoding_message_size;
                    let max_encoding_message_size = self.max_encoding_message_size;
                    let inner = self.inner.clone();
                    let fut = async move {
                        let method = CallToolStreamingSvc(inner);
                        let codec = tonic::codec::ProstCodec::default();
                        let mut grpc = tonic::server::Grpc::new(codec)
                            .apply_compression_config(
                                accept_compression_encodings,
                                send_compression_encodings,
                            )
                            .apply_max_message_size_config(
                                max_decoding_message_size,
                                max_encoding_message_size,
                            );
                        let res = grpc.server_streaming(method, req).await;
                        Ok(res)
                    };
                    Box::pin(fut)
                }
                _ => {
                    Box::pin(async move {
                        let mut response = http::Response::new(empty_body());
                        let headers = response.headers_mut();
                        headers
                            .insert(
                                tonic::Status::GRPC_STATUS,
                                (tonic::Code::Unimplemented as i32).into(),
                            );
                        headers
                            .insert(
                                http::header::CONTENT_TYPE,
                                tonic::metadata::GRPC_CONTENT_TYPE,
                            );
                        Ok(response)
                    })
                }
            }
        }
    }
    impl<T> Clone for McpServiceServer<T> {
        fn clone(&self) -> Self {
            let inner = self.inner.clone();
            Self {
                inner,
                accept_compression_encodings: self.accept_compression_encodings,
                send_compression_encodings: self.send_compression_encodings,
                max_decoding_message_size: self.max_decoding_message_size,
                max_encoding_message_size: self.max_encoding_message_size,
            }
        }
    }
    /// Generated gRPC service name
    pub const SERVICE_NAME: &str = "op.mcp.v1.McpService";
    impl<T> tonic::server::NamedService for McpServiceServer<T> {
        const NAME: &'static str = SERVICE_NAME;
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/grpc/client.rs">
//! gRPC Client for MCP

#[cfg(feature = "grpc")]
use crate::grpc::proto::mcp_service_client::McpServiceClient;
#[cfg(feature = "grpc")]
use crate::grpc::proto::*;
use anyhow::Result;
use prost_types::{ListValue as ProstListValue, Struct as ProstStruct, Value as ProstValue};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::BTreeMap;
use std::time::Duration;
#[cfg(feature = "grpc")]
use tonic::transport::{Channel, Endpoint};
use tracing::info;

/// gRPC client configuration
#[derive(Debug, Clone)]
pub struct GrpcClientConfig {
    pub endpoint: String,
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    pub tls_enabled: bool,
    pub tls_domain: Option<String>,
}

impl Default for GrpcClientConfig {
    fn default() -> Self {
        Self {
            endpoint: "http://[::1]:50051".to_string(),
            connect_timeout: Duration::from_secs(5),
            request_timeout: Duration::from_secs(30),
            tls_enabled: false,
            tls_domain: None,
        }
    }
}

impl GrpcClientConfig {
    pub fn with_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.endpoint = endpoint.into();
        self
    }

    pub fn with_tls(mut self, domain: Option<String>) -> Self {
        self.tls_enabled = true;
        self.tls_domain = domain;
        self
    }
}

/// gRPC client for MCP server
#[cfg(feature = "grpc")]
pub struct GrpcClient {
    client: McpServiceClient<Channel>,
    session_id: Option<String>,
}

#[cfg(feature = "grpc")]
impl GrpcClient {
    pub async fn connect(config: GrpcClientConfig) -> Result<Self> {
        info!(endpoint = %config.endpoint, "Connecting to gRPC MCP server");

        let endpoint = Endpoint::from_shared(config.endpoint.clone())?
            .connect_timeout(config.connect_timeout)
            .timeout(config.request_timeout);

        let channel = endpoint.connect().await?;
        let client = McpServiceClient::new(channel);

        Ok(Self {
            client,
            session_id: None,
        })
    }

    pub async fn connect_default() -> Result<Self> {
        Self::connect(GrpcClientConfig::default()).await
    }

    pub async fn initialize(&mut self, client_name: &str) -> Result<InitializeResponse> {
        let request = InitializeRequest {
            client_name: client_name.to_string(),
            client_version: Some(env!("CARGO_PKG_VERSION").to_string()),
            session_id: None,
            capabilities: vec!["tools".to_string()],
        };

        let response = self.client.initialize(request).await?.into_inner();
        self.session_id = Some(response.session_id.clone());

        info!(
            session = %response.session_id,
            agents = ?response.started_agents,
            "Session initialized"
        );

        Ok(response)
    }

    pub async fn health(&mut self) -> Result<HealthResponse> {
        let response = self.client.health(()).await?.into_inner();
        Ok(response)
    }

    pub async fn list_tools(
        &mut self,
        category: Option<&str>,
        query: Option<&str>,
        limit: u32,
    ) -> Result<ListToolsResponse> {
        let request = ListToolsRequest {
            category: category.map(String::from),
            query: query.map(String::from),
            limit,
            offset: 0,
        };

        let response = self.client.list_tools(request).await?.into_inner();
        Ok(response)
    }

    pub async fn call_tool(
        &mut self,
        tool_name: &str,
        arguments: Value,
    ) -> Result<CallToolResponse> {
        let request = CallToolRequest {
            tool_name: tool_name.to_string(),
            arguments: simd_to_prost_struct(&arguments)
                .ok()
                .map(|s| ToolArguments {
                    args: Some(tool_arguments::Args::Generic(s)),
                }),
            session_id: self.session_id.clone(),
            timeout_ms: None,
        };

        let response = self.client.call_tool(request).await?.into_inner();
        Ok(response)
    }

    pub async fn call_tool_streaming(
        &mut self,
        tool_name: &str,
        arguments: Value,
    ) -> Result<impl futures::Stream<Item = Result<ToolOutput, tonic::Status>>> {
        let request = CallToolRequest {
            tool_name: tool_name.to_string(),
            arguments: simd_to_prost_struct(&arguments)
                .ok()
                .map(|s| ToolArguments {
                    args: Some(tool_arguments::Args::Generic(s)),
                }),
            session_id: self.session_id.clone(),
            timeout_ms: None,
        };

        let response = self.client.call_tool_streaming(request).await?;
        Ok(response.into_inner())
    }

    pub async fn subscribe(
        &mut self,
        event_types: Vec<String>,
    ) -> Result<impl futures::Stream<Item = Result<McpEvent, tonic::Status>>> {
        let request = SubscribeRequest {
            event_types,
            session_id: self.session_id.clone(),
        };

        let response = self.client.subscribe(request).await?;
        Ok(response.into_inner())
    }

    pub async fn call_raw(&mut self, method: &str, params: Option<Value>) -> Result<McpResponse> {
        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(uuid::Uuid::new_v4().to_string()),
            method: method.to_string(),
            params: params.and_then(|p| simd_to_prost_struct(&p).ok()),
        };

        let response = self.client.call(request).await?.into_inner();
        Ok(response)
    }

    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }
}

// Minimal conversion helper for client
fn simd_to_prost_struct(value: &Value) -> Result<ProstStruct> {
    if let Some(obj) = value.as_object() {
        let fields: BTreeMap<String, ProstValue> = obj
            .iter()
            .map(|(k, v): (&String, &Value)| (k.clone(), simd_to_prost_value(v)))
            .collect();
        Ok(ProstStruct { fields })
    } else {
        Err(anyhow::anyhow!("Value is not an object"))
    }
}

fn simd_to_prost_value(value: &Value) -> ProstValue {
    use prost_types::value::Kind;
    match value {
        v if v.is_null() => ProstValue {
            kind: Some(Kind::NullValue(0)),
        },
        v if v.is_bool() => ProstValue {
            kind: Some(Kind::BoolValue(v.as_bool().unwrap())),
        },
        v if v.is_str() => ProstValue {
            kind: Some(Kind::StringValue(v.as_str().unwrap().to_string())),
        },
        v if v.is_f64() => ProstValue {
            kind: Some(Kind::NumberValue(v.as_f64().unwrap())),
        },
        v if v.is_i64() => ProstValue {
            kind: Some(Kind::NumberValue(v.as_i64().unwrap() as f64)),
        },
        v if v.is_u64() => ProstValue {
            kind: Some(Kind::NumberValue(v.as_u64().unwrap() as f64)),
        },
        v if v.is_array() => ProstValue {
            kind: Some(Kind::ListValue(ProstListValue {
                values: v
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(simd_to_prost_value)
                    .collect(),
            })),
        },
        v if v.is_object() => {
            let fields: BTreeMap<String, ProstValue> = v
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.to_string(), simd_to_prost_value(v)))
                .collect();
            ProstValue {
                kind: Some(Kind::StructValue(ProstStruct { fields })),
            }
        }
        _ => ProstValue {
            kind: Some(Kind::NullValue(0)),
        },
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/grpc/mod.rs">
//! gRPC Transport for op-mcp
//!
//! Provides high-performance gRPC transport for MCP protocol.
//!
//! ## Features
//! - Unary request/response (standard MCP calls)
//! - Server streaming (SSE-like events)
//! - Bidirectional streaming (full duplex)
//! - Run-on-connection agent support
//! - BTRFS cache integration
//! - StateStore execution tracking
//! - Snowball audit trail

#[cfg(feature = "grpc")]
mod client;
#[cfg(feature = "grpc")]
mod server;
#[cfg(feature = "grpc")]
mod service;

#[cfg(feature = "grpc")]
pub use crate::ServerMode as GrpcServerMode; // Direct export from crate root
#[cfg(feature = "grpc")]
pub use client::{GrpcClient, GrpcClientConfig};
#[cfg(feature = "grpc")]
pub use server::{GrpcConfig, GrpcTransport};
#[cfg(feature = "grpc")]
pub use service::{GrpcInfrastructure, McpGrpcService};

// Include generated protobuf code
#[cfg(feature = "grpc")]
pub mod proto {
    include!("generated/op.mcp.v1.rs");
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/grpc/server.rs">
//! gRPC Server Transport with Infrastructure Integration

#[cfg(feature = "grpc")]
use crate::grpc::proto::mcp_service_server::McpServiceServer;
#[cfg(feature = "grpc")]
use crate::grpc::service::{GrpcInfrastructure, McpGrpcService};
use crate::ServerMode; // Unified ServerMode from lib.rs
use anyhow::Result;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
#[cfg(feature = "grpc")]
use tonic::transport::Server;
use tracing::{error, info};

/// gRPC transport configuration
#[derive(Debug, Clone)]
pub struct GrpcConfig {
    pub address: SocketAddr,
    pub mode: ServerMode,
    pub tls_enabled: bool,
    pub tls_cert_path: Option<String>,
    pub tls_key_path: Option<String>,
    pub request_timeout: Duration,
    pub max_message_size: usize,
    pub enable_reflection: bool,
    pub enable_health: bool,
    pub max_concurrent_streams: u32,
    pub keepalive_interval: Duration,
    pub keepalive_timeout: Duration,
    pub cache_path: Option<PathBuf>,
    pub state_db_path: Option<PathBuf>,
    pub snowball_path: Option<PathBuf>,
}

impl Default for GrpcConfig {
    fn default() -> Self {
        Self {
            address: "[::1]:50051".parse().unwrap(),
            mode: ServerMode::Compact,
            tls_enabled: false,
            tls_cert_path: None,
            tls_key_path: None,
            request_timeout: Duration::from_secs(30),
            max_message_size: 16 * 1024 * 1024,
            enable_reflection: true,
            enable_health: true,
            max_concurrent_streams: 100,
            keepalive_interval: Duration::from_secs(30),
            keepalive_timeout: Duration::from_secs(10),
            cache_path: Some(PathBuf::from("/var/lib/op-dbus/cache/grpc")),
            state_db_path: Some(PathBuf::from("/var/lib/op-dbus/state/grpc.db")),
            snowball_path: Some(PathBuf::from("/var/lib/op-dbus/snowball/grpc")),
        }
    }
}

impl GrpcConfig {
    pub fn with_address(mut self, addr: SocketAddr) -> Self {
        self.address = addr;
        self
    }

    pub fn with_mode(mut self, mode: ServerMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_tls(mut self, cert_path: impl Into<String>, key_path: impl Into<String>) -> Self {
        self.tls_enabled = true;
        self.tls_cert_path = Some(cert_path.into());
        self.tls_key_path = Some(key_path.into());
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    pub fn with_cache_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.cache_path = Some(path.into());
        self
    }

    pub fn with_state_db_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.state_db_path = Some(path.into());
        self
    }

    pub fn with_snowball_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.snowball_path = Some(path.into());
        self
    }

    pub fn without_infrastructure(mut self) -> Self {
        self.cache_path = None;
        self.state_db_path = None;
        self.snowball_path = None;
        self
    }
}

/// gRPC transport for MCP server
#[cfg(feature = "grpc")]
pub struct GrpcTransport {
    config: GrpcConfig,
    service: McpGrpcService,
}

#[cfg(feature = "grpc")]
impl GrpcTransport {
    pub async fn new(config: GrpcConfig) -> Result<Self> {
        let infrastructure = GrpcInfrastructure::from_paths(
            config.cache_path.clone(),
            config.state_db_path.clone(),
            config.snowball_path.clone(),
        )
        .await?;

        let service = McpGrpcService::with_infrastructure(config.mode, infrastructure);

        Ok(Self { config, service })
    }

    pub async fn with_infrastructure(
        config: GrpcConfig,
        infrastructure: GrpcInfrastructure,
    ) -> Result<Self> {
        let service = McpGrpcService::with_infrastructure(config.mode, infrastructure);
        Ok(Self { config, service })
    }

    pub async fn with_defaults() -> Result<Self> {
        Self::new(GrpcConfig::default()).await
    }

    pub async fn without_infrastructure() -> Result<Self> {
        let config = GrpcConfig::default().without_infrastructure();
        let service = McpGrpcService::new(config.mode);
        Ok(Self { config, service })
    }

    pub async fn serve(self) -> Result<()> {
        let addr = self.config.address;

        info!(
            address = %addr,
            mode = %self.config.mode,
            tls = %self.config.tls_enabled,
            "Starting gRPC MCP server"
        );

        let mcp_service = McpServiceServer::new(self.service)
            .max_decoding_message_size(self.config.max_message_size)
            .max_encoding_message_size(self.config.max_message_size);

        Server::builder()
            .timeout(self.config.request_timeout)
            .max_concurrent_streams(self.config.max_concurrent_streams)
            .http2_keepalive_interval(Some(self.config.keepalive_interval))
            .http2_keepalive_timeout(Some(self.config.keepalive_timeout))
            .add_service(mcp_service)
            .serve(addr)
            .await
            .map_err(|e| {
                error!(error = %e, "gRPC server error");
                anyhow::anyhow!("gRPC server error: {}", e)
            })?;

        Ok(())
    }

    pub async fn serve_with_shutdown<F>(self, shutdown: F) -> Result<()>
    where
        F: std::future::Future<Output = ()>,
    {
        let addr = self.config.address;

        info!(address = %addr, "Starting gRPC MCP server with graceful shutdown");

        let mcp_service = McpServiceServer::new(self.service)
            .max_decoding_message_size(self.config.max_message_size)
            .max_encoding_message_size(self.config.max_message_size);

        Server::builder()
            .timeout(self.config.request_timeout)
            .add_service(mcp_service)
            .serve_with_shutdown(addr, shutdown)
            .await?;

        info!("gRPC server shut down gracefully");
        Ok(())
    }
}

#[cfg(feature = "grpc")]
#[allow(dead_code)]
pub async fn run_grpc_server(config: GrpcConfig) -> Result<()> {
    let transport = GrpcTransport::new(config).await?;
    transport.serve().await
}

#[cfg(feature = "grpc")]
#[allow(dead_code)]
pub async fn run_grpc_server_lightweight(address: SocketAddr, mode: ServerMode) -> Result<()> {
    let _config = GrpcConfig::default()
        .with_address(address)
        .with_mode(mode)
        .without_infrastructure();

    let service = McpGrpcService::new(mode);

    info!(address = %address, mode = %mode, "Starting lightweight gRPC server");

    Server::builder()
        .add_service(McpServiceServer::new(service))
        .serve(address)
        .await?;

    Ok(())
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/grpc/service.rs">
//! MCP gRPC Service Implementation

#[cfg(feature = "grpc")]
use crate::grpc::proto::mcp_service_server::McpService;
#[cfg(feature = "grpc")]
use crate::grpc::proto::*;
use crate::ServerMode;
use anyhow::Result;
use prost_types::{ListValue as ProstListValue, Struct as ProstStruct, Value as ProstValue};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, mpsc, RwLock};
#[cfg(feature = "grpc")]
use tokio_stream::{wrappers::ReceiverStream, Stream, StreamExt};
#[cfg(feature = "grpc")]
use tonic::{Request, Response, Status};
use tracing::warn;
use uuid::Uuid;

const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "op-mcp-grpc";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(feature = "grpc")]
type ResponseStream<T> = Pin<Box<dyn Stream<Item = Result<T, Status>> + Send>>;

#[allow(dead_code)]
struct Session {
    id: String,
    client_name: String,
    started_agents: Vec<String>,
    created_at: Instant,
}

/// Infrastructure integrations
pub struct GrpcInfrastructure {
    pub cache_path: Option<PathBuf>,
    pub state_db_path: Option<PathBuf>,
    pub snowball_path: Option<PathBuf>,
    pub tool_registry: Option<Arc<op_tools::ToolRegistry>>,
}

impl Clone for GrpcInfrastructure {
    fn clone(&self) -> Self {
        Self {
            cache_path: self.cache_path.clone(),
            state_db_path: self.state_db_path.clone(),
            snowball_path: self.snowball_path.clone(),
            tool_registry: self.tool_registry.clone(),
        }
    }
}

impl Default for GrpcInfrastructure {
    fn default() -> Self {
        Self {
            cache_path: None,
            state_db_path: None,
            snowball_path: None,
            tool_registry: None,
        }
    }
}

impl GrpcInfrastructure {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn from_paths(
        _cache_path: Option<PathBuf>,
        _state_db_path: Option<PathBuf>,
        _snowball_path: Option<PathBuf>,
    ) -> Result<Self> {
        Ok(Self::default())
    }

    pub fn with_tool_registry(mut self, registry: Arc<op_tools::ToolRegistry>) -> Self {
        self.tool_registry = Some(registry);
        self
    }
}

#[allow(dead_code)]
pub struct McpGrpcService {
    mode: ServerMode,
    sessions: RwLock<HashMap<String, Session>>,
    start_time: Instant,
    request_counter: AtomicU64,
    error_counter: AtomicU64,
    infrastructure: GrpcInfrastructure,
    /// Broadcast channel for subscription events (tool executions, state changes)
    event_tx: broadcast::Sender<McpEvent>,
}

impl McpGrpcService {
    pub fn new(mode: ServerMode) -> Self {
        let (event_tx, _) = broadcast::channel(256);
        Self {
            mode,
            sessions: RwLock::new(HashMap::new()),
            start_time: Instant::now(),
            request_counter: AtomicU64::new(0),
            error_counter: AtomicU64::new(0),
            infrastructure: GrpcInfrastructure::default(),
            event_tx,
        }
    }

    pub fn with_infrastructure(mode: ServerMode, infrastructure: GrpcInfrastructure) -> Self {
        let (event_tx, _) = broadcast::channel(256);
        Self {
            mode,
            sessions: RwLock::new(HashMap::new()),
            start_time: Instant::now(),
            request_counter: AtomicU64::new(0),
            error_counter: AtomicU64::new(0),
            infrastructure,
            event_tx,
        }
    }

    #[allow(dead_code)]
    async fn start_session_agents(&self, session_id: &str, client_name: &str) -> Vec<String> {
        let started = Vec::new();
        let session = Session {
            id: session_id.to_string(),
            client_name: client_name.to_string(),
            started_agents: started.clone(),
            created_at: Instant::now(),
        };
        self.sessions
            .write()
            .await
            .insert(session_id.to_string(), session);
        started
    }

    fn mode_to_proto(&self) -> i32 {
        match self.mode {
            ServerMode::Compact => 1,
            ServerMode::Agents => 2,
            ServerMode::Full => 3,
        }
    }

    /// Build an McpServer instance using the current tool registry.
    fn build_mcp_server(&self) -> crate::server::McpServer {
        crate::server::McpServer::with_executor(
            crate::server::McpServerConfig::default(),
            Arc::new(crate::server::DefaultToolExecutor::new(
                self.infrastructure
                    .tool_registry
                    .clone()
                    .unwrap_or_else(|| Arc::new(op_tools::ToolRegistry::new())),
            )),
        )
    }

    /// Emit an event to all active subscribers. Failures (no receivers) are silently ignored.
    #[allow(dead_code)]
    fn emit_event(&self, event_type: &str, data_json: String) {
        let _ = self.event_tx.send(McpEvent {
            event_type: event_type.to_string(),
            data_json,
            timestamp: chrono::Utc::now().timestamp(),
            sequence: 0, // subscribers track their own sequence
        });
    }
}

// Helper: simd_json::Value -> prost_types::Value
fn simd_to_prost_value(value: &Value) -> ProstValue {
    use prost_types::value::Kind;
    match value {
        v if v.is_null() => ProstValue {
            kind: Some(Kind::NullValue(0)),
        },
        v if v.is_bool() => ProstValue {
            kind: Some(Kind::BoolValue(v.as_bool().unwrap())),
        },
        v if v.is_str() => ProstValue {
            kind: Some(Kind::StringValue(v.as_str().unwrap().to_string())),
        },
        v if v.is_f64() => ProstValue {
            kind: Some(Kind::NumberValue(v.as_f64().unwrap())),
        },
        v if v.is_i64() => ProstValue {
            kind: Some(Kind::NumberValue(v.as_i64().unwrap() as f64)),
        },
        v if v.is_u64() => ProstValue {
            kind: Some(Kind::NumberValue(v.as_u64().unwrap() as f64)),
        },
        v if v.is_array() => ProstValue {
            kind: Some(Kind::ListValue(ProstListValue {
                values: v
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(simd_to_prost_value)
                    .collect(),
            })),
        },
        v if v.is_object() => {
            let fields: BTreeMap<String, ProstValue> = v
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.to_string(), simd_to_prost_value(v)))
                .collect();
            ProstValue {
                kind: Some(Kind::StructValue(ProstStruct { fields })),
            }
        }
        _ => ProstValue {
            kind: Some(Kind::NullValue(0)),
        },
    }
}

// Helper: prost_types::Value -> simd_json::Value
fn prost_to_simd_value(value: &ProstValue) -> Value {
    use prost_types::value::Kind;
    match &value.kind {
        Some(Kind::NullValue(_)) => Value::from(()),
        Some(Kind::NumberValue(f)) => Value::from(*f),
        Some(Kind::StringValue(s)) => Value::from(s.clone()),
        Some(Kind::BoolValue(b)) => Value::from(*b),
        Some(Kind::StructValue(s)) => {
            let obj: HashMap<String, Value> = s
                .fields
                .iter()
                .map(|(k, v)| (k.clone(), prost_to_simd_value(v)))
                .collect();
            Value::from(obj)
        }
        Some(Kind::ListValue(l)) => {
            let arr: Vec<Value> = l.values.iter().map(prost_to_simd_value).collect();
            Value::from(arr)
        }
        None => Value::from(()),
    }
}

fn simd_to_prost_struct(value: &Value) -> Result<ProstStruct, Status> {
    if let Some(obj) = value.as_object() {
        let fields: BTreeMap<String, ProstValue> = obj
            .iter()
            .map(|(k, v): (&String, &Value)| (k.clone(), simd_to_prost_value(v)))
            .collect();
        Ok(ProstStruct { fields })
    } else {
        Err(Status::invalid_argument("Value is not an object"))
    }
}

#[cfg(feature = "grpc")]
#[tonic::async_trait]
impl McpService for McpGrpcService {
    async fn call(&self, request: Request<McpRequest>) -> Result<Response<McpResponse>, Status> {
        self.request_counter.fetch_add(1, Ordering::Relaxed);
        let proto_req = request.into_inner();

        let params_simd = proto_req.params.map(|p| {
            let obj: HashMap<String, Value> = p
                .fields
                .into_iter()
                .map(|(k, v)| (k, prost_to_simd_value(&v)))
                .collect();
            Value::from(obj)
        });

        let internal_req = crate::protocol::McpRequest {
            jsonrpc: "2.0".to_string(),
            id: proto_req.id.as_ref().map(|v| simd_json::json!(v)),
            method: proto_req.method.clone(),
            params: params_simd,
            meta: None,
        };

        let server = crate::server::McpServer::with_executor(
            crate::server::McpServerConfig::default(),
            Arc::new(crate::server::DefaultToolExecutor::new(
                self.infrastructure
                    .tool_registry
                    .clone()
                    .unwrap_or_else(|| Arc::new(op_tools::ToolRegistry::new())),
            )),
        );

        let internal_resp = server.handle_request(internal_req).await;

        Ok(Response::new(McpResponse {
            jsonrpc: "2.0".to_string(),
            id: proto_req.id,
            result: internal_resp
                .result
                .and_then(|v| simd_to_prost_struct(&v).ok()),
            error: internal_resp.error.map(|e| McpError {
                code: e.code,
                message: e.message,
                data: e.data.and_then(|v| simd_to_prost_struct(&v).ok()),
            }),
        }))
    }

    type SubscribeStream = ResponseStream<McpEvent>;

    async fn subscribe(
        &self,
        request: Request<SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let req = request.into_inner();
        let _session_id = req.session_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let event_filter: Vec<String> = req.event_types;
        let (tx, rx) = mpsc::channel(32);
        let mut event_rx = self.event_tx.subscribe();

        tokio::spawn(async move {
            let mut sequence = 0u32;
            let mut heartbeat = tokio::time::interval(Duration::from_secs(30));

            loop {
                tokio::select! {
                    // Forward real events from the broadcast channel
                    recv_result = event_rx.recv() => {
                        match recv_result {
                            Ok(mut event) => {
                                // Filter by requested event_types (empty = accept all)
                                if !event_filter.is_empty()
                                    && !event_filter.iter().any(|t| t == &event.event_type)
                                {
                                    continue;
                                }
                                event.sequence = sequence;
                                sequence += 1;
                                if tx.send(Ok(event)).await.is_err() {
                                    break; // client disconnected
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!("Subscription lagged, dropped {} events", n);
                                // Continue receiving, don't kill the stream
                            }
                            Err(broadcast::error::RecvError::Closed) => {
                                break; // sender side dropped
                            }
                        }
                    }
                    // Periodic heartbeat so the client knows the stream is alive
                    _ = heartbeat.tick() => {
                        let ping = McpEvent {
                            event_type: "ping".to_string(),
                            data_json: String::new(),
                            timestamp: chrono::Utc::now().timestamp(),
                            sequence,
                        };
                        sequence += 1;
                        if tx.send(Ok(ping)).await.is_err() {
                            break;
                        }
                    }
                }
            }
        });

        Ok(Response::new(
            Box::pin(ReceiverStream::new(rx)) as Self::SubscribeStream
        ))
    }

    type StreamStream = ResponseStream<McpResponse>;

    async fn stream(
        &self,
        request: Request<tonic::Streaming<McpRequest>>,
    ) -> Result<Response<Self::StreamStream>, Status> {
        let mut stream = request.into_inner();
        let (tx, rx) = mpsc::channel(32);
        let mcp_server = self.build_mcp_server();

        tokio::spawn(async move {
            while let Some(msg) = stream.next().await {
                let proto_req = match msg {
                    Ok(r) => r,
                    Err(e) => {
                        warn!("Stream recv error: {}", e);
                        break;
                    }
                };

                // Convert proto McpRequest -> internal McpRequest (same as `call`)
                let params_simd = proto_req.params.map(|p| {
                    let obj: HashMap<String, Value> = p
                        .fields
                        .into_iter()
                        .map(|(k, v)| (k, prost_to_simd_value(&v)))
                        .collect();
                    Value::from(obj)
                });

                let internal_req = crate::protocol::McpRequest {
                    jsonrpc: "2.0".to_string(),
                    id: proto_req.id.as_ref().map(|v| simd_json::json!(v)),
                    method: proto_req.method.clone(),
                    params: params_simd,
                    meta: None,
                };

                // Route through the MCP handler
                let internal_resp = mcp_server.handle_request(internal_req).await;

                let proto_resp = McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: proto_req.id,
                    result: internal_resp
                        .result
                        .and_then(|v| simd_to_prost_struct(&v).ok()),
                    error: internal_resp.error.map(|e| McpError {
                        code: e.code,
                        message: e.message,
                        data: e.data.and_then(|v| simd_to_prost_struct(&v).ok()),
                    }),
                };

                if tx.send(Ok(proto_resp)).await.is_err() {
                    break; // client disconnected
                }
            }
        });

        Ok(Response::new(
            Box::pin(ReceiverStream::new(rx)) as Self::StreamStream
        ))
    }

    async fn health(&self, _request: Request<()>) -> Result<Response<HealthResponse>, Status> {
        Ok(Response::new(HealthResponse {
            healthy: true,
            version: SERVER_VERSION.to_string(),
            server_name: SERVER_NAME.to_string(),
            mode: self.mode_to_proto(),
            connected_agents: vec![],
            uptime_secs: self.start_time.elapsed().as_secs(),
        }))
    }

    async fn initialize(
        &self,
        request: Request<InitializeRequest>,
    ) -> Result<Response<InitializeResponse>, Status> {
        let req = request.into_inner();
        let session_id = req.session_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        Ok(Response::new(InitializeResponse {
            protocol_version: PROTOCOL_VERSION.to_string(),
            server_name: SERVER_NAME.to_string(),
            server_version: SERVER_VERSION.to_string(),
            capabilities: vec!["tools".to_string()],
            started_agents: vec![],
            session_id,
        }))
    }

    async fn list_tools(
        &self,
        _request: Request<ListToolsRequest>,
    ) -> Result<Response<ListToolsResponse>, Status> {
        let tools = if let Some(ref registry) = self.infrastructure.tool_registry {
            let all = registry.list().await;
            all.into_iter()
                .map(|t| ToolInfo {
                    name: t.name,
                    description: t.description,
                    input_schema: Some(convert_json_schema_to_tool_schema(&t.input_schema)),
                    category: if t.category.is_empty() {
                        None
                    } else {
                        Some(t.category)
                    },
                    tags: t.tags,
                })
                .collect()
        } else {
            vec![]
        };

        Ok(Response::new(ListToolsResponse {
            tools,
            total: 0,
            has_more: false,
        }))
    }

    async fn call_tool(
        &self,
        request: Request<CallToolRequest>,
    ) -> Result<Response<CallToolResponse>, Status> {
        let req = request.into_inner();
        let start = Instant::now();

        let arguments = if let Some(ToolArguments {
            args: Some(tool_arguments::Args::Generic(s)),
        }) = req.arguments
        {
            let obj: HashMap<String, Value> = s
                .fields
                .into_iter()
                .map(|(k, v)| (k, prost_to_simd_value(&v)))
                .collect();
            Value::from(obj)
        } else {
            json!({})
        };

        let registry = self
            .infrastructure
            .tool_registry
            .clone()
            .ok_or_else(|| Status::internal("No tool registry"))?;
        let tool = registry
            .get(&req.tool_name)
            .await
            .ok_or_else(|| Status::not_found("Tool not found"))?;

        let result = tool
            .execute(arguments)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let result_struct = simd_to_prost_struct(&result).ok();

        Ok(Response::new(CallToolResponse {
            success: true,
            result: result_struct,
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
        }))
    }

    type CallToolStreamingStream = ResponseStream<ToolOutput>;

    async fn call_tool_streaming(
        &self,
        request: Request<CallToolRequest>,
    ) -> Result<Response<Self::CallToolStreamingStream>, Status> {
        let req = request.into_inner();
        let tool_name = req.tool_name.clone();

        let arguments = if let Some(ToolArguments {
            args: Some(tool_arguments::Args::Generic(s)),
        }) = req.arguments
        {
            let obj: HashMap<String, Value> = s
                .fields
                .into_iter()
                .map(|(k, v)| (k, prost_to_simd_value(&v)))
                .collect();
            Value::from(obj)
        } else {
            json!({})
        };

        let registry = self
            .infrastructure
            .tool_registry
            .clone()
            .ok_or_else(|| Status::internal("No tool registry"))?;
        let tool = registry
            .get(&tool_name)
            .await
            .ok_or_else(|| Status::not_found(format!("Tool not found: {}", tool_name)))?;

        let event_tx = self.event_tx.clone();
        let (tx, rx) = mpsc::channel(32);

        tokio::spawn(async move {
            // Send a progress message indicating execution has started
            let start_msg = ToolOutput {
                output_type: OutputType::Progress as i32,
                content: format!("Executing tool: {}", tool_name),
                sequence: 0,
                is_final: false,
                exit_code: None,
            };
            if tx.send(Ok(start_msg)).await.is_err() {
                return;
            }

            // Execute the tool
            match tool.execute(arguments).await {
                Ok(result) => {
                    let result_json =
                        simd_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());

                    // Send the final result
                    let output = ToolOutput {
                        output_type: OutputType::Result as i32,
                        content: result_json.clone(),
                        sequence: 1,
                        is_final: true,
                        exit_code: Some(0),
                    };
                    let _ = tx.send(Ok(output)).await;

                    // Emit event for subscribers
                    let _ = event_tx.send(McpEvent {
                        event_type: "tool.completed".to_string(),
                        data_json: simd_json::to_string(&simd_json::json!({
                            "tool": tool_name,
                            "success": true
                        }))
                        .unwrap_or_default(),
                        timestamp: chrono::Utc::now().timestamp(),
                        sequence: 0,
                    });
                }
                Err(e) => {
                    let error_output = ToolOutput {
                        output_type: OutputType::Error as i32,
                        content: e.to_string(),
                        sequence: 1,
                        is_final: true,
                        exit_code: Some(1),
                    };
                    let _ = tx.send(Ok(error_output)).await;

                    // Emit failure event for subscribers
                    let _ = event_tx.send(McpEvent {
                        event_type: "tool.failed".to_string(),
                        data_json: simd_json::to_string(&simd_json::json!({
                            "tool": tool_name,
                            "error": e.to_string()
                        }))
                        .unwrap_or_default(),
                        timestamp: chrono::Utc::now().timestamp(),
                        sequence: 0,
                    });
                }
            }
        });

        Ok(Response::new(
            Box::pin(ReceiverStream::new(rx)) as Self::CallToolStreamingStream
        ))
    }

    async fn get_tool_schema(
        &self,
        request: Request<GetToolSchemaRequest>,
    ) -> Result<Response<GetToolSchemaResponse>, Status> {
        let req = request.into_inner();
        let registry = self
            .infrastructure
            .tool_registry
            .clone()
            .ok_or_else(|| Status::internal("No tool registry"))?;
        let def = registry
            .get_definition(&req.tool_name)
            .await
            .ok_or_else(|| Status::not_found("Tool not found"))?;

        Ok(Response::new(GetToolSchemaResponse {
            schema: Some(convert_json_schema_to_tool_schema(&def.input_schema)),
        }))
    }
}

fn convert_json_schema_to_tool_schema(schema: &Value) -> ToolSchema {
    let mut parameters = Vec::new();
    if let Some(props) = schema.get("properties").and_then(|v| v.as_object()) {
        for (name, prop) in props {
            let p_type = prop
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("string");
            let param_type = match p_type {
                "string" => ParameterType::String,
                "integer" => ParameterType::Integer,
                "number" => ParameterType::Number,
                "boolean" => ParameterType::Boolean,
                "array" => ParameterType::Array,
                "object" => ParameterType::Object,
                _ => ParameterType::String,
            };
            parameters.push(ToolParameter {
                name: name.to_string(),
                r#type: param_type as i32,
                description: prop
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                default_value: None,
                enum_values: vec![],
            });
        }
    }
    ToolSchema {
        parameters,
        required: vec![],
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/tools/filesystem.rs">
//! Filesystem Tools

use crate::tool_registry::{BoxedTool, Tool, ToolRegistry};
use anyhow::Result;
use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

pub async fn register_all(registry: &ToolRegistry) -> Result<usize> {
    registry.register(Arc::new(ReadFileTool)).await?;
    registry.register(Arc::new(WriteFileTool)).await?;
    registry.register(Arc::new(ListDirectoryTool)).await?;
    Ok(3)
}

/// Normalize a path without requiring it to exist, rejecting traversal attempts.
/// Returns Err if the path contains `..` components or is not absolute.
fn normalize_path(raw: &str) -> Result<PathBuf, &'static str> {
    let path = Path::new(raw);
    if !path.is_absolute() {
        return Err("path must be absolute");
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => return Err("path traversal not allowed"),
            Component::CurDir => {}
            other => normalized.push(other),
        }
    }
    Ok(normalized)
}

const READ_BLOCKED: &[&str] = &[
    "/etc/shadow",
    "/etc/gshadow",
    "/etc/sudoers",
    "/etc/sudoers.d",
    "/etc/ssh",
    "/root",
    "/proc/self",
    "/proc/1",
];

const WRITE_BLOCKED: &[&str] = &[
    "/etc/",
    "/boot/",
    "/bin/",
    "/sbin/",
    "/usr/bin/",
    "/usr/sbin/",
    "/lib/",
    "/lib64/",
    "/usr/lib/",
    "/proc/",
    "/sys/",
    "/dev/",
    "/root/",
];

fn is_read_blocked(path: &Path) -> bool {
    let s = path.to_string_lossy();
    READ_BLOCKED.iter().any(|blocked| s.starts_with(blocked))
}

fn is_write_blocked(path: &Path) -> bool {
    let s = path.to_string_lossy();
    WRITE_BLOCKED.iter().any(|blocked| s.starts_with(blocked))
}

pub struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str { "read_file" }
    fn description(&self) -> &str { "Read the contents of a file." }
    fn category(&self) -> &str { "filesystem" }
    fn tags(&self) -> Vec<String> { vec!["filesystem".into(), "read".into()] }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string", "description": "Absolute path to the file"}
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let raw = input.get("path").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing path"))?;

        let path = normalize_path(raw)
            .map_err(|e| anyhow::anyhow!("Invalid path: {}", e))?;

        if is_read_blocked(&path) {
            return Ok(json!({"success": false, "error": "Access denied"}));
        }

        // Resolve symlinks after normalization to catch symlink-based bypasses
        let canonical = tokio::fs::canonicalize(&path).await
            .map_err(|e| anyhow::anyhow!("Cannot resolve path: {}", e))?;

        if is_read_blocked(&canonical) {
            return Ok(json!({"success": false, "error": "Access denied"}));
        }

        match tokio::fs::read_to_string(&canonical).await {
            Ok(content) => Ok(json!({"success": true, "path": raw, "content": content})),
            Err(e) => Ok(json!({"success": false, "error": e.to_string()}))
        }
    }
}

pub struct WriteFileTool;

#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str { "write_file" }
    fn description(&self) -> &str { "Write content to a file." }
    fn category(&self) -> &str { "filesystem" }
    fn tags(&self) -> Vec<String> { vec!["filesystem".into(), "write".into()] }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"},
                "content": {"type": "string"}
            },
            "required": ["path", "content"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let raw = input.get("path").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing path"))?;
        let content = input.get("content").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing content"))?;

        let path = normalize_path(raw)
            .map_err(|e| anyhow::anyhow!("Invalid path: {}", e))?;

        if is_write_blocked(&path) {
            return Ok(json!({"success": false, "error": "Access denied"}));
        }

        // Canonicalize parent dir to resolve symlinks before writing
        if let Some(parent) = path.parent() {
            if parent.exists() {
                let canonical_parent = tokio::fs::canonicalize(parent).await
                    .map_err(|e| anyhow::anyhow!("Cannot resolve parent: {}", e))?;
                let canonical_path = canonical_parent.join(path.file_name().unwrap_or_default());
                if is_write_blocked(&canonical_path) {
                    return Ok(json!({"success": false, "error": "Access denied"}));
                }
            }
        }

        match tokio::fs::write(&path, content).await {
            Ok(_) => Ok(json!({"success": true, "path": raw, "bytes_written": content.len()})),
            Err(e) => Ok(json!({"success": false, "error": e.to_string()}))
        }
    }
}

pub struct ListDirectoryTool;

#[async_trait]
impl Tool for ListDirectoryTool {
    fn name(&self) -> &str { "list_directory" }
    fn description(&self) -> &str { "List contents of a directory." }
    fn category(&self) -> &str { "filesystem" }
    fn tags(&self) -> Vec<String> { vec!["filesystem".into(), "list".into()] }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {"type": "string"}
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let raw = input.get("path").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing path"))?;

        let path = normalize_path(raw)
            .map_err(|e| anyhow::anyhow!("Invalid path: {}", e))?;

        if is_read_blocked(&path) {
            return Ok(json!({"success": false, "error": "Access denied"}));
        }

        let mut entries = Vec::new();
        let mut dir = tokio::fs::read_dir(&path).await?;

        while let Some(entry) = dir.next_entry().await? {
            let meta = entry.metadata().await?;
            entries.push(json!({
                "name": entry.file_name().to_string_lossy(),
                "is_dir": meta.is_dir(),
                "size": meta.len()
            }));
        }

        Ok(json!({"success": true, "path": raw, "entries": entries}))
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/tools/mod.rs">
//! Built-in Tools - All Loaded at Startup
//!
//! This module contains all tool implementations organized by category.
//! All tools are registered at startup and never evicted.

pub mod response;
pub mod filesystem;
pub mod shell;
pub mod system;
pub mod systemd;
pub mod ovs;
pub mod plugin;
pub mod qdrant;

use crate::tool_registry::{BoxedTool, ToolRegistry};
use anyhow::Result;
use std::sync::Arc;
use tracing::info;

/// Load ALL tools into registry at startup
/// 
/// This is called once when the server starts.
/// All tools remain loaded for the lifetime of the server.
pub async fn load_all_tools(registry: &ToolRegistry) -> Result<usize> {
    let mut count = 0;

    // Response tools (always needed)
    info!("Loading response tools...");
    count += response::register_all(registry).await?;

    // Filesystem tools
    info!("Loading filesystem tools...");
    count += filesystem::register_all(registry).await?;

    // Shell tools
    info!("Loading shell tools...");
    count += shell::register_all(registry).await?;

    // System tools
    info!("Loading system tools...");
    count += system::register_all(registry).await?;

    // Systemd tools (D-Bus)
    info!("Loading systemd tools...");
    count += systemd::register_all(registry).await?;

    // OVS tools
    info!("Loading OVS tools...");
    count += ovs::register_all(registry).await?;

    // Plugin state tools
    info!("Loading plugin state tools...");
    count += plugin::register_all(registry).await?;

    // Qdrant vector search tools
    info!("Loading Qdrant tools...");
    count += qdrant::register_all(registry).await?;

    info!("✅ Loaded {} tools total (no eviction)", count);
    Ok(count)
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/tools/ovs.rs">
//! Open vSwitch Tools

use crate::tool_registry::{Tool, ToolRegistry};
use anyhow::Result;
use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;

pub async fn register_all(registry: &ToolRegistry) -> Result<usize> {
    registry.register(Arc::new(OvsListBridgesTool)).await?;
    registry.register(Arc::new(OvsShowBridgeTool)).await?;
    registry.register(Arc::new(OvsListPortsTool)).await?;
    registry.register(Arc::new(OvsDumpFlowsTool)).await?;
    registry.register(Arc::new(OvsAddBridgeTool)).await?;
    registry.register(Arc::new(OvsDelBridgeTool)).await?;
    registry.register(Arc::new(OvsAddPortTool)).await?;
    registry.register(Arc::new(OvsDelPortTool)).await?;
    registry.register(Arc::new(OvsAddFlowTool)).await?;
    registry.register(Arc::new(OvsDelFlowsTool)).await?;
    Ok(10)
}

macro_rules! ovs_tool {
    ($name:ident, $tool_name:expr, $desc:expr, $schema:expr, $exec:expr) => {
        pub struct $name;
        
        #[async_trait]
        impl Tool for $name {
            fn name(&self) -> &str { $tool_name }
            fn description(&self) -> &str { $desc }
            fn category(&self) -> &str { "ovs" }
            fn tags(&self) -> Vec<String> { vec!["ovs".into(), "network".into()] }
            fn input_schema(&self) -> Value { $schema }
            async fn execute(&self, input: Value) -> Result<Value> { $exec(input).await }
        }
    };
}

async fn run_ovs_vsctl(args: &[&str]) -> Result<Value> {
    let output = tokio::process::Command::new("ovs-vsctl").args(args).output().await?;
    if output.status.success() {
        Ok(json!({"success": true, "output": String::from_utf8_lossy(&output.stdout).trim()}))
    } else {
        Ok(json!({"success": false, "error": String::from_utf8_lossy(&output.stderr).trim()}))
    }
}

async fn run_ovs_ofctl(args: &[&str]) -> Result<Value> {
    let output = tokio::process::Command::new("ovs-ofctl").args(args).output().await?;
    if output.status.success() {
        Ok(json!({"success": true, "output": String::from_utf8_lossy(&output.stdout).trim()}))
    } else {
        Ok(json!({"success": false, "error": String::from_utf8_lossy(&output.stderr).trim()}))
    }
}

ovs_tool!(OvsListBridgesTool, "ovs_list_bridges", "List all OVS bridges.",
    json!({"type": "object", "properties": {}}),
    |_input: Value| async {
        let result = run_ovs_vsctl(&["list-br"]).await?;
        if let Some(output) = result.get("output").and_then(|v| v.as_str()) {
            let bridges: Vec<&str> = output.lines().filter(|l| !l.is_empty()).collect();
            Ok(json!({"success": true, "bridges": bridges, "count": bridges.len()}))
        } else {
            Ok(result)
        }
    }
);

ovs_tool!(OvsShowBridgeTool, "ovs_show_bridge", "Show OVS bridge details.",
    json!({"type": "object", "properties": {"bridge": {"type": "string"}}, "required": ["bridge"]}),
    |input: Value| async move {
        let bridge = input.get("bridge").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing bridge"))?;
        run_ovs_vsctl(&["show"]).await // Shows all, could filter
    }
);

ovs_tool!(OvsListPortsTool, "ovs_list_ports", "List ports on an OVS bridge.",
    json!({"type": "object", "properties": {"bridge": {"type": "string"}}, "required": ["bridge"]}),
    |input: Value| async move {
        let bridge = input.get("bridge").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing bridge"))?;
        let result = run_ovs_vsctl(&["list-ports", bridge]).await?;
        if let Some(output) = result.get("output").and_then(|v| v.as_str()) {
            let ports: Vec<&str> = output.lines().filter(|l| !l.is_empty()).collect();
            Ok(json!({"success": true, "bridge": bridge, "ports": ports, "count": ports.len()}))
        } else {
            Ok(result)
        }
    }
);

ovs_tool!(OvsDumpFlowsTool, "ovs_dump_flows", "Dump flows from an OVS bridge.",
    json!({"type": "object", "properties": {"bridge": {"type": "string"}}, "required": ["bridge"]}),
    |input: Value| async move {
        let bridge = input.get("bridge").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing bridge"))?;
        run_ovs_ofctl(&["dump-flows", bridge]).await
    }
);

ovs_tool!(OvsAddBridgeTool, "ovs_add_bridge", "Create an OVS bridge.",
    json!({"type": "object", "properties": {"bridge": {"type": "string"}}, "required": ["bridge"]}),
    |input: Value| async move {
        let bridge = input.get("bridge").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing bridge"))?;
        run_ovs_vsctl(&["add-br", bridge]).await
    }
);

ovs_tool!(OvsDelBridgeTool, "ovs_del_bridge", "Delete an OVS bridge.",
    json!({"type": "object", "properties": {"bridge": {"type": "string"}}, "required": ["bridge"]}),
    |input: Value| async move {
        let bridge = input.get("bridge").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing bridge"))?;
        run_ovs_vsctl(&["del-br", bridge]).await
    }
);

ovs_tool!(OvsAddPortTool, "ovs_add_port", "Add a port to an OVS bridge.",
    json!({"type": "object", "properties": {"bridge": {"type": "string"}, "port": {"type": "string"}}, "required": ["bridge", "port"]}),
    |input: Value| async move {
        let bridge = input.get("bridge").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing bridge"))?;
        let port = input.get("port").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing port"))?;
        run_ovs_vsctl(&["add-port", bridge, port]).await
    }
);

ovs_tool!(OvsDelPortTool, "ovs_del_port", "Remove a port from an OVS bridge.",
    json!({"type": "object", "properties": {"bridge": {"type": "string"}, "port": {"type": "string"}}, "required": ["bridge", "port"]}),
    |input: Value| async move {
        let bridge = input.get("bridge").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing bridge"))?;
        let port = input.get("port").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing port"))?;
        run_ovs_vsctl(&["del-port", bridge, port]).await
    }
);

ovs_tool!(OvsAddFlowTool, "ovs_add_flow", "Add a flow to an OVS bridge.",
    json!({"type": "object", "properties": {"bridge": {"type": "string"}, "flow": {"type": "string"}}, "required": ["bridge", "flow"]}),
    |input: Value| async move {
        let bridge = input.get("bridge").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing bridge"))?;
        let flow = input.get("flow").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing flow"))?;
        run_ovs_ofctl(&["add-flow", bridge, flow]).await
    }
);

ovs_tool!(OvsDelFlowsTool, "ovs_del_flows", "Delete flows from an OVS bridge.",
    json!({"type": "object", "properties": {"bridge": {"type": "string"}, "match_str": {"type": "string"}}, "required": ["bridge"]}),
    |input: Value| async move {
        let bridge = input.get("bridge").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing bridge"))?;
        if let Some(match_str) = input.get("match_str").and_then(|v| v.as_str()) {
            run_ovs_ofctl(&["del-flows", bridge, match_str]).await
        } else {
            run_ovs_ofctl(&["del-flows", bridge]).await
        }
    }
);
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/tools/plugin.rs">
//! Plugin State Tools (query/diff/apply)

use crate::tool_registry::{Tool, ToolRegistry};
use anyhow::Result;
use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;

const PLUGINS: &[&str] = &[
    "systemd", "network", "packagekit", "firewall", "users", "storage",
    "lxc", "openflow", "privacy"
];

pub async fn register_all(registry: &ToolRegistry) -> Result<usize> {
    let mut count = 0;
    for plugin in PLUGINS {
        registry.register(Arc::new(PluginQueryTool::new(plugin))).await?;
        registry.register(Arc::new(PluginDiffTool::new(plugin))).await?;
        registry.register(Arc::new(PluginApplyTool::new(plugin))).await?;
        count += 3;
    }
    Ok(count)
}

pub struct PluginQueryTool { plugin: String, name: String, desc: String }

impl PluginQueryTool {
    pub fn new(plugin: &str) -> Self {
        Self {
            plugin: plugin.to_string(),
            name: format!("plugin_{}_query", plugin),
            desc: format!("Query current state from {} plugin", plugin),
        }
    }
}

#[async_trait]
impl Tool for PluginQueryTool {
    fn name(&self) -> &str { &self.name }
    fn description(&self) -> &str { &self.desc }
    fn category(&self) -> &str { "plugin" }
    fn tags(&self) -> Vec<String> { vec!["plugin".into(), "state".into(), self.plugin.clone()] }
    fn input_schema(&self) -> Value { json!({"type": "object", "properties": {"filter": {"type": "object"}}}) }

    async fn execute(&self, _input: Value) -> Result<Value> {
        // TODO: Integrate with the authoritative plugin catalog / canonical
        // plugin document path instead of inventing plugin state locally.
        Ok(json!({"success": true, "plugin": self.plugin, "operation": "query", "state": {}}))
    }
}

pub struct PluginDiffTool { plugin: String, name: String, desc: String }

impl PluginDiffTool {
    pub fn new(plugin: &str) -> Self {
        Self {
            plugin: plugin.to_string(),
            name: format!("plugin_{}_diff", plugin),
            desc: format!("Calculate state diff for {} plugin", plugin),
        }
    }
}

#[async_trait]
impl Tool for PluginDiffTool {
    fn name(&self) -> &str { &self.name }
    fn description(&self) -> &str { &self.desc }
    fn category(&self) -> &str { "plugin" }
    fn tags(&self) -> Vec<String> { vec!["plugin".into(), "diff".into(), self.plugin.clone()] }
    fn input_schema(&self) -> Value { json!({"type": "object", "properties": {"desired_state": {"type": "object"}}, "required": ["desired_state"]}) }

    async fn execute(&self, _input: Value) -> Result<Value> {
        Ok(json!({"success": true, "plugin": self.plugin, "operation": "diff", "changes": []}))
    }
}

pub struct PluginApplyTool { plugin: String, name: String, desc: String }

impl PluginApplyTool {
    pub fn new(plugin: &str) -> Self {
        Self {
            plugin: plugin.to_string(),
            name: format!("plugin_{}_apply", plugin),
            desc: format!("Apply state changes for {} plugin", plugin),
        }
    }
}

#[async_trait]
impl Tool for PluginApplyTool {
    fn name(&self) -> &str { &self.name }
    fn description(&self) -> &str { &self.desc }
    fn category(&self) -> &str { "plugin" }
    fn tags(&self) -> Vec<String> { vec!["plugin".into(), "apply".into(), self.plugin.clone()] }
    fn input_schema(&self) -> Value { json!({"type": "object", "properties": {"diff": {"type": "object"}, "dry_run": {"type": "boolean"}}, "required": ["diff"]}) }

    async fn execute(&self, input: Value) -> Result<Value> {
        let dry_run = input.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);
        Ok(json!({"success": true, "plugin": self.plugin, "operation": "apply", "dry_run": dry_run, "applied": !dry_run}))
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/tools/qdrant.rs">
use crate::tool_registry::{Tool, ToolResult};
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;

#[derive(Debug, Serialize, Deserialize)]
struct QdrantSearchRequest {
    query: String,
    collection: String,
    limit: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
struct QdrantSearchPayload {
    vector: Vec<f32>, // In real impl, would be actual vector
    limit: usize,
    with_payload: bool,
    with_vector: bool,
}

pub struct QdrantTool {
    client: Client,
    qdrant_url: String,
}

impl QdrantTool {
    pub fn new(qdrant_url: Option<String>) -> Self {
        Self {
            client: Client::new(),
            qdrant_url: qdrant_url.unwrap_or_else(|| "http://localhost:6333".to_string()),
        }
    }
    
    async fn search(&self, request: QdrantSearchRequest) -> Result<ToolResult> {
        // TODO: In real impl, convert query to vector via embedding
        // For now, placeholder
        let payload = QdrantSearchPayload {
            vector: vec![], // Should be actual embedding
            limit: request.limit.unwrap_or(10),
            with_payload: true,
            with_vector: false,
        };
        
        let url = format!("{}/collections/{}/points/search", 
            self.qdrant_url, request.collection);
        
        let response = self.client
            .post(&url)
            .json(&payload)
            .send()
            .await?;
            
        if response.status().is_success() {
            Ok(ToolResult::success("Qdrant search would execute here"))
        } else {
            Ok(ToolResult::error("Qdrant search failed"))
        }
    }
}

impl Tool for QdrantTool {
    fn name(&self) -> &'static str {
        "qdrant_search"
    }
    
    fn description(&self) -> &'static str {
        "Search Qdrant vector database for code knowledge"
    }
    
    async fn execute(&self, input: &str) -> Result<ToolResult> {
        let request: QdrantSearchRequest = simd_json::from_str(input)?;
        self.search(request).await
    }
}

pub async fn register_all(registry: &crate::tool_registry::ToolRegistry) -> Result<usize> {
    let qdrant_url = std::env::var("QDRANT_URL")
        .ok()
        .or_else(|| Some("http://localhost:6333".to_string()));
    
    let tool = QdrantTool::new(qdrant_url);
    registry.register(Box::new(tool));
    Ok(1)
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/tools/response.rs">
//! Response Tools - Communication with User

use crate::tool_registry::{BoxedTool, Tool, ToolRegistry};
use anyhow::Result;
use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;

pub async fn register_all(registry: &ToolRegistry) -> Result<usize> {
    registry.register(Arc::new(RespondToUserTool)).await?;
    registry.register(Arc::new(CannotPerformTool)).await?;
    registry.register(Arc::new(RequestClarificationTool)).await?;
    Ok(3)
}

pub struct RespondToUserTool;

#[async_trait]
impl Tool for RespondToUserTool {
    fn name(&self) -> &str { "respond_to_user" }
    fn description(&self) -> &str { "Send a response message to the user." }
    fn category(&self) -> &str { "response" }
    fn tags(&self) -> Vec<String> { vec!["response".into(), "essential".into()] }
    
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "message": {"type": "string", "description": "The message to send"}
            },
            "required": ["message"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let message = input.get("message").and_then(|v| v.as_str()).unwrap_or("");
        Ok(json!({"success": true, "message": message, "delivered": true}))
    }
}

pub struct CannotPerformTool;

#[async_trait]
impl Tool for CannotPerformTool {
    fn name(&self) -> &str { "cannot_perform" }
    fn description(&self) -> &str { "Indicate that a requested action cannot be performed." }
    fn category(&self) -> &str { "response" }
    fn tags(&self) -> Vec<String> { vec!["response".into(), "error".into()] }
    
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "reason": {"type": "string", "description": "Why the action cannot be performed"}
            },
            "required": ["reason"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let reason = input.get("reason").and_then(|v| v.as_str()).unwrap_or("Unknown");
        Ok(json!({"success": true, "cannot_perform": true, "reason": reason}))
    }
}

pub struct RequestClarificationTool;

#[async_trait]
impl Tool for RequestClarificationTool {
    fn name(&self) -> &str { "request_clarification" }
    fn description(&self) -> &str { "Ask the user for clarification." }
    fn category(&self) -> &str { "response" }
    fn tags(&self) -> Vec<String> { vec!["response".into(), "clarification".into()] }
    
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "question": {"type": "string", "description": "The clarification question"}
            },
            "required": ["question"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let question = input.get("question").and_then(|v| v.as_str()).unwrap_or("");
        Ok(json!({"success": true, "needs_clarification": true, "question": question}))
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/tools/shell.rs">
//! Shell Execution Tools

use crate::tool_registry::{Tool, ToolRegistry};
use anyhow::Result;
use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use std::time::Duration;

pub async fn register_all(registry: &ToolRegistry) -> Result<usize> {
    registry.register(Arc::new(ShellExecuteTool::new())).await?;
    Ok(1)
}

pub struct ShellExecuteTool {
    allowed_commands: Vec<String>,
}

impl ShellExecuteTool {
    pub fn new() -> Self {
        Self {
            allowed_commands: vec![
                "ls", "cat", "grep", "find", "head", "tail", "wc", "sort", "uniq",
                "echo", "pwd", "whoami", "date", "uname", "df", "du", "free", "uptime",
                "ps", "top", "ip", "ss", "netstat", "ping", "dig", "curl", "wget",
                "git", "docker", "kubectl", "systemctl", "journalctl",
                "cargo", "rustc", "python", "python3", "pip", "pip3",
                "node", "npm", "yarn",
            ].into_iter().map(String::from).collect()
        }
    }
}

#[async_trait]
impl Tool for ShellExecuteTool {
    fn name(&self) -> &str { "shell_execute" }
    fn description(&self) -> &str { "Execute a whitelisted shell command." }
    fn category(&self) -> &str { "shell" }
    fn tags(&self) -> Vec<String> { vec!["shell".into(), "execute".into()] }
    
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {"type": "string", "description": "Command to execute (must be whitelisted)"},
                "args": {"type": "array", "items": {"type": "string"}},
                "timeout_secs": {"type": "integer", "default": 30}
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let command = input.get("command").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing command"))?;
        
        if !self.allowed_commands.contains(&command.to_string()) {
            return Ok(json!({
                "success": false,
                "error": format!("Command '{}' not whitelisted", command)
            }));
        }
        
        let args: Vec<String> = input.get("args")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        
        let timeout = input.get("timeout_secs").and_then(|v| v.as_u64()).unwrap_or(30);
        
        let result = tokio::time::timeout(
            Duration::from_secs(timeout),
            tokio::process::Command::new(command).args(&args).output()
        ).await;
        
        match result {
            Ok(Ok(output)) => Ok(json!({
                "success": output.status.success(),
                "exit_code": output.status.code(),
                "stdout": String::from_utf8_lossy(&output.stdout),
                "stderr": String::from_utf8_lossy(&output.stderr)
            })),
            Ok(Err(e)) => Ok(json!({"success": false, "error": e.to_string()})),
            Err(_) => Ok(json!({"success": false, "error": "Command timed out"}))
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/tools/system.rs">
//! System Information Tools

use crate::tool_registry::{Tool, ToolRegistry};
use anyhow::Result;
use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;

pub async fn register_all(registry: &ToolRegistry) -> Result<usize> {
    registry.register(Arc::new(ListNetworkInterfacesTool)).await?;
    Ok(1)
}

pub struct ListNetworkInterfacesTool;

#[async_trait]
impl Tool for ListNetworkInterfacesTool {
    fn name(&self) -> &str { "list_network_interfaces" }
    fn description(&self) -> &str { "List all network interfaces." }
    fn category(&self) -> &str { "network" }
    fn tags(&self) -> Vec<String> { vec!["network".into(), "interfaces".into()] }
    
    fn input_schema(&self) -> Value {
        json!({"type": "object", "properties": {}})
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        let mut interfaces = Vec::new();
        let mut dir = tokio::fs::read_dir("/sys/class/net").await?;
        
        while let Some(entry) = dir.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            let state = tokio::fs::read_to_string(format!("/sys/class/net/{}/operstate", name))
                .await.unwrap_or_else(|_| "unknown".into()).trim().to_string();
            let mac = tokio::fs::read_to_string(format!("/sys/class/net/{}/address", name))
                .await.unwrap_or_else(|_| "unknown".into()).trim().to_string();
            
            interfaces.push(json!({"name": name, "state": state, "mac": mac}));
        }
        
        Ok(json!({"success": true, "interfaces": interfaces}))
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/tools/systemd.rs">
//! Systemd D-Bus Tools

use crate::tool_registry::{Tool, ToolRegistry};
use anyhow::Result;
use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;

pub async fn register_all(registry: &ToolRegistry) -> Result<usize> {
    registry.register(Arc::new(SystemdUnitStatusTool)).await?;
    registry.register(Arc::new(SystemdListUnitsTool)).await?;
    registry.register(Arc::new(SystemdStartUnitTool)).await?;
    registry.register(Arc::new(SystemdStopUnitTool)).await?;
    registry.register(Arc::new(SystemdRestartUnitTool)).await?;
    registry.register(Arc::new(SystemdEnableUnitTool)).await?;
    registry.register(Arc::new(SystemdDisableUnitTool)).await?;
    registry.register(Arc::new(SystemdReloadDaemonTool)).await?;
    Ok(8)
}

async fn get_systemd_proxy() -> Result<zbus::Proxy<'static>> {
    let connection = zbus::Connection::system().await?;
    zbus::proxy::Builder::new(&connection)
        .destination("org.freedesktop.systemd1")?
        .path("/org/freedesktop/systemd1")?
        .interface("org.freedesktop.systemd1.Manager")?
        .build()
        .await
        .map_err(|e| anyhow::anyhow!("D-Bus error: {}", e))
}

pub struct SystemdUnitStatusTool;

#[async_trait]
impl Tool for SystemdUnitStatusTool {
    fn name(&self) -> &str { "systemd_unit_status" }
    fn description(&self) -> &str { "Get the status of a systemd unit." }
    fn category(&self) -> &str { "systemd" }
    fn tags(&self) -> Vec<String> { vec!["systemd".into(), "dbus".into(), "status".into()] }
    
    fn input_schema(&self) -> Value {
        json!({"type": "object", "properties": {"unit": {"type": "string"}}, "required": ["unit"]})
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let unit = input.get("unit").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing unit"))?;
        
        let connection = zbus::Connection::system().await?;
        let proxy = get_systemd_proxy().await?;
        
        let unit_path: zbus::zvariant::OwnedObjectPath = proxy.call("GetUnit", &(unit,)).await
            .map_err(|e| anyhow::anyhow!("Failed to get unit: {}", e))?;
        
        let unit_proxy = zbus::proxy::Builder::new(&connection)
            .destination("org.freedesktop.systemd1")?
            .path(unit_path.as_str())?
            .interface("org.freedesktop.systemd1.Unit")?
            .build().await?;
        
        let active: String = unit_proxy.get_property("ActiveState").await.unwrap_or_else(|_| "unknown".into());
        let sub: String = unit_proxy.get_property("SubState").await.unwrap_or_else(|_| "unknown".into());
        let load: String = unit_proxy.get_property("LoadState").await.unwrap_or_else(|_| "unknown".into());
        let desc: String = unit_proxy.get_property("Description").await.unwrap_or_else(|_| "No description".into());
        
        Ok(json!({"success": true, "unit": unit, "active_state": active, "sub_state": sub, "load_state": load, "description": desc}))
    }
}

pub struct SystemdListUnitsTool;

#[async_trait]
impl Tool for SystemdListUnitsTool {
    fn name(&self) -> &str { "systemd_list_units" }
    fn description(&self) -> &str { "List systemd units with optional filtering." }
    fn category(&self) -> &str { "systemd" }
    fn tags(&self) -> Vec<String> { vec!["systemd".into(), "dbus".into(), "list".into()] }
    
    fn input_schema(&self) -> Value {
        json!({"type": "object", "properties": {
            "unit_type": {"type": "string", "default": "service"},
            "state": {"type": "string", "default": "all"},
            "limit": {"type": "integer", "default": 50}
        }})
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let unit_type = input.get("unit_type").and_then(|v| v.as_str()).unwrap_or("service");
        let state_filter = input.get("state").and_then(|v| v.as_str()).unwrap_or("all");
        let limit = input.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
        
        let proxy = get_systemd_proxy().await?;
        let units: Vec<(String, String, String, String, String, String, zbus::zvariant::OwnedObjectPath, u32, String, zbus::zvariant::OwnedObjectPath)> = 
            proxy.call("ListUnits", &()).await?;
        
        let filtered: Vec<Value> = units.into_iter()
            .filter(|(name, _, _, active, _, _, _, _, _, _)| {
                let type_ok = unit_type == "all" || name.ends_with(&format!(".{}", unit_type));
                let state_ok = state_filter == "all" || active == state_filter;
                type_ok && state_ok
            })
            .take(limit)
            .map(|(name, desc, load, active, sub, _, _, _, _, _)| json!({
                "name": name, "description": desc, "load_state": load, "active_state": active, "sub_state": sub
            }))
            .collect();
        
        Ok(json!({"success": true, "units": filtered, "count": filtered.len()}))
    }
}

macro_rules! systemd_action_tool {
    ($name:ident, $tool_name:expr, $desc:expr, $method:expr, $action:expr) => {
        pub struct $name;
        
        #[async_trait]
        impl Tool for $name {
            fn name(&self) -> &str { $tool_name }
            fn description(&self) -> &str { $desc }
            fn category(&self) -> &str { "systemd" }
            fn tags(&self) -> Vec<String> { vec!["systemd".into(), "dbus".into()] }
            
            fn input_schema(&self) -> Value {
                json!({"type": "object", "properties": {"unit": {"type": "string"}}, "required": ["unit"]})
            }

            async fn execute(&self, input: Value) -> Result<Value> {
                let unit = input.get("unit").and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing unit"))?;
                
                let proxy = get_systemd_proxy().await?;
                let job: zbus::zvariant::OwnedObjectPath = proxy.call($method, &(unit, "replace")).await
                    .map_err(|e| anyhow::anyhow!("Failed to {} unit: {}", $action, e))?;
                
                Ok(json!({"success": true, "unit": unit, "action": $action, "job_path": job.as_str()}))
            }
        }
    };
}

systemd_action_tool!(SystemdStartUnitTool, "systemd_start_unit", "Start a systemd unit.", "StartUnit", "started");
systemd_action_tool!(SystemdStopUnitTool, "systemd_stop_unit", "Stop a systemd unit.", "StopUnit", "stopped");
systemd_action_tool!(SystemdRestartUnitTool, "systemd_restart_unit", "Restart a systemd unit.", "RestartUnit", "restarted");

pub struct SystemdEnableUnitTool;

#[async_trait]
impl Tool for SystemdEnableUnitTool {
    fn name(&self) -> &str { "systemd_enable_unit" }
    fn description(&self) -> &str { "Enable a systemd unit." }
    fn category(&self) -> &str { "systemd" }
    fn tags(&self) -> Vec<String> { vec!["systemd".into()] }
    fn input_schema(&self) -> Value { json!({"type": "object", "properties": {"unit": {"type": "string"}}, "required": ["unit"]}) }

    async fn execute(&self, input: Value) -> Result<Value> {
        let unit = input.get("unit").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing unit"))?;
        let proxy = get_systemd_proxy().await?;
        let _: (bool, Vec<(String, String, String)>) = proxy.call("EnableUnitFiles", &(vec![unit], false, true)).await?;
        Ok(json!({"success": true, "unit": unit, "action": "enabled"}))
    }
}

pub struct SystemdDisableUnitTool;

#[async_trait]
impl Tool for SystemdDisableUnitTool {
    fn name(&self) -> &str { "systemd_disable_unit" }
    fn description(&self) -> &str { "Disable a systemd unit." }
    fn category(&self) -> &str { "systemd" }
    fn tags(&self) -> Vec<String> { vec!["systemd".into()] }
    fn input_schema(&self) -> Value { json!({"type": "object", "properties": {"unit": {"type": "string"}}, "required": ["unit"]}) }

    async fn execute(&self, input: Value) -> Result<Value> {
        let unit = input.get("unit").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing unit"))?;
        let proxy = get_systemd_proxy().await?;
        let _: Vec<(String, String, String)> = proxy.call("DisableUnitFiles", &(vec![unit], false)).await?;
        Ok(json!({"success": true, "unit": unit, "action": "disabled"}))
    }
}

pub struct SystemdReloadDaemonTool;

#[async_trait]
impl Tool for SystemdReloadDaemonTool {
    fn name(&self) -> &str { "systemd_reload_daemon" }
    fn description(&self) -> &str { "Reload systemd daemon configuration." }
    fn category(&self) -> &str { "systemd" }
    fn tags(&self) -> Vec<String> { vec!["systemd".into()] }
    fn input_schema(&self) -> Value { json!({"type": "object", "properties": {}}) }

    async fn execute(&self, _input: Value) -> Result<Value> {
        let proxy = get_systemd_proxy().await?;
        let _: () = proxy.call("Reload", &()).await?;
        Ok(json!({"success": true, "action": "daemon-reload"}))
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/transport/http.rs">
//! HTTP Transport
//!
//! HTTP/REST transport with SSE support.
//! Provides three variants:
//! - HttpTransport: REST only
//! - SseTransport: SSE only (for clients that use separate SSE + POST)
//! - HttpSseTransport: Combined bidirectional (recommended)
//!
//! Authentication (audit item #3):
//!   - `/health` is always open.
//!   - Real socket-loopback callers bypass auth (Host header is NEVER trusted).
//!   - All other callers must present `Authorization: Bearer <token>` AND the
//!     token must be accepted by the configured [`AuthValidator`].
//!   - The default validator is fail-secure: it only accepts tokens listed in
//!     `OPDBUS_MCP_ALLOWED_PEERS`. If that env var is unset/empty, every
//!     bearer token is rejected.
//!   - Additionally, the `User-Agent` must match a known MCP client pattern
//!     (Codex, Cursor, Claude Desktop, etc.) unless `OPDBUS_MCP_ANY_AGENT=1`
//!     is set (for development). Unknown agents are logged and rejected.

use super::{McpHandler, Transport};
use crate::McpRequest;
use anyhow::Result;
use axum::{
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
    middleware,
    response::{
        sse::{Event, Sse},
        IntoResponse, Json, Response,
    },
    routing::{get, post},
    Router,
};
use futures::stream::{self, Stream};
use simd_json::{json, OwnedValue as Value};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};
use tracing::{debug, info, warn};
use uuid::Uuid;

// =============================================================================
// AuthValidator (audit item #3)
//
// The previous implementation accepted ANY token that matched the *shape* of a
// WireGuard pubkey or a UUID. That was not authentication; it was a regex.
// Real authorization now goes through an `AuthValidator`. The default
// `EnvAllowListValidator` is fail-secure: with no entries, it rejects every
// bearer token, and only loopback callers can reach the handlers.
// =============================================================================

/// Validates an opaque bearer token against an authoritative source
/// (WireGuard peer DB, A.N.N.A. Scribe session ledger, etc.).
#[async_trait::async_trait]
pub trait AuthValidator: Send + Sync + 'static {
    async fn validate(&self, token: &str) -> bool;
}

/// Default validator backed by the `OPDBUS_MCP_ALLOWED_PEERS` env var
/// (comma-separated list of allowed pubkeys and/or session UUIDs).
///
/// * If the env var is unset or empty, **every** bearer token is rejected.
/// * Comparisons are constant-time over the candidate's length to prevent
///   timing oracles on partial matches.
pub struct EnvAllowListValidator {
    allowed: Vec<String>,
}

impl EnvAllowListValidator {
    /// Read the allow-list from `OPDBUS_MCP_ALLOWED_PEERS`.
    pub fn from_env() -> Self {
        let raw = std::env::var("OPDBUS_MCP_ALLOWED_PEERS").unwrap_or_default();
        let allowed: Vec<String> = raw
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if allowed.is_empty() {
            warn!(
                "OPDBUS_MCP_ALLOWED_PEERS is empty; bearer-token auth will reject all tokens. \
                 Only loopback callers will be accepted."
            );
        } else {
            info!(count = allowed.len(), "Loaded MCP peer allow-list");
        }
        Self { allowed }
    }

    /// Construct an allow-list directly (primarily for tests and embedders).
    pub fn new<I, S>(entries: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            allowed: entries.into_iter().map(Into::into).collect(),
        }
    }
}

#[async_trait::async_trait]
impl AuthValidator for EnvAllowListValidator {
    async fn validate(&self, token: &str) -> bool {
        let token_bytes = token.as_bytes();
        let mut matched = false;
        for entry in &self.allowed {
            matched |= ct_eq(token_bytes, entry.as_bytes());
        }
        matched
    }
}

/// Constant-time byte-slice equality. Returns false on length mismatch
/// (length is not considered secret; the contents are).
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|token| !token.is_empty())
}

fn is_wireguard_pubkey_shape(token: &str) -> bool {
    token.len() == 44
        && token.ends_with('=')
        && token
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '/' | '='))
}

fn is_wireguard_session_id_shape(token: &str) -> bool {
    Uuid::parse_str(token).is_ok()
}

/// **Shape check only.** Used as a cheap pre-filter inside the validator.
/// MUST NOT be used as an authorization decision on its own.
fn is_wireguard_auth_token_shape(token: &str) -> bool {
    is_wireguard_pubkey_shape(token) || is_wireguard_session_id_shape(token)
}

fn is_loopback_addr(addr: &SocketAddr) -> bool {
    addr.ip().is_loopback()
}

/// Known MCP client User-Agent substrings.
/// A request must match at least one to be accepted from a non-loopback peer.
/// Set `OPDBUS_MCP_ANY_AGENT=1` to bypass this check during development.
const KNOWN_MCP_AGENTS: &[&str] = &[
    "codex",           // OpenAI Codex CLI
    "cursor",          // Cursor IDE
    "claude",          // Claude Desktop / Claude Code
    "anthropic",       // Anthropic SDK
    "continue",        // Continue.dev
    "cline",           // Cline VSCode extension
    "copilot",         // GitHub Copilot
    "windsurf",        // Windsurf IDE
    "mcp-client",      // generic MCP SDK default
    "op-dbus",         // internal op-dbus clients
];

fn is_known_mcp_agent(headers: &HeaderMap) -> bool {
    if std::env::var("OPDBUS_MCP_ANY_AGENT").as_deref() == Ok("1") {
        return true;
    }
    let ua = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    KNOWN_MCP_AGENTS.iter().any(|pat| ua.contains(pat))
}

/// Authentication middleware.
///
/// Loopback bypass uses the **actual socket peer address**, not the
/// attacker-controlled `Host` header. Any non-loopback caller must present a
/// bearer token accepted by the configured [`AuthValidator`] AND a User-Agent
/// matching a known MCP client (Codex, Cursor, Claude, etc.).
async fn wireguard_auth_middleware(
    State(validator): State<Arc<dyn AuthValidator>>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    headers: HeaderMap,
    mut request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    // /health is always open
    if request.uri().path() == "/health" {
        return Ok(next.run(request).await);
    }

    // Loopback bypass uses the real peer IP. The Host header is never trusted.
    if let Some(ConnectInfo(peer)) = connect_info {
        if is_loopback_addr(&peer) {
            return Ok(next.run(request).await);
        }
    }

    let Some(token) = extract_bearer_token(&headers) else {
        warn!("Rejected HTTP MCP request without bearer token");
        return Err(StatusCode::UNAUTHORIZED);
    };

    if !validator.validate(token).await {
        warn!("Rejected HTTP MCP request: bearer token not in allow-list");
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Require a known MCP client User-Agent — blocks random token holders
    // that aren't actual MCP clients (curl probes, port scanners, etc.)
    if !is_known_mcp_agent(&headers) {
        let ua = headers
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("(none)");
        warn!(user_agent = ua, "Rejected HTTP MCP request: unrecognised client agent");
        return Err(StatusCode::FORBIDDEN);
    }

    let ua = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");
    info!(user_agent = ua, "Accepted MCP request from known client");
    request.extensions_mut().insert(token.to_string());
    Ok(next.run(request).await)
}

/// Shared state for HTTP handlers
struct HttpState<H> {
    handler: Arc<H>,
    event_tx: broadcast::Sender<String>,
}

fn default_validator() -> Arc<dyn AuthValidator> {
    Arc::new(EnvAllowListValidator::from_env())
}

/// HTTP-only transport (REST endpoints)
pub struct HttpTransport {
    bind_addr: String,
    enable_cors: bool,
    validator: Arc<dyn AuthValidator>,
}

impl HttpTransport {
    pub fn new(bind_addr: impl Into<String>) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            enable_cors: true,
            validator: default_validator(),
        }
    }

    pub fn without_cors(mut self) -> Self {
        self.enable_cors = false;
        self
    }

    /// Inject a custom authorization backend (e.g. a live WireGuard peer DB).
    /// Defaults to [`EnvAllowListValidator::from_env`].
    pub fn with_auth_validator(mut self, validator: Arc<dyn AuthValidator>) -> Self {
        self.validator = validator;
        self
    }
}

#[async_trait::async_trait]
impl Transport for HttpTransport {
    async fn serve<H: McpHandler + 'static>(self, handler: Arc<H>) -> Result<()> {
        info!(addr = %self.bind_addr, "Starting HTTP transport");

        let (event_tx, _) = broadcast::channel(100);
        let state = Arc::new(HttpState { handler, event_tx });
        let validator = self.validator;

        let mut app = Router::new()
            .route("/", get(root_handler).post(mcp_handler::<H>))
            .route("/mcp", post(mcp_handler::<H>))
            .route("/message", post(mcp_handler::<H>))
            .route("/health", get(health_handler))
            .route(
                "/tools/list",
                get(tools_list_handler::<H>).post(tools_list_handler::<H>),
            )
            .route("/tools/call", post(tools_call_handler::<H>))
            .layer(middleware::from_fn_with_state(
                validator,
                wireguard_auth_middleware,
            ))
            .with_state(state);

        if self.enable_cors {
            app = app.layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods(Any)
                    .allow_headers(Any),
            );
        }

        let listener = tokio::net::TcpListener::bind(&self.bind_addr).await?;
        info!(addr = %self.bind_addr, "HTTP transport listening");

        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await?;
        Ok(())
    }
}

/// SSE-only transport
pub struct SseTransport {
    bind_addr: String,
    validator: Arc<dyn AuthValidator>,
}

impl SseTransport {
    pub fn new(bind_addr: impl Into<String>) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            validator: default_validator(),
        }
    }

    pub fn with_auth_validator(mut self, validator: Arc<dyn AuthValidator>) -> Self {
        self.validator = validator;
        self
    }
}

#[async_trait::async_trait]
impl Transport for SseTransport {
    async fn serve<H: McpHandler + 'static>(self, handler: Arc<H>) -> Result<()> {
        info!(addr = %self.bind_addr, "Starting SSE transport");

        let (event_tx, _) = broadcast::channel(100);
        let state = Arc::new(HttpState { handler, event_tx });
        let validator = self.validator;

        let app = Router::new()
            .route("/", get(sse_handler::<H>))
            .route("/sse", get(sse_handler::<H>))
            .route("/message", post(mcp_handler::<H>))
            .route("/health", get(health_handler))
            .layer(middleware::from_fn_with_state(
                validator,
                wireguard_auth_middleware,
            ))
            .layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods(Any)
                    .allow_headers(Any),
            )
            .with_state(state);

        let listener = tokio::net::TcpListener::bind(&self.bind_addr).await?;
        info!(addr = %self.bind_addr, "SSE transport listening");

        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await?;
        Ok(())
    }
}

/// HTTP+SSE bidirectional transport (recommended)
pub struct HttpSseTransport {
    bind_addr: String,
    base_path: String,
    validator: Arc<dyn AuthValidator>,
}

impl HttpSseTransport {
    pub fn new(bind_addr: impl Into<String>) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            base_path: String::new(),
            validator: default_validator(),
        }
    }

    pub fn with_base_path(mut self, path: impl Into<String>) -> Self {
        self.base_path = path.into();
        self
    }

    pub fn with_auth_validator(mut self, validator: Arc<dyn AuthValidator>) -> Self {
        self.validator = validator;
        self
    }
}

#[async_trait::async_trait]
impl Transport for HttpSseTransport {
    async fn serve<H: McpHandler + 'static>(self, handler: Arc<H>) -> Result<()> {
        info!(addr = %self.bind_addr, "Starting HTTP+SSE transport");

        let (event_tx, _) = broadcast::channel(100);
        let state = Arc::new(HttpState { handler, event_tx });
        let base_path = self.base_path.trim_end_matches('/').to_string();
        let validator = self.validator;

        let mut app = Router::new()
            .route("/", get(root_handler).post(mcp_handler::<H>))
            .route("/sse", get(sse_handler::<H>))
            .route("/mcp", post(mcp_handler::<H>))
            .route("/message", post(mcp_handler::<H>))
            .route("/health", get(health_handler))
            .route(
                "/tools/list",
                get(tools_list_handler::<H>).post(tools_list_handler::<H>),
            )
            .route("/tools/call", post(tools_call_handler::<H>));

        if !base_path.is_empty() {
            app = app
                .route(&base_path, get(sse_handler::<H>).post(mcp_handler::<H>))
                .route(&format!("{}/sse", base_path), get(sse_handler::<H>))
                .route(&format!("{}/message", base_path), post(mcp_handler::<H>))
                .route(
                    &format!("{}/tools/list", base_path),
                    get(tools_list_handler::<H>).post(tools_list_handler::<H>),
                )
                .route(
                    &format!("{}/tools/call", base_path),
                    post(tools_call_handler::<H>),
                );
        }

        let app = app
            .layer(middleware::from_fn_with_state(
                validator,
                wireguard_auth_middleware,
            ))
            .layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods(Any)
                    .allow_headers(Any),
            )
            .with_state(state);

        let listener = tokio::net::TcpListener::bind(&self.bind_addr).await?;
        info!(addr = %self.bind_addr, "HTTP+SSE transport listening");

        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await?;
        Ok(())
    }
}

// === Handlers ===

async fn root_handler() -> impl IntoResponse {
    Json(json!({
        "service": "op-mcp",
        "version": crate::SERVER_VERSION,
        "protocol": crate::PROTOCOL_VERSION,
        "endpoints": {
            "mcp": "POST /mcp",
            "sse": "GET /sse",
            "health": "GET /health",
            "tools_list": "GET /tools/list",
            "tools_call": "POST /tools/call"
        }
    }))
}

async fn health_handler() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "service": "op-mcp",
        "version": crate::SERVER_VERSION
    }))
}

async fn mcp_handler<H: McpHandler>(
    State(state): State<Arc<HttpState<H>>>,
    Json(request): Json<McpRequest>,
) -> Response {
    debug!(method = %request.method, "HTTP MCP request");
    let is_notification = request.id.is_none();
    let response = state.handler.handle_request(request).await;

    if is_notification {
        StatusCode::ACCEPTED.into_response()
    } else {
        Json(response).into_response()
    }
}

async fn tools_list_handler<H: McpHandler>(
    State(state): State<Arc<HttpState<H>>>,
) -> impl IntoResponse {
    let request = McpRequest::new("tools/list").with_id(json!(1));
    let response = state.handler.handle_request(request).await;
    Json(response)
}

async fn tools_call_handler<H: McpHandler>(
    State(state): State<Arc<HttpState<H>>>,
    Json(params): Json<Value>,
) -> impl IntoResponse {
    let request = McpRequest::new("tools/call")
        .with_id(json!(1))
        .with_params(params);
    let response = state.handler.handle_request(request).await;
    Json(response)
}

async fn sse_handler<H: McpHandler + 'static>(
    State(state): State<Arc<HttpState<H>>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    info!("SSE client connected");

    // Build initial events
    let initial_events = vec![
        Event::default().event("endpoint").data("/mcp"),
        Event::default().event("connected").data(
            json!({
                "server": "op-mcp",
                "version": crate::SERVER_VERSION
            })
            .to_string(),
        ),
    ];

    let initial_stream = stream::iter(initial_events.into_iter().map(Ok));

    // Keepalive stream
    let keepalive_stream = stream::unfold(0u64, |counter| async move {
        tokio::time::sleep(Duration::from_secs(30)).await;
        let event = Event::default()
            .event("ping")
            .data(json!({ "counter": counter }).to_string());
        Some((Ok(event), counter + 1))
    });

    // Broadcast stream for server-initiated events
    let rx = state.event_tx.subscribe();
    let broadcast_stream =
        tokio_stream::wrappers::BroadcastStream::new(rx).filter_map(|result| async move {
            match result {
                Ok(data) => Some(Ok(Event::default().data(data))),
                Err(_) => None,
            }
        });

    use futures::StreamExt;
    let combined = initial_stream
        .chain(broadcast_stream)
        .chain(keepalive_stream);

    Sse::new(combined).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(30))
            .text("keepalive"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue};

    #[test]
    fn should_extract_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_static("Bearer 123e4567-e89b-12d3-a456-426614174000"),
        );

        assert_eq!(
            extract_bearer_token(&headers),
            Some("123e4567-e89b-12d3-a456-426614174000")
        );
    }

    #[test]
    fn shape_check_accepts_pubkey_and_uuid() {
        assert!(is_wireguard_auth_token_shape(
            "123e4567-e89b-12d3-a456-426614174000"
        ));
        assert!(is_wireguard_auth_token_shape(
            "MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY="
        ));
        assert!(!is_wireguard_auth_token_shape("ya29.google-oauth-token"));
        assert!(!is_wireguard_auth_token_shape("not-a-wireguard-token"));
    }

    #[test]
    fn ct_eq_matches_only_identical_inputs() {
        assert!(ct_eq(b"abc", b"abc"));
        assert!(!ct_eq(b"abc", b"abd"));
        assert!(!ct_eq(b"abc", b"abcd"));
        assert!(!ct_eq(b"", b"a"));
        assert!(ct_eq(b"", b""));
    }

    #[tokio::test]
    async fn validator_accepts_listed_token_and_rejects_others() {
        let v = EnvAllowListValidator::new(vec!["123e4567-e89b-12d3-a456-426614174000"]);
        assert!(v.validate("123e4567-e89b-12d3-a456-426614174000").await);
        assert!(!v.validate("00000000-0000-0000-0000-000000000000").await);
        assert!(!v.validate("garbage").await);
        // wrong shape is rejected even though it would be exactly equal to no
        // entry anyway
        assert!(!v.validate("not-a-wireguard-token").await);
    }

    #[tokio::test]
    async fn validator_with_no_entries_rejects_everything() {
        let v = EnvAllowListValidator::new(Vec::<String>::new());
        assert!(!v.validate("123e4567-e89b-12d3-a456-426614174000").await);
        assert!(
            !v.validate("MDEyMzQ1Njc4OWFiY2RlZjAxMjM0NTY3ODlhYmNkZWY=")
                .await
        );
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/transport/mod.rs">
//! Transport Layer
//!
//! Provides multiple transport implementations:
//! - Stdio (standard input/output)
//! - HTTP (REST endpoints)
//! - SSE (Server-Sent Events)
//! - HTTP+SSE (bidirectional)
//! - WebSocket (full duplex)
//! - gRPC (high-performance RPC) [optional feature]

mod http;
mod stdio;
mod websocket;

pub use http::{HttpSseTransport, HttpTransport, SseTransport};
pub use stdio::StdioTransport;
pub use websocket::WebSocketTransport;

use anyhow::Result;
use std::sync::Arc;

/// Generic MCP server trait for transport layer
#[async_trait::async_trait]
pub trait McpHandler: Send + Sync {
    async fn handle_request(&self, request: crate::McpRequest) -> crate::McpResponse;
}

/// Transport trait - implement for new transport types
#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    /// Serve requests using this transport
    async fn serve<H: McpHandler + 'static>(self, handler: Arc<H>) -> Result<()>;
}

// Implement McpHandler for all server types
#[async_trait::async_trait]
impl McpHandler for crate::McpServer {
    async fn handle_request(&self, request: crate::McpRequest) -> crate::McpResponse {
        self.handle_request(request).await
    }
}

#[async_trait::async_trait]
impl McpHandler for crate::AgentsServer {
    async fn handle_request(&self, request: crate::McpRequest) -> crate::McpResponse {
        self.handle_request(request).await
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/transport/stdio.rs">
//! Stdio Transport
//!
//! Standard MCP transport over stdin/stdout.

use super::{McpHandler, Transport};
use crate::{JsonRpcError, McpRequest, McpResponse};
use anyhow::Result;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, error, info};

/// Stdio transport - reads JSON-RPC from stdin, writes to stdout
pub struct StdioTransport;

impl StdioTransport {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StdioTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl Transport for StdioTransport {
    async fn serve<H: McpHandler + 'static>(self, handler: Arc<H>) -> Result<()> {
        info!("Starting MCP stdio transport");

        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let mut reader = BufReader::new(stdin).lines();

        while let Some(line) = reader.next_line().await? {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            debug!(request = %line, "Received request");

            let mut line_mut = line.to_string();
            let response = match unsafe { simd_json::from_str::<McpRequest>(&mut line_mut) } {
                Ok(request) => handler.handle_request(request).await,
                Err(e) => {
                    error!(error = %e, "Parse error");
                    McpResponse::error(None, JsonRpcError::parse_error(e.to_string()))
                }
            };

            let response_json = simd_json::to_string(&response)?;
            debug!(response = %response_json, "Sending response");

            stdout.write_all(response_json.as_bytes()).await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
        }

        info!("Stdio transport shutting down");
        Ok(())
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/transport/websocket.rs">
//! WebSocket Transport
//!
//! Full-duplex WebSocket transport for MCP.

use super::{McpHandler, Transport};
use crate::{JsonRpcError, McpRequest, McpResponse};
use anyhow::Result;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use simd_json::json;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::{debug, error, info, warn};

/// WebSocket transport
pub struct WebSocketTransport {
    bind_addr: String,
}

impl WebSocketTransport {
    pub fn new(bind_addr: impl Into<String>) -> Self {
        Self {
            bind_addr: bind_addr.into(),
        }
    }
}

struct WsState<H> {
    handler: Arc<H>,
}

#[async_trait::async_trait]
impl Transport for WebSocketTransport {
    async fn serve<H: McpHandler + 'static>(self, handler: Arc<H>) -> Result<()> {
        info!(addr = %self.bind_addr, "Starting WebSocket transport");

        let state = Arc::new(WsState { handler });

        let app = Router::new()
            .route("/", get(ws_handler::<H>))
            .route("/ws", get(ws_handler::<H>))
            .route("/health", get(health_handler))
            .layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods(Any)
                    .allow_headers(Any),
            )
            .with_state(state);

        let listener = tokio::net::TcpListener::bind(&self.bind_addr).await?;
        info!(addr = %self.bind_addr, "WebSocket transport listening");

        axum::serve(listener, app).await?;
        Ok(())
    }
}

async fn health_handler() -> impl IntoResponse {
    axum::Json(json!({
        "status": "ok",
        "transport": "websocket"
    }))
}

async fn ws_handler<H: McpHandler + 'static>(
    ws: WebSocketUpgrade,
    State(state): State<Arc<WsState<H>>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws_connection(socket, state))
}

async fn handle_ws_connection<H: McpHandler>(socket: WebSocket, state: Arc<WsState<H>>) {
    info!("WebSocket client connected");

    let (mut sender, mut receiver) = socket.split();

    // Send welcome message
    let welcome = json!({
        "type": "welcome",
        "server": "op-mcp",
        "version": crate::SERVER_VERSION,
        "protocol": crate::PROTOCOL_VERSION
    });

    if let Err(e) = sender.send(Message::Text(welcome.to_string())).await {
        error!(error = %e, "Failed to send welcome");
        return;
    }

    // Handle incoming messages
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                debug!(request = %text, "WebSocket request");

                let mut text_mut = text.clone();
                let response = match unsafe { simd_json::from_str::<McpRequest>(&mut text_mut) } {
                    Ok(request) => state.handler.handle_request(request).await,
                    Err(e) => {
                        warn!(error = %e, "Invalid request");
                        McpResponse::error(None, JsonRpcError::parse_error(e.to_string()))
                    }
                };

                let response_json = simd_json::to_string(&response).unwrap_or_default();

                if let Err(e) = sender.send(Message::Text(response_json)).await {
                    error!(error = %e, "Failed to send response");
                    break;
                }
            }
            Ok(Message::Ping(data)) => {
                if let Err(e) = sender.send(Message::Pong(data)).await {
                    error!(error = %e, "Failed to send pong");
                    break;
                }
            }
            Ok(Message::Close(_)) => {
                info!("WebSocket client disconnected");
                break;
            }
            Ok(_) => {} // Ignore binary, pong, etc.
            Err(e) => {
                error!(error = %e, "WebSocket error");
                break;
            }
        }
    }

    info!("WebSocket connection closed");
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/agents_main.rs">
//! MCP Agents Server - Stdio Transport
//!
//! This binary provides a stdio-based MCP server that exposes agent tools
//! for use with clients like Gemini CLI that only support stdio transport.
//!
//! Usage:
//!   op-mcp-agents-server
//!
//! The server reads JSON-RPC requests from stdin and writes responses to stdout.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::sync::Arc;
use tokio::sync::RwLock;

/// MCP Protocol version
const PROTOCOL_VERSION: &str = "2024-11-05";
const SERVER_NAME: &str = "op-mcp-agents-server";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

// ============================================================================
// JSON-RPC Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

impl JsonRpcResponse {
    fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Option<Value>, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
        }
    }
}

// ============================================================================
// MCP Types
// ============================================================================

#[derive(Debug, Serialize)]
struct ServerInfo {
    name: String,
    version: String,
}

#[derive(Debug, Serialize)]
struct ServerCapabilities {
    tools: ToolsCapability,
}

#[derive(Debug, Serialize)]
struct ToolsCapability {
    #[serde(rename = "listChanged")]
    list_changed: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ToolDefinition {
    name: String,
    description: String,
    #[serde(rename = "inputSchema")]
    input_schema: Value,
}

// ============================================================================
// Agent Tool Definitions
// ============================================================================

fn get_agent_tools() -> Vec<ToolDefinition> {
    vec![
        // Sequential Thinking Agent
        ToolDefinition {
            name: "agent_sequential_thinking".to_string(),
            description: "A detailed thinking tool that helps break down complex problems into sequential steps. Use this for multi-step reasoning, planning, and analysis.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "thought": {
                        "type": "string",
                        "description": "The current thought or reasoning step"
                    },
                    "operation": {
                        "type": "string",
                        "description": "Operation to perform: think, plan, analyze, conclude",
                        "enum": ["think", "plan", "analyze", "conclude"]
                    },
                    "context": {
                        "type": "string",
                        "description": "Additional context for the thinking process"
                    }
                }
            }),
        },
        // Memory Agent
        ToolDefinition {
            name: "agent_memory".to_string(),
            description: "Store and retrieve information from conversation memory. Use for maintaining context across interactions.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "description": "Operation: store, retrieve, search, clear",
                        "enum": ["store", "retrieve", "search", "clear"]
                    },
                    "key": {
                        "type": "string",
                        "description": "Key for storing/retrieving data"
                    },
                    "value": {
                        "type": "string",
                        "description": "Value to store (for store operation)"
                    },
                    "query": {
                        "type": "string",
                        "description": "Search query (for search operation)"
                    }
                },
                "required": ["operation"]
            }),
        },
        // Code Review Agent
        ToolDefinition {
            name: "agent_code_review".to_string(),
            description: "Analyze code for issues, suggest improvements, and provide review feedback.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "description": "Operation: review, suggest, analyze_security, check_style",
                        "enum": ["review", "suggest", "analyze_security", "check_style"]
                    },
                    "code": {
                        "type": "string",
                        "description": "Code to review"
                    },
                    "language": {
                        "type": "string",
                        "description": "Programming language"
                    },
                    "focus": {
                        "type": "string",
                        "description": "Specific area to focus on"
                    }
                },
                "required": ["operation", "code"]
            }),
        },
        // Rust Expert Agent
        ToolDefinition {
            name: "agent_rust_pro".to_string(),
            description: "Expert Rust programming assistance. Helps with Rust code, best practices, error handling, and optimization.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "description": "Operation: help, review, optimize, explain, fix_error",
                        "enum": ["help", "review", "optimize", "explain", "fix_error"]
                    },
                    "code": {
                        "type": "string",
                        "description": "Rust code to analyze"
                    },
                    "error": {
                        "type": "string",
                        "description": "Error message to help fix"
                    },
                    "question": {
                        "type": "string",
                        "description": "Question about Rust"
                    }
                },
                "required": ["operation"]
            }),
        },
        // Python Expert Agent
        ToolDefinition {
            name: "agent_python_pro".to_string(),
            description: "Expert Python programming assistance. Helps with Python code, best practices, and optimization.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "description": "Operation: help, review, optimize, explain, fix_error",
                        "enum": ["help", "review", "optimize", "explain", "fix_error"]
                    },
                    "code": {
                        "type": "string",
                        "description": "Python code to analyze"
                    },
                    "error": {
                        "type": "string",
                        "description": "Error message to help fix"
                    },
                    "question": {
                        "type": "string",
                        "description": "Question about Python"
                    }
                },
                "required": ["operation"]
            }),
        },
        // DevOps Troubleshooter Agent
        ToolDefinition {
            name: "agent_devops_troubleshooter".to_string(),
            description: "DevOps and infrastructure troubleshooting. Helps diagnose and fix deployment, container, and infrastructure issues.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "description": "Operation: diagnose, suggest_fix, analyze_logs, check_config",
                        "enum": ["diagnose", "suggest_fix", "analyze_logs", "check_config"]
                    },
                    "issue": {
                        "type": "string",
                        "description": "Description of the issue"
                    },
                    "logs": {
                        "type": "string",
                        "description": "Relevant log output"
                    },
                    "config": {
                        "type": "string",
                        "description": "Configuration to analyze"
                    },
                    "context": {
                        "type": "string",
                        "description": "Additional context (k8s, docker, systemd, etc.)"
                    }
                },
                "required": ["operation"]
            }),
        },
        // Network Expert Agent
        ToolDefinition {
            name: "agent_network_expert".to_string(),
            description: "Network configuration and troubleshooting expert. Helps with OVS, routing, firewalls, and network debugging.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "description": "Operation: diagnose, configure, explain, troubleshoot",
                        "enum": ["diagnose", "configure", "explain", "troubleshoot"]
                    },
                    "issue": {
                        "type": "string",
                        "description": "Network issue or question"
                    },
                    "topology": {
                        "type": "string",
                        "description": "Network topology description"
                    },
                    "config": {
                        "type": "string",
                        "description": "Current network configuration"
                    }
                },
                "required": ["operation"]
            }),
        },
        // Database Architect Agent
        ToolDefinition {
            name: "agent_database_architect".to_string(),
            description: "Database design, optimization, and troubleshooting. Supports SQL, PostgreSQL, Redis, and more.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "description": "Operation: design, optimize, review, migrate, troubleshoot",
                        "enum": ["design", "optimize", "review", "migrate", "troubleshoot"]
                    },
                    "query": {
                        "type": "string",
                        "description": "SQL query to analyze"
                    },
                    "schema": {
                        "type": "string",
                        "description": "Database schema"
                    },
                    "requirements": {
                        "type": "string",
                        "description": "Requirements for design"
                    },
                    "database_type": {
                        "type": "string",
                        "description": "Database type (postgres, mysql, redis, etc.)"
                    }
                },
                "required": ["operation"]
            }),
        },
        // Security Auditor Agent
        ToolDefinition {
            name: "agent_security_auditor".to_string(),
            description: "Security analysis and auditing. Reviews code, configs, and infrastructure for vulnerabilities.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "description": "Operation: audit, scan, review, recommend",
                        "enum": ["audit", "scan", "review", "recommend"]
                    },
                    "target": {
                        "type": "string",
                        "description": "Code, config, or system to audit"
                    },
                    "target_type": {
                        "type": "string",
                        "description": "Type: code, config, infrastructure, api"
                    },
                    "focus": {
                        "type": "string",
                        "description": "Specific security area to focus on"
                    }
                },
                "required": ["operation"]
            }),
        },
        // Kubernetes Expert Agent
        ToolDefinition {
            name: "agent_kubernetes_expert".to_string(),
            description: "Kubernetes configuration, deployment, and troubleshooting expert.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "description": "Operation: deploy, diagnose, optimize, explain, generate",
                        "enum": ["deploy", "diagnose", "optimize", "explain", "generate"]
                    },
                    "manifest": {
                        "type": "string",
                        "description": "Kubernetes manifest YAML"
                    },
                    "issue": {
                        "type": "string",
                        "description": "Issue or question"
                    },
                    "requirements": {
                        "type": "string",
                        "description": "Deployment requirements"
                    }
                },
                "required": ["operation"]
            }),
        },
    ]
}

// ============================================================================
// Agent Execution
// ============================================================================

struct AgentServer {
    memory: Arc<RwLock<HashMap<String, String>>>,
    thinking_history: Arc<RwLock<Vec<String>>>,
}

impl AgentServer {
    fn new() -> Self {
        Self {
            memory: Arc::new(RwLock::new(HashMap::new())),
            thinking_history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    async fn execute_tool(&self, name: &str, args: Value) -> Result<Value> {
        match name {
            "agent_sequential_thinking" => self.sequential_thinking(args).await,
            "agent_memory" => self.memory_operations(args).await,
            "agent_code_review" => self.code_review(args).await,
            "agent_rust_pro" => self.language_expert("Rust", args).await,
            "agent_python_pro" => self.language_expert("Python", args).await,
            "agent_devops_troubleshooter" => self.devops_troubleshoot(args).await,
            "agent_network_expert" => self.network_expert(args).await,
            "agent_database_architect" => self.database_architect(args).await,
            "agent_security_auditor" => self.security_auditor(args).await,
            "agent_kubernetes_expert" => self.kubernetes_expert(args).await,
            _ => Err(anyhow::anyhow!("Unknown tool: {}", name)),
        }
    }

    async fn sequential_thinking(&self, args: Value) -> Result<Value> {
        // Accept either "thought" or "operation" field
        let thought = args
            .as_object()
            .and_then(|o| o.get("thought"))
            .and_then(|v| v.as_str())
            .or_else(|| {
                args.as_object()
                    .and_then(|o| o.get("operation"))
                    .and_then(|v| v.as_str())
            })
            .unwrap_or("No thought provided");

        let context = args
            .as_object()
            .and_then(|o| o.get("context"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Store in thinking history
        {
            let mut history = self.thinking_history.write().await;
            history.push(thought.to_string());
        }

        let step_number = self.thinking_history.read().await.len();

        Ok(json!({
            "status": "success",
            "step": step_number,
            "thought": thought,
            "context": context,
            "message": format!("Thinking step {} recorded: {}", step_number,
                if thought.len() > 50 { format!("{}...", &thought[..50]) } else { thought.to_string() })
        }))
    }

    async fn memory_operations(&self, args: Value) -> Result<Value> {
        let operation = args
            .as_object()
            .and_then(|o| o.get("operation"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing operation"))?;

        match operation {
            "store" => {
                let key = args
                    .as_object()
                    .and_then(|o| o.get("key"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing key"))?;
                let value = args
                    .as_object()
                    .and_then(|o| o.get("value"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing value"))?;

                self.memory
                    .write()
                    .await
                    .insert(key.to_string(), value.to_string());
                Ok(json!({ "status": "stored", "key": key }))
            }
            "retrieve" => {
                let key = args
                    .as_object()
                    .and_then(|o| o.get("key"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing key"))?;

                let value = self.memory.read().await.get(key).cloned();
                Ok(json!({ "status": "retrieved", "key": key, "value": value }))
            }
            "search" => {
                let query = args
                    .as_object()
                    .and_then(|o| o.get("query"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let memory = self.memory.read().await;
                let matches: Vec<_> = memory
                    .iter()
                    .filter(|(k, v)| k.contains(query) || v.contains(query))
                    .map(|(k, v)| json!({ "key": k, "value": v }))
                    .collect();

                Ok(json!({ "status": "searched", "matches": matches, "count": matches.len() }))
            }
            "clear" => {
                self.memory.write().await.clear();
                Ok(json!({ "status": "cleared" }))
            }
            _ => Err(anyhow::anyhow!("Unknown memory operation: {}", operation)),
        }
    }

    async fn code_review(&self, args: Value) -> Result<Value> {
        let operation = args
            .as_object()
            .and_then(|o| o.get("operation"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing operation"))?;
        let code = args
            .as_object()
            .and_then(|o| o.get("code"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing code"))?;
        let language = args
            .as_object()
            .and_then(|o| o.get("language"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        Ok(json!({
            "status": "success",
            "operation": operation,
            "language": language,
            "code_length": code.len(),
            "analysis": format!("Code review ({}) for {} code ({} chars). Use an LLM to get detailed analysis.",
                operation, language, code.len()),
            "note": "This agent provides structure for code review. Connect to an LLM for detailed analysis."
        }))
    }

    async fn language_expert(&self, language: &str, args: Value) -> Result<Value> {
        let operation = args
            .as_object()
            .and_then(|o| o.get("operation"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing operation"))?;

        Ok(json!({
            "status": "success",
            "language": language,
            "operation": operation,
            "message": format!("{} expert ready for {} operation", language, operation),
            "note": "This agent provides structure for language-specific help. Connect to an LLM for detailed assistance."
        }))
    }

    async fn devops_troubleshoot(&self, args: Value) -> Result<Value> {
        let operation = args
            .as_object()
            .and_then(|o| o.get("operation"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing operation"))?;
        let issue = args
            .as_object()
            .and_then(|o| o.get("issue"))
            .and_then(|v| v.as_str())
            .unwrap_or("No issue specified");

        Ok(json!({
            "status": "success",
            "operation": operation,
            "issue": issue,
            "message": format!("DevOps troubleshooter analyzing: {}",
                if issue.len() > 50 { format!("{}...", &issue[..50]) } else { issue.to_string() }),
            "note": "This agent provides structure for DevOps troubleshooting. Connect to an LLM for detailed diagnosis."
        }))
    }

    async fn network_expert(&self, args: Value) -> Result<Value> {
        let operation = args
            .as_object()
            .and_then(|o| o.get("operation"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing operation"))?;

        Ok(json!({
            "status": "success",
            "operation": operation,
            "message": format!("Network expert ready for {} operation", operation),
            "capabilities": ["OVS", "routing", "firewall", "DNS", "VPN", "troubleshooting"],
            "note": "This agent provides structure for network expertise. Connect to an LLM for detailed help."
        }))
    }

    async fn database_architect(&self, args: Value) -> Result<Value> {
        let operation = args
            .as_object()
            .and_then(|o| o.get("operation"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing operation"))?;
        let db_type = args
            .as_object()
            .and_then(|o| o.get("database_type"))
            .and_then(|v| v.as_str())
            .unwrap_or("generic");

        Ok(json!({
            "status": "success",
            "operation": operation,
            "database_type": db_type,
            "message": format!("Database architect ready for {} on {}", operation, db_type),
            "note": "This agent provides structure for database architecture. Connect to an LLM for detailed help."
        }))
    }

    async fn security_auditor(&self, args: Value) -> Result<Value> {
        let operation = args
            .as_object()
            .and_then(|o| o.get("operation"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing operation"))?;
        let target_type = args
            .as_object()
            .and_then(|o| o.get("target_type"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        Ok(json!({
            "status": "success",
            "operation": operation,
            "target_type": target_type,
            "message": format!("Security auditor ready for {} on {}", operation, target_type),
            "note": "This agent provides structure for security auditing. Connect to an LLM for detailed analysis."
        }))
    }

    async fn kubernetes_expert(&self, args: Value) -> Result<Value> {
        let operation = args
            .as_object()
            .and_then(|o| o.get("operation"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing operation"))?;

        Ok(json!({
            "status": "success",
            "operation": operation,
            "message": format!("Kubernetes expert ready for {} operation", operation),
            "capabilities": ["deployment", "services", "ingress", "configmaps", "secrets", "troubleshooting"],
            "note": "This agent provides structure for Kubernetes help. Connect to an LLM for detailed assistance."
        }))
    }
}

// ============================================================================
// MCP Protocol Handler
// ============================================================================

async fn handle_request(server: &AgentServer, request: JsonRpcRequest) -> JsonRpcResponse {
    match request.method.as_str() {
        "initialize" => JsonRpcResponse::success(
            request.id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {
                    "tools": { "listChanged": false }
                },
                "serverInfo": {
                    "name": SERVER_NAME,
                    "version": SERVER_VERSION
                }
            }),
        ),
        "initialized" => {
            // Notification, no response needed
            JsonRpcResponse::success(request.id, json!({}))
        }
        "tools/list" => {
            let tools = get_agent_tools();
            JsonRpcResponse::success(
                request.id,
                json!({
                    "tools": tools
                }),
            )
        }
        "tools/call" => {
            let tool_name = request
                .params
                .as_object()
                .and_then(|o| o.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let arguments = request
                .params
                .as_object()
                .and_then(|o| o.get("arguments"))
                .cloned()
                .unwrap_or(json!({}));

            match server.execute_tool(tool_name, arguments).await {
                Ok(result) => JsonRpcResponse::success(
                    request.id,
                    json!({
                        "content": [{
                            "type": "text",
                            "text": simd_json::to_string_pretty(&result).unwrap_or_default()
                        }],
                        "isError": false
                    }),
                ),
                Err(e) => JsonRpcResponse::success(
                    request.id,
                    json!({
                        "content": [{
                            "type": "text",
                            "text": format!("Error: {}", e)
                        }],
                        "isError": true
                    }),
                ),
            }
        }
        "ping" => JsonRpcResponse::success(request.id, json!({})),
        _ => JsonRpcResponse::error(
            request.id,
            -32601,
            format!("Method not found: {}", request.method),
        ),
    }
}

// ============================================================================
// Main Entry Point
// ============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    // Set up stderr logging (stdout is for JSON-RPC)
    eprintln!(
        "[{}] Starting {} v{}",
        SERVER_NAME, SERVER_NAME, SERVER_VERSION
    );

    let server = AgentServer::new();
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let mut line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[{}] Error reading stdin: {}", SERVER_NAME, e);
                break;
            }
        };

        if line.trim().is_empty() {
            continue;
        }

        // Parse JSON-RPC request
        let request: JsonRpcRequest = match unsafe { simd_json::from_str(&mut line) } {
            Ok(r) => r,
            Err(e) => {
                eprintln!("[{}] Parse error: {}", SERVER_NAME, e);
                let error_response =
                    JsonRpcResponse::error(None, -32700, format!("Parse error: {}", e));
                let _ = writeln!(stdout, "{}", simd_json::to_string(&error_response).unwrap());
                let _ = stdout.flush();
                continue;
            }
        };

        eprintln!("[{}] Received: {}", SERVER_NAME, request.method);

        // Handle request
        let response = handle_request(&server, request).await;

        // Write response
        if let Err(e) = writeln!(stdout, "{}", simd_json::to_string(&response).unwrap()) {
            eprintln!("[{}] Error writing response: {}", SERVER_NAME, e);
            break;
        }
        let _ = stdout.flush();
    }

    eprintln!("[{}] Shutting down", SERVER_NAME);
    Ok(())
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/agents_server.rs">
//! Agents MCP Server - D-Bus First Architecture
//!
//! Discovers agents via D-Bus introspection and exposes them as MCP tools.
//! This is the proper architecture for Project D-Bus.
//!
//! ## How It Works
//!
//! 1. Agent Manager starts agents as D-Bus services
//!    - org.dbusmcp.Agent.RustPro
//!    - org.dbusmcp.Agent.PythonPro
//!    - etc.
//!
//! 2. This server uses introspection to discover running agents
//!    - Lists services matching org.dbusmcp.Agent.*
//!    - Introspects each to get methods/properties
//!
//! 3. Exposes discovered agents as MCP tools
//!    - rust_pro_check, rust_pro_build, etc.
//!
//! 4. Tool calls are proxied to D-Bus
//!    - MCP tool call -> D-Bus method call -> Agent execution

use anyhow::{Context, Result};
use op_core::BusType;
use op_introspection::ServiceScanner;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use zbus::Connection;

/// Agent discovered via D-Bus introspection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredAgent {
    pub id: String,
    pub name: String,
    pub description: String,
    pub agent_type: String,
    pub service_name: String,
    pub object_path: String,
    pub operations: Vec<String>,
    pub available: bool,
}

/// MCP tool derived from a D-Bus agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTool {
    pub name: String,
    pub description: String,
    pub agent_id: String,
    pub operation: String,
    pub input_schema: Value,
}

/// Agents MCP Server - discovers and exposes D-Bus agents
pub struct AgentsServer {
    scanner: ServiceScanner,
    connection: Arc<RwLock<Option<Connection>>>,
    discovered_agents: Arc<RwLock<HashMap<String, DiscoveredAgent>>>,
    tools: Arc<RwLock<Vec<AgentTool>>>,
    bus_type: BusType,
}

impl AgentsServer {
    /// Create a new agents server
    pub fn new(bus_type: BusType) -> Self {
        Self {
            scanner: ServiceScanner::new(),
            connection: Arc::new(RwLock::new(None)),
            discovered_agents: Arc::new(RwLock::new(HashMap::new())),
            tools: Arc::new(RwLock::new(Vec::new())),
            bus_type,
        }
    }

    /// Initialize - connect to D-Bus and discover agents
    pub async fn initialize(&self) -> Result<()> {
        info!("Initializing Agents MCP Server (D-Bus first)");

        // Connect to D-Bus
        let conn = match self.bus_type {
            BusType::System => Connection::system().await?,
            BusType::Session => Connection::session().await?,
        };

        {
            let mut connection = self.connection.write().await;
            *connection = Some(conn);
        }

        // Discover agents
        self.discover_agents().await?;

        Ok(())
    }

    /// Discover agents via D-Bus introspection
    pub async fn discover_agents(&self) -> Result<()> {
        info!("Discovering D-Bus agents...");

        // List all services on the bus
        let services = self.scanner.list_services(self.bus_type).await?;

        // Filter for agent services
        let agent_services: Vec<_> = services
            .iter()
            .filter(|s| s.name.starts_with("org.dbusmcp.Agent."))
            .collect();

        info!("Found {} agent services on D-Bus", agent_services.len());

        let mut discovered = self.discovered_agents.write().await;
        let mut tools = self.tools.write().await;

        discovered.clear();
        tools.clear();

        // Introspect each agent service
        for service in agent_services {
            match self.introspect_agent(&service.name).await {
                Ok(agent) => {
                    info!("  ✓ {} ({} operations)", agent.name, agent.operations.len());

                    // Create tools for each operation
                    for op in &agent.operations {
                        let tool = AgentTool {
                            name: format!("{}_{}", agent.id, op),
                            description: format!(
                                "[{}] {} - {} operation",
                                agent.name, agent.description, op
                            ),
                            agent_id: agent.id.clone(),
                            operation: op.clone(),
                            input_schema: self.get_operation_schema(&agent.agent_type, op),
                        };
                        tools.push(tool);
                    }

                    discovered.insert(agent.id.clone(), agent);
                }
                Err(e) => {
                    warn!("  ✗ Failed to introspect {}: {}", service.name, e);
                }
            }
        }

        info!(
            "Discovered {} agents with {} total tools",
            discovered.len(),
            tools.len()
        );

        Ok(())
    }

    /// Introspect a single agent service
    async fn introspect_agent(&self, service_name: &str) -> Result<DiscoveredAgent> {
        let connection = self.connection.read().await;
        let conn = connection
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not connected to D-Bus"))?;

        // Extract agent type from service name
        // org.dbusmcp.Agent.RustPro -> rust_pro
        let agent_type_pascal = service_name
            .strip_prefix("org.dbusmcp.Agent.")
            .ok_or_else(|| anyhow::anyhow!("Invalid agent service name"))?;
        let agent_type = pascal_to_snake(agent_type_pascal);
        let agent_id = agent_type.clone();

        // Object path
        let object_path = format!("/org/dbusmcp/Agent/{}", agent_type_pascal);

        // Create proxy to call introspection methods
        let proxy =
            zbus::Proxy::new(conn, &*service_name, &*object_path, "org.dbusmcp.Agent").await?;

        // Get agent metadata
        let name: String = proxy
            .call("name", &())
            .await
            .unwrap_or_else(|_| agent_type_pascal.to_string());

        let description: String = proxy
            .call("description", &())
            .await
            .unwrap_or_else(|_| "D-Bus agent".to_string());

        let operations: Vec<String> = proxy
            .call("operations", &())
            .await
            .unwrap_or_else(|_| vec!["execute".to_string()]);

        Ok(DiscoveredAgent {
            id: agent_id,
            name,
            description,
            agent_type,
            service_name: service_name.to_string(),
            object_path,
            operations,
            available: true,
        })
    }

    /// Get input schema for an operation
    fn get_operation_schema(&self, _agent_type: &str, _operation: &str) -> Value {
        // Default schema - agents can override via D-Bus properties
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to operate on"
                },
                "args": {
                    "type": "string",
                    "description": "Additional arguments"
                }
            }
        })
    }

    /// Execute a tool by calling the D-Bus agent
    pub async fn execute_tool(&self, tool_name: &str, arguments: Value) -> Result<Value> {
        debug!("Executing tool: {} with args: {:?}", tool_name, arguments);

        // Find the tool
        let tools = self.tools.read().await;
        let tool = tools
            .iter()
            .find(|t| t.name == tool_name)
            .ok_or_else(|| anyhow::anyhow!("Tool not found: {}", tool_name))?;

        // Find the agent
        let agents = self.discovered_agents.read().await;
        let agent = agents
            .get(&tool.agent_id)
            .ok_or_else(|| anyhow::anyhow!("Agent not found: {}", tool.agent_id))?;

        if !agent.available {
            return Err(anyhow::anyhow!("Agent {} is not available", agent.id));
        }

        // Get D-Bus connection
        let connection = self.connection.read().await;
        let conn = connection
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Not connected to D-Bus"))?;

        // Create proxy
        let proxy = zbus::Proxy::new(
            conn,
            &*agent.service_name,
            &*agent.object_path,
            "org.dbusmcp.Agent",
        )
        .await?;

        // Build task JSON
        let task = json!({
            "task_type": agent.agent_type,
            "operation": tool.operation,
            "path": arguments.get("path").and_then(|v| v.as_str()),
            "args": arguments.get("args").and_then(|v| v.as_str()),
            "config": arguments.get("config").cloned().unwrap_or(json!({}))
        });

        let task_json = simd_json::to_string(&task)?;

        // Call D-Bus method
        let result: String = proxy
            .call("Execute", &(task_json,))
            .await
            .context("D-Bus Execute call failed")?;

        // Execute operation
        let mut result_mut = result.clone();
        let result_value: Value = unsafe { simd_json::from_str(&mut result_mut) }
            .unwrap_or(json!({ "raw_output": result }));

        Ok(result_value)
    }

    /// Get list of available agents
    pub async fn list_agents(&self) -> Vec<DiscoveredAgent> {
        let agents = self.discovered_agents.read().await;
        agents.values().cloned().collect()
    }

    /// Get list of available tools (for MCP tools/list)
    pub async fn list_tools(&self) -> Vec<Value> {
        let tools = self.tools.read().await;
        tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "inputSchema": t.input_schema
                })
            })
            .collect()
    }

    /// Refresh agent discovery
    pub async fn refresh(&self) -> Result<()> {
        info!("Refreshing agent discovery...");
        self.discover_agents().await
    }
}

/// Convert PascalCase to snake_case
fn pascal_to_snake(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pascal_to_snake() {
        assert_eq!(pascal_to_snake("RustPro"), "rust_pro");
        assert_eq!(pascal_to_snake("PythonPro"), "python_pro");
        assert_eq!(pascal_to_snake("SequentialThinking"), "sequential_thinking");
        assert_eq!(pascal_to_snake("BackendArchitect"), "backend_architect");
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/agents_server.rs.patch">
// Replace DbusAgentExecutor with TraitAgentExecutor that uses existing agent implementations

// Find this section and ADD the new executor:

/// Trait-based agent executor - uses AgentTrait implementations directly
/// This works WITHOUT separate D-Bus service processes
pub struct TraitAgentExecutor {
    agents: std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<String, Box<dyn op_agents::agents::base::AgentTrait + Send + Sync>>>>,
}

impl TraitAgentExecutor {
    pub fn new() -> Self {
        use op_agents::agents::{
            language::{RustProAgent, PythonProAgent},
            architecture::BackendArchitectAgent,
            infrastructure::NetworkEngineerAgent,
            orchestration::{MemoryAgent, ContextManagerAgent, SequentialThinkingAgent},
        };
        
        let mut agents: std::collections::HashMap<String, Box<dyn op_agents::agents::base::AgentTrait + Send + Sync>> = std::collections::HashMap::new();
        
        // Register the run-on-connection agents
        agents.insert("rust_pro".to_string(), Box::new(RustProAgent::new("rust_pro".to_string())));
        agents.insert("python_pro".to_string(), Box::new(PythonProAgent::new("python_pro".to_string())));
        agents.insert("backend_architect".to_string(), Box::new(BackendArchitectAgent::new("backend_architect".to_string())));
        agents.insert("network_engineer".to_string(), Box::new(NetworkEngineerAgent::new("network_engineer".to_string())));
        agents.insert("memory".to_string(), Box::new(MemoryAgent::new("memory".to_string())));
        agents.insert("context_manager".to_string(), Box::new(ContextManagerAgent::new("context_manager".to_string())));
        agents.insert("sequential_thinking".to_string(), Box::new(SequentialThinkingAgent::new("sequential_thinking".to_string())));
        
        Self {
            agents: std::sync::Arc::new(tokio::sync::RwLock::new(agents)),
        }
    }
}

impl Default for TraitAgentExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AgentExecutor for TraitAgentExecutor {
    async fn start_agent(&self, agent_id: &str, _dbus_service: Option<&str>) -> anyhow::Result<()> {
        tracing::info!(agent = %agent_id, "Agent ready (trait-based, no D-Bus)");
        Ok(())
    }
    
    async fn stop_agent(&self, agent_id: &str) -> anyhow::Result<()> {
        tracing::info!(agent = %agent_id, "Agent stopped");
        Ok(())
    }
    
    async fn execute(&self, agent_id: &str, operation: &str, args: serde_json::Value) -> anyhow::Result<serde_json::Value> {
        use op_agents::agents::base::AgentTask;
        
        let agents = self.agents.read().await;
        let agent = agents.get(agent_id)
            .ok_or_else(|| anyhow::anyhow!("Agent not found: {}", agent_id))?;
        
        let task = AgentTask {
            task_type: agent_id.replace('_', "-"),
            operation: operation.to_string(),
            path: args.get("path").and_then(|p| p.as_str()).map(String::from),
            args: Some(serde_json::to_string(&args).unwrap_or_default()),
            config: args.as_object()
                .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default(),
        };
        
        match agent.execute(task).await {
            Ok(result) => {
                Ok(serde_json::json!({
                    "success": result.success,
                    "output": result.data,
                    "operation": operation
                }))
            }
            Err(e) => {
                Err(anyhow::anyhow!("Agent execution failed: {}", e))
            }
        }
    }
    
    async fn is_running(&self, agent_id: &str) -> bool {
        self.agents.read().await.contains_key(agent_id)
    }
}

// Then update AgentsServer::new() to use TraitAgentExecutor:
impl AgentsServer {
    pub fn new(config: AgentsServerConfig) -> Self {
        Self {
            config,
            executor: std::sync::Arc::new(TraitAgentExecutor::new()),  // <-- Changed from DbusAgentExecutor
            client_info: tokio::sync::RwLock::new(None),
            running_agents: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        }
    }
    // ... rest unchanged
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/builtin_trait_agents.rs">
//! Built-in Trait Agent Implementations
//!
//! These provide the fallback implementations when D-Bus services aren't available.
//! They use op-agents crate implementations internally.

use anyhow::Result;
use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

use super::agents_server::AgentTraitImpl;

// =============================================================================
// MEMORY AGENT
// =============================================================================

/// In-memory implementation of the memory agent
pub struct MemoryAgentImpl {
    memories: RwLock<HashMap<String, MemoryEntry>>,
}

#[derive(Clone)]
struct MemoryEntry {
    value: String,
    tags: Vec<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl MemoryAgentImpl {
    pub fn new() -> Self {
        Self {
            memories: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for MemoryAgentImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentTraitImpl for MemoryAgentImpl {
    fn agent_id(&self) -> &str {
        "memory"
    }
    
    async fn execute(&self, operation: &str, args: Value) -> Result<Value> {
        match operation {
            "store" => {
                let key = args["key"].as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'key' parameter"))?;
                let value = args["value"].as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'value' parameter"))?;
                let tags: Vec<String> = args["tags"].as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                
                let mut memories = self.memories.write().await;
                memories.insert(key.to_string(), MemoryEntry {
                    value: value.to_string(),
                    tags,
                    created_at: chrono::Utc::now(),
                });
                
                debug!("Memory stored: {}", key);
                Ok(json!({ "success": true, "key": key }))
            }
            
            "recall" => {
                let memories = self.memories.read().await;
                
                if let Some(key) = args["key"].as_str() {
                    if let Some(entry) = memories.get(key) {
                        return Ok(json!({
                            "found": true,
                            "key": key,
                            "value": entry.value,
                            "tags": entry.tags,
                        }));
                    } else {
                        return Ok(json!({ "found": false, "key": key }));
                    }
                }
                
                if let Some(query) = args["query"].as_str() {
                    let query_lower = query.to_lowercase();
                    let matches: Vec<_> = memories.iter()
                        .filter(|(k, v)| {
                            k.to_lowercase().contains(&query_lower) ||
                            v.value.to_lowercase().contains(&query_lower) ||
                            v.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
                        })
                        .map(|(k, v)| json!({
                            "key": k,
                            "value": v.value,
                            "tags": v.tags,
                        }))
                        .collect();
                    
                    return Ok(json!({
                        "found": !matches.is_empty(),
                        "query": query,
                        "matches": matches,
                    }));
                }
                
                Err(anyhow::anyhow!("Either 'key' or 'query' parameter required"))
            }
            
            "list" => {
                let memories = self.memories.read().await;
                let limit = args["limit"].as_u64().unwrap_or(100) as usize;
                let filter_tags: Option<Vec<String>> = args["tags"].as_array()
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect());
                
                let mut entries: Vec<_> = memories.iter()
                    .filter(|(_, v)| {
                        if let Some(ref tags) = filter_tags {
                            tags.iter().any(|t| v.tags.contains(t))
                        } else {
                            true
                        }
                    })
                    .take(limit)
                    .map(|(k, v)| json!({
                        "key": k,
                        "value": v.value,
                        "tags": v.tags,
                    }))
                    .collect();
                
                Ok(json!({
                    "count": entries.len(),
                    "memories": entries,
                }))
            }
            
            _ => Err(anyhow::anyhow!("Unknown operation: {}", operation)),
        }
    }
}

// =============================================================================
// SEQUENTIAL THINKING AGENT
// =============================================================================

/// Sequential thinking agent implementation
pub struct SequentialThinkingAgentImpl {
    thoughts: RwLock<Vec<ThoughtStep>>,
}

#[derive(Clone)]
struct ThoughtStep {
    step: usize,
    thought: String,
    timestamp: chrono::DateTime<chrono::Utc>,
}

impl SequentialThinkingAgentImpl {
    pub fn new() -> Self {
        Self {
            thoughts: RwLock::new(Vec::new()),
        }
    }
}

impl Default for SequentialThinkingAgentImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentTraitImpl for SequentialThinkingAgentImpl {
    fn agent_id(&self) -> &str {
        "sequential_thinking"
    }
    
    async fn execute(&self, operation: &str, args: Value) -> Result<Value> {
        match operation {
            "think" => {
                let thought = args["thought"].as_str()
                    .ok_or_else(|| anyhow::anyhow!("Missing 'thought' parameter"))?;
                
                let mut thoughts = self.thoughts.write().await;
                let step = args["step"].as_u64().map(|s| s as usize)
                    .unwrap_or(thoughts.len() + 1);
                
                thoughts.push(ThoughtStep {
                    step,
                    thought: thought.to_string(),
                    timestamp: chrono::Utc::now(),
                });
                
                debug!("Thought step {} recorded", step);
                
                Ok(json!({
                    "success": true,
                    "step": step,
                    "total_thoughts": thoughts.len(),
                }))
            }
            
            "summarize" => {
                let thoughts = self.thoughts.read().await;
                let steps: Vec<_> = thoughts.iter()
                    .map(|t| json!({
                        "step": t.step,
                        "thought": t.thought,
                    }))
                    .collect();
                
                Ok(json!({
                    "total_steps": steps.len(),
                    "thoughts": steps,
                }))
            }
            
            "clear" => {
                let mut thoughts = self.thoughts.write().await;
                let count = thoughts.len();
                thoughts.clear();
                
                Ok(json!({
                    "success": true,
                    "cleared": count,
                }))
            }
            
            _ => Err(anyhow::anyhow!("Unknown operation: {}", operation)),
        }
    }
}

// =============================================================================
// REGISTRATION HELPER
// =============================================================================

/// Register all built-in trait agents with the server
pub async fn register_builtin_agents(server: &super::agents_server::AgentsServer) {
    tracing::info!("Registering built-in trait agent implementations");
    
    // Memory agent
    server.register_trait_agent(Box::new(MemoryAgentImpl::new())).await;
    
    // Sequential thinking
    server.register_trait_agent(Box::new(SequentialThinkingAgentImpl::new())).await;
    
    // TODO: Add more built-in agents as needed
    // server.register_trait_agent(Box::new(RustProAgentImpl::new())).await;
    // server.register_trait_agent(Box::new(PythonProAgentImpl::new())).await;
    
    tracing::info!("Built-in trait agents registered");
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/compact_main.rs">
//! Compact MCP Server Main
//!
//! Runs the compact MCP server in stdio mode with five meta-tools.

use op_mcp::compact::run_compact_stdio_server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    run_compact_stdio_server().await
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/compact.rs">
//! Compact Mode
//!
//! Provides 5 meta-tools for discovering and executing system tools:
//! - list_tools: Browse available tools with filtering
//! - search_tools: Search tools by keyword
//! - get_tool_schema: Get input schema for a specific tool
//! - execute_tool: Execute any tool by name
//! - respond: Send the final user response
//!
//! This mode saves ~95% of context tokens compared to exposing all tools.

use crate::{JsonRpcError, McpRequest, McpResponse, ToolExecutor, ToolInfo};
use anyhow::Result;
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info};

pub struct LazyOpToolsExecutor;

impl LazyOpToolsExecutor {
    async fn load_registry() -> Result<op_tools::ToolRegistry> {
        let registry = op_tools::ToolRegistry::new();
        op_tools::register_builtin_tools(&registry).await?;
        Ok(registry)
    }
}

#[async_trait::async_trait]
impl ToolExecutor for LazyOpToolsExecutor {
    async fn list_tools(&self) -> Result<Vec<ToolInfo>> {
        let registry = Self::load_registry().await?;
        Ok(registry
            .list()
            .await
            .into_iter()
            .map(|tool| ToolInfo {
                name: tool.name,
                description: tool.description,
                input_schema: tool.input_schema,
                annotations: None,
            })
            .collect())
    }

    async fn execute_tool(&self, name: &str, arguments: Value) -> Result<Value> {
        let registry = Self::load_registry().await?;
        let tool = registry
            .get(name)
            .await
            .ok_or_else(|| anyhow::anyhow!("Tool not found: {}", name))?;
        tool.execute(arguments).await
    }

    async fn get_tool_schema(&self, name: &str) -> Result<Option<Value>> {
        let registry = Self::load_registry().await?;
        Ok(registry
            .get_definition(name)
            .await
            .map(|definition| definition.input_schema))
    }

    async fn search_tools(&self, query: &str, limit: usize) -> Result<Vec<ToolInfo>> {
        let query = query.to_lowercase();
        let registry = Self::load_registry().await?;
        Ok(registry
            .list()
            .await
            .into_iter()
            .filter(|tool| {
                tool.name.to_lowercase().contains(&query)
                    || tool.description.to_lowercase().contains(&query)
                    || tool.category.to_lowercase().contains(&query)
                    || tool
                        .tags
                        .iter()
                        .any(|tag| tag.to_lowercase().contains(&query))
            })
            .take(limit)
            .map(|tool| ToolInfo {
                name: tool.name,
                description: tool.description,
                input_schema: tool.input_schema,
                annotations: None,
            })
            .collect())
    }
}

/// Session context passed through from gateway
#[derive(Debug, Clone, Default)]
pub struct SessionContext {
    pub session_id: Option<String>,
    pub is_controller: bool,
    pub peer_pubkey: Option<String>,
}

/// Compact server wraps a tool executor and exposes 5 meta-tools
pub struct CompactServer {
    executor: Arc<dyn ToolExecutor>,
    server_name: String,
    session: RwLock<SessionContext>,
}

impl CompactServer {
    pub fn new(executor: Arc<dyn ToolExecutor>) -> Self {
        Self {
            executor,
            server_name: "op-mcp-compact".to_string(),
            session: RwLock::new(SessionContext::default()),
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.server_name = name.into();
        self
    }

    /// Set session context (called by gateway after auth)
    pub async fn set_session(&self, ctx: SessionContext) {
        info!(
            session_id = ?ctx.session_id,
            is_controller = %ctx.is_controller,
            "Session context set"
        );
        *self.session.write().await = ctx;
    }

    /// Check if current session can execute controller-only tools
    pub async fn can_execute_controller_tools(&self) -> bool {
        self.session.read().await.is_controller
    }

    /// Handle MCP request
    pub async fn handle_request(&self, request: McpRequest) -> McpResponse {
        debug!(method = %request.method, "Handling compact MCP request");

        match request.method.as_str() {
            "initialize" => self.handle_initialize(request).await,
            "initialized" => McpResponse::success(request.id, json!({})),
            "ping" => McpResponse::success(request.id, json!({})),
            "tools/list" => self.handle_tools_list(request).await,
            "tools/call" => self.handle_tools_call(request).await,
            "notifications/initialized" => McpResponse::success(request.id, json!({})),
            _ => McpResponse::error(request.id, JsonRpcError::method_not_found(&request.method)),
        }
    }

    async fn handle_initialize(&self, request: McpRequest) -> McpResponse {
        info!("Compact MCP initialized");

        McpResponse::success(
            request.id,
            json!({
                "protocolVersion": crate::PROTOCOL_VERSION,
                "capabilities": {
                    "tools": { "listChanged": false }
                },
                "serverInfo": {
                    "name": self.server_name,
                    "version": crate::SERVER_VERSION
                },
                "instructions": "This server uses compact mode with 5 meta-tools. Use list_tools to discover available tools, get_tool_schema to get the input schema, execute_tool to run tools, and respond for the final answer."
            }),
        )
    }

    async fn handle_tools_list(&self, request: McpRequest) -> McpResponse {
        McpResponse::success(
            request.id,
            json!({
                "tools": compact_tools_schema(),
                "_meta": { "compactMode": true }
            }),
        )
    }

    async fn handle_tools_call(&self, request: McpRequest) -> McpResponse {
        let params = match &request.params {
            Some(p) => p,
            None => {
                return McpResponse::error(
                    request.id,
                    JsonRpcError::invalid_params("Missing params"),
                )
            }
        };

        let tool_name = match params
            .as_object()
            .and_then(|o| o.get("name"))
            .and_then(|n| n.as_str())
        {
            Some(n) => n,
            None => {
                return McpResponse::error(
                    request.id,
                    JsonRpcError::invalid_params("Missing tool name"),
                )
            }
        };

        let arguments = params
            .as_object()
            .and_then(|o| o.get("arguments"))
            .cloned()
            .unwrap_or(json!({}));

        // Route to meta-tool handlers
        match tool_name {
            "list_tools" => self.meta_list_tools(request.id, arguments).await,
            "search_tools" => self.meta_search_tools(request.id, arguments).await,
            "get_tool_schema" => self.meta_get_tool_schema(request.id, arguments).await,
            "execute_tool" => self.meta_execute_tool(request.id, arguments).await,
            "respond" => self.meta_respond(request.id, arguments).await,
            _ => McpResponse::error(
                request.id,
                JsonRpcError::new(-32001, format!(
                    "Unknown meta-tool: {}. Use list_tools, search_tools, get_tool_schema, execute_tool, or respond.",
                    tool_name
                )),
            ),
        }
    }

    async fn meta_list_tools(&self, id: Option<Value>, args: Value) -> McpResponse {
        let category = args
            .as_object()
            .and_then(|o| o.get("category"))
            .and_then(|c| c.as_str());
        let limit = args
            .as_object()
            .and_then(|o| o.get("limit"))
            .and_then(|l| l.as_u64())
            .unwrap_or(20) as usize;
        let offset = args
            .as_object()
            .and_then(|o| o.get("offset"))
            .and_then(|o| o.as_u64())
            .unwrap_or(0) as usize;

        match self.executor.list_tools().await {
            Ok(tools) => {
                let filtered: Vec<_> = tools
                    .into_iter()
                    .filter(|t| {
                        category
                            .map(|c| {
                                t.name.contains(c)
                                    || t.description.to_lowercase().contains(&c.to_lowercase())
                            })
                            .unwrap_or(true)
                    })
                    .skip(offset)
                    .take(limit)
                    .map(|t| {
                        json!({
                            "name": t.name,
                            "description": t.description
                        })
                    })
                    .collect();

                let total = filtered.len();

                McpResponse::success(
                    id,
                    json!({
                        "content": [{
                            "type": "text",
                            "text": simd_json::to_string_pretty(&json!({
                                "tools": filtered,
                                "count": total,
                                "offset": offset,
                                "limit": limit
                            })).unwrap()
                        }],
                        "isError": false
                    }),
                )
            }
            Err(e) => {
                error!(error = %e, "Failed to list tools");
                McpResponse::success(
                    id,
                    json!({
                        "content": [{ "type": "text", "text": format!("Error: {}", e) }],
                        "isError": true
                    }),
                )
            }
        }
    }

    async fn meta_search_tools(&self, id: Option<Value>, args: Value) -> McpResponse {
        let query = args
            .as_object()
            .and_then(|o| o.get("query"))
            .and_then(|q| q.as_str())
            .unwrap_or("");
        let limit = args
            .as_object()
            .and_then(|o| o.get("limit"))
            .and_then(|l| l.as_u64())
            .unwrap_or(10) as usize;

        match self.executor.search_tools(query, limit).await {
            Ok(tools) => {
                let results: Vec<_> = tools
                    .into_iter()
                    .map(|t| {
                        json!({
                            "name": t.name,
                            "description": t.description
                        })
                    })
                    .collect();

                McpResponse::success(
                    id,
                    json!({
                        "content": [{
                            "type": "text",
                            "text": simd_json::to_string_pretty(&json!({
                                "query": query,
                                "results": results,
                                "count": results.len()
                            })).unwrap()
                        }],
                        "isError": false
                    }),
                )
            }
            Err(e) => {
                error!(error = %e, "Failed to search tools");
                McpResponse::success(
                    id,
                    json!({
                        "content": [{ "type": "text", "text": format!("Error: {}", e) }],
                        "isError": true
                    }),
                )
            }
        }
    }

    async fn meta_get_tool_schema(&self, id: Option<Value>, args: Value) -> McpResponse {
        let tool_name = args
            .as_object()
            .and_then(|o| o.get("tool_name"))
            .and_then(|n| n.as_str())
            .unwrap_or("");

        if tool_name.is_empty() {
            return McpResponse::success(
                id,
                json!({
                    "content": [{ "type": "text", "text": "Error: tool_name is required" }],
                    "isError": true
                }),
            );
        }

        match self.executor.get_tool_schema(tool_name).await {
            Ok(Some(schema)) => McpResponse::success(
                id,
                json!({
                    "content": [{
                        "type": "text",
                        "text": simd_json::to_string_pretty(&json!({
                            "tool": tool_name,
                            "schema": schema
                        })).unwrap()
                    }],
                    "isError": false
                }),
            ),
            Ok(None) => McpResponse::success(
                id,
                json!({
                    "content": [{ "type": "text", "text": format!("Tool not found: {}", tool_name) }],
                    "isError": true
                }),
            ),
            Err(e) => {
                error!(error = %e, "Failed to get tool schema");
                McpResponse::success(
                    id,
                    json!({
                        "content": [{ "type": "text", "text": format!("Error: {}", e) }],
                        "isError": true
                    }),
                )
            }
        }
    }

    async fn meta_execute_tool(&self, id: Option<Value>, args: Value) -> McpResponse {
        let tool_name = args
            .as_object()
            .and_then(|o| o.get("tool_name"))
            .and_then(|n| n.as_str())
            .unwrap_or("");
        let arguments = args
            .as_object()
            .and_then(|o| o.get("arguments"))
            .cloned()
            .unwrap_or(json!({}));

        if tool_name.is_empty() {
            return McpResponse::success(
                id,
                json!({
                    "content": [{ "type": "text", "text": "Error: tool_name is required" }],
                    "isError": true
                }),
            );
        }

        info!(tool = %tool_name, "Executing tool via compact mode");

        match self.executor.execute_tool(tool_name, arguments).await {
            Ok(result) => {
                let text = simd_json::to_string_pretty(&result).unwrap_or_default();
                McpResponse::success(
                    id,
                    json!({
                        "content": [{
                            "type": "text",
                            "text": text
                        }],
                        "isError": false
                    }),
                )
            }
            Err(e) => {
                error!(tool = %tool_name, error = %e, "Tool execution failed");
                McpResponse::success(
                    id,
                    json!({
                        "content": [{
                            "type": "text",
                            "text": format!("Error executing {}: {}", tool_name, e)
                        }],
                        "isError": true
                    }),
                )
            }
        }
    }

    async fn meta_respond(&self, id: Option<Value>, args: Value) -> McpResponse {
        let message = args
            .as_object()
            .and_then(|o| o.get("message"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        McpResponse::success(
            id,
            json!({
                "content": [{
                    "type": "text",
                    "text": message
                }],
                "isError": false
            }),
        )
    }
}

/// Get the 5 compact meta-tool schemas
pub fn compact_tools_schema() -> Vec<Value> {
    vec![
        json!({
            "name": "list_tools",
            "description": "List available tools. Filter by category. Returns tool names and descriptions.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "category": {
                        "type": "string",
                        "description": "Filter by category (e.g., 'ovs', 'dbus', 'file', 'agent')"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum tools to return",
                        "default": 20
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Offset for pagination",
                        "default": 0
                    }
                }
            }
        }),
        json!({
            "name": "search_tools",
            "description": "Search tools by keyword in name or description.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum results",
                        "default": 10
                    }
                },
                "required": ["query"]
            }
        }),
        json!({
            "name": "get_tool_schema",
            "description": "Get the input schema for a specific tool. Call this before execute_tool to know the required arguments.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tool_name": {
                        "type": "string",
                        "description": "Name of the tool"
                    }
                },
                "required": ["tool_name"]
            }
        }),
        json!({
            "name": "execute_tool",
            "description": "Execute any tool by name with arguments. First use get_tool_schema to see required arguments.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tool_name": {
                        "type": "string",
                        "description": "Name of the tool to execute"
                    },
                    "arguments": {
                        "type": "object",
                        "description": "Arguments to pass to the tool"
                    }
                },
                "required": ["tool_name"]
            }
        }),
        json!({
            "name": "respond",
            "description": "Send the final response to the user.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "Response message"
                    }
                },
                "required": ["message"]
            }
        }),
    ]
}

/// Run compact server in stdio mode
pub async fn run_compact_stdio_server() -> Result<()> {
    use crate::transport::{StdioTransport, Transport};

    // Initialize logging to stderr
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .with_writer(std::io::stderr)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // ── WireGuard identity ────────────────────────────────────────────────────
    // Read the local WG pubkey, write the canonical IdentitySled to /dev/shm,
    // and stamp peer_pubkey into the session so tools can use it for auth.
    let wg_iface = std::env::var("WG_INTERFACE").unwrap_or_else(|_| "netmaker".to_string());
    let wg_id = op_identity::WireGuardIdentity::with_interface(&wg_iface);
    let peer_pubkey = match wg_id.get_local_pubkey() {
        Ok(pubkey) => {
            if let Err(e) = op_identity::write_sled_from_wg(&pubkey) {
                tracing::warn!(error = %e, "Failed to write identity sled to /dev/shm");
            } else {
                info!(interface = %wg_iface, pubkey = %pubkey, "WG identity sled written");
            }
            Some(pubkey)
        }
        Err(e) => {
            tracing::warn!(interface = %wg_iface, error = %e, "Could not read WG public key; set WG_PUBKEY env var to override");
            None
        }
    };

    // Load the authoritative op-tools registry lazily per request so the
    // chatbot sees five stable meta-tools while retaining access to every
    // live system, D-Bus, OVS, and PluginSchema projection tool.
    let executor: Arc<dyn ToolExecutor> = Arc::new(LazyOpToolsExecutor);
    let server = Arc::new(CompactServer::new(executor));

    // Stamp the WG identity into the session context.
    server
        .set_session(SessionContext {
            peer_pubkey,
            ..Default::default()
        })
        .await;

    info!("Starting compact MCP server (stdio)");

    StdioTransport::new().serve(server).await
}

// Implement McpHandler for CompactServer
#[async_trait::async_trait]
impl crate::transport::McpHandler for CompactServer {
    async fn handle_request(&self, request: McpRequest) -> McpResponse {
        self.handle_request(request).await
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/config.rs">
use anyhow::Result;
use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub name: String,
    pub version: String,
    pub tool_config: ToolConfig,
}

#[derive(Debug, Deserialize)]
pub struct ToolConfig {
    pub max_loaded_tools: usize,
    pub min_idle_secs: u64,
    pub enable_dbus_discovery: bool,
    pub enable_plugin_discovery: bool,
    pub enable_agent_discovery: bool,
    pub preload_essential: bool,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let s = Config::builder()
            .add_source(File::with_name("config/default"))
            .add_source(Environment::with_prefix("MCP").separator("_"))
            .build()?;
        s.try_deserialize()
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/external_client.rs">
//! External MCP Client - Connect to and introspect other MCP servers

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::RwLock;

/// External MCP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalMcpConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,

    /// Environment variables to pass to the server
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// API key (will be set as env var or header based on auth_method)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,

    /// API key environment variable name (default: API_KEY)
    #[serde(default = "default_api_key_env")]
    pub api_key_env: String,

    /// Authentication method
    #[serde(default)]
    pub auth_method: AuthMethod,

    /// Custom headers for HTTP-based MCP servers
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

fn default_api_key_env() -> String {
    "API_KEY".to_string()
}

/// Authentication method for MCP servers
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    /// No authentication
    #[default]
    None,

    /// API key in environment variable
    EnvVar,

    /// Bearer token in Authorization header (for HTTP-based MCP)
    BearerToken,

    /// Custom header (specify in headers field)
    CustomHeader,
}

/// External MCP server tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalTool {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
    pub server_name: String,
}

/// External MCP client
pub struct ExternalMcpClient {
    config: ExternalMcpConfig,
    process: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<BufReader<ChildStdout>>,
    tools: RwLock<Vec<ExternalTool>>,
    next_id: RwLock<u64>,
}

impl ExternalMcpClient {
    /// Create new external MCP client
    pub fn new(config: ExternalMcpConfig) -> Self {
        Self {
            config,
            process: None,
            stdin: None,
            stdout: None,
            tools: RwLock::new(Vec::new()),
            next_id: RwLock::new(1),
        }
    }

    /// Start the external MCP server process
    pub async fn start(&mut self) -> Result<()> {
        let start_time = std::time::Instant::now();
        tracing::info!("Starting external MCP server: {}", self.config.name);

        let mut cmd = Command::new(&self.config.command);
        cmd.args(&self.config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        // Add base environment variables
        cmd.envs(&self.config.env);

        // Handle API key authentication
        if let Some(api_key) = &self.config.api_key {
            match self.config.auth_method {
                AuthMethod::None => {
                    tracing::debug!("API key provided but auth_method is None");
                }
                AuthMethod::EnvVar => {
                    tracing::debug!("Setting API key in env var: {}", self.config.api_key_env);
                    cmd.env(&self.config.api_key_env, api_key);
                }
                AuthMethod::BearerToken | AuthMethod::CustomHeader => {
                    tracing::debug!("API key will be used in HTTP headers (not env)");
                    // For HTTP-based MCP, headers are handled at protocol level
                }
            }
        }

        let mut child = cmd
            .spawn()
            .context(format!("Failed to spawn MCP server: {}", self.config.name))?;

        let stdin = child.stdin.take().context("Failed to open stdin")?;
        let stdout = child.stdout.take().context("Failed to open stdout")?;

        self.stdin = Some(stdin);
        self.stdout = Some(BufReader::new(stdout));
        self.process = Some(child);

        // Initialize the MCP server with timeout and retry logic
        let init_start = std::time::Instant::now();
        let max_retries = 3;
        let mut retry_count = 0;

        let init_result = loop {
            match tokio::time::timeout(std::time::Duration::from_secs(10), self.initialize()).await
            {
                Ok(Ok(_)) => {
                    let init_duration = init_start.elapsed();
                    tracing::info!(
                        "External MCP server initialized in {:.2}s",
                        init_duration.as_secs_f32()
                    );
                    break Ok(());
                }
                Ok(Err(e)) => {
                    tracing::error!(
                        "Failed to initialize external MCP server {}: {}",
                        self.config.name,
                        e
                    );
                    break Err(e);
                }
                Err(_) => {
                    retry_count += 1;
                    if retry_count >= max_retries {
                        tracing::error!(
                            "External MCP server {} initialization timed out after {} attempts",
                            self.config.name,
                            max_retries
                        );
                        break Err(anyhow::anyhow!(
                            "Initialization timeout after {} attempts",
                            max_retries
                        ));
                    }
                    tracing::warn!(
                        "Initialization attempt {} timed out, retrying...",
                        retry_count
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    continue;
                }
            }
        };

        if let Err(e) = init_result {
            return Err(e);
        }

        // List available tools with timeout
        let tools_start = std::time::Instant::now();
        let tools_result =
            tokio::time::timeout(std::time::Duration::from_secs(15), self.refresh_tools()).await;

        match tools_result {
            Ok(Ok(_)) => {
                let tools_duration = tools_start.elapsed();
                tracing::info!(
                    "External MCP server tools loaded in {:.2}s",
                    tools_duration.as_secs_f32()
                );
            }
            Ok(Err(e)) => {
                tracing::error!(
                    "Failed to load tools from external MCP server {}: {}",
                    self.config.name,
                    e
                );
                return Err(e);
            }
            Err(_) => {
                tracing::error!(
                    "External MCP server {} tools loading timed out (15s)",
                    self.config.name
                );
                return Err(anyhow::anyhow!("Tools loading timeout"));
            }
        }

        let total_duration = start_time.elapsed();
        tracing::info!(
            "External MCP server started: {} ({} tools) in {:.2}s total",
            self.config.name,
            self.tools.read().await.len(),
            total_duration.as_secs_f32()
        );

        if total_duration.as_secs() > 5 {
            tracing::warn!("External MCP server {} took longer than expected to start (>5s). Consider optimizing or checking for startup issues.", self.config.name);
        }

        Ok(())
    }

    /// Initialize the MCP server
    async fn initialize(&mut self) -> Result<()> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": self.next_id().await,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "op-dbus-mcp-aggregator",
                    "version": "0.1.0"
                }
            }
        });

        let response = self.send_request(request).await?;

        if response.get("error").is_some() {
            anyhow::bail!("Failed to initialize MCP server: {:?}", response);
        }

        tracing::debug!("MCP server initialized: {}", self.config.name);
        Ok(())
    }

    /// Refresh tools list from the MCP server
    pub async fn refresh_tools(&mut self) -> Result<()> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": self.next_id().await,
            "method": "tools/list",
            "params": {}
        });

        let response = self.send_request(request).await?;

        if let Some(error) = response.get("error") {
            anyhow::bail!("Failed to list tools: {:?}", error);
        }

        let tools_array = response
            .get("result")
            .and_then(|r| r.get("tools"))
            .and_then(|t| t.as_array())
            .context("Invalid tools response")?;

        let mut tools = Vec::new();
        for tool in tools_array {
            let name = tool
                .get("name")
                .and_then(|n| n.as_str())
                .context("Tool missing name")?;
            let description = tool
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("");
            let input_schema = tool.get("inputSchema").cloned().unwrap_or(json!({}));

            tools.push(ExternalTool {
                name: format!("{}:{}", self.config.name, name),
                description: format!("[{}] {}", self.config.name, description),
                input_schema,
                server_name: self.config.name.clone(),
            });
        }

        *self.tools.write().await = tools;
        Ok(())
    }

    /// Get all tools from this MCP server
    pub async fn get_tools(&self) -> Vec<ExternalTool> {
        self.tools.read().await.clone()
    }

    /// Call a tool on the external MCP server
    pub async fn call_tool(&mut self, tool_name: &str, arguments: Value) -> Result<Value> {
        // Strip server prefix if present
        let tool_name = tool_name
            .strip_prefix(&format!("{}:", self.config.name))
            .unwrap_or(tool_name);

        let request = json!({
            "jsonrpc": "2.0",
            "id": self.next_id().await,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments
            }
        });

        let response = self.send_request(request).await?;

        if let Some(error) = response.get("error") {
            anyhow::bail!("Tool call failed: {:?}", error);
        }

        response
            .get("result")
            .cloned()
            .context("Missing result in response")
    }

    /// Send request to MCP server and get response
    async fn send_request(&mut self, request: Value) -> Result<Value> {
        let stdin = self.stdin.as_mut().context("MCP server not started")?;
        let stdout = self.stdout.as_mut().context("MCP server not started")?;

        // Send request
        let request_str = simd_json::to_string(&request)?;
        stdin.write_all(request_str.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;

        tracing::debug!("Sent request to {}: {}", self.config.name, request_str);

        // Read response
        let mut response_line = String::new();
        stdout.read_line(&mut response_line).await?;

        tracing::debug!(
            "Received response from {}: {}",
            self.config.name,
            response_line
        );

        let mut response_line = response_line;
        let response: Value = unsafe { simd_json::from_str(&mut response_line) }
            .context("Failed to parse MCP response")?;

        Ok(response)
    }

    /// Get next request ID
    async fn next_id(&self) -> u64 {
        let mut id = self.next_id.write().await;
        let current = *id;
        *id += 1;
        current
    }

    /// Stop the MCP server
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(mut process) = self.process.take() {
            tracing::info!("Stopping external MCP server: {}", self.config.name);
            process.kill().await?;
        }
        Ok(())
    }
}

impl Drop for ExternalMcpClient {
    fn drop(&mut self) {
        if let Some(mut process) = self.process.take() {
            let _ = process.start_kill();
        }
    }
}

/// External MCP manager - manages multiple external MCP servers
pub struct ExternalMcpManager {
    clients: RwLock<HashMap<String, ExternalMcpClient>>,
}

impl ExternalMcpManager {
    /// Create new manager
    pub fn new() -> Self {
        Self {
            clients: RwLock::new(HashMap::new()),
        }
    }

    /// Add and start an external MCP server
    pub async fn add_server(&self, config: ExternalMcpConfig) -> Result<()> {
        let name = config.name.clone();
        let mut client = ExternalMcpClient::new(config);

        client.start().await?;

        self.clients.write().await.insert(name, client);
        Ok(())
    }

    /// Load servers from config file
    pub async fn load_from_file(&self, path: &str) -> Result<()> {
        let content = tokio::fs::read_to_string(path)
            .await
            .context("Failed to read MCP config file")?;

        let mut content = content;
        let configs: Vec<ExternalMcpConfig> =
            unsafe { simd_json::from_str(&mut content) }.context("Failed to parse MCP config")?;

        for config in configs {
            if let Err(e) = self.add_server(config.clone()).await {
                tracing::error!("Failed to start MCP server {}: {}", config.name, e);
            }
        }

        Ok(())
    }

    /// Get all tools from all external MCP servers
    pub async fn get_all_tools(&self) -> Vec<ExternalTool> {
        let clients = self.clients.read().await;
        let mut all_tools = Vec::new();

        for client in clients.values() {
            all_tools.extend(client.get_tools().await);
        }

        all_tools
    }

    /// Call a tool (format: "server:tool" or just "tool")
    pub async fn call_tool(&self, tool_name: &str, arguments: Value) -> Result<Value> {
        let (server_name, actual_tool_name) = if let Some(idx) = tool_name.find(':') {
            (&tool_name[..idx], &tool_name[idx + 1..])
        } else {
            // Try to find which server has this tool
            return Err(anyhow::anyhow!(
                "Tool name must include server prefix: server:tool"
            ));
        };

        let mut clients = self.clients.write().await;
        let client = clients
            .get_mut(server_name)
            .context(format!("MCP server not found: {}", server_name))?;

        client.call_tool(actual_tool_name, arguments).await
    }

    /// Stop all MCP servers
    pub async fn stop_all(&self) -> Result<()> {
        let mut clients = self.clients.write().await;
        for (name, client) in clients.iter_mut() {
            if let Err(e) = client.stop().await {
                tracing::error!("Failed to stop MCP server {}: {}", name, e);
            }
        }
        clients.clear();
        Ok(())
    }
}

impl Default for ExternalMcpManager {
    fn default() -> Self {
        Self::new()
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/http_server.rs">
//! HTTP MCP Server - Exposes MCP functionality via HTTP endpoints
//!
//! This server acts as an HTTP proxy for MCP, allowing remote clients
//! like Antigravity IDE to connect via HTTPS.
//!
//! Authentication:
//! 1. HTTP/SSE requests must provide `Authorization: Bearer <wireguard-session-or-pubkey>`
//! 2. No bypass API keys
//! 3. No Google OAuth validation in the MCP transport layer

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::{get, post},
    Router,
    middleware::from_fn,
    middleware,
};
use op_agents::list_agent_types;
use serde::{Deserialize, Serialize};
use simd_json::json;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command as TokioCommand;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Extract client IP from headers or connection info
fn extract_client_ip(headers: &HeaderMap) -> String {
    // Check X-Forwarded-For (standard proxy header)
    if let Some(forwarded) = headers.get("x-forwarded-for") {
        if let Ok(s) = forwarded.to_str() {
            if let Some(client_ip) = s.split(',').next() {
                return client_ip.trim().to_string();
            }
        }
    }

    // Check X-Real-IP (nginx convention)
    if let Some(real_ip) = headers.get("x-real-ip") {
        if let Ok(s) = real_ip.to_str() {
            return s.trim().to_string();
        }
    }

    // Default - will be overridden by ConnectInfo if available
    "unknown".to_string()
}

/// Check if IP is localhost
fn is_localhost(ip: &str) -> bool {
    ip == "127.0.0.1" || ip == "::1" || ip == "localhost" || ip.starts_with("127.")
}

/// Check if IP is in a trusted mesh/VPN network
fn is_trusted_mesh(ip: &str) -> bool {
    // Netmaker ranges
    if ip.starts_with("10.101.") || ip.starts_with("10.102.") || ip.starts_with("10.103.") {
        return true;
    }

    // Tailscale CGNAT range: 100.64.0.0/10
    if let Some(first) = ip.split('.').next() {
        if first == "100" {
            if let Some(second) = ip.split('.').nth(1) {
                if let Ok(n) = second.parse::<u8>() {
                    if (64..=127).contains(&n) {
                        return true;
                    }
                }
            }
        }
    }

    // ZeroTier
    if ip.starts_with("10.147.") || ip.starts_with("10.244.") {
        return true;
    }

    // WireGuard common ranges
    if ip.starts_with("10.0.0.") || ip.starts_with("10.200.") || ip.starts_with("10.66.66.") {
        return true;
    }

    // Nebula
    if ip.starts_with("10.42.") {
        return true;
    }

    // IPv6 ULA for mesh
    if ip.starts_with("fd") {
        return true;
    }

    false
}

/// Check if IP is in a private network (RFC 1918)
fn is_private_network(ip: &str) -> bool {
    if ip.starts_with("192.168.") || ip.starts_with("10.") {
        return true;
    }

    // 172.16.0.0 - 172.31.255.255
    if let Some(rest) = ip.strip_prefix("172.") {
        if let Some(second_octet) = rest.split('.').next() {
            if let Ok(n) = second_octet.parse::<u8>() {
                if (16..=31).contains(&n) {
                    return true;
                }
            }
        }
    }

    // IPv6 link-local
    if ip.starts_with("fe80") {
        return true;
    }

    false
}

/// Check if IP should bypass authentication (local or trusted)
fn is_trusted_ip(ip: &str) -> bool {
    is_localhost(ip) || is_trusted_mesh(ip) || is_private_network(ip)
}

fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|token| !token.is_empty())
}

fn is_wireguard_pubkey(token: &str) -> bool {
    token.len() == 44
        && token.ends_with('=')
        && token
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '/' | '='))
}

fn is_wireguard_session_id(token: &str) -> bool {
    Uuid::parse_str(token).is_ok()
}

fn is_wireguard_auth_token(token: &str) -> bool {
    is_wireguard_pubkey(token) || is_wireguard_session_id(token)
}

fn is_dev_mode() -> bool {
    matches!(
        std::env::var("OPENCLAW_DEV_MODE").as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    )
}

// Authentication middleware
async fn auth_middleware(
    headers: HeaderMap,
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    if is_dev_mode() {
        debug!("OPENCLAW_DEV_MODE: skipping origin and token checks");
        return Ok(next.run(request).await);
    }

    let client_ip = extract_client_ip(&headers);

    let Some(token) = extract_bearer_token(&headers) else {
        warn!("Rejected MCP HTTP request from {} without bearer token", client_ip);
        return Err(StatusCode::UNAUTHORIZED);
    };

    if !is_wireguard_auth_token(token) {
        warn!(
            "Rejected MCP HTTP request from {} with non-WireGuard bearer token",
            client_ip
        );
        return Err(StatusCode::UNAUTHORIZED);
    }

    debug!("Accepted MCP HTTP request from {} with WireGuard bearer auth", client_ip);
    Ok(next.run(request).await)
}

#[derive(Clone)]
pub struct HttpMcpServer {
    mcp_command: Vec<String>,
    chat_control: Option<ChatControlConfig>,
}

impl HttpMcpServer {
    pub fn new(mcp_command: Vec<String>) -> Self {
        Self {
            mcp_command,
            chat_control: ChatControlConfig::from_env(),
        }
    }

    pub fn router(self) -> Router {
        Router::new()
            .route("/", get(handle_sse).post(handle_mcp_request)) // Root: GET for SSE, POST for MCP
            .route("/health", get(health_check))
            .route("/mcp", post(handle_mcp_request))
            .route("/initialize", post(handle_initialize))
            .route("/tools/list", post(handle_tools_list))
            .route("/tools/call", post(handle_tools_call))
            .route("/sse", get(handle_sse))
            .layer(middleware::from_fn(auth_middleware))
            .with_state(Arc::new(self))
    }
}

#[derive(Deserialize, Serialize)]
struct McpRequest {
    jsonrpc: String,
    id: simd_json::OwnedValue,
    method: String,
    params: Option<simd_json::OwnedValue>,
}

#[derive(Serialize)]
struct McpResponse {
    jsonrpc: String,
    id: simd_json::OwnedValue,
    result: Option<simd_json::OwnedValue>,
    error: Option<simd_json::OwnedValue>,
}

async fn health_check() -> Json<simd_json::OwnedValue> {
    Json(simd_json::json!({
        "status": "ok",
        "service": "mcp-http-proxy",
        "version": "1.0.0"
    }))
}

async fn handle_mcp_request(
    State(server): State<Arc<HttpMcpServer>>,
    Json(request): Json<McpRequest>,
) -> Result<Json<McpResponse>, StatusCode> {
    match server.call_mcp(&request).await {
        Ok(response) => Ok(Json(response)),
        Err(e) => {
            error!("MCP call failed: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn handle_initialize(
    State(server): State<Arc<HttpMcpServer>>,
) -> Result<Json<McpResponse>, StatusCode> {
    let request = McpRequest {
        jsonrpc: "2.0".to_string(),
        id: simd_json::json!(1),
        method: "initialize".to_string(),
        params: Some(simd_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {
                "name": "mcp-http-proxy",
                "version": "1.0.0"
            }
        })),
    };

    match server.call_mcp(&request).await {
        Ok(response) => Ok(Json(response)),
        Err(e) => {
            error!("Initialize failed: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn handle_tools_list(
    State(server): State<Arc<HttpMcpServer>>,
) -> Result<Json<McpResponse>, StatusCode> {
    let request = McpRequest {
        jsonrpc: "2.0".to_string(),
        id: simd_json::json!(2),
        method: "tools/list".to_string(),
        params: None,
    };

    match server.call_mcp(&request).await {
        Ok(response) => Ok(Json(response)),
        Err(e) => {
            error!("Tools list failed: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn handle_tools_call(
    State(server): State<Arc<HttpMcpServer>>,
    Json(params): Json<simd_json::OwnedValue>,
) -> Result<Json<McpResponse>, StatusCode> {
    let request = McpRequest {
        jsonrpc: "2.0".to_string(),
        id: simd_json::json!(3),
        method: "tools/call".to_string(),
        params: Some(params),
    };

    match server.call_mcp(&request).await {
        Ok(response) => Ok(Json(response)),
        Err(e) => {
            error!("Tools call failed: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

use axum::response::sse::{Event, Sse};
use futures::stream::{self, Stream};
use futures::StreamExt;
use std::convert::Infallible;
use std::time::Duration;

async fn handle_sse(
    State(server): State<Arc<HttpMcpServer>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let mut events = Vec::new();
    events.push(server.endpoint_event());

    if let Some(control_event) = server.chat_control_event() {
        events.push(control_event);
    }

    if let Some(tool_event) = server.snapshot_tools_event().await {
        events.push(tool_event);
    }

    if let Some(agent_event) = server.agents_event() {
        events.push(agent_event);
    }

    // Send collected events, then keep connection alive with periodic pings
    let initial_stream = stream::iter(events.into_iter().map(Ok::<_, Infallible>));

    let keep_alive_stream = stream::unfold(0u64, move |counter| async move {
        tokio::time::sleep(Duration::from_secs(30)).await;
        let event = Event::default()
            .event("ping")
            .data(json!({ "counter": counter }).to_string());
        Some((Ok::<_, Infallible>(event), counter + 1))
    });

    let stream = initial_stream.chain(keep_alive_stream);

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(30))
            .text("keepalive"),
    )
}

impl HttpMcpServer {
    async fn call_mcp(
        &self,
        request: &McpRequest,
    ) -> Result<McpResponse, Box<dyn std::error::Error + Send + Sync>> {
        // Serialize request to JSON
        let request_json = simd_json::to_string(request)?;
        info!("MCP Request: {}", request_json);

        // Spawn MCP process with environment variables inherited
        let mut cmd = TokioCommand::new(&self.mcp_command[0]);
        cmd.args(&self.mcp_command[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Inherit environment variables (including MCP_TOOL_OFFSET, MCP_TOOL_LIMIT)
        // This allows chunking to work across instances
        for (key, value) in std::env::vars() {
            cmd.env(key, value);
        }

        let mut child = cmd.spawn()?;

        // Send request to MCP server
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(request_json.as_bytes()).await?;
            stdin.write_all(b"\n").await?;
            stdin.flush().await?;
            drop(stdin); // Close stdin to signal end of input
        }

        // Read response from MCP server
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();

        let mut reader = BufReader::new(stdout).lines();
        let mut error_reader = BufReader::new(stderr).lines();

        // Read stderr for errors
        let error_handle = tokio::spawn(async move {
            let mut errors = Vec::new();
            while let Some(line) = error_reader.next_line().await.unwrap_or(None) {
                warn!("MCP stderr: {}", line);
                errors.push(line);
            }
            errors
        });

        // Read stdout for response
        let mut response_line = None;
        while let Some(line) = reader.next_line().await? {
            if !line.trim().is_empty() {
                response_line = Some(line);
                break;
            }
        }

        // Wait for process to complete
        let status = child.wait().await?;
        let errors = error_handle.await.unwrap_or_default();

        if !status.success() {
            let error_msg = if !errors.is_empty() {
                format!("MCP process failed with status: {}. Errors: {}", status, errors.join(" | "))
            } else {
                format!("MCP process failed with status: {}", status)
            };
            tracing::error!("{}", error_msg);
            return Err(error_msg.into());
        }

        if !errors.is_empty() {
            tracing::warn!("MCP process completed successfully but had stderr output: {}", errors.join(" | "));
        }

        if let Some(response_str) = response_line {
            info!("MCP Response: {}", response_str);

            // Parse and return response
            let parsed: simd_json::OwnedValue = simd_json::from_str(&response_str)?;
            Ok(McpResponse {
                jsonrpc: parsed
                    .get("jsonrpc")
                    .unwrap_or(&simd_json::json!("2.0"))
                    .as_str()
                    .unwrap_or("2.0")
                    .to_string(),
                id: parsed.get("id").unwrap_or(&simd_json::json!(null)).clone(),
                result: parsed.get("result").cloned(),
                error: parsed.get("error").cloned(),
            })
        } else {
            Err("No response from MCP server".into())
        }
    }

    fn endpoint_event(&self) -> Event {
        Event::default().event("endpoint").data("/mcp")
    }

    fn chat_control_event(&self) -> Option<Event> {
        self.chat_control.as_ref().map(|control| control.as_event())
    }

    fn agents_event(&self) -> Option<Event> {
        let agents = list_agent_types();
        if agents.is_empty() {
            return None;
        }

        let payload = json!({
            "name": "op-agents",
            "description": "Agent registry exposed alongside op-mcp",
            "count": agents.len(),
            "agents": agents,
        });

        Some(Event::default().event("agents").data(payload.to_string()))
    }

    async fn snapshot_tools_event(&self) -> Option<Event> {
        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: simd_json::json!("sse-tools"),
            method: "tools/list".to_string(),
            params: None,
        };

        match self.call_mcp(&request).await {
            Ok(response) => {
                if let Some(result) = response.result {
                    let tools = result.get("tools").cloned().unwrap_or_else(|| json!([]));
                    let count = tools.as_array().map(|arr| arr.len()).unwrap_or(0);
                    let payload = json!({
                        "name": "op-mcp",
                        "description": "Aggregated tool snapshot",
                        "count": count,
                        "tools": tools,
                    });
                    Some(Event::default().event("tools").data(payload.to_string()))
                } else {
                    warn!("Snapshot tools response missing result field");
                    None
                }
            }
            Err(e) => {
                warn!("Failed to snapshot tools for SSE: {}", e);
                None
            }
        }
    }
}

#[derive(Clone, Debug)]
struct ChatControlConfig {
    name: String,
    description: String,
    sse_url: String,
    post_url: String,
}

impl ChatControlConfig {
    fn from_env() -> Option<Self> {
        let base = std::env::var("CHAT_CONTROL_MCP_BASE_URL").ok();

        let sse_url = std::env::var("CHAT_CONTROL_MCP_SSE_URL").ok().or_else(|| {
            base.as_ref()
                .map(|b| format!("{}/sse", b.trim_end_matches('/')))
        });

        let post_url = std::env::var("CHAT_CONTROL_MCP_POST_URL").ok().or_else(|| {
            base.as_ref()
                .map(|b| format!("{}/mcp", b.trim_end_matches('/')))
        });

        let sse_url = sse_url?;
        let post_url = post_url.unwrap_or_else(|| "/api/chat/mcp".to_string());
        let name =
            std::env::var("CHAT_CONTROL_MCP_NAME").unwrap_or_else(|_| "chat-control".to_string());
        let description = std::env::var("CHAT_CONTROL_MCP_DESCRIPTION")
            .unwrap_or_else(|_| "Chat Control MCP (op-web) coordinator".to_string());

        Some(Self {
            name,
            description,
            sse_url,
            post_url,
        })
    }

    fn as_event(&self) -> Event {
        let payload = json!({
            "name": &self.name,
            "description": &self.description,
            "sseUrl": &self.sse_url,
            "postUrl": &self.post_url,
        });

        Event::default()
            .event("chat_control")
            .data(payload.to_string())
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/lib.rs">
//! op-mcp: Unified MCP Protocol Server
//!
//! Supports three server modes:
//! - **Compact**: 5 meta-tools with per-request lazy tool loading (recommended for LLMs)
//! - **Agents**: Always-on cognitive agents (memory, sequential_thinking, etc.)
//! - **Full**: All tools directly exposed (may hit client limits)
//!
//! Supports multiple transports:
//! - Stdio (standard MCP transport)
//! - HTTP (REST endpoints)
//! - SSE (Server-Sent Events)
//! - HTTP+SSE (bidirectional)
//! - WebSocket (full duplex)
//! - gRPC (high-performance RPC)

pub mod agents_server;
pub mod compact;
pub mod external_client;
pub mod protocol;
pub mod resources;
pub mod server;
pub mod transport;

pub mod tool_registry;

#[cfg(feature = "grpc")]
pub mod grpc;

// Re-exports
pub use agents_server::AgentsServer;
pub use compact::{run_compact_stdio_server, CompactServer, SessionContext};
pub use external_client::{
    AuthMethod, ExternalMcpClient, ExternalMcpConfig, ExternalMcpManager, ExternalTool,
};
pub use op_core::SecurityLevel;
pub use protocol::{JsonRpcError, McpError, McpRequest, McpResponse};
pub use resources::ResourceRegistry;
pub use server::{DefaultToolExecutor, McpServer, McpServerConfig, ToolExecutor, ToolInfo};
pub use tool_registry::{Tool, ToolRegistry};
pub use transport::{
    HttpSseTransport, HttpTransport, SseTransport, StdioTransport, Transport, WebSocketTransport,
};

#[cfg(feature = "grpc")]
pub use grpc::GrpcTransport;

/// Protocol version
pub const PROTOCOL_VERSION: &str = "2024-11-05";

/// Server info
pub const SERVER_NAME: &str = "op-mcp";
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Server mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerMode {
    /// 5 meta-tools for tool discovery and response
    Compact,
    /// Always-on cognitive agents
    Agents,
    /// All tools directly exposed
    Full,
}

impl std::fmt::Display for ServerMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServerMode::Compact => write!(f, "compact"),
            ServerMode::Agents => write!(f, "agents"),
            ServerMode::Full => write!(f, "full"),
        }
    }
}

impl std::str::FromStr for ServerMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "compact" => Ok(ServerMode::Compact),
            "agents" => Ok(ServerMode::Agents),
            "full" | "standard" => Ok(ServerMode::Full),
            _ => Err(format!("Unknown server mode: {}", s)),
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/lib.rs.grpc-additions">
// Add this to crates/op-mcp/src/lib.rs

// At the top with other module declarations:
#[cfg(feature = "grpc")]
pub mod grpc;

// Re-export gRPC types when feature is enabled
#[cfg(feature = "grpc")]
pub use grpc::{GrpcTransport, GrpcConfig, GrpcClient, GrpcClientConfig};
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/main.rs">
//! op-mcp-server: Unified MCP Protocol Server
//!
//! Supports multiple modes:
//!   - compact: 5 lazy meta-tools for discovering and executing system tools
//!   - agents:  Always-on cognitive agents (memory, sequential_thinking, rust_pro, etc.)
//!   - full:    All tools directly exposed
//!   - grpc:    gRPC transport mode for high-performance internal communication
//!   - grpc-agents: gRPC transport for agents
//!
//! Supports multiple transports:
//!   op-mcp-server                           # stdio, compact mode
//!   op-mcp-server --mode agents             # stdio, agents mode
//!   op-mcp-server --http 0.0.0.0:3001       # HTTP+SSE
//!   op-mcp-server --ws 0.0.0.0:3002         # WebSocket
//!   op-mcp-server --grpc 0.0.0.0:50051      # gRPC transport
//!   op-mcp-server --all                     # All transports

use anyhow::Result;
use clap::Parser;
use op_core::BusType;
use op_identity::{write_sled_from_wg, WireGuardIdentity};
#[cfg(feature = "grpc")]
use op_mcp::grpc::{GrpcConfig, GrpcTransport};
use op_mcp::{
    compact::LazyOpToolsExecutor,
    transport::{HttpSseTransport, StdioTransport, Transport, WebSocketTransport},
    AgentsServer, CompactServer, McpServer, McpServerConfig, ServerMode, ToolExecutor,
};
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Parser)]
#[command(name = "op-mcp-server")]
#[command(about = "Unified MCP Protocol Server")]
struct Cli {
    /// Server mode: compact (5 lazy meta-tools), agents (always-on), full (all tools), grpc, grpc-agents
    #[arg(long, short, default_value = "compact")]
    mode: String,

    /// Run stdio transport (default if no network transport specified)
    #[arg(long)]
    stdio: bool,

    /// Run HTTP+SSE transport on specified address
    #[arg(long, value_name = "ADDR")]
    http: Option<String>,

    /// Run SSE-only transport on specified address
    #[arg(long, value_name = "ADDR")]
    sse: Option<String>,

    /// Run WebSocket transport on specified address
    #[arg(long, value_name = "ADDR")]
    ws: Option<String>,

    /// Run gRPC transport on specified address
    #[arg(long, value_name = "ADDR")]
    grpc: Option<String>,

    /// gRPC port (shorthand, used with --mode grpc or grpc-agents)
    #[arg(long, value_name = "PORT")]
    grpc_port: Option<u16>,

    /// Run all transports with default addresses (binds to WG interface)
    #[arg(long)]
    all: bool,

    /// WireGuard interface to read identity from
    #[arg(long, env = "WG_INTERFACE", default_value = "netmaker")]
    wg_interface: String,

    /// Disable auto-start of run-on-connection agents (agents mode only)
    #[arg(long)]
    no_auto_start: bool,

    /// Log level
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Server name override
    #[arg(long)]
    name: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let level = match cli.log_level.as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(level)
        .with_writer(std::io::stderr)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // ── WireGuard identity ────────────────────────────────────────────────────
    // 1. Detect local WG IP for bind address resolution.
    // 2. Write canonical IdentitySled to /dev/shm for Ghostbridge auth.
    let wg_id = WireGuardIdentity::with_interface(&cli.wg_interface);
    let wg_ip: Option<String> = wg_id.get_local_ip();

    match wg_id.get_local_pubkey() {
        Ok(pubkey) => {
            if let Err(e) = write_sled_from_wg(&pubkey) {
                tracing::warn!(error = %e, "Failed to write WG identity sled to /dev/shm");
            } else {
                info!(
                    interface = %cli.wg_interface,
                    pubkey = %pubkey,
                    wg_ip = ?wg_ip,
                    "WG identity sled written"
                );
            }
        }
        Err(e) => {
            tracing::warn!(
                interface = %cli.wg_interface,
                error = %e,
                "Could not read WG public key — identity sled not written; set WG_PUBKEY env var to override"
            );
        }
    }

    // Check for gRPC modes
    if cli.mode == "grpc" || cli.mode == "grpc-agents" {
        #[cfg(feature = "grpc")]
        {
            let port = cli
                .grpc_port
                .unwrap_or(if cli.mode == "grpc" { 50051 } else { 50052 });
            // Bind to WG interface IP if available, else 0.0.0.0.
            let bind_ip = wg_ip.as_deref().unwrap_or("0.0.0.0");
            let addr: std::net::SocketAddr = format!("{bind_ip}:{}", port).parse()?;
            let server_mode = if cli.mode == "grpc-agents" {
                op_mcp::grpc::GrpcServerMode::Agents
            } else {
                op_mcp::grpc::GrpcServerMode::Compact
            };

            info!(mode = %cli.mode, port = %port, "Starting gRPC MCP server");

            let config = GrpcConfig::default()
                .with_address(addr)
                .with_mode(server_mode);

            let transport = GrpcTransport::new(config).await?;
            return transport.serve().await;
        }

        #[cfg(not(feature = "grpc"))]
        {
            anyhow::bail!("gRPC support not compiled in. Rebuild with --features grpc");
        }
    }

    // Parse server mode for non-gRPC modes
    let mode: ServerMode = cli.mode.parse().map_err(|e: String| anyhow::anyhow!(e))?;

    info!(mode = %mode, "Starting op-mcp-server");

    // Determine transports.
    // When --all is used the default ports bind to the WG interface IP (or
    // 0.0.0.0 if the interface is not up). Explicit --http/--ws/--grpc flags
    // always win regardless of the WG interface state.
    let run_stdio = cli.stdio
        || cli.all
        || (cli.http.is_none() && cli.sse.is_none() && cli.ws.is_none() && cli.grpc.is_none());
    let all_ip = wg_ip.as_deref().unwrap_or("0.0.0.0");
    let http_addr = cli.http.or(cli.sse).or(if cli.all {
        Some(format!("{all_ip}:3001"))
    } else {
        None
    });
    let ws_addr = cli.ws.or(if cli.all {
        Some(format!("{all_ip}:3002"))
    } else {
        None
    });
    let grpc_addr = cli.grpc.or(if cli.all {
        Some(format!("{all_ip}:50051"))
    } else {
        None
    });

    // Create and run server based on mode
    match mode {
        ServerMode::Compact => {
            let executor: Arc<dyn ToolExecutor> = Arc::new(LazyOpToolsExecutor);
            let server = Arc::new(CompactServer::new(executor));
            info!("Compact MCP server initialized with lazy op-tools registry");

            run_transports(
                server,
                run_stdio,
                http_addr,
                ws_addr,
                grpc_addr,
                Some("/mcp/compact"),
            )
            .await
        }

        ServerMode::Full => {
            let config = McpServerConfig {
                name: cli.name,
                compact_mode: false,
                ..Default::default()
            };

            let server = McpServer::new(config).await?;
            info!(mode = %mode, "MCP server initialized");

            run_transports(server, run_stdio, http_addr, ws_addr, grpc_addr, None).await
        }

        ServerMode::Agents => {
            let bus_type = if std::env::var("DBUS_AGENT_SESSION").is_ok() {
                BusType::Session
            } else {
                BusType::System
            };

            if cli.no_auto_start {
                info!("--no-auto-start is ignored for D-Bus agents mode");
            }

            let server = Arc::new(AgentsServer::new(bus_type));
            server.initialize().await?;

            let agents = server.list_agents().await;
            let agent_ids: Vec<_> = agents.iter().map(|agent| agent.id.as_str()).collect();
            info!(
                bus = %bus_type,
                agents = ?agent_ids,
                total = agents.len(),
                "Agents MCP server initialized"
            );

            run_transports(server, run_stdio, http_addr, ws_addr, grpc_addr, None).await
        }
    }
}

async fn run_transports<H>(
    server: Arc<H>,
    run_stdio: bool,
    http_addr: Option<String>,
    ws_addr: Option<String>,
    _grpc_addr: Option<String>,
    base_path: Option<&'static str>,
) -> Result<()>
where
    H: op_mcp::transport::McpHandler + 'static,
{
    let mut handles = Vec::new();

    // Spawn HTTP+SSE transport
    if let Some(addr) = http_addr {
        let server = server.clone();
        handles.push(tokio::spawn(async move {
            info!(addr = %addr, "Starting HTTP+SSE transport");
            let mut transport = HttpSseTransport::new(addr);
            if let Some(base_path) = base_path {
                transport = transport.with_base_path(base_path);
            }
            transport.serve(server).await
        }));
    }

    // Spawn WebSocket transport
    if let Some(addr) = ws_addr {
        let server = server.clone();
        handles.push(tokio::spawn(async move {
            info!(addr = %addr, "Starting WebSocket transport");
            WebSocketTransport::new(addr).serve(server).await
        }));
    }

    // gRPC transport would be spawned here if needed with the generic handler
    // For now, gRPC is handled separately with --mode grpc

    // Run stdio in main thread if enabled
    if run_stdio {
        info!("Starting stdio transport");
        StdioTransport::new().serve(server).await?;
    } else {
        for handle in handles {
            handle.await??;
        }
    }

    Ok(())
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/mod.rs">
//! op-mcp: Model Context Protocol implementations
//!
//! This crate provides MCP servers and tools for AI agent integration.

pub mod agents_server;
pub mod builtin_trait_agents;
pub mod compact_server;
pub mod critical;
pub mod stdio_server;
pub mod tool_adapter;

pub use agents_server::{AgentsServer, AgentsServerConfig, AgentDefinition, ExecutorType};
pub use builtin_trait_agents::register_builtin_agents;
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/mod.rs.patch">
// Add to mod.rs:
pub mod trait_agent_executor;
pub use trait_agent_executor::TraitAgentExecutor;
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/protocol.rs">
//! MCP Protocol Types
//!
//! JSON-RPC 2.0 protocol types for Model Context Protocol.

use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;

/// JSON-RPC 2.0 Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

impl McpRequest {
    pub fn new(method: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: method.into(),
            params: None,
            meta: None,
        }
    }

    pub fn with_id(mut self, id: impl Into<Value>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn with_params(mut self, params: Value) -> Self {
        self.params = Some(params);
        self
    }

    pub fn with_meta(mut self, meta: Value) -> Self {
        self.meta = Some(meta);
        self
    }
}

/// JSON-RPC 2.0 Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    #[serde(rename = "_meta", default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

impl McpResponse {
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
            meta: None,
        }
    }

    pub fn error(id: Option<Value>, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
            meta: None,
        }
    }

    pub fn with_meta(mut self, meta: Value) -> Self {
        self.meta = Some(meta);
        self
    }

    pub fn is_success(&self) -> bool {
        self.error.is_none()
    }
}

/// JSON-RPC 2.0 Error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }

    // Standard JSON-RPC error codes
    pub fn parse_error(msg: impl Into<String>) -> Self {
        Self::new(-32700, msg)
    }

    pub fn invalid_request(msg: impl Into<String>) -> Self {
        Self::new(-32600, msg)
    }

    pub fn method_not_found(method: &str) -> Self {
        Self::new(-32601, format!("Method not found: {}", method))
    }

    pub fn invalid_params(msg: impl Into<String>) -> Self {
        Self::new(-32602, msg)
    }

    pub fn internal_error(msg: impl Into<String>) -> Self {
        Self::new(-32603, msg)
    }
}

/// Type alias for backward compatibility
pub type McpError = JsonRpcError;

#[cfg(test)]
mod tests {
    use super::*;
    use simd_json::json;
    use simd_json::prelude::{ValueAsScalar, ValueObjectAccess};

    #[test]
    fn test_request_serialization() {
        let req = McpRequest::new("tools/list")
            .with_id(json!(1))
            .with_params(json!({"limit": 10}));

        let json_str = simd_json::to_string(&req).unwrap();
        assert!(json_str.contains("tools/list"));
    }

    #[test]
    fn test_response_success() {
        let resp = McpResponse::success(Some(json!(1)), json!({"tools": []}));
        assert!(resp.is_success());
    }

    #[test]
    fn test_response_error() {
        let resp = McpResponse::error(Some(json!(1)), JsonRpcError::method_not_found("unknown"));
        assert!(!resp.is_success());
    }

    #[test]
    fn test_request_meta_round_trip() {
        let req = McpRequest::new("initialize")
            .with_id(json!("abc"))
            .with_meta(json!({"traceId": "trace-123"}));

        let json_str = simd_json::to_string(&req).unwrap();
        assert!(json_str.contains("\"_meta\""));

        let mut json_buf = json_str.clone();
        let parsed: McpRequest = unsafe { simd_json::from_str(&mut json_buf) }.unwrap();
        assert_eq!(
            parsed
                .meta
                .as_ref()
                .and_then(|meta| meta.get("traceId"))
                .and_then(|v| v.as_str()),
            Some("trace-123")
        );
    }

    #[test]
    fn test_response_meta_round_trip() {
        let resp = McpResponse::success(Some(json!(7)), json!({"ok": true}))
            .with_meta(json!({"progressToken": "tok-1"}));

        let json_str = simd_json::to_string(&resp).unwrap();
        assert!(json_str.contains("\"_meta\""));

        let mut json_buf = json_str.clone();
        let parsed: McpResponse = unsafe { simd_json::from_str(&mut json_buf) }.unwrap();
        assert_eq!(
            parsed
                .meta
                .as_ref()
                .and_then(|meta| meta.get("progressToken"))
                .and_then(|v| v.as_str()),
            Some("tok-1")
        );
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/request_context.rs">
//! Request Context - Per-Request Tool Loading
//!
//! Tools are loaded when a request starts and unloaded when it completes.
//! This ensures:
//! - All tools available during request (no eviction)
//! - Memory freed between requests
//! - Clean isolation per request
//! - max_turns enforced per request (not session)
//! - **Security blocklist enforced at the single choke point** (audit item #7)

use anyhow::Result;
use simd_json::OwnedValue;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::compact::ToolDefinition;
use crate::tool_registry::{BoxedTool, Tool};

// =============================================================================
// SECURITY BLOCKLIST (audit item #7)
//
// `meta_execute_tool` in `request_handler.rs` previously routed user-controlled
// `tool_name` straight into `ctx.execute_tool`, turning the compact-mode API
// (advertised as 5 meta-tools) into an unauthenticated control plane for the
// full ~30-tool backing registry, including `shell_execute`, `write_file`,
// every `systemd_*` mutation, and every OVS mutation.
//
// The fix is enforced HERE rather than in the handler so that *both* the
// verbose `tools/call` path and the compact `execute_tool` path are gated by
// the same check. Adding new entry points in the future automatically
// inherits the protection.
//
// A controller session (gateway-authenticated, `is_controller == true`) may
// invoke blocked tools \u2014 it represents the operator. Anonymous / regular
// sessions cannot. Response tools are always allowed because they have no
// system effect and the LLM needs them to terminate a turn.
// =============================================================================

/// Tool-name substring patterns that require controller privileges to execute.
const BLOCKED_PATTERNS: &[&str] = &[
    // Shell / arbitrary write
    "shell_execute",
    "write_file",
    // Systemd mutations
    "systemd_start",
    "systemd_stop",
    "systemd_restart",
    "systemd_reload",
    "systemd_enable",
    "systemd_disable",
    "systemd_apply",
    // OVS mutations
    "ovs_create",
    "ovs_delete",
    "ovs_add",
    "ovs_set",
    "ovs_del",
    // Plugin mutations (matches any *_apply pattern)
    "_apply",
    // Btrfs mutations
    "btrfs_create",
    "btrfs_delete",
    "btrfs_snapshot",
];

/// Tool names that are always permitted, regardless of session privilege.
/// These tools have no system side effects and are required for the LLM to
/// communicate with the user at the end of a turn.
const ALWAYS_ALLOWED: &[&str] = &[
    "respond_to_user",
    "cannot_perform",
    "request_clarification",
];

fn is_response_tool(name: &str) -> bool {
    ALWAYS_ALLOWED.contains(&name)
}

fn is_tool_blocked(name: &str) -> bool {
    if is_response_tool(name) {
        return false;
    }
    BLOCKED_PATTERNS.iter().any(|pat| name.contains(pat))
}

/// Configuration for request handling
#[derive(Debug, Clone)]
pub struct RequestConfig {
    /// Maximum tool calls per REQUEST (not session)
    pub max_turns: u32,
    /// Request timeout in seconds
    pub timeout_secs: u64,
    /// Whether to preload all tools at request start
    pub preload_all: bool,
}

impl Default for RequestConfig {
    fn default() -> Self {
        Self {
            max_turns: 75,
            timeout_secs: 300, // 5 minutes per request
            preload_all: true,
        }
    }
}

/// Per-request context that holds loaded tools
/// 
/// Created at request start, dropped at request end.
/// All tools are loaded into this context and remain available
/// for the entire duration of the request.
pub struct RequestContext {
    /// Request ID for tracking
    pub request_id: String,
    /// Session ID for auth continuity
    pub session_id: Option<String>,
    /// Is this a controller (you/chatbot) with full access
    pub is_controller: bool,
    /// WireGuard peer public key (if authenticated)
    pub peer_pubkey: Option<String>,
    /// When request started
    pub started_at: Instant,
    /// Configuration
    pub config: RequestConfig,
    /// Loaded tools (owned for this request)
    tools: HashMap<String, BoxedTool>,
    /// Tool definitions (for list/search)
    definitions: HashMap<String, ToolDefinition>,
    /// Turn counter for this request
    turn_count: AtomicU32,
    /// Request-scoped variables
    variables: RwLock<HashMap<String, Value>>,
}

impl RequestContext {
    /// Create a new request context
    pub fn new(request_id: String, config: RequestConfig) -> Self {
        info!(request_id = %request_id, "Creating request context");
        Self {
            request_id,
            session_id: None,
            is_controller: false,
            peer_pubkey: None,
            started_at: Instant::now(),
            config,
            tools: HashMap::new(),
            definitions: HashMap::new(),
            turn_count: AtomicU32::new(0),
            variables: RwLock::new(HashMap::new()),
        }
    }

    /// Create with session info (from gateway auth)
    pub fn with_session(
        request_id: String,
        config: RequestConfig,
        session_id: String,
        is_controller: bool,
        peer_pubkey: Option<String>,
    ) -> Self {
        info!(
            request_id = %request_id,
            session_id = %session_id,
            is_controller = %is_controller,
            "Creating authenticated request context"
        );
        Self {
            request_id,
            session_id: Some(session_id),
            is_controller,
            peer_pubkey,
            started_at: Instant::now(),
            config,
            tools: HashMap::new(),
            definitions: HashMap::new(),
            turn_count: AtomicU32::new(0),
            variables: RwLock::new(HashMap::new()),
        }
    }

    /// Check if caller can access controller-only tools
    pub fn can_access_controller_tools(&self) -> bool {
        self.is_controller
    }

    /// Check if caller has any valid session
    pub fn is_authenticated(&self) -> bool {
        self.session_id.is_some()
    }

    /// Load a tool into this request context
    pub fn load_tool(&mut self, tool: BoxedTool) {
        let name = tool.name().to_string();
        let definition = ToolDefinition {
            name: name.clone(),
            description: tool.description().to_string(),
            input_schema: tool.input_schema(),
            category: tool.category().to_string(),
            tags: tool.tags(),
        };
        
        self.tools.insert(name.clone(), tool);
        self.definitions.insert(name.clone(), definition);
        debug!("Loaded tool into request context: {}", name);
    }

    /// Load all tools from a factory function
    pub async fn load_all_tools<F, Fut>(&mut self, factory: F) -> Result<usize>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<Vec<BoxedTool>>>,
    {
        let tools = factory().await?;
        let count = tools.len();
        
        for tool in tools {
            self.load_tool(tool);
        }
        
        info!(
            request_id = %self.request_id,
            tool_count = count,
            "Loaded all tools for request"
        );
        
        Ok(count)
    }

    /// Get current turn count
    pub fn turn_count(&self) -> u32 {
        self.turn_count.load(Ordering::Relaxed)
    }

    /// Increment turn count and check limit
    /// Returns Err if max_turns exceeded
    pub fn increment_turn(&self) -> Result<u32, TurnLimitError> {
        let current = self.turn_count.fetch_add(1, Ordering::Relaxed) + 1;
        
        if current > self.config.max_turns {
            warn!(
                request_id = %self.request_id,
                current = current,
                max = self.config.max_turns,
                "Turn limit exceeded"
            );
            return Err(TurnLimitError {
                current,
                max: self.config.max_turns,
            });
        }
        
        debug!(
            request_id = %self.request_id,
            turn = current,
            remaining = self.config.max_turns - current,
            "Turn {} of {}",
            current,
            self.config.max_turns
        );
        
        Ok(current)
    }

    /// Check if request has timed out
    pub fn is_timed_out(&self) -> bool {
        self.started_at.elapsed().as_secs() > self.config.timeout_secs
    }

    /// Get remaining turns
    pub fn remaining_turns(&self) -> u32 {
        self.config.max_turns.saturating_sub(self.turn_count())
    }

    /// Get a tool by name
    pub fn get_tool(&self, name: &str) -> Option<&BoxedTool> {
        self.tools.get(name)
    }

    /// Get tool definition
    pub fn get_definition(&self, name: &str) -> Option<&ToolDefinition> {
        self.definitions.get(name)
    }

    /// Execute a tool.
    ///
    /// This is the **single choke point** for all tool execution in compact
    /// mode. Both the verbose `tools/call` path and the compact-mode
    /// `execute_tool` meta-tool route through here, so the security check
    /// below covers both.
    pub async fn execute_tool(&self, name: &str, input: Value) -> Result<Value> {
        // -----------------------------------------------------------------
        // SECURITY GATE (audit item #7)
        // -----------------------------------------------------------------
        // Reject blocked tools unless the session is an authenticated
        // controller (i.e. the operator's session, validated by the gateway).
        if is_tool_blocked(name) {
            if !self.is_controller {
                warn!(
                    request_id = %self.request_id,
                    session_id = ?self.session_id,
                    tool = %name,
                    "Rejected blocked tool: non-controller session"
                );
                anyhow::bail!(
                    "Tool '{}' is restricted to controller sessions and cannot be invoked from compact mode",
                    name
                );
            }
            // Controller is allowed, but we still log every privileged
            // invocation so the audit trail records it.
            info!(
                request_id = %self.request_id,
                session_id = ?self.session_id,
                tool = %name,
                "Controller session invoking privileged tool"
            );
        }

        // Check turn limit
        self.increment_turn()?;
        
        // Check timeout
        if self.is_timed_out() {
            anyhow::bail!("Request timed out after {} seconds", self.config.timeout_secs);
        }
        
        // Get and execute tool
        let tool = self.tools.get(name)
            .ok_or_else(|| anyhow::anyhow!("Tool not found: {}", name))?;
        
        tool.execute(input).await
    }

    /// List all tools (paginated)
    pub fn list_tools(&self, offset: usize, limit: usize, category: Option<&str>) -> Vec<&ToolDefinition> {
        self.definitions.values()
            .filter(|d| category.map_or(true, |c| d.category == c))
            .skip(offset)
            .take(limit)
            .collect()
    }

    /// Search tools
    pub fn search_tools(&self, query: &str) -> Vec<&ToolDefinition> {
        let query_lower = query.to_lowercase();
        
        self.definitions.values()
            .filter(|d| {
                d.name.to_lowercase().contains(&query_lower) ||
                d.description.to_lowercase().contains(&query_lower) ||
                d.category.to_lowercase().contains(&query_lower)
            })
            .take(50)
            .collect()
    }

    /// Total tool count
    pub fn tool_count(&self) -> usize {
        self.tools.len()
    }

    /// Set a request-scoped variable
    pub async fn set_variable(&self, key: &str, value: Value) {
        self.variables.write().await.insert(key.to_string(), value);
    }

    /// Get a request-scoped variable
    pub async fn get_variable(&self, key: &str) -> Option<Value> {
        self.variables.read().await.get(key).cloned()
    }

    /// Get elapsed time
    pub fn elapsed_secs(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }

    /// Get summary for logging
    pub fn summary(&self) -> RequestSummary {
        RequestSummary {
            request_id: self.request_id.clone(),
            tools_loaded: self.tools.len(),
            turns_used: self.turn_count(),
            max_turns: self.config.max_turns,
            elapsed_secs: self.elapsed_secs(),
        }
    }
}

impl Drop for RequestContext {
    fn drop(&mut self) {
        info!(
            request_id = %self.request_id,
            tools_loaded = self.tools.len(),
            turns_used = self.turn_count(),
            elapsed_secs = self.elapsed_secs(),
            "Request context dropped, unloading {} tools",
            self.tools.len()
        );
        // Tools are automatically dropped here, freeing memory
    }
}

/// Error when turn limit is exceeded
#[derive(Debug, Clone)]
pub struct TurnLimitError {
    pub current: u32,
    pub max: u32,
}

impl std::fmt::Display for TurnLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Turn limit exceeded: {} of {} maximum tool calls used",
            self.current, self.max
        )
    }
}

impl std::error::Error for TurnLimitError {}

impl From<TurnLimitError> for anyhow::Error {
    fn from(e: TurnLimitError) -> Self {
        anyhow::anyhow!(e.to_string())
    }
}

/// Request summary for logging/metrics
#[derive(Debug, Clone)]
pub struct RequestSummary {
    pub request_id: String,
    pub tools_loaded: usize,
    pub turns_used: u32,
    pub max_turns: u32,
    pub elapsed_secs: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use simd_json::json;

    // --- Test helpers -----------------------------------------------------

    struct DummyTool {
        name: &'static str,
    }

    #[async_trait]
    impl Tool for DummyTool {
        fn name(&self) -> &str {
            self.name
        }
        fn description(&self) -> &str {
            "dummy"
        }
        fn input_schema(&self) -> Value {
            json!({})
        }
        async fn execute(&self, _input: Value) -> Result<Value> {
            Ok(json!({ "ok": true }))
        }
    }

    fn ctx(is_controller: bool) -> RequestContext {
        let mut ctx = RequestContext::with_session(
            "req-1".to_string(),
            RequestConfig::default(),
            "sess-1".to_string(),
            is_controller,
            None,
        );
        ctx.load_tool(Box::new(DummyTool { name: "shell_execute" }));
        ctx.load_tool(Box::new(DummyTool { name: "systemd_start_unit" }));
        ctx.load_tool(Box::new(DummyTool { name: "ovs_list_bridges" }));
        ctx.load_tool(Box::new(DummyTool { name: "respond_to_user" }));
        ctx
    }

    // --- Tests -------------------------------------------------------------

    #[test]
    fn test_turn_limit() {
        let config = RequestConfig {
            max_turns: 3,
            ..Default::default()
        };
        let ctx = RequestContext::new("test".to_string(), config);
        
        assert!(ctx.increment_turn().is_ok()); // 1
        assert!(ctx.increment_turn().is_ok()); // 2
        assert!(ctx.increment_turn().is_ok()); // 3
        assert!(ctx.increment_turn().is_err()); // 4 - exceeds limit
    }

    #[test]
    fn test_remaining_turns() {
        let config = RequestConfig {
            max_turns: 10,
            ..Default::default()
        };
        let ctx = RequestContext::new("test".to_string(), config);
        
        assert_eq!(ctx.remaining_turns(), 10);
        ctx.increment_turn().unwrap();
        assert_eq!(ctx.remaining_turns(), 9);
    }

    #[test]
    fn blocklist_classification_matches_audit_intent() {
        // Blocked:
        assert!(is_tool_blocked("shell_execute"));
        assert!(is_tool_blocked("write_file"));
        assert!(is_tool_blocked("systemd_start_unit"));
        assert!(is_tool_blocked("systemd_restart_unit"));
        assert!(is_tool_blocked("ovs_create_bridge"));
        assert!(is_tool_blocked("ovs_del_port"));
        assert!(is_tool_blocked("plugin_systemd_apply"));
        assert!(is_tool_blocked("btrfs_snapshot"));

        // Allowed:
        assert!(!is_tool_blocked("systemd_list_units"));
        assert!(!is_tool_blocked("systemd_unit_status"));
        assert!(!is_tool_blocked("ovs_list_bridges"));
        assert!(!is_tool_blocked("ovs_list_ports"));
        assert!(!is_tool_blocked("ovs_dump_flows"));
        assert!(!is_tool_blocked("read_file"));
        assert!(!is_tool_blocked("plugin_systemd_query"));

        // Response tools always allowed even though name structure could otherwise trip a pattern:
        assert!(!is_tool_blocked("respond_to_user"));
        assert!(!is_tool_blocked("cannot_perform"));
        assert!(!is_tool_blocked("request_clarification"));
    }

    #[tokio::test]
    async fn blocks_shell_execute_for_non_controller() {
        let c = ctx(/* is_controller */ false);
        let err = c.execute_tool("shell_execute", json!({}))
            .await
            .expect_err("non-controller must be blocked");
        let msg = err.to_string();
        assert!(msg.contains("restricted to controller sessions"), "got: {}", msg);
    }

    #[tokio::test]
    async fn blocks_systemd_mutation_for_non_controller() {
        let c = ctx(false);
        let err = c.execute_tool("systemd_start_unit", json!({}))
            .await
            .expect_err("non-controller must be blocked");
        assert!(err.to_string().contains("restricted to controller sessions"));
    }

    #[tokio::test]
    async fn allows_shell_execute_for_controller() {
        let c = ctx(true);
        let res = c.execute_tool("shell_execute", json!({})).await;
        assert!(res.is_ok(), "controller must be allowed, got: {:?}", res);
    }

    #[tokio::test]
    async fn allows_read_only_tool_for_non_controller() {
        let c = ctx(false);
        let res = c.execute_tool("ovs_list_bridges", json!({})).await;
        assert!(res.is_ok(), "read-only must always be allowed, got: {:?}", res);
    }

    #[tokio::test]
    async fn response_tools_always_allowed() {
        let c = ctx(false);
        let res = c.execute_tool("respond_to_user", json!({})).await;
        assert!(res.is_ok(), "respond_to_user must always be allowed, got: {:?}", res);
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/request_handler.rs">
//! Request Handler - Processes MCP requests with per-request tool loading
//!
//! Each request gets its own RequestContext with all tools loaded.
//! Tools are unloaded when the request completes.
//!
//! Security (audit item #7): `meta_execute_tool` is the compact-mode entry
//! point for arbitrary tool invocation. The blocklist itself is enforced in
//! `RequestContext::execute_tool` (single choke point covering both this path
//! and the verbose `tools/call` path). The hardening here is purely
//! input-shape validation plus a guard against meta-tool reflection.

use anyhow::Result;
use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::compact::{ToolDefinition, CompactServerConfig};
use crate::protocol::{McpRequest, McpResponse, JsonRpcError};
use crate::request_context::{RequestContext, RequestConfig};
use crate::tool_registry::Tool;
use crate::tools;
use crate::{PROTOCOL_VERSION, SERVER_NAME, SERVER_VERSION};

struct OpToolsAdapter {
    inner: op_tools::BoxedTool,
}

#[async_trait]
impl Tool for OpToolsAdapter {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn input_schema(&self) -> Value {
        self.inner.input_schema()
    }

    fn category(&self) -> &str {
        self.inner.category()
    }

    fn namespace(&self) -> &str {
        self.inner.namespace()
    }

    fn tags(&self) -> Vec<String> {
        self.inner.tags()
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        self.inner.execute(input).await
    }
}

/// Meta-tool names. `meta_execute_tool` must reject these as targets to
/// prevent trivial recursion / reflection from the compact path.
const META_TOOL_NAMES: &[&str] = &[
    "execute_tool",
    "list_tools",
    "search_tools",
    "get_tool_schema",
    "respond",
];

/// Request handler that creates per-request contexts
pub struct RequestHandler {
    config: CompactServerConfig,
}

impl RequestHandler {
    pub fn new(config: CompactServerConfig) -> Self {
        Self { config }
    }

    /// Handle an MCP request
    /// 
    /// This creates a RequestContext, loads all tools, processes the request,
    /// then drops the context (unloading tools).
    pub async fn handle(&self, request: McpRequest) -> McpResponse {
        let request_id = uuid::Uuid::new_v4().to_string();
        
        info!(
            request_id = %request_id,
            method = %request.method,
            "Handling MCP request"
        );

        match request.method.as_str() {
            "initialize" => self.handle_initialize(&request),
            "initialized" => McpResponse::success(request.id, json!({})),
            "tools/list" => self.handle_tools_list(&request, &request_id).await,
            "tools/call" => self.handle_tools_call(&request, &request_id).await,
            "ping" => McpResponse::success(request.id, json!({})),
            _ => McpResponse::error(
                request.id,
                JsonRpcError::method_not_found(&request.method),
            ),
        }
    }

    /// Handle initialize - no tools loaded yet
    fn handle_initialize(&self, request: &McpRequest) -> McpResponse {
        let server_name = self.config.name.as_deref().unwrap_or(SERVER_NAME);
        
        McpResponse::success(
            request.id.clone(),
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "serverInfo": {
                    "name": server_name,
                    "version": SERVER_VERSION
                },
                "capabilities": {
                    "tools": {
                        "listChanged": false
                    }
                },
                "_meta": {
                    "mode": "compact",
                    "max_turns_per_request": self.config.max_turns,
                    "description": "Compact mode: 5 meta-tools, per-request tool loading"
                }
            }),
        )
    }

    /// Handle tools/list - load tools, return meta-tools, unload
    async fn handle_tools_list(&self, request: &McpRequest, request_id: &str) -> McpResponse {
        // Create context and load tools
        let mut ctx = self.create_context(request_id);
        
        if let Err(e) = self.load_tools(&mut ctx).await {
            error!("Failed to load tools: {}", e);
            return McpResponse::error(
                request.id.clone(),
                JsonRpcError::new(-32000, format!("Failed to load tools: {}", e), None),
            );
        }

        // Return meta-tools (compact mode)
        let meta_tools = self.meta_tool_definitions();
        let underlying_count = ctx.tool_count();
        
        // Context is dropped here, unloading tools
        McpResponse::success(
            request.id.clone(),
            json!({
                "tools": meta_tools,
                "_meta": {
                    "mode": "compact",
                    "meta_tools": meta_tools.len(),
                    "underlying_tools": underlying_count,
                    "max_turns_per_request": self.config.max_turns
                }
            }),
        )
    }

    /// Handle tools/call - load tools, execute, unload
    async fn handle_tools_call(&self, request: &McpRequest, request_id: &str) -> McpResponse {
        // Create context and load tools
        let mut ctx = self.create_context(request_id);
        
        if let Err(e) = self.load_tools(&mut ctx).await {
            error!("Failed to load tools: {}", e);
            return McpResponse::error(
                request.id.clone(),
                JsonRpcError::new(-32000, format!("Failed to load tools: {}", e), None),
            );
        }

        let params = request.params.as_ref();
        
        let tool_name = params
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("");
        
        let arguments = params
            .and_then(|p| p.get("arguments"))
            .cloned()
            .unwrap_or(json!({}));

        info!(
            request_id = %request_id,
            tool = %tool_name,
            turn = ctx.turn_count() + 1,
            max_turns = self.config.max_turns,
            "Executing tool"
        );

        // Execute based on meta-tool name
        let result = match tool_name {
            "execute_tool" => self.meta_execute_tool(&ctx, arguments).await,
            "list_tools" => self.meta_list_tools(&ctx, arguments),
            "search_tools" => self.meta_search_tools(&ctx, arguments),
            "get_tool_schema" => self.meta_get_tool_schema(&ctx, arguments),
            "respond" => self.meta_respond(arguments),
            _ => Err(anyhow::anyhow!("Unknown meta-tool: {}", tool_name)),
        };

        let summary = ctx.summary();
        
        // Context is dropped here, unloading tools
        match result {
            Ok(value) => McpResponse::success(
                request.id.clone(),
                json!({
                    "content": [{
                        "type": "text",
                        "text": simd_json::to_string_pretty(&value).unwrap_or_default()
                    }],
                    "_meta": {
                        "request_id": summary.request_id,
                        "turn": summary.turns_used,
                        "max_turns": summary.max_turns,
                        "remaining": summary.max_turns - summary.turns_used,
                        "elapsed_secs": summary.elapsed_secs
                    }
                }),
            ),
            Err(e) => McpResponse::error(
                request.id.clone(),
                JsonRpcError::new(-32000, e.to_string(), None),
            ),
        }
    }

    /// Create a new request context
    fn create_context(&self, request_id: &str) -> RequestContext {
        let config = RequestConfig {
            max_turns: self.config.max_turns as u32,
            timeout_secs: 300,
            preload_all: true,
        };
        RequestContext::new(request_id.to_string(), config)
    }

    /// Load all tools into context
    async fn load_tools(&self, ctx: &mut RequestContext) -> Result<()> {
        // Load the authoritative op-tools registry per request. The chatbot
        // only sees five meta-tools, while execute_tool can reach every live
        // system, D-Bus, OVS, and PluginSchema projection tool.
        let registry = op_tools::ToolRegistry::new();
        op_tools::register_builtin_tools(&registry).await?;
        for definition in registry.list().await {
            if let Some(tool) = registry.get(&definition.name).await {
                ctx.load_tool(Arc::new(OpToolsAdapter { inner: tool }));
            }
        }

        // Response tools
        ctx.load_tool(Arc::new(tools::response::RespondToUserTool));
        ctx.load_tool(Arc::new(tools::response::CannotPerformTool));
        ctx.load_tool(Arc::new(tools::response::RequestClarificationTool));
        
        // Filesystem tools
        ctx.load_tool(Arc::new(tools::filesystem::ReadFileTool));
        ctx.load_tool(Arc::new(tools::filesystem::WriteFileTool));
        ctx.load_tool(Arc::new(tools::filesystem::ListDirectoryTool));
        
        // Shell tools
        ctx.load_tool(Arc::new(tools::shell::ShellExecuteTool::new()));
        
        // System tools
        ctx.load_tool(Arc::new(tools::system::ListNetworkInterfacesTool));
        
        // Systemd tools
        ctx.load_tool(Arc::new(tools::systemd::SystemdUnitStatusTool));
        ctx.load_tool(Arc::new(tools::systemd::SystemdListUnitsTool));
        ctx.load_tool(Arc::new(tools::systemd::SystemdStartUnitTool));
        ctx.load_tool(Arc::new(tools::systemd::SystemdStopUnitTool));
        ctx.load_tool(Arc::new(tools::systemd::SystemdRestartUnitTool));
        ctx.load_tool(Arc::new(tools::systemd::SystemdEnableUnitTool));
        ctx.load_tool(Arc::new(tools::systemd::SystemdDisableUnitTool));
        ctx.load_tool(Arc::new(tools::systemd::SystemdReloadDaemonTool));
        
        // OVS tools
        ctx.load_tool(Arc::new(tools::ovs::OvsListBridgesTool));
        ctx.load_tool(Arc::new(tools::ovs::OvsShowBridgeTool));
        ctx.load_tool(Arc::new(tools::ovs::OvsListPortsTool));
        ctx.load_tool(Arc::new(tools::ovs::OvsDumpFlowsTool));
        ctx.load_tool(Arc::new(tools::ovs::OvsAddBridgeTool));
        ctx.load_tool(Arc::new(tools::ovs::OvsDelBridgeTool));
        ctx.load_tool(Arc::new(tools::ovs::OvsAddPortTool));
        ctx.load_tool(Arc::new(tools::ovs::OvsDelPortTool));
        ctx.load_tool(Arc::new(tools::ovs::OvsAddFlowTool));
        ctx.load_tool(Arc::new(tools::ovs::OvsDelFlowsTool));
        
        // Plugin state tools (9 plugins \u00d7 3 ops = 27 tools)
        for plugin in &["systemd", "network", "packagekit", "firewall", "users", "storage", "lxc", "openflow", "privacy"] {
            ctx.load_tool(Arc::new(tools::plugin::PluginQueryTool::new(plugin)));
            ctx.load_tool(Arc::new(tools::plugin::PluginDiffTool::new(plugin)));
            ctx.load_tool(Arc::new(tools::plugin::PluginApplyTool::new(plugin)));
        }
        
        info!(
            request_id = %ctx.request_id,
            count = ctx.tool_count(),
            "Loaded all tools for request"
        );
        
        Ok(())
    }

    /// Meta-tool definitions (the 5 tools LLM sees)
    fn meta_tool_definitions(&self) -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                name: "execute_tool".to_string(),
                description: "Execute any available tool by name. Use list_tools or search_tools to discover tools first.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "tool_name": {"type": "string", "description": "Name of the tool to execute"},
                        "arguments": {"type": "object", "description": "Arguments to pass to the tool"}
                    },
                    "required": ["tool_name"]
                }),
                category: "meta".to_string(),
                tags: vec!["meta".to_string()],
            },
            ToolDefinition {
                name: "list_tools".to_string(),
                description: "List available tools, optionally by category. Categories: response, filesystem, shell, system, systemd, ovs, network, plugin.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "category": {"type": "string"},
                        "offset": {"type": "integer", "default": 0},
                        "limit": {"type": "integer", "default": 50}
                    }
                }),
                category: "meta".to_string(),
                tags: vec!["meta".to_string()],
            },
            ToolDefinition {
                name: "search_tools".to_string(),
                description: "Search for tools by keyword.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {"type": "string"}
                    },
                    "required": ["query"]
                }),
                category: "meta".to_string(),
                tags: vec!["meta".to_string()],
            },
            ToolDefinition {
                name: "get_tool_schema".to_string(),
                description: "Get the input schema for a specific tool.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "tool_name": {"type": "string"}
                    },
                    "required": ["tool_name"]
                }),
                category: "meta".to_string(),
                tags: vec!["meta".to_string()],
            },
            ToolDefinition {
                name: "respond".to_string(),
                description: "Send a response to the user.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "message": {"type": "string"}
                    },
                    "required": ["message"]
                }),
                category: "meta".to_string(),
                tags: vec!["meta".to_string()],
            },
        ]
    }

    // Meta-tool implementations

    /// Compact-mode `execute_tool` meta-tool.
    ///
    /// Security (audit item #7):
    /// 1. `tool_name` MUST be a non-empty string. Reject everything else
    ///    explicitly rather than silently falling through to a registry miss.
    /// 2. `arguments` MUST be a JSON object (or absent). Strings, arrays, and
    ///    nulls are rejected.
    /// 3. Meta-tool reflection (e.g. `execute_tool("execute_tool", \u2026)`) is
    ///    rejected outright.
    /// 4. The actual blocklist (shell_execute, systemd_*, ovs_create, etc.)
    ///    is enforced inside `ctx.execute_tool` so that the verbose
    ///    `tools/call` path is covered by the same check.
    async fn meta_execute_tool(&self, ctx: &RequestContext, args: Value) -> Result<Value> {
        let obj = args
            .as_object()
            .ok_or_else(|| anyhow::anyhow!("execute_tool: arguments must be a JSON object"))?;

        let tool_name = obj
            .get("tool_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("execute_tool: tool_name must be a non-empty string"))?
            .trim();

        if tool_name.is_empty() {
            anyhow::bail!("execute_tool: tool_name must be a non-empty string");
        }

        if META_TOOL_NAMES.contains(&tool_name) {
            warn!(
                request_id = %ctx.request_id,
                tool = %tool_name,
                "Rejected meta-tool reflection via execute_tool"
            );
            anyhow::bail!(
                "execute_tool cannot target meta-tool '{}'; call it directly via tools/call",
                tool_name
            );
        }

        // arguments defaults to {} but, if present, must be an object.
        let arguments = match obj.get("arguments") {
            None => json!({}),
            Some(v) if v.is_object() => v.clone(),
            Some(_) => anyhow::bail!("execute_tool: arguments must be a JSON object"),
        };

        // The blocklist check happens here, inside ctx.execute_tool.
        ctx.execute_tool(tool_name, arguments).await
    }

    fn meta_list_tools(&self, ctx: &RequestContext, args: Value) -> Result<Value> {
        let category = args.as_object().and_then(|o| o.get("category")).and_then(|v| v.as_str());
        let offset = args.as_object().and_then(|o| o.get("offset")).and_then(|v| v.as_u64()).unwrap_or(0) as usize;
        let limit = args.as_object().and_then(|o| o.get("limit")).and_then(|v| v.as_u64()).unwrap_or(50) as usize;
        
        let tools = ctx.list_tools(offset, limit, category);
        let total = ctx.tool_count();
        
        Ok(json!({
            "tools": tools.iter().map(|t| json!({
                "name": t.name,
                "description": t.description,
                "category": t.category
            })).collect::<Vec<_>>(),
            "total": total,
            "offset": offset,
            "limit": limit,
            "has_more": offset + tools.len() < total
        }))
    }

    fn meta_search_tools(&self, ctx: &RequestContext, args: Value) -> Result<Value> {
        let query = args.as_object().and_then(|o| o.get("query")).and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing query"))?;
        
        let results = ctx.search_tools(query);
        
        Ok(json!({
            "query": query,
            "results": results.iter().map(|t| json!({
                "name": t.name,
                "description": t.description,
                "category": t.category
            })).collect::<Vec<_>>(),
            "count": results.len()
        }))
    }

    fn meta_get_tool_schema(&self, ctx: &RequestContext, args: Value) -> Result<Value> {
        let tool_name = args.as_object().and_then(|o| o.get("tool_name")).and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing tool_name"))?;
        
        let def = ctx.get_definition(tool_name)
            .ok_or_else(|| anyhow::anyhow!("Tool not found: {}", tool_name))?;
        
        Ok(json!({
            "name": def.name,
            "description": def.description,
            "inputSchema": def.input_schema,
            "category": def.category
        }))
    }

    fn meta_respond(&self, args: Value) -> Result<Value> {
        let message = args.as_object().and_then(|o| o.get("message")).and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing message"))?;
        
        Ok(json!({
            "type": "response",
            "message": message,
            "delivered": true
        }))
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/resources.rs">
//! Resource Registry for MCP
//!
//! Provides documentation resources served via MCP resources protocol.

use serde::{Deserialize, Serialize};

/// Resource information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceInfo {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

/// Resource template information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceTemplateInfo {
    pub uri_template: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}

/// Resource registry
pub struct ResourceRegistry {
    resources: Vec<ResourceInfo>,
    templates: Vec<ResourceTemplateInfo>,
}

impl Default for ResourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceRegistry {
    pub fn new() -> Self {
        let resources = vec![
            ResourceInfo {
                uri: "docs://system-prompt".to_string(),
                name: "System Prompt".to_string(),
                description: Some("System prompt for op-mcp".to_string()),
                mime_type: Some("text/plain".to_string()),
            },
            ResourceInfo {
                uri: "docs://architecture".to_string(),
                name: "Architecture".to_string(),
                description: Some("System architecture documentation".to_string()),
                mime_type: Some("text/markdown".to_string()),
            },
        ];
        let templates = vec![ResourceTemplateInfo {
            uri_template: "docs://{name}".to_string(),
            name: "Documentation".to_string(),
            description: Some("Read bundled op-mcp documentation resources".to_string()),
            mime_type: Some("text/plain".to_string()),
        }];
        Self {
            resources,
            templates,
        }
    }

    pub fn add_resource(&mut self, resource: ResourceInfo) {
        self.resources.push(resource);
    }

    pub fn list_resources(&self) -> &[ResourceInfo] {
        &self.resources
    }

    pub fn list_templates(&self) -> &[ResourceTemplateInfo] {
        &self.templates
    }

    pub fn get_resource(&self, uri: &str) -> Option<&ResourceInfo> {
        self.resources.iter().find(|r| r.uri == uri)
    }

    pub async fn read_resource(&self, uri: &str) -> Option<String> {
        match uri {
            "docs://system-prompt" => Some(self.generate_system_prompt().await),
            "docs://architecture" => Some(ARCHITECTURE_DOC.to_string()),
            _ => None,
        }
    }

    async fn generate_system_prompt(&self) -> String {
        // Try to get from op_chat if available
        #[cfg(feature = "op-chat")]
        {
            let msg = op_chat::generate_system_prompt(None).await;
            return msg.content;
        }

        #[cfg(not(feature = "op-chat"))]
        {
            "You are a helpful assistant with access to system tools.".to_string()
        }
    }
}

const ARCHITECTURE_DOC: &str = r#"# op-mcp Architecture

## Overview

op-mcp is a unified MCP (Model Context Protocol) server supporting multiple transports:

- **Stdio**: Standard input/output for CLI integration
- **HTTP**: REST endpoints with SSE support
- **WebSocket**: Full-duplex communication
- **gRPC**: High-performance RPC (optional)

## Components

### McpServer
Core server handling all MCP protocol logic. Transport-agnostic.

### Transport Layer
Abstract `Transport` trait with implementations for each protocol.

### Tool System
`ToolExecutor` trait allows pluggable tool backends.

## Configuration

| Option | Default | Description |
|--------|---------|-------------|
| `compact_mode` | false | Use 5 lazy meta-tools instead of all |
| `max_tools` | 500 | Maximum tools to expose |
| `blocked_patterns` | [...] | Tool patterns to block |
"#;
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/router.rs">
//! MCP Router - HTTP endpoints for MCP protocol
//!
//! This module exports a router that can be mounted by op-http.
//! NO server code here - just route definitions.

use axum::{
    extract::State,
    response::{sse::Event, IntoResponse, Sse},
    routing::{get, post},
    Json, Router,
};
use futures::stream::{self, Stream, StreamExt};
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use std::{convert::Infallible, sync::Arc, time::Duration};

use crate::lazy_tools::LazyToolManager;
use crate::server::McpServer;

/// MCP service state
#[derive(Clone)]
pub struct McpState {
    pub server: Arc<McpServer>,
    pub tool_manager: Arc<LazyToolManager>,
}

impl McpState {
    pub async fn new() -> anyhow::Result<Self> {
        let server = Arc::new(McpServer::new(Default::default()).await?);
        let tool_manager = Arc::new(LazyToolManager::new().await?);

        Ok(Self {
            server,
            tool_manager,
        })
    }
}

/// Create the MCP router
///
/// Mount this at `/api/mcp` in the unified server:
/// ```ignore
/// use op_http::prelude::*;
/// use op_mcp::router::{create_router, McpState};
///
/// let state = McpState::new().await?;
/// let router = RouterBuilder::new()
///     .nest("/api/mcp", "mcp", create_router(state))
///     .build();
/// ```
pub fn create_router(state: McpState) -> Router {
    Router::new()
        .route("/", post(mcp_handler))
        .route("/health", get(health_handler))
        .route("/sse", get(sse_handler))
        .route("/tools", get(list_all_tools_handler))
        .route("/tools/:name", post(call_tool_handler))
        .route("/initialize", post(initialize_handler))
        .with_state(state)
}

/// Service info for op-http ServiceRouter trait
pub struct McpServiceRouter;

impl op_http::router::ServiceRouter for McpServiceRouter {
    fn prefix() -> &'static str {
        "/api/mcp"
    }

    fn name() -> &'static str {
        "mcp"
    }

    fn description() -> &'static str {
        "MCP protocol endpoints"
    }
}

// === Handlers ===

#[derive(Deserialize)]
struct McpRequest {
    jsonrpc: String,
    id: Value,
    method: String,
    params: Option<Value>,
}

#[derive(Serialize)]
struct McpResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<Value>,
}

async fn mcp_handler(
    State(state): State<McpState>,
    Json(request): Json<McpRequest>,
) -> impl IntoResponse {
    let result = match request.method.as_str() {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "serverInfo": {
                "name": "op-mcp",
                "version": env!("CARGO_PKG_VERSION")
            },
            "capabilities": {
                "tools": { "listChanged": true }
            }
        })),
        "tools/list" => {
            let tools = state.tool_manager.list_all_tools().await;
            Ok(json!({ "tools": tools }))
        }
        "tools/call" => {
            if let Some(params) = request.params {
                let name = params.as_object().and_then(|o| o.get("name")).and_then(|v| v.as_str()).unwrap_or("");
                let args = params.as_object().and_then(|o| o.get("arguments")).cloned().unwrap_or(json!({}));
                if let Some(tool) = state.tool_manager.get_tool(name).await {
                    match tool.execute(args).await {
                        Ok(result) => Ok(result),
                        Err(e) => Err(json!({
                            "code": -32603,
                            "message": e.to_string()
                        })),
                    }
                } else {
                    Err(json!({
                        "code": -32601,
                        "message": format!("Tool not found: {}", name)
                    }))
                }
            }
            else {
                Err(json!({
                    "code": -32602,
                    "message": "Missing params"
                }))
            }
        }
        _ => Err(json!({
            "code": -32601,
            "message": format!("Method not found: {}", request.method)
        })),
    };

    let response = match result {
        Ok(r) => McpResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: Some(r),
            error: None,
        },
        Err(e) => McpResponse {
            jsonrpc: "2.0".to_string(),
            id: request.id,
            result: None,
            error: Some(e),
        },
    };

    Json(response)
}

async fn health_handler() -> impl IntoResponse {
    Json(json!({
        "status": "healthy",
        "service": "mcp",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

async fn sse_handler(
    State(state): State<McpState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // Send initial events then keep alive
    let tools = state.tool_manager.list_all_tools().await;

    let initial_events = vec![
        Event::default()
            .event("endpoint")
            .data("/api/mcp"),
        Event::default()
            .event("tools")
            .data(json!({
                "name": "op-mcp",
                "count": tools.len(),
                "tools": tools
            }).to_string()),
    ];

    let initial_stream = stream::iter(initial_events.into_iter().map(Ok));

    let keepalive_stream = stream::unfold(0u64, |counter| async move {
        tokio::time::sleep(Duration::from_secs(30)).await;
        let event = Event::default()
            .event("ping")
            .data(json!({ "counter": counter }).to_string());
        Some((Ok(event), counter + 1))
    });

    Sse::new(initial_stream.chain(keepalive_stream)).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(30))
            .text("keepalive"),
    )
}

async fn list_all_tools_handler(State(state): State<McpState>) -> impl IntoResponse {
    let tools = state.tool_manager.list_all_tools().await;
    Json(json!({ "tools": tools }))
}

async fn call_tool_handler(
    State(state): State<McpState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(params): Json<Value>,
) -> impl IntoResponse {
    if let Some(tool) = state.tool_manager.get_tool(&name).await {
        match tool.execute(params).await {
            Ok(result) => Json(json!({ "result": result })),
            Err(e) => Json(json!({ "error": e.to_string() })),
        }
    } else {
        Json(json!({ "error": "Tool not found" }))
    }
}

async fn initialize_handler() -> impl IntoResponse {
    Json(json!({
        "protocolVersion": "2024-11-05",
        "serverInfo": {
            "name": "op-mcp",
            "version": env!("CARGO_PKG_VERSION")
        },
        "capabilities": {
            "tools": { "listChanged": true }
        }
    }))
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/server.rs">
//! Unified MCP Server
//!
//! Core server implementation that handles all MCP protocol logic.
//! Transport-agnostic - works with stdio, HTTP, WebSocket, gRPC, etc.

use crate::protocol::{JsonRpcError, McpRequest, McpResponse};
use crate::resources::ResourceRegistry;
use crate::{PROTOCOL_VERSION, SERVER_NAME, SERVER_VERSION};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Server configuration
#[derive(Debug, Clone)]
pub struct McpServerConfig {
    /// Server name override
    pub name: Option<String>,
    /// Enable compact mode (5 meta-tools instead of all tools)
    pub compact_mode: bool,
    /// Tool categories to expose (None = all)
    pub allowed_categories: Option<Vec<String>>,
    /// Tool name patterns to block
    pub blocked_patterns: Vec<String>,
    /// Maximum tools to return in list
    pub max_tools: usize,
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            name: None,
            compact_mode: false,
            allowed_categories: None,
            blocked_patterns: vec![
                "shell_execute".into(),
                "write_file".into(),
                "systemd_start".into(),
                "systemd_stop".into(),
                "systemd_restart".into(),
                "systemd_enable".into(),
                "systemd_disable".into(),
            ],
            max_tools: 500,
        }
    }
}

/// Tool information for MCP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<Value>,
}

/// Tool executor trait - implement this to provide tools
#[async_trait::async_trait]
pub trait ToolExecutor: Send + Sync {
    /// List available tools
    async fn list_tools(&self) -> Result<Vec<ToolInfo>>;

    /// Execute a tool by name
    async fn execute_tool(&self, name: &str, arguments: Value) -> Result<Value>;

    /// Get schema for a specific tool
    async fn get_tool_schema(&self, name: &str) -> Result<Option<Value>>;

    /// Search tools by query
    async fn search_tools(&self, query: &str, limit: usize) -> Result<Vec<ToolInfo>>;
}

/// Default tool executor using op_tools registry
pub struct DefaultToolExecutor {
    registry: Arc<op_tools::ToolRegistry>,
}

impl DefaultToolExecutor {
    pub fn new(registry: Arc<op_tools::ToolRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait::async_trait]
impl ToolExecutor for DefaultToolExecutor {
    async fn list_tools(&self) -> Result<Vec<ToolInfo>> {
        let tools = self.registry.list().await;
        Ok(tools
            .into_iter()
            .map(|t| ToolInfo {
                name: t.name,
                description: t.description,
                input_schema: t.input_schema,
                annotations: None,
            })
            .collect())
    }

    async fn execute_tool(&self, name: &str, arguments: Value) -> Result<Value> {
        if let Some(tool) = self.registry.get(name).await {
            tool.execute(arguments).await
        } else {
            Err(anyhow::anyhow!("Tool not found: {}", name))
        }
    }

    async fn get_tool_schema(&self, name: &str) -> Result<Option<Value>> {
        if let Some(def) = self.registry.get_definition(name).await {
            Ok(Some(def.input_schema))
        } else {
            Ok(None)
        }
    }

    async fn search_tools(&self, query: &str, limit: usize) -> Result<Vec<ToolInfo>> {
        let tools = self.registry.list().await;
        let query_lower = query.to_lowercase();
        Ok(tools
            .into_iter()
            .filter(|t| {
                t.name.to_lowercase().contains(&query_lower)
                    || t.description.to_lowercase().contains(&query_lower)
            })
            .take(limit)
            .map(|t| ToolInfo {
                name: t.name,
                description: t.description,
                input_schema: t.input_schema,
                annotations: None,
            })
            .collect())
    }
}

/// Unified MCP Server
#[allow(dead_code)]
pub struct McpServer {
    config: McpServerConfig,
    tool_executor: Arc<dyn ToolExecutor>,
    resources: ResourceRegistry,
    /// Client info from last initialize
    client_info: RwLock<Option<ClientInfo>>,
    /// Whether the current handler has completed initialize.
    initialized: RwLock<bool>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct ClientInfo {
    name: String,
    version: Option<String>,
}

impl McpServer {
    /// Create server with default tool executor
    pub async fn new(config: McpServerConfig) -> Result<Arc<Self>> {
        let registry = Arc::new(op_tools::ToolRegistry::new());
        op_tools::register_builtin_tools(&registry).await?;

        let tool_executor = Arc::new(DefaultToolExecutor::new(registry));
        Ok(Arc::new(Self::with_executor(config, tool_executor)))
    }

    /// Create server with custom tool executor
    pub fn with_executor(config: McpServerConfig, tool_executor: Arc<dyn ToolExecutor>) -> Self {
        Self {
            config,
            tool_executor,
            resources: ResourceRegistry::new(),
            client_info: RwLock::new(None),
            initialized: RwLock::new(false),
        }
    }

    /// Handle an MCP request
    pub async fn handle_request(&self, request: McpRequest) -> McpResponse {
        debug!(method = %request.method, "Handling MCP request");

        if !self.is_lifecycle_method(&request.method) && !*self.initialized.read().await {
            return McpResponse::error(
                request.id,
                JsonRpcError::new(
                    -32002,
                    "Server not initialized. Call initialize before using tools or resources.",
                ),
            );
        }

        match request.method.as_str() {
            "initialize" => self.handle_initialize(request).await,
            "initialized" => self.handle_initialized(request).await,
            "notifications/initialized" => self.handle_initialized(request).await,
            "ping" => McpResponse::success(request.id, json!({})),
            "tools/list" => self.handle_tools_list(request).await,
            "tools/call" => self.handle_tools_call(request).await,
            "resources/list" => self.handle_resources_list(request).await,
            "resources/templates/list" => self.handle_resources_templates_list(request).await,
            "resources/read" => self.handle_resources_read(request).await,
            // Compact mode meta-tools
            "list_tools" | "search_tools" | "get_tool_schema" | "execute_tool" | "respond" => {
                self.handle_compact_tool(request).await
            }
            _ => McpResponse::error(request.id, JsonRpcError::method_not_found(&request.method)),
        }
    }

    async fn handle_initialize(&self, request: McpRequest) -> McpResponse {
        // Extract client info
        let client_name = request
            .params
            .as_ref()
            .and_then(|p| p.as_object())
            .and_then(|obj| obj.get("clientInfo"))
            .and_then(|ci| ci.as_object())
            .and_then(|ci_obj| ci_obj.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("unknown");

        let client_version = request
            .params
            .as_ref()
            .and_then(|p| p.as_object())
            .and_then(|obj| obj.get("clientInfo"))
            .and_then(|ci| ci.as_object())
            .and_then(|ci_obj| ci_obj.get("version"))
            .and_then(|v| v.as_str());

        // Store client info
        *self.client_info.write().await = Some(ClientInfo {
            name: client_name.to_string(),
            version: client_version.map(String::from),
        });
        *self.initialized.write().await = true;

        // Auto-detect compact mode for known clients
        let use_compact = self.config.compact_mode || Self::should_use_compact_mode(client_name);

        info!(
            client = %client_name,
            version = %client_version.unwrap_or("?"),
            compact = %use_compact,
            "Client connected"
        );

        let server_name = self.config.name.as_deref().unwrap_or(SERVER_NAME);

        McpResponse::success(
            request.id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {
                    "tools": { "listChanged": false },
                    "resources": { "subscribe": false, "listChanged": false }
                },
                "serverInfo": {
                    "name": server_name,
                    "version": SERVER_VERSION
                },
                "instructions": "Initialize once, then use tools/list, tools/call, resources/list, resources/templates/list, and resources/read.",
                "_meta": {
                    "compactMode": use_compact
                }
            }),
        )
    }

    async fn handle_initialized(&self, request: McpRequest) -> McpResponse {
        McpResponse::success(request.id, json!({}))
    }

    async fn handle_tools_list(&self, request: McpRequest) -> McpResponse {
        // Check if compact mode
        let client_info = self.client_info.read().await;
        let use_compact = self.config.compact_mode
            || client_info
                .as_ref()
                .map(|c| Self::should_use_compact_mode(&c.name))
                .unwrap_or(false);

        if use_compact {
            return self.get_compact_tools_response(request.id).await;
        }

        // Full mode - return all tools
        match self.tool_executor.list_tools().await {
            Ok(tools) => {
                let filtered: Vec<_> = tools
                    .into_iter()
                    .filter(|t| !self.is_tool_blocked(&t.name))
                    .take(self.config.max_tools)
                    .collect();

                McpResponse::success(
                    request.id,
                    json!({
                        "tools": filtered
                    }),
                )
            }
            Err(e) => {
                error!(error = %e, "Failed to list tools");
                McpResponse::error(request.id, JsonRpcError::internal_error(e.to_string()))
            }
        }
    }

    async fn handle_tools_call(&self, request: McpRequest) -> McpResponse {
        let params = match &request.params {
            Some(p) => p,
            None => {
                return McpResponse::error(
                    request.id,
                    JsonRpcError::invalid_params("Missing params"),
                )
            }
        };

        let tool_name = match params
            .as_object()
            .and_then(|obj| obj.get("name"))
            .and_then(|n| n.as_str())
        {
            Some(n) => n,
            None => {
                return McpResponse::error(
                    request.id,
                    JsonRpcError::invalid_params("Missing tool name"),
                )
            }
        };

        // Check if blocked
        if self.is_tool_blocked(tool_name) {
            warn!(tool = %tool_name, "Blocked tool execution attempt");
            return McpResponse::error(
                request.id,
                JsonRpcError::new(-32001, format!("Tool '{}' is not available", tool_name)),
            );
        }

        let arguments = params
            .as_object()
            .and_then(|obj| obj.get("arguments"))
            .cloned()
            .unwrap_or(json!({}));

        // Inject code context for smart suggestions (if op-tools has code_search)
        #[cfg(feature = "code_search")]
        {
            let current_file = arguments
                .get("path")
                .and_then(|p| p.as_str())
                .or(arguments.get("file").and_then(|f| f.as_str()));

            if let Ok(code_context) =
                op_tools::code_search::inject_code_context(tool_name, &arguments, current_file)
                    .await
            {
                if !code_context.is_empty() {
                    arguments["_code_context"] = code_context.to_json();
                    debug!(tool = %tool_name, "Injected code context");
                }
            }
        }

        match self.tool_executor.execute_tool(tool_name, arguments).await {
            Ok(result) => McpResponse::success(
                request.id,
                json!({
                    "content": [{
                        "type": "text",
                        "text": simd_json::to_string_pretty(&result).unwrap_or_default()
                    }],
                    "isError": false
                }),
            ),
            Err(e) => McpResponse::success(
                request.id,
                json!({
                    "content": [{
                        "type": "text",
                        "text": format!("Error: {}", e)
                    }],
                    "isError": true
                }),
            ),
        }
    }

    async fn handle_resources_list(&self, request: McpRequest) -> McpResponse {
        let resources: Vec<_> = self
            .resources
            .list_resources()
            .iter()
            .map(|r| {
                json!({
                    "uri": r.uri,
                    "name": r.name,
                    "description": r.description,
                    "mimeType": r.mime_type
                })
            })
            .collect();

        McpResponse::success(request.id, json!({ "resources": resources }))
    }

    async fn handle_resources_templates_list(&self, request: McpRequest) -> McpResponse {
        let templates: Vec<_> = self
            .resources
            .list_templates()
            .iter()
            .map(|template| {
                json!({
                    "uriTemplate": template.uri_template,
                    "name": template.name,
                    "description": template.description,
                    "mimeType": template.mime_type
                })
            })
            .collect();

        McpResponse::success(request.id, json!({ "resourceTemplates": templates }))
    }

    async fn handle_resources_read(&self, request: McpRequest) -> McpResponse {
        let uri = request
            .params
            .as_ref()
            .and_then(|p| p.as_object())
            .and_then(|obj| obj.get("uri"))
            .and_then(|u| u.as_str())
            .unwrap_or("");

        if uri.is_empty() {
            return McpResponse::error(request.id, JsonRpcError::invalid_params("Missing uri"));
        }

        match self.resources.read_resource(uri).await {
            Some(content) => McpResponse::success(
                request.id,
                json!({
                    "contents": [{
                        "uri": uri,
                        "mimeType": "text/plain",
                        "text": content
                    }]
                }),
            ),
            None => McpResponse::error(
                request.id,
                JsonRpcError::new(-32002, format!("Resource not found: {}", uri)),
            ),
        }
    }

    /// Handle compact mode meta-tools
    async fn handle_compact_tool(&self, request: McpRequest) -> McpResponse {
        let params = request.params.as_ref().cloned().unwrap_or(json!({}));

        match request.method.as_str() {
            "list_tools" => {
                let category = params
                    .as_object()
                    .and_then(|obj| obj.get("category"))
                    .and_then(|c| c.as_str());
                let limit = params
                    .as_object()
                    .and_then(|obj| obj.get("limit"))
                    .and_then(|l| l.as_u64())
                    .unwrap_or(20) as usize;

                match self.tool_executor.list_tools().await {
                    Ok(tools) => {
                        let filtered: Vec<_> = tools
                            .into_iter()
                            .filter(|t| !self.is_tool_blocked(&t.name))
                            .filter(|t| {
                                category
                                    .map(|c| {
                                        t.name.contains(c)
                                            || t.description
                                                .to_lowercase()
                                                .contains(&c.to_lowercase())
                                    })
                                    .unwrap_or(true)
                            })
                            .take(limit)
                            .map(|t| {
                                json!({
                                    "name": t.name,
                                    "description": t.description
                                })
                            })
                            .collect();

                        McpResponse::success(
                            request.id,
                            json!({
                                "content": [{
                                    "type": "text",
                                    "text": simd_json::to_string_pretty(&filtered).unwrap()
                                }],
                                "isError": false
                            }),
                        )
                    }
                    Err(e) => McpResponse::success(
                        request.id,
                        json!({
                            "content": [{ "type": "text", "text": format!("Error: {}", e) }],
                            "isError": true
                        }),
                    ),
                }
            }
            "search_tools" => {
                let query = params
                    .as_object()
                    .and_then(|obj| obj.get("query"))
                    .and_then(|q| q.as_str())
                    .unwrap_or("");
                let limit = params
                    .as_object()
                    .and_then(|obj| obj.get("limit"))
                    .and_then(|l| l.as_u64())
                    .unwrap_or(10) as usize;

                match self.tool_executor.search_tools(query, limit).await {
                    Ok(tools) => {
                        let results: Vec<_> = tools
                            .into_iter()
                            .filter(|t| !self.is_tool_blocked(&t.name))
                            .map(|t| {
                                json!({
                                    "name": t.name,
                                    "description": t.description
                                })
                            })
                            .collect();

                        McpResponse::success(
                            request.id,
                            json!({
                                "content": [{
                                    "type": "text",
                                    "text": simd_json::to_string_pretty(&results).unwrap()
                                }],
                                "isError": false
                            }),
                        )
                    }
                    Err(e) => McpResponse::success(
                        request.id,
                        json!({
                            "content": [{ "type": "text", "text": format!("Error: {}", e) }],
                            "isError": true
                        }),
                    ),
                }
            }
            "get_tool_schema" => {
                let tool_name = params
                    .as_object()
                    .and_then(|obj| obj.get("tool_name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("");

                match self.tool_executor.get_tool_schema(tool_name).await {
                    Ok(Some(schema)) => McpResponse::success(
                        request.id,
                        json!({
                            "content": [{
                                "type": "text",
                                "text": simd_json::to_string_pretty(&schema).unwrap()
                            }],
                            "isError": false
                        }),
                    ),
                    Ok(None) => McpResponse::success(
                        request.id,
                        json!({
                            "content": [{ "type": "text", "text": format!("Tool not found: {}", tool_name) }],
                            "isError": true
                        }),
                    ),
                    Err(e) => McpResponse::success(
                        request.id,
                        json!({
                            "content": [{ "type": "text", "text": format!("Error: {}", e) }],
                            "isError": true
                        }),
                    ),
                }
            }
            "execute_tool" => {
                let tool_name = params
                    .as_object()
                    .and_then(|obj| obj.get("tool_name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("");
                let arguments = params
                    .as_object()
                    .and_then(|obj| obj.get("arguments"))
                    .cloned()
                    .unwrap_or(json!({}));

                // Delegate to tools/call logic
                let call_request = McpRequest {
                    jsonrpc: "2.0".into(),
                    id: request.id.clone(),
                    method: "tools/call".into(),
                    params: Some(json!({
                        "name": tool_name,
                        "arguments": arguments
                    })),
                    meta: None,
                };
                self.handle_tools_call(call_request).await
            }
            "respond" => {
                let message = params
                    .as_object()
                    .and_then(|obj| obj.get("message"))
                    .and_then(|m| m.as_str())
                    .unwrap_or("");

                McpResponse::success(
                    request.id,
                    json!({
                        "content": [{
                            "type": "text",
                            "text": message
                        }],
                        "isError": false
                    }),
                )
            }
            _ => McpResponse::error(request.id, JsonRpcError::method_not_found(&request.method)),
        }
    }

    /// Get compact mode tools response
    async fn get_compact_tools_response(&self, id: Option<Value>) -> McpResponse {
        let compact_tools = vec![
            json!({
                "name": "list_tools",
                "description": "List available tools. Filter by 'category'. Returns names and descriptions.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "category": { "type": "string", "description": "Filter by category" },
                        "limit": { "type": "integer", "description": "Max tools (default: 20)" }
                    }
                }
            }),
            json!({
                "name": "search_tools",
                "description": "Search tools by keyword.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Search query" },
                        "limit": { "type": "integer", "description": "Max results (default: 10)" }
                    },
                    "required": ["query"]
                }
            }),
            json!({
                "name": "get_tool_schema",
                "description": "Get input schema for a tool. Call before execute_tool.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "tool_name": { "type": "string", "description": "Tool name" }
                    },
                    "required": ["tool_name"]
                }
            }),
            json!({
                "name": "execute_tool",
                "description": "Execute a tool by name.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "tool_name": { "type": "string", "description": "Tool name" },
                        "arguments": { "type": "object", "description": "Tool arguments" }
                    },
                    "required": ["tool_name"]
                }
            }),
            json!({
                "name": "respond",
                "description": "Send the final response to the user.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "message": { "type": "string", "description": "Response message" }
                    },
                    "required": ["message"]
                }
            }),
        ];

        McpResponse::success(
            id,
            json!({
                "tools": compact_tools,
                "_meta": { "compactMode": true }
            }),
        )
    }

    /// Check if a tool should be blocked
    fn is_tool_blocked(&self, name: &str) -> bool {
        self.config
            .blocked_patterns
            .iter()
            .any(|p| name.contains(p))
    }

    /// Check if client should use compact mode
    fn should_use_compact_mode(client_name: &str) -> bool {
        let name_lower = client_name.to_lowercase();
        name_lower.contains("gemini")
            || name_lower.contains("claude")
            || name_lower.contains("cursor")
    }

    /// Get tool executor reference
    pub fn tool_executor(&self) -> &Arc<dyn ToolExecutor> {
        &self.tool_executor
    }

    fn is_lifecycle_method(&self, method: &str) -> bool {
        matches!(
            method,
            "initialize" | "initialized" | "notifications/initialized" | "ping"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    struct MockToolExecutor;

    #[async_trait::async_trait]
    impl ToolExecutor for MockToolExecutor {
        async fn list_tools(&self) -> Result<Vec<ToolInfo>> {
            Ok(vec![ToolInfo {
                name: "echo".to_string(),
                description: "Echo input".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "message": { "type": "string" }
                    }
                }),
                annotations: None,
            }])
        }

        async fn execute_tool(&self, _name: &str, arguments: Value) -> Result<Value> {
            Ok(arguments)
        }

        async fn get_tool_schema(&self, name: &str) -> Result<Option<Value>> {
            Ok((name == "echo").then(|| {
                json!({
                    "type": "object",
                    "properties": {
                        "message": { "type": "string" }
                    }
                })
            }))
        }

        async fn search_tools(&self, _query: &str, _limit: usize) -> Result<Vec<ToolInfo>> {
            self.list_tools().await
        }
    }

    fn test_server() -> McpServer {
        McpServer::with_executor(Default::default(), Arc::new(MockToolExecutor))
    }

    #[tokio::test]
    async fn should_reject_non_lifecycle_request_before_initialize() {
        let server = test_server();
        let response = server
            .handle_request(McpRequest::new("tools/list").with_id(json!(1)))
            .await;

        assert_eq!(response.error.as_ref().map(|err| err.code), Some(-32002));
    }

    #[tokio::test]
    async fn should_accept_initialized_notification_after_initialize() {
        let server = test_server();

        let init =
            server
                .handle_request(McpRequest::new("initialize").with_id(json!(1)).with_params(
                    json!({
                        "clientInfo": { "name": "test-client", "version": "1.0.0" }
                    }),
                ))
                .await;
        assert!(init.is_success());

        let response = server
            .handle_request(McpRequest::new("notifications/initialized"))
            .await;
        assert!(response.is_success());
    }

    #[tokio::test]
    async fn should_list_resource_templates() {
        let server = test_server();
        let _ =
            server
                .handle_request(McpRequest::new("initialize").with_id(json!(1)).with_params(
                    json!({
                        "clientInfo": { "name": "test-client" }
                    }),
                ))
                .await;

        let response = server
            .handle_request(McpRequest::new("resources/templates/list").with_id(json!(2)))
            .await;

        let templates = response
            .result
            .as_ref()
            .and_then(|result| result.get("resourceTemplates"))
            .and_then(|templates| templates.as_array())
            .cloned()
            .unwrap_or_default();

        assert_eq!(templates.len(), 1);
        assert_eq!(
            templates[0]
                .get("uriTemplate")
                .and_then(|value| value.as_str()),
            Some("docs://{name}")
        );
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/sse.rs">
//! SSE (Server-Sent Events) Transport for MCP
//!
//! Allows MCP server to run as a long-lived HTTP daemon.
//! Clients connect via SSE for responses and POST for requests.

use crate::{McpRequest, McpServer};
use axum::{
    extract::State,
    response::sse::{Event, Sse},
    routing::{get, post},
    Json, Router,
};
use futures::stream::Stream;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

/// SSE Transport state
pub struct SseTransport {
    mcp_server: Arc<McpServer>,
    /// Broadcast channel for SSE events
    event_tx: broadcast::Sender<String>,
}

impl SseTransport {
    pub fn new(mcp_server: Arc<McpServer>) -> Self {
        let (event_tx, _) = broadcast::channel(100);
        Self {
            mcp_server,
            event_tx,
        }
    }

    /// Create the Axum router for SSE transport
    pub fn router(self) -> Router {
        let state = Arc::new(self);

        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        Router::new()
            .route("/sse", get(sse_handler))
            .route("/message", post(message_handler))
            .route("/health", get(health_handler))
            .with_state(state)
            .layer(cors)
    }
}

/// SSE endpoint - clients connect here to receive responses
async fn sse_handler(
    State(state): State<Arc<SseTransport>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    info!("SSE client connected");

    let rx = state.event_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|result| {
        match result {
            Ok(data) => Some(Ok(Event::default().data(data))),
            Err(_) => None, // Skip lagged messages
        }
    });

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("ping"),
    )
}

/// Message endpoint - clients POST MCP requests here
async fn message_handler(
    State(state): State<Arc<SseTransport>>,
    Json(request): Json<McpRequest>,
) -> Json<simd_json::OwnedValue> {
    info!("Received MCP request via HTTP: {}", request.method);

    // Handle the request
    let response = state.mcp_server.handle_request(request).await;

    // Do NOT broadcast command responses to all SSE clients.
    // Responses should only go back to the caller.
    // The shared event_tx channel should be reserved for actual server events/notifications.

    // Return response directly (for non-SSE clients)
    Json(simd_json::serde::to_owned_value(&response).unwrap_or_default())
}

/// Health check endpoint
async fn health_handler() -> &'static str {
    "ok"
}

/// Run the SSE server
pub async fn run_sse_server(mcp_server: Arc<McpServer>, bind_addr: &str) -> anyhow::Result<()> {
    let transport = SseTransport::new(mcp_server);
    let app = transport.router();

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    info!("MCP SSE server listening on {}", bind_addr);

    axum::serve(listener, app).await?;
    Ok(())
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/tool_adapter_orchestrated.rs">
//! Tool Adapter with Orchestration Integration
//!
//! This module extends the tool adapter to use the orchestrated executor,
//! enabling workstacks, skills, and multi-agent coordination.

use anyhow::Result;
use op_chat::{
    ExecutionMode, OrchestratedExecutor, OrchestratedResult, Workflow, WorkflowStep,
};
use op_core::ExecutionTracker;
use op_tools::ToolRegistry;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Patterns that block tools from being exposed via MCP
const BLOCKED_PATTERNS: &[&str] = &[
    "shell_execute",
    "write_file",
    "systemd_start",
    "systemd_stop",
    "systemd_restart",
    "systemd_reload",
    "systemd_enable",
    "systemd_disable",
    "systemd_apply",
    "ovs_create",
    "ovs_delete",
    "ovs_add",
    "ovs_set",
    "_apply",
    "btrfs_create",
    "btrfs_delete",
    "btrfs_snapshot",
];

/// Check if a tool name should be blocked
fn is_tool_blocked(name: &str) -> bool {
    BLOCKED_PATTERNS
        .iter()
        .any(|pattern| name.contains(pattern))
}

/// Check if tool matches filter
fn matches_tool_filter(name: &str) -> bool {
    match std::env::var("MCP_TOOL_FILTER").ok().as_deref() {
        Some("systemd") => name.starts_with("dbus_systemd1_"),
        Some("login") => name.starts_with("dbus_login1_"),
        Some("ovs") => name.starts_with("ovs_"),
        Some("agents") => {
            name.starts_with("agent_")
                || name.starts_with("list_")
                || name.starts_with("spawn_")
                || name.contains("agent")
        }
        Some("core") => {
            name.starts_with("dbus_DBus_")
                || name.starts_with("dbus_login1_")
                || name.starts_with("ovs_")
                || name.starts_with("plugin_")
        }
        Some("skills") => is_orchestration_tool(name),
        Some("orchestration") => is_orchestration_tool(name),
        Some(_) | None => true,
    }
}

/// Check if this is an orchestration tool
fn is_orchestration_tool(name: &str) -> bool {
    name.starts_with("skill_")
        || name.starts_with("workstack_")
        || name.starts_with("workflow_")
}

/// Orchestrated Tool Adapter - Unified execution with orchestration
pub struct OrchestratedToolAdapter {
    tool_registry: Arc<ToolRegistry>,
    orchestrated_executor: Arc<OrchestratedExecutor>,
    execution_tracker: Arc<ExecutionTracker>,
}

impl OrchestratedToolAdapter {
    /// Create new orchestrated tool adapter
    pub async fn new(tool_registry: Arc<ToolRegistry>) -> Result<Self> {
        let execution_tracker = Arc::new(ExecutionTracker::new(1000));
        let orchestrated_executor = Arc::new(
            OrchestratedExecutor::new(tool_registry.clone(), execution_tracker.clone()).await?,
        );

        info!("Orchestrated tool adapter initialized");

        Ok(Self {
            tool_registry,
            orchestrated_executor,
            execution_tracker,
        })
    }

    /// List all available tools including orchestration tools
    pub async fn list_tools(
        &self,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Vec<Value>> {
        let mut all_tools = Vec::new();

        // Get regular tools from registry
        let local_tools = self.tool_registry.list().await;

        for tool in local_tools {
            if is_tool_blocked(&tool.name) {
                continue;
            }
            if !matches_tool_filter(&tool.name) {
                continue;
            }
            all_tools.push(json!({
                "name": tool.name,
                "description": tool.description,
                "inputSchema": tool.input_schema
            }));
        }

        // Add orchestration tools
        all_tools.extend(self.get_orchestration_tools().await);

        // Sort for consistent ordering
        all_tools.sort_by(|a, b| {
            let name_a = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let name_b = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
            name_a.cmp(name_b)
        });

        // Apply pagination
        let offset = offset.unwrap_or(0);
        if offset > 0 || limit.is_some() {
            let end = limit.map(|l| offset + l).unwrap_or(all_tools.len());
            all_tools = all_tools.into_iter().skip(offset).take(end - offset).collect();
        }

        Ok(all_tools)
    }

    /// Get orchestration tools (workstacks, skills, workflows)
    async fn get_orchestration_tools(&self) -> Vec<Value> {
        let mut tools = Vec::new();

        // Add workstack tools
        let workstack_registry = self.orchestrated_executor.workstack_registry().read().await;
        for workstack in workstack_registry.list() {
            tools.push(json!({
                "name": format!("workstack_{}", workstack.id),
                "description": format!("[Workstack] {}: {}", workstack.name, workstack.description),
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "arguments": {
                            "type": "string",
                            "description": "Arguments/context for the workstack"
                        },
                        "skip_phases": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Phases to skip"
                        }
                    },
                    "required": ["arguments"]
                }
            }));
        }

        // Add skill tools
        let skill_registry = self.orchestrated_executor.skill_registry().read().await;
        for skill in skill_registry.list() {
            tools.push(json!({
                "name": format!("skill_{}", skill.name),
                "description": format!("[Skill] {}: {}", skill.name, skill.metadata.description),
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "tool": {
                            "type": "string",
                            "description": "Tool to execute with this skill activated"
                        },
                        "arguments": {
                            "type": "object",
                            "description": "Arguments for the tool"
                        }
                    },
                    "required": ["tool", "arguments"]
                }
            }));
        }

        tools
    }

    /// Execute tool with orchestration support
    pub async fn execute_tool(
        &self,
        name: &str,
        arguments: Value,
        session_id: Option<String>,
    ) -> Result<Value> {
        // Check blocklist
        if is_tool_blocked(name) {
            warn!("Blocked attempt to execute restricted tool: {}", name);
            return Ok(json!({
                "content": [{
                    "type": "text",
                    "text": format!("Tool '{}' is not available via MCP for security reasons.", name)
                }],
                "isError": true
            }));
        }

        // Check filter
        if !matches_tool_filter(name) {
            return Ok(json!({
                "content": [{
                    "type": "text",
                    "text": format!("Tool '{}' is not available in this MCP instance (filtered).", name)
                }],
                "isError": true
            }));
        }

        info!(tool = %name, "Executing tool via orchestrated executor");

        // Execute via orchestrated executor (handles workstacks, skills, workflows, direct)
        let result = self
            .orchestrated_executor
            .execute(name, arguments, session_id)
            .await?;

        // Convert to MCP format
        self.orchestrated_result_to_mcp(result)
    }

    /// Convert orchestrated result to MCP format
    fn orchestrated_result_to_mcp(&self, result: OrchestratedResult) -> Result<Value> {
        let mode_str = match &result.mode {
            ExecutionMode::Direct { tool_name } => format!("direct:{}", tool_name),
            ExecutionMode::Workstack { workstack_id } => format!("workstack:{}", workstack_id),
            ExecutionMode::Skill { skill_name } => format!("skill:{}", skill_name),
            ExecutionMode::MultiAgent { agents } => format!("multi_agent:{}", agents.join(",")),
            ExecutionMode::Workflow { workflow_id } => format!("workflow:{}", workflow_id),
        };

        if result.success {
            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": result.content.to_string()
                }],
                "execution_id": result.execution_id,
                "duration_ms": result.duration_ms,
                "mode": mode_str,
                "skills_activated": result.skills_activated,
                "agents_involved": result.agents_involved,
                "trace": result.trace,
            }))
        } else {
            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": result.content.to_string()
                }],
                "isError": true,
                "execution_id": result.execution_id,
                "duration_ms": result.duration_ms,
                "mode": mode_str,
            }))
        }
    }

    /// Register a workflow
    pub async fn register_workflow(&self, workflow: Workflow) {
        self.orchestrated_executor.register_workflow(workflow).await;
    }

    /// Get execution tracker
    pub fn execution_tracker(&self) -> &Arc<ExecutionTracker> {
        &self.execution_tracker
    }

    /// Get tool registry
    pub fn tool_registry(&self) -> &Arc<ToolRegistry> {
        &self.tool_registry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orchestration_tool_detection() {
        assert!(is_orchestration_tool("skill_python_debugging"));
        assert!(is_orchestration_tool("workstack_full_stack_feature"));
        assert!(is_orchestration_tool("workflow_deploy_production"));
        assert!(!is_orchestration_tool("ovs_list_bridges"));
        assert!(!is_orchestration_tool("agent_python_pro"));
    }

    #[test]
    fn test_blocked_patterns() {
        assert!(is_tool_blocked("shell_execute"));
        assert!(is_tool_blocked("systemd_start"));
        assert!(!is_tool_blocked("systemd_status"));
        assert!(!is_tool_blocked("ovs_list_bridges"));
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/tool_adapter.rs">
//! Tool Adapter - Bridges op-tools and external MCPs to MCP protocol
//!
//! Aggregates tools from:
//! - External MCP servers (GitHub, filesystem, etc.)
//! - Local op-tools (filtered for safety)
//!
//! SECURITY: System commands (shell_execute, systemd_*, ovs_*, etc.) are
//! NOT exposed via MCP. Use the web interface for system operations.

use anyhow::Result;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;

use crate::external_client::{ExternalMcpManager, ExternalTool};
use op_core::{ToolDefinition, ToolRequest};
use op_tools::ToolRegistry;
use op_execution_tracker::{ExecutionContext, ExecutionResult, ExecutionStatus, ExecutionTracker};
use op_dynamic_loader::{ExecutionAwareLoader, SmartLoadingStrategy};

/// Patterns that block tools from being exposed via MCP.
/// Uses substring matching - if the tool name contains any of these patterns, it's blocked.
const BLOCKED_PATTERNS: &[&str] = &[
// Shell/Execution
"shell_execute",
"write_file",
// Systemd mutations
"systemd_start",
"systemd_stop",
"systemd_restart",
"systemd_reload",
"systemd_enable",
"systemd_disable",
"systemd_apply",
// OVS mutations
"ovs_create",
"ovs_delete",
"ovs_add",
"ovs_set",
// Plugin mutations (matches any *_apply pattern)
"_apply",
// BTRFS mutations
"btrfs_create",
"btrfs_delete",
"btrfs_snapshot",
];

/// Check if a tool name should be blocked from MCP exposure
fn is_tool_blocked(name: &str) -> bool {
BLOCKED_PATTERNS
.iter()
.any(|pattern| name.contains(pattern))
}

fn is_orchestration_tool(name: &str) -> bool {
name.starts_with("skill_") || name.starts_with("workstack_") || name.starts_with("workflow_")
}

/// Check if a tool should be included based on MCP_TOOL_FILTER environment variable
/// Returns true if tool should be included, false if filtered out
fn matches_tool_filter(name: &str) -> bool {
let filter = std::env::var("MCP_TOOL_FILTER").ok().as_deref();

match filter {
Some("systemd") => name.starts_with("dbus_systemd1_"),
Some("login") => name.starts_with("dbus_login1_"),
Some("ovs") => name.starts_with("ovs_"),
Some("agents") => name.starts_with("agent_") || name.starts_with("list_") || name.starts_with("spawn_") || name.contains("agent"),
Some("core") => name.starts_with("dbus_DBus_") || name.starts_with("dbus_login1_") || name.starts_with("ovs_") || name.starts_with("plugin_"),
Some("skills") => is_orchestration_tool(name),
        Some(unknown) => {
            tracing::warn!("Unknown MCP_TOOL_FILTER value: '{}'. Including all tools.", unknown);
            true // Default to include all for unknown filters
        }
    }
}

/// Tool Adapter - Unified interface for all tools
pub struct ToolAdapter {
    tool_registry: Arc<ToolRegistry>,
    external_mcp: Arc<ExternalMcpManager>,
    execution_tracker: Option<Arc<ExecutionTracker>>,
    dynamic_loader: Option<Arc<ExecutionAwareLoader>>,
}

impl ToolAdapter {
    /// Create new tool adapter
    pub async fn new() -> Result<Self> {
        let tool_registry = Arc::new(ToolRegistry::new());
        let external_mcp = Arc::new(ExternalMcpManager::new());

        tracing::info!("Tool adapter initialized");

        Ok(Self {
            tool_registry,
            external_mcp,
            execution_tracker: None,
            dynamic_loader: None,
        })
    }

    /// Create with a shared tool registry
    pub async fn with_registry(registry: Arc<ToolRegistry>) -> Result<Self> {
        let external_mcp = Arc::new(ExternalMcpManager::new());

        tracing::info!("Tool adapter initialized with shared registry");

        Ok(Self {
            tool_registry: registry,
            external_mcp,
            execution_tracker: None,
            dynamic_loader: None,
        })
    }

    /// Create with execution tracking enabled
    pub async fn with_execution_tracking(
        registry: Arc<ToolRegistry>,
        execution_tracker: Arc<ExecutionTracker>,
    ) -> Result<Self> {
        let external_mcp = Arc::new(ExternalMcpManager::new());

        tracing::info!("Tool adapter initialized with execution tracking");

        Ok(Self {
            tool_registry: registry,
            external_mcp,
    /// Create with dynamic loading enabled
    pub async fn with_dynamic_loading(
        registry: Arc<ToolRegistry>,
        execution_tracker: Arc<ExecutionTracker>,
        dynamic_loader: Arc<ExecutionAwareLoader>,
    ) -> Result<Self> {
        let external_mcp = Arc::new(ExternalMcpManager::new());

        tracing::info!("Tool adapter initialized with dynamic loading and execution tracking");

        Ok(Self {
            tool_registry: registry,
            external_mcp,
            execution_tracker: Some(execution_tracker),
            dynamic_loader: Some(dynamic_loader),
        })
    }
            execution_tracker: Some(execution_tracker),
            dynamic_loader: None,
        })
    }

    /// Create with external MCP configuration
    pub async fn with_external_mcps(mcp_config_path: Option<&str>) -> Result<Self> {
        let adapter = Self::new().await?;

        if let Some(path) = mcp_config_path {
            tracing::info!("Loading external MCP servers from: {}", path);
            adapter.external_mcp.load_from_file(path).await?;
        }

        Ok(adapter)
    }

    /// Create with both shared registry and external MCPs
    pub async fn with_registry_and_external_mcps(
        registry: Arc<ToolRegistry>,
        mcp_config_path: Option<&str>,
    ) -> Result<Self> {
        let external_mcp = Arc::new(ExternalMcpManager::new());

        if let Some(path) = mcp_config_path {
            tracing::info!("Loading external MCP servers from: {}", path);
            external_mcp.load_from_file(path).await?;
        }

        tracing::info!("Tool adapter initialized with shared registry and external MCPs");

        Ok(Self {
            tool_registry: registry,
            external_mcp,
            execution_tracker: None,
            dynamic_loader: None,
        })
    }

    /// Add external MCP server at runtime
    pub async fn add_external_mcp(
        &self,
        config: crate::external_client::ExternalMcpConfig,
    ) -> Result<()> {
        self.external_mcp.add_server(config).await
    }

    /// List all available tools in MCP format (filtered local + external)
    pub async fn list_tools(
        &self,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Vec<Value>> {
        let mut all_tools = Vec::new();
        let mut blocked_count = 0;
        let mut filtered_count = 0;

        // Get local tools from op-tools registry (with dynamic loading if available)
        let local_tools = if let Some(loader) = &self.dynamic_loader {
            tracing::debug!("Using dynamic loader for tool listing");
            loader.list_tools_with_dynamic_loading().await
        } else {
            tracing::debug!("Using direct registry for tool listing");
            self.tool_registry.list().await
        };
        let local_total = local_tools.len();

        // Log filter status
        if let Ok(filter) = std::env::var("MCP_TOOL_FILTER") {
            tracing::info!("MCP_TOOL_FILTER active: {}", filter);
        }

        // Collect all allowed tools first
        let mut allowed_tools = Vec::new();
        for tool in local_tools {
            if is_tool_blocked(&tool.name) {
                blocked_count += 1;
                tracing::trace!("Blocking tool from MCP: {}", tool.name);
            } else if !matches_tool_filter(&tool.name) {
                filtered_count += 1;
                tracing::trace!("Filtering tool from MCP: {}", tool.name);
            } else {
                allowed_tools.push(self.tool_definition_to_mcp(&tool));
            }
        }

        let local_allowed = local_total - blocked_count - filtered_count;

        // Get tools from external MCP servers
        let external_tools = self.external_mcp.get_all_tools().await;
        let mut external_allowed = 0;

        for tool in external_tools {
            if matches_tool_filter(&tool.name) {
                allowed_tools.push(self.external_tool_to_mcp(tool));
                external_allowed += 1;
            } else {
                filtered_count += 1;
                tracing::trace!("Filtering external tool from MCP: {}", tool.name);
            }
        }

        // Sort tools by name for consistent chunking across instances
        allowed_tools.sort_by(|a, b| {
            let name_a = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let name_b = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
            name_a.cmp(name_b)
        });

        // Apply chunking if offset and/or limit are set
        let offset = offset.unwrap_or(0);

        tracing::debug!(
            "Chunking check: offset={:?}, limit={:?}, total_tools={}",
            offset,
            limit,
            allowed_tools.len()
        );

        if offset > 0 || limit.is_some() {
            let end = limit.map(|l| offset + l).unwrap_or(allowed_tools.len());
            let before_count = allowed_tools.len();
            all_tools = allowed_tools
                .into_iter()
                .skip(offset)
                .take(end - offset)
                .collect();
            tracing::info!(
                "MCP tool chunking: offset={}, limit={:?}, showing tools {}-{} of {} (reduced from {} to {})",
                offset,
                limit,
                offset,
                all_tools.len() + offset,
                local_allowed + external_allowed,
                before_count,
                all_tools.len()
            );
        } else {
            all_tools = allowed_tools;
        }

        tracing::info!(
            "MCP tools: {} total ({} local allowed, {} blocked, {} filtered, {} external)",
            all_tools.len(),
            local_allowed,
            blocked_count,
            filtered_count,
            external_allowed
        );

        Ok(all_tools)
    }

    /// Execute tool and return MCP-formatted result
    pub async fn execute_tool(&self, name: &str, arguments: Value) -> Result<Value> {
        // Check if this is an external MCP tool (contains ':')
        if name.contains(':') {
            tracing::debug!("Executing external MCP tool: {}", name);
            return self.external_mcp.call_tool(name, arguments).await;
        }

        // Check blocklist before executing local tools
        if is_tool_blocked(name) {
            tracing::warn!(
                "Blocked attempt to execute restricted tool via MCP: {}",
                name
            );
            return Ok(json!({
                "content": [{
                    "type": "text",
                    "text": format!("Tool '{}' is not available via MCP for security reasons. Use the web interface for system operations.", name)
                }],
                "isError": true
            }));
        }

        // Check filter before executing local tools
        if !matches_tool_filter(name) {
            tracing::warn!("Filtered attempt to execute tool via MCP: {}", name);
            return Ok(json!({
                "content": [{
                    "type": "text",
                    "text": format!("Tool '{}' is not available in this MCP instance (filtered). Try a different MCP endpoint.", name)
                }],
                "isError": true
            }));
        }

        // Create execution context if tracking is enabled
        let execution_id = if let Some(tracker) = &self.execution_tracker {
            let context = ExecutionContext::new(name);
            let exec_id = tracker.track_execution(context).await?;
            tracker.update_status(&exec_id, ExecutionStatus::Dispatched).await?;
            Some(exec_id)
        } else {
            None
        };

        // Execute via dynamic loader if available, otherwise use direct registry
        tracing::debug!("Executing local tool: {}", name);

        let start_time = Utc::now();
        let request = ToolRequest::new(name, arguments);

        let result = if let Some(loader) = &self.dynamic_loader {
            tracing::debug!("Using dynamic loader for tool execution: {}", name);
            loader.execute_with_dynamic_loading(request, self.tool_registry.clone()).await
        } else {
            tracing::debug!("Using direct registry execution for tool: {}", name);
            self.tool_registry.execute(request).await
        };

        let end_time = Utc::now();
        let duration_ms = (end_time - start_time).num_milliseconds() as u64;

        // Update execution tracking if enabled
        if let Some(exec_id) = execution_id {
            if let Some(tracker) = &self.execution_tracker {
                let execution_result = ExecutionResult {
                    success: result.success,
                    result: Some(simd_json::json!({
                        "content": result.content.to_string(),
                        "duration_ms": duration_ms,
                    })),
                    error: result.error,
                    duration_ms,
                    finished_at: end_time,
                };

                tracker.complete_execution(&exec_id, execution_result).await?;
            }
        }

        // Convert ToolResult to MCP format
        if result.success {
            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": result.content.to_string()
                }],
                "execution_id": execution_id,
                "duration_ms": duration_ms,
            }))
        } else {
            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": result.error.unwrap_or_else(|| "Unknown error".to_string())
                }],
                "isError": true,
                "execution_id": execution_id,
                "duration_ms": duration_ms,
            }))
        }
    }

    /// Convert ToolDefinition to MCP format
    fn tool_definition_to_mcp(&self, tool: &ToolDefinition) -> Value {
        json!({
            "name": tool.name,
            "description": tool.description,
            "inputSchema": tool.input_schema
        })
    }

    /// Convert external tool to MCP format
    fn external_tool_to_mcp(&self, tool: ExternalTool) -> Value {
        json!({
            "name": tool.name,
            "description": tool.description,
            "inputSchema": tool.input_schema
        })
    }

    /// Get external MCP manager (for advanced operations)
    pub fn external_mcp_manager(&self) -> Arc<ExternalMcpManager> {
        self.external_mcp.clone()
    }

    /// Get tool registry (for advanced operations)
    /// Get dynamic loader (if enabled)
    pub fn dynamic_loader(&self) -> Option<Arc<ExecutionAwareLoader>> {
        self.dynamic_loader.clone()
    }
    pub fn tool_registry(&self) -> Arc<ToolRegistry> {
        self.tool_registry.clone()
    }

    /// Get execution tracker (if enabled)
    pub fn execution_tracker(&self) -> Option<Arc<ExecutionTracker>> {
        self.execution_tracker.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blocked_patterns() {
        // Shell/execution
        assert!(is_tool_blocked("shell_execute"));
        assert!(is_tool_blocked("write_file"));

        // Systemd mutations
        assert!(is_tool_blocked("systemd_start"));
        assert!(is_tool_blocked("systemd_stop"));
        assert!(is_tool_blocked("systemd_restart"));
        assert!(is_tool_blocked("systemd_reload"));
        assert!(is_tool_blocked("systemd_enable"));
        assert!(is_tool_blocked("systemd_disable"));
        assert!(is_tool_blocked("systemd_apply"));

        // OVS mutations
        assert!(is_tool_blocked("ovs_create_bridge"));
        assert!(is_tool_blocked("ovs_delete_port"));
        assert!(is_tool_blocked("ovs_add_port"));
        assert!(is_tool_blocked("ovs_set_controller"));

        // Plugin apply patterns
        assert!(is_tool_blocked("network_apply"));
        assert!(is_tool_blocked("plugin_apply"));

        // BTRFS mutations
        assert!(is_tool_blocked("btrfs_create_subvolume"));
        assert!(is_tool_blocked("btrfs_delete_snapshot"));
        assert!(is_tool_blocked("btrfs_snapshot"));
    }

    #[test]
    fn test_allowed_tools() {
        // Read operations should be allowed
        assert!(!is_tool_blocked("systemd_status"));
        assert!(!is_tool_blocked("systemd_list"));
        assert!(!is_tool_blocked("ovs_list_bridges"));
        assert!(!is_tool_blocked("ovs_list_ports"));
        assert!(!is_tool_blocked("read_file"));
        assert!(!is_tool_blocked("btrfs_list"));
        assert!(!is_tool_blocked("btrfs_info"));

        // Agent tools should be allowed
        assert!(!is_tool_blocked("agent_.list"));
        assert!(!is_tool_blocked("agent_status"));

        // Response tools should be allowed
        assert!(!is_tool_blocked("respond_to_user"));
        assert!(!is_tool_blocked("cannot_perform"));
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/tool_adapter.rs.backup">
1 | //! Tool Adapter - Bridges op-tools and external MCPs to MCP protocol
  2 | //!
  3 | //! Aggregates tools from:
  4 | //! - External MCP servers (GitHub, filesystem, etc.)
  5 | //! - Local op-tools (filtered for safety)
  6 | //!
  7 | //! SECURITY: System commands (shell_execute, systemd_*, ovs_*, etc.) are
  8 | //! NOT exposed via MCP. Use the web interface for system operations.
  9 |
 10 | use anyhow::Result;
 11 | use serde_json::{json, Value};
 12 | use std::sync::Arc;
 13 |
 14 | use crate::external_client::{ExternalMcpManager, ExternalTool};
 15 | use op_core::{ToolDefinition, ToolRequest};
 16 | use op_tools::ToolRegistry;
use op_execution_tracker::{ExecutionContext, ExecutionResult, ExecutionStatus, ExecutionTracker};
use op_dynamic_loader::{ExecutionAwareLoader, SmartLoadingStrategy};
 17 |
 18 | /// Patterns that block tools from being exposed via MCP.
 19 | /// Uses substring matching - if the tool name contains any of these patterns, it's blocked.
 20 | const BLOCKED_PATTERNS: &[&str] = &[
 21 |     // Shell/Execution
 22 |     "shell_execute",
 23 |     "write_file",
 24 |     // Systemd mutations
 25 |     "systemd_start",
 26 |     "systemd_stop",
 27 |     "systemd_restart",
 28 |     "systemd_reload",
 29 |     "systemd_enable",
 30 |     "systemd_disable",
 31 |     "systemd_apply",
 32 |     // OVS mutations
 33 |     "ovs_create",
 34 |     "ovs_delete",
 35 |     "ovs_add",
 36 |     "ovs_set",
 37 |     // Plugin mutations (matches any *_apply pattern)
 38 |     "_apply",
 39 |     // BTRFS mutations
 40 |     "btrfs_create",
 41 |     "btrfs_delete",
 42 |     "btrfs_snapshot",
 43 | ];
 44 |
 45 | /// Check if a tool name should be blocked from MCP exposure
 46 | fn is_tool_blocked(name: &str) -> bool {
 47 |     BLOCKED_PATTERNS
 48 |         .iter()
 49 |         .any(|pattern| name.contains(pattern))
 50 | }
 51 |
 52 | fn is_orchestration_tool(name: &str) -> bool {
 53 |     name.starts_with("skill_") || name.starts_with("workstack_") || name.starts_with("workflow_")
 54 | }
 55 |
 56 | /// Check if a tool should be included based on MCP_TOOL_FILTER environment variable
 57 | /// Returns true if tool should be included, false if filtered out
 58 | fn matches_tool_filter(name: &str) -> bool {
 59 |     let filter = std::env::var("MCP_TOOL_FILTER").ok().as_deref();
 60 |
 61 |     match filter {
 62 |         Some("systemd") => name.starts_with("dbus_systemd1_"),
 63 |         Some("login") => name.starts_with("dbus_login1_"),
 64 |         Some("ovs") => name.starts_with("ovs_"),
 65 |         Some("agents") => name.starts_with("agent_") || name.starts_with("list_") || name.starts_with("spawn_") || name.contains("agent"),
 66 |         Some("core") => name.starts_with("dbus_DBus_") || name.starts_with("dbus_login1_") || name.starts_with("ovs_") || name.starts_with("plugin_"),
 67 |         Some("skills") => is_orchestration_tool(name),
        Some(unknown) => {
            tracing::warn!("Unknown MCP_TOOL_FILTER value: '{}'. Including all tools.", unknown);
            true // Default to include all for unknown filters
        }
    }
}

/// Tool Adapter - Unified interface for all tools
pub struct ToolAdapter {
    tool_registry: Arc<ToolRegistry>,
    external_mcp: Arc<ExternalMcpManager>,
    execution_tracker: Option<Arc<ExecutionTracker>>,
    dynamic_loader: Option<Arc<ExecutionAwareLoader>>,
}

impl ToolAdapter {
    /// Create new tool adapter
    pub async fn new() -> Result<Self> {
        let tool_registry = Arc::new(ToolRegistry::new());
        let external_mcp = Arc::new(ExternalMcpManager::new());

        tracing::info!("Tool adapter initialized");

        Ok(Self {
            tool_registry,
            external_mcp,
            execution_tracker: None,
            dynamic_loader: None,
        })
    }

    /// Create with a shared tool registry
    pub async fn with_registry(registry: Arc<ToolRegistry>) -> Result<Self> {
        let external_mcp = Arc::new(ExternalMcpManager::new());

        tracing::info!("Tool adapter initialized with shared registry");

        Ok(Self {
            tool_registry: registry,
            external_mcp,
            execution_tracker: None,
            dynamic_loader: None,
        })
    }

    /// Create with execution tracking enabled
    pub async fn with_execution_tracking(
        registry: Arc<ToolRegistry>,
        execution_tracker: Arc<ExecutionTracker>,
    ) -> Result<Self> {
        let external_mcp = Arc::new(ExternalMcpManager::new());

        tracing::info!("Tool adapter initialized with execution tracking");

        Ok(Self {
            tool_registry: registry,
            external_mcp,
    /// Create with dynamic loading enabled
    pub async fn with_dynamic_loading(
        registry: Arc<ToolRegistry>,
        execution_tracker: Arc<ExecutionTracker>,
        dynamic_loader: Arc<ExecutionAwareLoader>,
    ) -> Result<Self> {
        let external_mcp = Arc::new(ExternalMcpManager::new());

        tracing::info!("Tool adapter initialized with dynamic loading and execution tracking");

        Ok(Self {
            tool_registry: registry,
            external_mcp,
            execution_tracker: Some(execution_tracker),
            dynamic_loader: Some(dynamic_loader),
        })
    }
            execution_tracker: Some(execution_tracker),
            dynamic_loader: None,
        })
    }

    /// Create with external MCP configuration
    pub async fn with_external_mcps(mcp_config_path: Option<&str>) -> Result<Self> {
        let adapter = Self::new().await?;

        if let Some(path) = mcp_config_path {
            tracing::info!("Loading external MCP servers from: {}", path);
            adapter.external_mcp.load_from_file(path).await?;
        }

        Ok(adapter)
    }

    /// Create with both shared registry and external MCPs
    pub async fn with_registry_and_external_mcps(
        registry: Arc<ToolRegistry>,
        mcp_config_path: Option<&str>,
    ) -> Result<Self> {
        let external_mcp = Arc::new(ExternalMcpManager::new());

        if let Some(path) = mcp_config_path {
            tracing::info!("Loading external MCP servers from: {}", path);
            external_mcp.load_from_file(path).await?;
        }

        tracing::info!("Tool adapter initialized with shared registry and external MCPs");

        Ok(Self {
            tool_registry: registry,
            external_mcp,
            execution_tracker: None,
            dynamic_loader: None,
        })
    }

    /// Add external MCP server at runtime
    pub async fn add_external_mcp(
        &self,
        config: crate::external_client::ExternalMcpConfig,
    ) -> Result<()> {
        self.external_mcp.add_server(config).await
    }

    /// List all available tools in MCP format (filtered local + external)
    pub async fn list_tools(
        &self,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Vec<Value>> {
        let mut all_tools = Vec::new();
        let mut blocked_count = 0;
        let mut filtered_count = 0;

        // Get local tools from op-tools registry (with dynamic loading if available)
        let local_tools = if let Some(loader) = &self.dynamic_loader {
            tracing::debug!("Using dynamic loader for tool listing");
            loader.list_tools_with_dynamic_loading().await
        } else {
            tracing::debug!("Using direct registry for tool listing");
            self.tool_registry.list().await
        };
        let local_total = local_tools.len();

        // Log filter status
        if let Ok(filter) = std::env::var("MCP_TOOL_FILTER") {
            tracing::info!("MCP_TOOL_FILTER active: {}", filter);
        }

        // Collect all allowed tools first
        let mut allowed_tools = Vec::new();
        for tool in local_tools {
            if is_tool_blocked(&tool.name) {
                blocked_count += 1;
                tracing::trace!("Blocking tool from MCP: {}", tool.name);
            } else if !matches_tool_filter(&tool.name) {
                filtered_count += 1;
                tracing::trace!("Filtering tool from MCP: {}", tool.name);
            } else {
                allowed_tools.push(self.tool_definition_to_mcp(&tool));
            }
        }

        let local_allowed = local_total - blocked_count - filtered_count;

        // Get tools from external MCP servers
        let external_tools = self.external_mcp.get_all_tools().await;
        let mut external_allowed = 0;

        for tool in external_tools {
            if matches_tool_filter(&tool.name) {
                allowed_tools.push(self.external_tool_to_mcp(tool));
                external_allowed += 1;
            } else {
                filtered_count += 1;
                tracing::trace!("Filtering external tool from MCP: {}", tool.name);
            }
        }

        // Sort tools by name for consistent chunking across instances
        allowed_tools.sort_by(|a, b| {
            let name_a = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let name_b = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
            name_a.cmp(name_b)
        });

        // Apply chunking if offset and/or limit are set
        let offset = offset.unwrap_or(0);

        tracing::debug!(
            "Chunking check: offset={:?}, limit={:?}, total_tools={}",
            offset,
            limit,
            allowed_tools.len()
        );

        if offset > 0 || limit.is_some() {
            let end = limit.map(|l| offset + l).unwrap_or(allowed_tools.len());
            let before_count = allowed_tools.len();
            all_tools = allowed_tools
                .into_iter()
                .skip(offset)
                .take(end - offset)
                .collect();
            tracing::info!(
                "MCP tool chunking: offset={}, limit={:?}, showing tools {}-{} of {} (reduced from {} to {})",
                offset,
                limit,
                offset,
                all_tools.len() + offset,
                local_allowed + external_allowed,
                before_count,
                all_tools.len()
            );
        } else {
            all_tools = allowed_tools;
        }

        tracing::info!(
            "MCP tools: {} total ({} local allowed, {} blocked, {} filtered, {} external)",
            all_tools.len(),
            local_allowed,
            blocked_count,
            filtered_count,
            external_allowed
        );

        Ok(all_tools)
    }

    /// Execute tool and return MCP-formatted result
    pub async fn execute_tool(&self, name: &str, arguments: Value) -> Result<Value> {
        // Check if this is an external MCP tool (contains ':')
        if name.contains(':') {
            tracing::debug!("Executing external MCP tool: {}", name);
            return self.external_mcp.call_tool(name, arguments).await;
        }

        // Check blocklist before executing local tools
        if is_tool_blocked(name) {
            tracing::warn!(
                "Blocked attempt to execute restricted tool via MCP: {}",
                name
            );
            return Ok(json!({
                "content": [{
                    "type": "text",
                    "text": format!("Tool '{}' is not available via MCP for security reasons. Use the web interface for system operations.", name)
                }],
                "isError": true
            }));
        }

        // Check filter before executing local tools
        if !matches_tool_filter(name) {
            tracing::warn!("Filtered attempt to execute tool via MCP: {}", name);
            return Ok(json!({
                "content": [{
                    "type": "text",
                    "text": format!("Tool '{}' is not available in this MCP instance (filtered). Try a different MCP endpoint.", name)
                }],
                "isError": true
            }));
        }

        // Create execution context if tracking is enabled
        let execution_id = if let Some(tracker) = &self.execution_tracker {
            let context = ExecutionContext::new(name);
            let exec_id = tracker.track_execution(context).await?;
            tracker.update_status(&exec_id, ExecutionStatus::Dispatched).await?;
            Some(exec_id)
        } else {
            None
        };

        // Execute via dynamic loader if available, otherwise use direct registry
        tracing::debug!("Executing local tool: {}", name);

        let start_time = Utc::now();
        let request = ToolRequest::new(name, arguments);

        let result = if let Some(loader) = &self.dynamic_loader {
            tracing::debug!("Using dynamic loader for tool execution: {}", name);
            loader.execute_with_dynamic_loading(request, self.tool_registry.clone()).await
        } else {
            tracing::debug!("Using direct registry execution for tool: {}", name);
            self.tool_registry.execute(request).await
        };

        let end_time = Utc::now();
        let duration_ms = (end_time - start_time).num_milliseconds() as u64;

        // Update execution tracking if enabled
        if let Some(exec_id) = execution_id {
            if let Some(tracker) = &self.execution_tracker {
                let execution_result = ExecutionResult {
                    success: result.success,
                    result: Some(serde_json::json!({
                        "content": result.content.to_string(),
                        "duration_ms": duration_ms,
                    })),
                    error: result.error,
                    duration_ms,
                    finished_at: end_time,
                };

                tracker.complete_execution(&exec_id, execution_result).await?;
            }
        }

        // Convert ToolResult to MCP format
        if result.success {
            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": result.content.to_string()
                }],
                "execution_id": execution_id,
                "duration_ms": duration_ms,
            }))
        } else {
            Ok(json!({
                "content": [{
                    "type": "text",
                    "text": result.error.unwrap_or_else(|| "Unknown error".to_string())
                }],
                "isError": true,
                "execution_id": execution_id,
                "duration_ms": duration_ms,
            }))
        }
    }

    /// Convert ToolDefinition to MCP format
    fn tool_definition_to_mcp(&self, tool: &ToolDefinition) -> Value {
        json!({
            "name": tool.name,
            "description": tool.description,
            "inputSchema": tool.input_schema
        })
    }

    /// Convert external tool to MCP format
    fn external_tool_to_mcp(&self, tool: ExternalTool) -> Value {
        json!({
            "name": tool.name,
            "description": tool.description,
            "inputSchema": tool.input_schema
        })
    }

    /// Get external MCP manager (for advanced operations)
    pub fn external_mcp_manager(&self) -> Arc<ExternalMcpManager> {
        self.external_mcp.clone()
    }

    /// Get tool registry (for advanced operations)
    /// Get dynamic loader (if enabled)
    pub fn dynamic_loader(&self) -> Option<Arc<ExecutionAwareLoader>> {
        self.dynamic_loader.clone()
    }
    pub fn tool_registry(&self) -> Arc<ToolRegistry> {
        self.tool_registry.clone()
    }

    /// Get execution tracker (if enabled)
    pub fn execution_tracker(&self) -> Option<Arc<ExecutionTracker>> {
        self.execution_tracker.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blocked_patterns() {
        // Shell/execution
        assert!(is_tool_blocked("shell_execute"));
        assert!(is_tool_blocked("write_file"));

        // Systemd mutations
        assert!(is_tool_blocked("systemd_start"));
        assert!(is_tool_blocked("systemd_stop"));
        assert!(is_tool_blocked("systemd_restart"));
        assert!(is_tool_blocked("systemd_reload"));
        assert!(is_tool_blocked("systemd_enable"));
        assert!(is_tool_blocked("systemd_disable"));
        assert!(is_tool_blocked("systemd_apply"));

        // OVS mutations
        assert!(is_tool_blocked("ovs_create_bridge"));
        assert!(is_tool_blocked("ovs_delete_port"));
        assert!(is_tool_blocked("ovs_add_port"));
        assert!(is_tool_blocked("ovs_set_controller"));

        // Plugin apply patterns
        assert!(is_tool_blocked("network_apply"));
        assert!(is_tool_blocked("plugin_apply"));

        // BTRFS mutations
        assert!(is_tool_blocked("btrfs_create_subvolume"));
        assert!(is_tool_blocked("btrfs_delete_snapshot"));
        assert!(is_tool_blocked("btrfs_snapshot"));
    }

    #[test]
    fn test_allowed_tools() {
        // Read operations should be allowed
        assert!(!is_tool_blocked("systemd_status"));
        assert!(!is_tool_blocked("systemd_list"));
        assert!(!is_tool_blocked("ovs_list_bridges"));
        assert!(!is_tool_blocked("ovs_list_ports"));
        assert!(!is_tool_blocked("read_file"));
        assert!(!is_tool_blocked("btrfs_list"));
        assert!(!is_tool_blocked("btrfs_info"));

        // Agent tools should be allowed
        assert!(!is_tool_blocked("agent_.list"));
        assert!(!is_tool_blocked("agent_status"));

        // Response tools should be allowed
        assert!(!is_tool_blocked("respond_to_user"));
        assert!(!is_tool_blocked("cannot_perform"));
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/tool_registry.rs">
//! Tool Registry - All Tools Always Loaded
//!
//! This replaces lazy_tools.rs with a simple registry that:
//! - Loads ALL tools at startup
//! - Never evicts tools
//! - Provides fast lookup for execute_tool
//!
//! The compact mode meta-tools use this registry to:
//! - list_tools: Paginate through all registered tools
//! - search_tools: Filter by name/description/category
//! - get_tool_schema: Return input schema for a tool
//! - execute_tool: Look up and execute any tool

use anyhow::Result;
use async_trait::async_trait;
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

use crate::server::{ToolExecutor, ToolInfo};
use op_core::ToolDefinition;

/// Tool trait - same as op_tools::Tool but standalone
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> Value;
    fn category(&self) -> &str {
        "general"
    }
    fn namespace(&self) -> &str {
        "system"
    }
    fn tags(&self) -> Vec<String> {
        vec![]
    }
    async fn execute(&self, input: Value) -> Result<Value>;
}

pub type BoxedTool = Arc<dyn Tool>;

/// Simple tool registry - NO eviction, all tools always available
pub struct ToolRegistry {
    tools: RwLock<HashMap<String, BoxedTool>>,
    definitions: RwLock<HashMap<String, ToolDefinition>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
            definitions: RwLock::new(HashMap::new()),
        }
    }

    /// Register a tool (never evicted)
    pub async fn register(&self, tool: BoxedTool) -> Result<()> {
        let name = tool.name().to_string();
        let definition = ToolDefinition {
            name: name.clone(),
            description: tool.description().to_string(),
            input_schema: tool.input_schema(),
            schema_version: String::new(),
            category: tool.category().to_string(),
            tags: tool.tags(),
            namespace: tool.namespace().to_string(),
        };

        self.tools.write().await.insert(name.clone(), tool);
        self.definitions
            .write()
            .await
            .insert(name.clone(), definition);

        debug!("Registered tool: {}", name);
        Ok(())
    }

    /// Get a tool by name (instant lookup, no loading)
    pub async fn get(&self, name: &str) -> Option<BoxedTool> {
        self.tools.read().await.get(name).cloned()
    }

    /// Execute a tool by name
    pub async fn execute(&self, name: &str, input: Value) -> Result<Value> {
        let tool = self
            .get(name)
            .await
            .ok_or_else(|| anyhow::anyhow!("Tool not found: {}", name))?;
        tool.execute(input).await
    }

    /// Get tool definition
    pub async fn get_definition(&self, name: &str) -> Option<ToolDefinition> {
        self.definitions.read().await.get(name).cloned()
    }

    /// List all tools (paginated)
    pub async fn list(
        &self,
        offset: usize,
        limit: usize,
        category: Option<&str>,
    ) -> Vec<ToolDefinition> {
        let defs = self.definitions.read().await;

        let filtered: Vec<_> = defs
            .values()
            .filter(|d| category.map_or(true, |c| d.category == c))
            .cloned()
            .collect();

        filtered.into_iter().skip(offset).take(limit).collect()
    }

    /// Search tools by query
    pub async fn search(&self, query: &str) -> Vec<ToolDefinition> {
        let query_lower = query.to_lowercase();
        let defs = self.definitions.read().await;

        defs.values()
            .filter(|d| {
                d.name.to_lowercase().contains(&query_lower)
                    || d.description.to_lowercase().contains(&query_lower)
                    || d.category.to_lowercase().contains(&query_lower)
                    || d.tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&query_lower))
            })
            .cloned()
            .take(50) // Reasonable limit for search results
            .collect()
    }

    /// Total tool count
    pub async fn count(&self) -> usize {
        self.tools.read().await.len()
    }

    /// Get all categories
    pub async fn categories(&self) -> Vec<String> {
        let defs = self.definitions.read().await;
        let mut cats: Vec<String> = defs
            .values()
            .map(|d| d.category.clone())
            .filter(|c| !c.is_empty())
            .collect();
        cats.sort();
        cats.dedup();
        cats
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Registry executor for CompactServer
pub struct RegistryExecutor {
    registry: Arc<ToolRegistry>,
}

impl RegistryExecutor {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl ToolExecutor for RegistryExecutor {
    async fn execute_tool(&self, tool_name: &str, arguments: Value) -> Result<Value> {
        self.registry.execute(tool_name, arguments).await
    }

    async fn list_tools(&self) -> Result<Vec<ToolInfo>> {
        let tools = self.registry.list(0, 1000, None).await;
        Ok(tools
            .into_iter()
            .map(|t| ToolInfo {
                name: t.name,
                description: t.description,
                input_schema: t.input_schema,
                annotations: None,
            })
            .collect())
    }

    async fn get_tool_schema(&self, name: &str) -> Result<Option<Value>> {
        Ok(self
            .registry
            .get_definition(name)
            .await
            .map(|d| d.input_schema))
    }

    async fn search_tools(&self, query: &str, limit: usize) -> Result<Vec<ToolInfo>> {
        let tools = self.registry.search(query).await;
        Ok(tools
            .into_iter()
            .take(limit)
            .map(|t| ToolInfo {
                name: t.name,
                description: t.description,
                input_schema: t.input_schema,
                annotations: None,
            })
            .collect())
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/src/trait_agent_executor.rs">
//! Trait-based Agent Executor
//!
//! Executes agents using the existing AgentTrait implementations
//! instead of requiring separate D-Bus service processes.
//!
//! This is the recommended executor for production use.

use anyhow::Result;
use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn, error};

use op_agents::agents::base::{AgentTrait, AgentTask, TaskResult};

// Import agent implementations
use op_agents::agents::{
    language::{RustProAgent, PythonProAgent, GolangProAgent, JavaProAgent, JavaScriptProAgent, TypeScriptProAgent},
    architecture::{BackendArchitectAgent, FrontendDeveloperAgent},
    infrastructure::{NetworkEngineerAgent, DeploymentAgent, CloudArchitectAgent},
    orchestration::{MemoryAgent, ContextManagerAgent, SequentialThinkingAgent, Mem0WrapperAgent, DxOptimizerAgent, TddOrchestratorAgent},
    seo::SearchSpecialistAgent,
    analysis::{DebuggerAgent, CodeReviewerAgent},
    aiml::PromptEngineerAgent,
    database::DatabaseArchitectAgent,
    operations::DevOpsTroubleshooterAgent,
    content::DocsArchitectAgent,
};

use super::agents_server::AgentExecutor;

/// Agent entry in the registry
struct AgentEntry {
    agent: Box<dyn AgentTrait + Send + Sync>,
    started: bool,
}

/// Trait-based agent executor
/// 
/// Uses the existing AgentTrait implementations to execute agent operations.
/// No D-Bus services required.
pub struct TraitAgentExecutor {
    agents: Arc<RwLock<HashMap<String, AgentEntry>>>,
}

impl TraitAgentExecutor {
    /// Create a new executor with default agents registered
    pub fn new() -> Self {
        let executor = Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
        };
        
        // Register agents synchronously during construction
        // We'll use a blocking approach since this is initialization
        let agents = executor.agents.clone();
        
        tokio::spawn(async move {
            let mut map = agents.write().await;
            
            // === INSTANT-ON AGENTS (Running at boot - always available) ===
            // These agents are pre-started for instant availability
            Self::register_agent(&mut map, "memory", Box::new(MemoryAgent::new("memory".to_string())));
            Self::register_agent(&mut map, "rust_pro", Box::new(RustProAgent::new("rust_pro".to_string())));
            Self::register_agent(&mut map, "backend_architect", Box::new(BackendArchitectAgent::new("backend_architect".to_string())));
            Self::register_agent(&mut map, "network_engineer", Box::new(NetworkEngineerAgent::new("network_engineer".to_string())));
            Self::register_agent(&mut map, "debugger", Box::new(DebuggerAgent::new("debugger".to_string())));
            Self::register_agent(&mut map, "search_specialist", Box::new(SearchSpecialistAgent::new("search_specialist".to_string())));
            
            // === Orchestration Agents (Critical - always loaded) ===
            Self::register_agent(&mut map, "context_manager", Box::new(ContextManagerAgent::new("context_manager".to_string())));
            Self::register_agent(&mut map, "sequential_thinking", Box::new(SequentialThinkingAgent::new("sequential_thinking".to_string())));
            Self::register_agent(&mut map, "dx_optimizer", Box::new(DxOptimizerAgent::new("dx_optimizer".to_string())));
            Self::register_agent(&mut map, "tdd_orchestrator", Box::new(TddOrchestratorAgent::new("tdd_orchestrator".to_string())));
            
            // === Language Agents (High priority) ===
            Self::register_agent(&mut map, "python_pro", Box::new(PythonProAgent::new("python_pro".to_string())));
            Self::register_agent(&mut map, "golang_pro", Box::new(GolangProAgent::new("golang_pro".to_string())));
            Self::register_agent(&mut map, "java_pro", Box::new(JavaProAgent::new("java_pro".to_string())));
            Self::register_agent(&mut map, "javascript_pro", Box::new(JavaScriptProAgent::new("javascript_pro".to_string())));
            Self::register_agent(&mut map, "typescript_pro", Box::new(TypeScriptProAgent::new("typescript_pro".to_string())));
            
            // === Architecture Agents (High priority) ===
            Self::register_agent(&mut map, "frontend_developer", Box::new(FrontendDeveloperAgent::new("frontend_developer".to_string())));
            Self::register_agent(&mut map, "database_architect", Box::new(DatabaseArchitectAgent::new("database_architect".to_string())));
            
            // === Infrastructure & Ops (Medium priority) ===
            Self::register_agent(&mut map, "deployment", Box::new(DeploymentAgent::new("deployment".to_string())));
            Self::register_agent(&mut map, "cloud_architect", Box::new(CloudArchitectAgent::new("cloud_architect".to_string())));
            Self::register_agent(&mut map, "devops_troubleshooter", Box::new(DevOpsTroubleshooterAgent::new("devops_troubleshooter".to_string())));
            
            // === Analysis & Quality (Medium priority) ===
            Self::register_agent(&mut map, "code_reviewer", Box::new(CodeReviewerAgent::new("code_reviewer".to_string())));
            Self::register_agent(&mut map, "prompt_engineer", Box::new(PromptEngineerAgent::new("prompt_engineer".to_string())));
            Self::register_agent(&mut map, "docs_architect", Box::new(DocsArchitectAgent::new("docs_architect".to_string())));
            
            // === Disabled/Special agents ===
            // mem0 disabled - pending embedder configuration
            // Self::register_agent(&mut map, "mem0", Box::new(Mem0WrapperAgent::new("mem0".to_string())));
            
            info!("TraitAgentExecutor: Registered {} agents (6 instant-on at boot)", map.len());
        });
        
        executor
    }
    
    fn register_agent(
        map: &mut HashMap<String, AgentEntry>,
        id: &str,
        agent: Box<dyn AgentTrait + Send + Sync>,
    ) {
        map.insert(id.to_string(), AgentEntry {
            agent,
            started: false,
        });
    }
    
    /// Register an additional agent at runtime
    pub async fn register(&self, id: &str, agent: Box<dyn AgentTrait + Send + Sync>) {
        let mut agents = self.agents.write().await;
        agents.insert(id.to_string(), AgentEntry {
            agent,
            started: false,
        });
        info!(agent = %id, "Registered agent");
    }
    
    /// List all registered agents
    pub async fn list_agents(&self) -> Vec<String> {
        self.agents.read().await.keys().cloned().collect()
    }
}

impl Default for TraitAgentExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentExecutor for TraitAgentExecutor {
    async fn start_agent(&self, agent_id: &str, _dbus_service: Option<&str>) -> Result<()> {
        let mut agents = self.agents.write().await;
        
        if let Some(entry) = agents.get_mut(agent_id) {
            entry.started = true;
            info!(agent = %agent_id, "✓ Agent started (trait-based)");
            Ok(())
        } else {
            warn!(agent = %agent_id, "Agent not found in registry");
            Err(anyhow::anyhow!("Agent not registered: {}", agent_id))
        }
    }
    
    async fn stop_agent(&self, agent_id: &str) -> Result<()> {
        let mut agents = self.agents.write().await;
        
        if let Some(entry) = agents.get_mut(agent_id) {
            entry.started = false;
            info!(agent = %agent_id, "Agent stopped");
        }
        
        Ok(())
    }
    
    async fn execute(&self, agent_id: &str, operation: &str, args: Value) -> Result<Value> {
        debug!(agent = %agent_id, operation = %operation, "Executing agent");
        
        let agents = self.agents.read().await;
        
        let entry = agents.get(agent_id)
            .ok_or_else(|| anyhow::anyhow!("Agent not found: {}", agent_id))?;
        
        // Build task
        let task = AgentTask {
            task_type: entry.agent.agent_type().to_string(),
            operation: operation.to_string(),
            path: args.get("path").and_then(|p| p.as_str()).map(String::from),
            args: Some(simd_json::to_string(&args).unwrap_or_else(|_| "{}".to_string())),
            config: args.as_object()
                .map(|obj| obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default(),
        };
        
        // Execute
        match entry.agent.execute(task).await {
            Ok(result) => {
                debug!(agent = %agent_id, success = %result.success, "Agent execution complete");
                
                Ok(json!({
                    "success": result.success,
                    "operation": result.operation,
                    "output": result.data,
                    "agent": agent_id
                }))
            }
            Err(e) => {
                error!(agent = %agent_id, error = %e, "Agent execution failed");
                Err(anyhow::anyhow!("Agent {} failed: {}", agent_id, e))
            }
        }
    }
    
    async fn is_running(&self, agent_id: &str) -> bool {
        self.agents.read().await
            .get(agent_id)
            .map(|e| e.started)
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_executor_creation() {
        let executor = TraitAgentExecutor::new();
        // Give time for async registration
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        
        let agents = executor.list_agents().await;
        assert!(!agents.is_empty());
    }
    
    #[tokio::test]
    async fn test_start_agent() {
        let executor = TraitAgentExecutor::new();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        
        let result = executor.start_agent("memory", None).await;
        assert!(result.is_ok());
        assert!(executor.is_running("memory").await);
    }
    
    #[tokio::test]
    async fn test_execute_memory_list() {
        let executor = TraitAgentExecutor::new();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        
        executor.start_agent("memory", None).await.unwrap();
        
        let result = executor.execute("memory", "list", json!({})).await;
        assert!(result.is_ok());
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/build.rs">
//! Build script for op-mcp
//!
//! Compiles proto files when the grpc feature is enabled.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(feature = "grpc")]
    {
        let proto_file = "proto/mcp.proto";

        // Check if proto file exists
        if std::path::Path::new(proto_file).exists() {
            println!("cargo:rerun-if-changed={}", proto_file);

            // Ensure output directory exists
            std::fs::create_dir_all("src/grpc/generated")?;

            tonic_build::configure()
                .build_server(true)
                .build_client(true)
                .out_dir("src/grpc/generated")
                .compile_protos(&[proto_file], &["proto"])?;

            println!("cargo:warning=gRPC proto compiled successfully");
        } else {
            println!("cargo:warning=Proto file not found: {}", proto_file);
        }
    }

    Ok(())
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/Cargo.toml">
[package]
name = "op-mcp"
version = "0.4.0"
edition = "2021"
description = "Unified MCP Protocol Server with multiple transport and mode support"

[features]
default = []
op-chat = []
code_search = []
grpc = ["tonic", "prost", "tonic-build"]

[dependencies]
# Core
anyhow = "1.0"
async-trait = "0.1"
chrono = "0.4"
serde = { version = "1.0", features = ["derive"] }
simd-json = { workspace = true }
thiserror = "1.0"
tracing = "0.1"
tracing-subscriber = "0.3"
tokio = { version = "1.0", features = ["full"] }
tokio-stream = { version = "0.1", features = ["sync"] }
futures = "0.3"
uuid = { version = "1.0", features = ["v4"] }
prost-types = { workspace = true }

# HTTP/WebSocket
axum = { version = "0.7", features = ["ws"] }
tower-http = { version = "0.5", features = ["cors"] }
reqwest.workspace = true

# D-Bus (for agent executor)
zbus = { workspace = true }

# CLI
clap = { version = "4.0", features = ["derive"] }

# gRPC (optional)
tonic = { workspace = true, optional = true }
prost = { workspace = true, optional = true }

# Internal crates
op-core = { path = "../op-core" }
op-identity = { path = "../op-identity" }
op-tools = { path = "../op-tools" }
op-plugins = { path = "../op-plugins" }
op-introspection = { path = "../op-introspection" }
op-state = { path = "../op-state" }
op-state-store = { path = "../op-state-store" }

[build-dependencies]
tonic-build = { workspace = true, optional = true }

[[bin]]
name = "op-mcp-server"
path = "src/main.rs"

[[bin]]
name = "op-mcp-compact"
path = "src/compact_main.rs"

[[bin]]
name = "op-mcp-agents"
path = "src/agents_main.rs"
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/Cargo.toml.grpc-additions">
# Add these to crates/op-mcp/Cargo.toml

# In [features] section:
# grpc = ["dep:tonic", "dep:prost", "dep:tokio-stream"]
# tls = ["tonic/tls"]
# reflection = ["dep:tonic-reflection"]

# In [dependencies] section:
# tonic = { version = "0.11", optional = true }
# prost = { version = "0.12", optional = true }
# tokio-stream = { version = "0.1", features = ["net"], optional = true }
# tonic-reflection = { version = "0.11", optional = true }

# In [build-dependencies] section:
# tonic-build = { version = "0.11", optional = true }

# Full example Cargo.toml additions:

[features]
default = []
grpc = ["dep:tonic", "dep:prost", "dep:tokio-stream", "dep:tonic-build"]
tls = ["tonic/tls"]
reflection = ["dep:tonic-reflection"]
full = ["grpc", "tls", "reflection"]

[dependencies]
tonic = { version = "0.11", optional = true }
prost = { version = "0.12", optional = true }
tokio-stream = { version = "0.1", features = ["net"], optional = true }
tonic-reflection = { version = "0.11", optional = true }
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1.0", features = ["v4"] }
futures = "0.3"

[build-dependencies]
tonic-build = { version = "0.11", optional = true }
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/compare-op-mcp.md">
# compare-op-mcp

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md, README.md, docs/ARCHITECTURE.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 41 |
| Proto files | 2 |
| Binary targets | 3 |
| UI files | 0 |
| Root-declared modules | 8 |
| Partial artifacts | 2 |
| Spec-listed source files | 20 |
| Spec-listed but missing | 0 |
| Extra implementation files | 21 |

## Current Implementation Overview

- Unified MCP Protocol Server with multiple transport and mode support
- Internal crate integrations: op-core, op-tools, op-plugins, op-introspection, op-state, op-state-store.
- Protocol assets: 2 `.proto` files.

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `src/grpc/generated/op.mcp.v1.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/grpc/generated/op.mcp.v1.rs |
| `src/grpc/service.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/grpc/service.rs |
| `src/grpc/server.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/grpc/server.rs |
| `src/grpc/mod.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/grpc/mod.rs |
| `src/grpc/client.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/grpc/client.rs |
| `src/tools/systemd.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/tools/systemd.rs |
| `src/tools/system.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/tools/system.rs |
| `src/tools/shell.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/tools/shell.rs |
| `src/tools/response.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/tools/response.rs |
| `src/tools/ovs.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/tools/ovs.rs |
| `src/tools/mod.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/tools/mod.rs |
| `src/tools/filesystem.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/tools/filesystem.rs |
| `src/tools/plugin.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/tools/plugin.rs |
| `src/transport/websocket.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/transport/websocket.rs |
| `src/transport/stdio.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/transport/stdio.rs |
| `src/transport/http.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/transport/http.rs |
| `src/transport/mod.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/transport/mod.rs |
| `src/trait_agent_executor.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/trait_agent_executor.rs |
| `src/tool_registry.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/tool_registry.rs |
| `src/tool_adapter_orchestrated.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/tool_adapter_orchestrated.rs |
| `build.rs` | ✅ Present | build script | build.rs |
| `grpc` | ✅ Present | grpc group | src/grpc/client.rs, src/grpc/generated/op.mcp.v1.rs, src/grpc/mod.rs, src/grpc/server.rs, src/grpc/service.rs |
| `root` | ✅ Present | root source group | src/agents_main.rs, src/agents_server.rs, src/builtin_trait_agents.rs, src/compact.rs, src/compact_main.rs, src/config.rs, src/external_client.rs, src/http_server.rs, ... (+14 more) |
| `tools` | ✅ Present | tools group | src/tools/filesystem.rs, src/tools/mod.rs, src/tools/ovs.rs, src/tools/plugin.rs, src/tools/qdrant.rs, src/tools/response.rs, src/tools/shell.rs, src/tools/system.rs, ... (+1 more) |
| `transport` | ✅ Present | transport group | src/transport/http.rs, src/transport/mod.rs, src/transport/stdio.rs, src/transport/websocket.rs |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| trait_agent_executor | ✅ Implemented | src/trait_agent_executor.rs | SPEC main module |
| tool_registry | ✅ Implemented | src/tool_registry.rs | SPEC main module |
| tool_adapter_orchestrated | ✅ Implemented | src/tool_adapter_orchestrated.rs | SPEC main module |
| tool_adapter | ✅ Implemented | src/tool_adapter.rs | SPEC main module |
| sse | ✅ Implemented | src/sse.rs | SPEC main module |
| server | ✅ Implemented | src/grpc/server.rs, src/server.rs | SPEC main module |
| router | ✅ Implemented | src/router.rs | SPEC main module |
| resources | ✅ Implemented | src/resources.rs | SPEC main module |
| request_handler | ✅ Implemented | src/request_handler.rs | SPEC main module |
| request_context | ✅ Implemented | src/request_context.rs | SPEC main module |
| Protocol `internal_agents.proto` | ✅ Implemented | proto/internal_agents.proto | proto |
| Protocol `mcp.proto` | ✅ Implemented | proto/mcp.proto | proto |
| Binary `op-mcp-server` | ✅ Implemented | src/main.rs | Cargo bin target |
| Binary `op-mcp-compact` | ✅ Implemented | src/compact_main.rs | Cargo bin target |
| Binary `op-mcp-agents` | ✅ Implemented | src/agents_main.rs | Cargo bin target |

## Dependencies Comparison

### Internal Workspace Dependencies
- `op-core` - not listed in SPEC dependency block
- `op-tools` - not listed in SPEC dependency block
- `op-plugins` - not listed in SPEC dependency block
- `op-introspection` - not listed in SPEC dependency block
- `op-state` - not listed in SPEC dependency block
- `op-state-store` - not listed in SPEC dependency block

### External Runtime Dependencies
- `anyhow` - documented in SPEC
- `async-trait` - documented in SPEC
- `chrono` - documented in SPEC
- `serde` - documented in SPEC
- `simd-json` - documented in SPEC
- `thiserror` - documented in SPEC
- `tracing` - documented in SPEC
- `tracing-subscriber` - documented in SPEC
- `tokio` - documented in SPEC
- `tokio-stream` - documented in SPEC
- `futures` - documented in SPEC
- `uuid` - documented in SPEC
- `prost-types` - not listed in SPEC dependency block
- `axum` - documented in SPEC
- `tower-http` - documented in SPEC
- `reqwest.workspace` - documented in SPEC
- `zbus` - not listed in SPEC dependency block
- `clap` - not listed in SPEC dependency block
- `tonic` - not listed in SPEC dependency block
- `prost` - not listed in SPEC dependency block

### Development and Build Dependencies
- `build:tonic-build`

## Notes and Observations

- Local documentation files present: README.md, SPEC.md, docs/ARCHITECTURE.md.
- Transitional or partial artifacts detected: src/agents_server.rs.patch, src/mod.rs.patch.
- Current implementation contains 21 Rust source files beyond the explicit spec/design source inventory.
- Root module declarations found in `lib.rs`/`main.rs`: agents_server, compact, protocol, resources, server, transport, tool_registry, grpc.
- Cargo feature flags: default, grpc.
- RPC or protocol definition files: proto/internal_agents.proto, proto/mcp.proto.
- 11 runtime dependencies are present in `Cargo.toml` but omitted from the SPEC dependency block.
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/README.md">
# op-mcp: Minimal MCP Protocol Adapter

## Architecture

This is a **thin protocol adapter** that exposes op-dbus-v2 functionality via the Model Context Protocol (MCP). It delegates all intelligence to existing crates:

- **op-chat** - Orchestration and LLM integration
- **op-tools** - Tool registry and execution
- **op-introspection** - D-Bus discovery and scanning

## Design Principle

**op-mcp = Protocol Adapter ONLY**

All complex functionality already exists in other crates. This crate just translates between:
- MCP JSON-RPC protocol (stdin/stdout)
- op-chat RPC calls

## Protocol Flow

```
stdin → MCP JSON-RPC → ChatActorHandle → stdout
```

### Supported Methods

- `initialize` - MCP handshake and capabilities
- `tools/list` → `chat.list_tools()` - List available tools
- `tools/call` → `chat.execute_tool()` - Execute a tool
- `resources/list` - List documentation resources (placeholder)
- `resources/read` - Read documentation (placeholder)

## Code Size

- **Before**: ~20,000 lines (95% duplication)
- **After**: ~350 lines (protocol adapter only)
- **Reduction**: 98% smaller!

## Building and Running

```bash
# Build the MCP server
cargo build --package op-mcp

# Run as MCP server
./target/debug/op-mcp-server

# Or install and run
cargo install --package op-mcp
op-mcp-server
```

## Dependencies

Minimal dependency set:
- `op-chat` - For orchestration
- `op-core` - For core types
- `tokio` - Async runtime
- `serde` - JSON serialization
- Standard logging/tracing crates

**No duplicate implementations** of:
- Tool registries
- Introspection systems
- Orchestrators
- Chat systems

## Integration

The MCP server integrates seamlessly with:

1. **Claude Desktop** - Add to MCP config:
   ```json
   {
     "mcpServers": {
       "op-dbus-v2": {
         "command": "op-mcp-server",
         "args": []
       }
     }
   }
   ```

2. **Other MCP Clients** - Any client that supports stdio-based MCP servers

## Error Handling

- Proper JSON-RPC 2.0 error responses
- Graceful handling of malformed requests
- Detailed error messages for debugging
- Protocol version compliance

## Testing

The minimal design makes testing straightforward:

- Unit tests for protocol translation
- Integration tests with op-chat
- Protocol compliance tests

## Future Extensions

If needed, this can be extended with:
- Resource registry with embedded documentation
- Additional MCP protocol features
- Health checking and monitoring
- Configuration management

## Migration from Old Implementation

The old implementation (`op-mcp.backup`) had massive duplication:

❌ **Removed** (now handled by other crates):
- Tool registry (use `op-tools`)
- Introspection system (use `op-introspection`)
- Chat orchestration (use `op-chat`)
- Agent management (use `op-agents`)
- Multiple web bridges
- Workflow systems

✅ **Kept** (minimal protocol adapter):
- MCP JSON-RPC protocol handling
- Request/response translation
- Resource serving (placeholder)

## Benefits

1. **Maintainable**: 350 lines vs 20,000 lines
2. **No Duplication**: Each feature exists in ONE place
3. **Clear Architecture**: Single responsibility principle
4. **Easy Testing**: Simple, focused components
5. **Protocol Compliant**: Proper MCP implementation

## Contributing

Keep it simple:
1. If you need new functionality, add it to the appropriate base crate
2. If you need MCP protocol features, add them here
3. Always delegate to existing crates - never duplicate
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-mcp/SPEC.md">
# op-mcp - Specification

## Overview
**Crate**: `op-mcp`  
**Location**: `crates/op-mcp`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-mcp"
version = "0.4.0"
edition = "2021"
description = "Unified MCP Protocol Server with multiple transport and mode support"
```

### Source Structure
```
op-mcp/src/grpc/generated/op.mcp.v1.rs
op-mcp/src/grpc/service.rs
op-mcp/src/grpc/server.rs
op-mcp/src/grpc/mod.rs
op-mcp/src/grpc/client.rs
op-mcp/src/tools/systemd.rs
op-mcp/src/tools/system.rs
op-mcp/src/tools/shell.rs
op-mcp/src/tools/response.rs
op-mcp/src/tools/ovs.rs
op-mcp/src/tools/mod.rs
op-mcp/src/tools/filesystem.rs
op-mcp/src/tools/plugin.rs
op-mcp/src/transport/websocket.rs
op-mcp/src/transport/stdio.rs
op-mcp/src/transport/http.rs
op-mcp/src/transport/mod.rs
op-mcp/src/trait_agent_executor.rs
op-mcp/src/tool_registry.rs
op-mcp/src/tool_adapter_orchestrated.rs
```

### Key Dependencies
```toml
# Core
anyhow = "1.0"
async-trait = "0.1"
chrono = "0.4"
serde = { version = "1.0", features = ["derive"] }
simd-json = { workspace = true }
thiserror = "1.0"
tracing = "0.1"
tracing-subscriber = "0.3"
tokio = { version = "1.0", features = ["full"] }
tokio-stream = { version = "0.1", features = ["sync"] }
futures = "0.3"
uuid = { version = "1.0", features = ["v4"] }

# HTTP/WebSocket
axum = { version = "0.7", features = ["ws"] }
tower-http = { version = "0.5", features = ["cors"] }
reqwest.workspace = true

# D-Bus (for agent executor)
```

### Binaries
```toml
[[bin]]
name = "op-mcp-server"
path = "src/main.rs"

[[bin]]
name = "op-mcp-compact"
path = "src/compact_main.rs"

[[bin]]
name = "op-mcp-agents"
path = "src/agents_main.rs"
```

### Features
```toml
[features]
default = ["grpc"]
grpc = ["tonic", "prost", "tonic-build"]
op-chat = ["dep:op-chat"]

[dependencies]
# Core
anyhow = "1.0"
async-trait = "0.1"
chrono = "0.4"
serde = { version = "1.0", features = ["derive"] }
```

## Documentation Files
README.md

## Module Structure
      39 Rust source files

### Main Modules
trait_agent_executor
tool_registry
tool_adapter_orchestrated
tool_adapter
sse
server
router
resources
request_handler
request_context

## Purpose
Unified MCP Protocol Server with multiple transport and mode support

## Build Information
- **Edition**: 2021
- **Version**: 0.4.0
- **License**: 

## Related Crates
Internal dependencies:
- op-core
- op-tools
- op-plugins
- op-introspection
- op-state
- op-state-store
- op-chat

---
*Generated from crate analysis*
</file>

</files>
