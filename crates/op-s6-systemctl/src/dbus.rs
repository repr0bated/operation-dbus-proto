//! D-Bus Service Implementation for s6-systemctl
//!
//! Implements the org.opdbus.v1.S6.Systemctl interface, mapping
//! systemctl commands to s6/s6-rc operations for Artix Linux.

use std::process::Command;
use tracing::{debug, error, info, warn};
use zbus::interface;

/// D-Bus service for s6-systemctl operations
pub struct S6SystemctlService {
    /// Base directory for s6 service definitions
    s6_svc_dir: String,
    /// s6-rc live directory
    s6_rc_dir: String,
}

impl S6SystemctlService {
    pub fn new() -> Self {
        Self {
            s6_svc_dir: "/etc/s6/sv".to_string(),
            s6_rc_dir: "/run/s6-rc".to_string(),
        }
    }

    /// Check if s6 tools are available
    fn check_s6_available(&self) -> bool {
        Command::new("s6-svscan").arg("--help").output().is_ok()
    }

    /// Execute s6-rc command and return (success, output)
    fn run_s6_rc(&self, args: &[&str]) -> (bool, String) {
        if !self.check_s6_available() {
            return (
                false,
                "s6 tools not available. Is s6 installed?".to_string(),
            );
        }

        let mut cmd = Command::new("s6-rc");
        cmd.args(args);

        match cmd.output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                if output.status.success() {
                    let result = if stdout.is_empty() { stderr } else { stdout };
                    (true, result.trim().to_string())
                } else {
                    let error = if stderr.is_empty() { stdout } else { stderr };
                    (false, error.trim().to_string())
                }
            }
            Err(e) => (false, format!("Failed to execute s6-rc: {}", e)),
        }
    }

    /// Execute s6-svc command and return (success, output)
    fn run_s6_svc(&self, service: &str, signal: &str) -> (bool, String) {
        if !self.check_s6_available() {
            return (
                false,
                "s6 tools not available. Is s6 installed?".to_string(),
            );
        }

        let service_path = format!("{}/{}", self.s6_svc_dir, service);

        let mut cmd = Command::new("s6-svc");
        cmd.arg(signal).arg(&service_path);

        match cmd.output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();

                if output.status.success() {
                    let result = if stdout.is_empty() { stderr } else { stdout };
                    (true, result.trim().to_string())
                } else {
                    let error = if stderr.is_empty() { stdout } else { stderr };
                    (false, error.trim().to_string())
                }
            }
            Err(e) => (false, format!("Failed to execute s6-svc: {}", e)),
        }
    }

    /// Execute s6-svstat command and parse output
    fn run_s6_svstat(&self, service: &str) -> Result<S6ServiceStatus, String> {
        let service_path = format!("{}/{}", self.s6_svc_dir, service);

        match Command::new("s6-svstat").arg(&service_path).output() {
            Ok(output) => {
                if !output.status.success() {
                    return Err(String::from_utf8_lossy(&output.stderr).to_string());
                }

                let stdout = String::from_utf8_lossy(&output.stdout);
                Ok(S6ServiceStatus::from_svstat_output(&stdout, service))
            }
            Err(e) => Err(format!("Failed to execute s6-svstat: {}", e)),
        }
    }

    /// Check if service is enabled (in the active bundle)
    fn is_service_enabled(&self, service: &str) -> bool {
        // Check /etc/s6-rc/default/<service> symlink exists
        let enabled_link = format!("/etc/s6-rc/default/{}", service);
        std::path::Path::new(&enabled_link).exists()
    }
}

/// Service status parsed from s6-svstat output
#[derive(Debug, serde::Serialize)]
struct S6ServiceStatus {
    name: String,
    active_state: String,
    sub_state: String,
    main_pid: Option<u32>,
    ready: bool,
    up_time: Option<String>,
}

impl S6ServiceStatus {
    fn from_svstat_output(output: &str, name: &str) -> Self {
        // Parse s6-svstat output: "up (pid 1234) X seconds" or "down X seconds"
        let output = output.trim();

        let (active_state, sub_state, main_pid, ready) = if output.starts_with("up") {
            let pid = output
                .split("pid ")
                .nth(1)
                .and_then(|s| s.split(')').next())
                .and_then(|p| p.parse::<u32>().ok());

            ("active".to_string(), "running".to_string(), pid, true)
        } else if output.starts_with("down") {
            ("inactive".to_string(), "dead".to_string(), None, false)
        } else {
            ("unknown".to_string(), "unknown".to_string(), None, false)
        };

        let up_time = output.split(" ").last().map(|s| s.to_string());

        Self {
            name: name.to_string(),
            active_state,
            sub_state,
            main_pid,
            ready,
            up_time,
        }
    }

    fn to_json(&self) -> String {
        match simd_json::to_string(self) {
            Ok(s) => s,
            Err(_) => format!(
                "{{\"name\":\"{}\",\"active_state\":\"{}\",\"sub_state\":\"{}\"}}",
                self.name, self.active_state, self.sub_state
            ),
        }
    }
}

#[interface(name = "opdbus.v1.S6.Systemctl")]
impl S6SystemctlService {
    /// Start a service (maps to: s6-rc -u change <service>)
    async fn start(&self, service: &str) -> (bool, String) {
        debug!("Starting service: {}", service);
        info!("systemctl start {} -> s6-rc -u change {}", service, service);

        let result = self.run_s6_rc(&["-u", "change", service]);

        if result.0 {
            info!("Service {} started successfully", service);
        } else {
            error!("Failed to start service {}: {}", service, result.1);
        }

        result
    }

    /// Stop a service (maps to: s6-rc -d change <service>)
    async fn stop(&self, service: &str) -> (bool, String) {
        debug!("Stopping service: {}", service);
        info!("systemctl stop {} -> s6-rc -d change {}", service, service);

        let result = self.run_s6_rc(&["-d", "change", service]);

        if result.0 {
            info!("Service {} stopped successfully", service);
        } else {
            error!("Failed to stop service {}: {}", service, result.1);
        }

        result
    }

    /// Restart a service (maps to: s6 process restart <service>)
    async fn restart(&self, service: &str) -> (bool, String) {
        debug!("Restarting service: {}", service);
        info!(
            "systemctl restart {} -> s6 process restart {}",
            service, service
        );

        match Command::new("s6")
            .args(["process", "restart", service])
            .output()
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                if output.status.success() {
                    info!("Service {} restarted successfully", service);
                    (
                        true,
                        if stdout.is_empty() { stderr } else { stdout }
                            .trim()
                            .to_string(),
                    )
                } else {
                    let msg = if stderr.is_empty() { stdout } else { stderr }
                        .trim()
                        .to_string();
                    error!("Failed to restart service {}: {}", service, msg);
                    (false, msg)
                }
            }
            Err(e) => (
                false,
                format!("Failed to execute s6 process restart: {}", e),
            ),
        }
    }

    /// Reload a service (maps to: s6-svc -h <service>)
    /// Sends SIGHUP to the service for configuration reload
    async fn reload(&self, service: &str) -> (bool, String) {
        debug!("Reloading service: {}", service);
        info!(
            "systemctl reload {} -> s6-svc -h {}/{}",
            service, self.s6_svc_dir, service
        );

        let result = self.run_s6_svc(service, "-h");

        if result.0 {
            info!("Service {} reloaded successfully", service);
        } else {
            error!("Failed to reload service {}: {}", service, result.1);
        }

        result
    }

    /// Enable a service (add to s6-rc bundle)
    /// Note: This creates a symlink in the s6-rc service directory
    async fn enable(&self, service: &str) -> (bool, String) {
        debug!("Enabling service: {}", service);
        info!(
            "systemctl enable {} -> s6-rc-bundle add {}",
            service, service
        );

        let service_src = format!("{}/{}", self.s6_svc_dir, service);
        if !std::path::Path::new(&service_src).exists() {
            return (
                false,
                format!("Service {} not found in {}", service, self.s6_svc_dir),
            );
        }

        // Use s6-rc-bundle or manual symlink approach
        // s6-rc-bundle is the modern way, fall back to manual symlink
        match Command::new("s6-rc-bundle")
            .args(["add", "default", service])
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    info!("Service {} enabled successfully", service);
                    (true, format!("Service {} enabled", service))
                } else {
                    let err = String::from_utf8_lossy(&output.stderr);
                    // Fallback: manual symlink to /etc/s6-rc/default
                    let enabled_dir = format!("/etc/s6-rc/default/{}", service);
                    match std::os::unix::fs::symlink(&service_src, &enabled_dir) {
                        Ok(_) => {
                            info!("Service {} enabled via symlink", service);
                            (true, format!("Service {} enabled", service))
                        }
                        Err(e) => {
                            error!("Failed to enable service {}: {}", service, e);
                            (
                                false,
                                format!("Failed to enable: {} (s6-rc-bundle: {})", e, err),
                            )
                        }
                    }
                }
            }
            Err(e) => {
                warn!("s6-rc-bundle not available, using manual symlink: {}", e);
                // Manual fallback
                let enabled_dir = format!("/etc/s6-rc/default/{}", service);
                match std::os::unix::fs::symlink(&service_src, &enabled_dir) {
                    Ok(_) => {
                        info!("Service {} enabled via symlink", service);
                        (true, format!("Service {} enabled", service))
                    }
                    Err(e) => {
                        error!("Failed to enable service {}: {}", service, e);
                        (false, format!("Failed to enable: {}", e))
                    }
                }
            }
        }
    }

    /// Disable a service (remove from s6-rc bundle)
    async fn disable(&self, service: &str) -> (bool, String) {
        debug!("Disabling service: {}", service);
        info!(
            "systemctl disable {} -> s6-rc-bundle delete {}",
            service, service
        );

        // Stop the service first
        let _ = self.run_s6_rc(&["-d", "change", service]);

        match Command::new("s6-rc-bundle")
            .args(["delete", "default", service])
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    info!("Service {} disabled successfully", service);
                    (true, format!("Service {} disabled", service))
                } else {
                    // Fallback: remove manual symlink
                    let enabled_link = format!("/etc/s6-rc/default/{}", service);
                    match std::fs::remove_file(&enabled_link) {
                        Ok(_) => {
                            info!("Service {} disabled via symlink removal", service);
                            (true, format!("Service {} disabled", service))
                        }
                        Err(e) => {
                            error!("Failed to disable service {}: {}", service, e);
                            (false, format!("Failed to disable: {}", e))
                        }
                    }
                }
            }
            Err(e) => {
                warn!("s6-rc-bundle not available, using manual removal: {}", e);
                let enabled_link = format!("/etc/s6-rc/default/{}", service);
                match std::fs::remove_file(&enabled_link) {
                    Ok(_) => {
                        info!("Service {} disabled via symlink removal", service);
                        (true, format!("Service {} disabled", service))
                    }
                    Err(e) => {
                        error!("Failed to disable service {}: {}", service, e);
                        (false, format!("Failed to disable: {}", e))
                    }
                }
            }
        }
    }

    /// Get detailed status of a service (JSON format)
    async fn status(&self, service: &str) -> String {
        debug!("Getting status for service: {}", service);

        match self.run_s6_svstat(service) {
            Ok(status) => status.to_json(),
            Err(e) => {
                warn!("Failed to get status for {}: {}", service, e);
                format!(
                    "{{\"name\":\"{}\",\"error\":\"{}\",\"active_state\":\"unknown\"}}",
                    service, e
                )
            }
        }
    }

    /// Check if a service is active (returns "active" or "inactive")
    async fn is_active(&self, service: &str) -> String {
        match self.run_s6_svstat(service) {
            Ok(status) => {
                if status.active_state == "active" {
                    "active".to_string()
                } else {
                    "inactive".to_string()
                }
            }
            Err(_) => "inactive".to_string(),
        }
    }

    /// Check if a service is enabled (returns "enabled" or "disabled")
    async fn is_enabled(&self, service: &str) -> String {
        if self.is_service_enabled(service) {
            "enabled".to_string()
        } else {
            "disabled".to_string()
        }
    }

    /// List all active units (JSON array)
    async fn list_units(&self) -> String {
        debug!("Listing all units");

        if !self.check_s6_available() {
            return r#"{"error":"s6 tools not available"}"#.to_string();
        }

        // Get list of running services from s6-rc
        match Command::new("s6-rc").args(["-a", "list"]).output() {
            Ok(output) => {
                if !output.status.success() {
                    return format!(
                        "{{\"error\":\"{}\"}}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                }

                let stdout = String::from_utf8_lossy(&output.stdout);
                let services: Vec<&str> = stdout.lines().collect();

                let mut units = Vec::new();
                for service in services {
                    let service = service.trim();
                    if service.is_empty() {
                        continue;
                    }

                    match self.run_s6_svstat(service) {
                        Ok(status) => units.push(status),
                        Err(_) => {
                            units.push(S6ServiceStatus {
                                name: service.to_string(),
                                active_state: "unknown".to_string(),
                                sub_state: "unknown".to_string(),
                                main_pid: None,
                                ready: false,
                                up_time: None,
                            });
                        }
                    }
                }

                match simd_json::to_string(&units) {
                    Ok(json) => json,
                    Err(e) => format!("{{\"error\":\"Failed to serialize: {}\"}}", e),
                }
            }
            Err(e) => {
                error!("Failed to list units: {}", e);
                format!("{{\"error\":\"Failed to list units: {}\"}}", e)
            }
        }
    }

    /// Get daemon/s6 supervisor status
    async fn daemon_status(&self) -> String {
        if self.check_s6_available() {
            // Check if s6-svscan is actually running
            match Command::new("pgrep").arg("-x").arg("s6-svscan").output() {
                Ok(output) => {
                    if output.status.success() {
                        "running".to_string()
                    } else {
                        "not-available".to_string()
                    }
                }
                Err(_) => {
                    // Fallback: check /run/s6-rc exists
                    if std::path::Path::new(&self.s6_rc_dir).exists() {
                        "running".to_string()
                    } else {
                        "not-available".to_string()
                    }
                }
            }
        } else {
            "not-available".to_string()
        }
    }
}
