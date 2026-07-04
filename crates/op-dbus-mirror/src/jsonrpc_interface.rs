//! JSON-RPC D-Bus Interfaces
//!
//! for a true 1:1 mirror of the JSON-RPC API.
//!
//! Authoritative Path: D-Bus method → MutationEngine.mutate → RCP Database → EventChain

use op_grpc_bridge::{ChangeType, MutationEngine};
use op_jsonrpc::protocol::JsonRpcRequest;
use op_network::rovs_proxy::OvsdbDbusClient;
use serde_json::Value;
use std::sync::Arc;
use zbus::interface;

fn str_to_simd(s: &str) -> Result<simd_json::OwnedValue, zbus::fdo::Error> {
    let mut bytes = s.as_bytes().to_vec();
    simd_json::to_owned_value(&mut bytes).map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))
}

/// OVSDB D-Bus interface - mirrors JSON-RPC methods at /org/opdbus/v1/mirror/ovsdb
pub struct OvsdbInterface {
    pub client: Arc<OvsdbDbusClient>,
    pub schema_engine: Option<Arc<MutationEngine>>,
}

impl OvsdbInterface {
    pub fn new(client: Arc<OvsdbDbusClient>, schema_engine: Option<Arc<MutationEngine>>) -> Self {
        Self {
            client,
            schema_engine,
        }
    }
}

#[interface(name = "org.opdbus.mirror.OvsdbV1")]
impl OvsdbInterface {
    /// Execute JSON-RPC transact on OVSDB
    async fn transact(&self, operations: String) -> zbus::fdo::Result<String> {
        let operations_val = str_to_simd(&operations)?;

        // Route through MutationEngine for authoritative recording if available
        if let Some(engine) = &self.schema_engine {
            match engine
                .mutate(
                    "net".to_string(),
                    "/org/opdbus/v1/mirror/ovsdb".to_string(),
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
            match self.client.transact_simd(operations_val).await {
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
                    "/org/opdbus/v1/mirror/ovsdb".to_string(),
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
                    "/org/opdbus/v1/mirror/ovsdb".to_string(),
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

    pub schema_engine: Option<Arc<MutationEngine>>,
}

        Self {
            schema_engine,
        }
    }
}

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
                Ok(serde_json::to_string(&response).unwrap_or_default())
            }
        } else {
            Ok(serde_json::to_string(&response).unwrap_or_default())
        }
    }

    async fn get_schema(&self) -> zbus::fdo::Result<String> {
        let request =
        Ok(serde_json::to_string(&response.result).unwrap_or_default())
    }

    async fn list_dbs(&self) -> zbus::fdo::Result<String> {
        let request = op_jsonrpc::protocol::JsonRpcRequest::new("list_dbs", simd_json::json!([]));
        Ok(serde_json::to_string(&response.result).unwrap_or_default())
    }
}
