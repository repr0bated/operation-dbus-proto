//! Zeroclaw route helpers backed by the SHM state tree.

use anyhow::{bail, Result};
use simd_json::prelude::*;

use crate::state_tree;

#[derive(Debug, Clone)]
pub struct ZeroclawSelection {
    pub provider: String,
    pub model: String,
    pub providers: Vec<String>,
    pub available: bool,
}

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
    fn from_value(value: &simd_json::OwnedValue) -> Option<Self> {
        let model = value.get("model")?.as_str()?.to_string();
        let provider = value
            .get("provider")
            .and_then(|v| v.as_str())
            .unwrap_or("zeroclaw")
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

pub fn routes() -> Option<Vec<ZeroclawRoute>> {
    let zeroclaw = state_tree::read_plugin("zeroclaw")?;
    let route_values = zeroclaw.get("model_routes")?.as_array()?;
    Some(
        route_values
            .iter()
            .filter_map(ZeroclawRoute::from_value)
            .collect(),
    )
}

pub fn selection() -> Option<ZeroclawSelection> {
    let zeroclaw = state_tree::read_plugin("zeroclaw")?;
    let provider = zeroclaw.get("selected_provider")?.as_str()?.to_string();
    let model = zeroclaw.get("selected_model")?.as_str()?.to_string();
    let mut providers = zeroclaw
        .get("providers")
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.get("id").and_then(|id| id.as_str()))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    providers.sort();
    providers.dedup();
    let available = zeroclaw
        .get("model_routes")
        .and_then(|value| value.as_array())
        .map(|routes| {
            routes.iter().any(|route| {
                route.get("model").and_then(|value| value.as_str()) == Some(model.as_str())
                    && route
                        .get("available")
                        .and_then(|value| value.as_bool())
                        .unwrap_or(false)
            })
        })
        .unwrap_or(false);
    Some(ZeroclawSelection {
        provider,
        model,
        providers,
        available,
    })
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
                "Zeroclaw route '{}' is {} but not available: {}",
                model,
                route.status,
                reason
            );
        }
    }

    Ok(())
}
