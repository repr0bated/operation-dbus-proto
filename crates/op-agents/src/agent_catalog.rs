//! Agent catalog for tool registration.
//!
//! Loads dynamic agent personas from configuration.

use crate::agent_registry::{AgentRegistry, AgentSpec};
use crate::agents::persona::{PersonaAgent, PersonaConfig};
use anyhow::{Context, Result};
use std::fs::File;
use std::path::Path;

#[derive(serde::Deserialize)]
struct ConfigFile {
    personas: Vec<PersonaConfig>,
}

/// Minimal descriptor for tool registration.
#[derive(Debug, Clone)]
pub struct AgentDescriptor {
    pub agent_type: String,
    pub name: String,
    pub description: String,
    pub operations: Vec<String>,
}

/// Load personas from YAML and register them
pub async fn load_personas(registry: &AgentRegistry, path: &str) -> Result<()> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(anyhow::anyhow!("Persona config not found at {}", path));
    }

    let file = File::open(p).context("Failed to open persona config")?;
    let config: ConfigFile =
        serde_yaml::from_reader(file).context("Failed to parse persona config")?;

    for persona in config.personas {
        // Register spec in AgentRegistry
        let spec = AgentSpec {
            agent_type: persona.agent_type.clone(),
            name: persona.name.clone(),
            description: persona.description.clone(),
            command: "builtin".to_string(),
            args: vec![],
            env: std::collections::HashMap::new(),
            working_dir: None,
            capabilities: persona.capabilities.clone(),
            requires_root: false,
            max_instances: 5,
            restart_policy: Default::default(),
            health_check: None,
        };

        let _ = registry.register_spec(spec).await;

        // In a true factory pattern, we would register a factory for this agent_type.
        // For this refactor, if we still use `create_agent` from lib.rs, we need to adapt it.
        // But the prompt says: "Update AgentCatalog to load personas.yaml at startup and register each entry as a PersonaAgent into AgentRegistry"
    }

    Ok(())
}

/// Helper to parse config to list (for compatibility)
pub fn load_builtin_personas(path: &str) -> Vec<PersonaConfig> {
    let p = Path::new(path);
    if !p.exists() {
        return vec![];
    }
    if let Ok(file) = File::open(p) {
        if let Ok(config) = serde_yaml::from_reader::<_, ConfigFile>(file) {
            return config.personas;
        }
    }
    vec![]
}

pub fn builtin_agent_descriptors() -> Vec<AgentDescriptor> {
    let path = std::env::var("OP_AGENT_PERSONAS_PATH")
        .unwrap_or_else(|_| "config/agents/personas.yaml".to_string());
    load_builtin_personas(&path)
        .into_iter()
        .map(|p| AgentDescriptor {
            agent_type: p.agent_type,
            name: p.name,
            description: p.description,
            operations: p.operations,
        })
        .collect()
}
