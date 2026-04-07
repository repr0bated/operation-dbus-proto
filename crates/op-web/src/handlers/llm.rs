//! LLM API Handlers

use axum::{
    extract::{Extension, Path},
    response::Json,
};
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use std::str::FromStr;
use std::sync::Arc;

use crate::state::AppState;
use op_llm::provider::ProviderType;

#[derive(Serialize)]
pub struct LlmStatusResponse {
    pub provider: String,
    pub model: String,
    pub available: bool,
}

#[derive(Serialize)]
pub struct LlmProvidersResponse {
    pub providers: Vec<String>,
    pub current: String,
}

/// GET /api/llm/status - Get LLM status
pub async fn llm_status_handler(
    Extension(state): Extension<Arc<AppState>>,
) -> Json<LlmStatusResponse> {
    let provider = state.chat_manager.current_provider().await.to_string();
    let model = state.chat_manager.current_model().await;
    let available = !state.chat_manager.available_providers().is_empty();
    Json(LlmStatusResponse {
        provider,
        model,
        available,
    })
}

/// GET /api/llm/providers - List available providers
pub async fn list_providers_handler(
    Extension(state): Extension<Arc<AppState>>,
) -> Json<LlmProvidersResponse> {
    let providers: Vec<String> = state
        .chat_manager
        .available_providers()
        .into_iter()
        .map(|provider| provider.to_string())
        .collect();
    let current = state.chat_manager.current_provider().await.to_string();
    Json(LlmProvidersResponse { providers, current })
}

/// GET /api/llm/models - List available models
pub async fn list_models_handler(Extension(state): Extension<Arc<AppState>>) -> Json<Value> {
    match state.chat_manager.list_models().await {
        Ok(models) => {
            let current = state.chat_manager.current_model().await;
            Json(json!({
                "models": models,
                "current": current
            }))
        }
        Err(e) => Json(json!({
            "models": [],
            "current": null,
            "error": e.to_string()
        })),
    }
}

/// GET /api/llm/models/:provider - List models for a provider
pub async fn list_models_for_provider_handler(
    Extension(state): Extension<Arc<AppState>>,
    Path(provider): Path<String>,
) -> Json<Value> {
    match ProviderType::from_str(&provider) {
        Ok(provider_type) => match state
            .chat_manager
            .list_models_for_provider(&provider_type)
            .await
        {
            Ok(models) => Json(json!({
                "provider": provider,
                "models": models,
                "current": state.chat_manager.current_model().await
            })),
            Err(e) => Json(json!({
                "provider": provider,
                "models": [],
                "current": null,
                "error": e.to_string()
            })),
        },
        Err(e) => Json(json!({
            "provider": provider,
            "models": [],
            "current": null,
            "error": e
        })),
    }
}

#[derive(Debug, Deserialize)]
pub struct SwitchModelRequest {
    pub model: String,
}

/// POST /api/llm/model - Switch model
pub async fn switch_model_handler(
    Extension(state): Extension<Arc<AppState>>,
    Json(request): Json<SwitchModelRequest>,
) -> Json<Value> {
    match state.chat_manager.switch_model(request.model.clone()).await {
        Ok(_) => {
            let _ = persist_model(&request.model).await;
            Json(json!({
                "success": true,
                "model": request.model
            }))
        }
        Err(e) => Json(json!({
            "success": false,
            "model": request.model,
            "note": e.to_string()
        })),
    }
}

#[derive(Debug, Deserialize)]
pub struct SwitchProviderRequest {
    pub provider: String,
}

/// POST /api/llm/provider - Switch provider
pub async fn switch_provider_handler(
    Extension(state): Extension<Arc<AppState>>,
    Json(request): Json<SwitchProviderRequest>,
) -> Json<Value> {
    match ProviderType::from_str(&request.provider) {
        Ok(provider_type) => match state.chat_manager.switch_provider(provider_type).await {
            Ok(_) => {
                let _ = persist_provider(&request.provider).await;
                let current_model = state.chat_manager.current_model().await;
                Json(json!({
                    "success": true,
                    "model": current_model
                }))
            }
            Err(e) => Json(json!({
                "success": false,
                "model": state.chat_manager.current_model().await,
                "note": e.to_string()
            })),
        },
        Err(e) => Json(json!({
            "success": false,
            "model": state.chat_manager.current_model().await,
            "note": e
        })),
    }
}

const PERSISTED_MODEL_PATH: &str = "/etc/op-dbus/llm-model";
const PERSISTED_PROVIDER_PATH: &str = "/etc/op-dbus/llm-provider";

async fn persist_model(model: &str) -> Result<(), String> {
    if let Some(parent) = std::path::Path::new(PERSISTED_MODEL_PATH).parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("create dir: {}", e))?;
    }
    tokio::fs::write(PERSISTED_MODEL_PATH, format!("{model}\n"))
        .await
        .map_err(|e| format!("write model: {}", e))?;
    Ok(())
}

async fn persist_provider(provider: &str) -> Result<(), String> {
    if let Some(parent) = std::path::Path::new(PERSISTED_PROVIDER_PATH).parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("create dir: {}", e))?;
    }
    tokio::fs::write(PERSISTED_PROVIDER_PATH, format!("{provider}\n"))
        .await
        .map_err(|e| format!("write provider: {}", e))?;
    Ok(())
}
