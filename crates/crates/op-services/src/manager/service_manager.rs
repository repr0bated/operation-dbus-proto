//! Core service manager

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use tracing::{error, info, warn};

use super::{DinitProxy, ProcessManager};
use crate::schema::{ManagerState, ServiceDef, ServiceName, ServiceStatus};
use crate::store::Store;

pub struct ServiceManager {
    store: Arc<Store>,
    dinit: Option<DinitProxy>,
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
        let dinit = match DinitProxy::new().await {
            Ok(d) => {
                info!("Connected to dinit-dbus");
                Some(d)
            }
            Err(e) => {
                warn!("dinit-dbus unavailable, using fallback: {}", e);
                None
            }
        };

        let (events, _) = broadcast::channel(256);

        Ok(Self {
            store,
            dinit,
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

        let result = if let Some(ref dinit) = self.dinit {
            dinit.start_service(name.as_str()).await
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

        let result = if let Some(ref dinit) = self.dinit {
            dinit.stop_service(name.as_str()).await
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
