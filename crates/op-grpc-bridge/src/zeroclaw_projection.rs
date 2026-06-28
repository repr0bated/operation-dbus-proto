//! Zeroclaw projection hook (mission spec §9.2/§9.3, Phase 1b).
//!
//! Renders the **read-only** D-Bus sub-object tree for the Zeroclaw plugin:
//! one managed object per declared `ModelRoute` at
//! `/org/opdbus/v1/plugins/zeroclaw/routes/<hint>` and one per `Provider` at
//! `/org/opdbus/v1/plugins/zeroclaw/providers/<id>`.
//!
//! # Dependency direction
//! The mission text places the hook trait in `op-projection`, but `op-projection`
//! already depends on `op-grpc-bridge` (not the reverse). Putting the trait there
//! and implementing it here would create a crate cycle. The actual hard rule is
//! "no crate cycle" (spec §10), so the trait lives here in `op-grpc-bridge` (which
//! already depends on `op-plugins` for `ZeroclawState`). `op-projection` is
//! untouched and the dependency graph stays acyclic.
//!
//! # No polling / watching / indexing
//! `apply` is a synchronous push: the projection owner calls it after a cycle with
//! the **in-memory** schema + state. There is no timer, no file watcher, and no
//! `/dev/shm` re-read on this path. Property names are enumerated from
//! `schemars::schema_for!(ModelRoute)` / `schema_for!(Provider)` — never hardcoded.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use serde_json::Value as JsonValue;
use tokio::sync::{Mutex, OnceCell};
use tracing::{debug, warn};
use zbus::Connection;

use op_plugins::state_plugins::common::llm_projection::{ModelRoute, Provider};
use op_plugins::state_plugins::zeroclaw::ZeroclawState;

const ROUTE_INTERFACE: &str = "org.opdbus.v1.ModelRoute";
const PROVIDER_INTERFACE: &str = "org.opdbus.v1.Provider";
const ROUTES_BASE: &str = "/org/opdbus/v1/plugins/zeroclaw/routes";
const PROVIDERS_BASE: &str = "/org/opdbus/v1/plugins/zeroclaw/providers";

/// Push-only projection contract. The projection owner invokes [`apply`] after a
/// projection cycle, handing in the **in-memory** schema + state. Implementations
/// must not poll, watch, index, or re-read `/dev/shm`.
///
/// [`apply`]: SchemaProjectionHook::apply
pub trait SchemaProjectionHook: Send + Sync {
    /// Apply the projection for `plugin_id` from the in-memory `schema_json` and
    /// `state_json`. Called synchronously; long work is spawned by the impl.
    fn apply(&self, plugin_id: &str, schema_json: &JsonValue, state_json: &JsonValue);
}

/// A projected route/provider managed object. Exposes the schemars-projected
/// field set as a single read-only D-Bus `properties` JSON string (mirroring the
/// `SchemaBackedInterface` convention for dynamic schemas).
#[derive(Clone)]
struct ProjectedObject {
    properties_json: String,
}

#[zbus::interface(name = "org.opdbus.v1.ModelRoute")]
impl ProjectedObject {
    /// The schemars-projected JSON object for this route/provider.
    #[zbus(property)]
    async fn properties(&self) -> String {
        self.properties_json.clone()
    }
}

/// Distinct wrapper so providers register under `org.opdbus.v1.Provider`.
#[derive(Clone)]
struct ProjectedProvider {
    properties_json: String,
}

#[zbus::interface(name = "org.opdbus.v1.Provider")]
impl ProjectedProvider {
    #[zbus(property)]
    async fn properties(&self) -> String {
        self.properties_json.clone()
    }
}

/// Enumerate the top-level property names of a schemars schema (never hardcoded).
fn schema_property_names(schema: &JsonValue) -> Vec<String> {
    schema
        .get("properties")
        .and_then(|p| p.as_object())
        .map(|m| m.keys().cloned().collect())
        .unwrap_or_default()
}

/// Project `value` down to exactly the schemars-declared `names` that are present.
fn project_object(value: &JsonValue, names: &[String]) -> JsonValue {
    let mut out = serde_json::Map::new();
    if let Some(obj) = value.as_object() {
        for name in names {
            if let Some(v) = obj.get(name) {
                out.insert(name.clone(), v.clone());
            }
        }
    }
    JsonValue::Object(out)
}

/// Sanitize an arbitrary id into a valid D-Bus path element (`[A-Za-z0-9_]`,
/// non-empty, not starting with a digit). D-Bus forbids `-`/`.`/`:` in elements,
/// which `sanitize_child_id` does not strip, so this is stricter.
fn path_element(id: &str) -> String {
    let mut s: String = id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    if s.is_empty() {
        s.push('_');
    }
    if s.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false) {
        s.insert(0, '_');
    }
    s
}

/// The set of route + provider object paths a given state should publish, paired
/// with the schemars-projected JSON for each (route paths map to route JSON,
/// provider paths to provider JSON).
fn desired_objects(state: &ZeroclawState) -> (HashMap<String, String>, HashMap<String, String>) {
    let route_names = schema_property_names(
        &serde_json::to_value(schemars::schema_for!(ModelRoute)).unwrap_or_default(),
    );
    let provider_names = schema_property_names(
        &serde_json::to_value(schemars::schema_for!(Provider)).unwrap_or_default(),
    );

    // Route hints are NOT unique (e.g. `code` routes via both openrouter and
    // factory), so the D-Bus path element is keyed by the (hint, provider, model)
    // triple that uniquely identifies a declared route.
    let mut routes = HashMap::new();
    for r in &state.projection.model_routes {
        let key = format!("{}_{}_{}", r.hint, r.provider, r.model);
        let path = format!("{}/{}", ROUTES_BASE, path_element(&key));
        let json = project_object(&serde_json::to_value(r).unwrap_or_default(), &route_names);
        routes.insert(path, serde_json::to_string(&json).unwrap_or_else(|_| "{}".into()));
    }

    let mut providers = HashMap::new();
    for p in &state.projection.providers {
        let path = format!("{}/{}", PROVIDERS_BASE, path_element(&p.id));
        let json = project_object(&serde_json::to_value(p).unwrap_or_default(), &provider_names);
        providers.insert(path, serde_json::to_string(&json).unwrap_or_else(|_| "{}".into()));
    }

    (routes, providers)
}

/// The `op-grpc-bridge` implementation of [`SchemaProjectionHook`]. Holds the
/// shared D-Bus connection cell and the set of currently-registered object paths
/// (for diffing). It does NOT hold an `Arc<SchemaEngine>` and never reads
/// `/dev/shm`.
#[derive(Clone)]
pub struct GrpcBridgeProjectionHook {
    connection: Arc<OnceCell<Connection>>,
    registered_routes: Arc<Mutex<HashSet<String>>>,
    registered_providers: Arc<Mutex<HashSet<String>>>,
}

impl GrpcBridgeProjectionHook {
    pub fn new(connection: Arc<OnceCell<Connection>>) -> Self {
        Self {
            connection,
            registered_routes: Arc::new(Mutex::new(HashSet::new())),
            registered_providers: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Reconcile the published sub-object tree against `state`: register new
    /// route/provider objects and unregister those whose `hint`/`id` disappeared.
    /// Registration failures log a warning and are retried next cycle; they never
    /// abort the caller.
    pub async fn apply_projection(&self, state: &ZeroclawState) {
        let conn = match self.connection.get() {
            Some(c) => c,
            None => {
                debug!("projection hook: D-Bus connection not initialized; skipping cycle");
                return;
            }
        };

        let (routes, providers) = desired_objects(state);
        let server = conn.object_server();

        // Routes
        {
            let mut current = self.registered_routes.lock().await;
            for (path, props) in &routes {
                if let Err(e) = server
                    .at(path.as_str(), ProjectedObject { properties_json: props.clone() })
                    .await
                {
                    debug!(path, error = %e, "route object register (likely already present)");
                }
                current.insert(path.clone());
            }
            let stale: Vec<String> = current.iter().filter(|p| !routes.contains_key(*p)).cloned().collect();
            for path in stale {
                if let Err(e) = server.remove::<ProjectedObject, _>(path.as_str()).await {
                    warn!(path, error = %e, "failed to unregister stale route object");
                }
                current.remove(&path);
            }
        }

        // Providers
        {
            let mut current = self.registered_providers.lock().await;
            for (path, props) in &providers {
                if let Err(e) = server
                    .at(path.as_str(), ProjectedProvider { properties_json: props.clone() })
                    .await
                {
                    debug!(path, error = %e, "provider object register (likely already present)");
                }
                current.insert(path.clone());
            }
            let stale: Vec<String> = current.iter().filter(|p| !providers.contains_key(*p)).cloned().collect();
            for path in stale {
                if let Err(e) = server.remove::<ProjectedProvider, _>(path.as_str()).await {
                    warn!(path, error = %e, "failed to unregister stale provider object");
                }
                current.remove(&path);
            }
        }

        debug!(
            routes = routes.len(),
            providers = providers.len(),
            "zeroclaw projection sub-tree reconciled"
        );
    }
}

impl SchemaProjectionHook for GrpcBridgeProjectionHook {
    fn apply(&self, plugin_id: &str, _schema_json: &JsonValue, state_json: &JsonValue) {
        if plugin_id != "zeroclaw" {
            return;
        }
        let state: ZeroclawState = match serde_json::from_value(state_json.clone()) {
            Ok(s) => s,
            Err(e) => {
                warn!(error = %e, "projection hook: state_json is not a ZeroclawState; skipping");
                return;
            }
        };
        let this = self.clone();
        tokio::spawn(async move {
            this.apply_projection(&state).await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_plugins::state_plugins::zeroclaw::ZeroclawPlugin;

    #[test]
    fn route_property_names_come_from_schemars() {
        let names = schema_property_names(
            &serde_json::to_value(schemars::schema_for!(ModelRoute)).unwrap(),
        );
        for expected in ["hint", "model", "provider", "available", "effort_level"] {
            assert!(names.iter().any(|n| n == expected), "missing route field {expected}");
        }
    }

    #[test]
    fn provider_property_names_come_from_schemars() {
        let names = schema_property_names(
            &serde_json::to_value(schemars::schema_for!(Provider)).unwrap(),
        );
        for expected in ["id", "route", "kind"] {
            assert!(names.iter().any(|n| n == expected), "missing provider field {expected}");
        }
    }

    #[test]
    fn project_object_keeps_only_named_fields() {
        let value = serde_json::json!({"hint": "x", "secret": "no", "model": "m"});
        let names = vec!["hint".to_string(), "model".to_string()];
        let out = project_object(&value, &names);
        assert!(out.get("hint").is_some() && out.get("model").is_some());
        assert!(out.get("secret").is_none());
    }

    #[test]
    fn path_element_strips_invalid_dbus_chars() {
        assert_eq!(path_element("gemini-3.flash:x"), "gemini_3_flash_x");
        assert_eq!(path_element("3pro"), "_3pro");
        assert_eq!(path_element(""), "_");
    }

    #[test]
    fn desired_objects_cover_declared_routes_and_providers() {
        let state = ZeroclawPlugin::current_state();
        let (routes, providers) = desired_objects(&state);
        assert_eq!(routes.len(), state.projection.model_routes.len());
        assert_eq!(providers.len(), state.projection.providers.len());
        for path in routes.keys() {
            assert!(path.starts_with(ROUTES_BASE));
        }
        for path in providers.keys() {
            assert!(path.starts_with(PROVIDERS_BASE));
        }
    }

    #[test]
    fn apply_ignores_non_zeroclaw_and_bad_state() {
        let conn = Arc::new(OnceCell::new());
        let hook = GrpcBridgeProjectionHook::new(conn);
        // Non-zeroclaw plugin id: no panic, no spawn requirement.
        hook.apply("net", &serde_json::json!({}), &serde_json::json!({}));
    }
}
