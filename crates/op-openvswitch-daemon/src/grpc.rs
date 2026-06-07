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
use tracing::info;

use crate::dbus::DaemonState;

/// Generated protobuf types
pub mod proto {
    tonic::include_proto!("ovsdaemon.v1");
}

use proto::ovsdb_service_server::{OvsdbService, OvsdbServiceServer};
use proto::{
    BridgeRequest, BridgeResponse, BridgesRequest, BridgesResponse, DatabaseRequest,
    DatabaseResponse, PortRequest, PortResponse, PortsRequest, PortsResponse, StatusRequest,
    StatusResponse,
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
        info!("gRPC add_port: {} to {}", name, bridge);

        let mut guard = self.ovsdb_client().await?;
        let client = guard
            .as_mut()
            .ok_or_else(|| Status::internal("OVSDB client unavailable"))?;

        let ops = serde_json::json!([
            {
                "op": "insert",
                "table": "Interface",
                "row": { "name": &name, "type": "system" },
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
}

/// Run the gRPC server
pub async fn run_grpc_server(addr: SocketAddr, state: DaemonState) -> Result<()> {
    info!("gRPC server starting on {}", addr);

    let service = OvsdbServiceImpl::new(state);

    tonic::transport::Server::builder()
        .add_service(OvsdbServiceServer::new(service))
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

    let base_service = OvsdbServiceImpl::new(state);
    let stream_server = streaming_service.into_server();

    tonic::transport::Server::builder()
        .add_service(OvsdbServiceServer::new(base_service))
        .add_service(stream_server)
        .serve(addr)
        .await
        .map_err(|e| anyhow::anyhow!("gRPC server error: {}", e))?;

    Ok(())
}
