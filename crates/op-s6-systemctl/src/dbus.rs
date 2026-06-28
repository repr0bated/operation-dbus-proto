//! D-Bus Service Implementation for s6-systemctl
//!
//! Implements the org.opdbus.v1.S6.Systemctl interface, mapping
//! systemctl commands to s6/s6-rc operations for Artix Linux.

use std::process::Command;
use tracing::{debug, error, info, warn};
use zbus::interface;

/// D-Bus service for s6-systemctl operations
pub struct S6SystemctlService {
    /// Base directory for Artix s6 service definitions.
    s6_svc_dir: String,
    /// Live supervision directory for running longruns.
    s6_runtime_dir: String,
    /// s6-rc live directory.
    s6_rc_dir: String,
}

impl S6SystemctlService {
    pub fn new() -> Self {
        Self {
            s6_svc_dir: "/etc/s6/sv".to_string(),
            s6_runtime_dir: "/run/service".to_string(),
            s6_rc_dir: "/run/s6-rc".to_string(),
        }
    }

    fn enable_bundle(&self) -> String {
        std::env::var("S6D_ENABLE_BUNDLE").unwrap_or_else(|_| "misc".to_string())
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
        if std::path::Path::new(&self.s6_rc_dir).exists() {
            cmd.arg("-l").arg(&self.s6_rc_dir);
        }
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

        let service_path = format!("{}/{}", self.s6_runtime_dir, service);

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
        let service_path = format!("{}/{}", self.s6_runtime_dir, service);

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
        let bundle_entry = format!(
            "{}/{}/contents.d/{}",
            self.s6_svc_dir,
            self.enable_bundle(),
            service
        );
        if std::path::Path::new(&bundle_entry).exists() {
            return true;
        }

        let Ok(entries) = std::fs::read_dir(&self.s6_svc_dir) else {
            return false;
        };
        entries.flatten().any(|entry| {
            let path = entry.path().join("contents.d").join(service);
            path.exists()
        })
    }

    fn run_artix_frontend_reload(&self) -> (bool, String) {
        let script = std::env::var("S6D_RELOAD_SCRIPT")
            .unwrap_or_else(|_| "/usr/local/sbin/op-s6-recompile-and-update".to_string());
        if std::path::Path::new(&script).exists() {
            match Command::new("sh")
                .env("SKIP_BUILD", "1")
                .arg(&script)
                .output()
            {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    if output.status.success() {
                        return (true, stdout);
                    }
                    return (false, if stderr.is_empty() { stdout } else { stderr });
                }
                Err(e) => return (false, format!("Failed to execute {script}: {e}")),
            }
        }

        let repo_script = "/home/jeremy/git/operation-dbus-proto/deploy/s6/recompile-and-update.sh";
        if std::path::Path::new(repo_script).exists() {
            match Command::new("sh")
                .env("SKIP_BUILD", "1")
                .arg(repo_script)
                .output()
            {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    if output.status.success() {
                        return (true, stdout);
                    }
                    return (false, if stderr.is_empty() { stdout } else { stderr });
                }
                Err(e) => return (false, format!("Failed to execute {repo_script}: {e}")),
            }
        }

        let steps: &[(&str, &[&str])] = &[
            ("s6", &["repository", "sync"]),
            ("s6", &["set", "check", "-F", "-u"]),
            ("s6", &["set", "commit", "-f", "-D", "default"]),
            ("s6", &["live", "install", "-b"]),
        ];
        let mut messages = Vec::new();
        for (bin, args) in steps {
            match Command::new(bin).args(*args).output() {
                Ok(output) if output.status.success() => {
                    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !stdout.is_empty() {
                        messages.push(stdout);
                    }
                }
                Ok(output) => {
                    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    return (
                        false,
                        format!(
                            "{} {:?} failed: {}",
                            bin,
                            args,
                            if stderr.is_empty() { stdout } else { stderr }
                        ),
                    );
                }
                Err(e) => return (false, format!("Failed to run {bin}: {e}")),
            }
        }
        (true, messages.join("\n"))
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

#[interface(name = "org.opdbus.v1.S6.Systemctl")]
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
            service, self.s6_runtime_dir, service
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
        let bundle = self.enable_bundle();
        info!("systemctl enable {} -> add to {} bundle", service, bundle);

        let service_src = format!("{}/{}", self.s6_svc_dir, service);
        if !std::path::Path::new(&service_src).exists() {
            return (
                false,
                format!("Service {} not found in {}", service, self.s6_svc_dir),
            );
        }

        let bundle_dir = format!("{}/{}/contents.d", self.s6_svc_dir, bundle);
        if let Err(e) = std::fs::create_dir_all(&bundle_dir) {
            return (
                false,
                format!("Failed to create bundle dir {bundle_dir}: {e}"),
            );
        }
        let bundle_entry = format!("{}/{}", bundle_dir, service);
        match std::fs::File::create(&bundle_entry) {
            Ok(_) => {
                info!("Service {} enabled in {} bundle", service, bundle);
                (
                    true,
                    format!("Service {service} enabled in {bundle}; run daemon-reload"),
                )
            }
            Err(e) => {
                error!("Failed to enable service {}: {}", service, e);
                (false, format!("Failed to enable {service}: {e}"))
            }
        }
    }

    /// Disable a service (remove from s6-rc bundle)
    async fn disable(&self, service: &str) -> (bool, String) {
        debug!("Disabling service: {}", service);
        let bundle = self.enable_bundle();
        info!(
            "systemctl disable {} -> remove from {} bundle",
            service, bundle
        );

        // Stop the service first
        let _ = self.run_s6_rc(&["-d", "change", service]);

        let enabled_link = format!("{}/{}/contents.d/{}", self.s6_svc_dir, bundle, service);
        match std::fs::remove_file(&enabled_link) {
            Ok(_) => {
                info!("Service {} disabled via bundle removal", service);
                (
                    true,
                    format!("Service {service} disabled from {bundle}; run daemon-reload"),
                )
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                (true, format!("Service {service} was not in {bundle}"))
            }
            Err(e) => {
                error!("Failed to disable service {}: {}", service, e);
                (false, format!("Failed to disable {service}: {e}"))
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

    /// Compatibility alias used by existing generated zbus proxies.
    async fn is_enabled_method(&self, service: &str) -> String {
        self.is_enabled(service).await
    }

    /// Show service properties as JSON.
    async fn show(&self, service: &str) -> String {
        let mut properties = serde_json::Map::new();
        properties.insert("Id".to_string(), serde_json::json!(service));
        properties.insert("Names".to_string(), serde_json::json!(service));
        properties.insert("LoadState".to_string(), serde_json::json!("loaded"));
        properties.insert(
            "UnitFileState".to_string(),
            serde_json::json!(self.is_enabled(service).await),
        );
        properties.insert(
            "Type".to_string(),
            serde_json::json!(self.get_unit_type(service).await),
        );

        match self.run_s6_svstat(service) {
            Ok(status) => {
                properties.insert(
                    "ActiveState".to_string(),
                    serde_json::json!(status.active_state),
                );
                properties.insert("SubState".to_string(), serde_json::json!(status.sub_state));
                properties.insert("MainPID".to_string(), serde_json::json!(status.main_pid));
                properties.insert("Ready".to_string(), serde_json::json!(status.ready));
                properties.insert("UpTime".to_string(), serde_json::json!(status.up_time));
            }
            Err(e) => {
                properties.insert("ActiveState".to_string(), serde_json::json!("unknown"));
                properties.insert("SubState".to_string(), serde_json::json!("error"));
                properties.insert("Error".to_string(), serde_json::json!(e));
            }
        }

        serde_json::Value::Object(properties).to_string()
    }

    /// List all active units (JSON array)
    async fn list_units(&self) -> String {
        debug!("Listing all units");

        if !self.check_s6_available() {
            return r#"{"error":"s6 tools not available"}"#.to_string();
        }

        // Get list of running services from s6-rc
        let mut cmd = Command::new("s6-rc");
        if std::path::Path::new(&self.s6_rc_dir).exists() {
            cmd.arg("-l").arg(&self.s6_rc_dir);
        }
        match cmd.args(["-a", "list"]).output() {
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

    /// List all available unit files from the s6 service directory.
    async fn list_unit_files(&self) -> String {
        debug!("Listing all unit files");

        let entries = match std::fs::read_dir(&self.s6_svc_dir) {
            Ok(entries) => entries,
            Err(e) => return format!("{{\"error\":\"Failed to list unit files: {}\"}}", e),
        };

        let mut units = Vec::new();
        for entry in entries.flatten() {
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => continue,
            };
            if !file_type.is_dir() && !file_type.is_symlink() {
                continue;
            }
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            if name.starts_with('.') {
                continue;
            }
            units.push(serde_json::json!({
                "unit_file": format!("{name}.service"),
                "state": self.is_enabled(&name).await,
                "vendor_preset": "disabled"
            }));
        }

        units.sort_by(|a, b| {
            a.get("unit_file")
                .and_then(|v| v.as_str())
                .cmp(&b.get("unit_file").and_then(|v| v.as_str()))
        });

        serde_json::Value::Array(units).to_string()
    }

    /// Retrieve service logs using s6-logwatch when present, with tail fallback.
    async fn journalctl(&self, service: &str, lines: u32) -> String {
        let lines = lines.max(1).to_string();
        let log_candidates = [
            format!("/var/log/op-dbus/{service}/current"),
            format!("/var/log/{service}/current"),
            format!("/var/log/{service}/access.log"),
            format!("/var/log/{service}/error.log"),
        ];
        let Some(log_path) = log_candidates
            .iter()
            .find(|path| std::path::Path::new(path.as_str()).exists())
        else {
            return format!("{{\"error\":\"No log file found for {}\"}}", service);
        };

        match Command::new("tail").args(["-n", &lines, log_path]).output() {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let entries: Vec<_> = stdout
                    .lines()
                    .map(|line| {
                        serde_json::json!({
                            "UNIT": service,
                            "LOG_PATH": log_path,
                            "MESSAGE": line
                        })
                    })
                    .collect();
                serde_json::Value::Array(entries).to_string()
            }
            Ok(output) => format!(
                "{{\"error\":\"{}\"}}",
                String::from_utf8_lossy(&output.stderr)
            ),
            Err(e) => format!("{{\"error\":\"Failed to read logs: {}\"}}", e),
        }
    }

    /// Re-read the active s6-rc database.
    async fn daemon_reload(&self) -> (bool, String) {
        self.run_artix_frontend_reload()
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

    /// Return the best-known unit type from the service definition.
    async fn get_unit_type(&self, service: &str) -> String {
        let type_path = format!("{}/{}/type", self.s6_svc_dir, service);
        match std::fs::read_to_string(&type_path) {
            Ok(value) => value.trim().to_string(),
            Err(_) => "unknown".to_string(),
        }
    }
}
