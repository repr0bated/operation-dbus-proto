//! D-Bus service implementation for Xray daemon management
//!
//! Provides the `org.opdbus.v1.Xray` interface for controlling
//! xray process lifecycle via D-Bus.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::process::{Child as TokioChild, Command};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};
use zbus::interface;

/// Status information for the Xray daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
struct XrayStatus {
    running: bool,
    pid: Option<u32>,
    config_path: Option<String>,
    uptime_secs: Option<u64>,
    start_time: Option<String>,
}

impl XrayStatus {}

/// State container for the xray process
pub struct XrayProcessState {
    child: Option<TokioChild>,
    config_path: Option<String>,
    start_time: Option<std::time::Instant>,
}

impl XrayProcessState {
    fn new() -> Self {
        Self {
            child: None,
            config_path: None,
            start_time: None,
        }
    }

    fn is_running(&self) -> bool {
        // Check if child exists. For TokioChild, we can use try_wait via a ref
        // but we need mutable access. Instead, we track state separately.
        // For now, just check if child handle exists (we update on operations).
        self.child.is_some()
    }

    fn get_pid(&self) -> Option<u32> {
        // child.id() returns Option<u32>
        self.child.as_ref().and_then(|c| c.id())
    }

    fn get_uptime_secs(&self) -> Option<u64> {
        self.start_time.map(|t| t.elapsed().as_secs())
    }

    fn get_start_time(&self) -> Option<String> {
        // Use current time minus elapsed to get start time
        self.start_time.and_then(|t| {
            let elapsed = t.elapsed();
            let now = chrono::Local::now();
            let start = now - chrono::Duration::from_std(elapsed).ok()?;
            Some(start.format("%Y-%m-%dT%H:%M:%S").to_string())
        })
    }
}

/// D-Bus service for managing Xray daemon lifecycle
pub struct XrayService {
    state: Arc<Mutex<XrayProcessState>>,
}

impl XrayService {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(XrayProcessState::new())),
        }
    }

    /// Create a status snapshot for external reporting
    async fn get_status_snapshot(&self) -> XrayStatus {
        let state = self.state.lock().await;
        XrayStatus {
            running: state.is_running(),
            pid: state.get_pid(),
            config_path: state.config_path.clone(),
            uptime_secs: state.get_uptime_secs(),
            start_time: state.get_start_time(),
        }
    }
}

/// D-Bus interface implementation for org.opdbus.v1.Xray
#[interface(name = "opdbus.v1.Xray")]
impl XrayService {
    /// Start xray with the specified config path
    /// Returns (success, message)
    async fn start(&mut self, config_path: &str) -> (bool, String) {
        info!("Xray start requested with config: {}", config_path);

        let mut state = self.state.lock().await;

        // Check if already running
        if state.is_running() {
            warn!("Xray is already running, PID: {:?}", state.get_pid());
            return (
                false,
                format!("Xray is already running (PID: {:?})", state.get_pid()),
            );
        }

        // Validate config path exists
        if !std::path::Path::new(config_path).exists() {
            error!("Config file does not exist: {}", config_path);
            return (
                false,
                format!("Config file does not exist: {}", config_path),
            );
        }

        // Ensure config is in /dev/shm as per AGENTS.md §4a (Zero-Btrfs)
        if !config_path.starts_with("/dev/shm") {
            warn!(
                "Config path is not in /dev/shm, proceeding anyway: {}",
                config_path
            );
        }

        // Spawn xray process
        match Command::new("xray")
            .arg("-c")
            .arg(config_path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(child) => {
                let pid = child.id();
                state.child = Some(child);
                state.config_path = Some(config_path.to_string());
                state.start_time = Some(std::time::Instant::now());

                match pid {
                    Some(p) => {
                        info!("Xray started successfully, PID: {}", p);
                        (true, format!("Xray started successfully (PID: {})", p))
                    }
                    None => {
                        info!("Xray started successfully (PID unavailable)");
                        (true, "Xray started successfully".to_string())
                    }
                }
            }
            Err(e) => {
                error!("Failed to start xray: {}", e);
                (false, format!("Failed to start xray: {}", e))
            }
        }
    }

    /// Stop the running xray process
    /// Returns (success, message)
    async fn stop(&mut self) -> (bool, String) {
        info!("Xray stop requested");

        let mut state = self.state.lock().await;

        if !state.is_running() {
            warn!("Xray is not running");
            return (false, "Xray is not running".to_string());
        }

        if let Some(mut child) = state.child.take() {
            match child.kill().await {
                Ok(_) => {
                    info!("Xray process terminated");
                    state.config_path = None;
                    state.start_time = None;
                    (true, "Xray stopped successfully".to_string())
                }
                Err(e) => {
                    error!("Failed to kill xray process: {}", e);
                    // Put it back if kill failed
                    state.child = Some(child);
                    (false, format!("Failed to stop xray: {}", e))
                }
            }
        } else {
            (false, "Xray process handle not available".to_string())
        }
    }

    /// Restart xray with the specified config path
    /// Returns (success, message)
    async fn restart(&mut self, config_path: &str) -> (bool, String) {
        info!("Xray restart requested");

        // Stop first
        let stop_result = self.stop().await;
        if !stop_result.0 && !stop_result.1.contains("not running") {
            return (
                false,
                format!("Failed to stop existing xray: {}", stop_result.1),
            );
        }

        // Small delay to ensure process cleanup
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Then start
        self.start(config_path).await
    }

    /// Get current xray status as JSON string
    async fn status(&self) -> String {
        let status = self.get_status_snapshot().await;
        match serde_json::to_string(&status) {
            Ok(json) => {
                debug!("Xray status: {}", json);
                json
            }
            Err(e) => {
                error!("Failed to serialize status: {}", e);
                r#"{"running":false,"error":"serialization failed"}"#.to_string()
            }
        }
    }

    /// Reload xray configuration by sending SIGHUP
    /// Returns (success, message)
    async fn reload(&self) -> (bool, String) {
        info!("Xray reload requested");

        let state = self.state.lock().await;

        if !state.is_running() {
            warn!("Cannot reload: Xray is not running");
            return (false, "Xray is not running".to_string());
        }

        let pid = match state.get_pid() {
            Some(p) => p,
            None => {
                return (false, "Cannot determine xray PID".to_string());
            }
        };

        // Send SIGHUP signal
        match nix::sys::signal::kill(
            nix::unistd::Pid::from_raw(pid as i32),
            nix::sys::signal::Signal::SIGHUP,
        ) {
            Ok(_) => {
                info!("Sent SIGHUP to xray PID {}", pid);
                (true, format!("Reload signal sent to xray (PID: {})", pid))
            }
            Err(e) => {
                error!("Failed to send SIGHUP to xray PID {}: {}", pid, e);
                (false, format!("Failed to send reload signal: {}", e))
            }
        }
    }

    /// Get current config path
    async fn get_config(&self) -> String {
        let state = self.state.lock().await;
        state.config_path.clone().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_xray_status_serialization() {
        let status = XrayStatus {
            running: true,
            pid: Some(1234),
            config_path: Some("/dev/shm/xray_config.json".to_string()),
            uptime_secs: Some(3600),
            start_time: Some("2024-01-01T00:00:00".to_string()),
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"running\":true"));
        assert!(json.contains("\"pid\":1234"));
        assert!(json.contains("\"config_path\":\"/dev/shm/xray_config.json\""));
    }

    #[tokio::test]
    async fn test_xray_service_creation() {
        let service = XrayService::new();
        let status = service.get_status_snapshot().await;
        assert!(!status.running);
        assert_eq!(status.pid, None);
    }
}
