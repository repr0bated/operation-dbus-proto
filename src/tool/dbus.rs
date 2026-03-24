//! D-Bus introspection tools for the root op-dbus project.

use async_trait::async_trait;
use serde_json::{json, Value};
use zbus::Connection;
use op_core::types::BusType;
use op_introspection::IntrospectionService;
use crate::error::Result;
use op_execution_tracker::{ExecutionRecord, ExecutionTiming};
use super::{Tool, ToolContext, ToolResult};

pub struct DbusListServicesTool;

#[async_trait]
impl Tool for DbusListServicesTool {
    fn name(&self) -> &'static str {
        "dbus.service.list"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "bus": {
                    "type": "string",
                    "enum": ["system", "session"],
                    "default": "system"
                }
            }
        })
    }

    fn output_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "services": {
                    "type": "array",
                    "items": {"type": "string"}
                }
            }
        })
    }

    fn description(&self) -> &'static str {
        "List all available D-Bus services"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let (start, timing) = ExecutionTiming::capture_start();
        
        let bus = input.get("bus").and_then(|v| v.as_str()).unwrap_or("system");
        
        let connection = if bus == "session" {
            Connection::session().await?
        } else {
            Connection::system().await?
        };

        let proxy = zbus::fdo::DBusProxy::new(&connection).await?;
        let names = proxy.list_names().await?;
        
        let services: Vec<String> = names
            .into_iter()
            .map(|n| n.to_string())
            .filter(|n| !n.starts_with(':'))
            .collect();

        let output = json!({
            "bus": bus,
            "services": services,
            "count": services.len()
        });

        let timing = timing.complete(start);

        let record = ExecutionRecord::builder(self.name())
            .input(input)
            .output(output.clone())
            .policy_id(&ctx.policy_id)
            .plugin_core_hash(&ctx.plugin.core_hash)
            .tunable_hash(&ctx.plugin.tunable_hash)
            .timing(timing)
            .prev_hash(&ctx.prev_hash)
            .build();

        Ok(ToolResult {
            output,
            execution_record: record,
        })
    }
}

pub struct DbusIntrospectTool;

#[async_trait]
impl Tool for DbusIntrospectTool {
    fn name(&self) -> &'static str {
        "dbus.introspect"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service": {"type": "string"},
                "path": {"type": "string", "default": "/"},
                "bus": {
                    "type": "string",
                    "enum": ["system", "session"],
                    "default": "system"
                }
            },
            "required": ["service"]
        })
    }

    fn output_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "introspection": {"type": "object"},
                "interface_count": {"type": "integer"},
                "child_count": {"type": "integer"}
            }
        })
    }

    fn description(&self) -> &'static str {
        "Get D-Bus introspection metadata as JSON for a service and path"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolResult> {
        let (start, timing) = ExecutionTiming::capture_start();
        
        let service = input.get("service").and_then(|v| v.as_str()).unwrap_or("");
        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or("/");
        let bus = input.get("bus").and_then(|v| v.as_str()).unwrap_or("system");
        
        let bus_type = if bus == "session" {
            BusType::Session
        } else {
            BusType::System
        };

        let introspection = IntrospectionService::new();
        let object_info = introspection.introspect(bus_type, service, path).await?;
        let interface_count = object_info.interfaces.len();
        let child_count = object_info.children.len();
        let introspection_value = serde_json::to_value(&object_info)
            .map_err(|e| crate::error::OpDbusError::General(e.to_string()))?;

        let output = json!({
            "bus": bus,
            "service": service,
            "path": path,
            "introspection": introspection_value,
            "interface_count": interface_count,
            "child_count": child_count
        });

        let timing = timing.complete(start);

        let record = ExecutionRecord::builder(self.name())
            .input(input)
            .output(output.clone())
            .policy_id(&ctx.policy_id)
            .plugin_core_hash(&ctx.plugin.core_hash)
            .tunable_hash(&ctx.plugin.tunable_hash)
            .timing(timing)
            .prev_hash(&ctx.prev_hash)
            .build();

        Ok(ToolResult {
            output,
            execution_record: record,
        })
    }
}
