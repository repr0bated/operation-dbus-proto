//! Open vSwitch Tools

use crate::tool_registry::{Tool, ToolRegistry};
use anyhow::Result;
use async_trait::async_trait;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;

pub async fn register_all(registry: &ToolRegistry) -> Result<usize> {
    registry.register(Arc::new(OvsListBridgesTool)).await?;
    registry.register(Arc::new(OvsShowBridgeTool)).await?;
    registry.register(Arc::new(OvsListPortsTool)).await?;
    registry.register(Arc::new(OvsDumpFlowsTool)).await?;
    registry.register(Arc::new(OvsAddBridgeTool)).await?;
    registry.register(Arc::new(OvsDelBridgeTool)).await?;
    registry.register(Arc::new(OvsAddPortTool)).await?;
    registry.register(Arc::new(OvsDelPortTool)).await?;
    registry.register(Arc::new(OvsAddFlowTool)).await?;
    registry.register(Arc::new(OvsDelFlowsTool)).await?;
    Ok(10)
}

macro_rules! ovs_tool {
    ($name:ident, $tool_name:expr, $desc:expr, $schema:expr, $exec:expr) => {
        pub struct $name;
        
        #[async_trait]
        impl Tool for $name {
            fn name(&self) -> &str { $tool_name }
            fn description(&self) -> &str { $desc }
            fn category(&self) -> &str { "ovs" }
            fn tags(&self) -> Vec<String> { vec!["ovs".into(), "network".into()] }
            fn input_schema(&self) -> Value { $schema }
            async fn execute(&self, input: Value) -> Result<Value> { $exec(input).await }
        }
    };
}

async fn run_ovs_vsctl(args: &[&str]) -> Result<Value> {
    let output = tokio::process::Command::new("ovs-vsctl").args(args).output().await?;
    if output.status.success() {
        Ok(json!({"success": true, "output": String::from_utf8_lossy(&output.stdout).trim()}))
    } else {
        Ok(json!({"success": false, "error": String::from_utf8_lossy(&output.stderr).trim()}))
    }
}

async fn run_ovs_ofctl(args: &[&str]) -> Result<Value> {
    let output = tokio::process::Command::new("ovs-ofctl").args(args).output().await?;
    if output.status.success() {
        Ok(json!({"success": true, "output": String::from_utf8_lossy(&output.stdout).trim()}))
    } else {
        Ok(json!({"success": false, "error": String::from_utf8_lossy(&output.stderr).trim()}))
    }
}

ovs_tool!(OvsListBridgesTool, "ovs_list_bridges", "List all OVS bridges.",
    json!({"type": "object", "properties": {}}),
    |_input: Value| async {
        let result = run_ovs_vsctl(&["list-br"]).await?;
        if let Some(output) = result.get("output").and_then(|v| v.as_str()) {
            let bridges: Vec<&str> = output.lines().filter(|l| !l.is_empty()).collect();
            Ok(json!({"success": true, "bridges": bridges, "count": bridges.len()}))
        } else {
            Ok(result)
        }
    }
);

ovs_tool!(OvsShowBridgeTool, "ovs_show_bridge", "Show OVS bridge details.",
    json!({"type": "object", "properties": {"bridge": {"type": "string"}}, "required": ["bridge"]}),
    |input: Value| async move {
        let bridge = input.get("bridge").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing bridge"))?;
        run_ovs_vsctl(&["show"]).await // Shows all, could filter
    }
);

ovs_tool!(OvsListPortsTool, "ovs_list_ports", "List ports on an OVS bridge.",
    json!({"type": "object", "properties": {"bridge": {"type": "string"}}, "required": ["bridge"]}),
    |input: Value| async move {
        let bridge = input.get("bridge").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing bridge"))?;
        let result = run_ovs_vsctl(&["list-ports", bridge]).await?;
        if let Some(output) = result.get("output").and_then(|v| v.as_str()) {
            let ports: Vec<&str> = output.lines().filter(|l| !l.is_empty()).collect();
            Ok(json!({"success": true, "bridge": bridge, "ports": ports, "count": ports.len()}))
        } else {
            Ok(result)
        }
    }
);

ovs_tool!(OvsDumpFlowsTool, "ovs_dump_flows", "Dump flows from an OVS bridge.",
    json!({"type": "object", "properties": {"bridge": {"type": "string"}}, "required": ["bridge"]}),
    |input: Value| async move {
        let bridge = input.get("bridge").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing bridge"))?;
        run_ovs_ofctl(&["dump-flows", bridge]).await
    }
);

ovs_tool!(OvsAddBridgeTool, "ovs_add_bridge", "Create an OVS bridge.",
    json!({"type": "object", "properties": {"bridge": {"type": "string"}}, "required": ["bridge"]}),
    |input: Value| async move {
        let bridge = input.get("bridge").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing bridge"))?;
        run_ovs_vsctl(&["add-br", bridge]).await
    }
);

ovs_tool!(OvsDelBridgeTool, "ovs_del_bridge", "Delete an OVS bridge.",
    json!({"type": "object", "properties": {"bridge": {"type": "string"}}, "required": ["bridge"]}),
    |input: Value| async move {
        let bridge = input.get("bridge").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing bridge"))?;
        run_ovs_vsctl(&["del-br", bridge]).await
    }
);

ovs_tool!(OvsAddPortTool, "ovs_add_port", "Add a port to an OVS bridge.",
    json!({"type": "object", "properties": {"bridge": {"type": "string"}, "port": {"type": "string"}}, "required": ["bridge", "port"]}),
    |input: Value| async move {
        let bridge = input.get("bridge").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing bridge"))?;
        let port = input.get("port").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing port"))?;
        run_ovs_vsctl(&["add-port", bridge, port]).await
    }
);

ovs_tool!(OvsDelPortTool, "ovs_del_port", "Remove a port from an OVS bridge.",
    json!({"type": "object", "properties": {"bridge": {"type": "string"}, "port": {"type": "string"}}, "required": ["bridge", "port"]}),
    |input: Value| async move {
        let bridge = input.get("bridge").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing bridge"))?;
        let port = input.get("port").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing port"))?;
        run_ovs_vsctl(&["del-port", bridge, port]).await
    }
);

ovs_tool!(OvsAddFlowTool, "ovs_add_flow", "Add a flow to an OVS bridge.",
    json!({"type": "object", "properties": {"bridge": {"type": "string"}, "flow": {"type": "string"}}, "required": ["bridge", "flow"]}),
    |input: Value| async move {
        let bridge = input.get("bridge").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing bridge"))?;
        let flow = input.get("flow").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing flow"))?;
        run_ovs_ofctl(&["add-flow", bridge, flow]).await
    }
);

ovs_tool!(OvsDelFlowsTool, "ovs_del_flows", "Delete flows from an OVS bridge.",
    json!({"type": "object", "properties": {"bridge": {"type": "string"}, "match_str": {"type": "string"}}, "required": ["bridge"]}),
    |input: Value| async move {
        let bridge = input.get("bridge").and_then(|v| v.as_str()).ok_or_else(|| anyhow::anyhow!("Missing bridge"))?;
        if let Some(match_str) = input.get("match_str").and_then(|v| v.as_str()) {
            run_ovs_ofctl(&["del-flows", bridge, match_str]).await
        } else {
            run_ovs_ofctl(&["del-flows", bridge]).await
        }
    }
);
