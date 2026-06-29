//! `persona` StatePlugin — projects the live agent-persona catalog from
//! `op-agents` through the `org.dbus.v1.plugins` surface (PluginService).
//!
//! This is the correct home for the capability the Lovable frontend mistakenly
//! called as a standalone `operation.agents.v1.PersonaService` gRPC package —
//! there is no such proto and there must not be one. New backend capabilities
//! register here as plugins (see OD-30 / [`crate::default_registry`]).
//!
//! The catalog is **live, not mocked**: the plugin's live state enumerates the
//! real built-in agent personas via `op_agents::builtin_agent_descriptors()`,
//! so the projection can never drift from the actual agent implementations. The
//! persona set is defined in code (op-agents), so it is read-only: mutating a
//! persona is rejected honestly rather than silently succeeding.

use super::plugin_schema_defs::schema_from_state;
use anyhow::Result;
use async_trait::async_trait;
use op_agents::builtin_agent_descriptors;
use op_state::{
    ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateAction, StateDiff,
    StatePlugin,
};
use op_state_store::{PluginSchema, SideEffect};
use serde::{Deserialize, Serialize};
use simd_json::{json, OwnedValue as Value};

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Persona {
    pub agent_type: String,
    pub name: String,
    pub description: String,
    pub operations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaState {
    pub status: String,
    pub persona_count: usize,
    pub personas: Vec<Persona>,
}

pub struct PersonaPlugin;

impl Default for PersonaPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl PersonaPlugin {
    pub fn new() -> Self {
        Self
    }

    /// Enumerate the live persona catalog from the real agent implementations.
    /// Pure (no I/O) and deterministic, so it backs both the live state query
    /// and the schema exemplar.
    fn live() -> PersonaState {
        let personas: Vec<Persona> = builtin_agent_descriptors()
            .into_iter()
            .map(|d| Persona {
                agent_type: d.agent_type,
                name: d.name,
                description: d.description,
                operations: d.operations,
            })
            .collect();
        PersonaState {
            status: "active".to_string(),
            persona_count: personas.len(),
            personas,
        }
    }
}

#[async_trait]
impl StatePlugin for PersonaPlugin {
    fn name(&self) -> &str {
        "persona"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<PluginSchema> {
        Some(persona_schema())
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        // The persona catalog is defined in code (op-agents); it is not runtime
        // mutable, so there is never anything to apply.
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![StateAction::NoOp {
                resource: "personas".to_string(),
            }],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: String::new(),
                desired_hash: String::new(),
            },
        })
    }

    async fn apply_state(&self, diff: &StateDiff) -> Result<ApplyResult> {
        // Honest read-only behavior: NoOp is fine; any real mutation is rejected
        // with an explanation rather than pretending to succeed.
        let mut errors = Vec::new();
        for action in &diff.actions {
            match action {
                StateAction::NoOp { .. } => {}
                other => errors.push(format!(
                    "persona catalog is code-defined (op-agents) and read-only; \
                     cannot apply {:?}",
                    other
                )),
            }
        }
        Ok(ApplyResult {
            success: errors.is_empty(),
            changes_applied: Vec::new(),
            errors,
            checkpoint: None,
        })
    }

    async fn verify_state(&self, desired: &Value) -> Result<bool> {
        let current = simd_json::serde::to_owned_value(Self::live())?;
        Ok(&current == desired)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: simd_json::serde::to_owned_value(Self::live())?,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        // Nothing to roll back: the catalog is code-defined.
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}

pub(crate) fn persona_schema() -> PluginSchema {
    let state =
        simd_json::serde::to_owned_value(PersonaPlugin::live()).unwrap_or_else(|_| json!({}));
    let mut schema = schema_from_state(
        "persona",
        "agents",
        "1.0.0",
        "Agent persona catalog — built-in agent types, names, descriptions, operations",
        &state,
    );

    // Add D-Bus methods for persona
    // https://docs.rs/crate/op-agents/latest/source/builtin_agents.rs
    schema.methods.insert(
        "get_identity".to_string(),
        super::plugin_schema_defs::method_decl_from_schemars::<GetIdentityInput>(
            "GetIdentity",
            SideEffect::Read,
            true,
            "persona.read",
            "obs.agent.persona.identity.get@v1",
        ),
    );
    schema.methods.insert(
        "set_identity".to_string(),
        super::plugin_schema_defs::method_decl_from_schemars::<SetIdentityInput>(
            "SetIdentity",
            SideEffect::Mutation,
            false,
            "persona.write",
            "mut.agent.persona.identity.set@v1",
        ),
    );
    schema.methods.insert(
        "authenticate".to_string(),
        super::plugin_schema_defs::method_decl_from_schemars::<AuthenticateInput>(
            "Authenticate",
            SideEffect::Mutation,
            false,
            "persona.write",
            "mut.agent.persona.authenticate@v1",
        ),
    );

    schema
}

/// Input struct for GetIdentity method
/// D-Bus method spec: https://docs.rs/crate/op-agents/latest/source/builtin_agents.rs
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetIdentityInput {
    /// Agent type to get identity for
    pub agent_type: String,
}

/// Input struct for SetIdentity method
/// D-Bus method spec: https://docs.rs/crate/op-agents/latest/source/builtin_agents.rs
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SetIdentityInput {
    /// Agent type to set identity for
    pub agent_type: String,
    /// New name
    pub name: String,
    /// New description
    pub description: Option<String>,
}

/// Input struct for Authenticate method
/// D-Bus method spec: https://docs.rs/crate/op-agents/latest/source/builtin_agents.rs
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AuthenticateInput {
    /// Agent type to authenticate
    pub agent_type: String,
    /// Credentials
    pub credentials: std::collections::HashMap<String, String>,
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("persona", |_ctx| std::sync::Arc::new(PersonaPlugin::new()))
}
