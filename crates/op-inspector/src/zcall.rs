//! zcall Introspection Adapter
//!
//! Introspects the complete zcall plugin/method surface, discovering:
//! - Every plugin (`zcall list`)
//! - Every method per plugin (`zcall methods <plugin>`)
//! - Each method's full typed schema (`zcall help <plugin> <method>`)
//!
//! Unlike `gcloud.rs` (which must regex-parse freeform `--help` text), zcall
//! already emits structured JSON for every introspection command, so this
//! adapter's parser is just JSON deserialization -- no fragile text
//! scraping. Follows the same shape (cached async shell-out parser + typed
//! schema structs) documented in `ADAPTER-WORKFLOW.md`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info};

/// Full zcall plugin/method surface, as discovered live.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZcallSchema {
    pub schema_version: String,
    pub plugin_count: usize,
    pub method_count: usize,
    pub plugins: HashMap<String, ZcallPlugin>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZcallPlugin {
    pub name: String,
    pub methods: HashMap<String, ZcallMethod>,
}

/// One method's full typed schema, as returned by `zcall help <plugin> <method>`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZcallMethod {
    pub name: String,
    pub side_effect: String,
    pub required_capability: String,
    pub subid: String,
    pub idempotent: bool,
    pub args: simd_json::OwnedValue,
    pub returns: simd_json::OwnedValue,
    pub grpc_path: String,
}

pub struct ZcallParser {
    /// Cache keyed by `"<plugin>.<method>"` (or `"__list__"`/`"__methods__.<plugin>"`).
    cache: Arc<Mutex<HashMap<String, String>>>,
    zcall_bin: String,
}

impl Default for ZcallParser {
    fn default() -> Self {
        Self::new()
    }
}

impl ZcallParser {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            zcall_bin: "zcall".to_string(),
        }
    }

    async fn run_cached(&self, cache_key: &str, args: &[&str]) -> Result<String> {
        {
            let cache = self.cache.lock().await;
            if let Some(cached) = cache.get(cache_key) {
                return Ok(cached.clone());
            }
        }

        let output = tokio::process::Command::new(&self.zcall_bin)
            .args(args)
            .output()
            .await
            .with_context(|| format!("failed to run zcall {}", args.join(" ")))?;

        let text = String::from_utf8_lossy(&output.stdout).to_string();

        {
            let mut cache = self.cache.lock().await;
            cache.insert(cache_key.to_string(), text.clone());
        }

        Ok(text)
    }

    /// List every plugin (`zcall list`).
    pub async fn list_plugins(&self) -> Result<Vec<String>> {
        let text = self.run_cached("__list__", &["list"]).await?;
        Ok(text.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
    }

    /// List every method name for one plugin (`zcall methods <plugin>`,
    /// first whitespace-separated column of each line).
    pub async fn list_methods(&self, plugin: &str) -> Result<Vec<String>> {
        let cache_key = format!("__methods__.{plugin}");
        let text = self.run_cached(&cache_key, &["methods", plugin]).await?;
        Ok(text
            .lines()
            .filter_map(|l| l.split_whitespace().next())
            .map(|s| s.to_string())
            .collect())
    }

    /// Full typed schema for one method (`zcall help <plugin> <method>`).
    pub async fn get_method(&self, plugin: &str, method: &str) -> Result<ZcallMethod> {
        let cache_key = format!("{plugin}.{method}");
        let text = self
            .run_cached(&cache_key, &["help", plugin, method])
            .await?;
        let mut text_mut = text.clone();
        let raw: simd_json::OwnedValue = unsafe { simd_json::from_str(&mut text_mut) }
            .with_context(|| format!("invalid JSON from zcall help {plugin} {method}"))?;

        Ok(ZcallMethod {
            name: raw
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or(method)
                .to_string(),
            side_effect: raw
                .get("side_effect")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            required_capability: raw
                .get("required_capability")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            subid: raw.get("subid").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            idempotent: raw.get("idempotent").and_then(|v| v.as_bool()).unwrap_or(false),
            args: raw.get("args").cloned().unwrap_or_default(),
            returns: raw.get("returns").cloned().unwrap_or_default(),
            grpc_path: raw
                .get("grpc_path")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        })
    }

    /// Full introspection sweep: every plugin, every method, every schema.
    /// Mirrors gcloud's `introspect_full` -- heavy (one subprocess per
    /// method, ~333 calls at time of writing), cached per-call so repeat
    /// sweeps within the same parser instance are cheap.
    pub async fn introspect_full(&self) -> Result<ZcallSchema> {
        let plugin_names = self.list_plugins().await?;
        info!(count = plugin_names.len(), "zcall: discovered plugins");

        let mut plugins = HashMap::new();
        let mut method_count = 0usize;

        for plugin_name in &plugin_names {
            let method_names = match self.list_methods(plugin_name).await {
                Ok(m) => m,
                Err(e) => {
                    debug!(plugin = %plugin_name, error = %e, "zcall: skipping plugin with no methods");
                    continue;
                }
            };

            let mut methods = HashMap::new();
            for method_name in &method_names {
                match self.get_method(plugin_name, method_name).await {
                    Ok(m) => {
                        methods.insert(method_name.clone(), m);
                        method_count += 1;
                    }
                    Err(e) => {
                        debug!(plugin = %plugin_name, method = %method_name, error = %e, "zcall: failed to fetch method schema");
                    }
                }
            }

            plugins.insert(
                plugin_name.clone(),
                ZcallPlugin {
                    name: plugin_name.clone(),
                    methods,
                },
            );
        }

        Ok(ZcallSchema {
            schema_version: "1.0".to_string(),
            plugin_count: plugins.len(),
            method_count,
            plugins,
        })
    }
}

pub async fn introspect_zcall() -> Result<ZcallSchema> {
    ZcallParser::new().introspect_full().await
}
