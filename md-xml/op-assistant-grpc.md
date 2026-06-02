This file is a merged representation of the entire codebase, combined into a single document by Repomix.

<file_summary>
This section contains a summary of this file.

<purpose>
This file contains a packed representation of the entire repository's contents.
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
- Files matching patterns in .gitignore are excluded
- Files matching default ignore patterns are excluded
- Files are sorted by Git change count (files with more changes are at the bottom)
</notes>

</file_summary>

<directory_structure>
proto/
  assistant/
    agent.proto
    common.proto
    cron.proto
    memory.proto
    model.proto
    namespace.proto
    session.proto
    soul.proto
    task.proto
src/
  bin/
    op-assistant-grpc.rs
  agents.rs
  auth.rs
  client.rs
  convert.rs
  cron.rs
  dbus_service.rs
  error.rs
  incus.rs
  lib.rs
  memory.rs
  models.rs
  namespace.rs
  server.rs
  sessions.rs
  soul.rs
  tasks.rs
  transport.rs
tests/
  integration.rs
build.rs
Cargo.toml
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="proto/assistant/agent.proto">
syntax = "proto3";

package assistant.v1;

import "google/protobuf/timestamp.proto";
import "google/protobuf/struct.proto";
import "assistant/common.proto";

service AgentService {
  rpc ListAgents(ListAgentsRequest) returns (ListAgentsResponse);
  rpc GetAgent(GetAgentRequest) returns (Agent);
  rpc CreateAgent(CreateAgentRequest) returns (Agent);
  rpc UpdateAgent(UpdateAgentRequest) returns (Agent);
  rpc DeleteAgent(DeleteAgentRequest) returns (Empty);
  rpc StartRun(StartRunRequest) returns (Run);
  rpc StreamRunEvents(StreamRunEventsRequest) returns (stream RunEvent);
  rpc CancelRun(CancelRunRequest) returns (Empty);
}

message Agent {
  string id = 1;
  string name = 2;
  string description = 3;
  string model = 4;
  string system_prompt = 5;
  repeated string tools = 6;
  google.protobuf.Timestamp created_at = 7;
  google.protobuf.Timestamp updated_at = 8;
  optional google.protobuf.Struct metadata = 9;
}

message Run {
  string id = 1;
  string agent_id = 2;
  string session_id = 3;
  string status = 4;        // pending|running|completed|failed|cancelled
  google.protobuf.Timestamp started_at = 5;
  optional google.protobuf.Timestamp finished_at = 6;
  optional string error = 7;
}

message RunEvent {
  string run_id = 1;
  string event_type = 2;    // tool_call|tool_result|message|status|error
  string payload_json = 3;
  google.protobuf.Timestamp timestamp = 4;
}

message ListAgentsRequest {
  Pagination pagination = 1;
  string filter = 2;
}
message ListAgentsResponse {
  repeated Agent agents = 1;
  uint32 total = 2;
}

message GetAgentRequest { string id = 1; }

message CreateAgentRequest {
  string name = 1;
  string description = 2;
  string model = 3;
  string system_prompt = 4;
  repeated string tools = 5;
  optional google.protobuf.Struct metadata = 6;
}

message UpdateAgentRequest {
  string id = 1;
  optional string name = 2;
  optional string description = 3;
  optional string model = 4;
  optional string system_prompt = 5;
  repeated string tools = 6;
  optional google.protobuf.Struct metadata = 7;
}

message DeleteAgentRequest { string id = 1; }

message StartRunRequest {
  string agent_id = 1;
  optional string session_id = 2;
  string input = 3;
  optional google.protobuf.Struct parameters = 4;
}

message StreamRunEventsRequest { string run_id = 1; }
message CancelRunRequest { string run_id = 1; }
</file>

<file path="proto/assistant/common.proto">
syntax = "proto3";

package assistant.v1;

import "google/protobuf/timestamp.proto";
import "google/protobuf/struct.proto";

message Empty {}

message Error {
  int32 code = 1;
  string message = 2;
  optional google.protobuf.Struct details = 3;
}

message Pagination {
  uint32 limit = 1;
  uint32 offset = 2;
  string cursor = 3;
}
</file>

<file path="proto/assistant/cron.proto">
syntax = "proto3";

package assistant.v1;

import "google/protobuf/timestamp.proto";
import "google/protobuf/struct.proto";
import "assistant/common.proto";

service CronService {
  rpc ListCronJobs(ListCronJobsRequest) returns (ListCronJobsResponse);
  rpc CreateCronJob(CreateCronJobRequest) returns (CronJob);
  rpc DeleteCronJob(DeleteCronJobRequest) returns (Empty);
  rpc TriggerCronJob(TriggerCronJobRequest) returns (CronJob);
}

message CronJob {
  string id = 1;
  string name = 2;
  string schedule = 3;          // cron expression
  string agent_id = 4;
  string task_name = 5;
  bool enabled = 6;
  google.protobuf.Timestamp created_at = 7;
  optional google.protobuf.Timestamp last_run = 8;
  optional google.protobuf.Timestamp next_run = 9;
  optional google.protobuf.Struct parameters = 10;
}

message ListCronJobsRequest {
  optional string agent_id = 1;
}
message ListCronJobsResponse {
  repeated CronJob jobs = 1;
}

message CreateCronJobRequest {
  string name = 1;
  string schedule = 2;
  string agent_id = 3;
  string task_name = 4;
  bool enabled = 5;
  optional google.protobuf.Struct parameters = 6;
}

message DeleteCronJobRequest { string id = 1; }
message TriggerCronJobRequest { string id = 1; }
</file>

<file path="proto/assistant/memory.proto">
syntax = "proto3";

package assistant.v1;

import "google/protobuf/timestamp.proto";
import "google/protobuf/struct.proto";
import "assistant/common.proto";

service MemoryService {
  rpc ReadMemory(ReadMemoryRequest) returns (ReadMemoryResponse);
  rpc WriteMemory(WriteMemoryRequest) returns (WriteMemoryResponse);
  rpc DeleteMemory(DeleteMemoryRequest) returns (DeleteMemoryResponse);
  rpc SearchMemory(SearchMemoryRequest) returns (SearchMemoryResponse);
  rpc GetMemoryStats(GetMemoryStatsRequest) returns (MemoryStats);
}

message MemoryEntry {
  string id = 1;
  string namespace = 2;
  string key = 3;
  string value = 4;            // JSON-encoded
  optional google.protobuf.Struct metadata = 5;
  google.protobuf.Timestamp created_at = 6;
  google.protobuf.Timestamp updated_at = 7;
}

message MemoryStats {
  string namespace = 1;
  uint64 entry_count = 2;
  uint64 bytes_used = 3;
  google.protobuf.Timestamp last_updated = 4;
}

message ReadMemoryRequest {
  string namespace = 1;
  repeated string keys = 2;
  Pagination pagination = 3;
}
message ReadMemoryResponse {
  repeated MemoryEntry entries = 1;
}

message WriteMemoryRequest {
  string namespace = 1;
  repeated MemoryEntry entries = 2;
}
message WriteMemoryResponse {
  uint32 written = 1;
}

message DeleteMemoryRequest {
  string namespace = 1;
  repeated string keys = 2;
}
message DeleteMemoryResponse {
  uint32 deleted = 1;
}

message SearchMemoryRequest {
  repeated string namespaces = 1;
  string query = 2;
  uint32 limit = 3;
}
message SearchMemoryResponse {
  repeated MemoryEntry entries = 1;
}

message GetMemoryStatsRequest { string namespace = 1; }
</file>

<file path="proto/assistant/model.proto">
syntax = "proto3";

package assistant.v1;

import "google/protobuf/struct.proto";
import "assistant/common.proto";

service ModelService {
  rpc ListModels(ListModelsRequest) returns (ListModelsResponse);
  rpc GetModel(GetModelRequest) returns (Model);
  rpc SwitchModel(SwitchModelRequest) returns (Model);
}

message Model {
  string id = 1;
  string name = 2;
  string provider = 3;
  string family = 4;
  uint32 context_window = 5;
  bool active = 6;
  optional google.protobuf.Struct capabilities = 7;
}

message ListModelsRequest {
  string filter = 1;
}
message ListModelsResponse {
  repeated Model models = 1;
}

message GetModelRequest { string id = 1; }

message SwitchModelRequest {
  string model_id = 1;
  optional string agent_id = 2;
  optional string session_id = 3;
}
</file>

<file path="proto/assistant/namespace.proto">
syntax = "proto3";

package assistant.v1;

import "google/protobuf/timestamp.proto";
import "assistant/common.proto";

service NamespaceMemoryService {
  rpc GetMemoryNamespace(GetMemoryNamespaceRequest) returns (MemoryNamespace);
  rpc SetMemoryNamespace(SetMemoryNamespaceRequest) returns (MemoryNamespace);
  rpc ClearMemoryNamespace(ClearMemoryNamespaceRequest) returns (Empty);
  rpc ListMemoryNamespaces(ListMemoryNamespacesRequest) returns (ListMemoryNamespacesResponse);
}

message MemoryNamespace {
  string agent_id = 1;
  string namespace = 2;
  uint64 entry_count = 3;
  google.protobuf.Timestamp created_at = 4;
  google.protobuf.Timestamp updated_at = 5;
}

message GetMemoryNamespaceRequest { string agent_id = 1; }

message SetMemoryNamespaceRequest {
  string agent_id = 1;
  string namespace = 2;
}

message ClearMemoryNamespaceRequest { string agent_id = 1; }

message ListMemoryNamespacesRequest {
  Pagination pagination = 1;
}

message ListMemoryNamespacesResponse {
  repeated MemoryNamespace namespaces = 1;
  uint32 total = 2;
}
</file>

<file path="proto/assistant/session.proto">
syntax = "proto3";

package assistant.v1;

import "google/protobuf/timestamp.proto";
import "google/protobuf/struct.proto";
import "assistant/common.proto";

service SessionService {
  rpc ListSessions(ListSessionsRequest) returns (ListSessionsResponse);
  rpc GetSession(GetSessionRequest) returns (Session);
  rpc CreateSession(CreateSessionRequest) returns (Session);
  rpc DeleteSession(DeleteSessionRequest) returns (Empty);
  rpc GetSessionHistory(GetSessionHistoryRequest) returns (SessionHistory);
  rpc SendMessage(SendMessageRequest) returns (Message);
}

message Session {
  string id = 1;
  string agent_id = 2;
  string title = 3;
  google.protobuf.Timestamp created_at = 4;
  google.protobuf.Timestamp updated_at = 5;
  optional google.protobuf.Struct metadata = 6;
}

message Message {
  string id = 1;
  string session_id = 2;
  string role = 3;       // user|assistant|system|tool
  string content = 4;
  google.protobuf.Timestamp timestamp = 5;
  optional google.protobuf.Struct metadata = 6;
}

message SessionHistory {
  string session_id = 1;
  repeated Message messages = 2;
}

message ListSessionsRequest {
  optional string agent_id = 1;
  Pagination pagination = 2;
}

message ListSessionsResponse {
  repeated Session sessions = 1;
  uint32 total = 2;
}

message GetSessionRequest { string id = 1; }

message CreateSessionRequest {
  string agent_id = 1;
  optional string title = 2;
  optional google.protobuf.Struct metadata = 3;
}

message DeleteSessionRequest { string id = 1; }

message GetSessionHistoryRequest {
  string session_id = 1;
  Pagination pagination = 2;
}

message SendMessageRequest {
  string session_id = 1;
  string content = 2;
  optional string role = 3;
  optional google.protobuf.Struct metadata = 4;
}
</file>

<file path="proto/assistant/soul.proto">
syntax = "proto3";

package assistant.v1;

import "google/protobuf/timestamp.proto";
import "google/protobuf/struct.proto";
import "assistant/common.proto";

service SoulService {
  rpc GetSoulMemory(GetSoulMemoryRequest) returns (SoulMemory);
  rpc UpdateSoulMemory(UpdateSoulMemoryRequest) returns (SoulMemory);
  rpc DeleteSoulMemory(DeleteSoulMemoryRequest) returns (Empty);
  rpc ListSoulMemories(ListSoulMemoriesRequest) returns (ListSoulMemoriesResponse);
}

message SoulMemory {
  string agent_id = 1;
  string identity = 2;
  string personality = 3;
  google.protobuf.Struct traits = 4;
  uint64 version = 5;
  google.protobuf.Timestamp created_at = 6;
  google.protobuf.Timestamp updated_at = 7;
}

message GetSoulMemoryRequest { string agent_id = 1; }

message UpdateSoulMemoryRequest {
  string agent_id = 1;
  optional string identity = 2;
  optional string personality = 3;
  optional google.protobuf.Struct traits = 4;
}

message DeleteSoulMemoryRequest { string agent_id = 1; }

message ListSoulMemoriesRequest {
  Pagination pagination = 1;
}
message ListSoulMemoriesResponse {
  repeated SoulMemory memories = 1;
  uint32 total = 2;
}
</file>

<file path="proto/assistant/task.proto">
syntax = "proto3";

package assistant.v1;

import "google/protobuf/timestamp.proto";
import "google/protobuf/struct.proto";
import "assistant/common.proto";

service TaskService {
  rpc ListTools(ListToolsRequest) returns (ListToolsResponse);
  rpc ExecuteTask(ExecuteTaskRequest) returns (TaskResult);
  rpc StreamTaskExecution(StreamTaskExecutionRequest) returns (stream TaskEvent);
  rpc GetTaskResult(GetTaskResultRequest) returns (TaskResult);
}

message Tool {
  string name = 1;
  string description = 2;
  string version = 3;
  optional google.protobuf.Struct schema = 4;
}

message ListToolsRequest {
  string filter = 1;
}
message ListToolsResponse {
  repeated Tool tools = 1;
}

message ExecuteTaskRequest {
  string tool_name = 1;
  optional google.protobuf.Struct arguments = 2;
  optional string session_id = 3;
  optional string agent_id = 4;
}

message TaskResult {
  string task_id = 1;
  string status = 2;            // pending|running|completed|failed
  optional google.protobuf.Struct output = 3;
  optional string error = 4;
  google.protobuf.Timestamp started_at = 5;
  optional google.protobuf.Timestamp finished_at = 6;
}

message TaskEvent {
  string task_id = 1;
  string event_type = 2;
  string payload_json = 3;
  google.protobuf.Timestamp timestamp = 4;
}

message StreamTaskExecutionRequest { string task_id = 1; }
message GetTaskResultRequest { string task_id = 1; }
</file>

<file path="src/bin/op-assistant-grpc.rs">
//! op-assistant-grpc — gRPC gateway for the self-hosted Assistant.
//!
//! Routes gRPC calls through the wg-xray Incus container's `op-grpc-bridge`
//! endpoint (default `10.200.0.1:50051`) with D-Bus-first transport and
//! ghostbridge schema-tag header injection for Xray OpenFlow routing.

use anyhow::Result;
use op_assistant_grpc::{run_grpc_server, ServerConfig};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("op_assistant_grpc=info,info")
            }),
        )
        .init();

    let cfg = ServerConfig::default();
    info!(
        host = %cfg.host,
        port = cfg.port,
        endpoint = %cfg.transport.rpc_endpoint,
        "op-assistant-grpc starting"
    );

    run_grpc_server(cfg).await
}
</file>

<file path="src/agents.rs">
//! AgentService implementation. Proxies all calls into the Assistant via the
//! [`AssistantClient`] and converts JSON responses into proto messages.

use crate::client::AssistantClient;
use crate::convert::*;
use crate::proto::agent_service_server::AgentService;
use crate::proto::{
    Agent, CancelRunRequest, CreateAgentRequest, DeleteAgentRequest, Empty, GetAgentRequest,
    ListAgentsRequest, ListAgentsResponse, Run, RunEvent, StartRunRequest, StreamRunEventsRequest,
    UpdateAgentRequest,
};
use async_stream::try_stream;
use futures::Stream;
use serde_json::{json, Value};
use std::pin::Pin;
use tonic::{Request, Response, Status};

pub struct AgentServiceImpl {
    client: AssistantClient,
}

impl AgentServiceImpl {
    pub fn new(client: AssistantClient) -> Self {
        Self { client }
    }
}

type RunEventStream = Pin<Box<dyn Stream<Item = Result<RunEvent, Status>> + Send>>;

#[tonic::async_trait]
impl AgentService for AgentServiceImpl {
    type StreamRunEventsStream = RunEventStream;

    async fn list_agents(
        &self,
        req: Request<ListAgentsRequest>,
    ) -> Result<Response<ListAgentsResponse>, Status> {
        let req = req.into_inner();
        let params = json!({
            "limit": req.pagination.as_ref().map(|p| p.limit).unwrap_or(0),
            "offset": req.pagination.as_ref().map(|p| p.offset).unwrap_or(0),
            "filter": req.filter,
        });
        let result = self.client.call("agents.list", params).await?;
        let agents = result
            .get("agents")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().map(agent_from_json).collect::<Vec<_>>())
            .unwrap_or_default();
        let total = result
            .get("total")
            .and_then(|t| t.as_u64())
            .unwrap_or(agents.len() as u64) as u32;
        Ok(Response::new(ListAgentsResponse { agents, total }))
    }

    async fn get_agent(&self, req: Request<GetAgentRequest>) -> Result<Response<Agent>, Status> {
        let id = req.into_inner().id;
        if id.is_empty() {
            return Err(Status::invalid_argument("agent id is required"));
        }
        let result = self.client.call("agents.get", json!({ "id": id })).await?;
        Ok(Response::new(agent_from_json(&result)))
    }

    async fn create_agent(
        &self,
        req: Request<CreateAgentRequest>,
    ) -> Result<Response<Agent>, Status> {
        let req = req.into_inner();
        let mut params = json!({
            "name": req.name,
            "description": req.description,
            "model": req.model,
            "system_prompt": req.system_prompt,
            "tools": req.tools,
        });
        if let Some(meta) = req.metadata {
            params["metadata"] = struct_to_json(meta);
        }
        let result = self.client.call("agents.create", params).await?;
        Ok(Response::new(agent_from_json(&result)))
    }

    async fn update_agent(
        &self,
        req: Request<UpdateAgentRequest>,
    ) -> Result<Response<Agent>, Status> {
        let req = req.into_inner();
        if req.id.is_empty() {
            return Err(Status::invalid_argument("agent id is required"));
        }
        let mut params = json!({ "id": req.id, "tools": req.tools });
        if let Some(v) = req.name {
            params["name"] = json!(v);
        }
        if let Some(v) = req.description {
            params["description"] = json!(v);
        }
        if let Some(v) = req.model {
            params["model"] = json!(v);
        }
        if let Some(v) = req.system_prompt {
            params["system_prompt"] = json!(v);
        }
        if let Some(meta) = req.metadata {
            params["metadata"] = struct_to_json(meta);
        }
        let result = self.client.call("agents.update", params).await?;
        Ok(Response::new(agent_from_json(&result)))
    }

    async fn delete_agent(
        &self,
        req: Request<DeleteAgentRequest>,
    ) -> Result<Response<Empty>, Status> {
        let id = req.into_inner().id;
        if id.is_empty() {
            return Err(Status::invalid_argument("agent id is required"));
        }
        self.client
            .call("agents.delete", json!({ "id": id }))
            .await?;
        Ok(Response::new(Empty {}))
    }

    async fn start_run(&self, req: Request<StartRunRequest>) -> Result<Response<Run>, Status> {
        let req = req.into_inner();
        let mut params = json!({
            "agent_id": req.agent_id,
            "input": req.input,
        });
        if let Some(sid) = req.session_id {
            params["session_id"] = json!(sid);
        }
        if let Some(p) = req.parameters {
            params["parameters"] = struct_to_json(p);
        }
        let result = self.client.call("agents.start_run", params).await?;
        Ok(Response::new(run_from_json(&result)))
    }

    async fn stream_run_events(
        &self,
        req: Request<StreamRunEventsRequest>,
    ) -> Result<Response<Self::StreamRunEventsStream>, Status> {
        let run_id = req.into_inner().run_id;
        if run_id.is_empty() {
            return Err(Status::invalid_argument("run_id is required"));
        }
        let client = self.client.clone();
        let stream = try_stream! {
            let result = client
                .call("agents.run_events", json!({ "run_id": run_id }))
                .await
                .map_err(Status::from)?;
            let events = result
                .get("events")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            for evt in events {
                yield run_event_from_json(&run_id, &evt);
            }
        };
        Ok(Response::new(Box::pin(stream)))
    }

    async fn cancel_run(&self, req: Request<CancelRunRequest>) -> Result<Response<Empty>, Status> {
        let run_id = req.into_inner().run_id;
        if run_id.is_empty() {
            return Err(Status::invalid_argument("run_id is required"));
        }
        self.client
            .call("agents.cancel_run", json!({ "run_id": run_id }))
            .await?;
        Ok(Response::new(Empty {}))
    }
}

pub(crate) fn agent_from_json(v: &Value) -> Agent {
    Agent {
        id: str_field(v, "id"),
        name: str_field(v, "name"),
        description: str_field(v, "description"),
        model: str_field(v, "model"),
        system_prompt: str_field(v, "system_prompt"),
        tools: string_list(v, "tools"),
        created_at: ts_field(v, "created_at"),
        updated_at: ts_field(v, "updated_at"),
        metadata: opt_struct(v, "metadata"),
    }
}

pub(crate) fn run_from_json(v: &Value) -> Run {
    Run {
        id: str_field(v, "id"),
        agent_id: str_field(v, "agent_id"),
        session_id: str_field(v, "session_id"),
        status: str_field(v, "status"),
        started_at: ts_field(v, "started_at"),
        finished_at: ts_field(v, "finished_at"),
        error: opt_str(v, "error"),
    }
}

pub(crate) fn run_event_from_json(run_id: &str, v: &Value) -> RunEvent {
    RunEvent {
        run_id: run_id.to_string(),
        event_type: str_field(v, "event_type"),
        payload_json: v
            .get("payload")
            .map(|p| p.to_string())
            .unwrap_or_else(|| v.to_string()),
        timestamp: ts_field(v, "timestamp"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_agent_from_json() {
        let v = json!({
            "id": "abc",
            "name": "Agent X",
            "tools": ["search", "code"],
        });
        let a = agent_from_json(&v);
        assert_eq!(a.id, "abc");
        assert_eq!(a.name, "Agent X");
        assert_eq!(a.tools, vec!["search", "code"]);
    }
}
</file>

<file path="src/auth.rs">
//! WireGuard identity extraction and authentication middleware.
//!
//! Trust is established at the WireGuard network layer; this middleware merely
//! extracts the public-key metadata and attaches it to the request extensions
//! so downstream handlers can attribute the call.

use tonic::{metadata::MetadataMap, Request, Status};

pub const WIREGUARD_PUBKEY_HEADER: &str = "x-wireguard-pubkey";

#[derive(Debug, Clone)]
pub struct WireGuardIdentity {
    pub pubkey: String,
}

#[allow(clippy::result_large_err)]
pub fn extract_wireguard_identity(metadata: &MetadataMap) -> Result<WireGuardIdentity, Status> {
    let raw = metadata
        .get(WIREGUARD_PUBKEY_HEADER)
        .ok_or_else(|| Status::unauthenticated("missing wireguard identity"))?;
    let pubkey = raw
        .to_str()
        .map_err(|_| Status::invalid_argument("invalid wireguard pubkey encoding"))?
        .to_string();

    if pubkey.is_empty() {
        return Err(Status::unauthenticated("empty wireguard pubkey"));
    }
    Ok(WireGuardIdentity { pubkey })
}

/// Tonic interceptor: extracts the WireGuard identity and attaches it to the
/// request extensions. Returns `Unauthenticated` when the header is missing.
#[allow(clippy::result_large_err)]
pub fn wireguard_auth_interceptor(mut req: Request<()>) -> Result<Request<()>, Status> {
    let identity = extract_wireguard_identity(req.metadata())?;
    req.extensions_mut().insert(identity);
    Ok(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tonic::metadata::MetadataValue;

    #[test]
    fn rejects_missing_identity() {
        let req = Request::new(());
        let result = wireguard_auth_interceptor(req);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::Unauthenticated);
    }

    #[test]
    fn accepts_valid_identity() {
        let mut req = Request::new(());
        req.metadata_mut().insert(
            WIREGUARD_PUBKEY_HEADER,
            MetadataValue::from_static("abcd1234"),
        );
        let result = wireguard_auth_interceptor(req).unwrap();
        let id = result.extensions().get::<WireGuardIdentity>().unwrap();
        assert_eq!(id.pubkey, "abcd1234");
    }

    #[test]
    fn rejects_empty_identity() {
        let mut req = Request::new(());
        req.metadata_mut()
            .insert(WIREGUARD_PUBKEY_HEADER, MetadataValue::from_static(""));
        let result = wireguard_auth_interceptor(req);
        assert!(result.is_err());
    }
}
</file>

<file path="src/client.rs">
//! Assistant client wrapper. Thin convenience layer on top of [`Transport`]
//! that normalises Assistant JSON-RPC responses into a `serde_json::Value`.

use crate::error::{AssistantError, Result};
use crate::transport::{Transport, TransportConfig};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Clone)]
pub struct AssistantClient {
    transport: Arc<Transport>,
}

impl AssistantClient {
    pub async fn new(cfg: TransportConfig) -> Result<Self> {
        let transport = Transport::new(cfg).await?;
        Ok(Self {
            transport: Arc::new(transport),
        })
    }

    pub fn from_transport(transport: Arc<Transport>) -> Self {
        Self { transport }
    }

    pub fn transport(&self) -> &Transport {
        &self.transport
    }

    /// Invoke a named Assistant method.
    pub async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let envelope = json!({
            "jsonrpc": "2.0",
            "id": uuid::Uuid::new_v4().to_string(),
            "method": method,
            "params": params,
        });
        let raw = self.transport.call(method, envelope).await?;
        unwrap_jsonrpc(raw)
    }
}

/// Strip JSON-RPC envelope. Accepts both `{result: ...}` and `{error: ...}`
/// shapes; if neither is present the raw value is returned as-is.
pub fn unwrap_jsonrpc(value: Value) -> Result<Value> {
    if let Some(err) = value.get("error") {
        let code = err.get("code").and_then(|c| c.as_i64()).unwrap_or(-32000);
        let message = err
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("assistant error")
            .to_string();
        return Err(match code {
            -32601 | 404 => AssistantError::NotFound(message),
            -32602 | 400 => AssistantError::InvalidRequest(message),
            401 => AssistantError::Unauthenticated(message),
            403 => AssistantError::Forbidden(message),
            -32603 | 500 => AssistantError::Internal(message),
            _ => AssistantError::Unknown(message),
        });
    }
    if let Some(result) = value.get("result") {
        return Ok(result.clone());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwrap_jsonrpc_returns_result() {
        let v = json!({"jsonrpc": "2.0", "id": "1", "result": {"x": 1}});
        let out = unwrap_jsonrpc(v).unwrap();
        assert_eq!(out, json!({"x": 1}));
    }

    #[test]
    fn unwrap_jsonrpc_maps_not_found() {
        let v = json!({"error": {"code": 404, "message": "no such agent"}});
        let err = unwrap_jsonrpc(v).unwrap_err();
        assert!(matches!(err, AssistantError::NotFound(_)));
    }

    #[test]
    fn unwrap_jsonrpc_returns_raw_when_no_envelope() {
        let v = json!({"agents": []});
        assert_eq!(unwrap_jsonrpc(v.clone()).unwrap(), v);
    }
}
</file>

<file path="src/convert.rs">
//! Shared helpers for converting between JSON values returned by the Assistant
//! HTTP API and the proto types generated by tonic.

use prost_types::value::Kind;
use prost_types::{ListValue, Struct, Timestamp, Value as PValue};
use serde_json::{Map, Value};

pub fn str_field(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(|s| s.as_str())
        .unwrap_or_default()
        .to_string()
}

pub fn u32_field(v: &Value, key: &str) -> u32 {
    v.get(key).and_then(|n| n.as_u64()).unwrap_or(0) as u32
}

pub fn u64_field(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(|n| n.as_u64()).unwrap_or(0)
}

pub fn bool_field(v: &Value, key: &str) -> bool {
    v.get(key).and_then(|n| n.as_bool()).unwrap_or(false)
}

pub fn opt_str(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(|s| s.as_str()).map(String::from)
}

pub fn string_list(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(|s| s.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|s| s.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

pub fn ts_field(v: &Value, key: &str) -> Option<Timestamp> {
    let raw = v.get(key)?;
    if let Some(s) = raw.as_str() {
        if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
            return Some(Timestamp {
                seconds: dt.timestamp(),
                nanos: dt.timestamp_subsec_nanos() as i32,
            });
        }
    }
    if let Some(secs) = raw.as_i64() {
        return Some(Timestamp {
            seconds: secs,
            nanos: 0,
        });
    }
    None
}

pub fn struct_to_json(s: Struct) -> Value {
    let mut map = Map::with_capacity(s.fields.len());
    for (k, v) in s.fields {
        map.insert(k, pvalue_to_json(v));
    }
    Value::Object(map)
}

pub fn pvalue_to_json(p: PValue) -> Value {
    match p.kind {
        None | Some(Kind::NullValue(_)) => Value::Null,
        Some(Kind::BoolValue(b)) => Value::Bool(b),
        Some(Kind::NumberValue(n)) => serde_json::Number::from_f64(n)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        Some(Kind::StringValue(s)) => Value::String(s),
        Some(Kind::StructValue(s)) => struct_to_json(s),
        Some(Kind::ListValue(ListValue { values })) => {
            Value::Array(values.into_iter().map(pvalue_to_json).collect())
        }
    }
}

pub fn json_to_struct(v: Value) -> Struct {
    match v {
        Value::Object(map) => Struct {
            fields: map
                .into_iter()
                .map(|(k, v)| (k, json_to_pvalue(v)))
                .collect(),
        },
        _ => Struct::default(),
    }
}

pub fn json_to_pvalue(v: Value) -> PValue {
    let kind = match v {
        Value::Null => Kind::NullValue(0),
        Value::Bool(b) => Kind::BoolValue(b),
        Value::Number(n) => Kind::NumberValue(n.as_f64().unwrap_or(0.0)),
        Value::String(s) => Kind::StringValue(s),
        Value::Array(arr) => Kind::ListValue(ListValue {
            values: arr.into_iter().map(json_to_pvalue).collect(),
        }),
        Value::Object(map) => Kind::StructValue(Struct {
            fields: map
                .into_iter()
                .map(|(k, v)| (k, json_to_pvalue(v)))
                .collect(),
        }),
    };
    PValue { kind: Some(kind) }
}

pub fn opt_struct(v: &Value, key: &str) -> Option<Struct> {
    v.get(key).cloned().map(json_to_struct)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn struct_roundtrip() {
        let original = json!({"a": 1.0, "b": "x", "c": [true, null]});
        let s = json_to_struct(original.clone());
        let back = struct_to_json(s);
        assert_eq!(back, original);
    }

    #[test]
    fn ts_parses_rfc3339() {
        let v = json!({"t": "2024-01-02T03:04:05Z"});
        let t = ts_field(&v, "t").unwrap();
        assert!(t.seconds > 0);
    }
}
</file>

<file path="src/cron.rs">
//! CronService implementation.

use crate::client::AssistantClient;
use crate::convert::*;
use crate::proto::cron_service_server::CronService;
use crate::proto::{
    CreateCronJobRequest, CronJob, DeleteCronJobRequest, Empty, ListCronJobsRequest,
    ListCronJobsResponse, TriggerCronJobRequest,
};
use serde_json::{json, Value};
use tonic::{Request, Response, Status};

pub struct CronServiceImpl {
    client: AssistantClient,
}

impl CronServiceImpl {
    pub fn new(client: AssistantClient) -> Self {
        Self { client }
    }
}

#[tonic::async_trait]
impl CronService for CronServiceImpl {
    async fn list_cron_jobs(
        &self,
        req: Request<ListCronJobsRequest>,
    ) -> Result<Response<ListCronJobsResponse>, Status> {
        let mut params = json!({});
        if let Some(a) = req.into_inner().agent_id {
            params["agent_id"] = json!(a);
        }
        let result = self.client.call("cron.list", params).await?;
        let jobs = result
            .get("jobs")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().map(cron_job_from_json).collect())
            .unwrap_or_default();
        Ok(Response::new(ListCronJobsResponse { jobs }))
    }

    async fn create_cron_job(
        &self,
        req: Request<CreateCronJobRequest>,
    ) -> Result<Response<CronJob>, Status> {
        let req = req.into_inner();
        let mut params = json!({
            "name": req.name,
            "schedule": req.schedule,
            "agent_id": req.agent_id,
            "task_name": req.task_name,
            "enabled": req.enabled,
        });
        if let Some(p) = req.parameters {
            params["parameters"] = struct_to_json(p);
        }
        let result = self.client.call("cron.create", params).await?;
        Ok(Response::new(cron_job_from_json(&result)))
    }

    async fn delete_cron_job(
        &self,
        req: Request<DeleteCronJobRequest>,
    ) -> Result<Response<Empty>, Status> {
        let id = req.into_inner().id;
        if id.is_empty() {
            return Err(Status::invalid_argument("cron id required"));
        }
        self.client.call("cron.delete", json!({ "id": id })).await?;
        Ok(Response::new(Empty {}))
    }

    async fn trigger_cron_job(
        &self,
        req: Request<TriggerCronJobRequest>,
    ) -> Result<Response<CronJob>, Status> {
        let id = req.into_inner().id;
        if id.is_empty() {
            return Err(Status::invalid_argument("cron id required"));
        }
        let result = self
            .client
            .call("cron.trigger", json!({ "id": id }))
            .await?;
        Ok(Response::new(cron_job_from_json(&result)))
    }
}

fn cron_job_from_json(v: &Value) -> CronJob {
    CronJob {
        id: str_field(v, "id"),
        name: str_field(v, "name"),
        schedule: str_field(v, "schedule"),
        agent_id: str_field(v, "agent_id"),
        task_name: str_field(v, "task_name"),
        enabled: bool_field(v, "enabled"),
        created_at: ts_field(v, "created_at"),
        last_run: ts_field(v, "last_run"),
        next_run: ts_field(v, "next_run"),
        parameters: opt_struct(v, "parameters"),
    }
}
</file>

<file path="src/dbus_service.rs">
//! D-Bus side of the Assistant gateway. Exposes a generic `call(method,
//! payload_json) -> response_json` surface so the gRPC side can dispatch
//! Assistant operations through D-Bus when available.
//!
//! Authentication is delegated to the bus policy files (`/usr/share/dbus-1/
//! system.d/ai.assistant.v1.conf`).

use crate::client::AssistantClient;
use crate::transport::{DEFAULT_DBUS_NAME, DEFAULT_DBUS_PATH};
use std::sync::Arc;
use zbus::object_server::SignalEmitter;

pub struct AssistantDbusService {
    client: Arc<AssistantClient>,
}

impl AssistantDbusService {
    pub fn new(client: Arc<AssistantClient>) -> Self {
        Self { client }
    }
}

#[zbus::interface(name = "ai.assistant.v1")]
impl AssistantDbusService {
    /// Generic JSON-RPC style passthrough. Returns the JSON-encoded response
    /// from the Assistant gateway.
    async fn call(&self, method: String, payload_json: String) -> zbus::fdo::Result<String> {
        let params: serde_json::Value = serde_json::from_str(&payload_json)
            .map_err(|e| zbus::fdo::Error::InvalidArgs(format!("invalid json: {}", e)))?;
        let result = self
            .client
            .call(&method, params)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        Ok(result.to_string())
    }

    /// Signal fired whenever an Assistant run emits an event. The gRPC side
    /// can subscribe to this signal to relay events to gRPC streaming clients.
    #[zbus(signal)]
    pub async fn run_event(
        emitter: &SignalEmitter<'_>,
        run_id: String,
        event_json: String,
    ) -> zbus::Result<()>;
}

/// Publish the D-Bus interface on the session bus. Returns the held connection
/// so callers can keep it alive.
pub async fn serve(client: Arc<AssistantClient>) -> zbus::Result<zbus::Connection> {
    let name = std::env::var("OP_ASSISTANT_DBUS_NAME").unwrap_or_else(|_| DEFAULT_DBUS_NAME.into());
    let path = std::env::var("OP_ASSISTANT_DBUS_PATH").unwrap_or_else(|_| DEFAULT_DBUS_PATH.into());

    let svc = AssistantDbusService::new(client);
    let conn = zbus::connection::Builder::session()?
        .name(name.as_str())?
        .serve_at(path.as_str(), svc)?
        .build()
        .await?;
    Ok(conn)
}
</file>

<file path="src/error.rs">
use thiserror::Error;
use tonic::Status;

#[derive(Debug, Error)]
pub enum AssistantError {
    #[error("Assistant resource not found: {0}")]
    NotFound(String),

    #[error("Unauthenticated: {0}")]
    Unauthenticated(String),

    #[error("Permission denied: {0}")]
    Forbidden(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Assistant returned internal error: {0}")]
    Internal(String),

    #[error("Transport error: {0}")]
    Transport(String),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("D-Bus error: {0}")]
    DBus(#[from] zbus::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

impl From<AssistantError> for Status {
    fn from(err: AssistantError) -> Self {
        match err {
            AssistantError::NotFound(m) => Status::not_found(m),
            AssistantError::Unauthenticated(m) => Status::unauthenticated(m),
            AssistantError::Forbidden(m) => Status::permission_denied(m),
            AssistantError::InvalidRequest(m) => Status::invalid_argument(m),
            AssistantError::Internal(m) => Status::internal(m),
            AssistantError::Transport(m) => Status::unavailable(m),
            AssistantError::Http(e) => {
                if let Some(code) = e.status() {
                    map_http_status(code.as_u16(), e.to_string())
                } else {
                    Status::unavailable(e.to_string())
                }
            }
            AssistantError::DBus(e) => Status::unavailable(format!("dbus: {}", e)),
            AssistantError::Serde(e) => Status::internal(format!("serde: {}", e)),
            AssistantError::Unknown(m) => Status::unknown(m),
        }
    }
}

pub fn map_http_status(code: u16, message: impl Into<String>) -> Status {
    let m = message.into();
    match code {
        400 => Status::invalid_argument(m),
        401 => Status::unauthenticated(m),
        403 => Status::permission_denied(m),
        404 => Status::not_found(m),
        408 | 504 => Status::deadline_exceeded(m),
        429 => Status::resource_exhausted(m),
        500..=599 => Status::internal(m),
        _ => Status::unknown(m),
    }
}

pub type Result<T> = std::result::Result<T, AssistantError>;
</file>

<file path="src/incus.rs">
//! wg-xray container endpoint + Xray schema-tag routing.
//!
//! Topology observed via `incus info wg-xray` + `systemctl cat ...`:
//!
//! - The privileged `wg-xray` Incus container runs `op-grpc-bridge` on
//!   `10.200.0.1:50051` (the `grpc-uplink` bridge IP, eth0 inside the CT).
//! - Xray runs alongside in the same CT and applies OpenFlow + PluginSchema
//!   tags to route traffic to WireGuard peers / wgcf egress.
//! - D-Bus session + system sockets are bind-mounted from the host, so D-Bus
//!   IPC works transparently across the container boundary.
//!
//! Outbound RPC calls from this crate target the bridge IP directly and carry
//! `x-ghostbridge-footprint` / `x-ghostbridge-trace-id` headers sourced from
//! `/dev/shm/plugin_schema.dat` so Xray's OpenFlow rules can route them.

use crate::error::{AssistantError, Result};
use std::fs::File;
use std::io::Read;

/// Default `op-grpc-bridge` endpoint inside the `wg-xray` container.
pub const DEFAULT_WG_XRAY_ENDPOINT: &str = "http://10.200.0.1:50051";
/// Xray SOCKS/MCP control plane (host-side proxy device, see
/// `incus config device show wg-xray`).
pub const DEFAULT_XRAY_MCP_ENDPOINT: &str = "tcp://127.0.0.1:1081";

pub const ENV_RPC_ENDPOINT: &str = "OP_ASSISTANT_RPC_ENDPOINT";
pub const ENV_XRAY_MCP: &str = "OP_ASSISTANT_XRAY_MCP";
pub const ENV_SCHEMA_PATH: &str = "OP_ASSISTANT_SCHEMA_PATH";

pub const HEADER_FOOTPRINT: &str = "x-ghostbridge-footprint";
pub const HEADER_TRACE_ID: &str = "x-ghostbridge-trace-id";

const DEFAULT_SCHEMA_PATH: &str = "/dev/shm/plugin_schema.dat";

/// Schema tags pulled from the host's PluginSchema sled. Injected into every
/// outbound RPC so Xray's OpenFlow controller can route the request.
#[derive(Debug, Clone, Default)]
pub struct SchemaTags {
    pub footprint_hex: String,
    pub trace_id: String,
}

impl SchemaTags {
    /// Load tags from `/dev/shm/plugin_schema.dat` (or `OP_ASSISTANT_SCHEMA_PATH`).
    /// Returns an all-zero/empty struct when the sled is missing — the
    /// transport may then choose to fail closed.
    pub fn load() -> Self {
        let path = std::env::var(ENV_SCHEMA_PATH).unwrap_or_else(|_| DEFAULT_SCHEMA_PATH.into());
        match read_schema_sled(&path) {
            Ok(tags) => tags,
            Err(e) => {
                tracing::debug!(error = %e, path = %path, "schema sled not loadable");
                Self::default()
            }
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.footprint_hex.is_empty() && self.footprint_hex.chars().any(|c| c != '0')
    }
}

/// Read the raw `IdentitySled` layout from shared memory. We re-implement the
/// minimal parse here rather than pulling in op-identity to keep this crate
/// dependency-light.
///
/// Layout (matches `op_identity::schema_bridge::IdentitySled`):
///   wg_pubkey:        [u8; 32]
///   mutation_index:   u64 (LE)
///   hashed_footprint: [u8; 32]
///   trace_id:         [u8; 32]
fn read_schema_sled(path: &str) -> Result<SchemaTags> {
    let mut buf = Vec::with_capacity(128);
    File::open(path)
        .and_then(|mut f| f.read_to_end(&mut buf))
        .map_err(|e| AssistantError::Transport(format!("read {path}: {e}")))?;

    const WG_OFF: usize = 0;
    const _WG_LEN: usize = 32;
    const MUT_OFF: usize = 32;
    const _MUT_LEN: usize = 8;
    const FP_OFF: usize = 40;
    const FP_LEN: usize = 32;
    const TRACE_OFF: usize = 72;
    const TRACE_LEN: usize = 32;
    const MIN_LEN: usize = TRACE_OFF + TRACE_LEN;

    if buf.len() < MIN_LEN {
        return Err(AssistantError::Transport(format!(
            "schema sled too short: {} < {}",
            buf.len(),
            MIN_LEN
        )));
    }
    let _ = WG_OFF;
    let _ = MUT_OFF;
    let footprint = &buf[FP_OFF..FP_OFF + FP_LEN];
    let trace = &buf[TRACE_OFF..TRACE_OFF + TRACE_LEN];
    Ok(SchemaTags {
        footprint_hex: hex_encode(footprint),
        trace_id: hex_encode(trace),
    })
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(nibble((b >> 4) & 0xF));
        s.push(nibble(b & 0xF));
    }
    s
}

fn nibble(n: u8) -> char {
    match n {
        0..=9 => (b'0' + n) as char,
        _ => (b'a' + (n - 10)) as char,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_encode_matches_lowercase() {
        assert_eq!(hex_encode(&[0xAB, 0xCD, 0x01]), "abcd01");
        assert_eq!(hex_encode(&[0; 4]), "00000000");
    }

    #[test]
    fn tags_default_invalid() {
        let t = SchemaTags::default();
        assert!(!t.is_valid());
    }

    #[test]
    fn read_short_buffer_fails() {
        let p = std::env::temp_dir().join("op-assistant-grpc-short-sled.dat");
        std::fs::write(&p, b"too short").unwrap();
        assert!(read_schema_sled(p.to_str().unwrap()).is_err());
        let _ = std::fs::remove_file(&p);
    }
}
</file>

<file path="src/lib.rs">
//! op-assistant-grpc — gRPC gateway for Assistant integration.
//!
//! Architecture:
//! ```text
//!  gRPC Client  →  AssistantGrpcServer  →  Transport (D-Bus | HTTP-RPC)  →  Assistant
//! ```
//!
//! - Authentication is WireGuard-identity based (zero-trust at the network layer).
//! - Primary transport is D-Bus; falls back to JSON-RPC over HTTP when D-Bus is unavailable.
//! - Each Assistant API surface (agents, sessions, tasks, models, cron, soul, namespace,
//!   memory) is exposed as its own gRPC service.

pub mod agents;
pub mod auth;
pub mod client;
pub mod convert;
pub mod cron;
pub mod dbus_service;
pub mod error;
pub mod incus;
pub mod memory;
pub mod models;
pub mod namespace;
pub mod server;
pub mod sessions;
pub mod soul;
pub mod tasks;
pub mod transport;

pub use auth::{wireguard_auth_interceptor, WireGuardIdentity};
pub use client::AssistantClient;
pub use error::AssistantError;
pub use server::{run_grpc_server, AssistantGrpcServer, ServerConfig};
pub use transport::{Transport, TransportConfig, TransportKind};

/// Generated protobuf types.
pub mod proto {
    tonic::include_proto!("assistant.v1");

    /// Combined FileDescriptorSet for tonic-reflection.
    pub const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("assistant_descriptor");
}
</file>

<file path="src/memory.rs">
//! MemoryService implementation — backed directly by op-cognitive-mcp's
//! `CognitiveMemoryStore` (CozoDB). No HTTP round-trip.

use crate::convert::*;
use crate::proto::memory_service_server::MemoryService;
use crate::proto::{
    DeleteMemoryRequest, DeleteMemoryResponse, GetMemoryStatsRequest, MemoryEntry, MemoryStats,
    ReadMemoryRequest, ReadMemoryResponse, SearchMemoryRequest, SearchMemoryResponse,
    WriteMemoryRequest, WriteMemoryResponse,
};
use op_cognitive_mcp::memory_store::{
    CognitiveMemoryStore, EntryQuery, MemoryEntry as StoreEntry, NamespaceKind,
};
use serde_json::Value;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct MemoryServiceImpl {
    store: Arc<CognitiveMemoryStore>,
}

impl MemoryServiceImpl {
    pub fn new(store: Arc<CognitiveMemoryStore>) -> Self {
        Self { store }
    }
}

#[tonic::async_trait]
impl MemoryService for MemoryServiceImpl {
    async fn read_memory(
        &self,
        req: Request<ReadMemoryRequest>,
    ) -> Result<Response<ReadMemoryResponse>, Status> {
        let req = req.into_inner();
        if req.namespace.is_empty() {
            return Err(Status::invalid_argument("namespace required"));
        }

        let entries = if req.keys.is_empty() {
            let q = EntryQuery {
                namespace_id: Some(req.namespace.clone()),
                key_pattern: None,
                tags: None,
                limit: req.pagination.as_ref().map(|p| p.limit as i64),
                offset: req.pagination.as_ref().map(|p| p.offset as i64),
            };
            self.store
                .query_entries(q)
                .await
                .map_err(|e| Status::internal(e.to_string()))?
        } else {
            let mut out = Vec::with_capacity(req.keys.len());
            for k in &req.keys {
                if let Some(e) = self
                    .store
                    .retrieve_entry(&req.namespace, k)
                    .await
                    .map_err(|e| Status::internal(e.to_string()))?
                {
                    out.push(e);
                }
            }
            out
        };

        Ok(Response::new(ReadMemoryResponse {
            entries: entries.iter().map(entry_to_proto).collect(),
        }))
    }

    async fn write_memory(
        &self,
        req: Request<WriteMemoryRequest>,
    ) -> Result<Response<WriteMemoryResponse>, Status> {
        let req = req.into_inner();
        if req.namespace.is_empty() {
            return Err(Status::invalid_argument("namespace required"));
        }

        ensure_namespace(&self.store, &req.namespace, NamespaceKind::Custom).await?;

        let mut written = 0u32;
        for entry in req.entries {
            let value: Value =
                serde_json::from_str(&entry.value).unwrap_or(Value::String(entry.value));
            let tags = tags_from_metadata(&entry.metadata);
            self.store
                .store_entry(&req.namespace, &entry.key, value, tags, None)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
            written += 1;
        }
        Ok(Response::new(WriteMemoryResponse { written }))
    }

    async fn delete_memory(
        &self,
        req: Request<DeleteMemoryRequest>,
    ) -> Result<Response<DeleteMemoryResponse>, Status> {
        let req = req.into_inner();
        if req.namespace.is_empty() {
            return Err(Status::invalid_argument("namespace required"));
        }
        let mut deleted = 0u32;
        for k in req.keys {
            if self
                .store
                .delete_entry(&req.namespace, &k)
                .await
                .map_err(|e| Status::internal(e.to_string()))?
            {
                deleted += 1;
            }
        }
        Ok(Response::new(DeleteMemoryResponse { deleted }))
    }

    async fn search_memory(
        &self,
        req: Request<SearchMemoryRequest>,
    ) -> Result<Response<SearchMemoryResponse>, Status> {
        let req = req.into_inner();
        if req.query.is_empty() {
            return Err(Status::invalid_argument("query required"));
        }
        let limit = if req.limit == 0 { 50 } else { req.limit as i64 };

        let namespaces = if req.namespaces.is_empty() {
            vec![None]
        } else {
            req.namespaces.iter().map(|n| Some(n.clone())).collect()
        };

        let mut out = Vec::new();
        for ns in namespaces {
            let q = EntryQuery {
                namespace_id: ns,
                key_pattern: Some(req.query.clone()),
                tags: None,
                limit: Some(limit),
                offset: None,
            };
            let rows = self
                .store
                .query_entries(q)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
            out.extend(rows.iter().map(entry_to_proto));
        }
        out.truncate(limit as usize);
        Ok(Response::new(SearchMemoryResponse { entries: out }))
    }

    async fn get_memory_stats(
        &self,
        req: Request<GetMemoryStatsRequest>,
    ) -> Result<Response<MemoryStats>, Status> {
        let ns = req.into_inner().namespace;
        if ns.is_empty() {
            return Err(Status::invalid_argument("namespace required"));
        }
        let q = EntryQuery {
            namespace_id: Some(ns.clone()),
            ..Default::default()
        };
        let entries = self
            .store
            .query_entries(q)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let bytes_used: u64 = entries
            .iter()
            .map(|e| {
                e.key.len() as u64
                    + serde_json::to_string(&e.value)
                        .map(|s| s.len() as u64)
                        .unwrap_or(0)
            })
            .sum();
        let last_updated =
            entries
                .iter()
                .map(|e| e.updated_at)
                .max()
                .map(|t| prost_types::Timestamp {
                    seconds: t.timestamp(),
                    nanos: t.timestamp_subsec_nanos() as i32,
                });

        Ok(Response::new(MemoryStats {
            namespace: ns,
            entry_count: entries.len() as u64,
            bytes_used,
            last_updated,
        }))
    }
}

pub(crate) fn entry_to_proto(e: &StoreEntry) -> MemoryEntry {
    MemoryEntry {
        id: e.id.clone(),
        namespace: e.namespace_id.clone(),
        key: e.key.clone(),
        value: serde_json::to_string(&e.value).unwrap_or_default(),
        metadata: if e.tags.is_empty() {
            None
        } else {
            Some(json_to_struct(serde_json::json!({ "tags": e.tags })))
        },
        created_at: Some(prost_types::Timestamp {
            seconds: e.created_at.timestamp(),
            nanos: e.created_at.timestamp_subsec_nanos() as i32,
        }),
        updated_at: Some(prost_types::Timestamp {
            seconds: e.updated_at.timestamp(),
            nanos: e.updated_at.timestamp_subsec_nanos() as i32,
        }),
    }
}

fn tags_from_metadata(meta: &Option<prost_types::Struct>) -> Vec<String> {
    let Some(m) = meta else { return Vec::new() };
    let json = struct_to_json(m.clone());
    json.get("tags")
        .and_then(|t| t.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

pub async fn ensure_namespace(
    store: &CognitiveMemoryStore,
    name: &str,
    kind: NamespaceKind,
) -> Result<(), Status> {
    if store
        .get_namespace_by_name(name)
        .await
        .map_err(|e| Status::internal(e.to_string()))?
        .is_some()
    {
        return Ok(());
    }
    store
        .upsert_namespace(name, kind, None, None, None, Value::Null)
        .await
        .map_err(|e| Status::internal(e.to_string()))?;
    Ok(())
}
</file>

<file path="src/models.rs">
//! ModelService implementation.

use crate::client::AssistantClient;
use crate::convert::*;
use crate::proto::model_service_server::ModelService;
use crate::proto::{
    GetModelRequest, ListModelsRequest, ListModelsResponse, Model, SwitchModelRequest,
};
use serde_json::{json, Value};
use tonic::{Request, Response, Status};

pub struct ModelServiceImpl {
    client: AssistantClient,
}

impl ModelServiceImpl {
    pub fn new(client: AssistantClient) -> Self {
        Self { client }
    }
}

#[tonic::async_trait]
impl ModelService for ModelServiceImpl {
    async fn list_models(
        &self,
        req: Request<ListModelsRequest>,
    ) -> Result<Response<ListModelsResponse>, Status> {
        let result = self
            .client
            .call("models.list", json!({ "filter": req.into_inner().filter }))
            .await?;
        let models = result
            .get("models")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().map(model_from_json).collect())
            .unwrap_or_default();
        Ok(Response::new(ListModelsResponse { models }))
    }

    async fn get_model(&self, req: Request<GetModelRequest>) -> Result<Response<Model>, Status> {
        let id = req.into_inner().id;
        if id.is_empty() {
            return Err(Status::invalid_argument("model id required"));
        }
        let result = self.client.call("models.get", json!({ "id": id })).await?;
        Ok(Response::new(model_from_json(&result)))
    }

    async fn switch_model(
        &self,
        req: Request<SwitchModelRequest>,
    ) -> Result<Response<Model>, Status> {
        let req = req.into_inner();
        let mut params = json!({ "model_id": req.model_id });
        if let Some(a) = req.agent_id {
            params["agent_id"] = json!(a);
        }
        if let Some(s) = req.session_id {
            params["session_id"] = json!(s);
        }
        let result = self.client.call("models.switch", params).await?;
        Ok(Response::new(model_from_json(&result)))
    }
}

fn model_from_json(v: &Value) -> Model {
    Model {
        id: str_field(v, "id"),
        name: str_field(v, "name"),
        provider: str_field(v, "provider"),
        family: str_field(v, "family"),
        context_window: u32_field(v, "context_window"),
        active: bool_field(v, "active"),
        capabilities: opt_struct(v, "capabilities"),
    }
}
</file>

<file path="src/namespace.rs">
//! NamespaceMemoryService — backed by `SoulMemoryStore::*_binding` for the
//! agent → namespace mapping and `CognitiveMemoryStore` for the namespace
//! itself.

use crate::proto::namespace_memory_service_server::NamespaceMemoryService;
use crate::proto::{
    ClearMemoryNamespaceRequest, Empty, GetMemoryNamespaceRequest, ListMemoryNamespacesRequest,
    ListMemoryNamespacesResponse, MemoryNamespace, SetMemoryNamespaceRequest,
};
use op_cognitive_mcp::memory_store::{CognitiveMemoryStore, EntryQuery, NamespaceKind};
use op_cognitive_mcp::soul_memory::{AgentNamespaceBinding, SoulMemoryStore};
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct NamespaceMemoryServiceImpl {
    memory: Arc<CognitiveMemoryStore>,
    bindings: Arc<SoulMemoryStore>,
}

impl NamespaceMemoryServiceImpl {
    pub fn new(memory: Arc<CognitiveMemoryStore>, bindings: Arc<SoulMemoryStore>) -> Self {
        Self { memory, bindings }
    }
}

#[tonic::async_trait]
impl NamespaceMemoryService for NamespaceMemoryServiceImpl {
    async fn get_memory_namespace(
        &self,
        req: Request<GetMemoryNamespaceRequest>,
    ) -> Result<Response<MemoryNamespace>, Status> {
        let agent = req.into_inner().agent_id;
        if agent.is_empty() {
            return Err(Status::invalid_argument("agent_id required"));
        }
        let binding = self
            .bindings
            .get_binding(&agent)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("no namespace bound for agent"))?;
        let count = count_entries(&self.memory, &binding.namespace).await;
        Ok(Response::new(binding_to_proto(&binding, count)))
    }

    async fn set_memory_namespace(
        &self,
        req: Request<SetMemoryNamespaceRequest>,
    ) -> Result<Response<MemoryNamespace>, Status> {
        let req = req.into_inner();
        if req.agent_id.is_empty() || req.namespace.is_empty() {
            return Err(Status::invalid_argument("agent_id and namespace required"));
        }
        crate::memory::ensure_namespace(&self.memory, &req.namespace, NamespaceKind::Agent).await?;
        let binding = self
            .bindings
            .bind_namespace(&req.agent_id, &req.namespace)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let count = count_entries(&self.memory, &binding.namespace).await;
        Ok(Response::new(binding_to_proto(&binding, count)))
    }

    async fn clear_memory_namespace(
        &self,
        req: Request<ClearMemoryNamespaceRequest>,
    ) -> Result<Response<Empty>, Status> {
        let agent = req.into_inner().agent_id;
        if agent.is_empty() {
            return Err(Status::invalid_argument("agent_id required"));
        }
        if let Some(binding) = self
            .bindings
            .get_binding(&agent)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
        {
            self.memory
                .delete_namespace(&binding.namespace)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
            self.bindings
                .clear_binding(&agent)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
        }
        Ok(Response::new(Empty {}))
    }

    async fn list_memory_namespaces(
        &self,
        _req: Request<ListMemoryNamespacesRequest>,
    ) -> Result<Response<ListMemoryNamespacesResponse>, Status> {
        let bindings = self
            .bindings
            .list_bindings()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let mut out = Vec::with_capacity(bindings.len());
        for b in &bindings {
            let count = count_entries(&self.memory, &b.namespace).await;
            out.push(binding_to_proto(b, count));
        }
        let total = out.len() as u32;
        Ok(Response::new(ListMemoryNamespacesResponse {
            namespaces: out,
            total,
        }))
    }
}

fn binding_to_proto(b: &AgentNamespaceBinding, entry_count: u64) -> MemoryNamespace {
    MemoryNamespace {
        agent_id: b.agent_id.clone(),
        namespace: b.namespace.clone(),
        entry_count,
        created_at: Some(prost_types::Timestamp {
            seconds: b.created_at.timestamp(),
            nanos: b.created_at.timestamp_subsec_nanos() as i32,
        }),
        updated_at: Some(prost_types::Timestamp {
            seconds: b.updated_at.timestamp(),
            nanos: b.updated_at.timestamp_subsec_nanos() as i32,
        }),
    }
}

async fn count_entries(store: &CognitiveMemoryStore, ns: &str) -> u64 {
    store
        .query_entries(EntryQuery {
            namespace_id: Some(ns.into()),
            ..Default::default()
        })
        .await
        .map(|e| e.len() as u64)
        .unwrap_or(0)
}
</file>

<file path="src/server.rs">
//! Tonic gRPC server wiring. Registers every Assistant gateway service,
//! reflection, health checks, the WireGuard auth interceptor, and an optional
//! gRPC-Web layer for browser clients.

use crate::agents::AgentServiceImpl;
use crate::auth::wireguard_auth_interceptor;
use crate::client::AssistantClient;
use crate::cron::CronServiceImpl;
use crate::memory::MemoryServiceImpl;
use crate::models::ModelServiceImpl;
use crate::namespace::NamespaceMemoryServiceImpl;
use crate::proto::agent_service_server::AgentServiceServer;
use crate::proto::cron_service_server::CronServiceServer;
use crate::proto::memory_service_server::MemoryServiceServer;
use crate::proto::model_service_server::ModelServiceServer;
use crate::proto::namespace_memory_service_server::NamespaceMemoryServiceServer;
use crate::proto::session_service_server::SessionServiceServer;
use crate::proto::soul_service_server::SoulServiceServer;
use crate::proto::task_service_server::TaskServiceServer;
use crate::sessions::SessionServiceImpl;
use crate::soul::SoulServiceImpl;
use crate::tasks::TaskServiceImpl;
use crate::transport::TransportConfig;
use op_cognitive_mcp::cozo_shuttle::CozoGraphShuttle;
use op_cognitive_mcp::memory_store::CognitiveMemoryStore;
use op_cognitive_mcp::soul_memory::SoulMemoryStore;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tonic::transport::Server;
use tracing::info;

pub const DEFAULT_GRPC_PORT: u16 = 50051;
pub const DEFAULT_GRPC_HOST: &str = "0.0.0.0";

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub transport: TransportConfig,
    pub enable_grpc_web: bool,
    pub enable_reflection: bool,
    /// CozoDB path backing memory / soul / namespace stores. Empty = in-memory.
    pub cozo_db_path: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: std::env::var("OP_ASSISTANT_GRPC_HOST")
                .unwrap_or_else(|_| DEFAULT_GRPC_HOST.to_string()),
            port: std::env::var("OP_ASSISTANT_GRPC_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(DEFAULT_GRPC_PORT),
            transport: TransportConfig::default(),
            enable_grpc_web: std::env::var("OP_ASSISTANT_GRPC_WEB")
                .map(|v| matches!(v.as_str(), "1" | "true" | "yes"))
                .unwrap_or(false),
            enable_reflection: true,
            cozo_db_path: std::env::var("OP_ASSISTANT_COZO_PATH").unwrap_or_default(),
        }
    }
}

pub struct AssistantGrpcServer {
    cfg: ServerConfig,
    client: AssistantClient,
    memory_store: Arc<CognitiveMemoryStore>,
    soul_store: Arc<SoulMemoryStore>,
}

impl AssistantGrpcServer {
    pub async fn new(cfg: ServerConfig) -> anyhow::Result<Self> {
        let client = AssistantClient::new(cfg.transport.clone()).await?;

        let shuttle = if cfg.cozo_db_path.is_empty() {
            CozoGraphShuttle::new_in_memory()?
        } else {
            CozoGraphShuttle::new_persistent(PathBuf::from(&cfg.cozo_db_path))?
        };
        let shuttle = Arc::new(shuttle);
        let memory_store = Arc::new(CognitiveMemoryStore::new(shuttle.clone()).await?);
        let soul_store = Arc::new(SoulMemoryStore::new(shuttle));

        Ok(Self {
            cfg,
            client,
            memory_store,
            soul_store,
        })
    }

    pub fn client(&self) -> &AssistantClient {
        &self.client
    }

    pub fn memory_store(&self) -> Arc<CognitiveMemoryStore> {
        self.memory_store.clone()
    }

    pub fn soul_store(&self) -> Arc<SoulMemoryStore> {
        self.soul_store.clone()
    }

    pub async fn serve(self) -> anyhow::Result<()> {
        let addr: SocketAddr = format!("{}:{}", self.cfg.host, self.cfg.port).parse()?;
        info!(%addr, transport = ?self.client.transport().primary_kind(), "starting op-assistant-grpc");

        let agent = AgentServiceServer::with_interceptor(
            AgentServiceImpl::new(self.client.clone()),
            wireguard_auth_interceptor,
        );
        let session = SessionServiceServer::with_interceptor(
            SessionServiceImpl::new(self.client.clone()),
            wireguard_auth_interceptor,
        );
        let task = TaskServiceServer::with_interceptor(
            TaskServiceImpl::new(self.client.clone()),
            wireguard_auth_interceptor,
        );
        let model = ModelServiceServer::with_interceptor(
            ModelServiceImpl::new(self.client.clone()),
            wireguard_auth_interceptor,
        );
        let cron = CronServiceServer::with_interceptor(
            CronServiceImpl::new(self.client.clone()),
            wireguard_auth_interceptor,
        );
        let soul = SoulServiceServer::with_interceptor(
            SoulServiceImpl::new(self.soul_store.clone()),
            wireguard_auth_interceptor,
        );
        let namespace = NamespaceMemoryServiceServer::with_interceptor(
            NamespaceMemoryServiceImpl::new(self.memory_store.clone(), self.soul_store.clone()),
            wireguard_auth_interceptor,
        );
        let memory = MemoryServiceServer::with_interceptor(
            MemoryServiceImpl::new(self.memory_store.clone()),
            wireguard_auth_interceptor,
        );

        let mut builder = Server::builder()
            .add_service(agent)
            .add_service(session)
            .add_service(task)
            .add_service(model)
            .add_service(cron)
            .add_service(soul)
            .add_service(namespace)
            .add_service(memory);

        if self.cfg.enable_reflection {
            let reflection = tonic_reflection::server::Builder::configure()
                .register_encoded_file_descriptor_set(crate::proto::FILE_DESCRIPTOR_SET)
                .build_v1()?;
            builder = builder.add_service(reflection);
        }

        let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
        health_reporter
            .set_serving::<AgentServiceServer<AgentServiceImpl>>()
            .await;
        builder = builder.add_service(health_service);

        builder.serve(addr).await?;
        Ok(())
    }
}

/// Convenience entry-point used by `op-dbus` and integration tests.
pub async fn run_grpc_server(cfg: ServerConfig) -> anyhow::Result<()> {
    AssistantGrpcServer::new(cfg).await?.serve().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_populate() {
        let cfg = ServerConfig::default();
        assert!(cfg.port > 0);
        assert!(!cfg.host.is_empty());
    }
}
</file>

<file path="src/sessions.rs">
//! SessionService implementation.

use crate::client::AssistantClient;
use crate::convert::*;
use crate::proto::session_service_server::SessionService;
use crate::proto::{
    CreateSessionRequest, DeleteSessionRequest, Empty, GetSessionHistoryRequest, GetSessionRequest,
    ListSessionsRequest, ListSessionsResponse, Message, SendMessageRequest, Session,
    SessionHistory,
};
use serde_json::{json, Value};
use tonic::{Request, Response, Status};

pub struct SessionServiceImpl {
    client: AssistantClient,
}

impl SessionServiceImpl {
    pub fn new(client: AssistantClient) -> Self {
        Self { client }
    }
}

#[tonic::async_trait]
impl SessionService for SessionServiceImpl {
    async fn list_sessions(
        &self,
        req: Request<ListSessionsRequest>,
    ) -> Result<Response<ListSessionsResponse>, Status> {
        let req = req.into_inner();
        let mut params = json!({
            "limit": req.pagination.as_ref().map(|p| p.limit).unwrap_or(0),
            "offset": req.pagination.as_ref().map(|p| p.offset).unwrap_or(0),
        });
        if let Some(a) = req.agent_id {
            params["agent_id"] = json!(a);
        }
        let result = self.client.call("sessions.list", params).await?;
        let sessions: Vec<Session> = result
            .get("sessions")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().map(session_from_json).collect())
            .unwrap_or_default();
        let total = result
            .get("total")
            .and_then(|t| t.as_u64())
            .unwrap_or(sessions.len() as u64) as u32;
        Ok(Response::new(ListSessionsResponse { sessions, total }))
    }

    async fn get_session(
        &self,
        req: Request<GetSessionRequest>,
    ) -> Result<Response<Session>, Status> {
        let id = req.into_inner().id;
        if id.is_empty() {
            return Err(Status::invalid_argument("session id required"));
        }
        let result = self
            .client
            .call("sessions.get", json!({ "id": id }))
            .await?;
        Ok(Response::new(session_from_json(&result)))
    }

    async fn create_session(
        &self,
        req: Request<CreateSessionRequest>,
    ) -> Result<Response<Session>, Status> {
        let req = req.into_inner();
        let mut params = json!({ "agent_id": req.agent_id });
        if let Some(t) = req.title {
            params["title"] = json!(t);
        }
        if let Some(m) = req.metadata {
            params["metadata"] = struct_to_json(m);
        }
        let result = self.client.call("sessions.create", params).await?;
        Ok(Response::new(session_from_json(&result)))
    }

    async fn delete_session(
        &self,
        req: Request<DeleteSessionRequest>,
    ) -> Result<Response<Empty>, Status> {
        let id = req.into_inner().id;
        if id.is_empty() {
            return Err(Status::invalid_argument("session id required"));
        }
        self.client
            .call("sessions.delete", json!({ "id": id }))
            .await?;
        Ok(Response::new(Empty {}))
    }

    async fn get_session_history(
        &self,
        req: Request<GetSessionHistoryRequest>,
    ) -> Result<Response<SessionHistory>, Status> {
        let req = req.into_inner();
        if req.session_id.is_empty() {
            return Err(Status::invalid_argument("session_id required"));
        }
        let params = json!({
            "session_id": req.session_id,
            "limit": req.pagination.as_ref().map(|p| p.limit).unwrap_or(0),
            "offset": req.pagination.as_ref().map(|p| p.offset).unwrap_or(0),
        });
        let result = self.client.call("sessions.history", params).await?;
        let messages = result
            .get("messages")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().map(message_from_json).collect())
            .unwrap_or_default();
        Ok(Response::new(SessionHistory {
            session_id: req.session_id,
            messages,
        }))
    }

    async fn send_message(
        &self,
        req: Request<SendMessageRequest>,
    ) -> Result<Response<Message>, Status> {
        let req = req.into_inner();
        if req.session_id.is_empty() {
            return Err(Status::invalid_argument("session_id required"));
        }
        let mut params = json!({
            "session_id": req.session_id,
            "content": req.content,
        });
        if let Some(r) = req.role {
            params["role"] = json!(r);
        }
        if let Some(m) = req.metadata {
            params["metadata"] = struct_to_json(m);
        }
        let result = self.client.call("sessions.send_message", params).await?;
        Ok(Response::new(message_from_json(&result)))
    }
}

pub(crate) fn session_from_json(v: &Value) -> Session {
    Session {
        id: str_field(v, "id"),
        agent_id: str_field(v, "agent_id"),
        title: str_field(v, "title"),
        created_at: ts_field(v, "created_at"),
        updated_at: ts_field(v, "updated_at"),
        metadata: opt_struct(v, "metadata"),
    }
}

pub(crate) fn message_from_json(v: &Value) -> Message {
    Message {
        id: str_field(v, "id"),
        session_id: str_field(v, "session_id"),
        role: str_field(v, "role"),
        content: str_field(v, "content"),
        timestamp: ts_field(v, "timestamp"),
        metadata: opt_struct(v, "metadata"),
    }
}
</file>

<file path="src/soul.rs">
//! SoulService — persistent agent identity, backed by `SoulMemoryStore` in
//! op-cognitive-mcp.

use crate::convert::*;
use crate::proto::soul_service_server::SoulService;
use crate::proto::{
    DeleteSoulMemoryRequest, Empty, GetSoulMemoryRequest, ListSoulMemoriesRequest,
    ListSoulMemoriesResponse, SoulMemory, UpdateSoulMemoryRequest,
};
use op_cognitive_mcp::soul_memory::{SoulMemory as StoreSoul, SoulMemoryStore, SoulUpdate};
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct SoulServiceImpl {
    store: Arc<SoulMemoryStore>,
}

impl SoulServiceImpl {
    pub fn new(store: Arc<SoulMemoryStore>) -> Self {
        Self { store }
    }
}

#[tonic::async_trait]
impl SoulService for SoulServiceImpl {
    async fn get_soul_memory(
        &self,
        req: Request<GetSoulMemoryRequest>,
    ) -> Result<Response<SoulMemory>, Status> {
        let id = req.into_inner().agent_id;
        if id.is_empty() {
            return Err(Status::invalid_argument("agent_id required"));
        }
        let soul = self
            .store
            .get_soul(&id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("soul memory not found"))?;
        Ok(Response::new(soul_to_proto(&soul)))
    }

    async fn update_soul_memory(
        &self,
        req: Request<UpdateSoulMemoryRequest>,
    ) -> Result<Response<SoulMemory>, Status> {
        let req = req.into_inner();
        if req.agent_id.is_empty() {
            return Err(Status::invalid_argument("agent_id required"));
        }
        let update = SoulUpdate {
            identity: req.identity,
            personality: req.personality,
            traits: req.traits.map(struct_to_json),
        };
        let soul = self
            .store
            .upsert_soul(&req.agent_id, update)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(soul_to_proto(&soul)))
    }

    async fn delete_soul_memory(
        &self,
        req: Request<DeleteSoulMemoryRequest>,
    ) -> Result<Response<Empty>, Status> {
        let id = req.into_inner().agent_id;
        if id.is_empty() {
            return Err(Status::invalid_argument("agent_id required"));
        }
        self.store
            .delete_soul(&id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(Empty {}))
    }

    async fn list_soul_memories(
        &self,
        req: Request<ListSoulMemoriesRequest>,
    ) -> Result<Response<ListSoulMemoriesResponse>, Status> {
        let req = req.into_inner();
        let limit = req
            .pagination
            .as_ref()
            .map(|p| p.limit as usize)
            .unwrap_or(0);
        let offset = req
            .pagination
            .as_ref()
            .map(|p| p.offset as usize)
            .unwrap_or(0);

        let souls = self
            .store
            .list_souls()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let total = souls.len() as u32;
        let page: Vec<SoulMemory> = souls
            .iter()
            .skip(offset)
            .take(if limit == 0 { usize::MAX } else { limit })
            .map(soul_to_proto)
            .collect();
        Ok(Response::new(ListSoulMemoriesResponse {
            memories: page,
            total,
        }))
    }
}

fn soul_to_proto(s: &StoreSoul) -> SoulMemory {
    SoulMemory {
        agent_id: s.agent_id.clone(),
        identity: s.identity.clone(),
        personality: s.personality.clone(),
        traits: Some(json_to_struct(s.traits.clone())),
        version: s.version as u64,
        created_at: Some(prost_types::Timestamp {
            seconds: s.created_at.timestamp(),
            nanos: s.created_at.timestamp_subsec_nanos() as i32,
        }),
        updated_at: Some(prost_types::Timestamp {
            seconds: s.updated_at.timestamp(),
            nanos: s.updated_at.timestamp_subsec_nanos() as i32,
        }),
    }
}
</file>

<file path="src/tasks.rs">
//! TaskService implementation.

use crate::client::AssistantClient;
use crate::convert::*;
use crate::proto::task_service_server::TaskService;
use crate::proto::{
    ExecuteTaskRequest, GetTaskResultRequest, ListToolsRequest, ListToolsResponse,
    StreamTaskExecutionRequest, TaskEvent, TaskResult, Tool,
};
use async_stream::try_stream;
use futures::Stream;
use serde_json::{json, Value};
use std::pin::Pin;
use tonic::{Request, Response, Status};

pub struct TaskServiceImpl {
    client: AssistantClient,
}

impl TaskServiceImpl {
    pub fn new(client: AssistantClient) -> Self {
        Self { client }
    }
}

type TaskEventStream = Pin<Box<dyn Stream<Item = Result<TaskEvent, Status>> + Send>>;

#[tonic::async_trait]
impl TaskService for TaskServiceImpl {
    type StreamTaskExecutionStream = TaskEventStream;

    async fn list_tools(
        &self,
        req: Request<ListToolsRequest>,
    ) -> Result<Response<ListToolsResponse>, Status> {
        let result = self
            .client
            .call("tools.list", json!({ "filter": req.into_inner().filter }))
            .await?;
        let tools = result
            .get("tools")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().map(tool_from_json).collect())
            .unwrap_or_default();
        Ok(Response::new(ListToolsResponse { tools }))
    }

    async fn execute_task(
        &self,
        req: Request<ExecuteTaskRequest>,
    ) -> Result<Response<TaskResult>, Status> {
        let req = req.into_inner();
        if req.tool_name.is_empty() {
            return Err(Status::invalid_argument("tool_name required"));
        }
        let mut params = json!({ "tool_name": req.tool_name });
        if let Some(a) = req.arguments {
            params["arguments"] = struct_to_json(a);
        }
        if let Some(s) = req.session_id {
            params["session_id"] = json!(s);
        }
        if let Some(a) = req.agent_id {
            params["agent_id"] = json!(a);
        }
        let result = self.client.call("tasks.execute", params).await?;
        Ok(Response::new(task_result_from_json(&result)))
    }

    async fn stream_task_execution(
        &self,
        req: Request<StreamTaskExecutionRequest>,
    ) -> Result<Response<Self::StreamTaskExecutionStream>, Status> {
        let task_id = req.into_inner().task_id;
        if task_id.is_empty() {
            return Err(Status::invalid_argument("task_id required"));
        }
        let client = self.client.clone();
        let stream = try_stream! {
            let result = client
                .call("tasks.events", json!({ "task_id": task_id }))
                .await
                .map_err(Status::from)?;
            let events = result.get("events").and_then(|v| v.as_array()).cloned().unwrap_or_default();
            for evt in events {
                yield task_event_from_json(&task_id, &evt);
            }
        };
        Ok(Response::new(Box::pin(stream)))
    }

    async fn get_task_result(
        &self,
        req: Request<GetTaskResultRequest>,
    ) -> Result<Response<TaskResult>, Status> {
        let task_id = req.into_inner().task_id;
        if task_id.is_empty() {
            return Err(Status::invalid_argument("task_id required"));
        }
        let result = self
            .client
            .call("tasks.get_result", json!({ "task_id": task_id }))
            .await?;
        Ok(Response::new(task_result_from_json(&result)))
    }
}

fn tool_from_json(v: &Value) -> Tool {
    Tool {
        name: str_field(v, "name"),
        description: str_field(v, "description"),
        version: str_field(v, "version"),
        schema: opt_struct(v, "schema"),
    }
}

fn task_result_from_json(v: &Value) -> TaskResult {
    TaskResult {
        task_id: str_field(v, "task_id"),
        status: str_field(v, "status"),
        output: opt_struct(v, "output"),
        error: opt_str(v, "error"),
        started_at: ts_field(v, "started_at"),
        finished_at: ts_field(v, "finished_at"),
    }
}

fn task_event_from_json(task_id: &str, v: &Value) -> TaskEvent {
    TaskEvent {
        task_id: task_id.to_string(),
        event_type: str_field(v, "event_type"),
        payload_json: v
            .get("payload")
            .map(|p| p.to_string())
            .unwrap_or_else(|| v.to_string()),
        timestamp: ts_field(v, "timestamp"),
    }
}
</file>

<file path="src/transport.rs">
//! D-Bus first transport layer with HTTP/JSON-RPC fallback.
//!
//! `Transport::new` attempts to acquire a session D-Bus connection. When the
//! connection succeeds the primary kind is D-Bus; otherwise the fallback HTTP
//! transport is used. Both kinds remain initialised so callers can request a
//! specific transport explicitly.

use crate::error::{AssistantError, Result};
use crate::incus::{
    SchemaTags, DEFAULT_WG_XRAY_ENDPOINT, ENV_RPC_ENDPOINT, HEADER_FOOTPRINT, HEADER_TRACE_ID,
};
use serde_json::Value;
use std::time::Duration;

pub const DEFAULT_DBUS_NAME: &str = "ai.assistant.v1";
pub const DEFAULT_DBUS_PATH: &str = "/ai/assistant";
/// Default RPC endpoint targets `op-grpc-bridge` inside the `wg-xray`
/// Incus container (`10.200.0.1:50051` on the `grpc-uplink` host bridge).
pub const DEFAULT_RPC_ENDPOINT: &str = DEFAULT_WG_XRAY_ENDPOINT;
pub const DEFAULT_HTTP_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Clone)]
pub struct TransportConfig {
    pub rpc_endpoint: String,
    pub dbus_name: String,
    pub dbus_path: String,
    pub http_timeout_secs: u64,
    pub force_kind: Option<TransportKind>,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            rpc_endpoint: std::env::var(ENV_RPC_ENDPOINT)
                .unwrap_or_else(|_| DEFAULT_RPC_ENDPOINT.to_string()),
            dbus_name: std::env::var("OP_ASSISTANT_DBUS_NAME")
                .unwrap_or_else(|_| DEFAULT_DBUS_NAME.to_string()),
            dbus_path: std::env::var("OP_ASSISTANT_DBUS_PATH")
                .unwrap_or_else(|_| DEFAULT_DBUS_PATH.to_string()),
            http_timeout_secs: std::env::var("OP_ASSISTANT_HTTP_TIMEOUT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_HTTP_TIMEOUT_SECS),
            force_kind: std::env::var("OP_ASSISTANT_TRANSPORT").ok().and_then(|v| {
                match v.to_lowercase().as_str() {
                    "dbus" => Some(TransportKind::DBus),
                    "rpc" | "http" => Some(TransportKind::Rpc),
                    _ => None,
                }
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportKind {
    DBus,
    Rpc,
}

pub struct Transport {
    cfg: TransportConfig,
    primary: TransportKind,
    dbus: Option<zbus::Connection>,
    http: reqwest::Client,
}

impl Transport {
    pub async fn new(cfg: TransportConfig) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(cfg.http_timeout_secs))
            .build()
            .map_err(AssistantError::Http)?;

        let dbus = match cfg.force_kind {
            Some(TransportKind::Rpc) => None,
            _ => zbus::Connection::session().await.ok(),
        };

        let primary = match (cfg.force_kind, dbus.is_some()) {
            (Some(TransportKind::DBus), false) => {
                return Err(AssistantError::Transport(
                    "OP_ASSISTANT_TRANSPORT=dbus but no session bus available".into(),
                ));
            }
            (Some(kind), _) => kind,
            (None, true) => TransportKind::DBus,
            (None, false) => TransportKind::Rpc,
        };

        Ok(Self {
            cfg,
            primary,
            dbus,
            http,
        })
    }

    pub fn primary_kind(&self) -> TransportKind {
        self.primary
    }

    pub fn config(&self) -> &TransportConfig {
        &self.cfg
    }

    pub fn http(&self) -> &reqwest::Client {
        &self.http
    }

    pub fn dbus(&self) -> Option<&zbus::Connection> {
        self.dbus.as_ref()
    }

    /// Dispatch a high-level Assistant call. The transport will try the
    /// primary route and transparently fall back to HTTP if the D-Bus call
    /// fails.
    pub async fn call(&self, method: &str, body: Value) -> Result<Value> {
        match self.primary {
            TransportKind::DBus => match self.dbus_call(method, &body).await {
                Ok(v) => Ok(v),
                Err(err) => {
                    tracing::warn!(?err, %method, "D-Bus call failed, falling back to RPC");
                    self.rpc_call(method, body).await
                }
            },
            TransportKind::Rpc => self.rpc_call(method, body).await,
        }
    }

    async fn dbus_call(&self, method: &str, body: &Value) -> Result<Value> {
        let conn = self
            .dbus
            .as_ref()
            .ok_or_else(|| AssistantError::Transport("dbus not initialised".into()))?;

        let payload = serde_json::to_string(body)?;
        let reply = conn
            .call_method(
                Some(self.cfg.dbus_name.as_str()),
                self.cfg.dbus_path.as_str(),
                Some(self.cfg.dbus_name.as_str()),
                method,
                &(payload,),
            )
            .await?;

        let response: String = reply.body().deserialize()?;
        let value: Value = serde_json::from_str(&response)?;
        Ok(value)
    }

    async fn rpc_call(&self, method: &str, body: Value) -> Result<Value> {
        let url = format!(
            "{}/rpc/{}",
            self.cfg.rpc_endpoint.trim_end_matches('/'),
            method
        );
        let tags = SchemaTags::load();
        let mut req = self.http.post(&url).json(&body);
        if tags.is_valid() {
            req = req
                .header(HEADER_FOOTPRINT, tags.footprint_hex)
                .header(HEADER_TRACE_ID, tags.trace_id);
        }
        let resp = req.send().await?;
        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(crate::error::map_http_status(status.as_u16(), text).into());
        }
        let value = resp.json::<Value>().await?;
        Ok(value)
    }
}

impl From<tonic::Status> for AssistantError {
    fn from(s: tonic::Status) -> Self {
        AssistantError::Internal(format!("grpc: {}", s.message()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_use_env_overrides() {
        // Just ensure default builder does not panic.
        let cfg = TransportConfig::default();
        assert!(!cfg.rpc_endpoint.is_empty());
        assert!(!cfg.dbus_name.is_empty());
        assert!(!cfg.dbus_path.is_empty());
    }

    #[tokio::test]
    async fn transport_initialises_with_rpc_fallback() {
        let cfg = TransportConfig {
            force_kind: Some(TransportKind::Rpc),
            ..Default::default()
        };
        let t = Transport::new(cfg).await.expect("init");
        assert_eq!(t.primary_kind(), TransportKind::Rpc);
    }
}
</file>

<file path="tests/integration.rs">
//! Integration tests for op-assistant-grpc.
//!
//! These hit the in-process CognitiveMemoryStore + SoulMemoryStore directly
//! through the service implementations; no live Assistant gateway required.

use op_assistant_grpc::memory::{ensure_namespace, MemoryServiceImpl};
use op_assistant_grpc::namespace::NamespaceMemoryServiceImpl;
use op_assistant_grpc::proto::memory_service_server::MemoryService;
use op_assistant_grpc::proto::namespace_memory_service_server::NamespaceMemoryService;
use op_assistant_grpc::proto::soul_service_server::SoulService;
use op_assistant_grpc::proto::{
    GetSoulMemoryRequest, MemoryEntry, ReadMemoryRequest, SetMemoryNamespaceRequest,
    UpdateSoulMemoryRequest, WriteMemoryRequest,
};
use op_assistant_grpc::soul::SoulServiceImpl;
use op_cognitive_mcp::cozo_shuttle::CozoGraphShuttle;
use op_cognitive_mcp::memory_store::{CognitiveMemoryStore, NamespaceKind};
use op_cognitive_mcp::soul_memory::SoulMemoryStore;
use std::sync::Arc;
use tonic::Request;

async fn fixture() -> (Arc<CognitiveMemoryStore>, Arc<SoulMemoryStore>) {
    let shuttle = Arc::new(CozoGraphShuttle::new_in_memory().expect("cozo in-memory"));
    let memory = Arc::new(
        CognitiveMemoryStore::new(shuttle.clone())
            .await
            .expect("memory store"),
    );
    let soul = Arc::new(SoulMemoryStore::new(shuttle));
    (memory, soul)
}

#[tokio::test]
async fn write_then_read_memory_entry() {
    let (memory, _soul) = fixture().await;
    let svc = MemoryServiceImpl::new(memory.clone());

    ensure_namespace(&memory, "demo", NamespaceKind::Custom)
        .await
        .unwrap();

    let entry = MemoryEntry {
        id: String::new(),
        namespace: "demo".into(),
        key: "k1".into(),
        value: "\"hello\"".into(),
        metadata: None,
        created_at: None,
        updated_at: None,
    };
    let wr = svc
        .write_memory(Request::new(WriteMemoryRequest {
            namespace: "demo".into(),
            entries: vec![entry],
        }))
        .await
        .unwrap();
    assert_eq!(wr.into_inner().written, 1);

    let rd = svc
        .read_memory(Request::new(ReadMemoryRequest {
            namespace: "demo".into(),
            keys: vec!["k1".into()],
            pagination: None,
        }))
        .await
        .unwrap();
    let entries = rd.into_inner().entries;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].key, "k1");
}

#[tokio::test]
async fn soul_upsert_and_get() {
    let (_memory, soul) = fixture().await;
    let svc = SoulServiceImpl::new(soul.clone());

    let updated = svc
        .update_soul_memory(Request::new(UpdateSoulMemoryRequest {
            agent_id: "agent-a".into(),
            identity: Some("identity-1".into()),
            personality: Some("calm".into()),
            traits: None,
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(updated.identity, "identity-1");
    assert_eq!(updated.version, 1);

    let got = svc
        .get_soul_memory(Request::new(GetSoulMemoryRequest {
            agent_id: "agent-a".into(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(got.personality, "calm");

    // Second update bumps version.
    let bumped = svc
        .update_soul_memory(Request::new(UpdateSoulMemoryRequest {
            agent_id: "agent-a".into(),
            identity: None,
            personality: Some("focused".into()),
            traits: None,
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(bumped.version, 2);
    assert_eq!(bumped.identity, "identity-1"); // preserved
}

#[tokio::test]
async fn namespace_binding_round_trip() {
    let (memory, soul) = fixture().await;
    let svc = NamespaceMemoryServiceImpl::new(memory.clone(), soul.clone());

    let bound = svc
        .set_memory_namespace(Request::new(SetMemoryNamespaceRequest {
            agent_id: "agent-b".into(),
            namespace: "ns-b".into(),
        }))
        .await
        .unwrap()
        .into_inner();
    assert_eq!(bound.namespace, "ns-b");
    assert_eq!(bound.agent_id, "agent-b");
}
</file>

<file path="build.rs">
use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    let protos = [
        "proto/assistant/common.proto",
        "proto/assistant/agent.proto",
        "proto/assistant/session.proto",
        "proto/assistant/task.proto",
        "proto/assistant/model.proto",
        "proto/assistant/cron.proto",
        "proto/assistant/soul.proto",
        "proto/assistant/namespace.proto",
        "proto/assistant/memory.proto",
    ];

    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(out_dir.join("assistant_descriptor.bin"))
        .compile_protos(&protos, &["proto"])?;

    for p in &protos {
        println!("cargo:rerun-if-changed={}", p);
    }
    println!("cargo:rerun-if-changed=build.rs");

    Ok(())
}
</file>

<file path="Cargo.toml">
[package]
name = "op-assistant-grpc"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "gRPC gateway for Assistant integration with D-Bus first transport and RPC fallback"

[dependencies]
# Cognitive memory (CozoDB-backed)
op-cognitive-mcp = { path = "../op-cognitive-mcp" }

# gRPC
tonic = { workspace = true }
tonic-web = { workspace = true }
tonic-reflection = { workspace = true }
tonic-health = { workspace = true }
prost = { workspace = true }
prost-types = { workspace = true }

# Async
tokio = { workspace = true, features = ["full", "sync"] }
tokio-stream = { workspace = true }
async-trait = { workspace = true }
futures = { workspace = true }
async-stream = "0.3"

# D-Bus
zbus = { workspace = true }

# HTTP / RPC fallback
reqwest = { workspace = true }

# Serialization
serde = { workspace = true }
serde_json = { workspace = true }

# Errors / logging
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }

# Utilities
uuid = { workspace = true }
chrono = { workspace = true }
hex = { workspace = true }

[build-dependencies]
tonic-build = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
tokio-test = "0.4"

[[bin]]
name = "op-assistant-grpc"
path = "src/bin/op-assistant-grpc.rs"
</file>

</files>
