//! LLM API routes for provider and model management

use axum::{
    extract::{Extension, Query},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use std::sync::Arc;
use tracing::{error, info};

use crate::state::AppState;
use op_llm::ProviderType;

/// LLM status response
#[derive(Debug, Serialize)]
pub struct LlmStatusResponse {
    pub provider: String,
    pub model: String,
    pub available_providers: Vec<String>,
}

/// Model list response
#[derive(Debug, Serialize)]
pub struct ModelsResponse {
    pub provider: String,
    pub models: Vec<ModelInfo>,
}

#[derive(Debug, Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub available: bool,
}

/// Provider switch request
#[derive(Debug, Deserialize)]
pub struct SwitchProviderRequest {
    pub provider: String,
}

/// Model switch request
#[derive(Debug, Deserialize)]
pub struct SwitchModelRequest {
    pub model: String,
}

/// Query params for models endpoint
#[derive(Debug, Deserialize)]
pub struct ModelsQuery {
    pub provider: Option<String>,
}

/// GET /api/llm/status - Get current LLM status
pub async fn get_llm_status(Extension(state): Extension<Arc<AppState>>) -> impl IntoResponse {
    let status = state.chat_manager.get_status().await;

    Json(LlmStatusResponse {
        provider: status["provider"].as_str().unwrap_or("unknown").to_string(),
        model: status["model"].as_str().unwrap_or("").to_string(),
        available_providers: status["available_providers"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
    })
}

/// GET /api/llm/models - Get models for current or specified provider
pub async fn get_models(
    Extension(state): Extension<Arc<AppState>>,
    Query(query): Query<ModelsQuery>,
) -> impl IntoResponse {
    let provider_type = if let Some(ref provider_str) = query.provider {
        match provider_str.parse::<ProviderType>() {
            Ok(pt) => pt,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(simd_json::json!({ "error": format!("Unknown provider: {}", provider_str) }))
                ).into_response();
            }
        }
    } else {
        state.chat_manager.current_provider().await
    };

    // Check if provider is available
    if !state.chat_manager.has_provider(&provider_type) {
        return (
            StatusCode::BAD_REQUEST,
            Json(simd_json::json!({
                "error": format!("Provider {} not available. Check API key.", provider_type),
                "provider": provider_type.to_string(),
                "models": []
            })),
        )
            .into_response();
    }

    match state
        .chat_manager
        .list_models_for_provider(&provider_type)
        .await
    {
        Ok(models) => {
            let model_infos: Vec<ModelInfo> = models
                .into_iter()
                .map(|m| ModelInfo {
                    id: m.id,
                    name: m.name,
                    description: m.description,
                    available: m.available,
                })
                .collect();

            Json(ModelsResponse {
                provider: provider_type.to_string(),
                models: model_infos,
            })
            .into_response()
        }
        Err(e) => {
            error!("Failed to list models for {}: {}", provider_type, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(simd_json::json!({
                    "error": format!("Failed to list models: {}", e),
                    "provider": provider_type.to_string(),
                    "models": []
                })),
            )
                .into_response()
        }
    }
}

/// POST /api/llm/provider - Switch provider
pub async fn switch_provider(
    Extension(state): Extension<Arc<AppState>>,
    Json(req): Json<SwitchProviderRequest>,
) -> impl IntoResponse {
    let provider_type = match req.provider.parse::<ProviderType>() {
        Ok(pt) => pt,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(simd_json::json!({ "error": format!("Unknown provider: {}", req.provider) })),
            );
        }
    };

    match state
        .chat_manager
        .switch_provider(provider_type.clone())
        .await
    {
        Ok(_) => {
            info!("Switched to provider: {}", provider_type);
            (
                StatusCode::OK,
                Json(simd_json::json!({
                    "success": true,
                    "provider": provider_type.to_string()
                })),
            )
        }
        Err(e) => {
            error!("Failed to switch provider: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(simd_json::json!({ "error": e.to_string() })),
            )
        }
    }
}

/// POST /api/llm/model - Switch model
pub async fn switch_model(
    Extension(state): Extension<Arc<AppState>>,
    Json(req): Json<SwitchModelRequest>,
) -> impl IntoResponse {
    match state.chat_manager.switch_model(&req.model).await {
        Ok(_) => {
            info!("Switched to model: {}", req.model);
            (
                StatusCode::OK,
                Json(simd_json::json!({
                    "success": true,
                    "model": req.model
                })),
            )
        }
        Err(e) => {
            error!("Failed to switch model: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(simd_json::json!({ "error": e.to_string() })),
            )
        }
    }
}
