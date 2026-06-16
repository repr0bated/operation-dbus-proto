//! gRPC Service for OpenVSwitch Daemon
//!
//! Provides gRPC endpoints that mirror the D-Bus interface for OVSDB operations.
//! This allows remote clients to interact with OVS over gRPC while the daemon
//! maintains D-Bus as the canonical control plane per AGENTS.md.
//!
//! The gRPC service delegates to the same `DaemonState` used by the D-Bus
//! interfaces — zero duplication, zero drift.

use anyhow::Result;
use std::net::SocketAddr;
use tonic::{Request, Response, Status};
use tonic_web;
use tracing::info;

use crate::dbus::DaemonState;

const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("ovsdaemon_descriptor");

/// Generated protobuf types
pub mod proto {
    tonic::include_proto!("ovsdaemon.v1");
}

use proto::ovsdb_service_server::{OvsdbService, OvsdbServiceServer};
use proto::{
    AttachPortRequest, AttachPortResponse, BridgeRequest, BridgeResponse, BridgesRequest,
    BridgesResponse, DatabaseRequest, DatabaseResponse, DetachPortRequest, DetachPortResponse,
    PortRequest, PortResponse, PortsRequest, PortsResponse, StatusRequest, StatusResponse,
};

/// gRPC service implementation — delegates to the shared `DaemonState`.
pub struct OvsdbServiceImpl {
    state: DaemonState,
}

impl OvsdbServiceImpl {
    pub fn new(state: DaemonState) -> Self {
        Self { state }
    }

    /// Helper: acquire the OVSDB client from the shared state.
    async fn ovsdb_client(
        &self,
    ) -> Result<tokio::sync::MutexGuard<'_, Option<rovs_ovsdb::Client>>, Status> {
        self.state
            .get_ovsdb()
            .await
            .map_err(|e| Status::internal(format!("OVSDB not available: {}", e)))
    }
}

#[tonic::async_trait]
impl OvsdbService for OvsdbServiceImpl {
    async fn create_bridge(
        &self,
        request: Request<BridgeRequest>,
    ) -> Result<Response<BridgeResponse>, Status> {
        let name = request.into_inner().name;
        info!("gRPC create_bridge: {}", name);

        let mut guard = self.ovsdb_client().await?;
        let client = guard
            .as_mut()
            .ok_or_else(|| Status::internal("OVSDB client unavailable"))?;

        let ops = serde_json::json!([
            {
                "op": "insert",
                "table": "Bridge",
                "row": { "name": &name, "stp_enable": false },
                "uuid-name": "new_bridge"
            },
            {
                "op": "mutate",
                "table": "Open_vSwitch",
                "where": [],
                "mutations": [["bridges", "insert", ["named-uuid", "new_bridge"]]]
            },
        ]);

        match client.transact(ops).await {
            Ok(result) => {
                // Extract bridge UUID from result if available
                let uuid = result
                    .get("result")
                    .and_then(|r| r.as_array())
                    .and_then(|a| a.first())
                    .and_then(|o| o.get("uuid"))
                    .and_then(|u| u.as_array())
                    .and_then(|a| a.get(1))
                    .and_then(|u| u.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                Ok(Response::new(BridgeResponse {
                    name,
                    success: true,
                    message: "Bridge created".to_string(),
                    uuid,
                }))
            }
            Err(e) => Ok(Response::new(BridgeResponse {
                name,
                success: false,
                message: format!("Failed: {}", e),
                uuid: String::new(),
            })),
        }
    }

    async fn delete_bridge(
        &self,
        request: Request<BridgeRequest>,
    ) -> Result<Response<BridgeResponse>, Status> {
        let name = request.into_inner().name;
        info!("gRPC delete_bridge: {}", name);

        let mut guard = self.ovsdb_client().await?;
        let client = guard
            .as_mut()
            .ok_or_else(|| Status::internal("OVSDB client unavailable"))?;

        let del_ops = serde_json::json!([
            {
                "op": "delete",
                "table": "Bridge",
                "where": [["name", "==", &name]]
            },
            {
                "op": "mutate",
                "table": "Open_vSwitch",
                "where": [],
                "mutations": [["bridges", "delete", ["set", []]]]
            },
        ]);

        match client.transact(del_ops).await {
            Ok(_) => Ok(Response::new(BridgeResponse {
                name,
                success: true,
                message: "Bridge deleted".to_string(),
                uuid: String::new(),
            })),
            Err(e) => Ok(Response::new(BridgeResponse {
                name,
                success: false,
                message: format!("Failed: {}", e),
                uuid: String::new(),
            })),
        }
    }

    async fn list_bridges(
        &self,
        _request: Request<BridgesRequest>,
    ) -> Result<Response<BridgesResponse>, Status> {
        let mut guard = self.ovsdb_client().await?;
        let client = guard
            .as_mut()
            .ok_or_else(|| Status::internal("OVSDB client unavailable"))?;

        let ops = serde_json::json!([{
            "op": "select",
            "table": "Bridge",
            "where": [],
            "columns": ["name"]
        }]);

        match client.transact(ops).await {
            Ok(result) => {
                let bridges: Vec<String> = result
                    .get("rows")
                    .and_then(|r| r.as_array())
                    .cloned()
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|r| r.get("name").and_then(|v| v.as_str()).map(String::from))
                    .collect();
                let count = bridges.len() as i32;
                Ok(Response::new(BridgesResponse { bridges, count }))
            }
            Err(e) => Err(Status::internal(format!("OVSDB query failed: {}", e))),
        }
    }

    async fn add_port(
        &self,
        request: Request<PortRequest>,
    ) -> Result<Response<PortResponse>, Status> {
        let req = request.into_inner();
        let name = req.name;
        let bridge = req.bridge;
        let iface_type = if req.interface_type.is_empty() {
            "system".to_string()
        } else {
            req.interface_type
        };
        info!(
            "gRPC add_port: {} to {} (type={})",
            name, bridge, iface_type
        );

        let mut guard = self.ovsdb_client().await?;
        let client = guard
            .as_mut()
            .ok_or_else(|| Status::internal("OVSDB client unavailable"))?;

        let mut iface_row = serde_json::json!({ "name": &name, "type": &iface_type });

        // Apply any extra options from the PortRequest
        for (key, value) in &req.options {
            iface_row
                .as_object_mut()
                .expect("iface_row is always an object")
                .insert(key.clone(), serde_json::Value::String(value.clone()));
        }

        let ops = serde_json::json!([
            {
                "op": "insert",
                "table": "Interface",
                "row": iface_row,
                "uuid-name": "new_iface"
            },
            {
                "op": "insert",
                "table": "Port",
                "row": {
                    "name": &name,
                    "interfaces": ["set", [["named-uuid", "new_iface"]]]
                },
                "uuid-name": "new_port"
            },
            {
                "op": "mutate",
                "table": "Bridge",
                "where": [["name", "==", &bridge]],
                "mutations": [["ports", "insert", ["named-uuid", "new_port"]]]
            },
        ]);

        match client.transact(ops).await {
            Ok(_) => Ok(Response::new(PortResponse {
                name,
                bridge,
                success: true,
                message: "Port added".to_string(),
                uuid: String::new(),
            })),
            Err(e) => Ok(Response::new(PortResponse {
                name,
                bridge,
                success: false,
                message: format!("Failed: {}", e),
                uuid: String::new(),
            })),
        }
    }

    async fn list_ports(
        &self,
        request: Request<PortsRequest>,
    ) -> Result<Response<PortsResponse>, Status> {
        let bridge = request.into_inner().bridge;
        let mut guard = self.ovsdb_client().await?;
        let client = guard
            .as_mut()
            .ok_or_else(|| Status::internal("OVSDB client unavailable"))?;

        let ops = serde_json::json!([{
            "op": "select",
            "table": "Bridge",
            "where": [["name", "==", &bridge]],
            "columns": ["ports"]
        }]);

        match client.transact(ops).await {
            Ok(result) => {
                let mut ports = Vec::new();
                if let Some(rows) = result.get("rows").and_then(|r| r.as_array()) {
                    if let Some(first) = rows.first() {
                        if let Some(port_set) = first.get("ports").and_then(|p| p.as_array()) {
                            for item in port_set.iter().skip(1) {
                                if let Some(uuid_arr) = item.as_array() {
                                    if uuid_arr.len() == 2 && uuid_arr[0] == "uuid" {
                                        if let Some(uuid) = uuid_arr[1].as_str() {
                                            let name_ops = serde_json::json!([{
                                                "op": "select",
                                                "table": "Port",
                                                "where": [["_uuid", "==", ["uuid", uuid]]],
                                                "columns": ["name"]
                                            }]);
                                            if let Ok(name_result) = client.transact(name_ops).await
                                            {
                                                if let Some(name_rows) = name_result
                                                    .get("rows")
                                                    .and_then(|r| r.as_array())
                                                {
                                                    if let Some(name_row) = name_rows.first() {
                                                        if let Some(port_name) = name_row
                                                            .get("name")
                                                            .and_then(|n| n.as_str())
                                                        {
                                                            ports.push(port_name.to_string());
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                let count = ports.len() as i32;
                Ok(Response::new(PortsResponse {
                    bridge,
                    ports,
                    count,
                }))
            }
            Err(e) => Err(Status::internal(format!("OVSDB query failed: {}", e))),
        }
    }

    async fn remove_port(
        &self,
        request: Request<PortRequest>,
    ) -> Result<Response<PortResponse>, Status> {
        let req = request.into_inner();
        let name = req.name;
        let bridge = req.bridge;
        info!("gRPC remove_port: {} from {}", name, bridge);

        let mut guard = self.ovsdb_client().await?;
        let client = guard
            .as_mut()
            .ok_or_else(|| Status::internal("OVSDB client unavailable"))?;

        let ops = serde_json::json!([
            {
                "op": "mutate",
                "table": "Bridge",
                "where": [["name", "==", &bridge]],
                "mutations": [["ports", "delete", ["name", &name]]]
            },
            {
                "op": "delete",
                "table": "Port",
                "where": [["name", "==", &name]]
            },
            {
                "op": "delete",
                "table": "Interface",
                "where": [["name", "==", &name]]
            },
        ]);

        match client.transact(ops).await {
            Ok(_) => Ok(Response::new(PortResponse {
                name,
                bridge,
                success: true,
                message: "Port removed".to_string(),
                uuid: String::new(),
            })),
            Err(e) => Ok(Response::new(PortResponse {
                name,
                bridge,
                success: false,
                message: format!("Failed: {}", e),
                uuid: String::new(),
            })),
        }
    }

    async fn list_databases(
        &self,
        _request: Request<DatabaseRequest>,
    ) -> Result<Response<DatabaseResponse>, Status> {
        let mut guard = self.ovsdb_client().await?;
        let client = guard
            .as_mut()
            .ok_or_else(|| Status::internal("OVSDB client unavailable"))?;

        match client.list_dbs().await {
            Ok(dbs) => {
                let count = dbs.len() as i32;
                Ok(Response::new(DatabaseResponse {
                    databases: dbs,
                    count,
                }))
            }
            Err(e) => Err(Status::internal(format!("OVSDB query failed: {}", e))),
        }
    }

    async fn get_status(
        &self,
        _request: Request<StatusRequest>,
    ) -> Result<Response<StatusResponse>, Status> {
        let connected = self.state.get_ovsdb().await.is_ok();
        Ok(Response::new(StatusResponse {
            connected,
            version: env!("CARGO_PKG_VERSION").to_string(),
            message: if connected {
                "Daemon connected to OVSDB".to_string()
            } else {
                "Daemon not connected to OVSDB".to_string()
            },
        }))
    }

    // ── Openvswitch proxy: attach/detach OVS internal port to container netns ────

    async fn attach_port_to_netns(
        &self,
        request: Request<AttachPortRequest>,
    ) -> Result<Response<AttachPortResponse>, Status> {
        let req = request.into_inner();
        let port_name = req.port_name;
        let bridge = req.bridge;
        let target_pid = req.target_pid;
        let iface_name = if req.iface_name.is_empty() {
            port_name.clone()
        } else {
            req.iface_name
        };
        info!(
            "gRPC attach_port_to_netns: {} on {} -> PID {} as {}",
            port_name, bridge, target_pid, iface_name
        );

        // Step 1: Create the OVS internal port via OVSDB
        let iface_type = "internal";
        let mut iface_row = serde_json::json!({ "name": &port_name, "type": iface_type });
        for (key, value) in &req.options {
            iface_row
                .as_object_mut()
                .expect("iface_row is always an object")
                .insert(key.clone(), serde_json::Value::String(value.clone()));
        }

        let mut guard = self.ovsdb_client().await?;
        let client = guard
            .as_mut()
            .ok_or_else(|| Status::internal("OVSDB client unavailable"))?;

        let add_ops = serde_json::json!([
            {
                "op": "insert",
                "table": "Interface",
                "row": iface_row,
                "uuid-name": "new_iface"
            },
            {
                "op": "insert",
                "table": "Port",
                "row": {
                    "name": &port_name,
                    "interfaces": ["set", [["named-uuid", "new_iface"]]]
                },
                "uuid-name": "new_port"
            },
            {
                "op": "mutate",
                "table": "Bridge",
                "where": [["name", "==", &bridge]],
                "mutations": [["ports", "insert", ["named-uuid", "new_port"]]]
            },
        ]);

        match client.transact(add_ops).await {
            Ok(_) => info!("OVSDB: internal port {} created on {}", port_name, bridge),
            Err(e) => {
                return Ok(Response::new(AttachPortResponse {
                    port_name,
                    bridge,
                    success: false,
                    message: format!("OVSDB add_port failed: {}", e),
                    iface_name: String::new(),
                }));
            }
        }

        // Drop the OVSDB lock before doing netns operations
        drop(guard);

        // Step 2: Move port into container netns, rename, bring up
        // The OVS internal port is on the host — bring it up first
        crate::netns::set_link_up_rtnetlink(&port_name)
            .map_err(|e| Status::internal(format!("bring up {} on host: {}", port_name, e)))?;

        // Move into container's netns
        match crate::netns::move_interface_to_netns(&port_name, target_pid) {
            Ok(_) => {}
            Err(e) => {
                return Ok(Response::new(AttachPortResponse {
                    port_name,
                    bridge,
                    success: false,
                    message: format!("move to netns failed: {}", e),
                    iface_name: String::new(),
                }));
            }
        }

        // Rename inside the container
        if port_name != iface_name {
            match crate::netns::rename_in_netns(&port_name, &iface_name, target_pid) {
                Ok(_) => {}
                Err(e) => {
                    return Ok(Response::new(AttachPortResponse {
                        port_name,
                        bridge,
                        success: false,
                        message: format!("rename in netns failed: {}", e),
                        iface_name,
                    }));
                }
            }
        }

        // Bring up inside container
        let _ = crate::netns::set_link_up_in_netns(&iface_name, target_pid);

        Ok(Response::new(AttachPortResponse {
            port_name,
            bridge,
            success: true,
            message: "Port attached to container netns".to_string(),
            iface_name,
        }))
    }

    async fn detach_port_from_netns(
        &self,
        request: Request<DetachPortRequest>,
    ) -> Result<Response<DetachPortResponse>, Status> {
        let req = request.into_inner();
        let port_name = req.port_name;
        let bridge = req.bridge;
        info!("gRPC detach_port_from_netns: {} from {}", port_name, bridge);

        let mut guard = self.ovsdb_client().await?;
        let client = guard
            .as_mut()
            .ok_or_else(|| Status::internal("OVSDB client unavailable"))?;

        // Remove from OVSDB — the port may be in a container netns but OVSDB
        // operations are namespace-agnostic
        let del_ops = serde_json::json!([
            {
                "op": "mutate",
                "table": "Bridge",
                "where": [["name", "==", &bridge]],
                "mutations": [["ports", "delete", ["name", &port_name]]]
            },
            {
                "op": "delete",
                "table": "Port",
                "where": [["name", "==", &port_name]]
            },
            {
                "op": "delete",
                "table": "Interface",
                "where": [["name", "==", &port_name]]
            },
        ]);

        match client.transact(del_ops).await {
            Ok(_) => Ok(Response::new(DetachPortResponse {
                port_name,
                bridge,
                success: true,
                message: "Port detached and removed".to_string(),
            })),
            Err(e) => Ok(Response::new(DetachPortResponse {
                port_name,
                bridge,
                success: false,
                message: format!("OVSDB remove failed: {}", e),
            })),
        }
    }
}

/// Run the gRPC server
pub async fn run_grpc_server(addr: SocketAddr, state: DaemonState) -> Result<()> {
    info!("gRPC server starting on {}", addr);

    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build_v1()
        .map_err(|e| anyhow::anyhow!("reflection build failed: {}", e))?;

    let service = OvsdbServiceImpl::new(state);

    tonic::transport::Server::builder()
        .accept_http1(true)
        .add_service(tonic_web::enable(OvsdbServiceServer::new(service)))
        .add_service(tonic_web::enable(reflection))
        .serve(addr)
        .await
        .map_err(|e| anyhow::anyhow!("gRPC server error: {}", e))?;

    Ok(())
}

/// Run the gRPC server with streaming support (M2: gRPC transport + projection wiring)
pub async fn run_grpc_server_with_streaming(
    addr: SocketAddr,
    state: DaemonState,
    streaming_service: crate::grpc_streaming::StreamingService,
) -> Result<()> {
    info!("gRPC server with streaming starting on {}", addr);

    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(FILE_DESCRIPTOR_SET)
        .build_v1()
        .map_err(|e| anyhow::anyhow!("reflection build failed: {}", e))?;

    let base_service = OvsdbServiceImpl::new(state);
    let stream_server = streaming_service.into_server();

    tonic::transport::Server::builder()
        .accept_http1(true)
        .add_service(tonic_web::enable(OvsdbServiceServer::new(base_service)))
        .add_service(tonic_web::enable(stream_server))
        .add_service(tonic_web::enable(reflection))
        .serve(addr)
        .await
        .map_err(|e| anyhow::anyhow!("gRPC server error: {}", e))?;

    Ok(())
}
