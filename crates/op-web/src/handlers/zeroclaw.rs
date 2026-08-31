//! ZeroClaw HTTP compatibility handlers and schema-driven UI surface.
//!
//! Port 8080 owns ordinary HTTP. Every model/provider request is adapted into
//! the bridge's schema-declared gRPC method pipeline on port 8090.

use axum::{
    body::Body,
    extract::Extension,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use op_grpc_bridge::GhostbridgeCallMetadata;
use serde::Deserialize;
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::state::AppState;

fn identity_header(headers: &HeaderMap, name: &'static str) -> Result<Option<String>, Response> {
    headers
        .get(name)
        .map(|value| {
            value.to_str().map(str::to_string).map_err(|_| {
                json_error_response(
                    StatusCode::BAD_REQUEST,
                    &format!("invalid {name} header encoding"),
                )
            })
        })
        .transpose()
}

fn ghostbridge_metadata(headers: &HeaderMap) -> Result<GhostbridgeCallMetadata, Response> {
    let footprint = identity_header(headers, "x-ghostbridge-footprint")?.ok_or_else(|| {
        json_error_response(StatusCode::UNAUTHORIZED, "missing x-ghostbridge-footprint")
    })?;
    let trace_id = identity_header(headers, "x-ghostbridge-trace-id")?;
    let wireguard_pubkey = identity_header(headers, "x-wireguard-pubkey")?;
    if trace_id.is_none() && wireguard_pubkey.is_none() {
        return Err(json_error_response(
            StatusCode::UNAUTHORIZED,
            "missing Ghostbridge trace or WireGuard identity",
        ));
    }
    Ok(GhostbridgeCallMetadata {
        footprint,
        trace_id,
        wireguard_pubkey,
    })
}

async fn call_zeroclaw_method(
    headers: &HeaderMap,
    method: &str,
    capability: &str,
    arguments: serde_json::Value,
) -> Result<serde_json::Value, Response> {
    let identity = ghostbridge_metadata(headers)?;
    let mut bytes = serde_json::to_vec(&arguments)
        .map_err(|error| json_error_response(StatusCode::BAD_REQUEST, &error.to_string()))?;
    let arguments = simd_json::to_owned_value(&mut bytes)
        .map_err(|error| json_error_response(StatusCode::BAD_REQUEST, &error.to_string()))?;
    let envelope = crate::state_manager_client::call_plugin_method(
        crate::zeroclaw_routes::ROUTER_PLUGIN_ID,
        method,
        arguments,
        capability,
        &identity,
    )
    .await
    .map_err(|error| {
        let message = error.to_string();
        let status = if message.contains("Unauthenticated") {
            StatusCode::UNAUTHORIZED
        } else if message.contains("Access denied") || message.contains("PermissionDenied") {
            StatusCode::FORBIDDEN
        } else {
            StatusCode::BAD_GATEWAY
        };
        json_error_response(status, &message)
    })?;
    let payload = envelope.get("result").cloned().ok_or_else(|| {
        json_error_response(
            StatusCode::BAD_GATEWAY,
            "ZeroClaw method response is missing its result payload",
        )
    })?;
    let json = simd_json::to_string(&payload)
        .map_err(|error| json_error_response(StatusCode::BAD_GATEWAY, &error.to_string()))?;
    serde_json::from_str(&json)
        .map_err(|error| json_error_response(StatusCode::BAD_GATEWAY, &error.to_string()))
}

#[derive(Debug, Deserialize)]
pub struct OpenAiChatMessage {
    pub role: String,
    #[serde(default)]
    pub content: String,
}

/// Accept both OpenAI `messages` and the legacy single `message` form.
#[derive(Debug, Deserialize)]
pub struct ZeroclawChatRequest {
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub messages: Vec<OpenAiChatMessage>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub stream: bool,
}

/// OpenAI-compatible chat adapter. The bridge remains the sole router.
pub async fn zeroclaw_chat_handler(
    headers: HeaderMap,
    Extension(_state): Extension<Arc<AppState>>,
    Json(req): Json<ZeroclawChatRequest>,
) -> Response {
    if req.stream {
        return json_error_response(
            StatusCode::BAD_REQUEST,
            "use op_chat.chat.ChatService.Send on the gRPC bridge for streaming",
        );
    }
    let messages = if req.messages.is_empty() {
        if req.message.is_empty() {
            return json_error_response(StatusCode::BAD_REQUEST, "messages must not be empty");
        }
        vec![serde_json::json!({"role": "user", "content": req.message})]
    } else {
        req.messages
            .into_iter()
            .map(|message| {
                serde_json::json!({
                    "role": message.role,
                    "content": message.content,
                })
            })
            .collect()
    };
    let result = match call_zeroclaw_method(
        &headers,
        "Chat",
        "cap.software.3tched-router.chat@v1",
        serde_json::json!({
            "messages": messages,
            "provider": req.provider.unwrap_or_default(),
            "model": req.model.unwrap_or_default(),
        }),
    )
    .await
    {
        Ok(result) => result,
        Err(response) => return response,
    };

    let content = result
        .get("content")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let model = result
        .get("model")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let provider = result
        .get("provider")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let finish_reason = result
        .get("finish_reason")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("stop");
    // Salad (and some other providers) emit usage counts as floats (e.g. 30.0).
    // OpenAI-compatible clients like ZeroClaw deserialize them as u64 — coerce.
    let usage = normalize_openai_usage(result.get("usage"));

    Json(serde_json::json!({
        "id": format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        "object": "chat.completion",
        "created": chrono::Utc::now().timestamp(),
        "model": model,
        "provider": provider,
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": content,
            },
            "finish_reason": finish_reason,
        }],
        "usage": usage,
    }))
    .into_response()
}

fn normalize_openai_usage(usage: Option<&serde_json::Value>) -> serde_json::Value {
    let Some(serde_json::Value::Object(map)) = usage else {
        return serde_json::Value::Null;
    };
    let mut out = serde_json::Map::new();
    for key in ["prompt_tokens", "completion_tokens", "total_tokens"] {
        if let Some(v) = map.get(key) {
            let n = v
                .as_u64()
                .or_else(|| v.as_i64().map(|i| i.max(0) as u64))
                .or_else(|| v.as_f64().map(|f| f.max(0.0).round() as u64))
                .unwrap_or(0);
            out.insert(key.to_string(), serde_json::json!(n));
        }
    }
    // Preserve any other usage fields untouched.
    for (k, v) in map {
        out.entry(k.clone()).or_insert_with(|| v.clone());
    }
    serde_json::Value::Object(out)
}

/// Streaming belongs to the bridge's gRPC ChatService, not a second HTTP
/// provider path.
pub async fn zeroclaw_chat_stream_handler(
    Extension(_state): Extension<Arc<AppState>>,
    Json(_req): Json<ZeroclawChatRequest>,
) -> Response {
    json_error_response(
        StatusCode::BAD_REQUEST,
        "use op_chat.chat.ChatService.Send on port 8090 for streaming",
    )
}

/// OpenAI-compatible model catalog backed by `zeroclaw.ListModels`.
pub async fn openai_models_handler(
    headers: HeaderMap,
    Extension(_state): Extension<Arc<AppState>>,
) -> Response {
    let result = match call_zeroclaw_method(
        &headers,
        "ListModels",
        "cap.software.3tched-router.models.read@v1",
        serde_json::json!({}),
    )
    .await
    {
        Ok(result) => result,
        Err(response) => return response,
    };
    let Some(routes) = result
        .get("model_routes")
        .and_then(serde_json::Value::as_array)
    else {
        return json_error_response(
            StatusCode::BAD_GATEWAY,
            "ZeroClaw ListModels returned no model_routes",
        );
    };
    let mut seen = std::collections::HashSet::new();
    let data = routes
        .iter()
        .filter_map(|route| {
            let model = route.get("model")?.as_str()?;
            if !seen.insert(model.to_string()) {
                return None;
            }
            let provider = route
                .get("upstream_provider")
                .and_then(serde_json::Value::as_str)
                .filter(|value| !value.is_empty())
                .or_else(|| route.get("provider").and_then(serde_json::Value::as_str))
                .unwrap_or_default();
            Some(serde_json::json!({
                "id": model,
                "object": "model",
                "owned_by": provider,
                "available": route
                    .get("available")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
                "route_hint": route
                    .get("hint")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default(),
            }))
        })
        .collect::<Vec<_>>();

    Json(serde_json::json!({
        "object": "list",
        "data": data,
    }))
    .into_response()
}

/// Helper to build a JSON error response.
fn json_error_response(status: StatusCode, message: &str) -> Response {
    let body = simd_json::to_string(&json!({"error": message})).unwrap_or_default();
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(body))
        .unwrap_or_else(|_| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .expect("empty body should always succeed")
        })
}

/// Read the zeroclaw `PluginSchema` directly from the sealed blob catalog
/// (Absolute Base). Per AGENTS.md the sealed blob IS the plugin; the monolithic
/// `/dev/shm/live-schema.json` is gone. Live projection values take precedence,
/// while schema field defaults provide the boot-safe provider/model catalog.
fn read_zeroclaw_schema_shm() -> Option<Value> {
    let schema = op_blob::catalog::read_plugin_schema_shm(crate::zeroclaw_routes::ROUTER_PLUGIN_ID)
        .or_else(|| {
            op_blob::catalog::read_plugin_schema_shm(
                crate::zeroclaw_routes::LEGACY_ROUTER_PLUGIN_ID,
            )
        })?;
    // `PluginSchema` serializes to { name, version, fields, methods, … }.
    let mut v: Value = simd_json::to_owned_value(&mut serde_json::to_vec(&schema).ok()?).ok()?;
    // Stamp the catalog_hash so consumers can verify lineage.
    if let Ok(bytes) = std::fs::read("/dev/shm/opdbus/plugin-blobs/.manifest.json") {
        if let Ok(manifest) = serde_json::from_slice::<serde_json::Value>(&bytes) {
            if let Some(hash) = manifest.get("catalog_hash").and_then(|h| h.as_str()) {
                if let Some(obj) = v.as_object_mut() {
                    obj.insert("catalog_hash".to_string(), simd_json::json!(hash));
                }
            }
        }
    }
    Some(v)
}

fn schema_field_default<'a>(schema: &'a Value, field: &str) -> Option<&'a Value> {
    schema.get("fields")?.get(field)?.get("default")
}

fn method_contract(schema: Option<&Value>, method: &str) -> Value {
    schema
        .and_then(|schema| schema.get("methods"))
        .and_then(|methods| methods.get(method))
        .cloned()
        .unwrap_or_else(|| json!({}))
}

/// Build the HTTP compatibility document from the same sealed Schemars method
/// contracts used by D-Bus and gRPC. No external documentation server exists.
fn compatibility_openapi(schema: Option<&Value>) -> Value {
    let chat = method_contract(schema, "Chat");
    let list_models = method_contract(schema, "ListModels");
    let chat_args = chat.get("args").cloned().unwrap_or_else(|| json!({}));

    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "ZeroClaw HTTP compatibility",
            "version": "1.0.0"
        },
        "paths": {
            "/v1/models": {
                "get": {
                    "operationId": "zeroclaw.ListModels",
                    "x-opdbus-method-contract": list_models,
                    "responses": {
                        "200": {
                            "description": "OpenAI-compatible model catalog"
                        }
                    }
                }
            },
            "/v1/chat/completions": {
                "post": {
                    "operationId": "zeroclaw.Chat",
                    "x-opdbus-method-contract": chat,
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": chat_args
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "OpenAI-compatible chat completion"
                        }
                    }
                }
            },
            "/api/zeroclaw/chat": {
                "post": {
                    "operationId": "zeroclaw.Chat",
                    "responses": {
                        "200": {
                            "description": "ZeroClaw chat completion"
                        }
                    }
                }
            },
            "/api/llm/chat": {
                "post": {
                    "operationId": "zeroclaw.Chat",
                    "responses": {
                        "200": {
                            "description": "ZeroClaw chat completion"
                        }
                    }
                }
            }
        }
    })
}

/// Serve the combined schema for schema-driven UI rendering.
///
/// Combines the locally-derived HTTP adapter document with the ZeroClaw plugin
/// projection (providers, model_routes, tools, structured_output) so the
/// frontend can render the entire chat interface from a single schema.
/// GET /api/zeroclaw/schema
pub async fn zeroclaw_schema_handler(Extension(_state): Extension<Arc<AppState>>) -> Response {
    // Read zeroclaw state directly from the SHM state tree. Fall back to the
    // sealed PluginSchema blob so the schema surface still renders when no
    // mutation has populated SHM yet.
    let (zeroclaw, zeroclaw_schema) = match crate::zeroclaw_routes::read_router_plugin() {
        Some(v) => {
            info!("Using tched_router from SHM state tree (live provider/model catalog)");
            (v, read_zeroclaw_schema_shm())
        }
        None => match read_zeroclaw_schema_shm() {
            Some(schema) => {
                warn!("Zeroclaw SHM state unavailable; using PluginSchema only");
                (schema.clone(), Some(schema))
            }
            None => {
                error!("Zeroclaw not available from SHM state tree or PluginSchema");
                return json_error_response(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Zeroclaw projection not available",
                );
            }
        },
    };

    // Providers + model_routes live under `projection` in the live state.
    let projection = zeroclaw.get("projection").and_then(|v| v.as_object());

    // Build a JSON Schema for the chat form from zeroclaw providers + model_routes
    let providers = projection
        .and_then(|p| p.get("providers"))
        .and_then(|v| v.as_array())
        .or_else(|| zeroclaw.get("providers").and_then(|v| v.as_array()))
        .or_else(|| schema_field_default(&zeroclaw, "providers").and_then(|v| v.as_array()))
        .or_else(|| {
            zeroclaw_schema
                .as_ref()
                .and_then(|schema| schema_field_default(schema, "providers"))
                .and_then(|v| v.as_array())
        })
        .map(|arr| {
            arr.iter()
                .filter_map(|p| p.get("id").and_then(|v| v.as_str()).map(String::from))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let model_routes = projection
        .and_then(|p| p.get("model_routes"))
        .and_then(|v| v.as_array())
        .or_else(|| zeroclaw.get("model_routes").and_then(|v| v.as_array()))
        .or_else(|| schema_field_default(&zeroclaw, "model_routes").and_then(|v| v.as_array()))
        .or_else(|| {
            zeroclaw_schema
                .as_ref()
                .and_then(|schema| schema_field_default(schema, "model_routes"))
                .and_then(|v| v.as_array())
        })
        .map(|arr| {
            arr.iter()
                .filter_map(|r| {
                    let model = r.get("model").and_then(|v| v.as_str())?;
                    let hint = r.get("hint").and_then(|v| v.as_str());
                    let provider = r
                        .get("provider")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let available = r
                        .get("available")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);

                    // Build label: "provider/model (hint) [status]"
                    let mut label = format!("{}/{}", provider, model);
                    if let Some(h) = hint {
                        label.push_str(&format!(" ({})", h));
                    }
                    if !available {
                        label.push_str(" [declared]");
                    }

                    Some(json!({"const": model, "title": label}))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let chat_schema = json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object",
        "title": "Zeroclaw Chat",
        "properties": {
            "provider": {
                "type": "string",
                "enum": providers,
                "description": "LLM provider route"
            },
            "model": {
                "type": "string",
                "oneOf": model_routes,
                "description": "Model to use for this turn"
            },
            "message": {
                "type": "string",
                "description": "Chat message",
                "minLength": 1
            }
        },
        "required": ["message"]
    });

    let structured_output = projection
        .and_then(|p| p.get("structured_output"))
        .cloned()
        .or_else(|| zeroclaw.get("structured_output").cloned())
        .or_else(|| schema_field_default(&zeroclaw, "structured_output").cloned())
        .or_else(|| {
            zeroclaw_schema
                .as_ref()
                .and_then(|schema| schema_field_default(schema, "structured_output"))
                .cloned()
        })
        .unwrap_or(json!({}));
    let tools = projection
        .and_then(|p| p.get("tools"))
        .cloned()
        .or_else(|| zeroclaw.get("tools").cloned())
        .or_else(|| schema_field_default(&zeroclaw, "tools").cloned())
        .or_else(|| {
            zeroclaw_schema
                .as_ref()
                .and_then(|schema| schema_field_default(schema, "tools"))
                .cloned()
        })
        .unwrap_or(json!([]));

    let openapi = compatibility_openapi(zeroclaw_schema.as_ref());
    let schema = json!({
        "openapi": openapi,
        "zeroclaw": {
            "plugin_state": zeroclaw,
            "schema": zeroclaw_schema,
            "chat_form_schema": chat_schema,
            "structured_output": structured_output,
            "tools": tools,
        }
    });

    let body = simd_json::to_string(&schema).unwrap_or_default();
    Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .body(Body::from(body))
        .unwrap_or_else(|_| {
            json_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to build schema response",
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_field_defaults_supply_boot_catalogs() {
        let schema = json!({
            "fields": {
                "providers": {
                    "default": [{"id": "factory"}]
                },
                "model_routes": {
                    "default": [{"model": "auto"}]
                }
            }
        });

        assert_eq!(
            schema_field_default(&schema, "providers")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(1)
        );
        assert_eq!(
            schema_field_default(&schema, "model_routes")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(1)
        );
    }
}
