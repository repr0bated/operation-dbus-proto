//! gRPC server implementation

use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use super::proto::service_manager_server::ServiceManager as ServiceManagerTrait;
use super::proto::*;
use crate::manager::ServiceManager;
use crate::schema::ServiceName;

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
        _req: Request<ReloadRequest>,
    ) -> Result<Response<ReloadResponse>, Status> {
        Err(Status::unimplemented("reload not yet implemented"))
    }

    async fn create(
        &self,
        _req: Request<CreateRequest>,
    ) -> Result<Response<CreateResponse>, Status> {
        Err(Status::unimplemented("create not yet implemented"))
    }

    async fn delete(
        &self,
        _req: Request<DeleteRequest>,
    ) -> Result<Response<DeleteResponse>, Status> {
        Err(Status::unimplemented("delete not yet implemented"))
    }

    async fn get(&self, _req: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        Err(Status::unimplemented("get not yet implemented"))
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
        _req: Request<EnableRequest>,
    ) -> Result<Response<EnableResponse>, Status> {
        Err(Status::unimplemented("enable not yet implemented"))
    }

    async fn disable(
        &self,
        _req: Request<DisableRequest>,
    ) -> Result<Response<DisableResponse>, Status> {
        Err(Status::unimplemented("disable not yet implemented"))
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
