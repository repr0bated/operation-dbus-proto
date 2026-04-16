//! D-Bus interface for socket service endpoints.
//!
//! Each socket-based service (gateway, future mail, dns) is published as a
//! discoverable D-Bus object at /org/opdbus/services/endpoints/{name}.
//!
//! This replaces hardcoded socket paths in consumers like op-web — they can
//! read the SocketPath property via D-Bus instead.

use std::path::PathBuf;
use tokio::net::UnixStream;
use tracing::debug;
use zbus::{interface, Connection};

/// A socket service endpoint published on D-Bus.
pub struct SocketEndpoint {
    /// Human-readable name (e.g. "gateway", "mail", "dns")
    name: String,
    /// Filesystem path to the Unix socket
    socket_path: PathBuf,
    /// Description of what this endpoint provides
    description: String,
    /// Protocol spoken by the service behind the Unix socket.
    backend_protocol: String,
    /// Network-facing protocol before nginx reaches this socket.
    ingress_protocol: String,
}

impl SocketEndpoint {
    pub fn new(
        name: impl Into<String>,
        socket_path: impl Into<PathBuf>,
        description: impl Into<String>,
        backend_protocol: impl Into<String>,
        ingress_protocol: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            socket_path: socket_path.into(),
            description: description.into(),
            backend_protocol: backend_protocol.into(),
            ingress_protocol: ingress_protocol.into(),
        }
    }
}

#[interface(name = "org.opdbus.services.v1.SocketEndpoint")]
impl SocketEndpoint {
    /// Human-readable endpoint name.
    #[zbus(property)]
    async fn name(&self) -> &str {
        &self.name
    }

    /// Filesystem path to the Unix socket.
    #[zbus(property)]
    async fn socket_path(&self) -> String {
        self.socket_path.to_string_lossy().to_string()
    }

    /// Whether the socket file exists and accepts connections.
    #[zbus(property)]
    async fn available(&self) -> bool {
        if !self.socket_path.exists() {
            return false;
        }
        // Try connecting to verify the socket is alive
        match UnixStream::connect(&self.socket_path).await {
            Ok(_) => true,
            Err(e) => {
                debug!(
                    "Socket {} exists but connect failed: {}",
                    self.socket_path.display(),
                    e
                );
                false
            }
        }
    }

    /// Description of the service behind this socket.
    #[zbus(property)]
    async fn description(&self) -> &str {
        &self.description
    }

    /// Protocol spoken by the backend service over the socket bridge.
    #[zbus(property)]
    async fn backend_protocol(&self) -> &str {
        &self.backend_protocol
    }

    /// Protocol used at the WireGuard/nginx edge before proxying to the socket.
    #[zbus(property)]
    async fn ingress_protocol(&self) -> &str {
        &self.ingress_protocol
    }

    /// Transport type (always "unix-socket" for these endpoints).
    #[zbus(property)]
    async fn transport(&self) -> &str {
        "unix-socket"
    }

    /// Health check — returns "ok" or an error description.
    async fn health_check(&self) -> zbus::fdo::Result<String> {
        if !self.socket_path.exists() {
            return Ok("socket file missing".to_string());
        }
        match UnixStream::connect(&self.socket_path).await {
            Ok(_) => Ok("ok".to_string()),
            Err(e) => Ok(format!("connect failed: {}", e)),
        }
    }
}

/// Register the well-known socket endpoints on the D-Bus connection.
///
/// Currently registers:
/// - /org/opdbus/services/endpoints/gateway  (OpenClaw gateway)
///
/// Future services (mail.sock, dns.sock) will be added here.
pub async fn register_socket_endpoints(conn: &Connection) -> anyhow::Result<()> {
    let gateway = SocketEndpoint::new(
        "gateway",
        "/run/services0/gateway.sock",
        "OpenClaw WebSocket gateway (socat bridge to 127.0.0.1:18789 inside services container)",
        "http-websocket",
        "https-websocket",
    );

    conn.object_server()
        .at("/org/opdbus/services/endpoints/gateway", gateway)
        .await?;

    tracing::info!("Registered D-Bus socket endpoint: /org/opdbus/services/endpoints/gateway");
    Ok(())
}
