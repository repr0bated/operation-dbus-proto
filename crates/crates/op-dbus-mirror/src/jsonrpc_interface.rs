//! JSON-RPC D-Bus Interfaces
//!
//! Exposes OVSDB and NonNet JSON-RPC methods as D-Bus interfaces
//! for a true 1:1 mirror of the JSON-RPC API.

use op_jsonrpc::nonnet::NonNetDb;
use op_jsonrpc::ovsdb::OvsdbClient;
use op_jsonrpc::protocol::JsonRpcRequest;
use std::sync::Arc;
use zbus::interface;

/// OVSDB D-Bus interface - mirrors JSON-RPC methods
pub struct OvsdbInterface {
    pub client: Arc<OvsdbClient>,
}

impl OvsdbInterface {
    pub fn new(client: Arc<OvsdbClient>) -> Self {
        Self { client }
    }
}

#[interface(name = "org.opdbus.OvsdbV1")]
impl OvsdbInterface {
    /// Execute JSON-RPC transact on OVSDB
    async fn transact(&self, operations: String) -> zbus::fdo::Result<String> {
        let ops: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut operations.clone()) }
            .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;

        match self.client.transact("Open_vSwitch", ops).await {
            Ok(result) => Ok(simd_json::to_string(&result).unwrap_or_default()),
            Err(e) => Err(zbus::fdo::Error::Failed(e.to_string())),
        }
    }

    /// Get OVSDB schema
    async fn get_schema(&self) -> zbus::fdo::Result<String> {
        match self.client.get_schema("Open_vSwitch").await {
            Ok(result) => Ok(simd_json::to_string(&result).unwrap_or_default()),
            Err(e) => Err(zbus::fdo::Error::Failed(e.to_string())),
        }
    }

    /// List OVSDB databases
    async fn list_dbs(&self) -> zbus::fdo::Result<String> {
        match self.client.list_dbs().await {
            Ok(dbs) => Ok(simd_json::to_string(&dbs).unwrap_or_default()),
            Err(e) => Err(zbus::fdo::Error::Failed(e.to_string())),
        }
    }

    /// Dump entire database
    async fn dump_db(&self) -> zbus::fdo::Result<String> {
        match self.client.dump_db("Open_vSwitch").await {
            Ok(result) => Ok(simd_json::to_string(&result).unwrap_or_default()),
            Err(e) => Err(zbus::fdo::Error::Failed(e.to_string())),
        }
    }

    /// Create bridge
    async fn create_bridge(&self, name: String) -> zbus::fdo::Result<()> {
        self.client
            .create_bridge(&name)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Delete bridge
    async fn delete_bridge(&self, name: String) -> zbus::fdo::Result<()> {
        self.client
            .delete_bridge(&name)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Add port to bridge
    async fn add_port(&self, bridge: String, port: String) -> zbus::fdo::Result<()> {
        self.client
            .add_port(&bridge, &port)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// List bridges
    async fn list_bridges(&self) -> zbus::fdo::Result<String> {
        match self.client.list_bridges().await {
            Ok(bridges) => Ok(simd_json::to_string(&bridges).unwrap_or_default()),
            Err(e) => Err(zbus::fdo::Error::Failed(e.to_string())),
        }
    }

    /// List ports on a bridge
    async fn list_ports(&self, bridge: String) -> zbus::fdo::Result<String> {
        match self.client.list_ports(&bridge).await {
            Ok(ports) => Ok(simd_json::to_string(&ports).unwrap_or_default()),
            Err(e) => Err(zbus::fdo::Error::Failed(e.to_string())),
        }
    }
}

/// NonNet D-Bus interface - mirrors JSON-RPC methods
pub struct NonNetInterface {
    pub nonnet: Arc<NonNetDb>,
}

impl NonNetInterface {
    pub fn new(nonnet: Arc<NonNetDb>) -> Self {
        Self { nonnet }
    }
}

#[interface(name = "org.opdbus.NonNetV1")]
impl NonNetInterface {
    /// Execute JSON-RPC transact on NonNet
    async fn transact(&self, request: String) -> zbus::fdo::Result<String> {
        let req: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut request.clone()) }
            .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;

        let json_req: JsonRpcRequest = simd_json::serde::from_owned_value(req)
            .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;

        let response = self.nonnet.handle_request(json_req).await;
        Ok(simd_json::to_string(&response).unwrap_or_default())
    }

    /// Get NonNet schema
    async fn get_schema(&self) -> zbus::fdo::Result<String> {
        let request =
            op_jsonrpc::protocol::JsonRpcRequest::new("get_schema", simd_json::json!(["OpNonNet"]));
        let response = self.nonnet.handle_request(request).await;
        Ok(simd_json::to_string(&response.result).unwrap_or_default())
    }

    /// List NonNet databases
    async fn list_dbs(&self) -> zbus::fdo::Result<String> {
        let request = op_jsonrpc::protocol::JsonRpcRequest::new("list_dbs", simd_json::json!([]));
        let response = self.nonnet.handle_request(request).await;
        Ok(simd_json::to_string(&response.result).unwrap_or_default())
    }
}
