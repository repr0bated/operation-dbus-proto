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
    Extension(_state): Extension<Arc<AppState>>,
) -> Json<LlmStatusResponse> {
    let selection = crate::tched_router_routes::selection();
    Json(LlmStatusResponse {
        provider: selection
            .as_ref()
            .map(|value| value.provider.clone())
            .unwrap_or_else(|| "tched_router-unavailable".to_string()),
        model: selection
            .as_ref()
            .map(|value| value.model.clone())
            .unwrap_or_default(),
        model_non_sandboxed: false,
        available: selection.map(|value| value.available).unwrap_or(false),
    })
}

/// GET /api/llm/providers - List available providers
pub async fn list_providers_handler(
    Extension(_state): Extension<Arc<AppState>>,
) -> Json<LlmProvidersResponse> {
    let selection = crate::tched_router_routes::selection();
    Json(LlmProvidersResponse {
        providers: selection
            .as_ref()
            .map(|value| value.providers.clone())
            .unwrap_or_default(),
        current: selection
            .map(|value| value.provider)
            .unwrap_or_else(|| "tched_router-unavailable".to_string()),
    })
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
