//! Router-plugin helpers backed by the SHM state tree.
//!
//! HTTP paths stay `/api/zeroclaw/*` and `/api/llm/*` (what the UI already
//! calls). The sealed plugin those handlers read is `tched_router`.

use anyhow::{bail, Result};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

use crate::state_tree;

/// Canonical D-Bus / SHM plugin id after the OpenCode rebrand.
pub const ROUTER_PLUGIN_ID: &str = "tched_router";
/// Pre-rebrand plugin id. Still accepted so a host that has not resealed yet
/// keeps serving the same UI endpoints.
pub const LEGACY_ROUTER_PLUGIN_ID: &str = "zeroclaw";

#[derive(Debug, Clone)]
pub struct ZeroclawRoute {
    pub provider: String,
    pub upstream_provider: String,
    pub transport: Option<String>,
    pub model: String,
    pub hint: Option<String>,
    pub kind: Option<String>,
    pub status: String,
    pub available: bool,
    pub status_reason: Option<String>,
    pub source: Option<String>,
}

impl ZeroclawRoute {
    fn from_value(value: &Value) -> Option<Self> {
        let model = value.get("model")?.as_str()?.to_string();
        let provider = value
            .get("provider")
            .and_then(|v| v.as_str())
            .unwrap_or(ROUTER_PLUGIN_ID)
            .to_string();
        let upstream_provider = value
            .get("upstream_provider")
            .and_then(|v| v.as_str())
            .unwrap_or(&provider)
            .to_string();
        let status = value
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("declared")
            .to_string();
        let available = value
            .get("available")
            .and_then(|v| v.as_bool())
            .unwrap_or_else(|| status == "available");

        Some(Self {
            provider,
            upstream_provider,
            transport: value
                .get("transport")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            model,
            hint: value
                .get("hint")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            kind: value
                .get("kind")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            status,
            available,
            status_reason: value
                .get("status_reason")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            source: value
                .get("source")
                .and_then(|v| v.as_str())
                .map(str::to_string),
        })
    }
}

/// Live router plugin state: `tched_router` first, then legacy `zeroclaw`.
pub fn read_router_plugin() -> Option<Value> {
    state_tree::read_plugin(ROUTER_PLUGIN_ID)
        .or_else(|| state_tree::read_plugin(LEGACY_ROUTER_PLUGIN_ID))
}

/// `model_routes` live at the top level (flattened catalog) or under the
/// historical `projection` / `catalog` objects.
pub fn model_route_values(state: &Value) -> Option<&Vec<Value>> {
    state
        .get("model_routes")
        .and_then(|v| v.as_array())
        .or_else(|| {
            state
                .get("catalog")
                .and_then(|v| v.get("model_routes"))
                .and_then(|v| v.as_array())
        })
        .or_else(|| {
            state
                .get("projection")
                .and_then(|v| v.get("model_routes"))
                .and_then(|v| v.as_array())
        })
}

pub fn routes() -> Option<Vec<ZeroclawRoute>> {
    let state = read_router_plugin()?;
    let route_values = model_route_values(&state)?;
    Some(
        route_values
            .iter()
            .filter_map(ZeroclawRoute::from_value)
            .collect(),
    )
}

pub fn route_for_model(model: &str) -> Option<ZeroclawRoute> {
    routes()?.into_iter().find(|route| route.model == model)
}

pub fn ensure_model_available(model: &str) -> Result<()> {
    if let Some(route) = route_for_model(model) {
        if !route.available {
            let reason = route
                .status_reason
                .unwrap_or_else(|| "route is not available".to_string());
            bail!(
                "Router route '{}' is {} but not available: {}",
                model,
                route.status,
                reason
            );
        }
    }

    Ok(())
}

pub fn selected_provider_model() -> Option<(String, String)> {
    let state = read_router_plugin()?;
    let provider = state.get("selected_provider")?.as_str()?.to_string();
    let model = state.get("selected_model")?.as_str()?.to_string();
    if provider.is_empty() && model.is_empty() {
        return None;
    }
    Some((provider, model))
}

#[cfg(test)]
mod tests {
    use super::*;
    use simd_json::json;

    #[test]
    fn model_routes_read_flattened_catalog() {
        let state = json!({
            "selected_provider": "opencode",
            "model_routes": [{"model": "deepseek-v4-flash-free", "provider": "opencode"}]
        });
        let routes = model_route_values(&state).expect("flattened routes");
        assert_eq!(routes.len(), 1);
    }

    #[test]
    fn model_routes_read_nested_projection() {
        let state = json!({
            "projection": {
                "model_routes": [{"model": "qwen3.6-27b", "provider": "salad"}]
            }
        });
        let routes = model_route_values(&state).expect("nested projection");
        assert_eq!(
            routes[0].get("model").and_then(|v| v.as_str()),
            Some("qwen3.6-27b")
        );
    }
}
