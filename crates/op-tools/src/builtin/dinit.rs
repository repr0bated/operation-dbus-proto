//! Dinit tools backed by Chimera's D-Bus manager.

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::sync::Arc;
use zbus::{proxy, Connection, Error as ZbusError};

use crate::{Tool, ToolRegistry};

type DinitFlags = HashMap<String, bool>;
type DinitStatusRecord = (String, String, String, String, DinitFlags, u32, i32, i32);
type DinitServiceRecord = (
    String,
    String,
    String,
    String,
    String,
    DinitFlags,
    u32,
    i32,
    i32,
);

pub struct DbusDinitStartServiceTool;
pub struct DbusDinitStopServiceTool;
pub struct DbusDinitStatusTool;
pub struct DbusDinitListServicesTool;

#[proxy(
    interface = "org.chimera.dinit.Manager",
    default_service = "org.chimera.dinit",
    default_path = "/org/chimera/dinit"
)]
trait DinitManager {
    #[zbus(name = "StartService")]
    fn start_service(&self, name: &str, wait_for_load: bool) -> zbus::Result<()>;

    #[zbus(name = "StopService")]
    fn stop_service(
        &self,
        name: &str,
        pin_stopped: bool,
        gentle: bool,
        restart: bool,
    ) -> zbus::Result<()>;

    #[zbus(name = "GetServiceStatus")]
    fn get_service_status(&self, name: &str) -> zbus::Result<DinitStatusRecord>;

    #[zbus(name = "ListServices")]
    fn list_services(&self) -> zbus::Result<Vec<DinitServiceRecord>>;
}

struct DinitDbusClient {
    conn: Connection,
}

impl DinitDbusClient {
    async fn connect() -> Result<Self> {
        let conn = Connection::system()
            .await
            .context("failed to connect to system D-Bus for dinit")?;
        Ok(Self { conn })
    }

    async fn proxy(&self) -> Result<DinitManagerProxy<'_>> {
        DinitManagerProxy::new(&self.conn)
            .await
            .context("failed to create dinit D-Bus proxy")
    }
}

fn is_service_already(error: &ZbusError) -> bool {
    matches!(
        error,
        ZbusError::MethodError(name, _, _)
            if name.as_str() == "org.chimera.dinit.Error.ServiceAlready"
    )
}

fn flags_to_value(flags: &DinitFlags) -> Value {
    simd_json::serde::to_owned_value(flags.clone()).unwrap_or_else(|_| json!({}))
}

fn status_to_json(service: &str, status: &DinitStatusRecord) -> Value {
    json!({
        "service": service,
        "current_state": status.0,
        "target_state": status.1,
        "stop_reason": status.2,
        "resource_state": status.3,
        "flags": flags_to_value(&status.4),
        "pid": status.5,
        "exit_status": status.6,
        "exit_signal": status.7,
        "manager": "dinit",
        "protocol": "dbus"
    })
}

fn service_to_json(service: &DinitServiceRecord) -> Value {
    json!({
        "service": service.0,
        "current_state": service.1,
        "target_state": service.2,
        "stop_reason": service.3,
        "resource_state": service.4,
        "flags": flags_to_value(&service.5),
        "pid": service.6,
        "exit_status": service.7,
        "exit_signal": service.8
    })
}

#[async_trait]
impl Tool for DbusDinitStartServiceTool {
    fn name(&self) -> &str {
        "dbus_dinit_start_service"
    }

    fn description(&self) -> &str {
        "Start a dinit service via D-Bus"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service": { "type": "string", "description": "Dinit service name" }
            },
            "required": ["service"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let service = input
            .get("service")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required parameter: service"))?;

        let client = DinitDbusClient::connect().await?;
        let proxy = client.proxy().await?;
        match proxy.start_service(service, false).await {
            Ok(()) => {}
            Err(error) if is_service_already(&error) => {}
            Err(error) => return Err(anyhow!("failed to start dinit service {service}: {error}")),
        }

        Ok(json!({
            "started": true,
            "service": service,
            "manager": "dinit",
            "protocol": "dbus"
        }))
    }

    fn category(&self) -> &str {
        "dinit"
    }
}

#[async_trait]
impl Tool for DbusDinitStopServiceTool {
    fn name(&self) -> &str {
        "dbus_dinit_stop_service"
    }

    fn description(&self) -> &str {
        "Stop a dinit service via D-Bus"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service": { "type": "string", "description": "Dinit service name" }
            },
            "required": ["service"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let service = input
            .get("service")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required parameter: service"))?;

        let client = DinitDbusClient::connect().await?;
        let proxy = client.proxy().await?;
        match proxy.stop_service(service, false, false, false).await {
            Ok(()) => {}
            Err(error) if is_service_already(&error) => {}
            Err(error) => return Err(anyhow!("failed to stop dinit service {service}: {error}")),
        }

        Ok(json!({
            "stopped": true,
            "service": service,
            "manager": "dinit",
            "protocol": "dbus"
        }))
    }

    fn category(&self) -> &str {
        "dinit"
    }
}

#[async_trait]
impl Tool for DbusDinitStatusTool {
    fn name(&self) -> &str {
        "dbus_dinit_get_service_status"
    }

    fn description(&self) -> &str {
        "Get dinit service status via D-Bus"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service": { "type": "string", "description": "Dinit service name" }
            },
            "required": ["service"]
        })
    }

    async fn execute(&self, input: Value) -> Result<Value> {
        let service = input
            .get("service")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required parameter: service"))?;

        let client = DinitDbusClient::connect().await?;
        let proxy = client.proxy().await?;
        let status = proxy
            .get_service_status(service)
            .await
            .with_context(|| format!("failed to fetch dinit status for {service}"))?;

        Ok(status_to_json(service, &status))
    }

    fn category(&self) -> &str {
        "dinit"
    }
}

#[async_trait]
impl Tool for DbusDinitListServicesTool {
    fn name(&self) -> &str {
        "dbus_dinit_list_services"
    }

    fn description(&self) -> &str {
        "List dinit services via D-Bus"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "required": []
        })
    }

    async fn execute(&self, _input: Value) -> Result<Value> {
        let client = DinitDbusClient::connect().await?;
        let proxy = client.proxy().await?;
        let services = proxy
            .list_services()
            .await
            .context("failed to list dinit services over D-Bus")?;
        let names: Vec<String> = services.iter().map(|service| service.0.clone()).collect();
        let entries: Vec<Value> = services.iter().map(service_to_json).collect();

        Ok(json!({
            "services": names,
            "entries": entries,
            "count": entries.len(),
            "manager": "dinit",
            "protocol": "dbus"
        }))
    }

    fn category(&self) -> &str {
        "dinit"
    }
}

pub async fn register_dinit_tools(registry: &ToolRegistry) -> Result<()> {
    registry
        .register_tool(Arc::new(DbusDinitStartServiceTool))
        .await?;
    registry
        .register_tool(Arc::new(DbusDinitStopServiceTool))
        .await?;
    registry
        .register_tool(Arc::new(DbusDinitStatusTool))
        .await?;
    registry
        .register_tool(Arc::new(DbusDinitListServicesTool))
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{status_to_json, DinitStatusRecord};

    #[test]
    fn test_status_to_json_preserves_service_name() {
        let status: DinitStatusRecord = (
            "started".into(),
            "started".into(),
            "normal".into(),
            "fds".into(),
            Default::default(),
            42,
            0,
            0,
        );

        let json = status_to_json("ovsdb-server", &status);
        assert_eq!(json["service"].as_str(), Some("ovsdb-server"));
        assert_eq!(json["current_state"].as_str(), Some("started"));
        assert_eq!(json["pid"].as_u64(), Some(42));
    }
}
