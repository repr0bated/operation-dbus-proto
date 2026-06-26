//! Shared Unix Domain Socket Listener
//!
//! Provides a single host-side UDS that thousands of containers can connect
//! to. The bridge extracts peer credentials (PID/UID) from each connection
//! and maps them to a container identity (session_id) for routing.
//!
//! ## Architecture
//!
//! ```text
//! Container A ──┐
//! Container B ──┼──► /run/ghostbridge/container.sock ──► Bridge
//! Container C ──┘         (shared UDS)                    │
//!                                                         ├─ SO_PEERCRED → PID/UID
//!                                                         ├─ PID → container name
//!                                                         └─ inject session_id metadata
//! ```
//!
//! The socket path is configurable via `GHOSTBRIDGE_SOCKET_PATH` env var,
//! defaulting to `/run/ghostbridge/container.sock`.

use std::os::unix::net::UCred;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::net::UnixListener;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Default path for the shared container socket.
pub const DEFAULT_SOCKET_PATH: &str = "/run/ghostbridge/container.sock";

/// Peer identity resolved from socket credentials.
#[derive(Debug, Clone)]
pub struct PeerIdentity {
    /// Process ID of the connecting peer.
    pub pid: u32,
    /// User ID of the connecting peer.
    pub uid: u32,
    /// Resolved container name (if the PID belongs to a known container).
    pub container_name: Option<String>,
    /// Session ID derived from the container identity.
    pub session_id: Option<String>,
}

/// Configuration for the shared socket listener.
#[derive(Debug, Clone)]
pub struct SharedSocketConfig {
    /// Filesystem path for the shared UDS.
    pub socket_path: PathBuf,
    /// File permissions (octal) for the socket file.
    pub socket_mode: u32,
}

impl Default for SharedSocketConfig {
    fn default() -> Self {
        let path = std::env::var("GHOSTBRIDGE_SOCKET_PATH")
            .unwrap_or_else(|_| DEFAULT_SOCKET_PATH.to_string());
        Self {
            socket_path: PathBuf::from(path),
            socket_mode: 0o666,
        }
    }
}

/// Manages the shared UDS and peer identity resolution.
pub struct SharedSocketManager {
    config: SharedSocketConfig,
    /// Mapping of container PIDs to container names (refreshed from incus).
    container_pids: Arc<RwLock<std::collections::HashMap<u32, String>>>,
}

impl SharedSocketManager {
    pub fn new(config: SharedSocketConfig) -> Self {
        Self {
            config,
            container_pids: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Create the shared socket, removing any stale file.
    pub async fn bind(&self) -> std::io::Result<UnixListener> {
        let path = &self.config.socket_path;

        // Ensure parent directory exists.
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Remove stale socket file.
        if path.exists() {
            tokio::fs::remove_file(path).await?;
        }

        let listener = UnixListener::bind(path)?;

        // Set permissions so containers can connect.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(self.config.socket_mode);
            std::fs::set_permissions(path, perms)?;
        }

        info!(
            socket_path = %path.display(),
            "Shared container socket bound"
        );

        Ok(listener)
    }

    /// Resolve peer identity from Unix socket credentials.
    pub async fn resolve_peer(&self, cred: UCred) -> PeerIdentity {
        let pid = cred.pid().unwrap_or(0) as u32;
        let uid = cred.uid();

        // Try to resolve the PID to a container name.
        let container_name = self.resolve_container_name(pid).await;
        let session_id = container_name
            .as_ref()
            .map(|name| format!("container:{}", name));

        debug!(
            pid,
            uid,
            container = ?container_name,
            session_id = ?session_id,
            "Resolved peer identity"
        );

        PeerIdentity {
            pid,
            uid,
            container_name,
            session_id,
        }
    }

    /// Refresh the container PID mapping from the system.
    ///
    /// Reads `/proc/<pid>/cgroup` or queries incus to map PIDs to container
    /// names. This is called periodically or on-demand when a new PID is seen.
    pub async fn refresh_container_map(&self) {
        let mut map = self.container_pids.write().await;

        // Strategy: read /proc/*/cgroup and look for incus container cgroup paths.
        // Format: `0::/lxc.payload.<container_name>/...`
        let Ok(proc_entries) = std::fs::read_dir("/proc") else {
            warn!("Cannot read /proc for container PID mapping");
            return;
        };

        map.clear();

        for entry in proc_entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            let Ok(pid) = name_str.parse::<u32>() else {
                continue;
            };

            let cgroup_path = format!("/proc/{}/cgroup", pid);
            if let Ok(cgroup) = std::fs::read_to_string(&cgroup_path) {
                // Look for incus/lxc container cgroup pattern.
                for line in cgroup.lines() {
                    if let Some(container) = extract_container_from_cgroup(line) {
                        map.insert(pid, container);
                        break;
                    }
                }
            }
        }

        debug!(count = map.len(), "Refreshed container PID map");
    }

    /// Resolve a single PID to a container name.
    async fn resolve_container_name(&self, pid: u32) -> Option<String> {
        // First check the cached map.
        {
            let map = self.container_pids.read().await;
            if let Some(name) = map.get(&pid) {
                return Some(name.clone());
            }
        }

        // Try reading the PID's cgroup directly.
        let cgroup_path = format!("/proc/{}/cgroup", pid);
        if let Ok(cgroup) = tokio::fs::read_to_string(&cgroup_path).await {
            for line in cgroup.lines() {
                if let Some(container) = extract_container_from_cgroup(line) {
                    // Cache it.
                    let mut map = self.container_pids.write().await;
                    map.insert(pid, container.clone());
                    return Some(container);
                }
            }
        }

        None
    }
}

/// Extract container name from a cgroup line.
///
/// Patterns:
/// - `0::/lxc.payload.<name>/...` (incus/lxd)
/// - `0::/incus.payload/<name>/...`
/// - `0::/machine.slice/incus-<name>.scope/...`
fn extract_container_from_cgroup(line: &str) -> Option<String> {
    // Pattern: lxc.payload.<name>
    if let Some(idx) = line.find("lxc.payload.") {
        let rest = &line[idx + "lxc.payload.".len()..];
        let name = rest.split('/').next().unwrap_or(rest);
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }

    // Pattern: incus.payload/<name>
    if let Some(idx) = line.find("incus.payload/") {
        let rest = &line[idx + "incus.payload/".len()..];
        let name = rest.split('/').next().unwrap_or(rest);
        if !name.is_empty() {
            return Some(name.to_string());
        }
    }

    // Pattern: incus-<name>.scope
    if let Some(idx) = line.find("incus-") {
        let rest = &line[idx + "incus-".len()..];
        if let Some(end) = rest.find(".scope") {
            let name = &rest[..end];
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_container_from_cgroup() {
        assert_eq!(
            extract_container_from_cgroup("0::/lxc.payload.qdrant/init.scope"),
            Some("qdrant".to_string())
        );
        assert_eq!(
            extract_container_from_cgroup("0::/incus.payload/mycontainer/init.scope"),
            Some("mycontainer".to_string())
        );
        assert_eq!(
            extract_container_from_cgroup(
                "0::/machine.slice/incus-assistant.scope/init.scope"
            ),
            Some("assistant".to_string())
        );
        assert_eq!(extract_container_from_cgroup("0::/user.slice"), None);
    }
}
