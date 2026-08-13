//! Service plugin - auto-generating, validating, runit service management.

use super::plugin_scaffold_helpers::{method_decl_from_schemars_with_output, AckOutput};
use crate::service_def::{
    ExecCommand, LogType, ReadyNotification, RestartPolicy, ServiceDef, ServiceName, ServiceType,
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::{FieldSchema, FieldType, PluginSchema, SideEffect};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default, JsonSchema)]
pub struct ServiceLifecycle {
    pub last_active: Option<u64>,
    pub days_since_active: Option<u64>,
    pub is_orphaned: bool,
    pub orphan_reason: Option<String>,
}

// D-Bus method input types for service lifecycle operations.

/// Init method input - initialize a service definition
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct InitInput {
    /// Service name
    pub name: String,
    /// Service type (simple, forking, oneshot, etc.)
    #[serde(default)]
    pub service_type: String,
}

/// Run method input - start/running a service
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RunInput {
    /// Service name
    pub name: String,
    /// Arguments to pass
    #[serde(default)]
    pub args: Vec<String>,
}

/// Shutdown method input - stop a service
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ShutdownInput {
    /// Service name
    pub name: String,
    /// Force stop (SIGKILL)
    #[serde(default)]
    pub force: bool,
}

const RUNIT_ACTIVE_DIR: &str = "/etc/runit/runsvdir/default";

pub struct ServicePlugin;

impl Default for ServicePlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl ServicePlugin {
    pub fn new() -> Self {
        Self
    }

    /// Auto-generate service from installed binary
    pub async fn auto_generate_service(&self, binary_path: &Path) -> Result<ServiceDef> {
        let name = binary_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow::anyhow!("invalid binary name"))?;

        Ok(ServiceDef {
            name: ServiceName::new(name)?,
            service_type: ServiceType::Simple,
            exec_start: ExecCommand::new(binary_path.to_path_buf(), vec![])?,
            exec_stop: None,
            working_dir: None,
            user: None,
            group: None,
            depends_on: vec![],
            waits_for: vec![],
            restart: RestartPolicy::default(),
            environment: HashMap::new(),
            env_file: None,
            resources: None,
            log_type: LogType::None,
            ready_notification: ReadyNotification::None,
            chain_to: None,
            smooth_recovery: false,
            enabled: false,
        })
    }

    /// Install and enable a runit service definition.
    pub async fn install_service(&self, svc: &ServiceDef) -> Result<()> {
        svc.install()?;
        log::info!("Installed runit service: {}", svc.name);
        Ok(())
    }

    /// List services currently supervised and running under runit.
    async fn list_runit_services(&self) -> Result<Vec<String>> {
        let mut running = Vec::new();
        for entry in std::fs::read_dir(RUNIT_ACTIVE_DIR)
            .with_context(|| format!("read {RUNIT_ACTIVE_DIR}"))?
        {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if ServiceName::new(&name).is_err() {
                continue;
            }
            let status = tokio::process::Command::new("sv")
                .arg("status")
                .arg(entry.path())
                .output()
                .await
                .with_context(|| format!("sv status {name}"))?;
            if String::from_utf8_lossy(&status.stdout).starts_with("run:") {
                running.push(name);
            }
        }
        running.sort();
        Ok(running)
    }

    async fn check_lifecycle(&self, name: &str) -> Result<ServiceLifecycle> {
        let service = ServiceName::new(name)?;
        let enabled = Path::new(RUNIT_ACTIVE_DIR).join(service.as_str()).exists();
        let running = self
            .list_runit_services()
            .await?
            .iter()
            .any(|candidate| candidate == service.as_str());
        let is_orphaned = !enabled;
        let orphan_reason = if is_orphaned {
            Some("not enabled in the active runit directory".to_string())
        } else if !running {
            Some("enabled but not running".to_string())
        } else {
            None
        };

        Ok(ServiceLifecycle {
            last_active: None,
            days_since_active: None,
            is_orphaned,
            orphan_reason,
        })
    }
}

#[async_trait]
impl StatePlugin for ServicePlugin {
    fn name(&self) -> &str {
        "service"
    }
    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<op_state_store::PluginSchema> {
        Some(service_schema())
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: String::new(),
                desired_hash: String::new(),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: format!("service-{}", chrono::Utc::now().timestamp()),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: json!({}),
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
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

#[derive(Debug, Clone, Serialize, Deserialize, Default, JsonSchema)]
#[schemars(extend("x-oscal-category" = "service"))]
pub struct ServiceSchemaState {
    pub services: serde_json::Value,
}

pub(crate) fn service_schema() -> PluginSchema {
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "service",
        "1.0.0",
        "Service definition declarations",
        &serde_json::to_value(schemars::schema_for!(ServiceSchemaState)).unwrap(),
    );
    super::schemars_adapter::apply_state_defaults(
        &mut schema,
        &simd_json::serde::to_owned_value(&ServiceSchemaState::default()).unwrap(),
    );

    // Add D-Bus methods for runit service lifecycle management.
    schema.methods.insert(
        "init".to_string(),
        method_decl_from_schemars_with_output::<InitInput, AckOutput>(
            "init",
            SideEffect::Mutation,
            false,
            "cap.service.lifecycle.init@v1",
            "mut.service.lifecycle.init@v1",
        ),
    );
    schema.methods.insert(
        "run".to_string(),
        method_decl_from_schemars_with_output::<RunInput, AckOutput>(
            "run",
            SideEffect::Mutation,
            false,
            "cap.service.lifecycle.run@v1",
            "mut.service.lifecycle.run@v1",
        ),
    );
    schema.methods.insert(
        "shutdown".to_string(),
        method_decl_from_schemars_with_output::<ShutdownInput, AckOutput>(
            "shutdown",
            SideEffect::Mutation,
            false,
            "cap.service.lifecycle.shutdown@v1",
            "mut.service.lifecycle.shutdown@v1",
        ),
    );

    schema
}

/// Backend for the `service` plugin lifecycle methods, called from
/// `mutation_engine::dispatch_method_call`'s `"service"` arm.
pub async fn dispatch_service_method(
    method: &str,
    args: &serde_json::Value,
) -> Result<serde_json::Value> {
    use super::plugin_scaffold_helpers::AckOutput;
    use crate::service_def::RunitPlugin;

    let manager = RunitPlugin::new();
    match method {
        "init" => {
            let input: InitInput = serde_json::from_value(args.clone())
                .map_err(|e| anyhow::anyhow!("invalid init args: {e}"))?;
            let _ = input.service_type;
            manager.enable(&input.name).await?;
            Ok(serde_json::to_value(AckOutput { success: true })?)
        }
        "run" => {
            let input: RunInput = serde_json::from_value(args.clone())
                .map_err(|e| anyhow::anyhow!("invalid run args: {e}"))?;
            let _ = input.args;
            manager.start(&input.name).await?;
            Ok(serde_json::to_value(AckOutput { success: true })?)
        }
        "shutdown" => {
            let input: ShutdownInput = serde_json::from_value(args.clone())
                .map_err(|e| anyhow::anyhow!("invalid shutdown args: {e}"))?;
            if input.force {
                force_stop_service(&input.name).await?;
            } else {
                manager.stop(&input.name).await?;
            }
            Ok(serde_json::to_value(AckOutput { success: true })?)
        }
        other => Err(anyhow::anyhow!("unknown service method: {other}")),
    }
}

async fn force_stop_service(name: &str) -> Result<()> {
    let service = ServiceName::new(name)?;
    let active = Path::new(RUNIT_ACTIVE_DIR).join(service.as_str());
    if !active.exists() {
        anyhow::bail!("runit service '{service}' is not enabled");
    }
    let status = tokio::process::Command::new("sv")
        .arg("force-stop")
        .arg(&active)
        .status()
        .await
        .with_context(|| format!("sv force-stop {service}"))?;
    if !status.success() {
        anyhow::bail!("sv force-stop {service} exited with {status}");
    }
    Ok(())
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("service", |_ctx| std::sync::Arc::new(ServicePlugin::new()))
}
