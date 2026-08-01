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
  services.proto
src/
  bin/
    op-services.rs
    systemctl-native.rs
    systemctl.rs
  dbus/
    interface.rs
    mod.rs
  grpc/
    mod.rs
    server.rs
  manager/
    mod.rs
    process.rs
    service_manager.rs
  schema/
    mod.rs
  store/
    mod.rs
  lib.rs
build.rs
Cargo.toml
compare-op-services.md
SPEC.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="proto/services.proto">
syntax = "proto3";
package opdbus.services.v1;

import "google/protobuf/timestamp.proto";
import "google/protobuf/duration.proto";

service ServiceManager {
    rpc Start(StartRequest) returns (StartResponse);
    rpc Stop(StopRequest) returns (StopResponse);
    rpc Restart(RestartRequest) returns (RestartResponse);
    rpc Reload(ReloadRequest) returns (ReloadResponse);
    
    rpc Create(CreateRequest) returns (CreateResponse);
    rpc Delete(DeleteRequest) returns (DeleteResponse);
    rpc Get(GetRequest) returns (GetResponse);
    rpc List(ListRequest) returns (ListResponse);
    
    rpc Enable(EnableRequest) returns (EnableResponse);
    rpc Disable(DisableRequest) returns (DisableResponse);
    
    rpc WatchStatus(WatchRequest) returns (stream ServiceEvent);
}

message ServiceDef {
    string name = 1;
    ServiceType type = 2;
    ExecConfig exec = 3;
    repeated string depends_on = 4;
    RestartPolicy restart = 5;
    map<string, string> environment = 6;
    optional ResourceLimits resources = 7;
    optional HealthCheck health_check = 8;
    bool enabled = 9;
}

enum ServiceType {
    SERVICE_TYPE_SIMPLE = 0;
    SERVICE_TYPE_FORKING = 1;
    SERVICE_TYPE_ONESHOT = 2;
    SERVICE_TYPE_NOTIFY = 3;
}

message ExecConfig {
    string start_program = 1;
    repeated string start_args = 2;
    optional string stop_program = 3;
    repeated string stop_args = 4;
    optional string working_dir = 5;
    optional string user = 6;
    optional string group = 7;
}

message RestartPolicy {
    RestartCondition condition = 1;
    google.protobuf.Duration delay = 2;
    optional uint32 max_retries = 3;
}

enum RestartCondition {
    RESTART_NEVER = 0;
    RESTART_ALWAYS = 1;
    RESTART_ON_FAILURE = 2;
}

message ResourceLimits {
    optional uint64 memory_max = 1;
    optional float cpu_quota = 2;
    optional uint32 tasks_max = 3;
}

message HealthCheck {
    string program = 1;
    repeated string args = 2;
    google.protobuf.Duration interval = 3;
    google.protobuf.Duration timeout = 4;
    uint32 retries = 5;
}

message ServiceStatus {
    string name = 1;
    ServiceState state = 2;
    optional uint32 pid = 3;
    optional string error = 4;
    optional google.protobuf.Timestamp started_at = 5;
}

enum ServiceState {
    STATE_STOPPED = 0;
    STATE_STARTING = 1;
    STATE_RUNNING = 2;
    STATE_STOPPING = 3;
    STATE_FAILED = 4;
}

message ServiceEvent {
    string name = 1;
    ServiceState old_state = 2;
    ServiceState new_state = 3;
    google.protobuf.Timestamp timestamp = 4;
}

// Request/Response messages
message StartRequest { string name = 1; }
message StartResponse { ServiceStatus status = 1; }

message StopRequest { string name = 1; }
message StopResponse { ServiceStatus status = 1; }

message RestartRequest { string name = 1; }
message RestartResponse { ServiceStatus status = 1; }

message ReloadRequest { string name = 1; }
message ReloadResponse { ServiceStatus status = 1; }

message CreateRequest { ServiceDef service = 1; }
message CreateResponse { ServiceDef service = 1; }

message DeleteRequest { string name = 1; }
message DeleteResponse {}

message GetRequest { string name = 1; }
message GetResponse { ServiceDef service = 1; ServiceStatus status = 2; }

message ListRequest { optional string filter = 1; }
message ListResponse { repeated ServiceDef services = 1; }

message EnableRequest { string name = 1; }
message EnableResponse {}

message DisableRequest { string name = 1; }
message DisableResponse {}

message WatchRequest { repeated string names = 1; }
</file>

<file path="src/bin/op-services.rs">
//! op-services daemon

use std::sync::Arc;
use tonic::transport::Server;
use tracing::info;
use tracing_subscriber::EnvFilter;

use op_services::dbus::interface::run_dbus_server;
use op_services::grpc::proto::service_manager_server::ServiceManagerServer;
use op_services::grpc::server::GrpcServer;
use op_services::manager::ServiceManager;
use op_services::store::Store;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("op_services=info".parse()?))
        .init();

    info!("Starting op-services daemon");

    // Initialize store — JSON flat file, no SQLite, no drift.
    let store = Arc::new(Store::default_store().await?);

    // Initialize service manager
    let manager = Arc::new(ServiceManager::new(store).await?);

    // Start D-Bus interface in background
    let dbus_manager = manager.clone();
    tokio::spawn(async move {
        if let Err(e) = run_dbus_server(dbus_manager).await {
            tracing::error!("D-Bus server error: {}", e);
        }
    });

    // Start gRPC server
    let grpc_server = GrpcServer::new(manager);
    let addr = std::env::var("OP_SERVICES_GRPC_ADDR")
        .unwrap_or_else(|_| "[::]:50053".to_string())
        .parse()?;

    info!("gRPC server listening on {}", addr);

    Server::builder()
        .add_service(ServiceManagerServer::new(grpc_server))
        .serve(addr)
        .await?;

    Ok(())
}
</file>

<file path="src/bin/systemctl-native.rs">
//! Native systemctl - D-Bus client (no network dependency)

use std::env;
use zbus::Connection;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    let conn = Connection::system().await?;
    let proxy = zbus::Proxy::new(
        &conn,
        "org.opdbus.services",
        "/org/opdbus/services",
        "org.opdbus.services.v1.Manager",
    )
    .await?;

    match args[1].as_str() {
        "start" => {
            let name = args
                .get(2)
                .ok_or_else(|| anyhow::anyhow!("missing service name"))?;
            let result: String = proxy.call("Start", &(name.as_str(),)).await?;
            println!("Started {}", name);
        }
        "stop" => {
            let name = args
                .get(2)
                .ok_or_else(|| anyhow::anyhow!("missing service name"))?;
            let _: String = proxy.call("Stop", &(name.as_str(),)).await?;
            println!("Stopped {}", name);
        }
        "restart" => {
            let name = args
                .get(2)
                .ok_or_else(|| anyhow::anyhow!("missing service name"))?;
            let _: String = proxy.call("Restart", &(name.as_str(),)).await?;
            println!("Restarted {}", name);
        }
        "status" => {
            let name = args
                .get(2)
                .ok_or_else(|| anyhow::anyhow!("missing service name"))?;
            let result: String = proxy.call("GetStatus", &(name.as_str(),)).await?;
            println!("● {}", name);
            println!("{}", result);
        }
        "list-units" | "list" => {
            let services: Vec<String> = proxy.call("ListServices", &()).await?;
            for svc in services {
                println!("{}", svc);
            }
        }
        _ => print_usage(),
    }

    Ok(())
}

fn print_usage() {
    eprintln!("Usage: systemctl <command> [service]");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  start <service>    Start a service");
    eprintln!("  stop <service>     Stop a service");
    eprintln!("  restart <service>  Restart a service");
    eprintln!("  status <service>   Show service status");
    eprintln!("  list-units         List all services");
}
</file>

<file path="src/bin/systemctl.rs">
//! systemctl compatibility wrapper

use std::env;
use tonic::transport::Channel;

use op_services::grpc::proto::service_manager_client::ServiceManagerClient;
use op_services::grpc::proto::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        return Ok(());
    }

    let mut client = ServiceManagerClient::connect("http://[::1]:50053").await?;

    match args[1].as_str() {
        "start" => {
            let name = args
                .get(2)
                .ok_or_else(|| anyhow::anyhow!("missing service name"))?;
            let resp = client.start(StartRequest { name: name.clone() }).await?;
            println!("Started {}", name);
            if let Some(status) = resp.into_inner().status {
                println!(
                    "State: {:?}",
                    ServiceState::try_from(status.state).unwrap_or(ServiceState::StateStopped)
                );
            }
        }
        "stop" => {
            let name = args
                .get(2)
                .ok_or_else(|| anyhow::anyhow!("missing service name"))?;
            let resp = client.stop(StopRequest { name: name.clone() }).await?;
            println!("Stopped {}", name);
        }
        "restart" => {
            let name = args
                .get(2)
                .ok_or_else(|| anyhow::anyhow!("missing service name"))?;
            client
                .restart(RestartRequest { name: name.clone() })
                .await?;
            println!("Restarted {}", name);
        }
        "status" => {
            let name = args
                .get(2)
                .ok_or_else(|| anyhow::anyhow!("missing service name"))?;
            let resp = client.get(GetRequest { name: name.clone() }).await?;
            if let Some(status) = resp.into_inner().status {
                let state =
                    ServiceState::try_from(status.state).unwrap_or(ServiceState::StateStopped);
                println!("● {} - {:?}", name, state);
                if let Some(pid) = status.pid {
                    println!("  PID: {}", pid);
                }
                if let Some(err) = status.error {
                    println!("  Error: {}", err);
                }
            }
        }
        "list-units" | "list" => {
            let resp = client.list(ListRequest { filter: None }).await?;
            for svc in resp.into_inner().services {
                println!("{}", svc.name);
            }
        }
        _ => print_usage(),
    }

    Ok(())
}

fn print_usage() {
    eprintln!("Usage: systemctl <command> [service]");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  start <service>    Start a service");
    eprintln!("  stop <service>     Stop a service");
    eprintln!("  restart <service>  Restart a service");
    eprintln!("  status <service>   Show service status");
    eprintln!("  list-units         List all services");
}
</file>

<file path="src/dbus/interface.rs">
//! D-Bus interface for org.opdbus.services

use std::sync::Arc;
use tracing::info;
use zbus::{interface, object_server::SignalEmitter as SignalContext, Connection};

use crate::manager::ServiceManager;
use crate::schema::ServiceName;

pub struct DbusInterface {
    manager: Arc<ServiceManager>,
}

impl DbusInterface {
    pub fn new(manager: Arc<ServiceManager>) -> Self {
        Self { manager }
    }
}

#[interface(name = "org.opdbus.services.v1.Manager")]
impl DbusInterface {
    async fn start(&self, name: &str) -> zbus::fdo::Result<String> {
        let name =
            ServiceName::new(name).map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;

        let status = self
            .manager
            .start(&name)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        Ok(serde_json::to_string(&status).unwrap_or_default())
    }

    async fn stop(&self, name: &str) -> zbus::fdo::Result<String> {
        let name =
            ServiceName::new(name).map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;

        let status = self
            .manager
            .stop(&name)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        Ok(serde_json::to_string(&status).unwrap_or_default())
    }

    async fn restart(&self, name: &str) -> zbus::fdo::Result<String> {
        let name =
            ServiceName::new(name).map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;

        let status = self
            .manager
            .restart(&name)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        Ok(serde_json::to_string(&status).unwrap_or_default())
    }

    async fn get_status(&self, name: &str) -> zbus::fdo::Result<String> {
        let name =
            ServiceName::new(name).map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;

        let status = self
            .manager
            .get_status(&name)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        Ok(serde_json::to_string(&status).unwrap_or_default())
    }

    async fn list_services(&self) -> zbus::fdo::Result<Vec<String>> {
        let services = self
            .manager
            .list()
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        Ok(services.into_iter().map(|s| s.name.to_string()).collect())
    }

    #[zbus(signal)]
    async fn service_state_changed(
        ctx: &SignalContext<'_>,
        name: &str,
        old_state: &str,
        new_state: &str,
    ) -> zbus::Result<()>;
}

pub async fn run_dbus_server(manager: Arc<ServiceManager>) -> anyhow::Result<()> {
    let conn = Connection::system().await?;

    let iface = DbusInterface::new(manager);
    conn.object_server()
        .at("/org/opdbus/services", iface)
        .await?;
    conn.request_name("org.opdbus.services").await?;

    info!("D-Bus interface started on org.opdbus.services");

    // Keep running
    std::future::pending::<()>().await;
    Ok(())
}
</file>

<file path="src/dbus/mod.rs">
//! D-Bus interface

pub mod interface;
</file>

<file path="src/grpc/mod.rs">
//! gRPC server

pub mod server;

pub mod proto {
    tonic::include_proto!("opdbus.services.v1");
}
</file>

<file path="src/grpc/server.rs">
//! gRPC server implementation

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use super::proto::service_manager_server::ServiceManager as ServiceManagerTrait;
use super::proto::*;
use crate::manager::ServiceManager;
use crate::schema::{self, ServiceName};

pub struct GrpcServer {
    manager: Arc<ServiceManager>,
}

impl GrpcServer {
    pub fn new(manager: Arc<ServiceManager>) -> Self {
        Self { manager }
    }
}

#[tonic::async_trait]
impl ServiceManagerTrait for GrpcServer {
    async fn start(&self, req: Request<StartRequest>) -> Result<Response<StartResponse>, Status> {
        let name = ServiceName::new(&req.get_ref().name)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let status = self
            .manager
            .start(&name)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(StartResponse {
            status: Some(status.into()),
        }))
    }

    async fn stop(&self, req: Request<StopRequest>) -> Result<Response<StopResponse>, Status> {
        let name = ServiceName::new(&req.get_ref().name)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let status = self
            .manager
            .stop(&name)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(StopResponse {
            status: Some(status.into()),
        }))
    }

    async fn restart(
        &self,
        req: Request<RestartRequest>,
    ) -> Result<Response<RestartResponse>, Status> {
        let name = ServiceName::new(&req.get_ref().name)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let status = self
            .manager
            .restart(&name)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(RestartResponse {
            status: Some(status.into()),
        }))
    }

    async fn reload(
        &self,
        req: Request<ReloadRequest>,
    ) -> Result<Response<ReloadResponse>, Status> {
        let name = ServiceName::new(&req.get_ref().name)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        // Reload by performing a stop + start cycle, since neither s6-rc
        // nor the process manager exposes a dedicated reload operation.
        let status = self
            .manager
            .restart(&name)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(ReloadResponse {
            status: Some(status.into()),
        }))
    }

    async fn create(
        &self,
        req: Request<CreateRequest>,
    ) -> Result<Response<CreateResponse>, Status> {
        let proto_def = req
            .get_ref()
            .service
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing service definition"))?;

        let service_def =
            proto_to_schema_def(proto_def).map_err(|e| Status::invalid_argument(e.to_string()))?;

        // Persist the service definition in the store
        self.manager
            .create(&service_def)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(CreateResponse {
            service: Some(service_def.into()),
        }))
    }

    async fn delete(
        &self,
        req: Request<DeleteRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        let name = ServiceName::new(&req.get_ref().name)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.manager
            .delete(&name)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(DeleteResponse {}))
    }

    async fn get(&self, req: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        let name = ServiceName::new(&req.get_ref().name)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        let service_def = self
            .manager
            .get(&name)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found(format!("service not found: {}", name)))?;

        let status = self
            .manager
            .get_status(&name)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(GetResponse {
            service: Some(service_def.into()),
            status: Some(status.into()),
        }))
    }

    async fn list(&self, _req: Request<ListRequest>) -> Result<Response<ListResponse>, Status> {
        let services = self
            .manager
            .list()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(ListResponse {
            services: services.into_iter().map(Into::into).collect(),
        }))
    }

    async fn enable(
        &self,
        req: Request<EnableRequest>,
    ) -> Result<Response<EnableResponse>, Status> {
        let name = ServiceName::new(&req.get_ref().name)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.manager
            .set_enabled(&name, true)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(EnableResponse {}))
    }

    async fn disable(
        &self,
        req: Request<DisableRequest>,
    ) -> Result<Response<DisableResponse>, Status> {
        let name = ServiceName::new(&req.get_ref().name)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;

        self.manager
            .set_enabled(&name, false)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(DisableResponse {}))
    }

    type WatchStatusStream = ReceiverStream<Result<ServiceEvent, Status>>;

    async fn watch_status(
        &self,
        _req: Request<WatchRequest>,
    ) -> Result<Response<Self::WatchStatusStream>, Status> {
        let (tx, rx) = mpsc::channel(128);
        let mut sub = self.manager.subscribe();

        tokio::spawn(async move {
            while let Ok(event) = sub.recv().await {
                if tx.send(Ok(event.into())).await.is_err() {
                    break;
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}

/// Convert a proto ServiceDef into the internal schema ServiceDef.
fn proto_to_schema_def(proto: &ServiceDef) -> anyhow::Result<schema::ServiceDef> {
    let name = ServiceName::new(&proto.name)?;

    let exec = proto
        .exec
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("missing exec config"))?;

    let exec_start =
        schema::ExecCommand::new(PathBuf::from(&exec.start_program), exec.start_args.clone())?;

    let exec_stop = match &exec.stop_program {
        Some(prog) if !prog.is_empty() => Some(schema::ExecCommand::new(
            PathBuf::from(prog),
            exec.stop_args.clone(),
        )?),
        _ => None,
    };

    let working_dir = exec.working_dir.as_deref().map(PathBuf::from);
    let user = exec.user.clone();
    let group = exec.group.clone();

    let service_type = match proto.r#type {
        t if t == ServiceType::Simple as i32 => schema::ServiceType::Simple,
        t if t == ServiceType::Forking as i32 => schema::ServiceType::Forking { pid_file: None },
        t if t == ServiceType::Oneshot as i32 => schema::ServiceType::Oneshot,
        t if t == ServiceType::Notify as i32 => schema::ServiceType::Notify,
        _ => schema::ServiceType::Simple,
    };

    let restart = match &proto.restart {
        Some(rp) => {
            let condition = match rp.condition {
                c if c == RestartCondition::RestartAlways as i32 => {
                    schema::RestartCondition::Always
                }
                c if c == RestartCondition::RestartOnFailure as i32 => {
                    schema::RestartCondition::OnFailure
                }
                _ => schema::RestartCondition::Never,
            };
            let delay_secs = rp.delay.as_ref().map(|d| d.seconds as u64).unwrap_or(1);
            schema::RestartPolicy {
                condition,
                delay_secs,
                max_retries: rp.max_retries,
            }
        }
        None => schema::RestartPolicy::default(),
    };

    let depends_on: Vec<ServiceName> = proto
        .depends_on
        .iter()
        .map(|n| ServiceName::new(n))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(schema::ServiceDef {
        name,
        service_type,
        exec_start,
        exec_stop,
        working_dir,
        user,
        group,
        depends_on,
        waits_for: Vec::new(),
        restart,
        environment: proto.environment.clone(),
        env_file: None,
        resources: None,
        log_type: schema::LogType::default(),
        ready_notification: schema::ReadyNotification::default(),
        chain_to: None,
        smooth_recovery: false,
        enabled: proto.enabled,
    })
}

// Conversions
impl From<crate::schema::ServiceStatus> for ServiceStatus {
    fn from(s: crate::schema::ServiceStatus) -> Self {
        Self {
            name: s.name.to_string(),
            state: match s.state {
                crate::schema::ManagerState::Stopped => ServiceState::StateStopped as i32,
                crate::schema::ManagerState::Starting => ServiceState::StateStarting as i32,
                crate::schema::ManagerState::Running => ServiceState::StateRunning as i32,
                crate::schema::ManagerState::Stopping => ServiceState::StateStopping as i32,
                crate::schema::ManagerState::Failed => ServiceState::StateFailed as i32,
            },
            pid: s.pid,
            error: s.error,
            started_at: s.started_at.map(|t| prost_types::Timestamp {
                seconds: t.timestamp(),
                nanos: t.timestamp_subsec_nanos() as i32,
            }),
        }
    }
}

impl From<crate::schema::ServiceDef> for ServiceDef {
    fn from(s: crate::schema::ServiceDef) -> Self {
        Self {
            name: s.name.to_string(),
            r#type: match s.service_type {
                crate::schema::ServiceType::Simple => ServiceType::Simple as i32,
                crate::schema::ServiceType::Forking { .. } => ServiceType::Forking as i32,
                crate::schema::ServiceType::Oneshot => ServiceType::Oneshot as i32,
                crate::schema::ServiceType::Notify => ServiceType::Notify as i32,
            },
            exec: Some(ExecConfig {
                start_program: s.exec_start.program.to_string_lossy().to_string(),
                start_args: s.exec_start.args,
                stop_program: s
                    .exec_stop
                    .as_ref()
                    .map(|c| c.program.to_string_lossy().to_string()),
                stop_args: s.exec_stop.map(|c| c.args).unwrap_or_default(),
                working_dir: s.working_dir.map(|p| p.to_string_lossy().to_string()),
                user: s.user,
                group: s.group,
            }),
            depends_on: s.depends_on.into_iter().map(|n| n.to_string()).collect(),
            restart: Some(RestartPolicy {
                condition: match s.restart.condition {
                    crate::schema::RestartCondition::Never => RestartCondition::RestartNever as i32,
                    crate::schema::RestartCondition::Always => {
                        RestartCondition::RestartAlways as i32
                    }
                    crate::schema::RestartCondition::OnFailure => {
                        RestartCondition::RestartOnFailure as i32
                    }
                },
                delay: Some(prost_types::Duration {
                    seconds: s.restart.delay_secs as i64,
                    nanos: 0,
                }),
                max_retries: s.restart.max_retries,
            }),
            environment: s.environment,
            resources: None,
            health_check: None,
            enabled: s.enabled,
        }
    }
}

impl From<crate::manager::ServiceEvent> for ServiceEvent {
    fn from(e: crate::manager::ServiceEvent) -> Self {
        Self {
            name: e.name.to_string(),
            old_state: match e.old_state {
                crate::schema::ManagerState::Stopped => ServiceState::StateStopped as i32,
                crate::schema::ManagerState::Starting => ServiceState::StateStarting as i32,
                crate::schema::ManagerState::Running => ServiceState::StateRunning as i32,
                crate::schema::ManagerState::Stopping => ServiceState::StateStopping as i32,
                crate::schema::ManagerState::Failed => ServiceState::StateFailed as i32,
            },
            new_state: match e.new_state {
                crate::schema::ManagerState::Stopped => ServiceState::StateStopped as i32,
                crate::schema::ManagerState::Starting => ServiceState::StateStarting as i32,
                crate::schema::ManagerState::Running => ServiceState::StateRunning as i32,
                crate::schema::ManagerState::Stopping => ServiceState::StateStopping as i32,
                crate::schema::ManagerState::Failed => ServiceState::StateFailed as i32,
            },
            timestamp: Some(prost_types::Timestamp {
                seconds: chrono::Utc::now().timestamp(),
                nanos: 0,
            }),
        }
    }
}
</file>

<file path="src/manager/mod.rs">
//! Service manager core

mod process;
mod service_manager;

pub use process::*;
pub use service_manager::*;
</file>

<file path="src/manager/process.rs">
//! Direct process management fallback

use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use std::collections::HashMap;
use std::process::Stdio;
use tokio::process::Command as TokioCommand;
use tokio::sync::RwLock;
use tracing::{error, info};

use crate::schema::{ServiceDef, ServiceName};

pub struct ProcessManager {
    processes: RwLock<HashMap<ServiceName, u32>>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            processes: RwLock::new(HashMap::new()),
        }
    }

    pub async fn start(&self, service: &ServiceDef) -> anyhow::Result<u32> {
        let mut cmd = TokioCommand::new(&service.exec_start.program);
        cmd.args(&service.exec_start.args);
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());

        if let Some(ref dir) = service.working_dir {
            cmd.current_dir(dir);
        }

        for (k, v) in &service.environment {
            cmd.env(k, v);
        }

        let child = cmd.spawn()?;
        let pid = child.id().unwrap_or(0);

        info!("Started {} with PID {}", service.name, pid);

        let mut procs = self.processes.write().await;
        procs.insert(service.name.clone(), pid);

        Ok(pid)
    }

    pub async fn stop(&self, name: &ServiceName) -> anyhow::Result<()> {
        let mut procs = self.processes.write().await;

        if let Some(pid) = procs.remove(name) {
            info!("Stopping {} (PID {})", name, pid);
            if let Err(e) = kill(Pid::from_raw(pid as i32), Signal::SIGTERM) {
                error!("Failed to send SIGTERM to {}: {}", pid, e);
            }
        }

        Ok(())
    }

    pub async fn get_pid(&self, name: &ServiceName) -> Option<u32> {
        let procs = self.processes.read().await;
        procs.get(name).copied()
    }
}
</file>

<file path="src/manager/service_manager.rs">
//! Core service manager — uses s6-rc CLI for service control on Artix Linux.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{info, warn};

use super::ProcessManager;
use crate::schema::{ManagerState, ServiceDef, ServiceName, ServiceStatus};
use crate::store::Store;

/// Path to the s6-rc live database.
const S6_RC_LIVE: &str = "/run/s6-rc";

/// Run `s6-rc -l /run/s6-rc <args…>` and return the raw output.
async fn s6rc(args: &[&str]) -> anyhow::Result<std::process::Output> {
    tokio::process::Command::new("s6-rc")
        .arg("-l")
        .arg(S6_RC_LIVE)
        .args(args)
        .output()
        .await
        .map_err(|e| anyhow::anyhow!("failed to run s6-rc: {e}"))
}

pub struct ServiceManager {
    store: Arc<Store>,
    /// True when the s6-rc live directory was present at construction time.
    s6_available: bool,
    process_mgr: ProcessManager,
    statuses: Arc<RwLock<HashMap<ServiceName, ServiceStatus>>>,
    events: broadcast::Sender<ServiceEvent>,
}

#[derive(Debug, Clone)]
pub struct ServiceEvent {
    pub name: ServiceName,
    pub old_state: ManagerState,
    pub new_state: ManagerState,
}

impl ServiceManager {
    pub async fn new(store: Arc<Store>) -> anyhow::Result<Self> {
        let s6_available = std::path::Path::new(S6_RC_LIVE).exists();
        if s6_available {
            info!("s6-rc live directory found at {S6_RC_LIVE}");
        } else {
            warn!("s6-rc live directory not found at {S6_RC_LIVE}, using process fallback");
        }

        let (events, _) = broadcast::channel(256);

        Ok(Self {
            store,
            s6_available,
            process_mgr: ProcessManager::new(),
            statuses: Arc::new(RwLock::new(HashMap::new())),
            events,
        })
    }

    pub async fn start(&self, name: &ServiceName) -> anyhow::Result<ServiceStatus> {
        let service = self
            .store
            .get_service(name)
            .await?
            .ok_or_else(|| anyhow::anyhow!("service not found: {}", name))?;

        self.set_state(name, ManagerState::Starting).await;

        let result: anyhow::Result<u32> = if self.s6_available {
            let out = s6rc(&["start", name.as_str()]).await?;
            if out.status.success() {
                Ok(0) // s6 doesn't hand us a PID directly
            } else {
                let stderr = String::from_utf8_lossy(&out.stderr);
                if stderr.contains("already") {
                    Ok(0)
                } else {
                    Err(anyhow::anyhow!("s6-rc start {} failed: {}", name, stderr))
                }
            }
        } else {
            self.process_mgr.start(&service).await
        };

        match result {
            Ok(pid) => {
                self.set_state_with_pid(name, ManagerState::Running, pid)
                    .await;
            }
            Err(e) => {
                self.set_state_with_error(name, ManagerState::Failed, e.to_string())
                    .await;
            }
        }

        self.get_status(name).await
    }

    pub async fn stop(&self, name: &ServiceName) -> anyhow::Result<ServiceStatus> {
        self.set_state(name, ManagerState::Stopping).await;

        let result: anyhow::Result<()> = if self.s6_available {
            let out = s6rc(&["stop", name.as_str()]).await?;
            if out.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&out.stderr);
                if stderr.contains("already") {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!("s6-rc stop {} failed: {}", name, stderr))
                }
            }
        } else {
            self.process_mgr.stop(name).await
        };

        match result {
            Ok(()) => self.set_state(name, ManagerState::Stopped).await,
            Err(e) => {
                self.set_state_with_error(name, ManagerState::Failed, e.to_string())
                    .await
            }
        }

        self.get_status(name).await
    }

    pub async fn restart(&self, name: &ServiceName) -> anyhow::Result<ServiceStatus> {
        self.stop(name).await?;
        self.start(name).await
    }

    pub async fn get_status(&self, name: &ServiceName) -> anyhow::Result<ServiceStatus> {
        let statuses = self.statuses.read().await;
        Ok(statuses
            .get(name)
            .cloned()
            .unwrap_or_else(|| ServiceStatus {
                name: name.clone(),
                state: ManagerState::Stopped,
                pid: None,
                error: None,
                started_at: None,
            }))
    }

    pub async fn get(&self, name: &ServiceName) -> anyhow::Result<Option<ServiceDef>> {
        self.store.get_service(name).await
    }

    pub async fn create(&self, service: &ServiceDef) -> anyhow::Result<()> {
        // Persist to the store and install the s6 run script
        self.store.save_service(service).await?;
        if let Err(e) = service.install() {
            warn!(
                "Failed to install s6 service files for {}: {}",
                service.name, e
            );
        }
        Ok(())
    }

    pub async fn delete(&self, name: &ServiceName) -> anyhow::Result<()> {
        // Best-effort stop before removal
        if let Err(e) = self.stop(name).await {
            warn!("Failed to stop service {} before deletion: {}", name, e);
        }

        // Remove from store
        self.store.delete_service(name).await?;

        // Remove the s6 service directory if it exists
        let path = format!("/etc/s6/sv/{}", name);
        if let Err(e) = tokio::fs::remove_dir_all(&path).await {
            if e.kind() != std::io::ErrorKind::NotFound {
                warn!("Failed to remove s6 service directory {}: {}", path, e);
            }
        }

        // Clear runtime status
        let mut statuses = self.statuses.write().await;
        statuses.remove(name);

        Ok(())
    }

    pub async fn set_enabled(&self, name: &ServiceName, enabled: bool) -> anyhow::Result<()> {
        let mut service = self
            .store
            .get_service(name)
            .await?
            .ok_or_else(|| anyhow::anyhow!("service not found: {}", name))?;

        service.enabled = enabled;
        self.store.save_service(&service).await?;
        Ok(())
    }

    pub async fn list(&self) -> anyhow::Result<Vec<ServiceDef>> {
        self.store.list_services().await
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ServiceEvent> {
        self.events.subscribe()
    }

    async fn set_state(&self, name: &ServiceName, state: ManagerState) {
        let mut statuses = self.statuses.write().await;
        let old_state = statuses
            .get(name)
            .map(|s| s.state.clone())
            .unwrap_or(ManagerState::Stopped);

        let status = statuses
            .entry(name.clone())
            .or_insert_with(|| ServiceStatus {
                name: name.clone(),
                state: ManagerState::Stopped,
                pid: None,
                error: None,
                started_at: None,
            });
        status.state = state.clone();
        status.error = None;

        if matches!(state, ManagerState::Running) {
            status.started_at = Some(chrono::Utc::now());
        }

        let _ = self.events.send(ServiceEvent {
            name: name.clone(),
            old_state,
            new_state: state,
        });
    }

    async fn set_state_with_pid(&self, name: &ServiceName, state: ManagerState, pid: u32) {
        let mut statuses = self.statuses.write().await;
        let status = statuses
            .entry(name.clone())
            .or_insert_with(|| ServiceStatus {
                name: name.clone(),
                state: ManagerState::Stopped,
                pid: None,
                error: None,
                started_at: None,
            });
        status.state = state;
        status.pid = Some(pid);
        status.started_at = Some(chrono::Utc::now());
    }

    async fn set_state_with_error(&self, name: &ServiceName, state: ManagerState, error: String) {
        let mut statuses = self.statuses.write().await;
        let status = statuses
            .entry(name.clone())
            .or_insert_with(|| ServiceStatus {
                name: name.clone(),
                state: ManagerState::Stopped,
                pid: None,
                error: None,
                started_at: None,
            });
        status.state = state;
        status.error = Some(error);
    }
}
</file>

<file path="src/schema/mod.rs">
//! Service Manager Schemas
//!
//! Re-exports service definition types from op-plugins

pub use op_plugins::service_def::*;
</file>

<file path="src/store/mod.rs">
//! JSON flat-file service store.
//!
//! No SQLite, no drift. Desired state = file contents.
//! Every mutation rewrites the entire services file atomically (write+rename).
//! Audit log uses append-only JSON-lines for efficient logging.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::schema::{ServiceDef, ServiceName};

const DEFAULT_SERVICES_PATH: &str = "/var/lib/op-dbus/services.json";
const DEFAULT_AUDIT_PATH: &str = "/var/lib/op-dbus/services-audit.jsonl";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ServicesCatalog {
    services: HashMap<String, ServiceDef>,
}

/// In-memory projection of service definitions with atomic JSON persistence.
pub struct Store {
    services_path: PathBuf,
    audit_path: PathBuf,
    data: RwLock<ServicesCatalog>,
}

impl Store {
    pub async fn new(path: impl AsRef<Path>) -> Result<Self> {
        let services_path = path.as_ref().to_path_buf();
        let audit_path = services_path.with_extension("audit.jsonl");
        Self::with_paths(services_path, audit_path).await
    }

    pub async fn default_store() -> Result<Self> {
        Self::with_paths(DEFAULT_SERVICES_PATH.into(), DEFAULT_AUDIT_PATH.into()).await
    }

    async fn with_paths(services_path: PathBuf, audit_path: PathBuf) -> Result<Self> {
        let catalog = if services_path.exists() {
            match tokio::fs::read_to_string(&services_path).await {
                Ok(contents) => {
                    match serde_json::from_str::<ServicesCatalog>(&contents) {
                        Ok(c) => {
                            info!(services = c.services.len(), "Loaded services from JSON");
                            c
                        }
                        Err(e) => {
                            warn!(error = %e, "Corrupt services JSON, starting fresh");
                            ServicesCatalog::default()
                        }
                    }
                }
                Err(e) => {
                    warn!(error = %e, "Failed to read services JSON, starting fresh");
                    ServicesCatalog::default()
                }
            }
        } else {
            info!("No services file found, starting fresh");
            ServicesCatalog::default()
        };

        Ok(Self {
            services_path,
            audit_path,
            data: RwLock::new(catalog),
        })
    }

    pub async fn get_service(&self, name: &ServiceName) -> Result<Option<ServiceDef>> {
        let guard = self.data.read().await;
        Ok(guard.services.get(name.as_str()).cloned())
    }

    pub async fn save_service(&self, service: &ServiceDef) -> Result<()> {
        let mut guard = self.data.write().await;
        guard
            .services
            .insert(service.name.as_str().to_string(), service.clone());
        drop(guard);
        self.flush().await?;
        Ok(())
    }

    pub async fn delete_service(&self, name: &ServiceName) -> Result<()> {
        let mut guard = self.data.write().await;
        let removed = guard.services.remove(name.as_str()).is_some();
        drop(guard);
        if removed {
            self.flush().await?;
        }
        Ok(())
    }

    pub async fn list_services(&self) -> Result<Vec<ServiceDef>> {
        let guard = self.data.read().await;
        Ok(guard.services.values().cloned().collect())
    }

    pub async fn audit(
        &self,
        service: Option<&str>,
        action: &str,
        details: Option<&str>,
    ) -> Result<()> {
        let entry = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "service_name": service,
            "action": action,
            "details": details,
        });
        let line = format!("{}\n", serde_json::to_string(&entry)?);
        tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.audit_path)
            .await?
            .write_all(line.as_bytes())
            .await?;
        Ok(())
    }

    /// Atomic flush: write to temp file, then rename.
    async fn flush(&self) -> Result<()> {
        let guard = self.data.read().await;
        let json = serde_json::to_string_pretty(&*guard)?;
        drop(guard);

        let tmp = self.services_path.with_extension("tmp");
        tokio::fs::write(&tmp, json).await?;
        tokio::fs::rename(&tmp, &self.services_path).await?;

        debug!(path = %self.services_path.display(), "Flushed services to JSON");
        Ok(())
    }
}
</file>

<file path="src/lib.rs">
//! op-services: System-wide service manager (systemd replacement)

pub mod dbus;
pub mod grpc;
pub mod manager;
pub mod schema;
pub mod store;

pub use manager::*;
pub use schema::*;
pub use store::*;
</file>

<file path="build.rs">
fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .compile_protos(&["proto/services.proto"], &["proto"])?;
    Ok(())
}
</file>

<file path="Cargo.toml">
[package]
name = "op-services"
version = "0.1.0"
edition = "2021"
description = "System-wide service manager - systemd replacement with dinit backend"

[[bin]]
name = "op-services"
path = "src/bin/op-services.rs"

[[bin]]
name = "systemctl"
path = "src/bin/systemctl.rs"

[[bin]]
name = "systemctl-native"
path = "src/bin/systemctl-native.rs"

[dependencies]
# Schema source of truth
op-plugins = { path = "../op-plugins" }

# gRPC
tonic = "0.12"
prost = "0.13"
prost-types = "0.13"

# D-Bus
zbus = { workspace = true }

# Async
tokio = { version = "1", features = ["full", "signal"] }
tokio-stream = "0.1"
futures = "0.3"

# Serialization
serde = { version = "1", features = ["derive"] }
simd-json = "0.13"
serde_json = "1"

# Process management
nix = { version = "0.29", features = ["signal", "process"] }
libc = "0.2"

# Error handling
thiserror = "1"
anyhow = "1"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Time
chrono = { version = "0.4", features = ["serde"] }

# Config
toml = "0.8"

[build-dependencies]
tonic-build = "0.12"
</file>

<file path="compare-op-services.md">
# compare-op-services

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 15 |
| Proto files | 1 |
| Binary targets | 3 |
| UI files | 0 |
| Root-declared modules | 5 |
| Partial artifacts | 0 |
| Spec-listed source files | 14 |
| Spec-listed but missing | 0 |
| Extra implementation files | 1 |

## Current Implementation Overview

- System-wide service manager - systemd replacement with dinit backend
- Internal crate integrations: op-plugins.
- Protocol assets: 1 `.proto` files.

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `src/bin/systemctl.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/bin/systemctl.rs |
| `src/bin/op-services.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/bin/op-services.rs |
| `src/bin/systemctl-native.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/bin/systemctl-native.rs |
| `src/dbus/mod.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/dbus/mod.rs |
| `src/dbus/interface.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/dbus/interface.rs |
| `src/grpc/server.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/grpc/server.rs |
| `src/grpc/mod.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/grpc/mod.rs |
| `src/manager/service_manager.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/manager/service_manager.rs |
| `src/manager/process.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/manager/process.rs |
| `src/manager/mod.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/manager/mod.rs |
| `src/manager/dinit_proxy.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/manager/dinit_proxy.rs |
| `src/schema/mod.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/schema/mod.rs |
| `src/store/mod.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/store/mod.rs |
| `src/lib.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/lib.rs |
| `bin` | ✅ Present | bin group | src/bin/op-services.rs, src/bin/systemctl-native.rs, src/bin/systemctl.rs |
| `build.rs` | ✅ Present | build script | build.rs |
| `dbus` | ✅ Present | dbus group | src/dbus/interface.rs, src/dbus/mod.rs |
| `grpc` | ✅ Present | grpc group | src/grpc/mod.rs, src/grpc/server.rs |
| `manager` | ✅ Present | manager group | src/manager/dinit_proxy.rs, src/manager/mod.rs, src/manager/process.rs, src/manager/service_manager.rs |
| `root` | ✅ Present | root source group | src/lib.rs |
| `schema` | ✅ Present | schema group | src/schema/mod.rs |
| `store` | ✅ Present | store group | src/store/mod.rs |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| Protocol `services.proto` | ✅ Implemented | proto/services.proto | proto |
| Binary `op-services` | ✅ Implemented | src/bin/op-services.rs | Cargo bin target |
| Binary `systemctl` | ✅ Implemented | src/bin/systemctl.rs | Cargo bin target |
| Binary `systemctl-native` | ✅ Implemented | src/bin/systemctl-native.rs | Cargo bin target |

## Dependencies Comparison

### Internal Workspace Dependencies
- `op-plugins` - documented in SPEC

### External Runtime Dependencies
- `tonic` - documented in SPEC
- `prost` - documented in SPEC
- `prost-types` - documented in SPEC
- `zbus` - documented in SPEC
- `sqlx` - documented in SPEC
- `tokio` - documented in SPEC
- `tokio-stream` - documented in SPEC
- `futures` - documented in SPEC
- `serde` - not listed in SPEC dependency block
- `simd-json` - not listed in SPEC dependency block
- `serde_json` - not listed in SPEC dependency block
- `nix` - not listed in SPEC dependency block
- `libc` - not listed in SPEC dependency block
- `thiserror` - not listed in SPEC dependency block
- `anyhow` - not listed in SPEC dependency block
- `tracing` - not listed in SPEC dependency block
- `tracing-subscriber` - not listed in SPEC dependency block
- `chrono` - not listed in SPEC dependency block
- `toml` - not listed in SPEC dependency block

### Development and Build Dependencies
- `build:tonic-build`

## Notes and Observations

- Local documentation files present: SPEC.md.
- Current implementation contains 1 Rust source files beyond the explicit spec/design source inventory.
- Root module declarations found in `lib.rs`/`main.rs`: dbus, grpc, manager, schema, store.
- RPC or protocol definition files: proto/services.proto.
- 11 runtime dependencies are present in `Cargo.toml` but omitted from the SPEC dependency block.
</file>

<file path="SPEC.md">
# op-services - Specification

## Overview
**Crate**: `op-services`  
**Location**: `crates/op-services`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-services"
version = "0.1.0"
edition = "2021"
description = "System-wide service manager - systemd replacement with dinit backend"
```

### Source Structure
```
op-services/src/bin/systemctl.rs
op-services/src/bin/op-services.rs
op-services/src/bin/systemctl-native.rs
op-services/src/dbus/mod.rs
op-services/src/dbus/interface.rs
op-services/src/grpc/server.rs
op-services/src/grpc/mod.rs
op-services/src/manager/service_manager.rs
op-services/src/manager/process.rs
op-services/src/manager/mod.rs
op-services/src/manager/dinit_proxy.rs
op-services/src/schema/mod.rs
op-services/src/store/mod.rs
op-services/src/lib.rs
```

### Key Dependencies
```toml
# Schema source of truth
op-plugins = { path = "../op-plugins" }

# gRPC
tonic = "0.12"
prost = "0.13"
prost-types = "0.13"

# D-Bus
zbus = { version = "4.0", features = ["tokio"] }

# Database
sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite"] }

# Async
tokio = { version = "1", features = ["full", "signal"] }
tokio-stream = "0.1"
futures = "0.3"

# Serialization
```

### Binaries
```toml
[[bin]]
name = "op-services"
path = "src/bin/op-services.rs"

[[bin]]
name = "systemctl"
path = "src/bin/systemctl.rs"

[[bin]]
name = "systemctl-native"
path = "src/bin/systemctl-native.rs"
```

### Features
```toml
# No features
```

## Documentation Files


## Module Structure
      14 Rust source files

### Main Modules


## Purpose
System-wide service manager - systemd replacement with dinit backend

## Build Information
- **Edition**: 2021
- **Version**: 0.1.0
- **License**: 

## Related Crates
Internal dependencies:
- op-plugins

---
*Generated from crate analysis*
</file>

</files>
