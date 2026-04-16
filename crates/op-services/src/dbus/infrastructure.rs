//! D-Bus objects for host infrastructure contracts.
//!
//! These objects describe the socket-based service plane and the separate OVS
//! privacy fabric. Live OVS row state is still projected by op-dbus-mirror.

use std::fs;
use std::path::PathBuf;

use zbus::{interface, Connection};

/// Shared socket directory mounted between host and socket-mode containers.
pub struct SocketPlane {
    name: String,
    host_path: PathBuf,
    container_path: PathBuf,
    owner_group: String,
    mode: String,
    scope: String,
    endpoint_names: Vec<String>,
}

impl SocketPlane {
    pub fn new(
        name: impl Into<String>,
        host_path: impl Into<PathBuf>,
        container_path: impl Into<PathBuf>,
        owner_group: impl Into<String>,
        mode: impl Into<String>,
        scope: impl Into<String>,
        endpoint_names: Vec<String>,
    ) -> Self {
        Self {
            name: name.into(),
            host_path: host_path.into(),
            container_path: container_path.into(),
            owner_group: owner_group.into(),
            mode: mode.into(),
            scope: scope.into(),
            endpoint_names,
        }
    }
}

#[interface(name = "org.opdbus.services.v1.SocketPlane")]
impl SocketPlane {
    #[zbus(property)]
    async fn name(&self) -> &str {
        &self.name
    }

    #[zbus(property)]
    async fn host_path(&self) -> String {
        self.host_path.to_string_lossy().to_string()
    }

    #[zbus(property)]
    async fn container_path(&self) -> String {
        self.container_path.to_string_lossy().to_string()
    }

    #[zbus(property)]
    async fn owner_group(&self) -> &str {
        &self.owner_group
    }

    #[zbus(property)]
    async fn mode(&self) -> &str {
        &self.mode
    }

    #[zbus(property)]
    async fn scope(&self) -> &str {
        &self.scope
    }

    #[zbus(property)]
    async fn exists(&self) -> bool {
        self.host_path.is_dir()
    }

    #[zbus(property)]
    async fn endpoint_names(&self) -> Vec<String> {
        self.endpoint_names.clone()
    }

    async fn describe(&self) -> zbus::fdo::Result<String> {
        let data = simd_json::json!({
            "name": self.name,
            "host_path": self.host_path.to_string_lossy(),
            "container_path": self.container_path.to_string_lossy(),
            "owner_group": self.owner_group,
            "mode": self.mode,
            "scope": self.scope,
            "exists": self.host_path.is_dir(),
            "endpoint_names": self.endpoint_names,
        });
        Ok(simd_json::to_string(&data).unwrap_or_default())
    }
}

/// Socket-mode Incus container contract.
pub struct SocketContainer {
    name: String,
    function: String,
    identity_key: String,
    identity_value: String,
    socket_plane: String,
    socket_mount_path: String,
    network_mode: String,
}

impl SocketContainer {
    pub fn new(
        name: impl Into<String>,
        function: impl Into<String>,
        identity_key: impl Into<String>,
        identity_value: impl Into<String>,
        socket_plane: impl Into<String>,
        socket_mount_path: impl Into<String>,
        network_mode: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            function: function.into(),
            identity_key: identity_key.into(),
            identity_value: identity_value.into(),
            socket_plane: socket_plane.into(),
            socket_mount_path: socket_mount_path.into(),
            network_mode: network_mode.into(),
        }
    }
}

#[interface(name = "org.opdbus.services.v1.SocketContainer")]
impl SocketContainer {
    #[zbus(property)]
    async fn name(&self) -> &str {
        &self.name
    }

    #[zbus(property)]
    async fn function(&self) -> &str {
        &self.function
    }

    #[zbus(property)]
    async fn identity_key(&self) -> &str {
        &self.identity_key
    }

    #[zbus(property)]
    async fn identity_value(&self) -> &str {
        &self.identity_value
    }

    #[zbus(property)]
    async fn socket_plane(&self) -> &str {
        &self.socket_plane
    }

    #[zbus(property)]
    async fn socket_mount_path(&self) -> &str {
        &self.socket_mount_path
    }

    #[zbus(property)]
    async fn network_mode(&self) -> &str {
        &self.network_mode
    }

    #[zbus(property)]
    async fn requires_loopback_only(&self) -> bool {
        self.network_mode == "loopback-only"
    }
}

/// Static contract object for a named OVS privacy-fabric port.
pub struct FabricPort {
    name: String,
    bridge: String,
    dataplane: String,
    purpose: String,
    expected_address: String,
    gateway_path: bool,
    openflow_managed: bool,
    grpc_routable: bool,
}

impl FabricPort {
    pub fn new(
        name: impl Into<String>,
        bridge: impl Into<String>,
        dataplane: impl Into<String>,
        purpose: impl Into<String>,
        expected_address: impl Into<String>,
        gateway_path: bool,
        openflow_managed: bool,
        grpc_routable: bool,
    ) -> Self {
        Self {
            name: name.into(),
            bridge: bridge.into(),
            dataplane: dataplane.into(),
            purpose: purpose.into(),
            expected_address: expected_address.into(),
            gateway_path,
            openflow_managed,
            grpc_routable,
        }
    }
}

#[interface(name = "org.opdbus.services.v1.FabricPort")]
impl FabricPort {
    #[zbus(property)]
    async fn name(&self) -> &str {
        &self.name
    }

    #[zbus(property)]
    async fn bridge(&self) -> &str {
        &self.bridge
    }

    #[zbus(property)]
    async fn dataplane(&self) -> &str {
        &self.dataplane
    }

    #[zbus(property)]
    async fn purpose(&self) -> &str {
        &self.purpose
    }

    #[zbus(property)]
    async fn expected_address(&self) -> &str {
        &self.expected_address
    }

    #[zbus(property)]
    async fn carries_gateway_traffic(&self) -> bool {
        self.gateway_path
    }

    #[zbus(property)]
    async fn openflow_managed(&self) -> bool {
        self.openflow_managed
    }

    #[zbus(property)]
    async fn grpc_routable(&self) -> bool {
        self.grpc_routable
    }

    #[zbus(property)]
    async fn exists(&self) -> bool {
        fs::metadata(format!("/sys/class/net/{}", self.name)).is_ok()
    }
}

/// Register static infrastructure contract objects.
pub async fn register_infrastructure_objects(conn: &Connection) -> anyhow::Result<()> {
    let services0 = SocketPlane::new(
        "services0",
        "/run/services0",
        "/run/services0",
        "_nginx",
        "0770",
        "system-services",
        vec!["gateway".to_string()],
    );
    conn.object_server()
        .at("/org/opdbus/services/socket_planes/services0", services0)
        .await?;

    let services = SocketContainer::new(
        "services",
        "system-services",
        "user.function",
        "system-services",
        "services0",
        "/run/services0",
        "loopback-only",
    );
    conn.object_server()
        .at("/org/opdbus/services/containers/services", services)
        .await?;

    for port in fabric_ports() {
        let path = format!(
            "/org/opdbus/services/fabric_ports/{}",
            dbus_path_segment(&port.name)
        );
        conn.object_server().at(path, port).await?;
    }

    tracing::info!("Registered D-Bus infrastructure contract objects");
    Ok(())
}

fn fabric_ports() -> Vec<FabricPort> {
    vec![
        FabricPort::new(
            "wgcf",
            "ovsbr0",
            "privacy-fabric",
            "Cloudflare WARP tunnel attached to ovsbr0",
            "172.16.0.2/32",
            false,
            true,
            false,
        ),
        FabricPort::new(
            "ovsbr0-mgmt",
            "ovsbr0",
            "privacy-fabric",
            "OVS management internal port",
            "10.200.0.1/24",
            false,
            true,
            false,
        ),
        FabricPort::new(
            "grpc-bridge",
            "ovsbr0",
            "privacy-fabric",
            "gRPC control-plane internal port",
            "10.200.0.2/24",
            false,
            true,
            true,
        ),
        FabricPort::new(
            "ovsbr0-sock",
            "ovsbr0",
            "privacy-fabric",
            "Socket-network anchor, not the current OpenClaw request path",
            "",
            false,
            true,
            false,
        ),
        FabricPort::new(
            "priv_wg",
            "ovsbr0",
            "privacy-fabric",
            "L2-only privacy chain ingress port",
            "",
            false,
            true,
            false,
        ),
        FabricPort::new(
            "priv_warp",
            "ovsbr0",
            "privacy-fabric",
            "L2-only privacy chain middle port",
            "",
            false,
            true,
            false,
        ),
        FabricPort::new(
            "priv_xray",
            "ovsbr0",
            "privacy-fabric",
            "Xray identity and egress port",
            "15.235.37.41/32",
            false,
            true,
            false,
        ),
    ]
}

fn dbus_path_segment(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}
