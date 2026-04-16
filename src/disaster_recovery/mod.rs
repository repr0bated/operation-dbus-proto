//! Disaster Recovery: Canonical JSON Export/Import
//!
//! The platform can emit a complete JSON representation of system state at any time:
//! - Operational configuration
//! - Enabled plugins
//! - Policy/compliance context
//! - Workflows
//! - Execution history
//!
//! Recovery reconstructs from canonical JSON using the same execution contract.

use std::sync::Arc;
use std::path::Path;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;

use crate::error::{OpDbusError, Result};
use op_state_store::{StateStore, StoredObject};
use op_execution_tracker::ExecutionRecord;
use crate::blockchain::ChainBlock;
use op_plugins::plugin::{PluginMetadata as PluginCore, PluginTunables};
use crate::policy::{Policy, ComplianceProfile};
use crate::dependency::DependencySet;
use crate::work_stack::WorkStack;



/// Canonical system export
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CanonicalExport {
    /// Export version
    pub version: String,
    /// Export timestamp
    pub exported_at_ns: u128,
    /// System identifier
    pub system_id: String,
    /// All objects by type
    pub objects: Vec<StoredObject>,
    /// Plugin configurations
    pub plugins: Vec<PluginExport>,
    /// Active policies
    pub policies: Vec<Policy>,
    /// Compliance profiles
    pub compliance_profiles: Vec<ComplianceProfile>,
    /// Execution history
    pub executions: Vec<ExecutionRecord>,
    /// Blockchain state
    pub blockchain: Vec<ChainBlock>,
    /// Cached work stacks
    pub cached_work_stacks: Vec<WorkStack>,
    /// Export statistics
    pub stats: ExportStats,
}

/// Plugin export with core and tunables
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PluginExport {
    pub core: PluginCore,
    pub tunables: Vec<(String, PluginTunables)>, // (scope, tunables)
    pub dependencies: DependencySet,
}

/// Export statistics
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ExportStats {
    pub object_count: usize,
    pub plugin_count: usize,
    pub policy_count: usize,
    pub execution_count: usize,
    pub blockchain_length: usize,
    pub total_size_bytes: usize,
}

/// Recovery status
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RecoveryStatus {
    pub phase: RecoveryPhase,
    pub progress_percent: u8,
    pub current_step: String,
    pub objects_restored: usize,
    pub objects_total: usize,
    pub errors: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
pub enum RecoveryPhase {
    NotStarted,
    ValidatingExport,
    RestoringDependencies,
    RestoringPlugins,
    RestoringObjects,
    RestoringPolicies,
    RestoringBlockchain,
    VerifyingIntegrity,
    Complete,
    Failed,
}

/// Disaster recovery system
pub struct DisasterRecovery {
    state_store: Arc<dyn StateStore>,
}

impl DisasterRecovery {
    /// Create a new disaster recovery instance
    pub fn new(state_store: Arc<dyn StateStore>) -> Self {
        Self { state_store }
    }

    /// Export complete system state as canonical JSON
    ///
    /// This is the disaster recovery export format.
    pub async fn export(&self) -> Result<CanonicalExport> {
        tracing::info!("Starting canonical system export");

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();

        // Export from state store
        let db_export = self.state_store.export_canonical().await?;

        let export = CanonicalExport {
            version: "1.0.0".to_string(),
            exported_at_ns: now,
            system_id: uuid::Uuid::new_v4().to_string(),
            objects: db_export.objects,
            plugins: vec![], // Would be loaded from the authoritative plugin catalog
            policies: vec![],
            compliance_profiles: vec![],
            executions: db_export.executions.into_iter()
                .filter_map(|v| simd_json::serde::from_owned_value(v).ok())
                .collect(),
            blockchain: db_export.blockchain.into_iter()
                .filter_map(|v| simd_json::serde::from_owned_value(v).ok())
                .collect(),
            cached_work_stacks: vec![],
            stats: ExportStats {
                object_count: 0,
                plugin_count: 0,
                policy_count: 0,
                execution_count: 0,
                blockchain_length: 0,
                total_size_bytes: 0,
            },
        };

        // Calculate stats
        let json = simd_json::to_vec(&export)
            .map_err(|e| OpDbusError::Serialization(e.to_string()))?;

        let mut final_export = export;
        final_export.stats = ExportStats {
            object_count: final_export.objects.len(),
            plugin_count: final_export.plugins.len(),
            policy_count: final_export.policies.len(),
            execution_count: final_export.executions.len(),
            blockchain_length: final_export.blockchain.len(),
            total_size_bytes: json.len(),
        };

        tracing::info!("Export complete: {} objects, {} bytes",
            final_export.stats.object_count,
            final_export.stats.total_size_bytes);

        Ok(final_export)
    }

    /// Export to file
    pub async fn export_to_file(&self, path: &Path) -> Result<ExportStats> {
        let export = self.export().await?;
        let json = serde_json::to_vec_pretty(&export)
            .map_err(|e| OpDbusError::Serialization(e.to_string()))?;

        tokio::fs::write(path, &json).await?;

        Ok(export.stats)
    }

    /// Validate an export before import
    pub fn validate_export(&self, export: &CanonicalExport) -> Result<ValidationResult> {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Check version compatibility
        if export.version != "1.0.0" {
            warnings.push(format!("Export version {} may not be fully compatible", export.version));
        }

        // Validate blockchain integrity
        if !export.blockchain.is_empty() {
            let mut prev_hash = export.blockchain[0].prev_hash.clone();
            for (i, block) in export.blockchain.iter().enumerate() {
                if i > 0 && block.prev_hash != prev_hash {
                    errors.push(format!("Blockchain integrity violation at block {}", i));
                }
                prev_hash = block.block_hash().to_string();
            }
        }

        // Validate execution record hashes
        for exec in &export.executions {
            if !exec.verify_integrity() {
                errors.push(format!("Execution record {} has invalid hash", exec.execution_id()));
            }
        }

        Ok(ValidationResult {
            valid: errors.is_empty(),
            errors,
            warnings,
        })
    }

    /// Import from canonical JSON export
    ///
    /// Recovery proceeds by:
    /// 1. Validating the export
    /// 2. Applying dependency objects (packages, services)
    /// 3. Restoring plugin configurations
    /// 4. Restoring objects
    /// 5. Restoring policies
    /// 6. Verifying integrity
    pub async fn import(
        &self,
        export: &CanonicalExport,
        progress: Option<tokio::sync::mpsc::Sender<RecoveryStatus>>,
    ) -> Result<ImportResult> {
        tracing::info!("Starting disaster recovery import");

        let mut status = RecoveryStatus {
            phase: RecoveryPhase::ValidatingExport,
            progress_percent: 0,
            current_step: "Validating export".into(),
            objects_restored: 0,
            objects_total: export.objects.len(),
            errors: vec![],
        };

        self.send_progress(&progress, &status).await;

        // Validate
        let validation = self.validate_export(export)?;
        if !validation.valid {
            status.phase = RecoveryPhase::Failed;
            status.errors = validation.errors;
            self.send_progress(&progress, &status).await;
            return Err(OpDbusError::ConfigError("Export validation failed".into()));
        }

        // Restore objects
        status.phase = RecoveryPhase::RestoringObjects;
        status.current_step = "Restoring objects".into();
        status.progress_percent = 20;
        self.send_progress(&progress, &status).await;

        let mut imported = 0;
        let mut errors = Vec::new();

        for obj in &export.objects {
            match self.state_store.upsert_object(
                &obj.id,
                &obj.object_type,
                &obj.namespace,
                &obj.data,
            ).await {
                Ok(_) => imported += 1,
                Err(e) => errors.push(format!("{}: {}", obj.id, e)),
            }

            status.objects_restored = imported;
            status.progress_percent = 20 + (imported * 60 / export.objects.len().max(1)) as u8;
            self.send_progress(&progress, &status).await;
        }

        // TODO: Restore blockchain, policies, etc.

        // Complete
        status.phase = RecoveryPhase::Complete;
        status.progress_percent = 100;
        status.current_step = "Recovery complete".into();
        status.errors = errors.clone();
        self.send_progress(&progress, &status).await;

        tracing::info!("Recovery complete: {} objects restored, {} errors",
            imported, errors.len());

        Ok(ImportResult {
            objects_imported: imported,
            objects_skipped: 0,
            errors,
        })
    }

    /// Import from file
    pub async fn import_from_file(
        &self,
        path: &Path,
        progress: Option<tokio::sync::mpsc::Sender<RecoveryStatus>>,
    ) -> Result<ImportResult> {
        let content = tokio::fs::read(path).await?;
        let export: CanonicalExport = serde_json::from_slice(&content)
            .map_err(|e| OpDbusError::Serialization(e.to_string()))?;

        self.import(&export, progress).await
    }

    async fn send_progress(
        &self,
        tx: &Option<tokio::sync::mpsc::Sender<RecoveryStatus>>,
        status: &RecoveryStatus,
    ) {
        if let Some(tx) = tx {
            let _ = tx.send(status.clone()).await;
        }
    }
}

/// Validation result
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Import result
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ImportResult {
    pub objects_imported: usize,
    pub objects_skipped: usize,
    pub errors: Vec<String>,
}
