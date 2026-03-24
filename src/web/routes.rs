use axum::{
    Router,
    routing::{get, post},
    extract::State,
    response::Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use anyhow::{self, Result};
use zbus::zvariant::OwnedValue as ZOwnedValue;

use crate::mcp::{McpCompactDispatcher, McpRequest, McpResponse};
use crate::json_rpc::{self, JsonRpcResponse};
use crate::plugins;
use crate::chatbot::Chatbot;

#[cfg(feature = "dev-antigravity")]
use crate::antigravity::InstanceRegistry;

use tower_http::services::ServeDir;

#[derive(Clone)]
pub struct AppState {
    pub dispatcher: Arc<McpCompactDispatcher>,
    pub chatbot: Option<Arc<Chatbot>>,
    #[cfg(feature = "dev-antigravity")]
    pub instance_registry: Arc<InstanceRegistry>,
}

pub fn create_router(state: AppState) -> Router {
    let router = Router::new()
        .nest_service("/", ServeDir::new("op-dbus-ui/dist"))
        .route("/api", get(root))
        .route("/health", get(health))
        .route("/mcp", post(handle_mcp))
        // JSON-RPC namespace for org.opdbus.* schemas
        .route("/jsonrpc", post(handle_jsonrpc))
        .route("/rpc", post(handle_jsonrpc))
        .route("/tools", get(list_tools))
        .route("/chat", post(handle_chat))
        .route("/api/chat", post(handle_chat))
        .route("/api/services", get(get_systemd_services))
        .route("/api/dbus/services", get(get_dbus_services))
        .route("/api/system/diagnostics", get(get_system_diagnostics))
        .route("/api/tools/execute", post(execute_tool));
    
    #[cfg(feature = "dev-antigravity")]
    let router = router.route("/api/instances", get(list_instances));
    
    router
        .fallback_service(ServeDir::new("op-dbus-ui/dist"))
        .with_state(state)
}

async fn root() -> Json<Value> {
    Json(json!({
        "name": "op-dbus",
        "version": env!("CARGO_PKG_VERSION"),
        "status": "running"
    }))
}

async fn health() -> Json<Value> {
    Json(json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

async fn handle_mcp(
    State(state): State<AppState>,
    Json(request): Json<McpRequest>,
) -> Json<McpResponse> {
    Json(state.dispatcher.handle_request(request).await)
}

async fn handle_jsonrpc(
    State(_state): State<AppState>,
    Json(request): Json<Value>,
) -> Json<JsonRpcResponse> {
    let owned_value = match simd_json::serde::to_owned_value(request) {
        Ok(v) => v,
        Err(e) => {
            return Json(JsonRpcResponse::error_with_code(
                simd_json::json!(null),
                json_rpc::error_codes::INVALID_REQUEST,
                format!("Invalid request: {}", e),
            ));
        }
    };

    let req = match json_rpc::parse_request(owned_value) {
        Ok(req) => req,
        Err(err) => return Json(err),
    };

    let id = req.id.clone();

    let method = req.method.as_str();
    let plugin = method.strip_prefix("org.opdbus.").and_then(|rest| {
        rest.split_once('.').map(|(p, suffix)| (p, suffix))
    });

    if let Some((plugin, suffix)) = plugin {
        if let Some(schema_json) = plugins::get_plugin_schema_json(plugin) {
            let mut schema_str = schema_json.to_string();
            let schema_value: simd_json::OwnedValue = match unsafe { simd_json::from_str(&mut schema_str) } {
                Ok(v) => v,
                Err(e) => {
                    return Json(JsonRpcResponse::error_with_code(
                        id,
                        json_rpc::error_codes::INTERNAL_ERROR,
                        format!("Schema parse error: {}", e),
                    ));
                }
            };

            if suffix == "schema.v1" {
                return Json(JsonRpcResponse::success(id, schema_value));
            }

            if suffix == "call.v1" {
                let params = if req.params.is_null() {
                    simd_json::json!({})
                } else {
                    req.params
                };
                return Json(handle_plugin_call(id, plugin, &schema_value, params).await);
            }

            if suffix == "get.v1" {
                let params = if req.params.is_null() {
                    simd_json::json!({})
                } else {
                    req.params
                };
                return Json(handle_plugin_get(id, plugin, &schema_value, params).await);
            }

            if suffix == "set.v1" {
                let params = if req.params.is_null() {
                    simd_json::json!({})
                } else {
                    req.params
                };
                return Json(handle_plugin_set(id, plugin, &schema_value, params).await);
            }
        } else {
            return Json(JsonRpcResponse::error_with_code(
                id,
                json_rpc::error_codes::NOT_FOUND,
                format!("Unknown plugin: {}", plugin),
            ));
        }
    }

    Json(JsonRpcResponse::error(
        id,
        json_rpc::JsonRpcError::method_not_found(method),
    ))
}

#[derive(Deserialize)]
struct PluginCallParams {
    object_type: String,
    object_id: String,
    method: String,
    #[serde(default)]
    args: Vec<Value>,
    #[serde(default)]
    bus: Option<String>,
}

#[derive(Deserialize)]
struct PluginGetParams {
    object_type: String,
    object_id: String,
    property: String,
    #[serde(default)]
    bus: Option<String>,
}

#[derive(Deserialize)]
struct PluginSetParams {
    object_type: String,
    object_id: String,
    property: String,
    value: Value,
    #[serde(default)]
    bus: Option<String>,
}

async fn handle_plugin_call(
    id: simd_json::OwnedValue,
    plugin: &str,
    schema_value: &simd_json::OwnedValue,
    params: simd_json::OwnedValue,
) -> JsonRpcResponse {
    let params = match simd_json::serde::from_owned_value::<PluginCallParams>(params) {
        Ok(p) => p,
        Err(e) => {
            return JsonRpcResponse::error_with_code(
                id,
                json_rpc::error_codes::INVALID_PARAMS,
                format!("Invalid params: {}", e),
            );
        }
    };

    let (base_path, interface) = match lookup_object_type(schema_value, &params.object_type) {
        Some(info) => info,
        None => {
            return JsonRpcResponse::error_with_code(
                id,
                json_rpc::error_codes::NOT_FOUND,
                format!("Unknown object_type '{}' for plugin '{}'", params.object_type, plugin),
            );
        }
    };

    let path = if params.object_id.is_empty() {
        base_path.clone()
    } else if base_path.ends_with('/') {
        format!("{}{}", base_path, params.object_id)
    } else {
        format!("{}/{}", base_path, params.object_id)
    };

    let bus = params.bus.as_deref().unwrap_or("system");
    let connection = if bus == "session" {
        match zbus::Connection::session().await {
            Ok(c) => c,
            Err(e) => {
                return JsonRpcResponse::error_with_code(
                    id,
                    json_rpc::error_codes::CONNECTION_ERROR,
                    format!("Session bus error: {}", e),
                );
            }
        }
    } else {
        match zbus::Connection::system().await {
            Ok(c) => c,
            Err(e) => {
                return JsonRpcResponse::error_with_code(
                    id,
                    json_rpc::error_codes::CONNECTION_ERROR,
                    format!("System bus error: {}", e),
                );
            }
        }
    };

    let proxy = match zbus::Proxy::new(
        &connection,
        format!("org.opdbus.{}.v1", plugin),
        path.as_str(),
        interface.as_str(),
    )
    .await
    {
        Ok(p) => p,
        Err(e) => {
            return JsonRpcResponse::error_with_code(
                id,
                json_rpc::error_codes::CONNECTION_ERROR,
                format!("D-Bus proxy error: {}", e),
            );
        }
    };

    let zbus_args: Vec<zbus::zvariant::OwnedValue> = match params
        .args
        .iter()
        .map(json_to_owned_value)
        .collect::<Result<Vec<_>, anyhow::Error>>()
    {
        Ok(v) => v,
        Err(e) => {
            return JsonRpcResponse::error_with_code(
                id,
                json_rpc::error_codes::INVALID_PARAMS,
                format!("Argument conversion error: {}", e),
            );
        }
    };

    let result: zbus::zvariant::OwnedValue = match proxy.call(params.method.as_str(), &zbus_args).await {
        Ok(v) => v,
        Err(e) => {
            return JsonRpcResponse::error_with_code(
                id,
                json_rpc::error_codes::TOOL_ERROR,
                format!("D-Bus call error: {}", e),
            );
        }
    };

    let result_json = match simd_json::serde::to_owned_value(&result) {
        Ok(v) => v,
        Err(e) => {
            return JsonRpcResponse::error_with_code(
                id,
                json_rpc::error_codes::INTERNAL_ERROR,
                format!("Result serialization error: {}", e),
            );
        }
    };

    let sig = result.signature().to_string();

    JsonRpcResponse::success(id, simd_json::json!({
        "bus": bus,
        "service": format!("org.opdbus.{}.v1", plugin),
        "path": path,
        "interface": interface,
        "method": params.method,
        "result": { "sig": sig, "value": result_json }
    }))
}

async fn handle_plugin_get(
    id: simd_json::OwnedValue,
    plugin: &str,
    schema_value: &simd_json::OwnedValue,
    params: simd_json::OwnedValue,
) -> JsonRpcResponse {
    let params = match simd_json::serde::from_owned_value::<PluginGetParams>(params) {
        Ok(p) => p,
        Err(e) => {
            return JsonRpcResponse::error_with_code(
                id,
                json_rpc::error_codes::INVALID_PARAMS,
                format!("Invalid params: {}", e),
            );
        }
    };

    let (base_path, interface) = match lookup_object_type(schema_value, &params.object_type) {
        Some(info) => info,
        None => {
            return JsonRpcResponse::error_with_code(
                id,
                json_rpc::error_codes::NOT_FOUND,
                format!("Unknown object_type '{}' for plugin '{}'", params.object_type, plugin),
            );
        }
    };

    let path = if params.object_id.is_empty() {
        base_path.clone()
    } else if base_path.ends_with('/') {
        format!("{}{}", base_path, params.object_id)
    } else {
        format!("{}/{}", base_path, params.object_id)
    };

    let bus = params.bus.as_deref().unwrap_or("system");
    let connection = if bus == "session" {
        match zbus::Connection::session().await {
            Ok(c) => c,
            Err(e) => {
                return JsonRpcResponse::error_with_code(
                    id,
                    json_rpc::error_codes::CONNECTION_ERROR,
                    format!("Session bus error: {}", e),
                );
            }
        }
    } else {
        match zbus::Connection::system().await {
            Ok(c) => c,
            Err(e) => {
                return JsonRpcResponse::error_with_code(
                    id,
                    json_rpc::error_codes::CONNECTION_ERROR,
                    format!("System bus error: {}", e),
                );
            }
        }
    };

    let interface_name = match zbus::names::InterfaceName::try_from(interface.as_str()) {
        Ok(i) => i,
        Err(e) => {
            return JsonRpcResponse::error_with_code(
                id,
                json_rpc::error_codes::INVALID_PARAMS,
                format!("Invalid interface name: {}", e),
            );
        }
    };

    let properties_proxy = match zbus::fdo::PropertiesProxy::builder(&connection)
        .destination(format!("org.opdbus.{}.v1", plugin).as_str())
        .and_then(|b| b.path(path.as_str()))
        .and_then(|b| b.build())
        .await
    {
        Ok(p) => p,
        Err(e) => {
            return JsonRpcResponse::error_with_code(
                id,
                json_rpc::error_codes::CONNECTION_ERROR,
                format!("Properties proxy error: {}", e),
            );
        }
    };

    let value: zbus::zvariant::OwnedValue = match properties_proxy.get(interface_name, params.property.as_str()).await {
        Ok(v) => v,
        Err(e) => {
            return JsonRpcResponse::error_with_code(
                id,
                json_rpc::error_codes::TOOL_ERROR,
                format!("D-Bus property get error: {}", e),
            );
        }
    };

    let value_json = match simd_json::serde::to_owned_value(&value) {
        Ok(v) => v,
        Err(e) => {
            return JsonRpcResponse::error_with_code(
                id,
                json_rpc::error_codes::INTERNAL_ERROR,
                format!("Value serialization error: {}", e),
            );
        }
    };

    JsonRpcResponse::success(id, simd_json::json!({
        "bus": bus,
        "service": format!("org.opdbus.{}.v1", plugin),
        "path": path,
        "interface": interface,
        "property": params.property,
        "value": { "sig": value.signature().to_string(), "value": value_json }
    }))
}

async fn handle_plugin_set(
    id: simd_json::OwnedValue,
    plugin: &str,
    schema_value: &simd_json::OwnedValue,
    params: simd_json::OwnedValue,
) -> JsonRpcResponse {
    let params = match simd_json::serde::from_owned_value::<PluginSetParams>(params) {
        Ok(p) => p,
        Err(e) => {
            return JsonRpcResponse::error_with_code(
                id,
                json_rpc::error_codes::INVALID_PARAMS,
                format!("Invalid params: {}", e),
            );
        }
    };

    let (base_path, interface) = match lookup_object_type(schema_value, &params.object_type) {
        Some(info) => info,
        None => {
            return JsonRpcResponse::error_with_code(
                id,
                json_rpc::error_codes::NOT_FOUND,
                format!("Unknown object_type '{}' for plugin '{}'", params.object_type, plugin),
            );
        }
    };

    let path = if params.object_id.is_empty() {
        base_path.clone()
    } else if base_path.ends_with('/') {
        format!("{}{}", base_path, params.object_id)
    } else {
        format!("{}/{}", base_path, params.object_id)
    };

    let bus = params.bus.as_deref().unwrap_or("system");
    let connection = if bus == "session" {
        match zbus::Connection::session().await {
            Ok(c) => c,
            Err(e) => {
                return JsonRpcResponse::error_with_code(
                    id,
                    json_rpc::error_codes::CONNECTION_ERROR,
                    format!("Session bus error: {}", e),
                );
            }
        }
    } else {
        match zbus::Connection::system().await {
            Ok(c) => c,
            Err(e) => {
                return JsonRpcResponse::error_with_code(
                    id,
                    json_rpc::error_codes::CONNECTION_ERROR,
                    format!("System bus error: {}", e),
                );
            }
        }
    };

    let interface_name = match zbus::names::InterfaceName::try_from(interface.as_str()) {
        Ok(i) => i,
        Err(e) => {
            return JsonRpcResponse::error_with_code(
                id,
                json_rpc::error_codes::INVALID_PARAMS,
                format!("Invalid interface name: {}", e),
            );
        }
    };

    let properties_proxy = match zbus::fdo::PropertiesProxy::builder(&connection)
        .destination(format!("org.opdbus.{}.v1", plugin).as_str())
        .and_then(|b| b.path(path.as_str()))
        .and_then(|b| b.build())
        .await
    {
        Ok(p) => p,
        Err(e) => {
            return JsonRpcResponse::error_with_code(
                id,
                json_rpc::error_codes::CONNECTION_ERROR,
                format!("Properties proxy error: {}", e),
            );
        }
    };

    let value = match json_to_owned_value(&params.value) {
        Ok(v) => v,
        Err(e) => {
            return JsonRpcResponse::error_with_code(
                id,
                json_rpc::error_codes::INVALID_PARAMS,
                format!("Invalid value: {}", e),
            );
        }
    };

    if let Err(e) = properties_proxy.set(interface_name, params.property.as_str(), value).await {
        return JsonRpcResponse::error_with_code(
            id,
            json_rpc::error_codes::TOOL_ERROR,
            format!("D-Bus property set error: {}", e),
        );
    }

    JsonRpcResponse::success(id, simd_json::json!({
        "bus": bus,
        "service": format!("org.opdbus.{}.v1", plugin),
        "path": path,
        "interface": interface,
        "property": params.property,
        "status": "ok"
    }))
}

fn lookup_object_type(
    schema: &simd_json::OwnedValue,
    object_type: &str,
) -> Option<(String, String)> {
    let object_types = schema.get("object_types")?;
    let entry = object_types.get(object_type)?;
    let base_path = entry.get("base_path")?.as_str()?.to_string();
    let interface = entry.get("interface")?.as_str()?.to_string();
    Some((base_path, interface))
}

fn json_to_owned_value(value: &Value) -> Result<zbus::zvariant::OwnedValue> {
    use zbus::zvariant::Str as ZStr;

    if let Some(obj) = value.as_object() {
        if let (Some(sig_val), Some(inner)) = (obj.get("sig"), obj.get("value")) {
            if let Some(sig) = sig_val.as_str() {
                return zvariant_from_sig(sig, inner);
            }
        }
    }

    if let Some(s) = value.as_str() {
        return Ok(zbus::zvariant::OwnedValue::from(ZStr::from(s)));
    }
    if let Some(b) = value.as_bool() {
        return Ok(zbus::zvariant::OwnedValue::from(b));
    }
    if let Some(i) = value.as_i64() {
        return Ok(zbus::zvariant::OwnedValue::from(i));
    }
    if let Some(u) = value.as_u64() {
        return Ok(zbus::zvariant::OwnedValue::from(u));
    }
    if let Some(f) = value.as_f64() {
        return Ok(zbus::zvariant::OwnedValue::from(f));
    }

    Err(anyhow::anyhow!(
        "Unsupported argument type; use string/number/bool or tagged {sig,value}"
    ))
}

fn zvariant_from_sig(sig: &str, value: &Value) -> Result<zbus::zvariant::OwnedValue> {
    use zbus::zvariant::Str as ZStr;

    match sig {
        "s" => value.as_str().map(|v| ZOwnedValue::from(ZStr::from(v)))
            .ok_or_else(|| anyhow::anyhow!("Expected string for sig 's'")),
        "b" => value.as_bool().map(ZOwnedValue::from)
            .ok_or_else(|| anyhow::anyhow!("Expected bool for sig 'b'")),
        "i" => value.as_i64().map(|v| ZOwnedValue::from(v as i32))
            .ok_or_else(|| anyhow::anyhow!("Expected i32 for sig 'i'")),
        "u" => value.as_u64().map(|v| ZOwnedValue::from(v as u32))
            .ok_or_else(|| anyhow::anyhow!("Expected u32 for sig 'u'")),
        "x" => value.as_i64().map(ZOwnedValue::from)
            .ok_or_else(|| anyhow::anyhow!("Expected i64 for sig 'x'")),
        "t" => value.as_u64().map(ZOwnedValue::from)
            .ok_or_else(|| anyhow::anyhow!("Expected u64 for sig 't'")),
        "d" => value.as_f64().map(ZOwnedValue::from)
            .ok_or_else(|| anyhow::anyhow!("Expected f64 for sig 'd'")),
        "o" => value.as_str().map(|v| ZOwnedValue::from(ZStr::from(v)))
            .ok_or_else(|| anyhow::anyhow!("Expected object path for sig 'o'")),
        "g" => value.as_str().map(|v| ZOwnedValue::from(ZStr::from(v)))
            .ok_or_else(|| anyhow::anyhow!("Expected signature string for sig 'g'")),
        "ay" => {
            let arr = value.as_array().ok_or_else(|| anyhow::anyhow!("Expected array for sig 'ay'"))?;
            let bytes: Result<Vec<u8>> = arr.iter()
                .map(|v| v.as_u64().map(|n| n as u8).ok_or_else(|| anyhow::anyhow!("Expected u8 in ay array")))
                .collect();
            Ok(ZOwnedValue::from(bytes?))
        }
        _ if sig.starts_with('a') => {
            let inner = &sig[1..];
            let arr = value.as_array().ok_or_else(|| anyhow::anyhow!("Expected array for sig '{}'", sig))?;
            match inner {
                "s" => {
                    let items: Result<Vec<String>> = arr.iter()
                        .map(|v| v.as_str().map(|s| s.to_string()).ok_or_else(|| anyhow::anyhow!("Expected string in array")))
                        .collect();
                    Ok(ZOwnedValue::from(items?))
                }
                "i" => {
                    let items: Result<Vec<i32>> = arr.iter()
                        .map(|v| v.as_i64().map(|n| n as i32).ok_or_else(|| anyhow::anyhow!("Expected i32 in array")))
                        .collect();
                    Ok(ZOwnedValue::from(items?))
                }
                "u" => {
                    let items: Result<Vec<u32>> = arr.iter()
                        .map(|v| v.as_u64().map(|n| n as u32).ok_or_else(|| anyhow::anyhow!("Expected u32 in array")))
                        .collect();
                    Ok(ZOwnedValue::from(items?))
                }
                "b" => {
                    let items: Result<Vec<bool>> = arr.iter()
                        .map(|v| v.as_bool().ok_or_else(|| anyhow::anyhow!("Expected bool in array")))
                        .collect();
                    Ok(ZOwnedValue::from(items?))
                }
                "d" => {
                    let items: Result<Vec<f64>> = arr.iter()
                        .map(|v| v.as_f64().ok_or_else(|| anyhow::anyhow!("Expected f64 in array")))
                        .collect();
                    Ok(ZOwnedValue::from(items?))
                }
                _ => Err(anyhow::anyhow!("Unsupported array signature '{}'", sig)),
            }
        }
        _ => Err(anyhow::anyhow!("Unsupported signature '{}'", sig)),
    }
}

async fn list_tools(State(state): State<AppState>) -> Json<Value> {
    let request = McpRequest {
        jsonrpc: "2.0".to_string(),
        method: "tools/call".to_string(),
        params: Some(json!({"name": "list_tools", "arguments": {}})),
        id: Some(json!(1)),
    };
    let response = state.dispatcher.handle_request(request).await;
    
    // Extract inner JSON from MCP text response
    if let Some(result) = response.result {
        if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
            if let Some(first) = content.first() {
                if let Some(text) = first.get("text").and_then(|t| t.as_str()) {
                    if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                        return Json(parsed);
                    }
                }
            }
        }
        Json(result)
    } else {
        Json(json!({"error": "Failed to list tools"}))
    }
}

#[derive(Deserialize)]
struct ChatRequest {
    message: String,
    session_id: Option<String>,
}

async fn handle_chat(State(state): State<AppState>, Json(payload): Json<ChatRequest>) -> Json<Value> {
    if let Some(chatbot) = &state.chatbot {
        let session_id = payload.session_id.unwrap_or_else(|| "default".to_string());
        match chatbot.chat(&session_id, &payload.message).await {
            Ok(response) => Json(json!(response)),
            Err(e) => Json(json!({ "error": e.to_string() })),
        }
    } else {
        Json(json!({ "error": "Chatbot not enabled" }))
    }
}

// === System APIs ===

async fn get_systemd_services() -> Json<Value> {
    let result = tokio::process::Command::new("systemctl")
        .args(["list-units", "--type=service", "--no-pager", "--plain", "--no-legend", "-a"])
        .output()
        .await;

    let services: Vec<Value> = match result {
        Ok(output) => {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter_map(|line| {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 4 {
                        let name = parts[0].trim_end_matches(".service");
                        let status = match parts[2] {
                            "active" => "active",
                            "inactive" => "inactive",
                            "failed" => "error",
                            _ => "inactive",
                        };
                        Some(json!({
                            "id": format!("svc-{}", name.replace('.', "-")),
                            "name": name,
                            "status": status,
                            "sub_state": parts[3],
                            "description": parts.get(4..).map(|p| p.join(" ")).unwrap_or_default()
                        }))
                    } else { None }
                })
                .take(50)
                .collect()
        }
        Err(e) => vec![json!({"error": e.to_string()})]
    };
    Json(json!({"services": services, "count": services.len()}))
}

async fn get_dbus_services() -> Json<Value> {
    match zbus::Connection::system().await {
        Ok(conn) => match zbus::fdo::DBusProxy::new(&conn).await {
            Ok(proxy) => match proxy.list_names().await {
                Ok(names) => {
                    let services: Vec<Value> = names.into_iter()
                        .map(|n| n.to_string())
                        .filter(|n| !n.starts_with(':'))
                        .map(|name| {
                            let cat = if name.contains("dbusmcp") || name.contains("op_dbus") {
                                "op-dbus"
                            } else if name.starts_with("org.freedesktop") {
                                "freedesktop"
                            } else { "other" };
                            json!({"name": name, "category": cat, "bus": "system"})
                        }).collect();
                    Json(json!({"services": services, "count": services.len()}))
                }
                Err(e) => Json(json!({"error": e.to_string()}))
            }
            Err(e) => Json(json!({"error": e.to_string()}))
        }
        Err(e) => Json(json!({"error": e.to_string()}))
    }
}

async fn get_system_diagnostics() -> Json<Value> {
    let load = tokio::fs::read_to_string("/proc/loadavg").await.unwrap_or_default();
    let lp: Vec<&str> = load.split_whitespace().collect();
    
    let meminfo = tokio::fs::read_to_string("/proc/meminfo").await.unwrap_or_default();
    let mut mem_total: u64 = 0;
    let mut mem_avail: u64 = 0;
    for line in meminfo.lines() {
        if line.starts_with("MemTotal:") {
            mem_total = line.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        } else if line.starts_with("MemAvailable:") {
            mem_avail = line.split_whitespace().nth(1).and_then(|s| s.parse().ok()).unwrap_or(0);
        }
    }
    let mem_used = mem_total.saturating_sub(mem_avail);
    
    let uptime: f64 = tokio::fs::read_to_string("/proc/uptime").await
        .unwrap_or_default().split_whitespace().next()
        .and_then(|s| s.parse().ok()).unwrap_or(0.0);
    
    let hostname = tokio::fs::read_to_string("/etc/hostname").await
        .unwrap_or_else(|_| "unknown".to_string()).trim().to_string();
    
    let cores = tokio::fs::read_to_string("/proc/cpuinfo").await
        .unwrap_or_default().lines().filter(|l| l.starts_with("processor")).count();

    Json(json!({
        "hostname": hostname,
        "load": {"1min": lp.first().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0),
                 "5min": lp.get(1).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0),
                 "15min": lp.get(2).and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0)},
        "memory": {"total_kb": mem_total, "available_kb": mem_avail, "used_kb": mem_used,
                   "percent_used": if mem_total > 0 { mem_used * 100 / mem_total } else { 0 },
                   "formatted": format!("{:.1}GB / {:.1}GB", mem_used as f64/1024.0/1024.0, mem_total as f64/1024.0/1024.0)},
        "uptime": {"seconds": uptime as u64, "formatted": format_uptime(uptime as u64)},
        "cpu_cores": cores
    }))
}

fn format_uptime(s: u64) -> String {
    let d = s / 86400; let h = (s % 86400) / 3600; let m = (s % 3600) / 60;
    if d > 0 { format!("{}d {}h", d, h) } else if h > 0 { format!("{}h {}m", h, m) } else { format!("{}m", m) }
}

#[derive(Deserialize)]
struct ToolExecRequest { tool_name: String, arguments: Value }

async fn execute_tool(State(state): State<AppState>, Json(p): Json<ToolExecRequest>) -> Json<Value> {
    let request = McpRequest {
        jsonrpc: "2.0".to_string(),
        method: "tools/call".to_string(),
        params: Some(json!({"name": p.tool_name, "arguments": p.arguments})),
        id: Some(json!(1)),
    };
    let resp = state.dispatcher.handle_request(request).await;
    if let Some(err) = resp.error {
        Json(json!({"success": false, "error": err.message}))
    } else {
        Json(json!({"success": true, "result": resp.result}))
    }
}

/// List all connected plugin instances
/// GET /api/instances
#[cfg(feature = "dev-antigravity")]
async fn list_instances(State(state): State<AppState>) -> Json<Value> {
    let instances = state.instance_registry.list();
    let primary = state.instance_registry.primary();
    
    Json(json!({
        "instances": instances,
        "count": instances.len(),
        "primary_id": primary.map(|p| p.instance_id),
    }))
}
