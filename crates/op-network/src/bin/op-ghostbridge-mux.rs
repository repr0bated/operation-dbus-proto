//! Ghostbridge container socket mux daemon.
//!
// containers ──▶ /run/ghostbridge/container.sock ──▶ mux (relay netns, one veth) ──▶ OVS ──▶ tags
//!
//! Owns the single shared UDS. Per connection:
//!   1. SO_PEERCRED → kernel-attested uid
//!   2. Look up the live schema registration through D-Bus (uid_base range match)
//!   3. mode=control  → forward to gRPC bridge internal socket
//!      mode=governed → bind(allocated_addr:port), connect(target), relay bytes
//!
//! Replaces the socat fleet with one supervised daemon. Run inside the relay
//! netns via `ip netns exec relay` in the s6 run script — UDS listening is
//! mount-namespace scoped (unaffected by netns), outbound TCP originates from
//! the relay netns's veth into OVS.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;
use tokio::io;
use tokio::net::{TcpSocket, UnixListener, UnixStream};
use tracing::{error, info, warn};

/// Default path for the shared container socket (the front door).
const DEFAULT_SOCKET_PATH: &str = "/run/ghostbridge/container.sock";

/// Internal socket where the gRPC bridge listens after mux takes over container.sock.
const DEFAULT_GRPC_BRIDGE_SOCKET: &str = "/run/ghostbridge/grpc-bridge.sock";

const PLUGIN_BUS: &str = "org.opdbus.v1.plugins";
const UNIX_SOCKET_PATH: &str = "/org/opdbus/v1/plugins/unix_socket";
const PROJECTED_INTERFACE: &str = "org.opdbus.v1.plugins.ProjectedObject";
const DBUS_CALL_TIMEOUT: Duration = Duration::from_secs(5);

/// Incus idmapped container uid range size (65536 uids per container).
const UID_RANGE_SIZE: u32 = 65536;

/// Minimal deserialization of the unix_socket plugin's live registration state.
/// We only need the fields the mux uses — the full SocketEndpoint lives in op-plugins.
#[derive(Debug, Clone, Deserialize)]
struct Registration {
    name: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    uid_base: u32,
    #[serde(default)]
    ports: Vec<u16>,
    #[serde(default)]
    bind_address: Option<String>,
    #[serde(default)]
    target: Option<String>,
}

fn default_mode() -> String {
    "control".to_string()
}

#[derive(Debug, Default, Deserialize)]
struct SocketState {
    #[serde(default)]
    sockets: Vec<Registration>,
}

/// Read the unix_socket plugin's current desired state through its projected
/// D-Bus method. This is deliberately performed per connection so a successful
/// register/unregister mutation is immediately visible without polling.
async fn read_registrations(connection: &zbus::Connection) -> Result<Vec<Registration>> {
    let proxy = zbus::Proxy::new(
        connection,
        PLUGIN_BUS,
        UNIX_SOCKET_PATH,
        PROJECTED_INTERFACE,
    )
    .await
    .context("create unix_socket D-Bus proxy")?;

    let raw: String = tokio::time::timeout(DBUS_CALL_TIMEOUT, async {
        proxy
            .call("CallMethod", &("list_registrations", "{}"))
            .await
    })
    .await
    .context("unix_socket list_registrations timed out")?
    .context("call unix_socket list_registrations")?;

    let state: SocketState =
        serde_json::from_str(&raw).context("decode unix_socket list_registrations response")?;
    Ok(state.sockets)
}

/// Find the registration matching a peer uid (uid_base <= uid < uid_base + 65536).
fn find_registration(regs: &[Registration], uid: u32) -> Option<&Registration> {
    regs.iter()
        .find(|r| uid >= r.uid_base && uid - r.uid_base < UID_RANGE_SIZE)
}

/// Refuse to unlink an active listener. Only remove a stale socket inode left
/// behind after its previous owner exited.
async fn prepare_socket_path(path: &Path) -> Result<()> {
    match UnixStream::connect(path).await {
        Ok(_) => bail!("{} already has an active listener", path.display()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::ConnectionRefused => {}
        Err(error) => {
            return Err(error).with_context(|| format!("probe existing socket {}", path.display()));
        }
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;

        let metadata = tokio::fs::symlink_metadata(path)
            .await
            .with_context(|| format!("inspect stale socket {}", path.display()))?;
        if !metadata.file_type().is_socket() {
            bail!("refusing to remove non-socket path {}", path.display());
        }
    }

    tokio::fs::remove_file(path)
        .await
        .with_context(|| format!("remove stale socket {}", path.display()))
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("op_ghostbridge_mux=info".parse()?)
                .add_directive("info".parse()?),
        )
        .init();

    let socket_path = std::env::var("GHOSTBRIDGE_SOCKET_PATH")
        .unwrap_or_else(|_| DEFAULT_SOCKET_PATH.to_string());
    let grpc_bridge_path = std::env::var("GRPC_BRIDGE_SOCKET")
        .unwrap_or_else(|_| DEFAULT_GRPC_BRIDGE_SOCKET.to_string());

    let dbus = zbus::Connection::system()
        .await
        .context("connect ghostbridge mux to system D-Bus")?;

    // Ensure parent dir exists and remove only a stale socket file.
    let path = PathBuf::from(&socket_path);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    prepare_socket_path(&path).await?;

    let listener = UnixListener::bind(&path).with_context(|| format!("bind {}", socket_path))?;

    // World-writable so containers can connect.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o666))?;
    }

    info!(socket = %socket_path, grpc_bridge = %grpc_bridge_path, "ghostbridge mux listening");

    loop {
        match listener.accept().await {
            Ok((conn, _)) => {
                let grpc = grpc_bridge_path.clone();
                let dbus = dbus.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(conn, &grpc, &dbus).await {
                        warn!(error = %e, "connection handler error");
                    }
                });
            }
            Err(e) => error!(error = %e, "accept failed"),
        }
    }
}

async fn handle_connection(
    conn: UnixStream,
    grpc_bridge_path: &str,
    dbus: &zbus::Connection,
) -> Result<()> {
    let cred = conn.peer_cred().context("getpeercred")?;
    let uid = cred.uid();

    let regs = read_registrations(dbus).await?;
    let reg = find_registration(&regs, uid);

    match reg {
        None => {
            // No schema row: default to control mode (forward to gRPC bridge).
            // This preserves backward compatibility for containers not yet in the schema.
            info!(uid, "no schema row, forwarding as control");
            forward_to_grpc(conn, grpc_bridge_path).await
        }
        Some(r) if r.mode == "governed" => {
            info!(uid, name = %r.name, mode = "governed", "proxied connection");
            proxy_governed(conn, r).await
        }
        Some(r) => {
            info!(uid, name = %r.name, mode = "control", "control connection");
            forward_to_grpc(conn, grpc_bridge_path).await
        }
    }
}

/// Control mode: forward to the gRPC bridge on its internal socket.
async fn forward_to_grpc(conn: UnixStream, grpc_bridge_path: &str) -> Result<()> {
    let backend = UnixStream::connect(grpc_bridge_path)
        .await
        .with_context(|| format!("connect grpc bridge {}", grpc_bridge_path))?;
    relay(conn, backend).await
}

/// Governed mode: bind to the identity's allocated address, connect to target, relay.
async fn proxy_governed(conn: UnixStream, reg: &Registration) -> Result<()> {
    let bind_addr = reg
        .bind_address
        .as_ref()
        .ok_or_else(|| anyhow!("governed registration {} has no bind_address", reg.name))?;
    let target = reg
        .target
        .as_ref()
        .ok_or_else(|| anyhow!("governed registration {} has no target", reg.name))?;
    let port = reg
        .ports
        .first()
        .ok_or_else(|| anyhow!("governed registration {} has no ports", reg.name))?;

    let bind_ip: std::net::IpAddr = bind_addr
        .parse()
        .with_context(|| format!("parse bind_address {}", bind_addr))?;
    let bind_sa = SocketAddr::new(bind_ip, *port);

    // Parse target: "unix:/path" or "tcp:host:port"
    if let Some(path) = target.strip_prefix("unix:") {
        // Unix target: direct connect, no bind (host-local, no OVS tagging).
        let backend = UnixStream::connect(path)
            .await
            .with_context(|| format!("connect unix target {}", path))?;
        info!(name = %reg.name, target = %path, "governed unix relay started");
        relay(conn, backend).await
    } else if let Some(addr) = target.strip_prefix("tcp:") {
        // TCP target: bind to allocated address first, then connect.
        // Outbound traffic originates from the relay netns's veth into OVS,
        // where table 0 classifies on nw_src/tp_src and stamps reg0.
        let target_sa: SocketAddr = addr
            .parse()
            .with_context(|| format!("parse tcp target {}", addr))?;
        let socket = TcpSocket::new_v4().context("create outbound tcp socket")?;
        socket
            .bind(bind_sa)
            .with_context(|| format!("bind outbound to {}", bind_sa))?;
        let backend = socket
            .connect(target_sa)
            .await
            .with_context(|| format!("connect tcp target {}", target_sa))?;
        info!(name = %reg.name, bind = %bind_sa, target = %target_sa, "governed tcp relay started");
        relay(conn, backend).await
    } else {
        bail!(
            "unsupported target format: {} (expected unix:/path or tcp:host:port)",
            target
        )
    }
}

/// Bidirectional byte relay between two streams. Returns when either direction closes.
async fn relay<A, B>(a: A, b: B) -> Result<()>
where
    A: io::AsyncRead + io::AsyncWrite + Unpin,
    B: io::AsyncRead + io::AsyncWrite + Unpin,
{
    let (mut a_read, mut a_write) = io::split(a);
    let (mut b_read, mut b_write) = io::split(b);

    tokio::select! {
        r = io::copy(&mut a_read, &mut b_write) => {
            r.context("relay a→b")?;
        }
        r = io::copy(&mut b_read, &mut a_write) => {
            r.context("relay b→a")?;
        }
    }
    Ok(())
}
