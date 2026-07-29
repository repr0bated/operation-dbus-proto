//! Zeroclaw route helpers backed by the SHM state tree.

use anyhow::{bail, Result};
use simd_json::prelude::*;

use crate::state_tree;

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
    let zeroclaw = state_tree::read_key("zeroclaw", "model_routes")?;
    let route_values = zeroclaw.get("model_routes")?.as_array()?;
    Some(
        route_values
            .iter()
            .filter_map(ZeroclawRoute::from_value)
            .collect(),
    )
}

pub fn route_for_model(model: &str) -> Option<ZeroclawRoute> {
    routes()?
        .into_iter()
        .find(|route| route.model == model)
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
