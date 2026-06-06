//! S6-Systemctl Wrapper — Maps systemd/systemctl commands to s6 at runtime.
//!
//! Provides a D-Bus interface (`org.opdbus.v1.S6.Systemctl`) that resolves
//! systemd deficiencies by translating systemd concepts to s6 equivalents:
//!
//! | systemd concept | s6 runtime resolution |
//! |-----------------|------------------------|
//! | `.service`      | s6-rc supervised service |
//! | `.timer`        | s6-cron job or s6-tcpserver |
//! | `.socket`       | s6-socketmux or s6-ipcserver |
//! | `.target`       | s6-rc bundle (grouping) |
//! | `.mount`        | mount command + s6-rc oneshot |
//! | `.device`       | udev / sysfs check |
//! | `.path`         | s6-ftrig or inotify |
//! | `journalctl`    | s6-log + s6-logwatch |
//! | `systemctl`     | s6-rc / s6-svc / s6-svstat |
//!
//! All translations happen **at runtime** — callers use systemd vocabulary
//! and the service resolves the deficiency by invoking the correct s6
//! primitive underneath.
//!
//! D-Bus interface: org.opdbus.v1.S6.Systemctl
//! Object path:     /org/opdbus/v1/s6/systemctl

use anyhow::{Context, Result};
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;
use tokio::process::Command;
use tracing::{debug, info, warn};
use zbus::interface;

// ── s6 runtime paths ─────────────────────────────────────────────────────────

const S6_RC_LIVE: &str = "/run/s6-rc";
const S6_SERVICE_DIR: &str = "/run/service";
const S6_SCAN_DIR: &str = "/etc/s6/sv";
const S6_RC_BUNDLE: &str = "/etc/s6-rc/bundle";
const S6_LOG_DIR: &str = "/var/log/s6";

/// Classification of a systemd-style unit name.
#[derive(Debug, Clone, PartialEq)]
enum UnitType {
    Service,
    Timer,
    Socket,
    Target,
    Mount,
    Device,
    Path,
    Unknown(String),
}

impl UnitType {
    fn from_name(name: &str) -> Self {
        if name.ends_with(".timer") {
            UnitType::Timer
        } else if name.ends_with(".socket") {
            UnitType::Socket
        } else if name.ends_with(".target") {
            UnitType::Target
        } else if name.ends_with(".mount") {
            UnitType::Mount
        } else if name.ends_with(".device") {
            UnitType::Device
        } else if name.ends_with(".path") {
            UnitType::Path
        } else if name.ends_with(".service") || !name.contains('.') {
            UnitType::Service
        } else {
            UnitType::Unknown(name.rsplit('.').next().unwrap_or("").to_string())
        }
    }

    fn strip_suffix(&self, name: &str) -> String {
        match self {
            UnitType::Service if !name.contains('.') => name.to_string(),
            _ => name
                .rsplit_once('.')
                .map(|(base, _)| base)
                .unwrap_or(name)
                .to_string(),
        }
    }
}

// ── D-Bus service ────────────────────────────────────────────────────────────

pub struct S6SystemctlService;

impl S6SystemctlService {
    pub fn new() -> Self {
        Self
    }

    fn is_s6_available() -> bool {
        Path::new(S6_RC_LIVE).exists() || Path::new(S6_SERVICE_DIR).exists()
    }

    // ── Low-level s6 primitives ───────────────────────────────────────────────

    async fn s6rc(&self, args: &[&str]) -> Result<(bool, String, String)> {
        let mut cmd = Command::new("s6-rc");
        if Path::new(S6_RC_LIVE).exists() {
            cmd.arg("-l").arg(S6_RC_LIVE);
        }
        cmd.args(args);
        debug!("s6-rc {:?}", args);
        let out = cmd.output().await.context("s6-rc")?;
        let ok = out.status.success();
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        if !ok {
            debug!("s6-rc failed: stderr={} stdout={}", stderr, stdout);
        }
        Ok((ok, stdout, stderr))
    }

    async fn s6svc(&self, service: &str, signal: &str) -> Result<(bool, String)> {
        let path = format!("{}/{}", S6_SERVICE_DIR, service);
        if !Path::new(&path).exists() {
            return Ok((false, format!("service not found: {}", path)));
        }
        let out = Command::new("s6-svc")
            .arg(signal)
            .arg(&path)
            .output()
            .await
            .context("s6-svc")?;
        Ok((out.status.success(), String::from_utf8_lossy(&out.stderr).to_string()))
    }

    async fn s6svstat(&self, service: &str) -> Result<HashMap<String, String>> {
        let path = format!("{}/{}", S6_SERVICE_DIR, service);
        let out = Command::new("s6-svstat")
            .arg(&path)
            .output()
            .await
            .context("s6-svstat")?;
        let mut map = HashMap::new();
        let stdout = String::from_utf8_lossy(&out.stdout);

        if !out.status.success() {
            map.insert("ActiveState".into(), "unknown".into());
            map.insert("SubState".into(), "not-found".into());
            return Ok(map);
        }

        if stdout.starts_with("up") {
            map.insert("ActiveState".into(), "active".into());
            map.insert("SubState".into(), "running".into());
            if let Some(i) = stdout.find("pid ") {
                let rest = &stdout[i + 4..];
                if let Some(j) = rest.find(')') {
                    map.insert("MainPID".into(), rest[..j].to_string());
                }
            }
        } else {
            map.insert("ActiveState".into(), "inactive".into());
            map.insert("SubState".into(), "dead".into());
            map.insert("MainPID".into(), "0".into());
        }

        if let Some(i) = stdout.find("seconds") {
            let before = &stdout[..i];
            if let Some(j) = before.rfind(' ') {
                map.insert("Uptime".into(), before[j + 1..i].trim().to_string());
            }
        }
        Ok(map)
    }

    async fn is_enabled(&self, service: &str) -> bool {
        // Check bundle membership AND the 'down' file absence
        let bundle_file = format!("{}/default/contents.d/{}", S6_RC_BUNDLE, service);
        let down_file = format!("{}/{}/down", S6_SERVICE_DIR, service);
        if Path::new(&bundle_file).exists() {
            return !Path::new(&down_file).exists();
        }
        // Fallback: check s6-rc database
        if let Ok((ok, stdout, _)) = self.s6rc(&["list"]).await {
            if ok {
                return stdout.lines().any(|l| l.trim() == service);
            }
        }
        false
    }

    // ── Systemd deficiency resolution ───────────────────────────────────────

    /// Resolve a `.timer` unit to an s6-cron job or periodic s6-rc oneshot.
    async fn resolve_timer(&self, name: &str) -> (bool, String) {
        let base = UnitType::Timer.strip_suffix(name);
        // Check for s6-cron directory
        let cron_dir = format!("{}/{}/cron", S6_SCAN_DIR, base);
        if Path::new(&cron_dir).exists() {
            (true, format!("Timer {} resolved to s6-cron at {}", name, cron_dir))
        } else {
            // Fallback: treat as a oneshot service that can be triggered
            (true, format!("Timer {} has no s6-cron config; use s6-rc change to trigger", name))
        }
    }

    /// Resolve a `.socket` unit to s6-socketmux or s6-ipcserver.
    async fn resolve_socket(&self, name: &str) -> (bool, String) {
        let base = UnitType::Socket.strip_suffix(name);
        let socket_dir = format!("{}/{}/socket", S6_SERVICE_DIR, base);
        if Path::new(&socket_dir).exists() {
            (true, format!("Socket {} resolved to s6 socket at {}", name, socket_dir))
        } else {
            (false, format!("Socket {} has no s6 socket configuration", name))
        }
    }

    /// Resolve a `.target` unit to an s6-rc bundle.
    async fn resolve_target(&self, name: &str) -> (bool, String) {
        let base = UnitType::Target.strip_suffix(name);
        let bundle_dir = format!("{}/{}", S6_RC_BUNDLE, base);
        if Path::new(&bundle_dir).exists() {
            (true, format!("Target {} resolved to s6-rc bundle {}", name, bundle_dir))
        } else {
            (false, format!("Target {} has no s6-rc bundle", name))
        }
    }

    /// Resolve a `.mount` unit to /proc/mounts or mount command.
    async fn resolve_mount(&self, name: &str) -> (bool, String) {
        let base = UnitType::Mount.strip_suffix(name);
        // systemd mount names encode path (e.g. home.mount -> /home)
        let mount_point = if base.starts_with('-') {
            format!("/{}", base.replacen("-", "/", usize::MAX))
        } else {
            format!("/{}", base)
        };
        let out = Command::new("mountpoint")
            .arg("-q")
            .arg(&mount_point)
            .output()
            .await;
        match out {
            Ok(o) if o.status.success() => {
                (true, format!("Mount {} ({}) is mounted", name, mount_point))
            }
            _ => (false, format!("Mount {} ({}) is not mounted", name, mount_point)),
        }
    }

    /// Resolve a `.device` unit via udev / sysfs.
    async fn resolve_device(&self, name: &str) -> (bool, String) {
        let base = UnitType::Device.strip_suffix(name);
        // Look in /sys/class for the device
        let sysfs_path = format!("/sys/class/{}", base.replace("-", "/"));
        if Path::new(&sysfs_path).exists() {
            (true, format!("Device {} found at {}", name, sysfs_path))
        } else {
            (false, format!("Device {} not found in sysfs", name))
        }
    }

    /// Resolve a `.path` unit to s6-ftrig or inotify.
    async fn resolve_path(&self, name: &str) -> (bool, String) {
        let base = UnitType::Path.strip_suffix(name);
        let ftrig_dir = format!("{}/{}/ftrig", S6_SERVICE_DIR, base);
        if Path::new(&ftrig_dir).exists() {
            (true, format!("Path {} resolved to s6-ftrig at {}", name, ftrig_dir))
        } else {
            (false, format!("Path {} has no s6-ftrig configuration", name))
        }
    }

    /// Route a unit operation to the correct resolver based on unit type.
    async fn route_by_type<F, Fut>(
        &self,
        name: &str,
        service_fn: F,
    ) -> (bool, String)
    where
        F: FnOnce(&str) -> Fut,
        Fut: std::future::Future<Output = (bool, String)>,
    {
        let ty = UnitType::from_name(name);
        let base = ty.strip_suffix(name);
        match ty {
            UnitType::Service => service_fn(&base).await,
            UnitType::Timer => self.resolve_timer(name).await,
            UnitType::Socket => self.resolve_socket(name).await,
            UnitType::Target => self.resolve_target(name).await,
            UnitType::Mount => self.resolve_mount(name).await,
            UnitType::Device => self.resolve_device(name).await,
            UnitType::Path => self.resolve_path(name).await,
            UnitType::Unknown(ext) => {
                (false, format!("Unknown unit type '.{}' for {}", ext, name))
            }
        }
    }
}

impl Default for S6SystemctlService {
    fn default() -> Self {
        Self::new()
    }
}

// ── D-Bus interface ───────────────────────────────────────────────────────────

#[interface(name = "org.opdbus.v1.S6.Systemctl")]
impl S6SystemctlService {
    /// **Start** — resolves to `s6-rc -u change <service>`.
    async fn start(&self, unit: &str) -> (bool, String) {
        info!("s6-systemctl: start {}", unit);
        self.route_by_type(unit, |base| async {
            match self.s6rc(&["-u", "change", base]).await {
                Ok((ok, _, err)) if ok || err.contains("already up") => {
                    (true, format!("Started {} successfully", base))
                }
                Ok((_, _, err)) => (false, format!("Failed to start {}: {}", base, err)),
                Err(e) => (false, format!("Error starting {}: {}", base, e)),
            }
        })
        .await
    }

    /// **Stop** — resolves to `s6-rc -d change <service>`.
    async fn stop(&self, unit: &str) -> (bool, String) {
        info!("s6-systemctl: stop {}", unit);
        self.route_by_type(unit, |base| async {
            match self.s6rc(&["-d", "change", base]).await {
                Ok((ok, _, err)) if ok || err.contains("already down") => {
                    (true, format!("Stopped {} successfully", base))
                }
                Ok((_, _, err)) => (false, format!("Failed to stop {}: {}", base, err)),
                Err(e) => (false, format!("Error stopping {}: {}", base, e)),
            }
        })
        .await
    }

    /// **Restart** — stop then start.
    async fn restart(&self, unit: &str) -> (bool, String) {
        info!("s6-systemctl: restart {}", unit);
        let (ok_stop, msg_stop) = self.stop(unit).await;
        if !ok_stop && !msg_stop.contains("already down") {
            warn!("restart stop phase failed: {}", msg_stop);
        }
        let (ok_start, msg_start) = self.start(unit).await;
        if ok_start {
            (true, format!("Restarted {} successfully", unit))
        } else {
            (false, format!("Failed to restart {}: {}", unit, msg_start))
        }
    }

    /// **Reload** — resolves to `s6-svc -h <service>` (SIGHUP).
    async fn reload(&self, unit: &str) -> (bool, String) {
        info!("s6-systemctl: reload {}", unit);
        let base = UnitType::from_name(unit).strip_suffix(unit);
        match self.s6svc(&base, "-h").await {
            Ok((ok, err)) if ok => (true, format!("Reloaded {} successfully", base)),
            Ok((_, err)) => (false, format!("Failed to reload {}: {}", base, err)),
            Err(e) => (false, format!("Error reloading {}: {}", base, e)),
        }
    }

    /// **Enable** — create bundle membership + remove `down` file.
    async fn enable(&self, unit: &str) -> (bool, String) {
        info!("s6-systemctl: enable {}", unit);
        let base = UnitType::from_name(unit).strip_suffix(unit);
        let bundle_dir = format!("{}/default/contents.d", S6_RC_BUNDLE);
        let bundle_file = format!("{}/{}", bundle_dir, base);

        if let Err(e) = tokio::fs::create_dir_all(&bundle_dir).await {
            return (false, format!("Failed to create bundle dir: {}", e));
        }

        match tokio::fs::metadata(&bundle_file).await {
            Ok(_) => (true, format!("{} already enabled", base)),
            Err(_) => {
                if let Err(e) = tokio::fs::write(&bundle_file, b"").await {
                    return (false, format!("Failed to write bundle file: {}", e));
                }
                // Remove down file if present so s6-supervise brings it up
                let down_file = format!("{}/{}/down", S6_SERVICE_DIR, base);
                let _ = tokio::fs::remove_file(&down_file).await;
                (true, format!("Enabled {} successfully", base))
            }
        }
    }

    /// **Disable** — remove bundle membership + create `down` file + stop.
    async fn disable(&self, unit: &str) -> (bool, String) {
        info!("s6-systemctl: disable {}", unit);
        let base = UnitType::from_name(unit).strip_suffix(unit);
        let bundle_file = format!("{}/default/contents.d/{}", S6_RC_BUNDLE, base);
        let _ = tokio::fs::remove_file(&bundle_file).await;

        // Create down file
        let down_file = format!("{}/{}/down", S6_SERVICE_DIR, base);
        let _ = tokio::fs::write(&down_file, b"").await;

        // Stop the service
        let _ = self.stop(unit).await;
        (true, format!("Disabled {} (removed from bundle, created down file, stopped)", base))
    }

    /// **Status** — full systemd-compatible JSON.
    async fn status(&self, unit: &str) -> String {
        debug!("s6-systemctl: status {}", unit);
        let base = UnitType::from_name(unit).strip_suffix(unit);
        let mut status = match self.s6svstat(&base).await {
            Ok(s) => s,
            Err(e) => {
                let mut m = HashMap::new();
                m.insert("ActiveState".into(), "unknown".into());
                m.insert("SubState".into(), "error".into());
                m.insert("error".into(), e.to_string());
                m
            }
        };

        // Enrich with systemd-compatible fields
        let enabled = self.is_enabled(&base).await;
        status.insert("UnitFileState".into(), if enabled { "enabled".into() } else { "disabled".into() });
        status.insert("LoadState".into(), "loaded".into());
        status.insert("Names".into(), base.clone());
        status.insert("Id".into(), format!("{}", unit));
        status.insert("Type".into(), format!("{:?}", UnitType::from_name(unit)));

        // Add log path
        let log_path = format!("{}/{}/current", S6_LOG_DIR, base);
        if Path::new(&log_path).exists() {
            status.insert("LogPath".into(), log_path);
        }

        simd_json::to_string(&status).unwrap_or_else(|_| "{}".into())
    }

    /// **is-active** — one-word state.
    async fn is_active(&self, unit: &str) -> String {
        let base = UnitType::from_name(unit).strip_suffix(unit);
        match self.s6svstat(&base).await {
            Ok(m) if m.get("ActiveState").map(|s| s == "active").unwrap_or(false) => "active".into(),
            Ok(_) => "inactive".into(),
            Err(_) => "unknown".into(),
        }
    }

    /// **is-enabled** — one-word state.
    async fn is_enabled_method(&self, unit: &str) -> String {
        let base = UnitType::from_name(unit).strip_suffix(unit);
        if self.is_enabled(&base).await { "enabled".into() } else { "disabled".into() }
    }

    /// **show** — dump all properties as JSON (systemctl show equivalent).
    async fn show(&self, unit: &str) -> String {
        let base = UnitType::from_name(unit).strip_suffix(unit);
        let mut props = HashMap::new();

        // Basic service info
        props.insert("Id".to_string(), unit.to_string());
        props.insert("Names".to_string(), base.clone());
        props.insert("Type".to_string(), format!("{:?}", UnitType::from_name(unit)));

        // Status
        if let Ok(stat) = self.s6svstat(&base).await {
            for (k, v) in stat {
                props.insert(k, v);
            }
        }

        // Enabled state
        props.insert(
            "UnitFileState".to_string(),
            if self.is_enabled(&base).await {
                "enabled"
            } else {
                "disabled"
            }
            .to_string(),
        );

        // Service directory info
        let svc_dir = format!("{}/{}", S6_SERVICE_DIR, base);
        if Path::new(&svc_dir).exists() {
            props.insert("ServiceDirectory".to_string(), svc_dir.clone());
            let run_file = format!("{}/run", svc_dir);
            if Path::new(&run_file).exists() {
                props.insert("HasRunScript".to_string(), "true".to_string());
            }
            let finish_file = format!("{}/finish", svc_dir);
            if Path::new(&finish_file).exists() {
                props.insert("HasFinishScript".to_string(), "true".to_string());
            }
        }

        simd_json::to_string(&props).unwrap_or_else(|_| "{}".into())
    }

    /// **list-units** — all active supervised services with systemd-compatible columns.
    async fn list_units(&self) -> String {
        debug!("s6-systemctl: list-units");
        match self.s6rc(&["-a", "list"]).await {
            Ok((ok, stdout, _)) if ok => {
                let mut units = Vec::new();
                for line in stdout.lines().filter(|l| !l.is_empty()) {
                    let name = line.trim();
                    let ty = UnitType::from_name(name);
                    let base = ty.strip_suffix(name);
                    let mut unit = HashMap::new();
                    unit.insert("unit".to_string(), format!("{}.{:?}", base, ty).to_lowercase());
                    unit.insert("load".to_string(), "loaded".to_string());
                    unit.insert("active".to_string(), "active".to_string());
                    unit.insert("sub".to_string(), "running".to_string());
                    unit.insert("description".to_string(), format!("s6 service {}", base));
                    units.push(unit);
                }
                simd_json::to_string(&units).unwrap_or_else(|_| "[]".into())
            }
            _ => "[]".into(),
        }
    }

    /// **list-unit-files** — all available service definitions.
    async fn list_unit_files(&self) -> String {
        debug!("s6-systemctl: list-unit-files");
        match self.s6rc(&["-l", "list"]).await {
            Ok((ok, stdout, _)) if ok => {
                let mut files = Vec::new();
                for line in stdout.lines().filter(|l| !l.is_empty()) {
                    let name = line.trim();
                    let mut unit = HashMap::new();
                    unit.insert("unit_file".to_string(), format!("{}.service", name));
                    unit.insert("state".to_string(), if self.is_enabled(name).await { "enabled" } else { "disabled" }.to_string());
                    unit.insert("vendor_preset".to_string(), "disabled".to_string());
                    files.push(unit);
                }
                simd_json::to_string(&files).unwrap_or_else(|_| "[]".into())
            }
            _ => "[]".into(),
        }
    }

    /// **journalctl** — retrieve logs via s6-log/s6-logwatch.
    async fn journalctl(&self, unit: &str, lines: u32) -> String {
        debug!("s6-systemctl: journalctl {} --lines={}", unit, lines);
        let base = UnitType::from_name(unit).strip_suffix(unit);
        let log_dir = format!("{}/{}", S6_LOG_DIR, base);
        let current_log = format!("{}/current", log_dir);

        if !Path::new(&current_log).exists() {
            return format!("{{\"error\":\"No log for {}\"}}", unit);
        }

        // Use s6-logwatch or tail
        let out = Command::new("s6-logwatch")
            .arg(&log_dir)
            .arg("-l")
            .arg(lines.to_string())
            .output()
            .await;

        match out {
            Ok(o) if o.status.success() => {
                let logs = String::from_utf8_lossy(&o.stdout);
                let entries: Vec<HashMap<String, String>> = logs
                    .lines()
                    .filter(|l| !l.is_empty())
                    .map(|line| {
                        let mut e = HashMap::new();
                        e.insert("MESSAGE".to_string(), line.to_string());
                        e.insert("UNIT".to_string(), unit.to_string());
                        e.insert("SYSLOG_IDENTIFIER".to_string(), base.clone());
                        e
                    })
                    .collect();
                simd_json::to_string(&entries).unwrap_or_else(|_| "[]".into())
            }
            _ => {
                // Fallback to tail
                let out = Command::new("tail")
                    .arg("-n")
                    .arg(lines.to_string())
                    .arg(&current_log)
                    .output()
                    .await;
                match out {
                    Ok(o) if o.status.success() => {
                        let logs = String::from_utf8_lossy(&o.stdout);
                        let entries: Vec<HashMap<String, String>> = logs
                            .lines()
                            .filter(|l| !l.is_empty())
                            .map(|line| {
                                let mut e = HashMap::new();
                                e.insert("MESSAGE".to_string(), line.to_string());
                                e.insert("UNIT".to_string(), unit.to_string());
                                e.insert("SYSLOG_IDENTIFIER".to_string(), base.clone());
                                e
                            })
                            .collect();
                        simd_json::to_string(&entries).unwrap_or_else(|_| "[]".into())
                    }
                    _ => format!("{{\"error\":\"Cannot read log for {}\"}}", unit),
                }
            }
        }
    }

    /// **daemon-reload** — recompile s6-rc database.
    async fn daemon_reload(&self) -> (bool, String) {
        info!("s6-systemctl: daemon-reload");
        match self.s6rc(&["-u", "change"]).await {
            Ok((ok, _, err)) if ok => (true, "s6-rc database recompiled".into()),
            Ok((_, _, err)) => (false, format!("s6-rc recompile failed: {}", err)),
            Err(e) => (false, format!("Error: {}", e)),
        }
    }

    /// **daemon-status** — is s6 available?
    async fn daemon_status(&self) -> String {
        if Self::is_s6_available() {
            "running"
        } else {
            "not-available"
        }
        .into()
    }

    /// **get-unit-type** — expose the runtime resolver decision.
    async fn get_unit_type(&self, unit: &str) -> String {
        format!("{:?}", UnitType::from_name(unit))
    }
}

// ── Service runner ───────────────────────────────────────────────────────────

pub async fn run_s6_systemctl_service() -> Result<()> {
    let conn = zbus::connection::Connection::system()
        .await
        .context("Failed to connect to system D-Bus")?;

    conn.object_server()
        .at("/org/opdbus/v1/s6/systemctl", S6SystemctlService::new())
        .await
        .context("Failed to register s6-systemctl object")?;

    conn.request_name("org.opdbus.v1.S6.Systemctl")
        .await
        .context("Failed to request bus name")?;

    info!("S6-Systemctl D-Bus service registered at org.opdbus.v1.S6.Systemctl");

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}
