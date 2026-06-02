//! Zeroclaw route helpers backed by the D-Bus projection cache.

use anyhow::{bail, Result};
use simd_json::prelude::*;

use crate::projection_client::ProjectionCache;

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

pub async fn routes(cache: &ProjectionCache) -> Option<Vec<ZeroclawRoute>> {
    let zeroclaw = crate::projection_client::get_projection(cache, "zeroclaw").await?;
    let route_values = zeroclaw.get("model_routes")?.as_array()?;
    Some(
        route_values
            .iter()
            .filter_map(ZeroclawRoute::from_value)
            .collect(),
    )
}

pub async fn route_for_model(cache: &ProjectionCache, model: &str) -> Option<ZeroclawRoute> {
    routes(cache)
        .await?
        .into_iter()
        .find(|route| route.model == model)
}

pub async fn ensure_model_available(cache: &ProjectionCache, model: &str) -> Result<()> {
    if let Some(route) = route_for_model(cache, model).await {
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
