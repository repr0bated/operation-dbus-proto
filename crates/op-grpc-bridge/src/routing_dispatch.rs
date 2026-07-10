//! routing method dispatch — D-Bus object is the authority.
//!
//! PluginService.CallMethod (zcall) lands in MutationEngine::mutate; typed
//! paths use dispatch_method_call. Both must call this module (same trap as
//! identity_sled). SHM catalog writes are secondary projections only.

use op_plugins::state_plugins::routing::{
    derive_from_registry, publish_catalog, RoutingState, SubdomainRoute, SUBID_REGISTRY,
};
use serde_json::Value as JsonValue;

use crate::mutation_engine::MutationEngine;

async fn cache_state(engine: &MutationEngine) -> RoutingState {
    match engine.get_state("routing").await {
        Some(v) => serde_json::from_value(serde_json::to_value(&v).unwrap_or_default())
            .unwrap_or_default(),
        None => RoutingState::default(),
    }
}

async fn replace_state(engine: &MutationEngine, state: &RoutingState) -> anyhow::Result<()> {
    let owned = simd_json::serde::to_owned_value(state)
        .map_err(|e| anyhow::anyhow!("serialize routing state: {e}"))?;
    engine
        .update_state_cache("routing".to_string(), owned)
        .await;
    Ok(())
}

/// Dispatch a routing plugin method. Returns the domain JSON result.
///
/// On mutating methods the full `RoutingState` is written to the state cache
/// so `MutationEngine::mutate` can project it as authoritative_value.
pub async fn dispatch_routing_method(
    engine: &MutationEngine,
    method: &str,
    args: &JsonValue,
) -> anyhow::Result<JsonValue> {
    let method = method.replace('-', "_");
    match method.as_str() {
        "get_state" => {
            let state = cache_state(engine).await;
            Ok(serde_json::json!({ "state": state }))
        }
        "list_subdomains" => {
            let state = cache_state(engine).await;
            Ok(serde_json::json!({ "subdomains": state.subdomains }))
        }
        "list_internal" => {
            let state = cache_state(engine).await;
            Ok(serde_json::json!({ "internal_services": state.internal_services }))
        }
        "get_identity_injection" => {
            let state = cache_state(engine).await;
            Ok(serde_json::json!({ "identity_injection": state.identity_injection }))
        }
        "list_tags" => {
            let state = cache_state(engine).await;
            let mut tags: Vec<String> = state.subdomains.iter().map(|s| s.tag.clone()).collect();
            tags.extend(state.internal_services.iter().map(|s| s.tag.clone()));
            tags.sort();
            tags.dedup();
            Ok(serde_json::json!({ "tags": tags }))
        }
        "derive" => {
            let registry = args
                .get("registry_path")
                .and_then(|v| v.as_str())
                .unwrap_or(SUBID_REGISTRY);
            let state = derive_from_registry(registry)?;
            replace_state(engine, &state).await?;
            Ok(serde_json::json!({ "state": state }))
        }
        "publish" => {
            let mut state = cache_state(engine).await;
            if state.subdomains.is_empty() && state.internal_services.is_empty() {
                state = derive_from_registry(SUBID_REGISTRY)?;
                replace_state(engine, &state).await?;
            }
            let catalog = publish_catalog(&state)?;
            Ok(serde_json::json!({
                "catalog_path": state.catalog_path,
                "route_count": catalog.xray_routes.len() as u32,
                "state": state,
            }))
        }
        "upsert_subdomain" => {
            let route: SubdomainRoute = if let Some(r) = args.get("route") {
                serde_json::from_value(r.clone())?
            } else {
                serde_json::from_value(args.clone())?
            };
            if route.tag.is_empty() {
                anyhow::bail!("upsert_subdomain requires route.tag");
            }
            let mut state = cache_state(engine).await;
            if let Some(existing) = state.subdomains.iter_mut().find(|s| s.tag == route.tag) {
                *existing = route;
            } else {
                state.subdomains.push(route);
            }
            state.status = if state.subdomains.is_empty() {
                "empty"
            } else {
                "ready"
            }
            .to_string();
            replace_state(engine, &state).await?;
            Ok(serde_json::json!({ "state": state }))
        }
        "remove_subdomain" => {
            let tag = args
                .get("tag")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("remove_subdomain requires tag"))?
                .to_string();
            let mut state = cache_state(engine).await;
            state.subdomains.retain(|s| s.tag != tag);
            state.status = if state.subdomains.is_empty() {
                "empty"
            } else {
                "ready"
            }
            .to_string();
            replace_state(engine, &state).await?;
            Ok(serde_json::json!({ "state": state }))
        }
        other => Err(anyhow::anyhow!("unknown routing method: {other}")),
    }
}
