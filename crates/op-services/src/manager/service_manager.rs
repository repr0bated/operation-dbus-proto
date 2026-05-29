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
