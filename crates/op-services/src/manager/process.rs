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
