//! JSON-RPC D-Bus Interfaces
//!
//! Exposes OVSDB and NonNet JSON-RPC methods as D-Bus interfaces
//! for a true 1:1 mirror of the JSON-RPC API.
//!
//! Authoritative Path: D-Bus method → SchemaEngine.mutate → RCP Database → EventChain

use op_grpc_bridge::{ChangeType, SchemaEngine};
use op_jsonrpc::nonnet::NonNetDb;
use op_jsonrpc::protocol::JsonRpcRequest;
use op_network::ovsdb::OvsdbClient;
use serde_json::Value;
use std::sync::Arc;
use zbus::interface;

fn str_to_simd(s: &str) -> Result<simd_json::OwnedValue, zbus::fdo::Error> {
    let mut bytes = s.as_bytes().to_vec();
    simd_json::to_owned_value(&mut bytes).map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))
}

/// OVSDB D-Bus interface - mirrors JSON-RPC methods
pub struct OvsdbInterface {
    pub client: Arc<OvsdbClient>,
    pub schema_engine: Option<Arc<SchemaEngine>>,
}

impl OvsdbInterface {
    pub fn new(client: Arc<OvsdbClient>, schema_engine: Option<Arc<SchemaEngine>>) -> Self {
        Self {
            client,
            schema_engine,
        }
    }
}

#[interface(name = "org.opdbus.OvsdbV1")]
impl OvsdbInterface {
    /// Execute JSON-RPC transact on OVSDB
    async fn transact(&self, operations: String) -> zbus::fdo::Result<String> {
        let operations_val = str_to_simd(&operations)?;

        // Route through SchemaEngine for authoritative recording if available
        if let Some(engine) = &self.schema_engine {
            match engine
                .mutate(
                    "net".to_string(),
                    "/org/opdbus/v1/ovsdb".to_string(),
                    ChangeType::MethodCall,
                    Some("transact".to_string()),
                    operations_val,
                    "dbus-client".to_string(),
                    None,
                )
                .await
            {
                Ok(result) => Ok(serde_json::to_string(&result.result).unwrap_or_default()),
                Err(e) => Err(zbus::fdo::Error::Failed(e.to_string())),
            }
        } else {
            match self.client.transact_simd("Open_vSwitch", operations_val).await {
                Ok(result) => Ok(serde_json::to_string(&result).unwrap_or_default()),
                Err(e) => Err(zbus::fdo::Error::Failed(e.to_string())),
            }
        }
    }

    /// Get OVSDB schema (returns list of databases as a proxy for schema info)
    async fn get_schema(&self) -> zbus::fdo::Result<String> {
        match self.client.list_dbs().await {
            Ok(dbs) => Ok(serde_json::to_string(&dbs).unwrap_or_default()),
            Err(e) => Err(zbus::fdo::Error::Failed(e.to_string())),
        }
    }

    /// List OVSDB databases
    async fn list_dbs(&self) -> zbus::fdo::Result<String> {
        match self.client.list_dbs().await {
            Ok(dbs) => Ok(serde_json::to_string(&dbs).unwrap_or_default()),
            Err(e) => Err(zbus::fdo::Error::Failed(e.to_string())),
        }
    }

    /// Dump entire database
    async fn dump_db(&self) -> zbus::fdo::Result<String> {
        match self.client.dump_db("Open_vSwitch").await {
            Ok(result) => Ok(serde_json::to_string(&result).unwrap_or_default()),
            Err(e) => Err(zbus::fdo::Error::Failed(e.to_string())),
        }
    }

    /// Create bridge
    async fn create_bridge(&self, name: String) -> zbus::fdo::Result<()> {
        if let Some(engine) = &self.schema_engine {
            engine
                .mutate(
                    "net".to_string(),
                    "/org/opdbus/v1/ovsdb".to_string(),
                    ChangeType::MethodCall,
                    Some("create_bridge".to_string()),
                    simd_json::json!(name),
                    "dbus-client".to_string(),
                    None,
                )
                .await
                .map(|_| ())
                .map_err(|e: anyhow::Error| zbus::fdo::Error::Failed(e.to_string()))
        } else {
            self.client
                .create_bridge(&name)
                .await
                .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
        }
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
        if let Some(engine) = &self.schema_engine {
            engine
                .mutate(
                    "net".to_string(),
                    "/org/opdbus/v1/ovsdb".to_string(),
                    ChangeType::MethodCall,
                    Some("add_port".to_string()),
                    simd_json::json!([bridge, port]),
                    "dbus-client".to_string(),
                    None,
                )
                .await
                .map(|_| ())
                .map_err(|e: anyhow::Error| zbus::fdo::Error::Failed(e.to_string()))
        } else {
            self.client
                .add_port(&bridge, &port)
                .await
                .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
        }
    }

    /// List bridges
    async fn list_bridges(&self) -> zbus::fdo::Result<String> {
        match self.client.list_bridges().await {
            Ok(bridges) => Ok(serde_json::to_string(&bridges).unwrap_or_default()),
            Err(e) => Err(zbus::fdo::Error::Failed(e.to_string())),
        }
    }

    /// List ports on a bridge
    async fn list_ports(&self, bridge: String) -> zbus::fdo::Result<String> {
        match self.client.list_bridge_ports(&bridge).await {
            Ok(ports) => Ok(serde_json::to_string(&ports).unwrap_or_default()),
            Err(e) => Err(zbus::fdo::Error::Failed(e.to_string())),
        }
    }
}

/// NonNet D-Bus interface - mirrors JSON-RPC methods
pub struct NonNetInterface {
    pub nonnet: Arc<NonNetDb>,
    pub schema_engine: Option<Arc<SchemaEngine>>,
}

impl NonNetInterface {
    pub fn new(nonnet: Arc<NonNetDb>, schema_engine: Option<Arc<SchemaEngine>>) -> Self {
        Self {
            nonnet,
            schema_engine,
        }
    }
}

#[interface(name = "org.opdbus.NonNetV1")]
impl NonNetInterface {
    /// Execute JSON-RPC transact on NonNet
    async fn transact(&self, request: String) -> zbus::fdo::Result<String> {
        let req_simd = str_to_simd(&request)?;
        let req_serde: Value = serde_json::from_str(&request)
            .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;

        let json_req: JsonRpcRequest = serde_json::from_value(req_serde.clone())
            .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;

        if json_req.method == "mutate"
            || json_req.method == "update"
            || json_req.method == "insert"
            || json_req.method == "delete"
        {
            if let Some(engine) = &self.schema_engine {
                match engine
                    .mutate(
                        "nonnet".to_string(),
                        "/org/opdbus/v1/nonnet".to_string(),
                        ChangeType::MethodCall,
                        Some(json_req.method.clone()),
                        req_simd,
                        "dbus-client".to_string(),
                        None,
                    )
                    .await
                {
                    Ok(result) => Ok(serde_json::to_string(&result.result).unwrap_or_default()),
                    Err(e) => Err(zbus::fdo::Error::Failed(e.to_string())),
                }
            } else {
                let response = self.nonnet.handle_request(json_req).await;
                Ok(serde_json::to_string(&response).unwrap_or_default())
            }
        } else {
            let response = self.nonnet.handle_request(json_req).await;
            Ok(serde_json::to_string(&response).unwrap_or_default())
        }
    }

    /// Get NonNet schema
    async fn get_schema(&self) -> zbus::fdo::Result<String> {
        let request =
            op_jsonrpc::protocol::JsonRpcRequest::new("get_schema", simd_json::json!(["OpNonNet"]));
        let response = self.nonnet.handle_request(request).await;
        Ok(serde_json::to_string(&response.result).unwrap_or_default())
    }

    /// List NonNet databases
    async fn list_dbs(&self) -> zbus::fdo::Result<String> {
        let request = op_jsonrpc::protocol::JsonRpcRequest::new("list_dbs", simd_json::json!([]));
        let response = self.nonnet.handle_request(request).await;
        Ok(serde_json::to_string(&response.result).unwrap_or_default())
    }
}
