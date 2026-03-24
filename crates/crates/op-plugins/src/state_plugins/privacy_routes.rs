use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff, StatePlugin,
};
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use std::path::PathBuf;

const DEFAULT_PRIVACY_ROUTES_PATH: &str = "/var/lib/op-dbus/privacy-routes.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PrivacyRoutesState {
    #[serde(default)]
    pub routes: Vec<PrivacyRoute>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PrivacyRoute {
    pub name: String,
    pub route_id: String,
    pub user_id: String,
    pub email: String,
    pub wireguard_public_key: String,
    pub assigned_ip: String,
    pub selector_ip: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub container_name: Option<String>,
    pub ingress_port: String,
    pub next_hop: String,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

pub struct PrivacyRoutesPlugin {
    store_path: PathBuf,
}

impl Default for PrivacyRoutesPlugin {
    fn default() -> Self {
        Self::new(DEFAULT_PRIVACY_ROUTES_PATH)
    }
}

impl PrivacyRoutesPlugin {
    pub fn new(store_path: impl Into<PathBuf>) -> Self {
        Self {
            store_path: store_path.into(),
        }
    }

    async fn load_store(&self) -> Result<PrivacyRoutesState> {
        match tokio::fs::read_to_string(&self.store_path).await {
            Ok(mut content) => {
                let mut state: PrivacyRoutesState = unsafe { simd_json::from_str(&mut content) }
                    .context("invalid privacy route store")?;
                state.routes.sort_by(|a, b| a.route_id.cmp(&b.route_id));
                Ok(state)
            }
            Err(_) => Ok(PrivacyRoutesState { routes: Vec::new() }),
        }
    }

    async fn save_store(&self, state: &PrivacyRoutesState) -> Result<()> {
        if let Some(parent) = self.store_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("failed to create privacy route directory")?;
        }

        let content = simd_json::to_string_pretty(state).context("serialize privacy routes")?;
        tokio::fs::write(&self.store_path, content)
            .await
            .context("write privacy routes")?;
        Ok(())
    }
}

#[async_trait]
impl StatePlugin for PrivacyRoutesPlugin {
    fn name(&self) -> &str {
        "privacy_routes"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    async fn query_current_state(&self) -> Result<Value> {
        let state = self.load_store().await?;
        Ok(simd_json::serde::to_owned_value(state)?)
    }

    async fn calculate_diff(&self, current: &Value, desired: &Value) -> Result<StateDiff> {
        let current_state: PrivacyRoutesState = simd_json::serde::from_owned_value(current.clone())
            .context("deserialize current privacy routes")?;
        let desired_state: PrivacyRoutesState = simd_json::serde::from_owned_value(desired.clone())
            .context("deserialize desired privacy routes")?;

        let current_by_id: HashMap<&str, &PrivacyRoute> = current_state
            .routes
            .iter()
            .map(|route| (route.route_id.as_str(), route))
            .collect();
        let desired_by_id: HashMap<&str, &PrivacyRoute> = desired_state
            .routes
            .iter()
            .map(|route| (route.route_id.as_str(), route))
            .collect();

        let mut actions = Vec::new();

        for desired_route in &desired_state.routes {
            match current_by_id.get(desired_route.route_id.as_str()) {
                Some(current_route) if *current_route == desired_route => {}
                Some(_) => actions.push(StateAction::Modify {
                    resource: desired_route.route_id.clone(),
                    changes: simd_json::serde::to_owned_value(desired_route.clone())
                        .context("serialize desired privacy route modify")?,
                }),
                None => actions.push(StateAction::Create {
                    resource: desired_route.route_id.clone(),
                    config: simd_json::serde::to_owned_value(desired_route.clone())
                        .context("serialize desired privacy route create")?,
                }),
            }
        }

        for current_route in &current_state.routes {
            if !desired_by_id.contains_key(current_route.route_id.as_str()) {
                actions.push(StateAction::Delete {
                    resource: current_route.route_id.clone(),
                });
            }
        }

        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions,
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: format!("{:x}", md5::compute(simd_json::to_string(current)?)),
                desired_hash: format!("{:x}", md5::compute(simd_json::to_string(desired)?)),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        let mut state = self.load_store().await?;
        let mut routes_by_id: HashMap<String, PrivacyRoute> = state
            .routes
            .drain(..)
            .map(|route| (route.route_id.clone(), route))
            .collect();

        let mut changes_applied = Vec::new();
        let mut errors = Vec::new();

        for action in &diff.actions {
            match action {
                StateAction::Create { resource, config } => {
                    let route: PrivacyRoute = simd_json::serde::from_owned_value(config.clone())
                        .context("deserialize route create")?;
                    routes_by_id.insert(resource.clone(), route);
                    changes_applied.push(format!("created privacy route {}", resource));
                }
                StateAction::Modify { resource, changes } => {
                    let route: PrivacyRoute = simd_json::serde::from_owned_value(changes.clone())
                        .context("deserialize route modify")?;
                    routes_by_id.insert(resource.clone(), route);
                    changes_applied.push(format!("updated privacy route {}", resource));
                }
                StateAction::Delete { resource } => {
                    routes_by_id.remove(resource);
                    changes_applied.push(format!("deleted privacy route {}", resource));
                }
                StateAction::NoOp { .. } => {}
            }
        }

        state.routes = routes_by_id.into_values().collect();
        state.routes.sort_by(|a, b| a.route_id.cmp(&b.route_id));

        if let Err(e) = self.save_store(&state).await {
            errors.push(e.to_string());
        }

        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied,
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let current = self.query_current_state().await?;
        let current_state: PrivacyRoutesState = simd_json::serde::from_owned_value(current)?;
        let desired_state: PrivacyRoutesState =
            simd_json::serde::from_owned_value(desired.clone())?;
        Ok(current_state == desired_state)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        let current = self.query_current_state().await?;
        Ok(Checkpoint {
            id: format!("privacy-routes-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: current,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, checkpoint: &Checkpoint) -> Result<()> {
        let state: PrivacyRoutesState =
            simd_json::serde::from_owned_value(checkpoint.state_snapshot.clone())?;
        self.save_store(&state).await
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: true,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_privacy_routes_plugin_create_modify_delete() {
        let temp_dir = tempfile::tempdir().expect("tempdir");
        let store_path = temp_dir.path().join("privacy-routes.json");
        let plugin = PrivacyRoutesPlugin::new(&store_path);

        let desired = PrivacyRoutesState {
            routes: vec![PrivacyRoute {
                name: "route-a".to_string(),
                route_id: "route-a".to_string(),
                user_id: "user-a".to_string(),
                email: "user@example.com".to_string(),
                wireguard_public_key: "pubkey".to_string(),
                assigned_ip: "10.100.0.2/32".to_string(),
                selector_ip: "10.100.0.2".to_string(),
                container_name: Some("privacy-user-a".to_string()),
                ingress_port: "ovsbr0-sock".to_string(),
                next_hop: "priv_wg".to_string(),
                enabled: true,
                created_at: "2026-01-01T00:00:00Z".to_string(),
                updated_at: "2026-01-01T00:00:00Z".to_string(),
            }],
        };

        let current = plugin.query_current_state().await.expect("query current");
        let desired_value =
            simd_json::serde::to_owned_value(desired.clone()).expect("serialize desired");
        let diff = plugin
            .calculate_diff(&current, &desired_value)
            .await
            .expect("calculate diff");
        assert_eq!(diff.actions.len(), 1);

        let result = plugin.apply_state(&diff).await.expect("apply");
        assert!(result.success);

        let stored = plugin.query_current_state().await.expect("query stored");
        let stored_state: PrivacyRoutesState =
            simd_json::serde::from_owned_value(stored).expect("deserialize stored");
        assert_eq!(stored_state, desired);

        let empty = simd_json::serde::to_owned_value(PrivacyRoutesState { routes: Vec::new() })
            .expect("serialize empty");
        let delete_diff = plugin
            .calculate_diff(
                &simd_json::serde::to_owned_value(desired).expect("serialize current desired"),
                &empty,
            )
            .await
            .expect("calculate delete diff");
        assert_eq!(delete_diff.actions.len(), 1);
    }
}
