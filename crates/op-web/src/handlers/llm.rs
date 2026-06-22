//! LLM API Handlers

use axum::{
    extract::{Extension, Path},
    response::Json,
};
use serde::Serialize;
use simd_json::{json, OwnedValue as Value};
use std::str::FromStr;
use std::sync::Arc;

use crate::state::AppState;
use op_llm::provider::ProviderType;

#[derive(Serialize)]
pub struct LlmStatusResponse {
    pub provider: String,
    pub model: String,
    pub model_non_sandboxed: bool,
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
    let model_non_sandboxed = state.chat_manager.current_model_non_sandboxed().await;
    Json(LlmStatusResponse {
        provider,
        model,
        model_non_sandboxed,
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
