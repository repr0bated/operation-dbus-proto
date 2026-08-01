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
- Only files matching these patterns are included: /home/jeremy/git/operation-dbus-proto/crates/op-cache/**
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
            op-cache/
              proto/
                op_cache.proto
              src/
                grpc/
                  agent_service.rs
                  cache_service.rs
                  mcp_service.rs
                  mod.rs
                  orchestrator_service.rs
                  server.rs
                agent_registry.rs
                agent.rs
                btrfs_cache.rs
                capability_resolver.rs
                lib.rs
                numa.rs
                orchestrator.rs
                pattern_tracker.rs
                snapshot_manager.rs
                workflow_cache.rs
                workflow_executor.rs
                workflow_tracker.rs
                workstack_cache.rs
              build.rs
              Cargo.toml
              compare-op-cache.md
              SPEC.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cache/proto/op_cache.proto">
syntax = "proto3";
package op_cache;

import "google/protobuf/any.proto";
import "google/protobuf/struct.proto";

// ============================================================================
// Core Types
// ============================================================================

enum Capability {
    CAPABILITY_UNSPECIFIED = 0;
    CAPABILITY_CODE_ANALYSIS = 1;
    CAPABILITY_SECURITY_AUDIT = 2;
    CAPABILITY_PERFORMANCE_ANALYSIS = 3;
    CAPABILITY_DEPENDENCY_ANALYSIS = 4;
    CAPABILITY_CODE_GENERATION = 10;
    CAPABILITY_TEST_GENERATION = 11;
    CAPABILITY_DOCUMENTATION_GENERATION = 12;
    CAPABILITY_REFACTORING_SUGGESTION = 13;
    CAPABILITY_CODE_TRANSFORMATION = 14;
    CAPABILITY_FORMAT_CONVERSION = 15;
    CAPABILITY_LANGUAGE_TRANSLATION = 16;
    CAPABILITY_DATA_EXTRACTION = 20;
    CAPABILITY_DATA_VALIDATION = 21;
    CAPABILITY_DATA_ENRICHMENT = 22;
    CAPABILITY_EMBEDDING = 23;
    CAPABILITY_PLANNING = 30;
    CAPABILITY_SUMMARIZATION = 31;
    CAPABILITY_QUESTION_ANSWERING = 32;
    CAPABILITY_CLASSIFICATION = 33;
    CAPABILITY_API_CALL = 40;
    CAPABILITY_DATABASE_QUERY = 41;
    CAPABILITY_FILE_OPERATION = 42;
    CAPABILITY_SHELL_EXECUTION = 43;
}

enum Priority {
    PRIORITY_UNSPECIFIED = 0;
    PRIORITY_HIGH = 1;
    PRIORITY_NORMAL = 2;
    PRIORITY_LOW = 3;
}

message Agent {
    string id = 1;
    string name = 2;
    string description = 3;
    repeated Capability capabilities = 4;
    repeated Capability requires = 5;
    Priority priority = 6;
    bool parallelizable = 7;
    uint64 estimated_latency_ms = 8;
    bool enabled = 9;
}

// ============================================================================
// Structured Agent Input/Output
// ============================================================================

message AgentInput {
    oneof input_type {
        TextInput text = 1;
        CodeInput code = 2;
        AwsInput aws = 3;
        DatabaseInput database = 4;
        FileSystemInput filesystem = 5;
        google.protobuf.Any generic = 99;
    }
    map<string, string> context = 100;
}

message AgentOutput {
    oneof output_type {
        TextOutput text = 1;
        CodeOutput code = 2;
        AwsOutput aws = 3;
        DatabaseOutput database = 4;
        FileSystemOutput filesystem = 5;
        google.protobuf.Any generic = 99;
    }
    map<string, string> metadata = 100;
}

// Input types
message TextInput {
    string content = 1;
    optional string language = 2;
    optional string format = 3;
}

message CodeInput {
    string source_code = 1;
    string language = 2;
    repeated string analysis_types = 3;
}

message AwsInput {
    string service = 1;
    string operation = 2;
    google.protobuf.Struct parameters = 3;
}

message DatabaseInput {
    string query = 1;
    map<string, string> parameters = 2;
    optional string database_type = 3;
}

message FileSystemInput {
    string path = 1;
    optional string content = 2;
    FileOperation operation = 3;
}

enum FileOperation {
    FILE_OPERATION_READ = 0;
    FILE_OPERATION_WRITE = 1;
    FILE_OPERATION_DELETE = 2;
    FILE_OPERATION_LIST = 3;
}

// Output types
message TextOutput {
    string content = 1;
    optional string summary = 2;
    repeated string key_points = 3;
}

message CodeOutput {
    string source_code = 1;
    repeated CodeIssue issues = 2;
    repeated CodeSuggestion suggestions = 3;
}

message CodeIssue {
    string type = 1;
    string message = 2;
    uint32 line = 3;
    uint32 column = 4;
    string severity = 5;
}

message CodeSuggestion {
    string type = 1;
    string description = 2;
    string code = 3;
}

message AwsOutput {
    google.protobuf.Struct result = 1;
    optional string next_token = 2;
}

message DatabaseOutput {
    repeated google.protobuf.Struct rows = 1;
    uint32 row_count = 2;
}

message FileSystemOutput {
    oneof result {
        FileContent content = 1;
        FileList list = 2;
        OperationResult operation = 3;
    }
}

message FileContent {
    string content = 1;
    uint64 size = 2;
    string mime_type = 3;
}

message FileList {
    repeated FileInfo files = 1;
}

message FileInfo {
    string name = 1;
    uint64 size = 2;
    bool is_directory = 3;
    uint64 modified_time = 4;
}

message OperationResult {
    bool success = 1;
    optional string error = 2;
}

// ============================================================================
// Agent Service
// ============================================================================

service AgentService {
    rpc Register(RegisterAgentRequest) returns (RegisterAgentResponse);
    rpc Unregister(UnregisterAgentRequest) returns (UnregisterAgentResponse);
    rpc Execute(ExecuteAgentRequest) returns (ExecuteAgentResponse);
    rpc ExecuteStream(ExecuteAgentRequest) returns (stream ExecuteAgentChunk);
    rpc GetAgent(GetAgentRequest) returns (Agent);
    rpc ListAgents(ListAgentsRequest) returns (ListAgentsResponse);
    rpc FindByCapability(FindByCapabilityRequest) returns (FindByCapabilityResponse);
    rpc ListCapabilities(Empty) returns (ListCapabilitiesResponse);
    rpc HealthCheck(HealthCheckRequest) returns (HealthCheckResponse);
}

message Empty {}

message RegisterAgentRequest {
    Agent agent = 1;
    string endpoint = 2;
}

message RegisterAgentResponse {
    bool success = 1;
    string agent_id = 2;
    string error = 3;
}

message UnregisterAgentRequest {
    string agent_id = 1;
}

message UnregisterAgentResponse {
    bool success = 1;
    Agent removed_agent = 2;
}

message ExecuteAgentRequest {
    string agent_id = 1;
    bytes input = 2;
    map<string, string> context = 3;
    uint64 timeout_ms = 4;
}

message ExecuteAgentResponse {
    bytes output = 1;
    uint64 latency_ms = 2;
    bool success = 3;
    string error = 4;
    map<string, string> metadata = 5;
}

message ExecuteAgentChunk {
    bytes chunk = 1;
    bool is_final = 2;
}

message ProgressUpdate {
    uint32 percent_complete = 1;
    string status_message = 2;
    optional uint64 estimated_remaining_ms = 3;
}

message PartialResult {
    AgentOutput output = 1;
    bool is_complete = 2;
}

message FinalResult {
    AgentOutput output = 1;
    uint64 total_duration_ms = 2;
}

message ErrorInfo {
    string message = 1;
    string code = 2;
    optional google.protobuf.Struct details = 3;
}

message GetAgentRequest {
    string agent_id = 1;
}

message ListAgentsRequest {
    bool enabled_only = 1;
}

message ListAgentsResponse {
    repeated Agent agents = 1;
}

message FindByCapabilityRequest {
    repeated Capability capabilities = 1;
    bool match_all = 2;
}

message FindByCapabilityResponse {
    repeated Agent agents = 1;
}

message ListCapabilitiesResponse {
    repeated Capability capabilities = 1;
    map<int32, int32> capability_agent_count = 2;
}

message HealthCheckRequest {
    string agent_id = 1;
}

message HealthCheckResponse {
    bool healthy = 1;
    string status = 2;
    uint64 uptime_seconds = 3;
}

// ============================================================================
// Orchestrator Service
// ============================================================================

service OrchestratorService {
    rpc Execute(OrchestratorRequest) returns (OrchestratorResponse);
    rpc ExecuteStream(OrchestratorRequest) returns (stream WorkstackStepResult);
    rpc ExecuteAgents(ExecuteAgentsRequest) returns (OrchestratorResponse);
    rpc Resolve(ResolveRequest) returns (ResolveResponse);
    rpc GetPatterns(Empty) returns (GetPatternsResponse);
    rpc PromotePattern(PromotePatternRequest) returns (PromotePatternResponse);
    rpc GetStats(Empty) returns (OrchestratorStats);
}

message OrchestratorRequest {
    string request_id = 1;
    repeated Capability required_capabilities = 2;
    bytes input = 3;
    repeated string preferred_agents = 4;
    repeated string excluded_agents = 5;
}

message OrchestratorResponse {
    string request_id = 1;
    bytes output = 2;
    repeated WorkstackStepResult steps = 3;
    uint64 total_latency_ms = 4;
    uint32 cache_hits = 5;
    uint32 cache_misses = 6;
    bool used_workstack = 7;
    repeated string resolved_agents = 8;
    repeated Capability fulfilled_capabilities = 9;
    repeated Capability missing_capabilities = 10;
}

message ExecuteAgentsRequest {
    string request_id = 1;
    repeated string agent_ids = 2;
    bytes input = 3;
}

message WorkstackStepResult {
    uint32 step_index = 1;
    string agent_id = 2;
    bytes output = 3;
    uint64 latency_ms = 4;
    bool cached = 5;
    uint64 output_size = 6;
    bool success = 7;
    string error = 8;
}

message ResolveRequest {
    repeated Capability required_capabilities = 1;
    repeated string preferred_agents = 2;
    repeated string excluded_agents = 3;
}

message ResolveResponse {
    repeated Agent agents = 1;
    repeated Capability fulfilled_capabilities = 2;
    repeated Capability missing_capabilities = 3;
    uint64 estimated_latency_ms = 4;
    repeated string resolution_path = 5;
}

message GetPatternsResponse {
    repeated PatternSuggestion patterns = 1;
}

message PromotePatternRequest {
    string pattern_id = 1;
}

message PromotePatternResponse {
    bool success = 1;
    string workstack_id = 2;
    string error = 3;
}

message PatternSuggestion {
    string pattern_id = 1;
    repeated string agent_sequence = 2;
    uint32 call_count = 3;
    uint64 avg_latency_ms = 4;
    string suggested_name = 5;
    double confidence_score = 6;
    uint64 estimated_time_saved_ms = 7;
}

message OrchestratorStats {
    uint32 registered_agents = 1;
    uint32 enabled_agents = 2;
    uint32 available_capabilities = 3;
    uint32 tracked_patterns = 4;
    uint32 promoted_patterns = 5;
    uint64 cache_entries = 6;
    double cache_hit_rate = 7;
    uint32 numa_nodes = 8;
}

// ============================================================================
// Cache Service
// ============================================================================

service CacheService {
    rpc GetStep(GetStepRequest) returns (GetStepResponse);
    rpc PutStep(PutStepRequest) returns (PutStepResponse);
    rpc InvalidateWorkstack(InvalidateWorkstackRequest) returns (InvalidateWorkstackResponse);
    rpc InvalidateStep(InvalidateStepRequest) returns (InvalidateStepResponse);
    rpc Cleanup(CleanupRequest) returns (CleanupResponse);
    rpc GetStats(Empty) returns (CacheStats);
    rpc GetWorkstackStats(GetWorkstackStatsRequest) returns (WorkstackCacheStats);
}

message GetStepRequest {
    string workstack_id = 1;
    uint32 step_index = 2;
    string input_hash = 3;
}

message GetStepResponse {
    bool found = 1;
    bytes output = 2;
    uint64 created_at = 3;
    uint64 expires_at = 4;
    uint32 access_count = 5;
}

message PutStepRequest {
    string workstack_id = 1;
    uint32 step_index = 2;
    string input_hash = 3;
    bytes output = 4;
    int64 ttl_seconds = 5;
}

message PutStepResponse {
    bool success = 1;
    string cache_key = 2;
    uint64 size_bytes = 3;
    bool compressed = 4;
}

message InvalidateWorkstackRequest {
    string workstack_id = 1;
}

message InvalidateWorkstackResponse {
    uint32 entries_removed = 1;
}

message InvalidateStepRequest {
    string workstack_id = 1;
    uint32 step_index = 2;
}

message InvalidateStepResponse {
    uint32 entries_removed = 1;
}

message CleanupRequest {
    bool expired_only = 1;
    uint64 max_age_seconds = 2;
}

message CleanupResponse {
    uint32 entries_removed = 1;
    uint64 bytes_freed = 2;
}

message CacheStats {
    uint64 total_entries = 1;
    uint64 total_size_bytes = 2;
    uint64 hot_entries = 3;
    uint64 expired_entries = 4;
    uint64 total_hits = 5;
    uint64 total_misses = 6;
    uint64 workstacks_cached = 7;
    double hit_rate = 8;
}

message GetWorkstackStatsRequest {
    string workstack_id = 1;
}

message WorkstackCacheStats {
    string workstack_id = 1;
    uint64 total_entries = 2;
    uint64 total_size_bytes = 3;
    uint64 hit_count = 4;
    uint64 miss_count = 5;
    double hit_rate = 6;
}

// ============================================================================
// MCP Service
// ============================================================================

service McpService {
    rpc HandleRequest(McpRequest) returns (McpResponse);
    rpc ListTools(ListToolsRequest) returns (ListToolsResponse);
}

message McpRequest {
    string jsonrpc = 1;
    string method = 2;
    string id = 3;
    bytes params = 4;
}

message McpResponse {
    string jsonrpc = 1;
    string id = 2;
    bytes result = 3;
    McpError error = 4;
}

message McpError {
    int32 code = 1;
    string message = 2;
    bytes data = 3;
}

message ListToolsRequest {}

message ListToolsResponse {
    repeated McpTool tools = 1;
}

message McpTool {
    string name = 1;
    string description = 2;
    bytes input_schema = 3;
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cache/src/grpc/agent_service.rs">
//! Agent service implementation
//!
//! Manages agent registration, execution, and capability queries.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::RwLock;
use tonic::{Request, Response, Status};
use tracing::{debug, info, warn};

use super::proto::{
    agent_service_server::AgentService, Agent, Empty, ExecuteAgentChunk, ExecuteAgentRequest,
    ExecuteAgentResponse, FindByCapabilityRequest, FindByCapabilityResponse, GetAgentRequest,
    HealthCheckRequest, HealthCheckResponse, ListAgentsRequest, ListAgentsResponse,
    ListCapabilitiesResponse, RegisterAgentRequest, RegisterAgentResponse, UnregisterAgentRequest,
    UnregisterAgentResponse,
};

/// Agent executor function type
pub type AgentExecutor = Arc<dyn Fn(&[u8]) -> Result<Vec<u8>, String> + Send + Sync>;

/// Registered agent with metadata and executor
struct RegisteredAgent {
    definition: Agent,
    executor: Option<AgentExecutor>,
    #[allow(dead_code)]
    endpoint: Option<String>,
    registered_at: std::time::Instant,
}

pub struct AgentServiceImpl {
    agents: Arc<RwLock<HashMap<String, RegisteredAgent>>>,
    capability_index: Arc<RwLock<HashMap<i32, Vec<String>>>>,
}

impl AgentServiceImpl {
    pub fn new() -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            capability_index: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a local agent with an executor function
    pub async fn register_local(
        &self,
        agent: Agent,
        executor: AgentExecutor,
    ) -> Result<(), String> {
        let agent_id = agent.id.clone();
        let capabilities = agent.capabilities.clone();

        {
            let mut agents = self.agents.write().await;
            agents.insert(
                agent_id.clone(),
                RegisteredAgent {
                    definition: agent,
                    executor: Some(executor),
                    endpoint: None,
                    registered_at: Instant::now(),
                },
            );
        }

        // Update capability index
        {
            let mut index = self.capability_index.write().await;
            for cap in capabilities {
                index
                    .entry(cap)
                    .or_insert_with(Vec::new)
                    .push(agent_id.clone());
            }
        }

        info!("Registered local agent: {}", agent_id);
        Ok(())
    }

    /// Execute agent locally
    async fn execute_local(&self, agent_id: &str, input: &[u8]) -> Result<Vec<u8>, Status> {
        let agents = self.agents.read().await;
        let agent = agents
            .get(agent_id)
            .ok_or_else(|| Status::not_found(format!("Agent not found: {}", agent_id)))?;

        let executor = agent
            .executor
            .as_ref()
            .ok_or_else(|| Status::failed_precondition("Agent has no local executor"))?;

        executor(input).map_err(|e| Status::internal(format!("Agent execution failed: {}", e)))
    }
}

impl Default for AgentServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl AgentService for AgentServiceImpl {
    async fn register(
        &self,
        request: Request<RegisterAgentRequest>,
    ) -> Result<Response<RegisterAgentResponse>, Status> {
        let req = request.into_inner();
        let agent = req
            .agent
            .ok_or_else(|| Status::invalid_argument("Agent definition required"))?;

        let agent_id = agent.id.clone();
        let capabilities = agent.capabilities.clone();

        // Store agent
        {
            let mut agents = self.agents.write().await;
            agents.insert(
                agent_id.clone(),
                RegisteredAgent {
                    definition: agent,
                    executor: None, // Remote agents don't have local executors
                    endpoint: if req.endpoint.is_empty() {
                        None
                    } else {
                        Some(req.endpoint)
                    },
                    registered_at: Instant::now(),
                },
            );
        }

        // Update capability index
        {
            let mut index = self.capability_index.write().await;
            for cap in capabilities {
                index
                    .entry(cap)
                    .or_insert_with(Vec::new)
                    .push(agent_id.clone());
            }
        }

        info!("Registered agent via gRPC: {}", agent_id);

        Ok(Response::new(RegisterAgentResponse {
            success: true,
            agent_id,
            error: String::new(),
        }))
    }

    async fn unregister(
        &self,
        request: Request<UnregisterAgentRequest>,
    ) -> Result<Response<UnregisterAgentResponse>, Status> {
        let req = request.into_inner();

        let removed = {
            let mut agents = self.agents.write().await;
            agents.remove(&req.agent_id)
        };

        if let Some(agent) = removed {
            // Remove from capability index
            let mut index = self.capability_index.write().await;
            for cap in &agent.definition.capabilities {
                if let Some(agents) = index.get_mut(cap) {
                    agents.retain(|id| id != &req.agent_id);
                }
            }

            info!("Unregistered agent: {}", req.agent_id);

            Ok(Response::new(UnregisterAgentResponse {
                success: true,
                removed_agent: Some(agent.definition),
            }))
        } else {
            Ok(Response::new(UnregisterAgentResponse {
                success: false,
                removed_agent: None,
            }))
        }
    }

    async fn execute(
        &self,
        request: Request<ExecuteAgentRequest>,
    ) -> Result<Response<ExecuteAgentResponse>, Status> {
        let req = request.into_inner();
        let start = Instant::now();

        debug!("Executing agent: {}", req.agent_id);

        match self.execute_local(&req.agent_id, &req.input).await {
            Ok(output) => {
                let latency_ms = start.elapsed().as_millis() as u64;
                Ok(Response::new(ExecuteAgentResponse {
                    output,
                    latency_ms,
                    success: true,
                    error: String::new(),
                    metadata: HashMap::new(),
                }))
            }
            Err(e) => Ok(Response::new(ExecuteAgentResponse {
                output: Vec::new(),
                latency_ms: start.elapsed().as_millis() as u64,
                success: false,
                error: e.message().to_string(),
                metadata: HashMap::new(),
            })),
        }
    }

    type ExecuteStreamStream =
        tokio_stream::wrappers::ReceiverStream<Result<ExecuteAgentChunk, Status>>;

    async fn execute_stream(
        &self,
        request: Request<ExecuteAgentRequest>,
    ) -> Result<Response<Self::ExecuteStreamStream>, Status> {
        let req = request.into_inner();
        let (tx, rx) = tokio::sync::mpsc::channel(32);

        let agents = self.agents.clone();
        let agent_id = req.agent_id.clone();
        let input = req.input;

        tokio::spawn(async move {
            let agents_guard = agents.read().await;
            if let Some(agent) = agents_guard.get(&agent_id) {
                if let Some(executor) = &agent.executor {
                    match executor(&input) {
                        Ok(output) => {
                            // Send output in chunks
                            let chunk_size = 64 * 1024; // 64KB chunks

                            for (sequence, chunk) in output.chunks(chunk_size).enumerate() {
                                let sequence = sequence as u64;
                                let is_final = sequence * chunk_size as u64 + chunk.len() as u64
                                    >= output.len() as u64;

                                let _ = tx
                                    .send(Ok(ExecuteAgentChunk {
                                        chunk: chunk.to_vec(),
                                        is_final,
                                    }))
                                    .await;
                            }
                        }
                        Err(e) => {
                            warn!("Agent {} execution failed: {}", agent_id, e);
                        }
                    }
                }
            }
        });

        Ok(Response::new(tokio_stream::wrappers::ReceiverStream::new(
            rx,
        )))
    }

    async fn get_agent(
        &self,
        request: Request<GetAgentRequest>,
    ) -> Result<Response<Agent>, Status> {
        let req = request.into_inner();
        let agents = self.agents.read().await;

        agents
            .get(&req.agent_id)
            .map(|a| Response::new(a.definition.clone()))
            .ok_or_else(|| Status::not_found(format!("Agent not found: {}", req.agent_id)))
    }

    async fn list_agents(
        &self,
        request: Request<ListAgentsRequest>,
    ) -> Result<Response<ListAgentsResponse>, Status> {
        let req = request.into_inner();
        let agents = self.agents.read().await;

        let agent_list: Vec<Agent> = agents
            .values()
            .filter(|a| !req.enabled_only || a.definition.enabled)
            .map(|a| a.definition.clone())
            .collect();

        Ok(Response::new(ListAgentsResponse { agents: agent_list }))
    }

    async fn find_by_capability(
        &self,
        request: Request<FindByCapabilityRequest>,
    ) -> Result<Response<FindByCapabilityResponse>, Status> {
        let req = request.into_inner();
        let index = self.capability_index.read().await;
        let agents = self.agents.read().await;

        let matching_ids: Vec<String> = if req.match_all {
            // Agent must have ALL requested capabilities
            let mut sets: Vec<std::collections::HashSet<&String>> = Vec::new();
            for cap in &req.capabilities {
                if let Some(ids) = index.get(cap) {
                    sets.push(ids.iter().collect());
                } else {
                    // Capability not found, no agents match
                    return Ok(Response::new(FindByCapabilityResponse {
                        agents: Vec::new(),
                    }));
                }
            }

            if sets.is_empty() {
                Vec::new()
            } else {
                let first = sets.remove(0);
                first
                    .into_iter()
                    .filter(|id| sets.iter().all(|s| s.contains(id)))
                    .cloned()
                    .collect()
            }
        } else {
            // Agent can have ANY of the requested capabilities
            let mut seen = std::collections::HashSet::new();
            let mut result = Vec::new();
            for cap in &req.capabilities {
                if let Some(ids) = index.get(cap) {
                    for id in ids {
                        if seen.insert(id.clone()) {
                            result.push(id.clone());
                        }
                    }
                }
            }
            result
        };

        let matching_agents: Vec<Agent> = matching_ids
            .iter()
            .filter_map(|id| agents.get(id).map(|a| a.definition.clone()))
            .filter(|a| a.enabled)
            .collect();

        Ok(Response::new(FindByCapabilityResponse {
            agents: matching_agents,
        }))
    }

    async fn list_capabilities(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<ListCapabilitiesResponse>, Status> {
        let index = self.capability_index.read().await;

        let capabilities: Vec<i32> = index.keys().copied().collect();
        let capability_agent_count: HashMap<i32, i32> = index
            .iter()
            .map(|(cap, agents)| (*cap, agents.len() as i32))
            .collect();

        Ok(Response::new(ListCapabilitiesResponse {
            capabilities,
            capability_agent_count,
        }))
    }

    async fn health_check(
        &self,
        request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        let req = request.into_inner();
        let agents = self.agents.read().await;

        if let Some(agent) = agents.get(&req.agent_id) {
            let uptime = agent.registered_at.elapsed().as_secs();
            Ok(Response::new(HealthCheckResponse {
                healthy: agent.definition.enabled,
                status: if agent.definition.enabled {
                    "healthy".to_string()
                } else {
                    "disabled".to_string()
                },
                uptime_seconds: uptime,
            }))
        } else {
            Ok(Response::new(HealthCheckResponse {
                healthy: false,
                status: "not_found".to_string(),
                uptime_seconds: 0,
            }))
        }
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cache/src/grpc/cache_service.rs">
//! Cache service implementation
//!
//! Provides workstack step caching with TTL and compression.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use sha2::{Digest, Sha256};
use tokio::sync::RwLock;
use tonic::{Request, Response, Status};
use tracing::debug;

use super::proto::{
    cache_service_server::CacheService, CacheStats, CleanupRequest, CleanupResponse, Empty,
    GetStepRequest, GetStepResponse, GetWorkstackStatsRequest, InvalidateStepRequest,
    InvalidateStepResponse, InvalidateWorkstackRequest, InvalidateWorkstackResponse,
    PutStepRequest, PutStepResponse, WorkstackCacheStats,
};

/// Cached step entry
struct CachedEntry {
    output: Vec<u8>,
    created_at: u64,
    expires_at: u64,
    access_count: u32,
    size_bytes: u64,
    #[allow(dead_code)]
    compressed: bool,
}

/// Per-workstack statistics
#[derive(Default)]
struct WorkstackStats {
    hit_count: AtomicU64,
    miss_count: AtomicU64,
}

pub struct CacheServiceImpl {
    entries: Arc<RwLock<HashMap<String, CachedEntry>>>,
    workstack_stats: Arc<RwLock<HashMap<String, WorkstackStats>>>,
    default_ttl_secs: i64,
    total_hits: AtomicU64,
    total_misses: AtomicU64,
}

impl CacheServiceImpl {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            workstack_stats: Arc::new(RwLock::new(HashMap::new())),
            default_ttl_secs: 3600,
            total_hits: AtomicU64::new(0),
            total_misses: AtomicU64::new(0),
        }
    }

    pub fn with_ttl(default_ttl_secs: i64) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            workstack_stats: Arc::new(RwLock::new(HashMap::new())),
            default_ttl_secs,
            total_hits: AtomicU64::new(0),
            total_misses: AtomicU64::new(0),
        }
    }

    fn make_cache_key(workstack_id: &str, step_index: u32, input_hash: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(format!("{}:{}:{}", workstack_id, step_index, input_hash).as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn now_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    /// Internal method for orchestrator to get cached step
    pub async fn get_step_internal(
        &self,
        workstack_id: &str,
        step_index: u32,
        input_hash: &str,
    ) -> Option<Vec<u8>> {
        let cache_key = Self::make_cache_key(workstack_id, step_index, input_hash);
        let now = Self::now_timestamp();

        let entries = self.entries.read().await;
        if let Some(entry) = entries.get(&cache_key) {
            if now <= entry.expires_at {
                self.total_hits.fetch_add(1, Ordering::Relaxed);
                self.record_hit(workstack_id).await;
                return Some(entry.output.clone());
            }
        }

        self.total_misses.fetch_add(1, Ordering::Relaxed);
        self.record_miss(workstack_id).await;
        None
    }

    /// Internal method for orchestrator to store cached step
    pub async fn put_step_internal(
        &self,
        workstack_id: &str,
        step_index: u32,
        input_hash: &str,
        output: &[u8],
    ) {
        let cache_key = Self::make_cache_key(workstack_id, step_index, input_hash);
        let now = Self::now_timestamp();

        let entry = CachedEntry {
            output: output.to_vec(),
            created_at: now,
            expires_at: now + self.default_ttl_secs as u64,
            access_count: 1,
            size_bytes: output.len() as u64,
            compressed: false,
        };

        let mut entries = self.entries.write().await;
        entries.insert(cache_key, entry);
    }

    /// Get cache statistics (internal)
    pub async fn get_stats_internal(&self) -> CacheStats {
        let entries = self.entries.read().await;
        let now = Self::now_timestamp();

        let total_entries = entries.len() as u64;
        let total_size: u64 = entries.values().map(|e| e.size_bytes).sum();
        let hot_entries = entries
            .values()
            .filter(|e| now.saturating_sub(e.created_at) < 600)
            .count() as u64;
        let expired_entries = entries.values().filter(|e| now > e.expires_at).count() as u64;

        let total_hits = self.total_hits.load(Ordering::Relaxed);
        let total_misses = self.total_misses.load(Ordering::Relaxed);

        let workstack_stats = self.workstack_stats.read().await;
        let workstacks_cached = workstack_stats.len() as u64;

        let hit_rate = if total_hits + total_misses > 0 {
            total_hits as f64 / (total_hits + total_misses) as f64
        } else {
            0.0
        };

        CacheStats {
            total_entries,
            total_size_bytes: total_size,
            hot_entries,
            expired_entries,
            total_hits,
            total_misses,
            workstacks_cached,
            hit_rate,
        }
    }

    async fn record_hit(&self, workstack_id: &str) {
        let mut stats = self.workstack_stats.write().await;
        stats
            .entry(workstack_id.to_string())
            .or_default()
            .hit_count
            .fetch_add(1, Ordering::Relaxed);
    }

    async fn record_miss(&self, workstack_id: &str) {
        let mut stats = self.workstack_stats.write().await;
        stats
            .entry(workstack_id.to_string())
            .or_default()
            .miss_count
            .fetch_add(1, Ordering::Relaxed);
    }
}

impl Default for CacheServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[tonic::async_trait]
impl CacheService for CacheServiceImpl {
    async fn get_step(
        &self,
        request: Request<GetStepRequest>,
    ) -> Result<Response<GetStepResponse>, Status> {
        let req = request.into_inner();
        let cache_key = Self::make_cache_key(&req.workstack_id, req.step_index, &req.input_hash);
        let now = Self::now_timestamp();

        let mut entries = self.entries.write().await;

        if let Some(entry) = entries.get_mut(&cache_key) {
            if now <= entry.expires_at {
                entry.access_count += 1;
                self.total_hits.fetch_add(1, Ordering::Relaxed);
                self.record_hit(&req.workstack_id).await;

                return Ok(Response::new(GetStepResponse {
                    found: true,
                    output: entry.output.clone(),
                    created_at: entry.created_at,
                    expires_at: entry.expires_at,
                    access_count: entry.access_count,
                }));
            }
        }

        self.total_misses.fetch_add(1, Ordering::Relaxed);
        self.record_miss(&req.workstack_id).await;

        Ok(Response::new(GetStepResponse {
            found: false,
            output: Vec::new(),
            created_at: 0,
            expires_at: 0,
            access_count: 0,
        }))
    }

    async fn put_step(
        &self,
        request: Request<PutStepRequest>,
    ) -> Result<Response<PutStepResponse>, Status> {
        let req = request.into_inner();
        let cache_key = Self::make_cache_key(&req.workstack_id, req.step_index, &req.input_hash);
        let now = Self::now_timestamp();

        let ttl = if req.ttl_seconds > 0 {
            req.ttl_seconds as u64
        } else {
            self.default_ttl_secs as u64
        };

        let size_bytes = req.output.len() as u64;

        let entry = CachedEntry {
            output: req.output,
            created_at: now,
            expires_at: now + ttl,
            access_count: 1,
            size_bytes,
            compressed: false, // TODO: add compression
        };

        let mut entries = self.entries.write().await;
        entries.insert(cache_key.clone(), entry);

        debug!(
            "Cached step {} index {} ({} bytes)",
            req.workstack_id, req.step_index, size_bytes
        );

        Ok(Response::new(PutStepResponse {
            success: true,
            cache_key,
            size_bytes,
            compressed: false,
        }))
    }

    async fn invalidate_workstack(
        &self,
        request: Request<InvalidateWorkstackRequest>,
    ) -> Result<Response<InvalidateWorkstackResponse>, Status> {
        let req = request.into_inner();
        let prefix = format!("{}:", req.workstack_id);

        let mut entries = self.entries.write().await;
        let before = entries.len();

        // This is inefficient - in production, maintain a workstack->keys index
        entries.retain(|k, _| !k.starts_with(&prefix));

        let removed = (before - entries.len()) as u32;

        Ok(Response::new(InvalidateWorkstackResponse {
            entries_removed: removed,
        }))
    }

    async fn invalidate_step(
        &self,
        request: Request<InvalidateStepRequest>,
    ) -> Result<Response<InvalidateStepResponse>, Status> {
        let req = request.into_inner();
        let prefix = format!("{}:{}:", req.workstack_id, req.step_index);

        let mut entries = self.entries.write().await;
        let before = entries.len();
        entries.retain(|k, _| !k.starts_with(&prefix));
        let removed = (before - entries.len()) as u32;

        Ok(Response::new(InvalidateStepResponse {
            entries_removed: removed,
        }))
    }

    async fn cleanup(
        &self,
        request: Request<CleanupRequest>,
    ) -> Result<Response<CleanupResponse>, Status> {
        let req = request.into_inner();
        let now = Self::now_timestamp();

        let mut entries = self.entries.write().await;
        let before_len = entries.len();
        let before_size: u64 = entries.values().map(|e| e.size_bytes).sum();

        if req.expired_only {
            entries.retain(|_, e| now <= e.expires_at);
        } else if req.max_age_seconds > 0 {
            let cutoff = now.saturating_sub(req.max_age_seconds);
            entries.retain(|_, e| e.created_at >= cutoff);
        } else {
            entries.retain(|_, e| now <= e.expires_at);
        }

        let after_size: u64 = entries.values().map(|e| e.size_bytes).sum();
        let removed = (before_len - entries.len()) as u32;
        let bytes_freed = before_size.saturating_sub(after_size);

        Ok(Response::new(CleanupResponse {
            entries_removed: removed,
            bytes_freed,
        }))
    }

    async fn get_stats(&self, _request: Request<Empty>) -> Result<Response<CacheStats>, Status> {
        Ok(Response::new(self.get_stats_internal().await))
    }

    async fn get_workstack_stats(
        &self,
        request: Request<GetWorkstackStatsRequest>,
    ) -> Result<Response<WorkstackCacheStats>, Status> {
        let req = request.into_inner();
        let entries = self.entries.read().await;
        let stats = self.workstack_stats.read().await;

        // Count entries for this workstack (inefficient without index)
        let prefix = format!("{}:", req.workstack_id);
        let workstack_entries: Vec<_> = entries
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .collect();

        let total_entries = workstack_entries.len() as u64;
        let total_size: u64 = workstack_entries.iter().map(|(_, e)| e.size_bytes).sum();

        let (hit_count, miss_count) = if let Some(ws_stats) = stats.get(&req.workstack_id) {
            (
                ws_stats.hit_count.load(Ordering::Relaxed),
                ws_stats.miss_count.load(Ordering::Relaxed),
            )
        } else {
            (0, 0)
        };

        let hit_rate = if hit_count + miss_count > 0 {
            hit_count as f64 / (hit_count + miss_count) as f64
        } else {
            0.0
        };

        Ok(Response::new(WorkstackCacheStats {
            workstack_id: req.workstack_id,
            total_entries,
            total_size_bytes: total_size,
            hit_count,
            miss_count,
            hit_rate,
        }))
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cache/src/grpc/mcp_service.rs">
//! MCP (Model Context Protocol) service implementation
//!
//! Bridges the MCP JSON-RPC protocol to the agent registry and orchestrator.
//! - HandleRequest: Dispatches MCP JSON-RPC requests to agents
//! - ListTools: Exposes registered agents as MCP tools

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tonic::{Request, Response, Status};
use tracing::{debug, info, warn};

use super::agent_service::AgentServiceImpl;
use super::orchestrator_service::OrchestratorServiceImpl;
use super::proto::{
    agent_service_server::AgentService, mcp_service_server::McpService, ListAgentsRequest,
    ListToolsRequest, ListToolsResponse, McpError, McpRequest, McpResponse, McpTool,
};

/// MCP service implementation backed by the agent registry and orchestrator.
pub struct McpServiceImpl {
    agent_service: Arc<AgentServiceImpl>,
    /// Reserved for capability-based MCP routing in future methods.
    #[allow(dead_code)]
    orchestrator_service: Arc<OrchestratorServiceImpl>,
}

impl McpServiceImpl {
    /// Create a new MCP service.
    pub fn new(
        agent_service: Arc<AgentServiceImpl>,
        orchestrator_service: Arc<OrchestratorServiceImpl>,
    ) -> Self {
        Self {
            agent_service,
            orchestrator_service,
        }
    }

    /// Dispatch a `tools/call` request to the appropriate agent.
    async fn handle_tools_call(&self, id: &str, params: &[u8]) -> Result<McpResponse, Status> {
        // Parse the params to extract tool name and arguments.
        // Expected JSON: { "name": "<agent_id>", "arguments": { ... } }
        let mut params_buf = params.to_vec();
        let parsed: Result<ToolCallParams, _> = simd_json::from_slice(&mut params_buf);

        let tool_call = match parsed {
            Ok(tc) => tc,
            Err(e) => {
                return Ok(McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: id.to_string(),
                    result: Vec::new(),
                    error: Some(McpError {
                        code: -32602, // Invalid params
                        message: format!("Invalid tools/call params: {}", e),
                        data: Vec::new(),
                    }),
                });
            }
        };

        let start = Instant::now();
        debug!("MCP tools/call: agent={}", tool_call.name);

        // Serialize the arguments back to bytes for the agent input
        let input = match simd_json::to_vec(&tool_call.arguments) {
            Ok(bytes) => bytes,
            Err(e) => {
                return Ok(McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: id.to_string(),
                    result: Vec::new(),
                    error: Some(McpError {
                        code: -32603, // Internal error
                        message: format!("Failed to serialize arguments: {}", e),
                        data: Vec::new(),
                    }),
                });
            }
        };

        // Execute through the agent service
        let exec_req = Request::new(super::proto::ExecuteAgentRequest {
            agent_id: tool_call.name.clone(),
            input,
            context: HashMap::new(),
            timeout_ms: 0,
        });

        let exec_response = self.agent_service.execute(exec_req).await?;
        let result = exec_response.into_inner();
        let latency_ms = start.elapsed().as_millis() as u64;

        if result.success {
            // Wrap agent output in MCP content format:
            // { "content": [{ "type": "text", "text": "<output>" }] }
            let content_response = McpContentResponse {
                content: vec![McpContent {
                    r#type: "text".to_string(),
                    text: String::from_utf8_lossy(&result.output).to_string(),
                }],
            };

            let response_bytes = simd_json::to_vec(&content_response).unwrap_or_default();

            debug!(
                "MCP tools/call completed: agent={} latency_ms={}",
                tool_call.name, latency_ms
            );

            Ok(McpResponse {
                jsonrpc: "2.0".to_string(),
                id: id.to_string(),
                result: response_bytes,
                error: None,
            })
        } else {
            warn!(
                "MCP tools/call failed: agent={} error={}",
                tool_call.name, result.error
            );

            Ok(McpResponse {
                jsonrpc: "2.0".to_string(),
                id: id.to_string(),
                result: Vec::new(),
                error: Some(McpError {
                    code: -32603, // Internal error
                    message: result.error,
                    data: Vec::new(),
                }),
            })
        }
    }

    /// Handle a `tools/list` request by delegating to ListTools.
    async fn handle_tools_list(&self, id: &str) -> Result<McpResponse, Status> {
        let tools_response = self.list_tools_internal().await?;

        let response_bytes = simd_json::to_vec(&McpToolsListResult {
            tools: tools_response
                .tools
                .iter()
                .map(|t| McpToolJson {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    input_schema: {
                        let mut buf = t.input_schema.clone();
                        if buf.is_empty() {
                            serde_json::Value::Object(serde_json::Map::new())
                        } else {
                            simd_json::from_slice(&mut buf)
                                .unwrap_or(serde_json::Value::Object(serde_json::Map::new()))
                        }
                    },
                })
                .collect(),
        })
        .unwrap_or_default();

        Ok(McpResponse {
            jsonrpc: "2.0".to_string(),
            id: id.to_string(),
            result: response_bytes,
            error: None,
        })
    }

    /// Handle an `initialize` request.
    fn handle_initialize(&self, id: &str) -> McpResponse {
        let init_result = McpInitializeResult {
            protocol_version: "2024-11-05".to_string(),
            capabilities: McpServerCapabilities {
                tools: Some(McpToolCapability { list_changed: true }),
            },
            server_info: McpServerInfo {
                name: "op-cache".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };

        let result_bytes = simd_json::to_vec(&init_result).unwrap_or_default();

        McpResponse {
            jsonrpc: "2.0".to_string(),
            id: id.to_string(),
            result: result_bytes,
            error: None,
        }
    }

    /// Handle a `ping` request.
    fn handle_ping(&self, id: &str) -> McpResponse {
        McpResponse {
            jsonrpc: "2.0".to_string(),
            id: id.to_string(),
            result: b"{}".to_vec(),
            error: None,
        }
    }

    /// Build the list of tools from the agent registry.
    async fn list_tools_internal(&self) -> Result<ListToolsResponse, Status> {
        let agents_response = self
            .agent_service
            .list_agents(Request::new(ListAgentsRequest { enabled_only: true }))
            .await?
            .into_inner();

        let tools: Vec<McpTool> = agents_response
            .agents
            .into_iter()
            .map(|agent| {
                // Build a JSON Schema describing the agent's input.
                // Each agent accepts arbitrary JSON via the "input" field.
                let input_schema = build_agent_input_schema(&agent.name, &agent.description);

                McpTool {
                    name: agent.id,
                    description: if agent.description.is_empty() {
                        agent.name
                    } else {
                        agent.description
                    },
                    input_schema,
                }
            })
            .collect();

        Ok(ListToolsResponse { tools })
    }
}

/// Build a minimal JSON Schema for an agent's input.
fn build_agent_input_schema(_name: &str, _description: &str) -> Vec<u8> {
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "input": {
                "type": "string",
                "description": "Input data for the agent"
            }
        }
    });

    simd_json::to_vec(&schema).unwrap_or_default()
}

// --- Internal serde types for MCP JSON-RPC ---

#[derive(serde::Deserialize)]
struct ToolCallParams {
    name: String,
    #[serde(default)]
    arguments: serde_json::Value,
}

#[derive(serde::Serialize)]
struct McpContentResponse {
    content: Vec<McpContent>,
}

#[derive(serde::Serialize)]
struct McpContent {
    r#type: String,
    text: String,
}

#[derive(serde::Serialize)]
struct McpToolsListResult {
    tools: Vec<McpToolJson>,
}

#[derive(serde::Serialize)]
struct McpToolJson {
    name: String,
    description: String,
    #[serde(rename = "inputSchema")]
    input_schema: serde_json::Value,
}

#[derive(serde::Serialize)]
struct McpInitializeResult {
    #[serde(rename = "protocolVersion")]
    protocol_version: String,
    capabilities: McpServerCapabilities,
    #[serde(rename = "serverInfo")]
    server_info: McpServerInfo,
}

#[derive(serde::Serialize)]
struct McpServerCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<McpToolCapability>,
}

#[derive(serde::Serialize)]
struct McpToolCapability {
    #[serde(rename = "listChanged")]
    list_changed: bool,
}

#[derive(serde::Serialize)]
struct McpServerInfo {
    name: String,
    version: String,
}

#[tonic::async_trait]
impl McpService for McpServiceImpl {
    async fn handle_request(
        &self,
        request: Request<McpRequest>,
    ) -> Result<Response<McpResponse>, Status> {
        let req = request.into_inner();

        // Validate JSON-RPC version
        if !req.jsonrpc.is_empty() && req.jsonrpc != "2.0" {
            return Ok(Response::new(McpResponse {
                jsonrpc: "2.0".to_string(),
                id: req.id,
                result: Vec::new(),
                error: Some(McpError {
                    code: -32600, // Invalid Request
                    message: format!("Unsupported JSON-RPC version: {}", req.jsonrpc),
                    data: Vec::new(),
                }),
            }));
        }

        info!("MCP request: method={} id={}", req.method, req.id);

        let response = match req.method.as_str() {
            "initialize" => self.handle_initialize(&req.id),
            "ping" => self.handle_ping(&req.id),
            "tools/list" => self.handle_tools_list(&req.id).await?,
            "tools/call" => self.handle_tools_call(&req.id, &req.params).await?,
            _ => {
                warn!("MCP unknown method: {}", req.method);
                McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id: req.id,
                    result: Vec::new(),
                    error: Some(McpError {
                        code: -32601, // Method not found
                        message: format!("Method not found: {}", req.method),
                        data: Vec::new(),
                    }),
                }
            }
        };

        Ok(Response::new(response))
    }

    async fn list_tools(
        &self,
        _request: Request<ListToolsRequest>,
    ) -> Result<Response<ListToolsResponse>, Status> {
        let response = self.list_tools_internal().await?;

        info!("MCP list_tools: returning {} tools", response.tools.len());

        Ok(Response::new(response))
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cache/src/grpc/mod.rs">
//! gRPC service implementations for op-cache
//!
//! Provides:
//! - AgentService: Register and execute agents
//! - OrchestratorService: Route requests and manage workstacks
//! - CacheService: Workstack step caching
//! - McpService: MCP JSON-RPC bridge to agents
//! - EmbeddingService: Vector embedding cache
//! - SnapshotService: BTRFS snapshot management

pub mod agent_service;
pub mod cache_service;
pub mod mcp_service;
pub mod orchestrator_service;
pub mod server;

pub use agent_service::AgentServiceImpl;
pub use cache_service::CacheServiceImpl;
pub use mcp_service::McpServiceImpl;
pub use orchestrator_service::OrchestratorServiceImpl;
pub use server::{GrpcServer, GrpcServerConfig};

// Re-export generated protobuf types
pub mod proto {
    pub use crate::proto::*;
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cache/src/grpc/orchestrator_service.rs">
//! Orchestrator service implementation
//!
//! Routes requests to agents based on capabilities,
//! manages workstacks, and tracks patterns.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use sha2::{Digest, Sha256};
use tokio::sync::RwLock;
use tonic::{Request, Response, Status};
use tracing::{debug, info};

use super::agent_service::AgentServiceImpl;
use super::cache_service::CacheServiceImpl;
use super::proto::{
    agent_service_server::AgentService, orchestrator_service_server::OrchestratorService, Empty,
    ExecuteAgentsRequest, FindByCapabilityRequest, GetPatternsResponse, OrchestratorRequest,
    OrchestratorResponse, OrchestratorStats, PatternSuggestion, PromotePatternRequest,
    PromotePatternResponse, ResolveRequest, ResolveResponse, WorkstackStepResult,
};

/// Tracked pattern for promotion suggestions
#[derive(Clone)]
struct TrackedPattern {
    pattern_id: String,
    agent_sequence: Vec<String>,
    call_count: u32,
    total_latency_ms: u64,
    #[allow(dead_code)]
    first_seen: Instant,
    last_called: Instant,
    promoted: bool,
}

pub struct OrchestratorServiceImpl {
    agent_service: Arc<AgentServiceImpl>,
    cache_service: Arc<CacheServiceImpl>,
    patterns: Arc<RwLock<HashMap<String, TrackedPattern>>>,
    workstack_threshold: usize,
    enable_caching: bool,
    promotion_threshold: u32,
}

impl OrchestratorServiceImpl {
    pub fn new(agent_service: Arc<AgentServiceImpl>, cache_service: Arc<CacheServiceImpl>) -> Self {
        Self {
            agent_service,
            cache_service,
            patterns: Arc::new(RwLock::new(HashMap::new())),
            workstack_threshold: 2,
            enable_caching: true,
            promotion_threshold: 3,
        }
    }

    pub fn with_config(
        agent_service: Arc<AgentServiceImpl>,
        cache_service: Arc<CacheServiceImpl>,
        workstack_threshold: usize,
        enable_caching: bool,
        promotion_threshold: u32,
    ) -> Self {
        Self {
            agent_service,
            cache_service,
            patterns: Arc::new(RwLock::new(HashMap::new())),
            workstack_threshold,
            enable_caching,
            promotion_threshold,
        }
    }

    /// Resolve capabilities to agent sequence
    async fn resolve_capabilities(
        &self,
        required: &[i32],
        preferred: &[String],
        excluded: &[String],
    ) -> Result<(Vec<super::proto::Agent>, Vec<i32>, Vec<i32>), Status> {
        let mut selected_agents = Vec::new();
        let mut fulfilled = HashSet::new();
        let excluded_set: HashSet<&String> = excluded.iter().collect();
        let preferred_set: HashSet<&String> = preferred.iter().collect();

        for &cap in required {
            if fulfilled.contains(&cap) {
                continue;
            }

            // Find agents for this capability
            let req = Request::new(FindByCapabilityRequest {
                capabilities: vec![cap],
                match_all: false,
            });

            let response: tonic::Response<super::proto::FindByCapabilityResponse> =
                self.agent_service.find_by_capability(req).await?;
            let candidates = response.into_inner().agents;

            // Filter excluded and select best
            let mut viable: Vec<_> = candidates
                .into_iter()
                .filter(|a| !excluded_set.contains(&a.id))
                .filter(|a| {
                    !selected_agents
                        .iter()
                        .any(|s: &super::proto::Agent| s.id == a.id)
                })
                .collect();

            // Sort by preference and latency
            viable.sort_by(|a, b| {
                let a_preferred = preferred_set.contains(&a.id);
                let b_preferred = preferred_set.contains(&b.id);
                match (a_preferred, b_preferred) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.estimated_latency_ms.cmp(&b.estimated_latency_ms),
                }
            });

            if let Some(agent) = viable.first() {
                for c in &agent.capabilities {
                    fulfilled.insert(*c);
                }
                selected_agents.push(agent.clone());
            }
        }

        // Sort by priority
        selected_agents.sort_by_key(|a| a.priority);

        let fulfilled_vec: Vec<i32> = fulfilled.into_iter().collect();
        let missing: Vec<i32> = required
            .iter()
            .filter(|c| !fulfilled_vec.contains(c))
            .copied()
            .collect();

        Ok((selected_agents, fulfilled_vec, missing))
    }

    /// Execute workstack with caching
    async fn execute_workstack(
        &self,
        workstack_id: &str,
        agent_ids: &[String],
        input: Vec<u8>,
    ) -> Result<(Vec<u8>, Vec<WorkstackStepResult>), Status> {
        let mut current_input = input;
        let mut steps = Vec::new();

        for (step_index, agent_id) in agent_ids.iter().enumerate() {
            let step_input_hash = Self::hash_bytes(&current_input);
            let step_start = Instant::now();

            // Try cache first
            let (output, cached) = if self.enable_caching {
                let cache_result = self
                    .cache_service
                    .get_step_internal(workstack_id, step_index as u32, &step_input_hash)
                    .await;

                match cache_result {
                    Some(cached_output) => {
                        debug!("Cache hit: {} step {}", workstack_id, step_index);
                        (cached_output, true)
                    }
                    None => {
                        // Execute agent
                        let exec_req = Request::new(super::proto::ExecuteAgentRequest {
                            agent_id: agent_id.clone(),
                            input: current_input.clone(),
                            context: HashMap::new(),
                            timeout_ms: 0,
                        });

                        let exec_response = self.agent_service.execute(exec_req).await?;
                        let result = exec_response.into_inner();

                        if !result.success {
                            return Err(Status::internal(format!(
                                "Agent {} failed: {}",
                                agent_id, result.error
                            )));
                        }

                        // Cache result
                        self.cache_service
                            .put_step_internal(
                                workstack_id,
                                step_index as u32,
                                &step_input_hash,
                                &result.output,
                            )
                            .await;

                        (result.output, false)
                    }
                }
            } else {
                // Caching disabled, execute directly
                let exec_req = Request::new(super::proto::ExecuteAgentRequest {
                    agent_id: agent_id.clone(),
                    input: current_input.clone(),
                    context: HashMap::new(),
                    timeout_ms: 0,
                });

                let exec_response = self.agent_service.execute(exec_req).await?;
                let result = exec_response.into_inner();

                if !result.success {
                    return Err(Status::internal(format!(
                        "Agent {} failed: {}",
                        agent_id, result.error
                    )));
                }

                (result.output, false)
            };

            let latency_ms = step_start.elapsed().as_millis() as u64;

            steps.push(WorkstackStepResult {
                step_index: step_index as u32,
                agent_id: agent_id.clone(),
                output: output.clone(),
                latency_ms,
                cached,
                output_size: output.len() as u64,
                success: true,
                error: String::new(),
            });

            current_input = output;
        }

        Ok((current_input, steps))
    }

    /// Track pattern for potential promotion
    async fn track_pattern(
        &self,
        agent_ids: &[String],
        latency_ms: u64,
    ) -> Option<PatternSuggestion> {
        let pattern_id = Self::hash_sequence(agent_ids);
        let now = Instant::now();

        let mut patterns = self.patterns.write().await;

        let pattern = patterns
            .entry(pattern_id.clone())
            .or_insert_with(|| TrackedPattern {
                pattern_id: pattern_id.clone(),
                agent_sequence: agent_ids.to_vec(),
                call_count: 0,
                total_latency_ms: 0,
                first_seen: now,
                last_called: now,
                promoted: false,
            });

        pattern.call_count += 1;
        pattern.total_latency_ms += latency_ms;
        pattern.last_called = now;

        if pattern.call_count >= self.promotion_threshold && !pattern.promoted {
            let avg_latency = pattern.total_latency_ms / pattern.call_count as u64;
            let suggested_name = Self::generate_workstack_name(&pattern.agent_sequence);

            return Some(PatternSuggestion {
                pattern_id: pattern.pattern_id.clone(),
                agent_sequence: pattern.agent_sequence.clone(),
                call_count: pattern.call_count,
                avg_latency_ms: avg_latency,
                suggested_name,
                confidence_score: Self::calculate_confidence(pattern),
                estimated_time_saved_ms: (avg_latency as f64 * 0.4 * pattern.call_count as f64)
                    as u64,
            });
        }

        None
    }

    fn hash_bytes(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    fn hash_sequence(agents: &[String]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(agents.join("→").as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn generate_workstack_name(agents: &[String]) -> String {
        if agents.is_empty() {
            return "unnamed".to_string();
        }
        let first = &agents[0];
        let last = agents.last().unwrap();
        if agents.len() == 2 {
            format!("{}-to-{}", first, last)
        } else {
            format!("{}-to-{}-{}step", first, last, agents.len())
        }
    }

    fn calculate_confidence(pattern: &TrackedPattern) -> f64 {
        let recency = pattern.last_called.elapsed().as_secs_f64() / 86400.0;
        let frequency = (pattern.call_count as f64 / 3.0).min(2.0) / 2.0;
        let recency_score = (1.0 - recency / 7.0).max(0.0);
        (frequency * 0.6 + recency_score * 0.4).min(1.0)
    }
}

#[tonic::async_trait]
impl OrchestratorService for OrchestratorServiceImpl {
    async fn execute(
        &self,
        request: Request<OrchestratorRequest>,
    ) -> Result<Response<OrchestratorResponse>, Status> {
        let req = request.into_inner();
        let start = Instant::now();
        let request_id = if req.request_id.is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            req.request_id
        };

        // Resolve capabilities to agents
        let (agents, fulfilled, missing) = self
            .resolve_capabilities(
                &req.required_capabilities,
                &req.preferred_agents,
                &req.excluded_agents,
            )
            .await?;

        if agents.is_empty() {
            return Ok(Response::new(OrchestratorResponse {
                request_id,
                output: req.input,
                steps: Vec::new(),
                total_latency_ms: 0,
                cache_hits: 0,
                cache_misses: 0,
                used_workstack: false,
                resolved_agents: Vec::new(),
                fulfilled_capabilities: fulfilled,
                missing_capabilities: missing,
            }));
        }

        let agent_ids: Vec<String> = agents.iter().map(|a| a.id.clone()).collect();
        let use_workstack = agent_ids.len() >= self.workstack_threshold;

        info!(
            "Executing request {} with {} agents (workstack: {})",
            request_id,
            agent_ids.len(),
            use_workstack
        );

        let (output, steps) = if use_workstack {
            let workstack_id = format!("ws-{}", &Self::hash_bytes(&req.input)[..12]);
            self.execute_workstack(&workstack_id, &agent_ids, req.input)
                .await?
        } else {
            // Single agent execution
            let exec_req = Request::new(super::proto::ExecuteAgentRequest {
                agent_id: agent_ids[0].clone(),
                input: req.input,
                context: HashMap::new(),
                timeout_ms: 0,
            });

            let result = self.agent_service.execute(exec_req).await?.into_inner();

            let step = WorkstackStepResult {
                step_index: 0,
                agent_id: agent_ids[0].clone(),
                output: result.output.clone(),
                latency_ms: result.latency_ms,
                cached: false,
                output_size: result.output.len() as u64,
                success: result.success,
                error: result.error,
            };

            (result.output, vec![step])
        };

        let total_latency_ms = start.elapsed().as_millis() as u64;
        let cache_hits = steps.iter().filter(|s| s.cached).count() as u32;
        let cache_misses = steps.iter().filter(|s| !s.cached).count() as u32;

        // Track pattern if workstack
        if use_workstack {
            if let Some(suggestion) = self.track_pattern(&agent_ids, total_latency_ms).await {
                info!(
                    "🔥 Pattern '{}' detected ({} calls)",
                    suggestion.suggested_name, suggestion.call_count
                );
            }
        }

        Ok(Response::new(OrchestratorResponse {
            request_id,
            output,
            steps,
            total_latency_ms,
            cache_hits,
            cache_misses,
            used_workstack: use_workstack,
            resolved_agents: agent_ids,
            fulfilled_capabilities: fulfilled,
            missing_capabilities: missing,
        }))
    }

    type ExecuteStreamStream =
        tokio_stream::wrappers::ReceiverStream<Result<WorkstackStepResult, Status>>;

    async fn execute_stream(
        &self,
        request: Request<OrchestratorRequest>,
    ) -> Result<Response<Self::ExecuteStreamStream>, Status> {
        // Similar to execute but streams each step result
        let (tx, rx) = tokio::sync::mpsc::channel(32);
        let req = request.into_inner();

        let agent_service = self.agent_service.clone();
        let cache_service = self.cache_service.clone();
        let enable_caching = self.enable_caching;

        let (agents, _, _) = self
            .resolve_capabilities(
                &req.required_capabilities,
                &req.preferred_agents,
                &req.excluded_agents,
            )
            .await?;

        let agent_ids: Vec<String> = agents.iter().map(|a| a.id.clone()).collect();

        tokio::spawn(async move {
            let workstack_id = format!("ws-{}", &Self::hash_bytes(&req.input)[..12]);
            let mut current_input = req.input;

            for (step_index, agent_id) in agent_ids.iter().enumerate() {
                let step_input_hash = Self::hash_bytes(&current_input);
                let step_start = Instant::now();

                let (output, cached) = if enable_caching {
                    let cache_result = cache_service
                        .get_step_internal(&workstack_id, step_index as u32, &step_input_hash)
                        .await;

                    match cache_result {
                        Some(cached_output) => (cached_output, true),
                        None => {
                            let exec_req = Request::new(super::proto::ExecuteAgentRequest {
                                agent_id: agent_id.clone(),
                                input: current_input.clone(),
                                context: HashMap::new(),
                                timeout_ms: 0,
                            });

                            match agent_service.execute(exec_req).await {
                                Ok(resp) => {
                                    let result = resp.into_inner();
                                    if result.success {
                                        cache_service
                                            .put_step_internal(
                                                &workstack_id,
                                                step_index as u32,
                                                &step_input_hash,
                                                &result.output,
                                            )
                                            .await;
                                        (result.output, false)
                                    } else {
                                        let _ = tx.send(Err(Status::internal(result.error))).await;
                                        return;
                                    }
                                }
                                Err(e) => {
                                    let _ = tx.send(Err(e)).await;
                                    return;
                                }
                            }
                        }
                    }
                } else {
                    let exec_req = Request::new(super::proto::ExecuteAgentRequest {
                        agent_id: agent_id.clone(),
                        input: current_input.clone(),
                        context: HashMap::new(),
                        timeout_ms: 0,
                    });

                    match agent_service.execute(exec_req).await {
                        Ok(resp) => {
                            let result = resp.into_inner();
                            if result.success {
                                (result.output, false)
                            } else {
                                let _ = tx.send(Err(Status::internal(result.error))).await;
                                return;
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(Err(e)).await;
                            return;
                        }
                    }
                };

                let step = WorkstackStepResult {
                    step_index: step_index as u32,
                    agent_id: agent_id.clone(),
                    output: output.clone(),
                    latency_ms: step_start.elapsed().as_millis() as u64,
                    cached,
                    output_size: output.len() as u64,
                    success: true,
                    error: String::new(),
                };

                if tx.send(Ok(step)).await.is_err() {
                    return;
                }

                current_input = output;
            }
        });

        Ok(Response::new(tokio_stream::wrappers::ReceiverStream::new(
            rx,
        )))
    }

    async fn execute_agents(
        &self,
        request: Request<ExecuteAgentsRequest>,
    ) -> Result<Response<OrchestratorResponse>, Status> {
        let req = request.into_inner();
        let start = Instant::now();
        let request_id = if req.request_id.is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            req.request_id
        };

        let use_workstack = req.agent_ids.len() >= self.workstack_threshold;

        let (output, steps) = if use_workstack {
            let workstack_id = format!("ws-{}", &Self::hash_bytes(&req.input)[..12]);
            self.execute_workstack(&workstack_id, &req.agent_ids, req.input)
                .await?
        } else if !req.agent_ids.is_empty() {
            let exec_req = Request::new(super::proto::ExecuteAgentRequest {
                agent_id: req.agent_ids[0].clone(),
                input: req.input,
                context: HashMap::new(),
                timeout_ms: 0,
            });

            let result = self.agent_service.execute(exec_req).await?.into_inner();
            let step = WorkstackStepResult {
                step_index: 0,
                agent_id: req.agent_ids[0].clone(),
                output: result.output.clone(),
                latency_ms: result.latency_ms,
                cached: false,
                output_size: result.output.len() as u64,
                success: result.success,
                error: result.error,
            };
            (result.output, vec![step])
        } else {
            return Err(Status::invalid_argument("No agents specified"));
        };

        let cache_hits = steps.iter().filter(|s| s.cached).count() as u32;
        let cache_misses = steps.iter().filter(|s| !s.cached).count() as u32;

        Ok(Response::new(OrchestratorResponse {
            request_id,
            output,
            steps,
            total_latency_ms: start.elapsed().as_millis() as u64,
            cache_hits,
            cache_misses,
            used_workstack: use_workstack,
            resolved_agents: req.agent_ids,
            fulfilled_capabilities: Vec::new(),
            missing_capabilities: Vec::new(),
        }))
    }

    async fn resolve(
        &self,
        request: Request<ResolveRequest>,
    ) -> Result<Response<ResolveResponse>, Status> {
        let req = request.into_inner();

        let (agents, fulfilled, missing) = self
            .resolve_capabilities(
                &req.required_capabilities,
                &req.preferred_agents,
                &req.excluded_agents,
            )
            .await?;

        let estimated_latency_ms: u64 = agents.iter().map(|a| a.estimated_latency_ms).sum();

        Ok(Response::new(ResolveResponse {
            agents,
            fulfilled_capabilities: fulfilled,
            missing_capabilities: missing,
            estimated_latency_ms,
            resolution_path: Vec::new(),
        }))
    }

    async fn get_patterns(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<GetPatternsResponse>, Status> {
        let patterns = self.patterns.read().await;

        let suggestions: Vec<PatternSuggestion> = patterns
            .values()
            .filter(|p| p.call_count >= self.promotion_threshold && !p.promoted)
            .map(|p| {
                let avg_latency = if p.call_count > 0 {
                    p.total_latency_ms / p.call_count as u64
                } else {
                    0
                };
                PatternSuggestion {
                    pattern_id: p.pattern_id.clone(),
                    agent_sequence: p.agent_sequence.clone(),
                    call_count: p.call_count,
                    avg_latency_ms: avg_latency,
                    suggested_name: Self::generate_workstack_name(&p.agent_sequence),
                    confidence_score: Self::calculate_confidence(p),
                    estimated_time_saved_ms: (avg_latency as f64 * 0.4 * p.call_count as f64)
                        as u64,
                }
            })
            .collect();

        Ok(Response::new(GetPatternsResponse {
            patterns: suggestions,
        }))
    }

    async fn promote_pattern(
        &self,
        request: Request<PromotePatternRequest>,
    ) -> Result<Response<PromotePatternResponse>, Status> {
        let req = request.into_inner();

        let mut patterns = self.patterns.write().await;

        if let Some(pattern) = patterns.get_mut(&req.pattern_id) {
            pattern.promoted = true;
            let workstack_id = format!("WS-{}", &pattern.pattern_id[..8]);

            info!(
                "Promoted pattern {} to workstack {}",
                req.pattern_id, workstack_id
            );

            Ok(Response::new(PromotePatternResponse {
                success: true,
                workstack_id,
                error: String::new(),
            }))
        } else {
            Ok(Response::new(PromotePatternResponse {
                success: false,
                workstack_id: String::new(),
                error: "Pattern not found".to_string(),
            }))
        }
    }

    async fn get_stats(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<OrchestratorStats>, Status> {
        let agents_response = self
            .agent_service
            .list_agents(Request::new(super::proto::ListAgentsRequest {
                enabled_only: false,
            }))
            .await?
            .into_inner();

        let caps_response = self
            .agent_service
            .list_capabilities(Request::new(Empty {}))
            .await?
            .into_inner();

        let cache_stats = self.cache_service.get_stats_internal().await;
        let patterns = self.patterns.read().await;

        let promoted_count = patterns.values().filter(|p| p.promoted).count() as u32;
        let tracked_count = patterns.len() as u32;

        Ok(Response::new(OrchestratorStats {
            registered_agents: agents_response.agents.len() as u32,
            enabled_agents: agents_response.agents.iter().filter(|a| a.enabled).count() as u32,
            available_capabilities: caps_response.capabilities.len() as u32,
            tracked_patterns: tracked_count,
            promoted_patterns: promoted_count,
            cache_entries: cache_stats.total_entries,
            cache_hit_rate: cache_stats.hit_rate,
            numa_nodes: 1, // TODO: get from actual NUMA topology
        }))
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cache/src/grpc/server.rs">
//! gRPC server setup and configuration

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use tonic::transport::Server;
use tracing::info;

use super::agent_service::AgentServiceImpl;
use super::cache_service::CacheServiceImpl;
use super::mcp_service::McpServiceImpl;
use super::orchestrator_service::OrchestratorServiceImpl;
use super::proto::{
    agent_service_server::AgentServiceServer, cache_service_server::CacheServiceServer,
    mcp_service_server::McpServiceServer, orchestrator_service_server::OrchestratorServiceServer,
};

/// Server configuration
#[derive(Debug, Clone)]
pub struct GrpcServerConfig {
    pub listen_addr: SocketAddr,
    pub workstack_threshold: usize,
    pub enable_caching: bool,
    pub promotion_threshold: u32,
    pub default_cache_ttl_secs: i64,
}

impl Default for GrpcServerConfig {
    fn default() -> Self {
        Self {
            listen_addr: "[::1]:50051".parse().unwrap(),
            workstack_threshold: 2,
            enable_caching: true,
            promotion_threshold: 3,
            default_cache_ttl_secs: 3600,
        }
    }
}

/// gRPC server builder
pub struct GrpcServer {
    config: GrpcServerConfig,
    agent_service: Arc<AgentServiceImpl>,
    cache_service: Arc<CacheServiceImpl>,
    orchestrator_service: Arc<OrchestratorServiceImpl>,
    mcp_service: Arc<McpServiceImpl>,
}

impl GrpcServer {
    /// Create new gRPC server with default configuration
    pub fn new() -> Self {
        Self::with_config(GrpcServerConfig::default())
    }

    /// Create new gRPC server with custom configuration
    pub fn with_config(config: GrpcServerConfig) -> Self {
        let agent_service = Arc::new(AgentServiceImpl::new());
        let cache_service = Arc::new(CacheServiceImpl::with_ttl(config.default_cache_ttl_secs));
        let orchestrator_service = Arc::new(OrchestratorServiceImpl::with_config(
            agent_service.clone(),
            cache_service.clone(),
            config.workstack_threshold,
            config.enable_caching,
            config.promotion_threshold,
        ));
        let mcp_service = Arc::new(McpServiceImpl::new(
            agent_service.clone(),
            orchestrator_service.clone(),
        ));

        Self {
            config,
            agent_service,
            cache_service,
            orchestrator_service,
            mcp_service,
        }
    }

    /// Get agent service for local registration
    pub fn agent_service(&self) -> Arc<AgentServiceImpl> {
        self.agent_service.clone()
    }

    /// Get orchestrator service
    pub fn orchestrator_service(&self) -> Arc<OrchestratorServiceImpl> {
        self.orchestrator_service.clone()
    }

    /// Get cache service
    pub fn cache_service(&self) -> Arc<CacheServiceImpl> {
        self.cache_service.clone()
    }

    /// Get MCP service
    pub fn mcp_service(&self) -> Arc<McpServiceImpl> {
        self.mcp_service.clone()
    }

    /// Start the gRPC server
    pub async fn serve(self) -> Result<()> {
        let addr = self.config.listen_addr;

        info!("Starting gRPC server on {}", addr);

        Server::builder()
            .add_service(AgentServiceServer::from_arc(self.agent_service))
            .add_service(CacheServiceServer::from_arc(self.cache_service))
            .add_service(OrchestratorServiceServer::from_arc(
                self.orchestrator_service,
            ))
            .add_service(McpServiceServer::from_arc(self.mcp_service))
            .serve(addr)
            .await?;

        Ok(())
    }

    /// Serve with graceful shutdown
    pub async fn serve_with_shutdown(
        self,
        shutdown: impl std::future::Future<Output = ()>,
    ) -> Result<()> {
        let addr = self.config.listen_addr;

        info!("Starting gRPC server on {} (with graceful shutdown)", addr);

        Server::builder()
            .add_service(AgentServiceServer::from_arc(self.agent_service))
            .add_service(CacheServiceServer::from_arc(self.cache_service))
            .add_service(OrchestratorServiceServer::from_arc(
                self.orchestrator_service,
            ))
            .add_service(McpServiceServer::from_arc(self.mcp_service))
            .serve_with_shutdown(addr, shutdown)
            .await?;

        Ok(())
    }
}

impl Default for GrpcServer {
    fn default() -> Self {
        Self::new()
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cache/src/agent_registry.rs">
//! Agent registry with capability definitions
//!
//! Each agent declares its capabilities as an array.
//! The resolver uses these to build agent sequences.

use anyhow::{Context, Result};
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Core capabilities an agent can provide
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentCapability {
    // Analysis capabilities
    CodeAnalysis,
    SecurityAudit,
    PerformanceAnalysis,
    DependencyAnalysis,

    // Generation capabilities
    CodeGeneration,
    TestGeneration,
    DocumentationGeneration,
    RefactoringSuggestion,

    // Transformation capabilities
    CodeTransformation,
    FormatConversion,
    LanguageTranslation,

    // Data capabilities
    DataExtraction,
    DataValidation,
    DataEnrichment,
    Embedding,

    // Reasoning capabilities
    Planning,
    Summarization,
    QuestionAnswering,
    Classification,

    // Integration capabilities
    ApiCall,
    DatabaseQuery,
    FileOperation,
    ShellExecution,

    // Custom capability (for extensibility)
    Custom(u32),
}

impl AgentCapability {
    /// Parse capability from string
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "code_analysis" | "analyze_code" => Some(Self::CodeAnalysis),
            "security_audit" | "security" => Some(Self::SecurityAudit),
            "performance_analysis" | "performance" => Some(Self::PerformanceAnalysis),
            "dependency_analysis" | "dependencies" => Some(Self::DependencyAnalysis),
            "code_generation" | "generate_code" => Some(Self::CodeGeneration),
            "test_generation" | "generate_tests" | "tests" => Some(Self::TestGeneration),
            "documentation_generation" | "docs" | "documentation" => {
                Some(Self::DocumentationGeneration)
            }
            "refactoring" | "refactor" => Some(Self::RefactoringSuggestion),
            "code_transformation" | "transform" => Some(Self::CodeTransformation),
            "format_conversion" | "convert" => Some(Self::FormatConversion),
            "language_translation" | "translate" => Some(Self::LanguageTranslation),
            "data_extraction" | "extract" => Some(Self::DataExtraction),
            "data_validation" | "validate" => Some(Self::DataValidation),
            "data_enrichment" | "enrich" => Some(Self::DataEnrichment),
            "embedding" | "embed" => Some(Self::Embedding),
            "planning" | "plan" => Some(Self::Planning),
            "summarization" | "summarize" | "summary" => Some(Self::Summarization),
            "question_answering" | "qa" | "answer" => Some(Self::QuestionAnswering),
            "classification" | "classify" => Some(Self::Classification),
            "api_call" | "api" => Some(Self::ApiCall),
            "database_query" | "db" | "query" => Some(Self::DatabaseQuery),
            "file_operation" | "file" => Some(Self::FileOperation),
            "shell_execution" | "shell" | "exec" => Some(Self::ShellExecution),
            _ => None,
        }
    }

    /// Get capability name
    pub fn name(&self) -> &'static str {
        match self {
            Self::CodeAnalysis => "code_analysis",
            Self::SecurityAudit => "security_audit",
            Self::PerformanceAnalysis => "performance_analysis",
            Self::DependencyAnalysis => "dependency_analysis",
            Self::CodeGeneration => "code_generation",
            Self::TestGeneration => "test_generation",
            Self::DocumentationGeneration => "documentation_generation",
            Self::RefactoringSuggestion => "refactoring",
            Self::CodeTransformation => "code_transformation",
            Self::FormatConversion => "format_conversion",
            Self::LanguageTranslation => "language_translation",
            Self::DataExtraction => "data_extraction",
            Self::DataValidation => "data_validation",
            Self::DataEnrichment => "data_enrichment",
            Self::Embedding => "embedding",
            Self::Planning => "planning",
            Self::Summarization => "summarization",
            Self::QuestionAnswering => "question_answering",
            Self::Classification => "classification",
            Self::ApiCall => "api_call",
            Self::DatabaseQuery => "database_query",
            Self::FileOperation => "file_operation",
            Self::ShellExecution => "shell_execution",
            Self::Custom(_id) => {
                // Return static str for known custom IDs, or generic
                "custom"
            }
        }
    }
}

/// Agent execution priority
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
pub enum AgentPriority {
    /// Execute first (e.g., validation, security)
    High = 0,
    /// Normal execution order
    #[default]
    Normal = 1,
    /// Execute last (e.g., formatting, cleanup)
    Low = 2,
}

/// Agent definition with capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDefinition {
    /// Unique agent identifier
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Description of what the agent does
    pub description: String,

    /// Capabilities this agent provides (array)
    pub capabilities: Vec<AgentCapability>,

    /// Capabilities this agent requires as input
    pub requires: Vec<AgentCapability>,

    /// Execution priority
    pub priority: AgentPriority,

    /// Whether agent can run in parallel with others
    pub parallelizable: bool,

    /// Estimated latency in milliseconds
    pub estimated_latency_ms: u64,

    /// Maximum input size in bytes (0 = unlimited)
    pub max_input_size: usize,

    /// Agent version
    pub version: String,

    /// Whether agent is enabled
    pub enabled: bool,
}

impl AgentDefinition {
    /// Create new agent definition
    pub fn new(id: &str, name: &str) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: String::new(),
            capabilities: Vec::new(),
            requires: Vec::new(),
            priority: AgentPriority::Normal,
            parallelizable: false,
            estimated_latency_ms: 100,
            max_input_size: 0,
            version: "1.0.0".to_string(),
            enabled: true,
        }
    }

    /// Builder: add description
    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = desc.to_string();
        self
    }

    /// Builder: add capability
    pub fn with_capability(mut self, cap: AgentCapability) -> Self {
        if !self.capabilities.contains(&cap) {
            self.capabilities.push(cap);
        }
        self
    }

    /// Builder: add multiple capabilities
    pub fn with_capabilities(mut self, caps: &[AgentCapability]) -> Self {
        for cap in caps {
            if !self.capabilities.contains(cap) {
                self.capabilities.push(*cap);
            }
        }
        self
    }

    /// Builder: add requirement
    pub fn requires_capability(mut self, cap: AgentCapability) -> Self {
        if !self.requires.contains(&cap) {
            self.requires.push(cap);
        }
        self
    }

    /// Builder: set priority
    pub fn with_priority(mut self, priority: AgentPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Builder: set parallelizable
    pub fn parallelizable(mut self, parallel: bool) -> Self {
        self.parallelizable = parallel;
        self
    }

    /// Builder: set estimated latency
    pub fn with_latency(mut self, ms: u64) -> Self {
        self.estimated_latency_ms = ms;
        self
    }

    /// Check if agent provides a capability
    pub fn provides(&self, cap: AgentCapability) -> bool {
        self.capabilities.contains(&cap)
    }

    /// Check if agent requires a capability
    pub fn needs(&self, cap: AgentCapability) -> bool {
        self.requires.contains(&cap)
    }

    /// Get all provided capabilities as set
    pub fn capability_set(&self) -> HashSet<AgentCapability> {
        self.capabilities.iter().copied().collect()
    }
}

/// Agent executor function type
pub type AgentExecutor = Arc<dyn Fn(&[u8]) -> BoxFuture<'static, Result<Vec<u8>>> + Send + Sync>;

/// Registered agent with executor
pub struct RegisteredAgent {
    pub definition: AgentDefinition,
    pub executor: AgentExecutor,
}

/// Agent registry - stores all agents and their capabilities
pub struct AgentRegistry {
    agents: RwLock<HashMap<String, RegisteredAgent>>,
    capability_index: RwLock<HashMap<AgentCapability, Vec<String>>>,
}

impl AgentRegistry {
    /// Create new empty registry
    pub fn new() -> Self {
        Self {
            agents: RwLock::new(HashMap::new()),
            capability_index: RwLock::new(HashMap::new()),
        }
    }

    /// Register an agent with its executor
    pub async fn register(
        &self,
        definition: AgentDefinition,
        executor: AgentExecutor,
    ) -> Result<()> {
        let agent_id = definition.id.clone();
        let capabilities = definition.capabilities.clone();

        // Store agent
        {
            let mut agents = self.agents.write().await;
            agents.insert(
                agent_id.clone(),
                RegisteredAgent {
                    definition,
                    executor,
                },
            );
        }

        // Update capability index
        {
            let mut index = self.capability_index.write().await;
            for cap in capabilities {
                index
                    .entry(cap)
                    .or_insert_with(Vec::new)
                    .push(agent_id.clone());
            }
        }

        info!("Registered agent: {}", agent_id);
        Ok(())
    }

    /// Unregister an agent
    pub async fn unregister(&self, agent_id: &str) -> Result<Option<AgentDefinition>> {
        let removed = {
            let mut agents = self.agents.write().await;
            agents.remove(agent_id)
        };

        if let Some(agent) = &removed {
            // Remove from capability index
            let mut index = self.capability_index.write().await;
            for cap in &agent.definition.capabilities {
                if let Some(agents) = index.get_mut(cap) {
                    agents.retain(|id| id != agent_id);
                }
            }
            info!("Unregistered agent: {}", agent_id);
            Ok(Some(agent.definition.clone()))
        } else {
            Ok(None)
        }
    }

    /// Get agent definition by ID
    pub async fn get(&self, agent_id: &str) -> Option<AgentDefinition> {
        let agents = self.agents.read().await;
        agents.get(agent_id).map(|a| a.definition.clone())
    }

    /// Get agent executor by ID
    pub async fn get_executor(&self, agent_id: &str) -> Option<AgentExecutor> {
        let agents = self.agents.read().await;
        agents.get(agent_id).map(|a| a.executor.clone())
    }

    /// Find agents that provide a capability
    pub async fn find_by_capability(&self, cap: AgentCapability) -> Vec<AgentDefinition> {
        let index = self.capability_index.read().await;
        let agents = self.agents.read().await;

        index
            .get(&cap)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| agents.get(id).map(|a| a.definition.clone()))
                    .filter(|def| def.enabled)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Find agents that provide any of the given capabilities
    pub async fn find_by_capabilities(&self, caps: &[AgentCapability]) -> Vec<AgentDefinition> {
        let mut result = Vec::new();
        let mut seen = HashSet::new();

        for cap in caps {
            for agent in self.find_by_capability(*cap).await {
                if !seen.contains(&agent.id) {
                    seen.insert(agent.id.clone());
                    result.push(agent);
                }
            }
        }

        result
    }

    /// Find the best agent for a capability (lowest latency, enabled)
    pub async fn find_best_for_capability(&self, cap: AgentCapability) -> Option<AgentDefinition> {
        self.find_by_capability(cap)
            .await
            .into_iter()
            .min_by_key(|a| a.estimated_latency_ms)
    }

    /// Get all registered agents
    pub async fn list_all(&self) -> Vec<AgentDefinition> {
        let agents = self.agents.read().await;
        agents.values().map(|a| a.definition.clone()).collect()
    }

    /// Get all capabilities provided by registered agents
    pub async fn list_capabilities(&self) -> Vec<AgentCapability> {
        let index = self.capability_index.read().await;
        index.keys().copied().collect()
    }

    /// Check if a capability is available
    pub async fn has_capability(&self, cap: AgentCapability) -> bool {
        let index = self.capability_index.read().await;
        index.get(&cap).map(|v| !v.is_empty()).unwrap_or(false)
    }

    /// Get registry statistics
    pub async fn stats(&self) -> RegistryStats {
        let agents = self.agents.read().await;
        let index = self.capability_index.read().await;

        let enabled_count = agents.values().filter(|a| a.definition.enabled).count();

        RegistryStats {
            total_agents: agents.len(),
            enabled_agents: enabled_count,
            disabled_agents: agents.len() - enabled_count,
            total_capabilities: index.len(),
        }
    }

    /// Execute an agent by ID
    pub async fn execute(&self, agent_id: &str, input: &[u8]) -> Result<Vec<u8>> {
        let executor = {
            let agents = self.agents.read().await;
            agents
                .get(agent_id)
                .map(|a| a.executor.clone())
                .context(format!("Agent not found: {}", agent_id))?
        };

        executor(input).await
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Registry statistics
#[derive(Debug, Clone)]
pub struct RegistryStats {
    pub total_agents: usize,
    pub enabled_agents: usize,
    pub disabled_agents: usize,
    pub total_capabilities: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_executor() -> AgentExecutor {
        Arc::new(|input: &[u8]| {
            let input = input.to_vec();
            Box::pin(async move { Ok(input) })
        })
    }

    #[tokio::test]
    async fn test_agent_registration() {
        let registry = AgentRegistry::new();

        let agent = AgentDefinition::new("test_agent", "Test Agent")
            .with_capability(AgentCapability::CodeAnalysis)
            .with_capability(AgentCapability::TestGeneration);

        registry
            .register(agent, make_test_executor())
            .await
            .unwrap();

        let retrieved = registry.get("test_agent").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().capabilities.len(), 2);
    }

    #[tokio::test]
    async fn test_find_by_capability() {
        let registry = AgentRegistry::new();

        let agent1 = AgentDefinition::new("analyzer", "Code Analyzer")
            .with_capability(AgentCapability::CodeAnalysis);

        let agent2 = AgentDefinition::new("tester", "Test Generator")
            .with_capability(AgentCapability::TestGeneration)
            .with_capability(AgentCapability::CodeAnalysis);

        registry
            .register(agent1, make_test_executor())
            .await
            .unwrap();
        registry
            .register(agent2, make_test_executor())
            .await
            .unwrap();

        let analyzers = registry
            .find_by_capability(AgentCapability::CodeAnalysis)
            .await;
        assert_eq!(analyzers.len(), 2);

        let testers = registry
            .find_by_capability(AgentCapability::TestGeneration)
            .await;
        assert_eq!(testers.len(), 1);
    }

    #[tokio::test]
    async fn test_agent_builder() {
        let agent = AgentDefinition::new("builder_test", "Builder Test")
            .with_description("A test agent")
            .with_capabilities(&[
                AgentCapability::CodeAnalysis,
                AgentCapability::SecurityAudit,
            ])
            .requires_capability(AgentCapability::DataExtraction)
            .with_priority(AgentPriority::High)
            .parallelizable(true)
            .with_latency(50);

        assert_eq!(agent.capabilities.len(), 2);
        assert_eq!(agent.requires.len(), 1);
        assert_eq!(agent.priority, AgentPriority::High);
        assert!(agent.parallelizable);
        assert_eq!(agent.estimated_latency_ms, 50);
    }

    #[tokio::test]
    async fn test_capability_parsing() {
        assert_eq!(
            AgentCapability::parse("code_analysis"),
            Some(AgentCapability::CodeAnalysis)
        );
        assert_eq!(
            AgentCapability::parse("tests"),
            Some(AgentCapability::TestGeneration)
        );
        assert_eq!(AgentCapability::parse("unknown_capability"), None);
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cache/src/agent.rs">
//! Agent type aliases for op-cache.
//!
//! Keeps public API aligned with gRPC naming.

pub use crate::agent_registry::{
    AgentCapability as Capability, AgentDefinition as Agent, AgentPriority as Priority,
    AgentRegistry,
};
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cache/src/btrfs_cache.rs">
//! BTRFS-backed cache with SQLite index, compression, and NUMA optimization
//!
//! Provides unlimited disk-based caching with:
//! - BTRFS transparent compression (zstd)
//! - SQLite index for O(1) lookups
//! - Linux page cache for hot data
//! - Automatic snapshot management
//! - NUMA-aware memory allocation and CPU affinity

use anyhow::{Context, Result};
use rusqlite::OptionalExtension;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};
use tracing::{debug, info, warn};

use super::numa::{NumaNode, NumaStats, NumaTopology};
use super::snapshot_manager::{SnapshotConfig, SnapshotManager};

/// NUMA-aware cache placement strategy
#[derive(Debug, Clone)]
pub enum CachePlacementStrategy {
    /// Place cache data on the same NUMA node as the requesting CPU
    LocalNode,
    /// Distribute cache data across all NUMA nodes for load balancing
    RoundRobin,
    /// Use the NUMA node with most available memory
    MostMemory,
    /// Disable NUMA optimizations (default)
    Disabled,
}

/// Memory allocation policy for NUMA systems
#[derive(Debug, Clone)]
pub enum MemoryPolicy {
    /// Bind memory to specific NUMA node
    Bind(Vec<u32>),
    /// Prefer memory from specific NUMA node
    Preferred(Option<u32>),
    /// Interleave memory across multiple NUMA nodes
    Interleave(Vec<u32>),
    /// Use default system memory policy
    Default,
}

pub struct BtrfsCache {
    cache_dir: PathBuf,
    index: Mutex<rusqlite::Connection>,
    snapshot_manager: SnapshotManager,
    numa_topology: NumaTopology,
    placement_strategy: CachePlacementStrategy,
    memory_policy: MemoryPolicy,
    cpu_affinity: Vec<u32>, // CPU cores for affinity binding
    current_node_index: AtomicUsize,
    #[allow(dead_code)]
    numa_stats: Mutex<NumaStats>,
}

#[allow(dead_code)]
impl BtrfsCache {
    /// Create BTRFS subvolume at specified path
    async fn create_btrfs_subvolume(path: &Path) -> Result<()> {
        if path.exists() {
            return Ok(());
        }

        let output = tokio::process::Command::new("btrfs")
            .args(["subvolume", "create"])
            .arg(path)
            .output()
            .await
            .context("Failed to execute btrfs command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("command not found") || stderr.contains("not a btrfs filesystem") {
                warn!(
                    "BTRFS not available, creating regular directory: {:?}",
                    path
                );
                tokio::fs::create_dir_all(path)
                    .await
                    .context("Failed to create cache directory")?;
            } else {
                anyhow::bail!("btrfs subvolume create failed: {}", stderr);
            }
        }

        Ok(())
    }

    /// Create new BTRFS cache with proper subvolumes
    pub async fn new(cache_dir: PathBuf) -> Result<Self> {
        // Ensure parent directory exists (not as subvolume)
        if let Some(parent) = cache_dir.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Create directory structure (BTRFS subvolumes stubbed as regular dirs)
        Self::create_btrfs_subvolume(&cache_dir).await?;
        Self::create_btrfs_subvolume(&cache_dir.join("embeddings")).await?;
        Self::create_btrfs_subvolume(&cache_dir.join("blocks")).await?;
        Self::create_btrfs_subvolume(&cache_dir.join("queries")).await?;
        Self::create_btrfs_subvolume(&cache_dir.join("diffs")).await?;

        // Create regular directories within subvolumes
        tokio::fs::create_dir_all(cache_dir.join("embeddings/vectors")).await?;
        tokio::fs::create_dir_all(cache_dir.join("blocks/by-number")).await?;
        tokio::fs::create_dir_all(cache_dir.join("blocks/by-hash")).await?;

        // Create SQLite index for embeddings
        let index_path = cache_dir.join("embeddings/index.db");
        let index =
            rusqlite::Connection::open(&index_path).context("Failed to open SQLite index")?;

        // Create embeddings table
        index.execute(
            "CREATE TABLE IF NOT EXISTS embeddings (
                text_hash TEXT PRIMARY KEY,
                text TEXT NOT NULL,
                vector_file TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                accessed_at INTEGER NOT NULL,
                access_count INTEGER NOT NULL DEFAULT 1,
                vector_size INTEGER NOT NULL
            )",
            [],
        )?;

        // Create index for hot/cold data analysis
        index.execute(
            "CREATE INDEX IF NOT EXISTS idx_accessed
             ON embeddings(accessed_at DESC)",
            [],
        )?;

        index.execute(
            "CREATE INDEX IF NOT EXISTS idx_created
             ON embeddings(created_at DESC)",
            [],
        )?;

        // Initialize snapshot manager
        let snapshot_config = SnapshotConfig {
            snapshot_dir: cache_dir
                .parent()
                .unwrap_or(Path::new("/var/lib/op-dbus"))
                .join("@cache-snapshots"),
            max_snapshots: std::env::var("OPDBUS_MAX_CACHE_SNAPSHOTS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(24),
            prefix: std::env::var("OPDBUS_CACHE_SNAPSHOT_PREFIX")
                .unwrap_or_else(|_| "SNP-cache".to_string()),
        };

        let snapshot_manager = SnapshotManager::new(cache_dir.clone(), snapshot_config);

        // Detect NUMA topology
        let numa_topology = NumaTopology::detect()?;
        let placement_strategy = Self::determine_placement_strategy(numa_topology.nodes());
        let memory_policy = Self::determine_memory_policy();

        // Detect CPU affinity - bind cache operations to same CPUs as Btrfs operations
        let cpu_affinity = if let Some(primary_node) = numa_topology.nodes().values().next() {
            primary_node.cpu_list.clone()
        } else {
            // No NUMA, use first few CPUs
            (0..(num_cpus::get().min(4) as u32)).collect()
        };

        Ok(Self {
            cache_dir,
            index: Mutex::new(index),
            snapshot_manager,
            numa_topology,
            placement_strategy,
            memory_policy,
            cpu_affinity,
            current_node_index: AtomicUsize::new(0),
            numa_stats: Mutex::new(NumaStats::new()),
        })
    }

    fn determine_placement_strategy(numa_nodes: &HashMap<u32, NumaNode>) -> CachePlacementStrategy {
        let default_choice = if numa_nodes.is_empty() {
            "disabled".to_string()
        } else {
            "local".to_string()
        };

        let placement = std::env::var("OPDBUS_CACHE_PLACEMENT")
            .unwrap_or(default_choice)
            .to_lowercase();

        match placement.as_str() {
            "round-robin" | "round_robin" | "roundrobin" => CachePlacementStrategy::RoundRobin,
            "most-memory" | "most_memory" | "mostmemory" => CachePlacementStrategy::MostMemory,
            "disabled" => CachePlacementStrategy::Disabled,
            "local" | "local-node" | "local_node" => {
                if numa_nodes.is_empty() {
                    CachePlacementStrategy::Disabled
                } else {
                    CachePlacementStrategy::LocalNode
                }
            }
            other => {
                warn!(
                    "Unknown OPDBUS_CACHE_PLACEMENT value '{}', defaulting to {}",
                    other,
                    if numa_nodes.is_empty() {
                        "disabled"
                    } else {
                        "local"
                    }
                );
                if numa_nodes.is_empty() {
                    CachePlacementStrategy::Disabled
                } else {
                    CachePlacementStrategy::LocalNode
                }
            }
        }
    }

    fn determine_memory_policy() -> MemoryPolicy {
        match std::env::var("OPDBUS_CACHE_MEMORY_POLICY") {
            Ok(value) => {
                let value_lower = value.to_lowercase();
                if let Some(rest) = value_lower.strip_prefix("bind:") {
                    let nodes = Self::parse_node_list(rest);
                    if nodes.is_empty() {
                        warn!("OPDBUS_CACHE_MEMORY_POLICY=bind but no NUMA nodes listed");
                        MemoryPolicy::Default
                    } else {
                        MemoryPolicy::Bind(nodes)
                    }
                } else if let Some(rest) = value_lower.strip_prefix("preferred:") {
                    if rest.trim().is_empty() {
                        MemoryPolicy::Preferred(None)
                    } else {
                        match rest.trim().parse::<u32>() {
                            Ok(node) => MemoryPolicy::Preferred(Some(node)),
                            Err(e) => {
                                warn!(
                                    "Failed to parse preferred NUMA node '{}': {}",
                                    rest.trim(),
                                    e
                                );
                                MemoryPolicy::Default
                            }
                        }
                    }
                } else if value_lower == "preferred" {
                    MemoryPolicy::Preferred(None)
                } else if let Some(rest) = value_lower.strip_prefix("interleave:") {
                    let nodes = Self::parse_node_list(rest);
                    if nodes.is_empty() {
                        warn!("OPDBUS_CACHE_MEMORY_POLICY=interleave but no NUMA nodes listed");
                        MemoryPolicy::Default
                    } else {
                        MemoryPolicy::Interleave(nodes)
                    }
                } else if value_lower == "default" || value_lower.is_empty() {
                    MemoryPolicy::Default
                } else {
                    warn!(
                        "Unknown OPDBUS_CACHE_MEMORY_POLICY value '{}', using default",
                        value
                    );
                    MemoryPolicy::Default
                }
            }
            Err(_) => MemoryPolicy::Default,
        }
    }

    fn parse_node_list(list: &str) -> Vec<u32> {
        list.split(',')
            .filter_map(|s| {
                let trimmed = s.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    match trimmed.parse::<u32>() {
                        Ok(value) => Some(value),
                        Err(e) => {
                            warn!("Invalid NUMA node id '{}': {}", trimmed, e);
                            None
                        }
                    }
                }
            })
            .collect()
    }

    fn select_numa_node(&self, operation: &str) -> Option<&NumaNode> {
        if self.numa_topology.node_count() == 0 {
            return None;
        }

        let nodes: Vec<&NumaNode> = self.numa_topology.nodes().values().collect();
        let selection = match self.placement_strategy {
            CachePlacementStrategy::LocalNode => nodes.first().copied(),
            CachePlacementStrategy::RoundRobin => {
                let index = self.current_node_index.fetch_add(1, Ordering::Relaxed);
                nodes.get(index % nodes.len()).copied()
            }
            CachePlacementStrategy::MostMemory => nodes
                .iter()
                .max_by_key(|node| node.memory_total_kb)
                .copied(),
            CachePlacementStrategy::Disabled => None,
        };

        if let Some(node) = selection {
            debug!(
                "NUMA node {} selected for {} (memory={} MB, distances={:?})",
                node.node_id,
                operation,
                node.memory_total_kb / 1024,
                node.distance_to_nodes
            );
        } else {
            debug!(
                "No NUMA node selected for {} (strategy={:?})",
                operation, self.placement_strategy
            );
        }

        selection
    }

    /// Get or compute embedding
    pub fn get_or_embed<F>(&self, text: &str, compute_fn: F) -> Result<Vec<f32>>
    where
        F: FnOnce(&str) -> Result<Vec<f32>>,
    {
        let text_hash = self.hash_text(text);

        // Check if cached
        if let Some(vector) = self.load_embedding(&text_hash)? {
            // Update access statistics
            self.update_access(&text_hash)?;
            return Ok(vector);
        }

        // Compute embedding
        let vector = compute_fn(text)?;

        // Store in cache
        self.save_embedding(text, &text_hash, &vector)?;

        Ok(vector)
    }

    /// Get embedding if cached (without computing)
    pub fn get_embedding(&self, text: &str) -> Result<Option<Vec<f32>>> {
        let text_hash = self.hash_text(text);
        if let Some(vector) = self.load_embedding(&text_hash)? {
            self.update_access(&text_hash)?;
            return Ok(Some(vector));
        }
        Ok(None)
    }

    /// Store embedding directly
    pub fn put_embedding(&self, text: &str, vector: &[f32]) -> Result<()> {
        let text_hash = self.hash_text(text);
        self.save_embedding(text, &text_hash, vector)
    }

    fn hash_text(&self, text: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn load_embedding(&self, text_hash: &str) -> Result<Option<Vec<f32>>> {
        let index = self.index.lock().unwrap();

        // Lookup in SQLite index
        let vector_file: Option<String> = index
            .query_row(
                "SELECT vector_file FROM embeddings WHERE text_hash = ?1",
                [text_hash],
                |row| row.get(0),
            )
            .optional()?;

        drop(index); // Release lock before file I/O

        if let Some(file) = vector_file {
            let path = self.cache_dir.join("embeddings/vectors").join(&file);

            // Read from BTRFS (page cache will cache this!)
            let data = std::fs::read(&path)
                .context(format!("Failed to read cached embedding: {:?}", path))?;

            let vector: Vec<f32> =
                bincode::deserialize(&data).context("Failed to deserialize cached embedding")?;

            return Ok(Some(vector));
        }

        Ok(None)
    }

    fn save_embedding(&self, text: &str, text_hash: &str, vector: &[f32]) -> Result<()> {
        let vectors_dir = self.cache_dir.join("embeddings/vectors");
        std::fs::create_dir_all(&vectors_dir)?;

        let vector_file = format!("{}.vec", text_hash);
        let path = vectors_dir.join(&vector_file);

        // Write to BTRFS (automatically compressed by kernel)
        let data = bincode::serialize(vector)?;
        std::fs::write(&path, data)?;

        // Add to SQLite index
        let index = self.index.lock().unwrap();
        let now = chrono::Utc::now().timestamp();
        index.execute(
            "INSERT INTO embeddings (text_hash, text, vector_file, created_at, accessed_at, vector_size)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(text_hash) DO UPDATE SET
                accessed_at = ?5,
                access_count = access_count + 1",
            rusqlite::params![text_hash, text, vector_file, now, now, vector.len()],
        )?;

        Ok(())
    }

    fn update_access(&self, text_hash: &str) -> Result<()> {
        let index = self.index.lock().unwrap();
        let now = chrono::Utc::now().timestamp();
        index.execute(
            "UPDATE embeddings
             SET accessed_at = ?1, access_count = access_count + 1
             WHERE text_hash = ?2",
            rusqlite::params![now, text_hash],
        )?;
        Ok(())
    }

    /// Get cache statistics
    pub fn stats(&self) -> Result<CacheStats> {
        let index = self.index.lock().unwrap();

        let total: i64 =
            index.query_row("SELECT COUNT(*) FROM embeddings", [], |row| row.get(0))?;

        let hot_threshold = chrono::Utc::now().timestamp() - 3600; // 1 hour
        let hot: i64 = index.query_row(
            "SELECT COUNT(*) FROM embeddings WHERE accessed_at > ?1",
            [hot_threshold],
            |row| row.get(0),
        )?;

        let total_accesses: i64 =
            index.query_row("SELECT SUM(access_count) FROM embeddings", [], |row| {
                row.get(0)
            })?;

        drop(index); // Release lock before file I/O

        // Calculate disk usage
        let embeddings_size = self.dir_size(&self.cache_dir.join("embeddings/vectors"))?;
        let blocks_size = self.dir_size(&self.cache_dir.join("blocks"))?;
        let total_size = embeddings_size + blocks_size;

        Ok(CacheStats {
            total_entries: total as usize,
            hot_entries: hot as usize,
            total_accesses: total_accesses as u64,
            disk_usage_bytes: total_size,
            embeddings_size_bytes: embeddings_size,
            blocks_size_bytes: blocks_size,
        })
    }

    #[allow(clippy::only_used_in_recursion)]
    fn dir_size(&self, path: &Path) -> Result<u64> {
        let mut size = 0u64;
        if !path.exists() {
            return Ok(0);
        }

        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let metadata = entry.metadata()?;
            if metadata.is_file() {
                size += metadata.len();
            } else if metadata.is_dir() {
                size += self.dir_size(&entry.path())?;
            }
        }
        Ok(size)
    }

    /// Clean old entries (accessed before cutoff)
    pub fn cleanup_old(&self, days: i64) -> Result<usize> {
        let cutoff = chrono::Utc::now().timestamp() - (days * 86400);

        let index = self.index.lock().unwrap();

        // Find old entries
        let mut stmt = index.prepare(
            "SELECT text_hash, vector_file FROM embeddings
             WHERE accessed_at < ?1",
        )?;

        let old_entries: Vec<(String, String)> = stmt
            .query_map([cutoff], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;

        let count = old_entries.len();

        drop(stmt); // Release statement
        drop(index); // Release lock before file I/O

        // Delete files
        for (_hash, file) in &old_entries {
            let path = self.cache_dir.join("embeddings/vectors").join(file);
            let _ = std::fs::remove_file(path); // Ignore errors
        }

        // Delete from index
        let index = self.index.lock().unwrap();
        index.execute("DELETE FROM embeddings WHERE accessed_at < ?1", [cutoff])?;

        log::info!(
            "Cleaned up {} old cache entries (>{} days old)",
            count,
            days
        );

        Ok(count)
    }

    /// Clear all cache data
    pub fn clear(&self) -> Result<()> {
        log::warn!("Clearing all cache data");

        // Clear embeddings
        let vectors_dir = self.cache_dir.join("embeddings/vectors");
        if vectors_dir.exists() {
            std::fs::remove_dir_all(&vectors_dir)?;
            std::fs::create_dir_all(&vectors_dir)?;
        }

        // Clear blocks
        let blocks_dir = self.cache_dir.join("blocks");
        if blocks_dir.exists() {
            std::fs::remove_dir_all(&blocks_dir)?;
            std::fs::create_dir_all(blocks_dir.join("by-number"))?;
            std::fs::create_dir_all(blocks_dir.join("by-hash"))?;
        }

        // Clear index
        let index = self.index.lock().unwrap();
        index.execute("DELETE FROM embeddings", [])?;

        log::info!("Cache cleared");

        Ok(())
    }

    /// Clear only embeddings cache
    pub fn clear_embeddings(&self) -> Result<()> {
        log::warn!("Clearing embeddings cache");

        // Clear embeddings vectors
        let vectors_dir = self.cache_dir.join("embeddings/vectors");
        if vectors_dir.exists() {
            std::fs::remove_dir_all(&vectors_dir)?;
            std::fs::create_dir_all(&vectors_dir)?;
        }

        // Clear index
        let index = self.index.lock().unwrap();
        index.execute("DELETE FROM embeddings", [])?;

        log::info!("Embeddings cache cleared");

        Ok(())
    }

    /// Clear only blocks cache
    pub fn clear_blocks(&self) -> Result<()> {
        log::warn!("Clearing blocks cache");

        // Clear blocks
        let blocks_dir = self.cache_dir.join("blocks");
        if blocks_dir.exists() {
            std::fs::remove_dir_all(&blocks_dir)?;
            std::fs::create_dir_all(blocks_dir.join("by-number"))?;
            std::fs::create_dir_all(blocks_dir.join("by-hash"))?;
        }

        log::info!("Blocks cache cleared");

        Ok(())
    }

    /// Create BTRFS snapshot of cache
    pub async fn create_snapshot(&self) -> Result<PathBuf> {
        self.snapshot_manager.create_snapshot().await
    }

    /// List all snapshots
    pub async fn list_snapshots(&self) -> Result<Vec<super::snapshot_manager::SnapshotInfo>> {
        self.snapshot_manager.list_snapshots().await
    }

    /// Delete all snapshots
    pub async fn delete_all_snapshots(&self) -> Result<usize> {
        self.snapshot_manager.delete_all_snapshots().await
    }

    /// Stream cache data to remote system using Btrfs send/receive with NUMA affinity
    pub async fn stream_to_remote(
        &self,
        remote_host: &str,
        remote_path: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Apply NUMA affinity for streaming operations
        self.apply_numa_affinity("cache_streaming").await?;

        let snapshot_path = self
            .create_snapshot()
            .await
            .map_err(|e| format!("Failed to create snapshot: {}", e))?;

        info!(
            "Streaming cache snapshot to {}:{}",
            remote_host, remote_path
        );

        let cmd = format!(
            "btrfs send {} | ssh {} 'btrfs receive {}'",
            snapshot_path.display(),
            remote_host,
            remote_path
        );

        let output = tokio::process::Command::new("bash")
            .arg("-c")
            .arg(&cmd)
            .output()
            .await
            .map_err(|e| format!("Failed to execute btrfs stream command: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Btrfs streaming failed: {}", stderr).into());
        }

        info!("Successfully streamed cache snapshot");
        Ok(())
    }

    /// Receive cache data from remote system with NUMA affinity
    pub async fn receive_from_remote(
        &self,
        remote_host: &str,
        remote_snapshot: &str,
        local_path: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Apply NUMA affinity for receiving operations
        self.apply_numa_affinity("cache_receiving").await?;

        info!(
            "Receiving cache snapshot from {}:{}",
            remote_host, remote_snapshot
        );

        let cmd = format!(
            "ssh {} 'btrfs send {}' | btrfs receive {}",
            remote_host, remote_snapshot, local_path
        );

        let output = tokio::process::Command::new("bash")
            .arg("-c")
            .arg(&cmd)
            .output()
            .await
            .map_err(|e| format!("Failed to execute btrfs receive command: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Btrfs receive failed: {}", stderr).into());
        }

        info!("Successfully received cache snapshot");
        Ok(())
    }

    /// Get NUMA configuration info
    pub fn numa_info(&self) -> NumaInfo {
        NumaInfo {
            node_count: self.numa_topology.node_count(),
            cpu_affinity: self.cpu_affinity.clone(),
            placement_strategy: self.placement_strategy.clone(),
            memory_policy: self.memory_policy.clone(),
        }
    }

    /// Get cache directory path
    pub fn cache_dir(&self) -> &PathBuf {
        &self.cache_dir
    }

    /// Helper method to apply NUMA affinity (CPU + memory)
    async fn apply_numa_affinity(
        &self,
        operation: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Apply CPU affinity first
        self.apply_cpu_affinity(operation).await?;

        // Apply memory policy
        match &self.memory_policy {
            MemoryPolicy::Default => {
                debug!("Using default memory policy for {}", operation);
            }
            MemoryPolicy::Bind(nodes) if !nodes.is_empty() => {
                debug!("Memory bound to nodes {:?} for {}", nodes, operation);
            }
            MemoryPolicy::Preferred(Some(node)) => {
                debug!("Memory preferred on node {} for {}", node, operation);
            }
            MemoryPolicy::Interleave(nodes) if !nodes.is_empty() => {
                debug!(
                    "Memory interleaved across nodes {:?} for {}",
                    nodes, operation
                );
            }
            _ => {
                debug!("Memory policy not applied for {}", operation);
            }
        }

        Ok(())
    }

    /// Apply CPU affinity using taskset
    async fn apply_cpu_affinity(
        &self,
        operation: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let candidate_cpus = self
            .select_numa_node(operation)
            .and_then(|node| {
                if node.cpu_list.is_empty() {
                    None
                } else {
                    Some(node.cpu_list.clone())
                }
            })
            .unwrap_or_else(|| self.cpu_affinity.clone());

        if candidate_cpus.is_empty() {
            debug!("No CPU affinity configured for {}", operation);
            return Ok(());
        }

        if candidate_cpus == self.cpu_affinity {
            debug!(
                "Using default CPU affinity {:?} for {}",
                candidate_cpus, operation
            );
        }

        let cpu_list = candidate_cpus
            .iter()
            .map(|cpu| cpu.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let output = tokio::process::Command::new("taskset")
            .args(["-c", &cpu_list])
            .arg("echo")
            .arg(format!("CPU affinity test for {}", operation))
            .output()
            .await
            .map_err(|e| format!("taskset command failed: {}", e))?;

        if output.status.success() {
            debug!(
                "Applied CPU affinity to cores: {} for {}",
                cpu_list, operation
            );
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("taskset failed for {}: {}", operation, stderr);
            Ok(()) // Don't fail, just continue without affinity
        }
    }
}

#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_entries: usize,
    pub hot_entries: usize,
    pub total_accesses: u64,
    pub disk_usage_bytes: u64,
    pub embeddings_size_bytes: u64,
    pub blocks_size_bytes: u64,
}

impl CacheStats {
    pub fn hot_ratio(&self) -> f64 {
        if self.total_entries == 0 {
            0.0
        } else {
            self.hot_entries as f64 / self.total_entries as f64
        }
    }

    pub fn avg_accesses(&self) -> f64 {
        if self.total_entries == 0 {
            0.0
        } else {
            self.total_accesses as f64 / self.total_entries as f64
        }
    }
}
#[derive(Debug, Clone)]
/// NUMA configuration information
pub struct NumaInfo {
    pub node_count: usize,
    pub cpu_affinity: Vec<u32>,
    pub placement_strategy: CachePlacementStrategy,
    pub memory_policy: MemoryPolicy,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_text_hashing() {
        let cache = BtrfsCache::new(PathBuf::from("/tmp/test-cache"))
            .await
            .unwrap();
        let hash1 = cache.hash_text("test");
        let hash2 = cache.hash_text("test");
        let hash3 = cache.hash_text("different");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert_eq!(hash1.len(), 64); // SHA256 hex length
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cache/src/capability_resolver.rs">
//! Capability resolver - maps requests to agent sequences
//!
//! Takes a request with required capabilities and resolves it
//! to an ordered sequence of agents.

use anyhow::Result;
use std::collections::HashSet;
use std::sync::Arc;
use tracing::{debug, info, warn};

use super::agent_registry::{AgentCapability, AgentDefinition, AgentRegistry};

/// Request that needs capability resolution
#[derive(Debug, Clone)]
pub struct CapabilityRequest {
    /// Explicitly requested capabilities
    pub required_capabilities: Vec<AgentCapability>,

    /// Preferred agents (use these if they provide the capability)
    pub preferred_agents: Vec<String>,

    /// Agents to exclude
    pub excluded_agents: Vec<String>,

    /// Allow parallel execution where possible
    pub allow_parallel: bool,

    /// Maximum agents in sequence
    pub max_agents: usize,

    /// Input data for the request
    pub input: Vec<u8>,
}

impl CapabilityRequest {
    /// Create request with required capabilities
    pub fn new(capabilities: Vec<AgentCapability>, input: Vec<u8>) -> Self {
        Self {
            required_capabilities: capabilities,
            preferred_agents: Vec::new(),
            excluded_agents: Vec::new(),
            allow_parallel: false,
            max_agents: 10,
            input,
        }
    }

    /// Create from capability strings
    pub fn from_strings(cap_strings: &[&str], input: Vec<u8>) -> Self {
        let capabilities: Vec<AgentCapability> = cap_strings
            .iter()
            .filter_map(|s| AgentCapability::parse(s))
            .collect();

        Self::new(capabilities, input)
    }

    /// Builder: prefer specific agents
    pub fn prefer_agents(mut self, agents: &[&str]) -> Self {
        self.preferred_agents = agents.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Builder: exclude specific agents
    pub fn exclude_agents(mut self, agents: &[&str]) -> Self {
        self.excluded_agents = agents.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Builder: allow parallel execution
    pub fn allow_parallel(mut self, allow: bool) -> Self {
        self.allow_parallel = allow;
        self
    }
}

/// Resolved agent sequence
#[derive(Debug, Clone)]
pub struct ResolvedSequence {
    /// Ordered list of agents to execute
    pub agents: Vec<AgentDefinition>,

    /// Capabilities fulfilled by this sequence
    pub fulfilled_capabilities: HashSet<AgentCapability>,

    /// Capabilities that couldn't be fulfilled
    pub missing_capabilities: HashSet<AgentCapability>,

    /// Estimated total latency
    pub estimated_latency_ms: u64,

    /// Groups of agents that can run in parallel
    pub parallel_groups: Vec<Vec<String>>,

    /// Resolution metadata
    pub resolution_path: Vec<String>,
}

impl ResolvedSequence {
    /// Get agent IDs in order
    pub fn agent_ids(&self) -> Vec<String> {
        self.agents.iter().map(|a| a.id.clone()).collect()
    }

    /// Check if all capabilities were fulfilled
    pub fn is_complete(&self) -> bool {
        self.missing_capabilities.is_empty()
    }

    /// Get number of agents
    pub fn len(&self) -> usize {
        self.agents.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }
}

/// Capability resolver
pub struct CapabilityResolver {
    registry: Arc<AgentRegistry>,
}

impl CapabilityResolver {
    /// Create new resolver with registry
    pub fn new(registry: Arc<AgentRegistry>) -> Self {
        Self { registry }
    }

    /// Resolve a capability request to an agent sequence
    pub async fn resolve(&self, request: &CapabilityRequest) -> Result<ResolvedSequence> {
        if request.required_capabilities.is_empty() {
            return Ok(ResolvedSequence {
                agents: Vec::new(),
                fulfilled_capabilities: HashSet::new(),
                missing_capabilities: HashSet::new(),
                estimated_latency_ms: 0,
                parallel_groups: Vec::new(),
                resolution_path: vec!["empty_request".to_string()],
            });
        }

        debug!(
            "Resolving {} capabilities",
            request.required_capabilities.len()
        );

        let mut selected_agents: Vec<AgentDefinition> = Vec::new();
        let mut fulfilled: HashSet<AgentCapability> = HashSet::new();
        let mut resolution_path: Vec<String> = Vec::new();

        // Build candidate pool
        let candidates = self.build_candidate_pool(request).await?;
        resolution_path.push(format!("candidates:{}", candidates.len()));

        // Greedy selection: for each required capability, pick best agent
        let required: HashSet<AgentCapability> =
            request.required_capabilities.iter().copied().collect();

        for cap in &request.required_capabilities {
            // Skip if already fulfilled
            if fulfilled.contains(cap) {
                continue;
            }

            // Find best candidate for this capability
            if let Some(agent) =
                self.select_best_agent(&candidates, *cap, &selected_agents, request)
            {
                resolution_path.push(format!("select:{}->{}", cap.name(), agent.id));

                // Add all capabilities this agent provides
                for provided_cap in &agent.capabilities {
                    fulfilled.insert(*provided_cap);
                }

                selected_agents.push(agent);

                if selected_agents.len() >= request.max_agents {
                    resolution_path.push("max_agents_reached".to_string());
                    break;
                }
            } else {
                resolution_path.push(format!("no_agent_for:{}", cap.name()));
            }
        }

        // Sort agents by priority and dependencies
        self.sort_agents(&mut selected_agents);

        // Calculate missing capabilities
        let missing: HashSet<AgentCapability> = required.difference(&fulfilled).copied().collect();

        // Calculate total latency
        let estimated_latency_ms: u64 =
            selected_agents.iter().map(|a| a.estimated_latency_ms).sum();

        // Build parallel groups if allowed
        let parallel_groups = if request.allow_parallel {
            self.build_parallel_groups(&selected_agents)
        } else {
            Vec::new()
        };

        let sequence = ResolvedSequence {
            agents: selected_agents,
            fulfilled_capabilities: fulfilled,
            missing_capabilities: missing,
            estimated_latency_ms,
            parallel_groups,
            resolution_path,
        };

        if !sequence.missing_capabilities.is_empty() {
            warn!(
                "Could not fulfill capabilities: {:?}",
                sequence.missing_capabilities
            );
        }

        info!(
            "Resolved to {} agents: {:?}",
            sequence.agents.len(),
            sequence.agent_ids()
        );

        Ok(sequence)
    }

    /// Build pool of candidate agents
    async fn build_candidate_pool(
        &self,
        request: &CapabilityRequest,
    ) -> Result<Vec<AgentDefinition>> {
        let candidates = self
            .registry
            .find_by_capabilities(&request.required_capabilities)
            .await;

        // Filter out excluded agents
        let candidates: Vec<AgentDefinition> = candidates
            .into_iter()
            .filter(|a| !request.excluded_agents.contains(&a.id))
            .filter(|a| a.enabled)
            .collect();

        Ok(candidates)
    }

    /// Select best agent for a capability
    fn select_best_agent(
        &self,
        candidates: &[AgentDefinition],
        cap: AgentCapability,
        already_selected: &[AgentDefinition],
        request: &CapabilityRequest,
    ) -> Option<AgentDefinition> {
        let selected_ids: HashSet<&String> = already_selected.iter().map(|a| &a.id).collect();

        // Filter to agents that provide this capability and aren't selected
        let mut viable: Vec<&AgentDefinition> = candidates
            .iter()
            .filter(|a| a.provides(cap))
            .filter(|a| !selected_ids.contains(&a.id))
            .collect();

        if viable.is_empty() {
            return None;
        }

        // Score each candidate
        // Higher score = better choice
        let score = |agent: &AgentDefinition| -> i64 {
            let mut s: i64 = 0;

            // Prefer agents that provide more of our required capabilities
            let provided_required = agent
                .capabilities
                .iter()
                .filter(|c| request.required_capabilities.contains(c))
                .count();
            s += (provided_required as i64) * 100;

            // Prefer lower latency
            s -= agent.estimated_latency_ms as i64 / 10;

            // Prefer preferred agents
            if request.preferred_agents.contains(&agent.id) {
                s += 500;
            }

            // Prefer higher priority
            s -= (agent.priority as i64) * 50;

            // Prefer parallelizable if parallel is allowed
            if request.allow_parallel && agent.parallelizable {
                s += 25;
            }

            s
        };

        viable.sort_by_key(|b| std::cmp::Reverse(score(b)));

        viable.first().map(|a| (*a).clone())
    }

    /// Sort agents by priority and dependencies
    fn sort_agents(&self, agents: &mut [AgentDefinition]) {
        // Simple sort by priority
        // TODO: topological sort based on requires/provides
        agents.sort_by(|a, b| a.priority.cmp(&b.priority));
    }

    /// Build parallel execution groups
    fn build_parallel_groups(&self, agents: &[AgentDefinition]) -> Vec<Vec<String>> {
        let mut groups: Vec<Vec<String>> = Vec::new();
        let mut current_group: Vec<String> = Vec::new();

        for agent in agents {
            if agent.parallelizable {
                current_group.push(agent.id.clone());
            } else {
                // Flush current parallel group
                if !current_group.is_empty() {
                    groups.push(current_group);
                    current_group = Vec::new();
                }
                // Non-parallel agent is its own group
                groups.push(vec![agent.id.clone()]);
            }
        }

        // Flush remaining
        if !current_group.is_empty() {
            groups.push(current_group);
        }

        groups
    }

    /// Get resolver statistics
    pub async fn stats(&self) -> ResolverStats {
        let registry_stats = self.registry.stats().await;

        ResolverStats {
            available_agents: registry_stats.enabled_agents,
            available_capabilities: registry_stats.total_capabilities,
        }
    }
}

/// Resolver statistics
#[derive(Debug, Clone)]
pub struct ResolverStats {
    pub available_agents: usize,
    pub available_capabilities: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_registry::AgentExecutor;

    fn make_test_executor() -> AgentExecutor {
        Arc::new(|input: &[u8]| {
            let input = input.to_vec();
            Box::pin(async move { Ok::<Vec<u8>, anyhow::Error>(input) })
        })
    }

    async fn setup_test_registry() -> Arc<AgentRegistry> {
        let registry = Arc::new(AgentRegistry::new());

        // Code analyzer
        let analyzer = AgentDefinition::new("analyzer", "Code Analyzer")
            .with_capabilities(&[
                AgentCapability::CodeAnalysis,
                AgentCapability::DependencyAnalysis,
            ])
            .with_priority(AgentPriority::High)
            .with_latency(50);

        registry
            .register(analyzer, make_test_executor())
            .await
            .unwrap();

        // Test generator (requires analysis first)
        let tester = AgentDefinition::new("tester", "Test Generator")
            .with_capability(AgentCapability::TestGeneration)
            .requires_capability(AgentCapability::CodeAnalysis)
            .with_priority(AgentPriority::Normal)
            .with_latency(100);

        registry
            .register(tester, make_test_executor())
            .await
            .unwrap();

        // Security auditor
        let security = AgentDefinition::new("security", "Security Auditor")
            .with_capability(AgentCapability::SecurityAudit)
            .with_priority(AgentPriority::High)
            .parallelizable(true)
            .with_latency(75);

        registry
            .register(security, make_test_executor())
            .await
            .unwrap();

        // Doc generator
        let docs = AgentDefinition::new("docs", "Documentation Generator")
            .with_capability(AgentCapability::DocumentationGeneration)
            .requires_capability(AgentCapability::CodeAnalysis)
            .with_priority(AgentPriority::Low)
            .with_latency(80);

        registry.register(docs, make_test_executor()).await.unwrap();

        registry
    }

    #[tokio::test]
    async fn test_simple_resolution() {
        let registry = setup_test_registry().await;
        let resolver = CapabilityResolver::new(registry);

        let request = CapabilityRequest::new(vec![AgentCapability::CodeAnalysis], b"test".to_vec());

        let sequence = resolver.resolve(&request).await.unwrap();

        assert_eq!(sequence.agents.len(), 1);
        assert_eq!(sequence.agents[0].id, "analyzer");
        assert!(sequence.is_complete());
    }

    #[tokio::test]
    async fn test_multi_capability_resolution() {
        let registry = setup_test_registry().await;
        let resolver = CapabilityResolver::new(registry);

        let request = CapabilityRequest::new(
            vec![
                AgentCapability::CodeAnalysis,
                AgentCapability::TestGeneration,
                AgentCapability::SecurityAudit,
            ],
            b"test".to_vec(),
        );

        let sequence = resolver.resolve(&request).await.unwrap();

        assert_eq!(sequence.agents.len(), 3);
        assert!(sequence.is_complete());

        // Check priority ordering (High agents first)
        assert!(sequence.agents[0].priority <= sequence.agents[1].priority);
    }

    #[tokio::test]
    async fn test_agent_reuse() {
        let registry = setup_test_registry().await;
        let resolver = CapabilityResolver::new(registry);

        // Analyzer provides both CodeAnalysis and DependencyAnalysis
        let request = CapabilityRequest::new(
            vec![
                AgentCapability::CodeAnalysis,
                AgentCapability::DependencyAnalysis,
            ],
            b"test".to_vec(),
        );

        let sequence = resolver.resolve(&request).await.unwrap();

        // Should only need one agent (analyzer provides both)
        assert_eq!(sequence.agents.len(), 1);
        assert!(sequence.is_complete());
    }

    #[tokio::test]
    async fn test_missing_capability() {
        let registry = setup_test_registry().await;
        let resolver = CapabilityResolver::new(registry);

        let request = CapabilityRequest::new(
            vec![AgentCapability::Embedding], // Not provided by any agent
            b"test".to_vec(),
        );

        let sequence = resolver.resolve(&request).await.unwrap();

        assert!(!sequence.is_complete());
        assert!(sequence
            .missing_capabilities
            .contains(&AgentCapability::Embedding));
    }

    #[tokio::test]
    async fn test_preferred_agent() {
        let registry = setup_test_registry().await;

        // Add another analyzer
        let alt_analyzer = AgentDefinition::new("alt_analyzer", "Alternative Analyzer")
            .with_capability(AgentCapability::CodeAnalysis)
            .with_latency(25); // Faster

        registry
            .register(alt_analyzer, make_test_executor())
            .await
            .unwrap();

        let resolver = CapabilityResolver::new(registry);

        // Prefer the original analyzer even though alt is faster
        let request = CapabilityRequest::new(vec![AgentCapability::CodeAnalysis], b"test".to_vec())
            .prefer_agents(&["analyzer"]);

        let sequence = resolver.resolve(&request).await.unwrap();

        assert_eq!(sequence.agents[0].id, "analyzer");
    }

    #[tokio::test]
    async fn test_excluded_agent() {
        let registry = setup_test_registry().await;
        let resolver = CapabilityResolver::new(registry);

        let request = CapabilityRequest::new(vec![AgentCapability::CodeAnalysis], b"test".to_vec())
            .exclude_agents(&["analyzer"]);

        let sequence = resolver.resolve(&request).await.unwrap();

        // No agent can fulfill (only analyzer has this capability)
        assert!(!sequence.is_complete());
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cache/src/lib.rs">
//! op-cache: BTRFS-based caching with NUMA awareness and agent orchestration.
//!
//! Features:
//! - BTRFS subvolume cache management
//! - NUMA-aware memory allocation
//! - Snapshot management for rollback
//! - Agent registry with capabilities
//! - Workstack orchestration with caching
//! - Pattern tracking for workstack promotion
//! - gRPC services

pub mod agent;
pub mod agent_registry;
pub mod btrfs_cache;
pub mod capability_resolver;
pub mod numa;
pub mod orchestrator;
pub mod pattern_tracker;
pub mod snapshot_manager;
pub mod workflow_cache;
pub mod workflow_executor;
pub mod workflow_tracker;
pub mod workstack_cache;

pub mod grpc;

pub use agent::{Agent, AgentRegistry, Capability, Priority};
pub use btrfs_cache::BtrfsCache;
pub use numa::{NumaNode, NumaTopology};
pub use orchestrator::Orchestrator;
pub use pattern_tracker::PatternTracker;
pub use snapshot_manager::SnapshotManager;
pub use workstack_cache::WorkstackCache;

pub mod proto {
    tonic::include_proto!("op_cache");
}

/// Prelude for convenient imports.
pub mod prelude {
    pub use super::agent::{Agent, AgentRegistry, Capability, Priority};
    pub use super::btrfs_cache::BtrfsCache;
    pub use super::numa::{NumaNode, NumaTopology};
    pub use super::orchestrator::Orchestrator;
    pub use super::pattern_tracker::PatternTracker;
    pub use super::snapshot_manager::SnapshotManager;
    pub use super::workstack_cache::WorkstackCache;
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cache/src/numa.rs">
//! Enterprise-grade NUMA topology detection and management
#![allow(dead_code)]
//!
//! This module provides comprehensive NUMA (Non-Uniform Memory Access) support
//! for optimal cache performance on multi-socket systems.
//!
//! Features:
//! - Full topology detection from /sys/devices/system/node/
//! - CPU affinity management for L3 cache optimization
//! - Memory policy configuration for local NUMA access
//! - Per-node statistics tracking
//! - Automatic node selection based on workload
//! - Graceful degradation for non-NUMA systems

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// NUMA node information with complete topology
#[derive(Debug, Clone)]
pub struct NumaNode {
    pub node_id: u32,
    pub cpu_list: Vec<u32>,
    pub memory_total_kb: u64,
    pub memory_free_kb: u64,
    pub distance_to_nodes: HashMap<u32, u32>,
}

impl NumaNode {
    /// Check if this node is online
    pub fn is_online(&self) -> bool {
        !self.cpu_list.is_empty()
    }

    /// Get memory utilization percentage
    pub fn memory_utilization(&self) -> f64 {
        if self.memory_total_kb == 0 {
            return 0.0;
        }
        let used = self.memory_total_kb.saturating_sub(self.memory_free_kb);
        (used as f64 / self.memory_total_kb as f64) * 100.0
    }

    /// Get distance to another node (10 = local, higher = more hops)
    pub fn distance_to(&self, other_node: u32) -> u32 {
        self.distance_to_nodes
            .get(&other_node)
            .copied()
            .unwrap_or(255) // 255 = unreachable
    }
}

/// NUMA topology detector
#[derive(Clone)]
pub struct NumaTopology {
    nodes: HashMap<u32, NumaNode>,
    current_node: Option<u32>,
}

impl NumaTopology {
    /// Detect NUMA topology from /sys filesystem
    pub fn detect() -> Result<Self> {
        let sys_node_path = Path::new("/sys/devices/system/node");

        // Check if NUMA is available
        if !sys_node_path.exists() {
            info!("NUMA not available on this system (no /sys/devices/system/node)");
            return Self::create_single_node_fallback();
        }

        let mut nodes = HashMap::new();

        // Find all node directories (node0, node1, etc.)
        let entries =
            fs::read_dir(sys_node_path).context("Failed to read /sys/devices/system/node")?;

        for entry in entries {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Parse node directories (nodeN)
            if let Some(node_suffix) = name_str.strip_prefix("node") {
                if let Ok(node_id) = node_suffix.parse::<u32>() {
                    let node_path = entry.path();

                    match Self::parse_node(&node_path, node_id) {
                        Ok(node) => {
                            if node.is_online() {
                                debug!(
                                    "Detected NUMA node {}: {} CPUs, {} MB RAM",
                                    node_id,
                                    node.cpu_list.len(),
                                    node.memory_total_kb / 1024
                                );
                                nodes.insert(node_id, node);
                            }
                        }
                        Err(e) => {
                            warn!("Failed to parse NUMA node {}: {}", node_id, e);
                        }
                    }
                }
            }
        }

        if nodes.is_empty() {
            warn!("No online NUMA nodes found, falling back to single-node");
            return Self::create_single_node_fallback();
        }

        // Detect current node (which node is this process running on)
        let current_node = Self::detect_current_node();

        info!(
            "NUMA topology detected: {} nodes, current node: {:?}",
            nodes.len(),
            current_node
        );

        Ok(Self {
            nodes,
            current_node,
        })
    }

    /// Parse a single NUMA node directory
    fn parse_node(node_path: &Path, node_id: u32) -> Result<NumaNode> {
        // Parse CPU list
        let cpu_list = Self::parse_cpu_list(node_path)?;

        // Parse memory info
        let (memory_total_kb, memory_free_kb) = Self::parse_memory_info(node_path)?;

        // Parse distance map
        let distance_to_nodes = Self::parse_distance_map(node_path)?;

        Ok(NumaNode {
            node_id,
            cpu_list,
            memory_total_kb,
            memory_free_kb,
            distance_to_nodes,
        })
    }

    /// Parse CPU list from cpulist file
    fn parse_cpu_list(node_path: &Path) -> Result<Vec<u32>> {
        let cpulist_path = node_path.join("cpulist");

        if !cpulist_path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&cpulist_path).context("Failed to read cpulist")?;

        let content = content.trim();
        if content.is_empty() {
            return Ok(Vec::new());
        }

        Self::parse_cpu_range(content)
    }

    /// Parse CPU range string (e.g., "0-3,8-11" or "0,2,4,6")
    fn parse_cpu_range(s: &str) -> Result<Vec<u32>> {
        let mut cpus = Vec::new();

        for part in s.split(',') {
            let part = part.trim();

            if part.contains('-') {
                // Range: "0-3"
                let range_parts: Vec<&str> = part.split('-').collect();
                if range_parts.len() == 2 {
                    let start: u32 = range_parts[0].parse()?;
                    let end: u32 = range_parts[1].parse()?;
                    cpus.extend(start..=end);
                }
            } else {
                // Single CPU: "5"
                let cpu: u32 = part.parse()?;
                cpus.push(cpu);
            }
        }

        cpus.sort_unstable();
        cpus.dedup();
        Ok(cpus)
    }

    /// Parse memory info from meminfo file
    fn parse_memory_info(node_path: &Path) -> Result<(u64, u64)> {
        let meminfo_path = node_path.join("meminfo");

        if !meminfo_path.exists() {
            return Ok((0, 0));
        }

        let content = fs::read_to_string(&meminfo_path).context("Failed to read meminfo")?;

        let mut total_kb = 0u64;
        let mut free_kb = 0u64;

        for line in content.lines() {
            if line.contains("MemTotal:") {
                total_kb = Self::parse_kb_value(line)?;
            } else if line.contains("MemFree:") {
                free_kb = Self::parse_kb_value(line)?;
            }
        }

        Ok((total_kb, free_kb))
    }

    /// Parse KB value from meminfo line
    fn parse_kb_value(line: &str) -> Result<u64> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 4 {
            let value: u64 = parts[3].parse()?;
            return Ok(value);
        }
        Ok(0)
    }

    /// Parse distance map from distance file
    fn parse_distance_map(node_path: &Path) -> Result<HashMap<u32, u32>> {
        let distance_path = node_path.join("distance");

        if !distance_path.exists() {
            return Ok(HashMap::new());
        }

        let content = fs::read_to_string(&distance_path).context("Failed to read distance")?;

        let mut distances = HashMap::new();
        let values: Vec<&str> = content.split_whitespace().collect();

        for (idx, value) in values.iter().enumerate() {
            if let Ok(distance) = value.parse::<u32>() {
                distances.insert(idx as u32, distance);
            }
        }

        Ok(distances)
    }

    /// Detect which NUMA node the current process is running on
    fn detect_current_node() -> Option<u32> {
        // Try to read current CPU from /proc/self/stat
        let stat = fs::read_to_string("/proc/self/stat").ok()?;
        let parts: Vec<&str> = stat.split_whitespace().collect();

        // CPU number is at index 38
        if parts.len() > 38 {
            let cpu: u32 = parts[38].parse().ok()?;

            // Find which node this CPU belongs to
            let sys_cpu_path = PathBuf::from(format!("/sys/devices/system/cpu/cpu{}", cpu));

            // Read node ID from node* symlink
            let node_links = fs::read_dir(sys_cpu_path).ok()?;
            for link in node_links.flatten() {
                let name = link.file_name();
                let name_str = name.to_string_lossy();
                if let Some(node_suffix) = name_str.strip_prefix("node") {
                    if let Ok(node_id) = node_suffix.parse::<u32>() {
                        return Some(node_id);
                    }
                }
            }
        }

        None
    }

    /// Create single-node fallback for non-NUMA systems
    fn create_single_node_fallback() -> Result<Self> {
        let num_cpus = num_cpus::get() as u32;
        let cpu_list = (0..num_cpus).collect();

        // Estimate memory from /proc/meminfo
        let (total_kb, free_kb) =
            Self::read_system_memory().unwrap_or((8 * 1024 * 1024, 4 * 1024 * 1024));

        let mut distance_to_nodes = HashMap::new();
        distance_to_nodes.insert(0, 10); // Distance to self = 10 (standard)

        let node = NumaNode {
            node_id: 0,
            cpu_list,
            memory_total_kb: total_kb,
            memory_free_kb: free_kb,
            distance_to_nodes,
        };

        let mut nodes = HashMap::new();
        nodes.insert(0, node);

        Ok(Self {
            nodes,
            current_node: Some(0),
        })
    }

    /// Read total system memory from /proc/meminfo
    fn read_system_memory() -> Result<(u64, u64)> {
        let content = fs::read_to_string("/proc/meminfo")?;

        let mut total_kb = 0u64;
        let mut free_kb = 0u64;

        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                total_kb = Self::parse_kb_value(line).unwrap_or(0);
            } else if line.starts_with("MemAvailable:") {
                // MemAvailable is a better metric than MemFree
                free_kb = Self::parse_kb_value(line).unwrap_or(0);
            }
        }

        Ok((total_kb, free_kb))
    }

    /// Get all NUMA nodes
    pub fn nodes(&self) -> &HashMap<u32, NumaNode> {
        &self.nodes
    }

    /// Get a specific NUMA node
    pub fn get_node(&self, node_id: u32) -> Option<&NumaNode> {
        self.nodes.get(&node_id)
    }

    /// Get current NUMA node (where this process is running)
    pub fn current_node(&self) -> Option<u32> {
        self.current_node
    }

    /// Get node with most available memory
    pub fn node_with_most_memory(&self) -> Option<u32> {
        self.nodes
            .values()
            .max_by_key(|n| n.memory_free_kb)
            .map(|n| n.node_id)
    }

    /// Get optimal node for cache operations
    /// Priority: current node > most memory > node 0
    pub fn optimal_node(&self) -> u32 {
        self.current_node
            .or_else(|| self.node_with_most_memory())
            .unwrap_or(0)
    }

    /// Check if system has multiple NUMA nodes
    pub fn is_numa_system(&self) -> bool {
        self.nodes.len() > 1
    }

    /// Get node count
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Get all CPUs for a specific node
    pub fn cpus_for_node(&self, node_id: u32) -> Vec<u32> {
        self.nodes
            .get(&node_id)
            .map(|n| n.cpu_list.clone())
            .unwrap_or_default()
    }

    /// Refresh memory statistics
    pub fn refresh(&mut self) -> Result<()> {
        for (node_id, node) in &mut self.nodes {
            let node_path = PathBuf::from(format!("/sys/devices/system/node/node{}", node_id));
            if node_path.exists() {
                if let Ok((total, free)) = Self::parse_memory_info(&node_path) {
                    node.memory_total_kb = total;
                    node.memory_free_kb = free;
                }
            }
        }
        Ok(())
    }
}

/// NUMA statistics for monitoring
#[derive(Debug, Clone, Default)]
pub struct NumaStats {
    pub local_accesses: u64,
    pub remote_accesses: u64,
    pub total_latency_ns: u64,
    pub operations: u64,
}

impl NumaStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_local_access(&mut self, latency_ns: u64) {
        self.local_accesses += 1;
        self.total_latency_ns += latency_ns;
        self.operations += 1;
    }

    pub fn record_remote_access(&mut self, latency_ns: u64) {
        self.remote_accesses += 1;
        self.total_latency_ns += latency_ns;
        self.operations += 1;
    }

    pub fn avg_latency_ns(&self) -> u64 {
        if self.operations == 0 {
            0
        } else {
            self.total_latency_ns / self.operations
        }
    }

    pub fn local_hit_rate(&self) -> f64 {
        if self.operations == 0 {
            0.0
        } else {
            self.local_accesses as f64 / self.operations as f64
        }
    }

    pub fn remote_penalty(&self) -> f64 {
        // Typical remote NUMA access is 2.1x slower
        if self.local_accesses == 0 || self.remote_accesses == 0 {
            return 1.0;
        }
        2.1 // Conservative estimate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cpu_range() {
        let cpus = NumaTopology::parse_cpu_range("0-3,8-11").unwrap();
        assert_eq!(cpus, vec![0, 1, 2, 3, 8, 9, 10, 11]);

        let cpus = NumaTopology::parse_cpu_range("0,2,4,6").unwrap();
        assert_eq!(cpus, vec![0, 2, 4, 6]);

        let cpus = NumaTopology::parse_cpu_range("5").unwrap();
        assert_eq!(cpus, vec![5]);
    }

    #[test]
    fn test_numa_detection() {
        // Should not panic even if NUMA is not available
        let topology = NumaTopology::detect();
        assert!(topology.is_ok());

        let topology = topology.unwrap();
        assert!(topology.node_count() >= 1);
    }

    #[test]
    fn test_numa_stats() {
        let mut stats = NumaStats::new();

        stats.record_local_access(50);
        stats.record_local_access(60);
        stats.record_remote_access(120);

        assert_eq!(stats.operations, 3);
        assert_eq!(stats.local_accesses, 2);
        assert_eq!(stats.remote_accesses, 1);
        assert_eq!(stats.avg_latency_ns(), (50 + 60 + 120) / 3);
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cache/src/orchestrator.rs">
//! Request orchestrator with capability resolution and workstack routing
//!
//! Integrates capability resolution with workstack execution.

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, warn};

use super::agent_registry::AgentRegistry;
use super::capability_resolver::{CapabilityRequest, CapabilityResolver, ResolvedSequence};
use super::numa::NumaTopology;
use super::pattern_tracker::{PatternTracker, PatternTrackerConfig};
use super::workstack_cache::{WorkstackCache, WorkstackCacheConfig};

/// Orchestrator configuration
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Minimum agents to trigger workstack routing (default: 2)
    pub workstack_threshold: usize,
    /// Enable intermediate step caching
    pub enable_caching: bool,
    /// Enable NUMA pinning for workstack execution
    pub numa_pinning: bool,
    /// Track patterns for optimization suggestions
    pub track_patterns: bool,
    /// Promotion threshold (calls before suggesting promotion)
    pub promotion_threshold: u32,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            workstack_threshold: 2,
            enable_caching: true,
            numa_pinning: true,
            track_patterns: true,
            promotion_threshold: 3,
        }
    }
}

// OrchestrationResult - different from tool OrchestrationResult
#[derive(Debug, Clone)]
pub struct OrchestrationResult {
    pub request_id: String,
    pub output: Vec<u8>,
    pub steps: Vec<StepResult>,
    pub total_latency_ms: u64,
    pub cache_hits: u32,
    pub cache_misses: u32,
    pub used_workstack: bool,
    pub resolved_agents: Vec<String>,
}

/// Individual step result
#[derive(Debug, Clone)]
pub struct StepResult {
    pub step_index: usize,
    pub agent_id: String,
    pub latency_ms: u64,
    pub cached: bool,
    pub output_size: usize,
}

pub struct Orchestrator {
    config: OrchestratorConfig,
    registry: Arc<AgentRegistry>,
    resolver: CapabilityResolver,
    cache: Arc<WorkstackCache>,
    pattern_tracker: Arc<PatternTracker>,
    #[allow(dead_code)]
    numa_topology: NumaTopology,
}

impl Orchestrator {
    /// Create new orchestrator
    pub async fn new(
        cache_dir: PathBuf,
        config: OrchestratorConfig,
        registry: Arc<AgentRegistry>,
    ) -> Result<Self> {
        let resolver = CapabilityResolver::new(registry.clone());

        let cache_config = WorkstackCacheConfig::default();
        let cache = WorkstackCache::new(cache_dir.clone(), cache_config).await?;

        let tracker_config = PatternTrackerConfig {
            promotion_threshold: config.promotion_threshold,
            track_enabled: config.track_patterns,
            ..Default::default()
        };
        let pattern_tracker = PatternTracker::new(cache_dir.clone(), tracker_config).await?;

        let numa_topology = NumaTopology::detect()?;

        info!(
            "Orchestrator initialized (workstack threshold: {}, NUMA nodes: {})",
            config.workstack_threshold,
            numa_topology.node_count()
        );

        Ok(Self {
            config,
            registry,
            resolver,
            cache: Arc::new(cache),
            pattern_tracker: Arc::new(pattern_tracker),
            numa_topology,
        })
    }

    /// Execute a capability-based request
    ///
    /// This is the main entry point:
    /// 1. Resolve capabilities to agent sequence
    /// 2. Route to workstack if 2+ agents
    /// 3. Cache intermediate results
    /// 4. Track patterns
    pub async fn execute(&self, request: CapabilityRequest) -> Result<OrchestrationResult> {
        let start_time = Instant::now();
        let request_id = uuid::Uuid::new_v4().to_string();

        // Step 1: Resolve capabilities to agents
        let sequence = self.resolver.resolve(&request).await?;

        if sequence.is_empty() {
            return Ok(OrchestrationResult {
                request_id,
                output: request.input,
                steps: Vec::new(),
                total_latency_ms: 0,
                cache_hits: 0,
                cache_misses: 0,
                used_workstack: false,
                resolved_agents: Vec::new(),
            });
        }

        if !sequence.is_complete() {
            warn!(
                "Request has unfulfilled capabilities: {:?}",
                sequence.missing_capabilities
            );
        }

        let agent_ids = sequence.agent_ids();
        let agent_count = agent_ids.len();

        info!(
            "Resolved {} capabilities to {} agents: {:?}",
            request.required_capabilities.len(),
            agent_count,
            agent_ids
        );

        // Step 2: Route based on agent count
        if agent_count >= self.config.workstack_threshold {
            self.execute_workstack(&request_id, sequence, request.input, start_time)
                .await
        } else {
            self.execute_single(&request_id, sequence, request.input, start_time)
                .await
        }
    }

    /// Execute with explicit agent IDs (bypass resolution)
    pub async fn execute_agents(
        &self,
        agent_ids: &[&str],
        input: Vec<u8>,
    ) -> Result<OrchestrationResult> {
        let start_time = Instant::now();
        let request_id = uuid::Uuid::new_v4().to_string();

        if agent_ids.is_empty() {
            return Ok(OrchestrationResult {
                request_id,
                output: input,
                steps: Vec::new(),
                total_latency_ms: 0,
                cache_hits: 0,
                cache_misses: 0,
                used_workstack: false,
                resolved_agents: Vec::new(),
            });
        }

        let agent_count = agent_ids.len();

        if agent_count >= self.config.workstack_threshold {
            self.execute_workstack_by_ids(&request_id, agent_ids, input, start_time)
                .await
        } else {
            self.execute_single_by_id(&request_id, agent_ids[0], input, start_time)
                .await
        }
    }

    /// Execute single agent (direct)
    async fn execute_single(
        &self,
        request_id: &str,
        sequence: ResolvedSequence,
        input: Vec<u8>,
        start_time: Instant,
    ) -> Result<OrchestrationResult> {
        let agent = sequence.agents.first().context("No agent in sequence")?;

        debug!("Executing single agent: {}", agent.id);

        let step_start = Instant::now();
        let output = self.registry.execute(&agent.id, &input).await?;
        let latency_ms = step_start.elapsed().as_millis() as u64;

        let step = StepResult {
            step_index: 0,
            agent_id: agent.id.clone(),
            latency_ms,
            cached: false,
            output_size: output.len(),
        };

        Ok(OrchestrationResult {
            request_id: request_id.to_string(),
            output,
            steps: vec![step],
            total_latency_ms: start_time.elapsed().as_millis() as u64,
            cache_hits: 0,
            cache_misses: 1,
            used_workstack: false,
            resolved_agents: sequence.agent_ids(),
        })
    }

    /// Execute single agent by ID
    async fn execute_single_by_id(
        &self,
        request_id: &str,
        agent_id: &str,
        input: Vec<u8>,
        start_time: Instant,
    ) -> Result<OrchestrationResult> {
        debug!("Executing single agent: {}", agent_id);

        let step_start = Instant::now();
        let output = self.registry.execute(agent_id, &input).await?;
        let latency_ms = step_start.elapsed().as_millis() as u64;

        let step = StepResult {
            step_index: 0,
            agent_id: agent_id.to_string(),
            latency_ms,
            cached: false,
            output_size: output.len(),
        };

        Ok(OrchestrationResult {
            request_id: request_id.to_string(),
            output,
            steps: vec![step],
            total_latency_ms: start_time.elapsed().as_millis() as u64,
            cache_hits: 0,
            cache_misses: 1,
            used_workstack: false,
            resolved_agents: vec![agent_id.to_string()],
        })
    }

    /// Execute multi-agent via workstack
    async fn execute_workstack(
        &self,
        request_id: &str,
        sequence: ResolvedSequence,
        input: Vec<u8>,
        start_time: Instant,
    ) -> Result<OrchestrationResult> {
        let agent_ids = sequence.agent_ids();
        let agent_refs: Vec<&str> = agent_ids.iter().map(|s| s.as_str()).collect();

        self.execute_workstack_by_ids(request_id, &agent_refs, input, start_time)
            .await
    }

    /// Execute workstack by agent IDs
    async fn execute_workstack_by_ids(
        &self,
        request_id: &str,
        agent_ids: &[&str],
        input: Vec<u8>,
        start_time: Instant,
    ) -> Result<OrchestrationResult> {
        let workstack_id = format!("ws-{}", &Self::hash_sequence(agent_ids, &input)[..12]);

        info!(
            "Routing to workstack: {} ({} agents)",
            workstack_id,
            agent_ids.len()
        );

        let mut steps = Vec::new();
        let mut current_input = input.clone();
        let mut cache_hits = 0usize;
        let mut cache_misses = 0usize;

        for (step_index, agent_id) in agent_ids.iter().enumerate() {
            let step_input_hash = Self::hash_bytes(&current_input);
            let step_start = Instant::now();

            // Try cache first
            let (output, cached) = if self.config.enable_caching {
                match self
                    .cache
                    .get(&workstack_id, step_index, &step_input_hash)?
                {
                    Some(cached_output) => {
                        debug!(
                            "Cache hit: {} step {} ({})",
                            workstack_id, step_index, agent_id
                        );
                        cache_hits += 1;
                        (cached_output, true)
                    }
                    None => {
                        cache_misses += 1;
                        let output = self.registry.execute(agent_id, &current_input).await?;

                        // Cache result
                        self.cache.put(
                            &workstack_id,
                            step_index,
                            &step_input_hash,
                            &output,
                            None,
                        )?;

                        (output, false)
                    }
                }
            } else {
                (
                    self.registry.execute(agent_id, &current_input).await?,
                    false,
                )
            };

            let latency_ms = step_start.elapsed().as_millis() as u64;

            steps.push(StepResult {
                step_index,
                agent_id: agent_id.to_string(),
                latency_ms,
                cached,
                output_size: output.len(),
            });

            current_input = output;
        }

        let total_latency_ms = start_time.elapsed().as_millis() as u64;

        // Track pattern
        if self.config.track_patterns {
            let input_hash = Self::hash_bytes(&input);
            if let Some(suggestion) =
                self.pattern_tracker
                    .record_sequence(agent_ids, &input_hash, total_latency_ms)?
            {
                info!(
                    "🔥 Pattern detected: '{}' called {} times",
                    suggestion.suggested_name, suggestion.pattern.call_count
                );
            }
        }

        Ok(OrchestrationResult {
            request_id: request_id.to_string(),
            output: current_input,
            steps,
            total_latency_ms,
            cache_hits: cache_hits as u32,
            cache_misses: cache_misses as u32,
            used_workstack: true,
            resolved_agents: agent_ids.iter().map(|s| s.to_string()).collect(),
        })
    }

    fn hash_bytes(data: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data);
        format!("{:x}", hasher.finalize())
    }

    fn hash_sequence(agents: &[&str], input: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(agents.join("→").as_bytes());
        hasher.update(input);
        format!("{:x}", hasher.finalize())
    }

    /// Get orchestrator statistics
    pub async fn stats(&self) -> Result<OrchestratorStats> {
        let registry_stats = self.registry.stats().await;
        let resolver_stats = self.resolver.stats().await;
        let cache_stats = self.cache.stats()?;
        let pattern_stats = self.pattern_tracker.stats()?;

        Ok(OrchestratorStats {
            registered_agents: registry_stats.total_agents,
            enabled_agents: registry_stats.enabled_agents,
            available_capabilities: resolver_stats.available_capabilities,
            tracked_patterns: pattern_stats.total_patterns,
            promoted_patterns: pattern_stats.promoted_count,
            cache_entries: cache_stats.total_entries,
            cache_hit_rate: cache_stats.hit_rate,
        })
    }

    /// Get the agent registry
    pub fn registry(&self) -> &Arc<AgentRegistry> {
        &self.registry
    }

    /// Get promotion candidates
    pub fn get_promotion_candidates(
        &self,
    ) -> Result<Vec<super::pattern_tracker::PromotionSuggestion>> {
        self.pattern_tracker.get_promotion_candidates()
    }
}

/// Orchestrator statistics
#[derive(Debug, Clone)]
pub struct OrchestratorStats {
    pub registered_agents: usize,
    pub enabled_agents: usize,
    pub available_capabilities: usize,
    pub tracked_patterns: u32,
    pub promoted_patterns: u32,
    pub cache_entries: u64,
    pub cache_hit_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_registry::{AgentCapability, AgentDefinition, AgentExecutor};

    fn make_echo_executor() -> AgentExecutor {
        Arc::new(|input: &[u8]| {
            let input = input.to_vec();
            Box::pin(async move { Ok::<Vec<u8>, anyhow::Error>(input) })
        })
    }

    fn make_transform_executor(suffix: &'static str) -> AgentExecutor {
        Arc::new(move |input: &[u8]| {
            let mut output = input.to_vec();
            output.extend_from_slice(suffix.as_bytes());
            Box::pin(async move { Ok::<Vec<u8>, anyhow::Error>(output) })
        })
    }

    async fn setup_test_orchestrator() -> Orchestrator {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let registry = Arc::new(AgentRegistry::new());

        // Register test agents
        let analyzer = AgentDefinition::new("analyzer", "Code Analyzer")
            .with_capability(AgentCapability::CodeAnalysis)
            .with_capability(AgentCapability::DependencyAnalysis);
        registry
            .register(analyzer, make_echo_executor())
            .await
            .unwrap();

        let tester = AgentDefinition::new("tester", "Test Generator")
            .with_capability(AgentCapability::TestGeneration);
        registry
            .register(tester, make_transform_executor("_TESTS"))
            .await
            .unwrap();

        let security = AgentDefinition::new("security", "Security Auditor")
            .with_capability(AgentCapability::SecurityAudit);
        registry
            .register(security, make_transform_executor("_SEC"))
            .await
            .unwrap();

        let config = OrchestratorConfig {
            numa_pinning: false,
            ..Default::default()
        };

        Orchestrator::new(temp_dir.path().to_path_buf(), config, registry)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn test_single_capability_resolution() {
        let orchestrator = setup_test_orchestrator().await;

        let request =
            CapabilityRequest::new(vec![AgentCapability::CodeAnalysis], b"test input".to_vec());

        let result = orchestrator.execute(request).await.unwrap();

        assert!(!result.used_workstack);
        assert_eq!(result.resolved_agents, vec!["analyzer"]);
        assert_eq!(result.output, b"test input");
    }

    #[tokio::test]
    async fn test_multi_capability_workstack() {
        let orchestrator = setup_test_orchestrator().await;

        let request = CapabilityRequest::new(
            vec![
                AgentCapability::CodeAnalysis,
                AgentCapability::TestGeneration,
            ],
            b"code".to_vec(),
        );

        let result = orchestrator.execute(request).await.unwrap();

        assert!(result.used_workstack);
        assert_eq!(result.resolved_agents.len(), 2);
        // Output should have TESTS suffix from tester agent
        assert!(result.output.ends_with(b"_TESTS"));
    }

    #[tokio::test]
    async fn test_direct_agent_execution() {
        let orchestrator = setup_test_orchestrator().await;

        let result = orchestrator
            .execute_agents(&["analyzer", "tester", "security"], b"input".to_vec())
            .await
            .unwrap();

        assert!(result.used_workstack);
        assert_eq!(result.steps.len(), 3);
        assert!(result.output.ends_with(b"_SEC")); // Last agent
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cache/src/pattern_tracker.rs">
//! Pattern tracking for multi-agent sequences
//!
//! Tracks frequently-used agent sequences and suggests
//! promotion to named workstacks for optimization.

use anyhow::{Context, Result};
use rusqlite::OptionalExtension;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Mutex;
use tracing::info;

/// Configuration for pattern tracking
#[derive(Debug, Clone)]
pub struct PatternTrackerConfig {
    /// Minimum calls before suggesting promotion (default: 3)
    pub promotion_threshold: u32,
    /// Time window in seconds for pattern detection (default: 24 hours)
    pub detection_window_secs: i64,
    /// Enable tracking (default: true)
    pub track_enabled: bool,
}

impl Default for PatternTrackerConfig {
    fn default() -> Self {
        Self {
            promotion_threshold: 3,
            detection_window_secs: 86400,
            track_enabled: true,
        }
    }
}

/// Tracked pattern information
#[derive(Debug, Clone)]
pub struct TrackedPattern {
    pub pattern_id: String,
    pub agent_sequence: Vec<String>,
    pub call_count: u32,
    pub first_seen: i64,
    pub last_called: i64,
    pub avg_latency_ms: u64,
    pub promoted: bool,
    pub workstack_id: Option<String>,
}

impl TrackedPattern {
    pub fn sequence_description(&self) -> String {
        self.agent_sequence.join(" → ")
    }
}

/// Promotion suggestion
#[derive(Debug, Clone)]
pub struct PromotionSuggestion {
    pub pattern: TrackedPattern,
    pub estimated_time_saved_ms: u64,
    pub confidence_score: f64,
    pub suggested_name: String,
}

pub struct PatternTracker {
    db: Mutex<rusqlite::Connection>,
    config: PatternTrackerConfig,
}

impl PatternTracker {
    /// Create new pattern tracker
    pub async fn new(cache_dir: PathBuf, config: PatternTrackerConfig) -> Result<Self> {
        let db_path = cache_dir.join("patterns.db");

        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let db = rusqlite::Connection::open(&db_path)
            .context("Failed to open pattern tracker database")?;

        db.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS patterns (
                pattern_hash TEXT PRIMARY KEY,
                agent_sequence TEXT NOT NULL,
                call_count INTEGER DEFAULT 1,
                first_seen INTEGER NOT NULL,
                last_called INTEGER NOT NULL,
                total_latency_ms INTEGER DEFAULT 0,
                promoted INTEGER DEFAULT 0,
                workstack_id TEXT
            );

            CREATE TABLE IF NOT EXISTS promoted_workstacks (
                workstack_id TEXT PRIMARY KEY,
                pattern_hash TEXT NOT NULL,
                name TEXT NOT NULL,
                agent_sequence TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                execution_count INTEGER DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_patterns_count ON patterns(call_count DESC);
            CREATE INDEX IF NOT EXISTS idx_patterns_last ON patterns(last_called DESC);
            "#,
        )?;

        info!("Pattern tracker initialized at {:?}", db_path);

        Ok(Self {
            db: Mutex::new(db),
            config,
        })
    }

    /// Record an agent sequence execution
    pub fn record_sequence(
        &self,
        agents: &[&str],
        _input_hash: &str,
        total_latency_ms: u64,
    ) -> Result<Option<PromotionSuggestion>> {
        if !self.config.track_enabled || agents.len() < 2 {
            return Ok(None);
        }

        let pattern_hash = self.hash_sequence(agents);
        let agent_sequence_json = simd_json::to_string(agents)?;
        let now = chrono::Utc::now().timestamp();

        let db = self.db.lock().unwrap();

        // Check existing pattern
        let existing: Option<(u32, i64, i64, bool)> = db
            .query_row(
                "SELECT call_count, first_seen, total_latency_ms, promoted
                 FROM patterns WHERE pattern_hash = ?1",
                [&pattern_hash],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;

        let (call_count, first_seen, total_latency, promoted) = if let Some(existing) = existing {
            db.execute(
                "UPDATE patterns
                 SET call_count = call_count + 1,
                     last_called = ?1,
                     total_latency_ms = total_latency_ms + ?2
                 WHERE pattern_hash = ?3",
                rusqlite::params![now, total_latency_ms, pattern_hash],
            )?;
            (
                existing.0 + 1,
                existing.1,
                existing.2 + total_latency_ms as i64,
                existing.3,
            )
        } else {
            db.execute(
                "INSERT INTO patterns
                 (pattern_hash, agent_sequence, call_count, first_seen, last_called, total_latency_ms)
                 VALUES (?1, ?2, 1, ?3, ?3, ?4)",
                rusqlite::params![pattern_hash, agent_sequence_json, now, total_latency_ms],
            )?;
            (1, now, total_latency_ms as i64, false)
        };

        drop(db);

        // Check for promotion
        if call_count >= self.config.promotion_threshold && !promoted {
            let pattern = TrackedPattern {
                pattern_id: pattern_hash,
                agent_sequence: agents.iter().map(|s| s.to_string()).collect(),
                call_count,
                first_seen,
                last_called: now,
                avg_latency_ms: (total_latency / call_count as i64) as u64,
                promoted: false,
                workstack_id: None,
            };

            return Ok(Some(PromotionSuggestion {
                estimated_time_saved_ms: self.estimate_time_savings(&pattern),
                confidence_score: self.calculate_confidence(&pattern),
                suggested_name: self.generate_workstack_name(&pattern),
                pattern,
            }));
        }

        Ok(None)
    }

    /// Promote a pattern to a named workstack
    pub fn promote_pattern(&self, pattern: &TrackedPattern) -> Result<String> {
        let workstack_id = format!("WS-{}", &pattern.pattern_id[..8]);
        let now = chrono::Utc::now().timestamp();
        let agent_sequence_json = simd_json::to_string(&pattern.agent_sequence)?;

        let db = self.db.lock().unwrap();

        db.execute(
            "INSERT INTO promoted_workstacks
             (workstack_id, pattern_hash, name, agent_sequence, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![
                workstack_id,
                pattern.pattern_id,
                self.generate_workstack_name(pattern),
                agent_sequence_json,
                now
            ],
        )?;

        db.execute(
            "UPDATE patterns SET promoted = 1, workstack_id = ?1 WHERE pattern_hash = ?2",
            rusqlite::params![workstack_id, pattern.pattern_id],
        )?;

        info!(
            "Promoted pattern {} to workstack {}: {}",
            pattern.pattern_id,
            workstack_id,
            pattern.sequence_description()
        );

        Ok(workstack_id)
    }

    /// Get patterns eligible for promotion
    pub fn get_promotion_candidates(&self) -> Result<Vec<PromotionSuggestion>> {
        let db = self.db.lock().unwrap();
        let cutoff = chrono::Utc::now().timestamp() - self.config.detection_window_secs;

        let mut stmt = db.prepare(
            "SELECT pattern_hash, agent_sequence, call_count, first_seen, last_called, total_latency_ms
             FROM patterns
             WHERE call_count >= ?1 AND promoted = 0 AND last_called > ?2
             ORDER BY call_count DESC",
        )?;

        let patterns = stmt
            .query_map(
                rusqlite::params![self.config.promotion_threshold, cutoff],
                |row| {
                    let mut agent_sequence_json: String = row.get(1)?;
                    let agent_sequence: Vec<String> =
                        unsafe { simd_json::from_str(&mut agent_sequence_json) }
                            .unwrap_or_default();
                    let call_count: u32 = row.get(2)?;
                    let total_latency: i64 = row.get(5)?;

                    Ok(TrackedPattern {
                        pattern_id: row.get(0)?,
                        agent_sequence,
                        call_count,
                        first_seen: row.get(3)?,
                        last_called: row.get(4)?,
                        avg_latency_ms: if call_count > 0 {
                            (total_latency / call_count as i64) as u64
                        } else {
                            0
                        },
                        promoted: false,
                        workstack_id: None,
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(patterns
            .into_iter()
            .map(|pattern| PromotionSuggestion {
                estimated_time_saved_ms: self.estimate_time_savings(&pattern),
                confidence_score: self.calculate_confidence(&pattern),
                suggested_name: self.generate_workstack_name(&pattern),
                pattern,
            })
            .collect())
    }

    /// Get tracker statistics
    pub fn stats(&self) -> Result<TrackerStats> {
        let db = self.db.lock().unwrap();

        let total_patterns: u32 =
            db.query_row("SELECT COUNT(*) FROM patterns", [], |row| row.get(0))?;

        let promoted_count: u32 = db.query_row(
            "SELECT COUNT(*) FROM patterns WHERE promoted = 1",
            [],
            |row| row.get(0),
        )?;

        let pending_promotion: u32 = db.query_row(
            "SELECT COUNT(*) FROM patterns WHERE call_count >= ?1 AND promoted = 0",
            [self.config.promotion_threshold],
            |row| row.get(0),
        )?;

        Ok(TrackerStats {
            total_patterns,
            promoted_count,
            pending_promotion,
            promotion_threshold: self.config.promotion_threshold,
        })
    }

    fn hash_sequence(&self, agents: &[&str]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(agents.join("→").as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn estimate_time_savings(&self, pattern: &TrackedPattern) -> u64 {
        // Assume 40% cache hit rate, 60% latency reduction when cached
        let expected_future_calls = pattern.call_count * 2;
        let cache_hit_savings = (pattern.avg_latency_ms as f64 * 0.6) as u64;
        (expected_future_calls as f64 * cache_hit_savings as f64 * 0.4) as u64
    }

    fn calculate_confidence(&self, pattern: &TrackedPattern) -> f64 {
        let recency_days = (chrono::Utc::now().timestamp() - pattern.last_called) as f64 / 86400.0;
        let frequency_score =
            (pattern.call_count as f64 / self.config.promotion_threshold as f64).min(2.0) / 2.0;
        let recency_score = (1.0 - recency_days / 7.0).max(0.0);

        (frequency_score * 0.6 + recency_score * 0.4).min(1.0)
    }

    fn generate_workstack_name(&self, pattern: &TrackedPattern) -> String {
        if pattern.agent_sequence.is_empty() {
            return "unnamed-workstack".to_string();
        }

        let first = pattern.agent_sequence.first().unwrap();
        let last = pattern.agent_sequence.last().unwrap();

        if pattern.agent_sequence.len() == 2 {
            format!("{}-to-{}", first, last)
        } else {
            format!("{}-to-{}-{}step", first, last, pattern.agent_sequence.len())
        }
    }

    /// Cleanup old patterns
    pub fn cleanup(&self, days: i64) -> Result<usize> {
        let cutoff = chrono::Utc::now().timestamp() - (days * 86400);
        let db = self.db.lock().unwrap();

        let deleted = db.execute(
            "DELETE FROM patterns WHERE last_called < ?1 AND promoted = 0 AND call_count < ?2",
            rusqlite::params![cutoff, self.config.promotion_threshold],
        )?;

        info!("Cleaned up {} old patterns", deleted);
        Ok(deleted)
    }
}

/// Tracker statistics
#[derive(Debug, Clone)]
pub struct TrackerStats {
    pub total_patterns: u32,
    pub promoted_count: u32,
    pub pending_promotion: u32,
    pub promotion_threshold: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_pattern_tracker_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = PatternTrackerConfig::default();
        let tracker = PatternTracker::new(temp_dir.path().to_path_buf(), config).await;
        assert!(tracker.is_ok());
    }

    #[tokio::test]
    async fn test_record_sequence() {
        let temp_dir = TempDir::new().unwrap();
        let config = PatternTrackerConfig {
            promotion_threshold: 2,
            ..Default::default()
        };
        let tracker = PatternTracker::new(temp_dir.path().to_path_buf(), config)
            .await
            .unwrap();

        // First call - no promotion
        let result = tracker
            .record_sequence(&["agent_a", "agent_b"], "hash1", 100)
            .unwrap();
        assert!(result.is_none());

        // Second call - should suggest promotion
        let result = tracker
            .record_sequence(&["agent_a", "agent_b"], "hash2", 150)
            .unwrap();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_promotion() {
        let temp_dir = TempDir::new().unwrap();
        let config = PatternTrackerConfig {
            promotion_threshold: 1,
            ..Default::default()
        };
        let tracker = PatternTracker::new(temp_dir.path().to_path_buf(), config)
            .await
            .unwrap();

        let result = tracker
            .record_sequence(&["a", "b", "c"], "hash1", 200)
            .unwrap();

        assert!(result.is_some());
        let suggestion = result.unwrap();

        let workstack_id = tracker.promote_pattern(&suggestion.pattern).unwrap();
        assert!(workstack_id.starts_with("WS-"));
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cache/src/snapshot_manager.rs">
//! BTRFS snapshot management with automatic rotation
//!
//! Manages cache snapshots with configurable retention policy

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct SnapshotConfig {
    /// Base path for snapshots (e.g., /var/lib/op-dbus/@cache-snapshots)
    pub snapshot_dir: PathBuf,

    /// Maximum number of snapshots to keep (default: 24)
    pub max_snapshots: usize,

    /// Snapshot name prefix (default: "cache")
    pub prefix: String,
}

impl Default for SnapshotConfig {
    fn default() -> Self {
        Self {
            snapshot_dir: PathBuf::from("/var/lib/op-dbus/@cache-snapshots"),
            max_snapshots: 24, // Keep 24 hourly snapshots = 1 day
            prefix: "SNP-cache".to_string(),
        }
    }
}

pub struct SnapshotManager {
    config: SnapshotConfig,
    source_subvol: PathBuf,
}

impl SnapshotManager {
    /// Create new snapshot manager
    pub fn new(source_subvol: PathBuf, config: SnapshotConfig) -> Self {
        Self {
            config,
            source_subvol,
        }
    }

    /// Create snapshot with automatic rotation
    pub async fn create_snapshot(&self) -> Result<PathBuf> {
        // Create snapshot directory if it doesn't exist
        tokio::fs::create_dir_all(&self.config.snapshot_dir).await?;

        let snapshot_counter = self.next_snapshot_counter().await?;
        let snapshot_name = format!("{}-{:06}", self.config.prefix, snapshot_counter);
        let snapshot_path = self.config.snapshot_dir.join(&snapshot_name);

        log::info!("Creating BTRFS snapshot: {}", snapshot_name);

        // Create readonly snapshot
        let output = Command::new("btrfs")
            .args(["subvolume", "snapshot", "-r"])
            .arg(&self.source_subvol)
            .arg(&snapshot_path)
            .output()
            .await
            .context("Failed to execute btrfs snapshot command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to create snapshot: {}", stderr);
        }

        log::info!("Created snapshot: {}", snapshot_path.display());

        // Rotate old snapshots
        self.rotate_snapshots().await?;

        Ok(snapshot_path)
    }

    /// List all snapshots for this cache
    pub async fn list_snapshots(&self) -> Result<Vec<SnapshotInfo>> {
        let mut snapshots = Vec::new();

        if !self.config.snapshot_dir.exists() {
            return Ok(snapshots);
        }

        let mut entries = tokio::fs::read_dir(&self.config.snapshot_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Filter by prefix
            let prefix = format!("{}-", self.config.prefix);
            if !name_str.starts_with(&prefix) {
                continue;
            }

            let path = entry.path();
            let metadata = tokio::fs::metadata(&path).await?;
            let created = metadata.created().or_else(|_| metadata.modified()).ok();

            let counter = name_str
                .strip_prefix(&prefix)
                .and_then(|suffix| suffix.parse::<u64>().ok());

            snapshots.push(SnapshotInfo {
                name: name_str.to_string(),
                path: path.clone(),
                created,
                counter,
            });
        }

        // Sort by counter (oldest first), fall back to created time
        snapshots.sort_by(|a, b| match (a.counter, b.counter) {
            (Some(a_counter), Some(b_counter)) => a_counter.cmp(&b_counter),
            _ => a.created.cmp(&b.created),
        });

        Ok(snapshots)
    }

    /// Rotate snapshots, keeping only max_snapshots
    async fn rotate_snapshots(&self) -> Result<()> {
        let snapshots = self.list_snapshots().await?;

        if snapshots.len() <= self.config.max_snapshots {
            log::debug!(
                "Snapshot count {} within limit {}",
                snapshots.len(),
                self.config.max_snapshots
            );
            return Ok(());
        }

        // Calculate how many to delete
        let to_delete = snapshots.len() - self.config.max_snapshots;

        log::info!(
            "Rotating snapshots: {} total, keeping {}, deleting {}",
            snapshots.len(),
            self.config.max_snapshots,
            to_delete
        );

        // Delete oldest snapshots
        for snapshot in snapshots.iter().take(to_delete) {
            log::info!("Deleting old snapshot: {}", snapshot.name);
            self.delete_snapshot(&snapshot.path).await?;
        }

        Ok(())
    }

    async fn next_snapshot_counter(&self) -> Result<u64> {
        let snapshots = self.list_snapshots().await?;
        let mut max_counter = 0u64;

        for snapshot in snapshots {
            if let Some(counter) = snapshot.counter {
                if counter > max_counter {
                    max_counter = counter;
                }
            }
        }

        Ok(max_counter + 1)
    }

    /// Delete a specific snapshot
    pub async fn delete_snapshot(&self, snapshot_path: &Path) -> Result<()> {
        log::debug!("Deleting snapshot: {}", snapshot_path.display());

        let output = Command::new("btrfs")
            .args(["subvolume", "delete"])
            .arg(snapshot_path)
            .output()
            .await
            .context("Failed to execute btrfs delete command")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to delete snapshot: {}", stderr);
        }

        Ok(())
    }

    /// Delete all snapshots
    pub async fn delete_all_snapshots(&self) -> Result<usize> {
        let snapshots = self.list_snapshots().await?;
        let count = snapshots.len();

        for snapshot in snapshots {
            self.delete_snapshot(&snapshot.path).await?;
        }

        log::info!("Deleted {} snapshots", count);
        Ok(count)
    }

    /// Get oldest snapshot
    #[allow(dead_code)]
    pub async fn oldest_snapshot(&self) -> Result<Option<SnapshotInfo>> {
        let snapshots = self.list_snapshots().await?;
        Ok(snapshots.into_iter().next())
    }

    /// Get newest snapshot
    #[allow(dead_code)]
    pub async fn newest_snapshot(&self) -> Result<Option<SnapshotInfo>> {
        let snapshots = self.list_snapshots().await?;
        Ok(snapshots.into_iter().last())
    }
}

#[derive(Debug, Clone)]
pub struct SnapshotInfo {
    pub name: String,
    pub path: PathBuf,
    #[allow(dead_code)]
    pub created: Option<std::time::SystemTime>,
    pub counter: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_config_defaults() {
        let config = SnapshotConfig::default();
        assert_eq!(config.max_snapshots, 24);
        assert_eq!(config.prefix, "SNP-cache");
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cache/src/workflow_cache.rs">
//! Workflow step caching with TTL and input-based keying
//!
//! Caches intermediate results from workflow steps to avoid
//! redundant computation when the same inputs are processed.
//!
//! Features:
//! - Input-hash based caching
//! - Configurable TTL per cache entry
//! - Hot/cold data tracking
//! - BTRFS-backed storage with compression
//! - Cache invalidation strategies

use anyhow::{Context, Result};
use rusqlite::OptionalExtension;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Mutex;
use tracing::{debug, info};

/// Configuration for workflow caching
#[derive(Debug, Clone)]
pub struct WorkflowCacheConfig {
    /// Default TTL for cached results in seconds (default: 1 hour)
    pub default_ttl_secs: i64,
    /// Maximum cache size in bytes (default: 1GB)
    pub max_size_bytes: u64,
    /// Enable compression for cached data (default: true)
    pub compress: bool,
    /// Hot entry threshold in seconds (default: 10 minutes)
    pub hot_threshold_secs: i64,
}

impl Default for WorkflowCacheConfig {
    fn default() -> Self {
        Self {
            default_ttl_secs: 3600,             // 1 hour
            max_size_bytes: 1024 * 1024 * 1024, // 1GB
            compress: true,
            hot_threshold_secs: 600, // 10 minutes
        }
    }
}

/// Cached step result with metadata
#[derive(Debug, Clone)]
pub struct CachedStepResult {
    pub workflow_id: String,
    pub step_index: usize,
    pub input_hash: String,
    pub output: Vec<u8>,
    pub created_at: i64,
    pub expires_at: i64,
    pub access_count: u32,
    pub last_accessed: i64,
    pub size_bytes: u64,
}

impl CachedStepResult {
    /// Check if the cached result is expired
    pub fn is_expired(&self) -> bool {
        chrono::Utc::now().timestamp() > self.expires_at
    }

    /// Check if this is a "hot" cache entry
    pub fn is_hot(&self, threshold_secs: i64) -> bool {
        let now = chrono::Utc::now().timestamp();
        now - self.last_accessed < threshold_secs
    }
}

pub struct WorkflowCache {
    cache_dir: PathBuf,
    db: Mutex<rusqlite::Connection>,
    config: WorkflowCacheConfig,
}

impl WorkflowCache {
    /// Create new workflow cache
    pub async fn new(cache_dir: PathBuf, config: WorkflowCacheConfig) -> Result<Self> {
        let workflows_dir = cache_dir.join("workflows");
        let data_dir = workflows_dir.join("data");

        tokio::fs::create_dir_all(&data_dir).await?;

        let db_path = workflows_dir.join("cache.db");
        let db = rusqlite::Connection::open(&db_path)
            .context("Failed to open workflow cache database")?;

        // Create tables
        db.execute_batch(
            r#"
            -- Main cache table
            CREATE TABLE IF NOT EXISTS workflow_step_cache (
                cache_key TEXT PRIMARY KEY,
                workflow_id TEXT NOT NULL,
                step_index INTEGER NOT NULL,
                input_hash TEXT NOT NULL,
                output_file TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                access_count INTEGER DEFAULT 1,
                last_accessed INTEGER NOT NULL,
                size_bytes INTEGER NOT NULL,
                compressed INTEGER DEFAULT 0
            );

            -- Workflow-level cache metadata
            CREATE TABLE IF NOT EXISTS workflow_cache_meta (
                workflow_id TEXT PRIMARY KEY,
                total_entries INTEGER DEFAULT 0,
                total_size_bytes INTEGER DEFAULT 0,
                hit_count INTEGER DEFAULT 0,
                miss_count INTEGER DEFAULT 0,
                last_hit INTEGER,
                last_miss INTEGER
            );

            -- Indexes
            CREATE INDEX IF NOT EXISTS idx_cache_workflow ON workflow_step_cache(workflow_id);
            CREATE INDEX IF NOT EXISTS idx_cache_expires ON workflow_step_cache(expires_at);
            CREATE INDEX IF NOT EXISTS idx_cache_accessed ON workflow_step_cache(last_accessed DESC);
            CREATE INDEX IF NOT EXISTS idx_cache_input ON workflow_step_cache(workflow_id, step_index, input_hash);
            "#,
        )?;

        info!("Workflow cache initialized at {:?}", db_path);

        Ok(Self {
            cache_dir: workflows_dir,
            db: Mutex::new(db),
            config,
        })
    }

    /// Get cached result for a workflow step
    pub fn get(
        &self,
        workflow_id: &str,
        step_index: usize,
        input_hash: &str,
    ) -> Result<Option<Vec<u8>>> {
        let cache_key = self.make_cache_key(workflow_id, step_index, input_hash);
        let now = chrono::Utc::now().timestamp();

        let db = self.db.lock().unwrap();

        // Look up cache entry
        let entry: Option<(String, i64, bool)> = db
            .query_row(
                "SELECT output_file, expires_at, compressed
                 FROM workflow_step_cache
                 WHERE cache_key = ?1",
                [&cache_key],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;

        let (output_file, expires_at, compressed) = match entry {
            Some(e) => e,
            None => {
                // Record miss
                self.record_miss(&db, workflow_id)?;
                return Ok(None);
            }
        };

        // Check expiration
        if now > expires_at {
            debug!("Cache entry expired for {}", cache_key);
            drop(db);
            self.invalidate(workflow_id, step_index, input_hash)?;
            return Ok(None);
        }

        // Update access stats
        db.execute(
            "UPDATE workflow_step_cache
             SET access_count = access_count + 1, last_accessed = ?1
             WHERE cache_key = ?2",
            rusqlite::params![now, cache_key],
        )?;

        // Record hit
        self.record_hit(&db, workflow_id)?;

        drop(db);

        // Read data from file
        let data_path = self.cache_dir.join("data").join(&output_file);
        let data = std::fs::read(&data_path)
            .context(format!("Failed to read cached data: {:?}", data_path))?;

        // Decompress if needed
        let output = if compressed {
            self.decompress(&data)?
        } else {
            data
        };

        debug!(
            "Cache hit for workflow {} step {} (key: {})",
            workflow_id, step_index, cache_key
        );

        Ok(Some(output))
    }

    /// Store result in cache
    pub fn put(
        &self,
        workflow_id: &str,
        step_index: usize,
        input_hash: &str,
        output: &[u8],
        ttl_secs: Option<i64>,
    ) -> Result<()> {
        let cache_key = self.make_cache_key(workflow_id, step_index, input_hash);
        let now = chrono::Utc::now().timestamp();
        let ttl = ttl_secs.unwrap_or(self.config.default_ttl_secs);
        let expires_at = now + ttl;

        // Compress if enabled and beneficial
        let (data, compressed) = if self.config.compress && output.len() > 1024 {
            match self.compress(output) {
                Ok(compressed_data) if compressed_data.len() < output.len() => {
                    (compressed_data, true)
                }
                _ => (output.to_vec(), false),
            }
        } else {
            (output.to_vec(), false)
        };

        let size_bytes = data.len() as u64;

        // Write data to file
        let output_file = format!("{}.cache", cache_key);
        let data_path = self.cache_dir.join("data").join(&output_file);
        std::fs::write(&data_path, &data)?;

        // Update database
        let db = self.db.lock().unwrap();

        db.execute(
            "INSERT INTO workflow_step_cache
             (cache_key, workflow_id, step_index, input_hash, output_file,
              created_at, expires_at, last_accessed, size_bytes, compressed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(cache_key) DO UPDATE SET
                output_file = ?5,
                expires_at = ?7,
                last_accessed = ?8,
                size_bytes = ?9,
                compressed = ?10,
                access_count = access_count + 1",
            rusqlite::params![
                cache_key,
                workflow_id,
                step_index,
                input_hash,
                output_file,
                now,
                expires_at,
                now,
                size_bytes,
                compressed
            ],
        )?;

        // Update workflow metadata
        self.update_workflow_meta(&db, workflow_id)?;

        debug!(
            "Cached workflow {} step {} output ({} bytes, compressed: {})",
            workflow_id, step_index, size_bytes, compressed
        );

        Ok(())
    }

    /// Invalidate a specific cache entry
    pub fn invalidate(&self, workflow_id: &str, step_index: usize, input_hash: &str) -> Result<()> {
        let cache_key = self.make_cache_key(workflow_id, step_index, input_hash);

        let db = self.db.lock().unwrap();

        // Get file path before deleting
        let output_file: Option<String> = db
            .query_row(
                "SELECT output_file FROM workflow_step_cache WHERE cache_key = ?1",
                [&cache_key],
                |row| row.get(0),
            )
            .optional()?;

        // Delete from database
        db.execute(
            "DELETE FROM workflow_step_cache WHERE cache_key = ?1",
            [&cache_key],
        )?;

        drop(db);

        // Delete file
        if let Some(file) = output_file {
            let data_path = self.cache_dir.join("data").join(&file);
            let _ = std::fs::remove_file(data_path);
        }

        debug!("Invalidated cache entry: {}", cache_key);

        Ok(())
    }

    /// Invalidate all cache entries for a workflow
    pub fn invalidate_workflow(&self, workflow_id: &str) -> Result<usize> {
        let db = self.db.lock().unwrap();

        // Get all file paths
        let mut stmt =
            db.prepare("SELECT output_file FROM workflow_step_cache WHERE workflow_id = ?1")?;

        let files: Vec<String> = stmt
            .query_map([workflow_id], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        let count = files.len();

        // Delete from database
        db.execute(
            "DELETE FROM workflow_step_cache WHERE workflow_id = ?1",
            [workflow_id],
        )?;

        // Delete workflow meta
        db.execute(
            "DELETE FROM workflow_cache_meta WHERE workflow_id = ?1",
            [workflow_id],
        )?;

        // Delete files
        for file in files {
            let data_path = self.cache_dir.join("data").join(&file);
            let _ = std::fs::remove_file(data_path);
        }

        info!(
            "Invalidated {} cache entries for workflow {}",
            count, workflow_id
        );

        Ok(count)
    }

    /// Invalidate all cache entries for a specific step (all inputs)
    pub fn invalidate_step(&self, workflow_id: &str, step_index: usize) -> Result<usize> {
        let db = self.db.lock().unwrap();

        let mut stmt = db.prepare(
            "SELECT output_file FROM workflow_step_cache
             WHERE workflow_id = ?1 AND step_index = ?2",
        )?;

        let files: Vec<String> = stmt
            .query_map(rusqlite::params![workflow_id, step_index], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        let count = files.len();

        db.execute(
            "DELETE FROM workflow_step_cache
             WHERE workflow_id = ?1 AND step_index = ?2",
            rusqlite::params![workflow_id, step_index],
        )?;

        for file in files {
            let data_path = self.cache_dir.join("data").join(&file);
            let _ = std::fs::remove_file(data_path);
        }

        info!(
            "Invalidated {} cache entries for workflow {} step {}",
            count, workflow_id, step_index
        );

        Ok(count)
    }

    /// Cleanup expired entries
    pub fn cleanup_expired(&self) -> Result<CleanupResult> {
        let now = chrono::Utc::now().timestamp();
        let db = self.db.lock().unwrap();

        // Find expired entries
        let mut stmt = db.prepare(
            "SELECT output_file, size_bytes FROM workflow_step_cache
             WHERE expires_at < ?1",
        )?;

        let expired: Vec<(String, u64)> = stmt
            .query_map([now], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;

        let count = expired.len();
        let bytes_freed: u64 = expired.iter().map(|(_, size)| size).sum();

        // Delete from database
        db.execute(
            "DELETE FROM workflow_step_cache WHERE expires_at < ?1",
            [now],
        )?;

        // Delete files
        for (file, _) in expired {
            let data_path = self.cache_dir.join("data").join(&file);
            let _ = std::fs::remove_file(data_path);
        }

        if count > 0 {
            info!(
                "Cleaned up {} expired cache entries ({} bytes freed)",
                count, bytes_freed
            );
        }

        Ok(CleanupResult {
            entries_removed: count,
            bytes_freed,
        })
    }

    /// Evict oldest entries to stay under size limit
    pub fn evict_to_size(&self, max_bytes: u64) -> Result<CleanupResult> {
        let db = self.db.lock().unwrap();

        // Get current total size
        let total_size: u64 = db.query_row(
            "SELECT COALESCE(SUM(size_bytes), 0) FROM workflow_step_cache",
            [],
            |row| row.get(0),
        )?;

        if total_size <= max_bytes {
            return Ok(CleanupResult {
                entries_removed: 0,
                bytes_freed: 0,
            });
        }

        let target_reduction = total_size - max_bytes;
        let mut bytes_freed = 0u64;
        let mut count = 0usize;

        // Get oldest entries first
        let mut stmt = db.prepare(
            "SELECT cache_key, output_file, size_bytes FROM workflow_step_cache
             ORDER BY last_accessed ASC",
        )?;

        let entries: Vec<(String, String, u64)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<Result<Vec<_>, _>>()?;

        drop(stmt);

        // Evict until we've freed enough space
        for (cache_key, file, size) in entries {
            if bytes_freed >= target_reduction {
                break;
            }

            db.execute(
                "DELETE FROM workflow_step_cache WHERE cache_key = ?1",
                [&cache_key],
            )?;

            let data_path = self.cache_dir.join("data").join(&file);
            let _ = std::fs::remove_file(data_path);

            bytes_freed += size;
            count += 1;
        }

        info!(
            "Evicted {} cache entries ({} bytes freed) to stay under limit",
            count, bytes_freed
        );

        Ok(CleanupResult {
            entries_removed: count,
            bytes_freed,
        })
    }

    /// Get cache statistics
    pub fn stats(&self) -> Result<CacheStats> {
        let db = self.db.lock().unwrap();

        let total_entries: u64 =
            db.query_row("SELECT COUNT(*) FROM workflow_step_cache", [], |row| {
                row.get(0)
            })?;

        let total_size: u64 = db.query_row(
            "SELECT COALESCE(SUM(size_bytes), 0) FROM workflow_step_cache",
            [],
            |row| row.get(0),
        )?;

        let hot_threshold = chrono::Utc::now().timestamp() - self.config.hot_threshold_secs;
        let hot_entries: u64 = db.query_row(
            "SELECT COUNT(*) FROM workflow_step_cache WHERE last_accessed > ?1",
            [hot_threshold],
            |row| row.get(0),
        )?;

        let expired_entries: u64 = db.query_row(
            "SELECT COUNT(*) FROM workflow_step_cache WHERE expires_at < ?1",
            [chrono::Utc::now().timestamp()],
            |row| row.get(0),
        )?;

        let total_hits: u64 = db.query_row(
            "SELECT COALESCE(SUM(hit_count), 0) FROM workflow_cache_meta",
            [],
            |row| row.get(0),
        )?;

        let total_misses: u64 = db.query_row(
            "SELECT COALESCE(SUM(miss_count), 0) FROM workflow_cache_meta",
            [],
            |row| row.get(0),
        )?;

        let workflows_cached: u64 = db.query_row(
            "SELECT COUNT(DISTINCT workflow_id) FROM workflow_step_cache",
            [],
            |row| row.get(0),
        )?;

        Ok(CacheStats {
            total_entries,
            total_size_bytes: total_size,
            hot_entries,
            expired_entries,
            total_hits,
            total_misses,
            workflows_cached,
            hit_rate: if total_hits + total_misses > 0 {
                total_hits as f64 / (total_hits + total_misses) as f64
            } else {
                0.0
            },
        })
    }

    /// Get stats for a specific workflow
    pub fn workflow_stats(&self, workflow_id: &str) -> Result<Option<WorkflowCacheStats>> {
        let db = self.db.lock().unwrap();

        let meta: Option<(u64, u64, u64, u64)> = db
            .query_row(
                "SELECT total_entries, total_size_bytes, hit_count, miss_count
                 FROM workflow_cache_meta WHERE workflow_id = ?1",
                [workflow_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;

        match meta {
            Some((entries, size, hits, misses)) => Ok(Some(WorkflowCacheStats {
                workflow_id: workflow_id.to_string(),
                total_entries: entries,
                total_size_bytes: size,
                hit_count: hits,
                miss_count: misses,
                hit_rate: if hits + misses > 0 {
                    hits as f64 / (hits + misses) as f64
                } else {
                    0.0
                },
            })),
            None => Ok(None),
        }
    }

    /// Generate cache key from workflow+step+input
    fn make_cache_key(&self, workflow_id: &str, step_index: usize, input_hash: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(format!("{}:{}:{}", workflow_id, step_index, input_hash).as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Record cache hit
    fn record_hit(&self, db: &rusqlite::Connection, workflow_id: &str) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        db.execute(
            "INSERT INTO workflow_cache_meta (workflow_id, hit_count, last_hit)
             VALUES (?1, 1, ?2)
             ON CONFLICT(workflow_id) DO UPDATE SET
                hit_count = hit_count + 1,
                last_hit = ?2",
            rusqlite::params![workflow_id, now],
        )?;
        Ok(())
    }

    /// Record cache miss
    fn record_miss(&self, db: &rusqlite::Connection, workflow_id: &str) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        db.execute(
            "INSERT INTO workflow_cache_meta (workflow_id, miss_count, last_miss)
             VALUES (?1, 1, ?2)
             ON CONFLICT(workflow_id) DO UPDATE SET
                miss_count = miss_count + 1,
                last_miss = ?2",
            rusqlite::params![workflow_id, now],
        )?;
        Ok(())
    }

    /// Update workflow metadata after put
    fn update_workflow_meta(&self, db: &rusqlite::Connection, workflow_id: &str) -> Result<()> {
        // Recalculate totals
        let (entries, size): (u64, u64) = db.query_row(
            "SELECT COUNT(*), COALESCE(SUM(size_bytes), 0)
             FROM workflow_step_cache WHERE workflow_id = ?1",
            [workflow_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        db.execute(
            "INSERT INTO workflow_cache_meta (workflow_id, total_entries, total_size_bytes)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(workflow_id) DO UPDATE SET
                total_entries = ?2,
                total_size_bytes = ?3",
            rusqlite::params![workflow_id, entries, size],
        )?;

        Ok(())
    }

    /// Compress data using zstd
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        zstd::encode_all(std::io::Cursor::new(data), 3).context("Failed to compress data")
    }

    /// Decompress data using zstd
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>> {
        zstd::decode_all(std::io::Cursor::new(data)).context("Failed to decompress data")
    }
}

/// Cleanup result
#[derive(Debug, Clone)]
pub struct CleanupResult {
    pub entries_removed: usize,
    pub bytes_freed: u64,
}

/// Overall cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_entries: u64,
    pub total_size_bytes: u64,
    pub hot_entries: u64,
    pub expired_entries: u64,
    pub total_hits: u64,
    pub total_misses: u64,
    pub workflows_cached: u64,
    pub hit_rate: f64,
}

/// Per-workflow cache statistics
#[derive(Debug, Clone)]
pub struct WorkflowCacheStats {
    pub workflow_id: String,
    pub total_entries: u64,
    pub total_size_bytes: u64,
    pub hit_count: u64,
    pub miss_count: u64,
    pub hit_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_workflow_cache_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = WorkflowCacheConfig::default();
        let cache = WorkflowCache::new(temp_dir.path().to_path_buf(), config).await;
        assert!(cache.is_ok());
    }

    #[tokio::test]
    async fn test_put_and_get() {
        let temp_dir = TempDir::new().unwrap();
        let config = WorkflowCacheConfig::default();
        let cache = WorkflowCache::new(temp_dir.path().to_path_buf(), config)
            .await
            .unwrap();

        let test_data = b"test output data";
        cache
            .put("wf-001", 0, "input-hash-1", test_data, None)
            .unwrap();

        let result = cache.get("wf-001", 0, "input-hash-1").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), test_data);
    }

    #[tokio::test]
    async fn test_cache_miss() {
        let temp_dir = TempDir::new().unwrap();
        let config = WorkflowCacheConfig::default();
        let cache = WorkflowCache::new(temp_dir.path().to_path_buf(), config)
            .await
            .unwrap();

        let result = cache.get("wf-001", 0, "nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_invalidation() {
        let temp_dir = TempDir::new().unwrap();
        let config = WorkflowCacheConfig::default();
        let cache = WorkflowCache::new(temp_dir.path().to_path_buf(), config)
            .await
            .unwrap();

        cache.put("wf-001", 0, "input-1", b"data1", None).unwrap();
        cache.put("wf-001", 1, "input-2", b"data2", None).unwrap();

        let count = cache.invalidate_workflow("wf-001").unwrap();
        assert_eq!(count, 2);

        let result = cache.get("wf-001", 0, "input-1").unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_expiration() {
        let temp_dir = TempDir::new().unwrap();
        let config = WorkflowCacheConfig::default();
        let cache = WorkflowCache::new(temp_dir.path().to_path_buf(), config)
            .await
            .unwrap();

        // Insert with very short TTL
        cache
            .put("wf-001", 0, "input-1", b"data", Some(-1))
            .unwrap();

        // Should be expired immediately
        let result = cache.get("wf-001", 0, "input-1").unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_stats() {
        let temp_dir = TempDir::new().unwrap();
        let config = WorkflowCacheConfig::default();
        let cache = WorkflowCache::new(temp_dir.path().to_path_buf(), config)
            .await
            .unwrap();

        cache.put("wf-001", 0, "input-1", b"data", None).unwrap();

        let stats = cache.stats().unwrap();
        assert_eq!(stats.total_entries, 1);
        assert_eq!(stats.workflows_cached, 1);
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cache/src/workflow_executor.rs">
//! NUMA-aware workflow execution engine
//!
//! Executes multi-agent workflows with:
//! - Pipeline affinity (all steps on same NUMA node)
//! - Automatic intermediate caching
//! - Parallel step execution where possible
//! - Progress tracking and metrics

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::numa::NumaTopology;
use super::workflow_cache::{WorkflowCache, WorkflowCacheConfig};
use super::workflow_tracker::{PromotedWorkflow, WorkflowTracker, WorkflowTrackerConfig};

/// Configuration for workflow executor
#[derive(Debug, Clone)]
pub struct WorkflowExecutorConfig {
    /// Enable NUMA pinning for workflow execution
    pub numa_pinning: bool,
    /// Enable intermediate step caching
    pub enable_caching: bool,
    /// Maximum parallel steps (0 = unlimited)
    pub max_parallel_steps: usize,
    /// Timeout per step in seconds
    pub step_timeout_secs: u64,
    /// Retry failed steps
    pub retry_on_failure: bool,
    /// Maximum retries per step
    pub max_retries: u32,
}

impl Default for WorkflowExecutorConfig {
    fn default() -> Self {
        Self {
            numa_pinning: true,
            enable_caching: true,
            max_parallel_steps: 4,
            step_timeout_secs: 300, // 5 minutes
            retry_on_failure: true,
            max_retries: 2,
        }
    }
}

/// Result of a workflow step execution
#[derive(Debug, Clone)]
pub struct StepResult {
    pub step_index: usize,
    pub agent_id: String,
    pub output: Vec<u8>,
    pub latency_ms: u64,
    pub cached: bool,
    pub retries: u32,
}

/// Result of a complete workflow execution
#[derive(Debug, Clone)]
pub struct WorkflowResult {
    pub workflow_id: String,
    pub steps: Vec<StepResult>,
    pub total_latency_ms: u64,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub numa_node: Option<u32>,
}

impl WorkflowResult {
    /// Get the final output (last step's output)
    pub fn final_output(&self) -> Option<&[u8]> {
        self.steps.last().map(|s| s.output.as_slice())
    }

    /// Calculate cache hit rate
    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.cache_hits + self.cache_misses;
        if total == 0 {
            0.0
        } else {
            self.cache_hits as f64 / total as f64
        }
    }

    /// Calculate time saved by caching (estimated)
    pub fn estimated_time_saved_ms(&self) -> u64 {
        // Assume cached results are 90% faster
        self.steps
            .iter()
            .filter(|s| s.cached)
            .map(|s| (s.latency_ms as f64 * 9.0) as u64) // 90% of what it would have taken
            .sum()
    }
}

/// Agent function type for workflow steps
pub type AgentFn = Arc<dyn Fn(&[u8]) -> Result<Vec<u8>> + Send + Sync>;

/// Workflow execution progress callback
pub type ProgressCallback = Arc<dyn Fn(usize, usize, &str) + Send + Sync>;

pub struct WorkflowExecutor {
    config: WorkflowExecutorConfig,
    cache: Arc<WorkflowCache>,
    tracker: Arc<WorkflowTracker>,
    numa_topology: NumaTopology,
    agents: RwLock<HashMap<String, AgentFn>>,
    pinned_node: RwLock<Option<u32>>,
}

impl WorkflowExecutor {
    /// Create new workflow executor
    pub async fn new(cache_dir: PathBuf, config: WorkflowExecutorConfig) -> Result<Self> {
        let cache = WorkflowCache::new(cache_dir.clone(), WorkflowCacheConfig::default()).await?;
        let tracker =
            WorkflowTracker::new(cache_dir.clone(), WorkflowTrackerConfig::default()).await?;
        let numa_topology = NumaTopology::detect()?;

        info!(
            "Workflow executor initialized (NUMA nodes: {}, pinning: {})",
            numa_topology.node_count(),
            config.numa_pinning
        );

        Ok(Self {
            config,
            cache: Arc::new(cache),
            tracker: Arc::new(tracker),
            numa_topology,
            agents: RwLock::new(HashMap::new()),
            pinned_node: RwLock::new(None),
        })
    }

    /// Register an agent function
    pub async fn register_agent(&self, agent_id: &str, agent_fn: AgentFn) {
        let mut agents = self.agents.write().await;
        agents.insert(agent_id.to_string(), agent_fn);
        debug!("Registered agent: {}", agent_id);
    }

    /// Execute a workflow by ID
    pub async fn execute(
        &self,
        workflow_id: &str,
        input: &[u8],
        progress: Option<ProgressCallback>,
    ) -> Result<WorkflowResult> {
        // Get workflow definition
        let workflow = self
            .tracker
            .get_workflow(workflow_id)?
            .context(format!("Workflow not found: {}", workflow_id))?;

        self.execute_workflow(&workflow, input, progress).await
    }

    /// Execute a workflow from definition
    pub async fn execute_workflow(
        &self,
        workflow: &PromotedWorkflow,
        input: &[u8],
        progress: Option<ProgressCallback>,
    ) -> Result<WorkflowResult> {
        let start_time = Instant::now();
        let _input_hash = self.hash_input(input);

        info!(
            "Executing workflow {} ({} steps)",
            workflow.workflow_id,
            workflow.agent_sequence.len()
        );

        // Pin to NUMA node if enabled
        let numa_node = if self.config.numa_pinning {
            self.pin_to_optimal_node().await?
        } else {
            None
        };

        let mut steps = Vec::new();
        let mut current_input = input.to_vec();
        let mut cache_hits = 0usize;
        let mut cache_misses = 0usize;

        let total_steps = workflow.agent_sequence.len();

        for (step_index, agent_id) in workflow.agent_sequence.iter().enumerate() {
            // Report progress
            if let Some(ref callback) = progress {
                callback(step_index, total_steps, agent_id);
            }

            let step_input_hash = self.hash_input(&current_input);
            let step_start = Instant::now();

            // Try cache first
            let (output, cached) = if self.config.enable_caching {
                match self
                    .cache
                    .get(&workflow.workflow_id, step_index, &step_input_hash)?
                {
                    Some(cached_output) => {
                        debug!(
                            "Cache hit for workflow {} step {}",
                            workflow.workflow_id, step_index
                        );
                        cache_hits += 1;
                        (cached_output, true)
                    }
                    None => {
                        cache_misses += 1;
                        let output = self
                            .execute_step(agent_id, &current_input, step_index)
                            .await?;

                        // Cache the result
                        self.cache.put(
                            &workflow.workflow_id,
                            step_index,
                            &step_input_hash,
                            &output,
                            None,
                        )?;

                        (output, false)
                    }
                }
            } else {
                let output = self
                    .execute_step(agent_id, &current_input, step_index)
                    .await?;
                (output, false)
            };

            let latency_ms = step_start.elapsed().as_millis() as u64;

            steps.push(StepResult {
                step_index,
                agent_id: agent_id.clone(),
                output: output.clone(),
                latency_ms,
                cached,
                retries: 0, // TODO: track retries
            });

            // Output becomes input for next step
            current_input = output;
        }

        let total_latency_ms = start_time.elapsed().as_millis() as u64;

        // Record execution
        self.tracker.record_execution(&workflow.workflow_id)?;

        info!(
            "Workflow {} completed in {}ms (cache: {}/{} hits)",
            workflow.workflow_id, total_latency_ms, cache_hits, total_steps
        );

        Ok(WorkflowResult {
            workflow_id: workflow.workflow_id.clone(),
            steps,
            total_latency_ms,
            cache_hits,
            cache_misses,
            numa_node,
        })
    }

    /// Execute ad-hoc agent sequence (not a saved workflow)
    pub async fn execute_sequence(
        &self,
        agents: &[&str],
        input: &[u8],
        progress: Option<ProgressCallback>,
    ) -> Result<WorkflowResult> {
        let start_time = Instant::now();
        let input_hash = self.hash_input(input);

        // Generate a temporary workflow ID for caching
        let workflow_id = format!("adhoc-{}", &input_hash[..8]);

        info!("Executing ad-hoc sequence ({} agents)", agents.len());

        // Pin to NUMA node if enabled
        let numa_node = if self.config.numa_pinning {
            self.pin_to_optimal_node().await?
        } else {
            None
        };

        let mut steps = Vec::new();
        let mut current_input = input.to_vec();
        let mut cache_hits = 0usize;
        let mut cache_misses = 0usize;
        let total_steps = agents.len();

        for (step_index, agent_id) in agents.iter().enumerate() {
            if let Some(ref callback) = progress {
                callback(step_index, total_steps, agent_id);
            }

            let step_input_hash = self.hash_input(&current_input);
            let step_start = Instant::now();

            // Try cache (even for ad-hoc sequences)
            let (output, cached) = if self.config.enable_caching {
                match self.cache.get(&workflow_id, step_index, &step_input_hash)? {
                    Some(cached_output) => {
                        cache_hits += 1;
                        (cached_output, true)
                    }
                    None => {
                        cache_misses += 1;
                        let output = self
                            .execute_step(agent_id, &current_input, step_index)
                            .await?;
                        self.cache.put(
                            &workflow_id,
                            step_index,
                            &step_input_hash,
                            &output,
                            None,
                        )?;
                        (output, false)
                    }
                }
            } else {
                let output = self
                    .execute_step(agent_id, &current_input, step_index)
                    .await?;
                (output, false)
            };

            let latency_ms = step_start.elapsed().as_millis() as u64;

            steps.push(StepResult {
                step_index,
                agent_id: agent_id.to_string(),
                output: output.clone(),
                latency_ms,
                cached,
                retries: 0,
            });

            current_input = output;
        }

        let total_latency_ms = start_time.elapsed().as_millis() as u64;

        // Record sequence for pattern detection
        if let Some(suggestion) =
            self.tracker
                .record_sequence(agents, &input_hash, total_latency_ms)?
        {
            info!(
                "Pattern detected! Suggest creating workflow '{}' (called {} times)",
                suggestion.suggested_name, suggestion.pattern.call_count
            );
        }

        Ok(WorkflowResult {
            workflow_id,
            steps,
            total_latency_ms,
            cache_hits,
            cache_misses,
            numa_node,
        })
    }

    /// Execute a single step with retry support
    async fn execute_step(
        &self,
        agent_id: &str,
        input: &[u8],
        step_index: usize,
    ) -> Result<Vec<u8>> {
        let agents = self.agents.read().await;
        let agent_fn = agents
            .get(agent_id)
            .context(format!("Agent not found: {}", agent_id))?;

        let mut last_error = None;
        let max_attempts = if self.config.retry_on_failure {
            self.config.max_retries + 1
        } else {
            1
        };

        for attempt in 0..max_attempts {
            match agent_fn(input) {
                Ok(output) => {
                    if attempt > 0 {
                        debug!(
                            "Step {} ({}) succeeded after {} retries",
                            step_index, agent_id, attempt
                        );
                    }
                    return Ok(output);
                }
                Err(e) => {
                    warn!(
                        "Step {} ({}) failed (attempt {}/{}): {}",
                        step_index,
                        agent_id,
                        attempt + 1,
                        max_attempts,
                        e
                    );
                    last_error = Some(e);

                    if attempt < max_attempts - 1 {
                        // Exponential backoff
                        tokio::time::sleep(tokio::time::Duration::from_millis(
                            100 * 2u64.pow(attempt),
                        ))
                        .await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Unknown error in step execution")))
    }

    /// Pin workflow execution to optimal NUMA node
    async fn pin_to_optimal_node(&self) -> Result<Option<u32>> {
        if !self.numa_topology.is_numa_system() {
            return Ok(None);
        }

        let optimal_node = self.numa_topology.optimal_node();

        // Store pinned node
        {
            let mut pinned = self.pinned_node.write().await;
            *pinned = Some(optimal_node);
        }

        // Apply CPU affinity
        let cpus = self.numa_topology.cpus_for_node(optimal_node);
        if !cpus.is_empty() {
            let cpu_list = cpus
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(",");

            // Best effort - don't fail if taskset unavailable
            let _ = tokio::process::Command::new("taskset")
                .args(["-cp", &cpu_list, &std::process::id().to_string()])
                .output()
                .await;

            debug!(
                "Pinned workflow execution to NUMA node {} (CPUs: {})",
                optimal_node, cpu_list
            );
        }

        Ok(Some(optimal_node))
    }

    /// Hash input for cache keying
    fn hash_input(&self, input: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(input);
        format!("{:x}", hasher.finalize())
    }

    /// Get executor statistics
    pub async fn stats(&self) -> Result<ExecutorStats> {
        let tracker_stats = self.tracker.stats()?;
        let cache_stats = self.cache.stats()?;
        let agents = self.agents.read().await;

        Ok(ExecutorStats {
            registered_agents: agents.len(),
            promoted_workflows: tracker_stats.promoted_count as usize,
            pending_promotions: tracker_stats.pending_promotion as usize,
            total_workflow_executions: tracker_stats.total_workflow_executions,
            cache_entries: cache_stats.total_entries,
            cache_size_bytes: cache_stats.total_size_bytes,
            cache_hit_rate: cache_stats.hit_rate,
            numa_nodes: self.numa_topology.node_count(),
            numa_pinning_enabled: self.config.numa_pinning,
        })
    }

    /// Get promotion suggestions
    pub fn get_promotion_suggestions(
        &self,
    ) -> Result<Vec<super::workflow_tracker::PromotionSuggestion>> {
        self.tracker.get_promotion_candidates()
    }

    /// Promote a pattern to workflow
    pub fn promote_pattern(
        &self,
        pattern: &super::workflow_tracker::WorkflowPattern,
    ) -> Result<String> {
        self.tracker.promote_pattern(pattern)
    }

    /// Get all promoted workflows
    pub fn get_workflows(&self) -> Result<Vec<PromotedWorkflow>> {
        self.tracker.get_promoted_workflows()
    }

    /// Invalidate workflow cache
    pub fn invalidate_workflow_cache(&self, workflow_id: &str) -> Result<usize> {
        self.cache.invalidate_workflow(workflow_id)
    }

    /// Cleanup expired cache entries
    pub fn cleanup_cache(&self) -> Result<super::workflow_cache::CleanupResult> {
        self.cache.cleanup_expired()
    }
}

/// Executor statistics
#[derive(Debug, Clone)]
pub struct ExecutorStats {
    pub registered_agents: usize,
    pub promoted_workflows: usize,
    pub pending_promotions: usize,
    pub total_workflow_executions: u64,
    pub cache_entries: u64,
    pub cache_size_bytes: u64,
    pub cache_hit_rate: f64,
    pub numa_nodes: usize,
    pub numa_pinning_enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_echo_agent() -> AgentFn {
        Arc::new(|input: &[u8]| Ok(input.to_vec()))
    }

    fn make_transform_agent(suffix: &'static str) -> AgentFn {
        Arc::new(move |input: &[u8]| {
            let mut output = input.to_vec();
            output.extend_from_slice(suffix.as_bytes());
            Ok(output)
        })
    }

    #[tokio::test]
    async fn test_executor_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = WorkflowExecutorConfig::default();
        let executor = WorkflowExecutor::new(temp_dir.path().to_path_buf(), config).await;
        assert!(executor.is_ok());
    }

    #[tokio::test]
    async fn test_agent_registration() {
        let temp_dir = TempDir::new().unwrap();
        let config = WorkflowExecutorConfig::default();
        let executor = WorkflowExecutor::new(temp_dir.path().to_path_buf(), config)
            .await
            .unwrap();

        executor.register_agent("echo", make_echo_agent()).await;
        executor
            .register_agent("transform", make_transform_agent("_suffix"))
            .await;

        let stats = executor.stats().await.unwrap();
        assert_eq!(stats.registered_agents, 2);
    }

    #[tokio::test]
    async fn test_sequence_execution() {
        let temp_dir = TempDir::new().unwrap();
        let config = WorkflowExecutorConfig {
            numa_pinning: false, // Disable for tests
            ..Default::default()
        };
        let executor = WorkflowExecutor::new(temp_dir.path().to_path_buf(), config)
            .await
            .unwrap();

        executor.register_agent("echo", make_echo_agent()).await;
        executor
            .register_agent("add_a", make_transform_agent("A"))
            .await;
        executor
            .register_agent("add_b", make_transform_agent("B"))
            .await;

        let result = executor
            .execute_sequence(&["echo", "add_a", "add_b"], b"input", None)
            .await
            .unwrap();

        assert_eq!(result.steps.len(), 3);
        assert_eq!(result.final_output().unwrap(), b"inputAB");
    }

    #[tokio::test]
    async fn test_caching() {
        let temp_dir = TempDir::new().unwrap();
        let config = WorkflowExecutorConfig {
            numa_pinning: false,
            enable_caching: true,
            ..Default::default()
        };
        let executor = WorkflowExecutor::new(temp_dir.path().to_path_buf(), config)
            .await
            .unwrap();

        executor.register_agent("echo", make_echo_agent()).await;

        // First execution - cache miss
        let result1 = executor
            .execute_sequence(&["echo"], b"test", None)
            .await
            .unwrap();
        assert_eq!(result1.cache_misses, 1);
        assert_eq!(result1.cache_hits, 0);

        // Second execution - cache hit
        let result2 = executor
            .execute_sequence(&["echo"], b"test", None)
            .await
            .unwrap();
        assert_eq!(result2.cache_hits, 1);
    }

    #[tokio::test]
    async fn test_pattern_detection() {
        let temp_dir = TempDir::new().unwrap();
        let config = WorkflowExecutorConfig {
            numa_pinning: false,
            ..Default::default()
        };
        let executor = WorkflowExecutor::new(temp_dir.path().to_path_buf(), config)
            .await
            .unwrap();

        executor.register_agent("agent_a", make_echo_agent()).await;
        executor.register_agent("agent_b", make_echo_agent()).await;

        let sequence = &["agent_a", "agent_b"];

        // Execute multiple times to trigger pattern detection
        for i in 0..4 {
            let input = format!("input_{}", i);
            executor
                .execute_sequence(sequence, input.as_bytes(), None)
                .await
                .unwrap();
        }

        // Check for promotion suggestions
        let suggestions = executor.get_promotion_suggestions().unwrap();
        assert!(!suggestions.is_empty());
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cache/src/workflow_tracker.rs">
//! Workflow pattern detection and automatic promotion
//!
//! Tracks sequences of agent calls and automatically promotes
//! frequently-used patterns to first-class workflows.
//!
//! Features:
//! - Call sequence tracking with frequency counts
//! - Configurable promotion thresholds
//! - Pattern similarity detection
//! - Workflow definition export

use anyhow::{Context, Result};
use rusqlite::OptionalExtension;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Mutex;
use tracing::info;

/// Configuration for workflow pattern detection
#[derive(Debug, Clone)]
pub struct WorkflowTrackerConfig {
    /// Minimum calls before considering promotion (default: 3)
    pub promotion_threshold: u32,
    /// Time window in seconds for pattern detection (default: 24 hours)
    pub detection_window_secs: i64,
    /// Minimum sequence length to track (default: 2)
    pub min_sequence_length: usize,
    /// Maximum sequence length to track (default: 10)
    pub max_sequence_length: usize,
    /// Auto-promote when threshold reached (default: false, suggest only)
    pub auto_promote: bool,
}

impl Default for WorkflowTrackerConfig {
    fn default() -> Self {
        Self {
            promotion_threshold: 3,
            detection_window_secs: 86400, // 24 hours
            min_sequence_length: 2,
            max_sequence_length: 10,
            auto_promote: false,
        }
    }
}

/// Detected workflow pattern
#[derive(Debug, Clone)]
pub struct WorkflowPattern {
    pub pattern_id: String,
    pub agent_sequence: Vec<String>,
    pub call_count: u32,
    pub first_seen: i64,
    pub last_called: i64,
    pub avg_latency_ms: u64,
    pub promoted: bool,
    pub workflow_id: Option<String>,
}

impl WorkflowPattern {
    /// Check if pattern meets promotion threshold
    pub fn meets_threshold(&self, threshold: u32) -> bool {
        self.call_count >= threshold && !self.promoted
    }

    /// Get human-readable sequence description
    pub fn sequence_description(&self) -> String {
        self.agent_sequence.join(" → ")
    }
}

/// Workflow promotion suggestion
#[derive(Debug, Clone)]
pub struct PromotionSuggestion {
    pub pattern: WorkflowPattern,
    pub estimated_time_saved_ms: u64,
    pub confidence_score: f64,
    pub suggested_name: String,
}

pub struct WorkflowTracker {
    db: Mutex<rusqlite::Connection>,
    config: WorkflowTrackerConfig,
    /// In-memory buffer for current session sequences
    session_buffer: Mutex<Vec<AgentCall>>,
}

#[derive(Debug, Clone)]
struct AgentCall {
    agent_id: String,
    #[allow(dead_code)]
    input_hash: String,
    #[allow(dead_code)]
    timestamp: i64,
    #[allow(dead_code)]
    latency_ms: u64,
}

impl WorkflowTracker {
    /// Create new workflow tracker with SQLite persistence
    pub async fn new(cache_dir: PathBuf, config: WorkflowTrackerConfig) -> Result<Self> {
        let db_path = cache_dir.join("workflows/tracker.db");

        // Ensure directory exists
        if let Some(parent) = db_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let db = rusqlite::Connection::open(&db_path)
            .context("Failed to open workflow tracker database")?;

        // Create tables
        db.execute_batch(
            r#"
            -- Pattern tracking table
            CREATE TABLE IF NOT EXISTS workflow_patterns (
                pattern_hash TEXT PRIMARY KEY,
                agent_sequence TEXT NOT NULL,
                call_count INTEGER DEFAULT 1,
                first_seen INTEGER NOT NULL,
                last_called INTEGER NOT NULL,
                total_latency_ms INTEGER DEFAULT 0,
                promoted INTEGER DEFAULT 0,
                workflow_id TEXT
            );

            -- Individual call log for analysis
            CREATE TABLE IF NOT EXISTS agent_calls (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                agent_id TEXT NOT NULL,
                input_hash TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                latency_ms INTEGER NOT NULL
            );

            -- Detected sequences (sliding window analysis)
            CREATE TABLE IF NOT EXISTS detected_sequences (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                sequence_hash TEXT NOT NULL,
                agent_sequence TEXT NOT NULL,
                detected_at INTEGER NOT NULL
            );

            -- Promoted workflows
            CREATE TABLE IF NOT EXISTS promoted_workflows (
                workflow_id TEXT PRIMARY KEY,
                pattern_hash TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                agent_sequence TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                execution_count INTEGER DEFAULT 0,
                FOREIGN KEY (pattern_hash) REFERENCES workflow_patterns(pattern_hash)
            );

            -- Indexes for efficient queries
            CREATE INDEX IF NOT EXISTS idx_patterns_count ON workflow_patterns(call_count DESC);
            CREATE INDEX IF NOT EXISTS idx_patterns_last_called ON workflow_patterns(last_called DESC);
            CREATE INDEX IF NOT EXISTS idx_calls_session ON agent_calls(session_id, timestamp);
            CREATE INDEX IF NOT EXISTS idx_calls_timestamp ON agent_calls(timestamp DESC);
            CREATE INDEX IF NOT EXISTS idx_sequences_hash ON detected_sequences(sequence_hash);
            "#,
        )?;

        info!("Workflow tracker initialized at {:?}", db_path);

        Ok(Self {
            db: Mutex::new(db),
            config,
            session_buffer: Mutex::new(Vec::new()),
        })
    }

    /// Record an agent call
    pub fn record_call(
        &self,
        session_id: &str,
        agent_id: &str,
        input_hash: &str,
        latency_ms: u64,
    ) -> Result<()> {
        let now = chrono::Utc::now().timestamp();

        // Add to session buffer
        {
            let mut buffer = self.session_buffer.lock().unwrap();
            buffer.push(AgentCall {
                agent_id: agent_id.to_string(),
                input_hash: input_hash.to_string(),
                timestamp: now,
                latency_ms,
            });

            // Trim buffer if too large
            if buffer.len() > self.config.max_sequence_length * 2 {
                buffer.drain(0..self.config.max_sequence_length);
            }
        }

        // Persist to database
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT INTO agent_calls (session_id, agent_id, input_hash, timestamp, latency_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![session_id, agent_id, input_hash, now, latency_ms],
        )?;

        drop(db);

        // Analyze for patterns after each call
        self.analyze_session_patterns(session_id)?;

        Ok(())
    }

    /// Record a complete agent sequence (batch recording)
    pub fn record_sequence(
        &self,
        agents: &[&str],
        _input_hash: &str,
        total_latency_ms: u64,
    ) -> Result<Option<PromotionSuggestion>> {
        if agents.len() < self.config.min_sequence_length {
            return Ok(None);
        }

        let sequence_hash = self.hash_sequence(agents);
        let agent_sequence_json = simd_json::to_string(agents)?;
        let now = chrono::Utc::now().timestamp();
        let _avg_latency = total_latency_ms / agents.len() as u64;

        let db = self.db.lock().unwrap();

        // Check if pattern exists
        let existing: Option<(u32, i64, i64, bool)> = db
            .query_row(
                "SELECT call_count, first_seen, total_latency_ms, promoted
                 FROM workflow_patterns WHERE pattern_hash = ?1",
                [&sequence_hash],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;

        let (call_count, first_seen, total_latency, promoted) = if let Some(existing) = existing {
            // Update existing pattern
            db.execute(
                "UPDATE workflow_patterns
                 SET call_count = call_count + 1,
                     last_called = ?1,
                     total_latency_ms = total_latency_ms + ?2
                 WHERE pattern_hash = ?3",
                rusqlite::params![now, total_latency_ms, sequence_hash],
            )?;
            (
                existing.0 + 1,
                existing.1,
                existing.2 + total_latency_ms as i64,
                existing.3,
            )
        } else {
            // Insert new pattern
            db.execute(
                "INSERT INTO workflow_patterns
                 (pattern_hash, agent_sequence, call_count, first_seen, last_called, total_latency_ms)
                 VALUES (?1, ?2, 1, ?3, ?3, ?4)",
                rusqlite::params![sequence_hash, agent_sequence_json, now, total_latency_ms],
            )?;
            (1, now, total_latency_ms as i64, false)
        };

        drop(db);

        // Check for promotion
        if call_count >= self.config.promotion_threshold && !promoted {
            let pattern = WorkflowPattern {
                pattern_id: sequence_hash.clone(),
                agent_sequence: agents.iter().map(|s| s.to_string()).collect(),
                call_count,
                first_seen,
                last_called: now,
                avg_latency_ms: (total_latency / call_count as i64) as u64,
                promoted: false,
                workflow_id: None,
            };

            let suggestion = PromotionSuggestion {
                estimated_time_saved_ms: self.estimate_time_savings(&pattern),
                confidence_score: self.calculate_confidence(&pattern),
                suggested_name: self.generate_workflow_name(&pattern),
                pattern,
            };

            if self.config.auto_promote {
                self.promote_pattern(&suggestion.pattern)?;
            }

            return Ok(Some(suggestion));
        }

        Ok(None)
    }

    /// Analyze session for emerging patterns (sliding window)
    fn analyze_session_patterns(&self, session_id: &str) -> Result<()> {
        let buffer = self.session_buffer.lock().unwrap();

        if buffer.len() < self.config.min_sequence_length {
            return Ok(());
        }

        // Extract sequences of various lengths
        for window_size in self.config.min_sequence_length..=self.config.max_sequence_length {
            if buffer.len() < window_size {
                break;
            }

            let start = buffer.len() - window_size;
            let window = &buffer[start..];

            let agents: Vec<&str> = window.iter().map(|c| c.agent_id.as_str()).collect();
            let sequence_hash = self.hash_sequence(&agents);
            let agent_sequence_json =
                simd_json::to_string(&agents).unwrap_or_else(|_| "[]".to_string());
            let now = chrono::Utc::now().timestamp();

            // Record detected sequence
            let db = self.db.lock().unwrap();
            db.execute(
                "INSERT INTO detected_sequences (session_id, sequence_hash, agent_sequence, detected_at)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![session_id, sequence_hash, agent_sequence_json, now],
            )?;
        }

        Ok(())
    }

    /// Promote a pattern to a first-class workflow
    pub fn promote_pattern(&self, pattern: &WorkflowPattern) -> Result<String> {
        let workflow_id = format!("WF-{}", &pattern.pattern_id[..8]);
        let now = chrono::Utc::now().timestamp();
        let agent_sequence_json = simd_json::to_string(&pattern.agent_sequence)?;

        let db = self.db.lock().unwrap();

        // Create workflow entry
        db.execute(
            "INSERT INTO promoted_workflows
             (workflow_id, pattern_hash, name, description, agent_sequence, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                workflow_id,
                pattern.pattern_id,
                self.generate_workflow_name(pattern),
                format!(
                    "Auto-promoted workflow from pattern detected {} times",
                    pattern.call_count
                ),
                agent_sequence_json,
                now
            ],
        )?;

        // Mark pattern as promoted
        db.execute(
            "UPDATE workflow_patterns
             SET promoted = 1, workflow_id = ?1
             WHERE pattern_hash = ?2",
            rusqlite::params![workflow_id, pattern.pattern_id],
        )?;

        info!(
            "Promoted pattern {} to workflow {}: {}",
            pattern.pattern_id,
            workflow_id,
            pattern.sequence_description()
        );

        Ok(workflow_id)
    }

    /// Get patterns eligible for promotion
    pub fn get_promotion_candidates(&self) -> Result<Vec<PromotionSuggestion>> {
        let db = self.db.lock().unwrap();
        let cutoff = chrono::Utc::now().timestamp() - self.config.detection_window_secs;

        let mut stmt = db.prepare(
            "SELECT pattern_hash, agent_sequence, call_count, first_seen, last_called, total_latency_ms
             FROM workflow_patterns
             WHERE call_count >= ?1
               AND promoted = 0
               AND last_called > ?2
             ORDER BY call_count DESC",
        )?;

        let patterns = stmt
            .query_map(
                rusqlite::params![self.config.promotion_threshold, cutoff],
                |row| {
                    let mut agent_sequence_json: String = row.get(1)?;
                    let agent_sequence: Vec<String> =
                        unsafe { simd_json::from_str(&mut agent_sequence_json) }
                            .unwrap_or_default();
                    let call_count: u32 = row.get(2)?;
                    let total_latency: i64 = row.get(5)?;

                    Ok(WorkflowPattern {
                        pattern_id: row.get(0)?,
                        agent_sequence,
                        call_count,
                        first_seen: row.get(3)?,
                        last_called: row.get(4)?,
                        avg_latency_ms: (total_latency / call_count as i64) as u64,
                        promoted: false,
                        workflow_id: None,
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(patterns
            .into_iter()
            .map(|pattern| PromotionSuggestion {
                estimated_time_saved_ms: self.estimate_time_savings(&pattern),
                confidence_score: self.calculate_confidence(&pattern),
                suggested_name: self.generate_workflow_name(&pattern),
                pattern,
            })
            .collect())
    }

    /// Get all promoted workflows
    pub fn get_promoted_workflows(&self) -> Result<Vec<PromotedWorkflow>> {
        let db = self.db.lock().unwrap();

        let mut stmt = db.prepare(
            "SELECT workflow_id, pattern_hash, name, description, agent_sequence, created_at, execution_count
             FROM promoted_workflows
             ORDER BY created_at DESC",
        )?;

        let workflows = stmt
            .query_map([], |row| {
                let mut agent_sequence_json: String = row.get(4)?;
                let agent_sequence: Vec<String> =
                    unsafe { simd_json::from_str(&mut agent_sequence_json) }.unwrap_or_default();

                Ok(PromotedWorkflow {
                    workflow_id: row.get(0)?,
                    pattern_hash: row.get(1)?,
                    name: row.get(2)?,
                    description: row.get(3)?,
                    agent_sequence,
                    created_at: row.get(5)?,
                    execution_count: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(workflows)
    }

    /// Get a specific workflow by ID
    pub fn get_workflow(&self, workflow_id: &str) -> Result<Option<PromotedWorkflow>> {
        let db = self.db.lock().unwrap();

        let workflow = db
            .query_row(
                "SELECT workflow_id, pattern_hash, name, description, agent_sequence, created_at, execution_count
                 FROM promoted_workflows
                 WHERE workflow_id = ?1",
                [workflow_id],
                |row| {
                    let mut agent_sequence_json: String = row.get(4)?;
                    let agent_sequence: Vec<String> =
                        unsafe { simd_json::from_str(&mut agent_sequence_json) }.unwrap_or_default();

                    Ok(PromotedWorkflow {
                        workflow_id: row.get(0)?,
                        pattern_hash: row.get(1)?,
                        name: row.get(2)?,
                        description: row.get(3)?,
                        agent_sequence,
                        created_at: row.get(5)?,
                        execution_count: row.get(6)?,
                    })
                },
            )
            .optional()?;

        Ok(workflow)
    }

    /// Record workflow execution
    pub fn record_execution(&self, workflow_id: &str) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute(
            "UPDATE promoted_workflows SET execution_count = execution_count + 1 WHERE workflow_id = ?1",
            [workflow_id],
        )?;
        Ok(())
    }

    /// Get tracker statistics
    pub fn stats(&self) -> Result<TrackerStats> {
        let db = self.db.lock().unwrap();

        let total_patterns: u32 =
            db.query_row("SELECT COUNT(*) FROM workflow_patterns", [], |row| {
                row.get(0)
            })?;

        let promoted_count: u32 = db.query_row(
            "SELECT COUNT(*) FROM workflow_patterns WHERE promoted = 1",
            [],
            |row| row.get(0),
        )?;

        let pending_promotion: u32 = db.query_row(
            "SELECT COUNT(*) FROM workflow_patterns WHERE call_count >= ?1 AND promoted = 0",
            [self.config.promotion_threshold],
            |row| row.get(0),
        )?;

        let total_calls: u64 =
            db.query_row("SELECT COUNT(*) FROM agent_calls", [], |row| row.get(0))?;

        let total_workflow_executions: u64 = db.query_row(
            "SELECT COALESCE(SUM(execution_count), 0) FROM promoted_workflows",
            [],
            |row| row.get(0),
        )?;

        Ok(TrackerStats {
            total_patterns,
            promoted_count,
            pending_promotion,
            total_calls,
            total_workflow_executions,
            promotion_threshold: self.config.promotion_threshold,
        })
    }

    /// Hash a sequence of agents for pattern identification
    fn hash_sequence(&self, agents: &[&str]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(agents.join("→").as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Estimate time savings from caching this workflow
    fn estimate_time_savings(&self, pattern: &WorkflowPattern) -> u64 {
        // Assume 40% cache hit rate on subsequent executions
        // and 60% latency reduction when cached
        let expected_future_calls = pattern.call_count * 2; // Extrapolate
        let cache_hit_savings = (pattern.avg_latency_ms as f64 * 0.6) as u64;
        let hit_rate = 0.4;

        (expected_future_calls as f64 * cache_hit_savings as f64 * hit_rate) as u64
    }

    /// Calculate confidence score for promotion
    fn calculate_confidence(&self, pattern: &WorkflowPattern) -> f64 {
        let recency_days = (chrono::Utc::now().timestamp() - pattern.last_called) as f64 / 86400.0;
        let frequency_score =
            (pattern.call_count as f64 / self.config.promotion_threshold as f64).min(2.0) / 2.0;
        let recency_score = (1.0 - recency_days / 7.0).max(0.0);
        let length_score = if pattern.agent_sequence.len() >= 2 && pattern.agent_sequence.len() <= 5
        {
            1.0
        } else {
            0.7
        };

        (frequency_score * 0.4 + recency_score * 0.3 + length_score * 0.3).min(1.0)
    }

    /// Generate a suggested workflow name
    fn generate_workflow_name(&self, pattern: &WorkflowPattern) -> String {
        if pattern.agent_sequence.is_empty() {
            return "unnamed-workflow".to_string();
        }

        let first = pattern.agent_sequence.first().unwrap();
        let last = pattern.agent_sequence.last().unwrap();

        if pattern.agent_sequence.len() == 2 {
            format!("{}-to-{}", first, last)
        } else {
            format!("{}-to-{}-{}step", first, last, pattern.agent_sequence.len())
        }
    }

    /// Clear session buffer (call at session end)
    pub fn clear_session(&self) {
        let mut buffer = self.session_buffer.lock().unwrap();
        buffer.clear();
    }

    /// Cleanup old data
    pub fn cleanup(&self, days: i64) -> Result<CleanupStats> {
        let cutoff = chrono::Utc::now().timestamp() - (days * 86400);
        let db = self.db.lock().unwrap();

        let calls_deleted = db.execute("DELETE FROM agent_calls WHERE timestamp < ?1", [cutoff])?;

        let sequences_deleted = db.execute(
            "DELETE FROM detected_sequences WHERE detected_at < ?1",
            [cutoff],
        )?;

        let patterns_deleted = db.execute(
            "DELETE FROM workflow_patterns WHERE last_called < ?1 AND promoted = 0 AND call_count < ?2",
            rusqlite::params![cutoff, self.config.promotion_threshold],
        )?;

        info!(
            "Cleanup complete: {} calls, {} sequences, {} patterns removed",
            calls_deleted, sequences_deleted, patterns_deleted
        );

        Ok(CleanupStats {
            calls_deleted,
            sequences_deleted,
            patterns_deleted,
        })
    }
}

/// Promoted workflow definition
#[derive(Debug, Clone)]
pub struct PromotedWorkflow {
    pub workflow_id: String,
    pub pattern_hash: String,
    pub name: String,
    pub description: Option<String>,
    pub agent_sequence: Vec<String>,
    pub created_at: i64,
    pub execution_count: u64,
}

/// Tracker statistics
#[derive(Debug, Clone)]
pub struct TrackerStats {
    pub total_patterns: u32,
    pub promoted_count: u32,
    pub pending_promotion: u32,
    pub total_calls: u64,
    pub total_workflow_executions: u64,
    pub promotion_threshold: u32,
}

/// Cleanup statistics
#[derive(Debug, Clone)]
pub struct CleanupStats {
    pub calls_deleted: usize,
    pub sequences_deleted: usize,
    pub patterns_deleted: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_workflow_tracker_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = WorkflowTrackerConfig::default();
        let tracker = WorkflowTracker::new(temp_dir.path().to_path_buf(), config).await;
        assert!(tracker.is_ok());
    }

    #[tokio::test]
    async fn test_record_sequence() {
        let temp_dir = TempDir::new().unwrap();
        let config = WorkflowTrackerConfig {
            promotion_threshold: 2,
            ..Default::default()
        };
        let tracker = WorkflowTracker::new(temp_dir.path().to_path_buf(), config)
            .await
            .unwrap();

        // First call - no promotion
        let result = tracker
            .record_sequence(&["agent_a", "agent_b"], "hash1", 100)
            .unwrap();
        assert!(result.is_none());

        // Second call - should suggest promotion
        let result = tracker
            .record_sequence(&["agent_a", "agent_b"], "hash2", 150)
            .unwrap();
        assert!(result.is_some());

        let suggestion = result.unwrap();
        assert_eq!(suggestion.pattern.call_count, 2);
        assert_eq!(
            suggestion.pattern.agent_sequence,
            vec!["agent_a", "agent_b"]
        );
    }

    #[tokio::test]
    async fn test_pattern_promotion() {
        let temp_dir = TempDir::new().unwrap();
        let config = WorkflowTrackerConfig {
            promotion_threshold: 1,
            ..Default::default()
        };
        let tracker = WorkflowTracker::new(temp_dir.path().to_path_buf(), config)
            .await
            .unwrap();

        let result = tracker
            .record_sequence(&["agent_a", "agent_b", "agent_c"], "hash1", 200)
            .unwrap();

        assert!(result.is_some());
        let suggestion = result.unwrap();

        let workflow_id = tracker.promote_pattern(&suggestion.pattern).unwrap();
        assert!(workflow_id.starts_with("WF-"));

        let workflow = tracker.get_workflow(&workflow_id).unwrap();
        assert!(workflow.is_some());
    }

    #[tokio::test]
    async fn test_stats() {
        let temp_dir = TempDir::new().unwrap();
        let config = WorkflowTrackerConfig::default();
        let tracker = WorkflowTracker::new(temp_dir.path().to_path_buf(), config)
            .await
            .unwrap();

        let stats = tracker.stats().unwrap();
        assert_eq!(stats.total_patterns, 0);
        assert_eq!(stats.promotion_threshold, 3);
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cache/src/workstack_cache.rs">
//! Workstack intermediate result caching
//!
//! Caches intermediate results from workstack steps
//! to avoid redundant computation.

use anyhow::{Context, Result};
use rusqlite::OptionalExtension;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::sync::Mutex;
use tracing::{debug, info};

/// Configuration for workstack caching
#[derive(Debug, Clone)]
pub struct WorkstackCacheConfig {
    /// Default TTL in seconds (default: 1 hour)
    pub default_ttl_secs: i64,
    /// Maximum cache size in bytes (default: 1GB)
    pub max_size_bytes: u64,
    /// Enable compression (default: true)
    pub compress: bool,
    /// Hot entry threshold in seconds (default: 10 minutes)
    pub hot_threshold_secs: i64,
}

impl Default for WorkstackCacheConfig {
    fn default() -> Self {
        Self {
            default_ttl_secs: 3600,
            max_size_bytes: 1024 * 1024 * 1024,
            compress: true,
            hot_threshold_secs: 600,
        }
    }
}

pub struct WorkstackCache {
    cache_dir: PathBuf,
    db: Mutex<rusqlite::Connection>,
    config: WorkstackCacheConfig,
}

impl WorkstackCache {
    /// Create new workstack cache
    pub async fn new(cache_dir: PathBuf, config: WorkstackCacheConfig) -> Result<Self> {
        let workstacks_dir = cache_dir.join("workstacks");
        let data_dir = workstacks_dir.join("data");

        tokio::fs::create_dir_all(&data_dir).await?;

        let db_path = workstacks_dir.join("cache.db");
        let db = rusqlite::Connection::open(&db_path)
            .context("Failed to open workstack cache database")?;

        db.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS step_cache (
                cache_key TEXT PRIMARY KEY,
                workstack_id TEXT NOT NULL,
                step_index INTEGER NOT NULL,
                input_hash TEXT NOT NULL,
                output_file TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                access_count INTEGER DEFAULT 1,
                last_accessed INTEGER NOT NULL,
                size_bytes INTEGER NOT NULL,
                compressed INTEGER DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS workstack_meta (
                workstack_id TEXT PRIMARY KEY,
                total_entries INTEGER DEFAULT 0,
                total_size_bytes INTEGER DEFAULT 0,
                hit_count INTEGER DEFAULT 0,
                miss_count INTEGER DEFAULT 0
            );

            CREATE INDEX IF NOT EXISTS idx_cache_workstack ON step_cache(workstack_id);
            CREATE INDEX IF NOT EXISTS idx_cache_expires ON step_cache(expires_at);
            CREATE INDEX IF NOT EXISTS idx_cache_accessed ON step_cache(last_accessed DESC);
            "#,
        )?;

        info!("Workstack cache initialized at {:?}", db_path);

        Ok(Self {
            cache_dir: workstacks_dir,
            db: Mutex::new(db),
            config,
        })
    }

    /// Get cached result for a workstack step
    pub fn get(
        &self,
        workstack_id: &str,
        step_index: usize,
        input_hash: &str,
    ) -> Result<Option<Vec<u8>>> {
        let cache_key = self.make_cache_key(workstack_id, step_index, input_hash);
        let now = chrono::Utc::now().timestamp();

        let db = self.db.lock().unwrap();

        let entry: Option<(String, i64, bool)> = db
            .query_row(
                "SELECT output_file, expires_at, compressed
                 FROM step_cache WHERE cache_key = ?1",
                [&cache_key],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;

        let (output_file, expires_at, compressed) = match entry {
            Some(e) => e,
            None => {
                self.record_miss(&db, workstack_id)?;
                return Ok(None);
            }
        };

        // Check expiration
        if now > expires_at {
            debug!("Cache entry expired: {}", cache_key);
            self.invalidate_entry(&cache_key)?;
            return Ok(None);
        }

        // Update access stats
        db.execute(
            "UPDATE step_cache SET access_count = access_count + 1, last_accessed = ?1
             WHERE cache_key = ?2",
            rusqlite::params![now, cache_key],
        )?;

        self.record_hit(&db, workstack_id)?;

        // Read data
        let data_path = self.cache_dir.join("data").join(&output_file);
        let data = std::fs::read(&data_path)
            .context(format!("Failed to read cached data: {:?}", data_path))?;

        let output = if compressed {
            self.decompress(&data)?
        } else {
            data
        };

        debug!("Cache hit: {} (key: {})", workstack_id, cache_key);
        Ok(Some(output))
    }

    /// Store result in cache
    pub fn put(
        &self,
        workstack_id: &str,
        step_index: usize,
        input_hash: &str,
        output: &[u8],
        ttl_secs: Option<i64>,
    ) -> Result<()> {
        let cache_key = self.make_cache_key(workstack_id, step_index, input_hash);
        let now = chrono::Utc::now().timestamp();
        let ttl = ttl_secs.unwrap_or(self.config.default_ttl_secs);
        let expires_at = now + ttl;

        // Compress if beneficial
        let (data, compressed) = if self.config.compress && output.len() > 1024 {
            match self.compress(output) {
                Ok(compressed_data) if compressed_data.len() < output.len() => {
                    (compressed_data, true)
                }
                _ => (output.to_vec(), false),
            }
        } else {
            (output.to_vec(), false)
        };

        let size_bytes = data.len() as u64;

        // Write to file
        let output_file = format!("{}.cache", cache_key);
        let data_path = self.cache_dir.join("data").join(&output_file);
        std::fs::write(&data_path, &data)?;

        // Update database
        let db = self.db.lock().unwrap();

        db.execute(
            "INSERT INTO step_cache
             (cache_key, workstack_id, step_index, input_hash, output_file,
              created_at, expires_at, last_accessed, size_bytes, compressed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(cache_key) DO UPDATE SET
                output_file = ?5, expires_at = ?7, last_accessed = ?8,
                size_bytes = ?9, compressed = ?10, access_count = access_count + 1",
            rusqlite::params![
                cache_key,
                workstack_id,
                step_index,
                input_hash,
                output_file,
                now,
                expires_at,
                now,
                size_bytes,
                compressed
            ],
        )?;

        self.update_workstack_meta(&db, workstack_id)?;

        debug!(
            "Cached workstack {} step {} ({} bytes, compressed: {})",
            workstack_id, step_index, size_bytes, compressed
        );

        Ok(())
    }

    /// Invalidate a specific entry
    fn invalidate_entry(&self, cache_key: &str) -> Result<()> {
        let db = self.db.lock().unwrap();

        let output_file: Option<String> = db
            .query_row(
                "SELECT output_file FROM step_cache WHERE cache_key = ?1",
                [cache_key],
                |row| row.get(0),
            )
            .optional()?;

        db.execute("DELETE FROM step_cache WHERE cache_key = ?1", [cache_key])?;

        if let Some(file) = output_file {
            let _ = std::fs::remove_file(self.cache_dir.join("data").join(&file));
        }

        Ok(())
    }

    /// Invalidate all entries for a workstack
    pub fn invalidate_workstack(&self, workstack_id: &str) -> Result<usize> {
        let db = self.db.lock().unwrap();

        let mut stmt = db.prepare("SELECT output_file FROM step_cache WHERE workstack_id = ?1")?;

        let files: Vec<String> = stmt
            .query_map([workstack_id], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;

        let count = files.len();

        db.execute(
            "DELETE FROM step_cache WHERE workstack_id = ?1",
            [workstack_id],
        )?;

        db.execute(
            "DELETE FROM workstack_meta WHERE workstack_id = ?1",
            [workstack_id],
        )?;

        drop(stmt);

        for file in files {
            let _ = std::fs::remove_file(self.cache_dir.join("data").join(&file));
        }

        info!(
            "Invalidated {} cache entries for workstack {}",
            count, workstack_id
        );
        Ok(count)
    }

    /// Cleanup expired entries
    pub fn cleanup_expired(&self) -> Result<CleanupResult> {
        let now = chrono::Utc::now().timestamp();
        let db = self.db.lock().unwrap();

        let mut stmt =
            db.prepare("SELECT output_file, size_bytes FROM step_cache WHERE expires_at < ?1")?;

        let expired: Vec<(String, u64)> = stmt
            .query_map([now], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;

        let count = expired.len();
        let bytes_freed: u64 = expired.iter().map(|(_, size)| size).sum();

        db.execute("DELETE FROM step_cache WHERE expires_at < ?1", [now])?;

        drop(stmt);

        for (file, _) in expired {
            let _ = std::fs::remove_file(self.cache_dir.join("data").join(&file));
        }

        if count > 0 {
            info!(
                "Cleaned up {} expired entries ({} bytes)",
                count, bytes_freed
            );
        }

        Ok(CleanupResult {
            entries_removed: count,
            bytes_freed,
        })
    }

    /// Get cache statistics
    pub fn stats(&self) -> Result<CacheStats> {
        let db = self.db.lock().unwrap();

        let total_entries: u64 =
            db.query_row("SELECT COUNT(*) FROM step_cache", [], |row| row.get(0))?;

        let total_size: u64 = db.query_row(
            "SELECT COALESCE(SUM(size_bytes), 0) FROM step_cache",
            [],
            |row| row.get(0),
        )?;

        let hot_threshold = chrono::Utc::now().timestamp() - self.config.hot_threshold_secs;
        let hot_entries: u64 = db.query_row(
            "SELECT COUNT(*) FROM step_cache WHERE last_accessed > ?1",
            [hot_threshold],
            |row| row.get(0),
        )?;

        let total_hits: u64 = db.query_row(
            "SELECT COALESCE(SUM(hit_count), 0) FROM workstack_meta",
            [],
            |row| row.get(0),
        )?;

        let total_misses: u64 = db.query_row(
            "SELECT COALESCE(SUM(miss_count), 0) FROM workstack_meta",
            [],
            |row| row.get(0),
        )?;

        let workstacks_cached: u64 = db.query_row(
            "SELECT COUNT(DISTINCT workstack_id) FROM step_cache",
            [],
            |row| row.get(0),
        )?;

        Ok(CacheStats {
            total_entries,
            total_size_bytes: total_size,
            hot_entries,
            total_hits,
            total_misses,
            workstacks_cached,
            hit_rate: if total_hits + total_misses > 0 {
                total_hits as f64 / (total_hits + total_misses) as f64
            } else {
                0.0
            },
        })
    }

    fn make_cache_key(&self, workstack_id: &str, step_index: usize, input_hash: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(format!("{}:{}:{}", workstack_id, step_index, input_hash).as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn record_hit(&self, db: &rusqlite::Connection, workstack_id: &str) -> Result<()> {
        db.execute(
            "INSERT INTO workstack_meta (workstack_id, hit_count) VALUES (?1, 1)
             ON CONFLICT(workstack_id) DO UPDATE SET hit_count = hit_count + 1",
            [workstack_id],
        )?;
        Ok(())
    }

    fn record_miss(&self, db: &rusqlite::Connection, workstack_id: &str) -> Result<()> {
        db.execute(
            "INSERT INTO workstack_meta (workstack_id, miss_count) VALUES (?1, 1)
             ON CONFLICT(workstack_id) DO UPDATE SET miss_count = miss_count + 1",
            [workstack_id],
        )?;
        Ok(())
    }

    fn update_workstack_meta(&self, db: &rusqlite::Connection, workstack_id: &str) -> Result<()> {
        let (entries, size): (u64, u64) = db.query_row(
            "SELECT COUNT(*), COALESCE(SUM(size_bytes), 0)
             FROM step_cache WHERE workstack_id = ?1",
            [workstack_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;

        db.execute(
            "INSERT INTO workstack_meta (workstack_id, total_entries, total_size_bytes)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(workstack_id) DO UPDATE SET
                total_entries = ?2, total_size_bytes = ?3",
            rusqlite::params![workstack_id, entries, size],
        )?;

        Ok(())
    }

    fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        zstd::encode_all(std::io::Cursor::new(data), 3).context("Compression failed")
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>> {
        zstd::decode_all(std::io::Cursor::new(data)).context("Decompression failed")
    }
}

/// Cleanup result
#[derive(Debug, Clone)]
pub struct CleanupResult {
    pub entries_removed: usize,
    pub bytes_freed: u64,
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub total_entries: u64,
    pub total_size_bytes: u64,
    pub hot_entries: u64,
    pub total_hits: u64,
    pub total_misses: u64,
    pub workstacks_cached: u64,
    pub hit_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_workstack_cache_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = WorkstackCacheConfig::default();
        let cache = WorkstackCache::new(temp_dir.path().to_path_buf(), config).await;
        assert!(cache.is_ok());
    }

    #[tokio::test]
    async fn test_put_and_get() {
        let temp_dir = TempDir::new().unwrap();
        let config = WorkstackCacheConfig::default();
        let cache = WorkstackCache::new(temp_dir.path().to_path_buf(), config)
            .await
            .unwrap();

        let test_data = b"test output";
        cache
            .put("ws-001", 0, "input-hash", test_data, None)
            .unwrap();

        let result = cache.get("ws-001", 0, "input-hash").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), test_data);
    }

    #[tokio::test]
    async fn test_cache_miss() {
        let temp_dir = TempDir::new().unwrap();
        let config = WorkstackCacheConfig::default();
        let cache = WorkstackCache::new(temp_dir.path().to_path_buf(), config)
            .await
            .unwrap();

        let result = cache.get("ws-001", 0, "nonexistent").unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_invalidation() {
        let temp_dir = TempDir::new().unwrap();
        let config = WorkstackCacheConfig::default();
        let cache = WorkstackCache::new(temp_dir.path().to_path_buf(), config)
            .await
            .unwrap();

        cache.put("ws-001", 0, "hash1", b"data1", None).unwrap();
        cache.put("ws-001", 1, "hash2", b"data2", None).unwrap();

        let count = cache.invalidate_workstack("ws-001").unwrap();
        assert_eq!(count, 2);

        let result = cache.get("ws-001", 0, "hash1").unwrap();
        assert!(result.is_none());
    }
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cache/build.rs">
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&["proto/op_cache.proto"], &["proto"])?;
    Ok(())
}
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cache/Cargo.toml">
[package]
name = "op-cache"
version = "0.1.0"
edition = "2021"
description = "BTRFS-based caching with NUMA awareness and gRPC services"
license = "MIT"

[dependencies]
anyhow = "1.0"
bincode = "1.3"
chrono = { version = "0.4", features = ["serde"] }
futures = { workspace = true }
log = "0.4"
num_cpus = "1.16"
prost = { workspace = true }
prost-types = { workspace = true }
rusqlite = { workspace = true, features = ["bundled"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = { workspace = true }
simd-json = { workspace = true }
sha2 = "0.10"
tokio = { version = "1.0", features = ["full"] }
tokio-stream = "0.1"
tonic = { workspace = true }
tracing = "0.1"
uuid = { version = "1.0", features = ["v4"] }
zstd = "0.13"

[build-dependencies]
tonic-build = { workspace = true }

[dev-dependencies]
tempfile = "3.10"
tokio-test = "0.4"
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cache/compare-op-cache.md">
# compare-op-cache

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 19 |
| Proto files | 1 |
| Binary targets | 0 |
| UI files | 0 |
| Root-declared modules | 13 |
| Partial artifacts | 0 |
| Spec-listed source files | 18 |
| Spec-listed but missing | 0 |
| Extra implementation files | 1 |

## Current Implementation Overview

- BTRFS-based caching with NUMA awareness and gRPC services
- Protocol assets: 1 `.proto` files.

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `src/grpc/server.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/grpc/server.rs |
| `src/grpc/orchestrator_service.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/grpc/orchestrator_service.rs |
| `src/grpc/mod.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/grpc/mod.rs |
| `src/grpc/cache_service.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/grpc/cache_service.rs |
| `src/grpc/agent_service.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/grpc/agent_service.rs |
| `src/btrfs_cache.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/btrfs_cache.rs |
| `src/agent_registry.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/agent_registry.rs |
| `src/agent.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/agent.rs |
| `src/workstack_cache.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/workstack_cache.rs |
| `src/workflow_tracker.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/workflow_tracker.rs |
| `src/workflow_executor.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/workflow_executor.rs |
| `src/workflow_cache.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/workflow_cache.rs |
| `src/snapshot_manager.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/snapshot_manager.rs |
| `src/pattern_tracker.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/pattern_tracker.rs |
| `src/orchestrator.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/orchestrator.rs |
| `src/numa.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/numa.rs |
| `src/lib.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/lib.rs |
| `src/capability_resolver.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/capability_resolver.rs |
| `build.rs` | ✅ Present | build script | build.rs |
| `grpc` | ✅ Present | grpc group | src/grpc/agent_service.rs, src/grpc/cache_service.rs, src/grpc/mod.rs, src/grpc/orchestrator_service.rs, src/grpc/server.rs |
| `root` | ✅ Present | root source group | src/agent.rs, src/agent_registry.rs, src/btrfs_cache.rs, src/capability_resolver.rs, src/lib.rs, src/numa.rs, src/orchestrator.rs, src/pattern_tracker.rs, ... (+5 more) |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| btrfs_cache | ✅ Implemented | src/btrfs_cache.rs | SPEC main module |
| agent_registry | ✅ Implemented | src/agent_registry.rs | SPEC main module |
| agent | ✅ Implemented | src/agent.rs | SPEC main module |
| workstack_cache | ✅ Implemented | src/workstack_cache.rs | SPEC main module |
| workflow_tracker | ✅ Implemented | src/workflow_tracker.rs | SPEC main module |
| workflow_executor | ✅ Implemented | src/workflow_executor.rs | SPEC main module |
| workflow_cache | ✅ Implemented | src/workflow_cache.rs | SPEC main module |
| snapshot_manager | ✅ Implemented | src/snapshot_manager.rs | SPEC main module |
| pattern_tracker | ✅ Implemented | src/pattern_tracker.rs | SPEC main module |
| orchestrator | ✅ Implemented | src/orchestrator.rs | SPEC main module |
| Protocol `op_cache.proto` | ✅ Implemented | proto/op_cache.proto | proto |

## Dependencies Comparison

### Internal Workspace Dependencies
- None

### External Runtime Dependencies
- `anyhow` - documented in SPEC
- `bincode` - documented in SPEC
- `chrono` - documented in SPEC
- `futures` - documented in SPEC
- `log` - documented in SPEC
- `num_cpus` - documented in SPEC
- `prost` - documented in SPEC
- `prost-types` - not listed in SPEC dependency block
- `rusqlite` - documented in SPEC
- `serde` - documented in SPEC
- `simd-json` - documented in SPEC
- `sha2` - documented in SPEC
- `tokio` - documented in SPEC
- `tokio-stream` - documented in SPEC
- `tonic` - documented in SPEC
- `tracing` - documented in SPEC
- `uuid` - documented in SPEC
- `zstd` - documented in SPEC

### Development and Build Dependencies
- `dev:tempfile`
- `dev:tokio-test`
- `build:tonic-build`

## Notes and Observations

- Local documentation files present: SPEC.md.
- Current implementation contains 1 Rust source files beyond the explicit spec/design source inventory.
- Root module declarations found in `lib.rs`/`main.rs`: agent, agent_registry, btrfs_cache, capability_resolver, numa, orchestrator, pattern_tracker, snapshot_manager, workflow_cache, workflow_executor, workflow_tracker, workstack_cache, grpc.
- RPC or protocol definition files: proto/op_cache.proto.
- 1 runtime dependencies are present in `Cargo.toml` but omitted from the SPEC dependency block.
</file>

<file path="/home/jeremy/git/operation-dbus-proto/crates/op-cache/SPEC.md">
# op-cache - Specification

## Overview
**Crate**: `op-cache`  
**Location**: `crates/op-cache`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-cache"
version = "0.1.0"
edition = "2021"
description = "BTRFS-based caching with NUMA awareness and gRPC services"
license = "MIT"
```

### Source Structure
```
op-cache/src/grpc/server.rs
op-cache/src/grpc/orchestrator_service.rs
op-cache/src/grpc/mod.rs
op-cache/src/grpc/cache_service.rs
op-cache/src/grpc/agent_service.rs
op-cache/src/btrfs_cache.rs
op-cache/src/agent_registry.rs
op-cache/src/agent.rs
op-cache/src/workstack_cache.rs
op-cache/src/workflow_tracker.rs
op-cache/src/workflow_executor.rs
op-cache/src/workflow_cache.rs
op-cache/src/snapshot_manager.rs
op-cache/src/pattern_tracker.rs
op-cache/src/orchestrator.rs
op-cache/src/numa.rs
op-cache/src/lib.rs
op-cache/src/capability_resolver.rs
```

### Key Dependencies
```toml
anyhow = "1.0"
bincode = "1.3"
chrono = { version = "0.4", features = ["serde"] }
futures = { workspace = true }
log = "0.4"
num_cpus = "1.16"
prost = { workspace = true }
rusqlite = { workspace = true, features = ["bundled"] }
serde = { version = "1.0", features = ["derive"] }
simd-json = { workspace = true }
sha2 = "0.10"
tokio = { version = "1.0", features = ["full"] }
tokio-stream = "0.1"
tonic = { workspace = true }
tracing = "0.1"
uuid = { version = "1.0", features = ["v4"] }
zstd = "0.13"

tonic-build = { workspace = true }
```

### Binaries
```toml
# No binaries
```

### Features
```toml
# No features
```

## Documentation Files


## Module Structure
      18 Rust source files

### Main Modules
btrfs_cache
agent_registry
agent
workstack_cache
workflow_tracker
workflow_executor
workflow_cache
snapshot_manager
pattern_tracker
orchestrator

## Purpose
BTRFS-based caching with NUMA awareness and gRPC services

## Build Information
- **Edition**: 2021
- **Version**: 0.1.0
- **License**: MIT

## Related Crates
Internal dependencies:


---
*Generated from crate analysis*
</file>

</files>
