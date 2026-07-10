//! Routing — D-Bus authority for identity injection, subdomains, and internal services.
//!
//! The D-Bus object `/org/opdbus/v1/plugins/routing` is the payload. gRPC is
//! transport. `/dev/shm/xray-routes.json` is an optional secondary projection
//! for xray-core (cannot speak D-Bus). If they disagree, D-Bus wins — re-publish.
//!
//! Tags come from the subid registry; this plugin does not invent a second
//! taxonomy and has no LLM / model fields. Compliance/Cozo is out of scope.

use anyhow::{Context, Result};
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::Path;

/// Canonical subid registry (tag vocabulary + backends).
pub const SUBID_REGISTRY: &str = "/etc/ghostbridge/subid-registry.json";
/// Secondary projection for xray-core / identity shuttle.
pub const XRAY_ROUTES_PATH: &str = "/dev/shm/xray-routes.json";
/// Companion OpenFlow rules projection (legacy shape from op-gemma).
pub const OPENFLOW_ROUTES_PATH: &str = "/dev/shm/openflow-routes.json";

/// Where identity headers are stamped.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[schemars(extend("x-oscal-subid" = "src.network.routing.identity-injection.stamp-at@v1"))]
pub enum StampAt {
    /// Decoy VPS WireGuard ingress (authoritative for this host).
    DecoyWgIngress,
}

impl Default for StampAt {
    fn default() -> Self {
        Self::DecoyWgIngress
    }
}

/// Where stamped traffic is forwarded.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[schemars(extend("x-oscal-subid" = "src.network.routing.identity-injection.forward-to@v1"))]
pub enum ForwardTo {
    /// 3tched tonic-web / op-grpc-bridge-zeroclaw (:8090 / mesh alias).
    ThreetchedTonicWeb,
}

impl Default for ForwardTo {
    fn default() -> Self {
        Self::ThreetchedTonicWeb
    }
}

/// Identity-injection hop contract (verbal, first-class).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "src.network.routing.identity-injection@v1"))]
pub struct IdentityInjection {
    #[serde(default = "default_true")]
    #[schemars(extend("x-oscal-subid" = "mut.network.routing.identity-injection.enabled@v1"))]
    pub enabled: bool,
    /// Headers stamped at the decoy hop.
    #[serde(default = "default_stamp_headers")]
    #[schemars(extend("x-oscal-subid" = "sch.network.routing.identity-injection.headers@v1"))]
    pub stamp_headers: Vec<String>,
    #[serde(default)]
    pub stamp_at: StampAt,
    #[serde(default)]
    pub forward_to: ForwardTo,
    #[serde(default = "default_identity_notes")]
    #[schemars(extend("x-oscal-subid" = "exp.network.routing.identity-injection.notes@v1"))]
    pub notes: String,
}

impl Default for IdentityInjection {
    fn default() -> Self {
        Self {
            enabled: true,
            stamp_headers: default_stamp_headers(),
            stamp_at: StampAt::DecoyWgIngress,
            forward_to: ForwardTo::ThreetchedTonicWeb,
            notes: default_identity_notes(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_stamp_headers() -> Vec<String> {
    vec![
        "X-Ghostbridge-Footprint".to_string(),
        "X-Ghostbridge-Trace-ID".to_string(),
        "X-WireGuard-Pubkey".to_string(),
    ]
}

fn default_identity_notes() -> String {
    "Identity is stamped at decoy WG ingress and forwarded to 3tched tonic-web :8090. \
     Do not read local wg show on 3tched for identity."
        .to_string()
}

/// Backend kind for a public subdomain route.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[schemars(extend("x-oscal-subid" = "sch.network.routing.backend-kind@v1"))]
pub enum BackendKind {
    Ingress,
    Unix,
    Tcp,
    Grpc,
    Dns,
}

/// One public subdomain → service route.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "exp.network.routing.subdomain.render@v1"))]
pub struct SubdomainRoute {
    /// Xray outbound / OpenFlow tag (routing key).
    #[schemars(extend("x-oscal-subid" = "sch.network.routing.subdomain.tag@v1"))]
    pub tag: String,
    /// SNI / Host match list.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "sch.network.routing.subdomain.hosts@v1"))]
    pub hosts: Vec<String>,
    /// Human / registry subject name.
    #[serde(default)]
    pub service: String,
    pub backend_kind: BackendKind,
    /// Descriptive ref: unix path, host:port, or "ingress". Not an invented NIC.
    #[serde(default)]
    pub backend_ref: String,
    /// Full backend object for SHM catalog compatibility (op-gemma shape).
    #[serde(default)]
    pub backend: serde_json::Value,
    #[serde(default = "default_true")]
    pub public: bool,
}

/// Non-routable internal / file / schema subject.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "obs.network.routing.internal-service@v1"))]
pub struct InternalService {
    pub tag: String,
    #[serde(default)]
    pub service: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub routable: bool,
}

/// Authoritative routing plugin state (D-Bus payload).
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.network.plugin.routing.schema@v1"))]
pub struct RoutingState {
    #[serde(default)]
    pub identity_injection: IdentityInjection,
    #[serde(default)]
    pub subdomains: Vec<SubdomainRoute>,
    #[serde(default)]
    pub internal_services: Vec<InternalService>,
    #[serde(default = "default_catalog_path")]
    #[schemars(extend("x-oscal-subid" = "src.network.routing.catalog-path@v1"))]
    pub catalog_path: String,
    /// empty | partial | ready | unavailable
    #[serde(default = "default_status")]
    #[schemars(extend("x-oscal-subid" = "obs.network.routing.status@v1"))]
    pub status: String,
}

impl Default for RoutingState {
    fn default() -> Self {
        Self {
            identity_injection: IdentityInjection::default(),
            subdomains: vec![],
            internal_services: vec![],
            catalog_path: default_catalog_path(),
            status: default_status(),
        }
    }
}

fn default_catalog_path() -> String {
    XRAY_ROUTES_PATH.to_string()
}

fn default_status() -> String {
    "empty".to_string()
}

// ── Registry parse (same vocabulary as op-gemma) ─────────────────────────────

#[derive(Debug, Clone, Deserialize)]
struct SubidRegistryFile {
    #[allow(dead_code)]
    version: u32,
    entries: Vec<RegistryEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct RegistryEntry {
    #[allow(dead_code)]
    uuid: String,
    #[allow(dead_code)]
    subid: String,
    subject: String,
    tags: Vec<String>,
    #[serde(default)]
    subdomains: Vec<String>,
    #[serde(default)]
    gemma: Option<GemmaConfig>,
}

#[derive(Debug, Clone, Deserialize)]
struct GemmaConfig {
    backend: serde_json::Value,
}

/// SHM catalog shape consumed by the identity shuttle (compatible with op-gemma).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XrayRoutingMap {
    pub version: u32,
    pub xray_routes: Vec<XrayRouteOut>,
    pub openflow_rules: Vec<OpenFlowRuleOut>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XrayRouteOut {
    pub tag: String,
    pub subdomains: Vec<String>,
    pub backend: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenFlowRuleOut {
    pub priority: u16,
    #[serde(rename = "match_fields")]
    pub match_fields: HashMap<String, String>,
    pub actions: Vec<String>,
    pub description: Option<String>,
}

fn backend_kind_and_ref(backend: &serde_json::Value) -> Option<(BackendKind, String, bool)> {
    let ty = backend.get("type")?.as_str()?;
    match ty {
        "ingress" => Some((BackendKind::Ingress, "ingress".to_string(), true)),
        "unix" => {
            let path = backend
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Some((BackendKind::Unix, path, true))
        }
        "tcp" => {
            let host = backend.get("host").and_then(|v| v.as_str()).unwrap_or("");
            let port = backend.get("port").and_then(|v| v.as_u64()).unwrap_or(0);
            Some((BackendKind::Tcp, format!("{host}:{port}"), true))
        }
        "grpc" => {
            let host = backend.get("host").and_then(|v| v.as_str()).unwrap_or("");
            let port = backend.get("port").and_then(|v| v.as_u64()).unwrap_or(0);
            Some((BackendKind::Grpc, format!("{host}:{port}"), true))
        }
        "dns" => {
            let host = backend.get("host").and_then(|v| v.as_str()).unwrap_or("");
            let port = backend.get("port").and_then(|v| v.as_u64()).unwrap_or(53);
            Some((BackendKind::Dns, format!("{host}:{port}"), true))
        }
        "file" => {
            let path = backend
                .get("path")
                .and_then(|v| v.as_str())
                .unwrap_or("file");
            Some((BackendKind::Unix, path.to_string(), false)) // kind unused when !routable
        }
        "internal" => Some((BackendKind::Unix, "internal".to_string(), false)),
        _ => None,
    }
}

/// Derive verbal routing state from the subid registry.
pub fn derive_from_registry(registry_path: &str) -> Result<RoutingState> {
    let raw = fs::read_to_string(registry_path)
        .with_context(|| format!("failed to read subid registry: {registry_path}"))?;
    let parsed: SubidRegistryFile =
        serde_json::from_str(&raw).context("failed to parse subid registry")?;

    let mut subdomains = Vec::new();
    let mut internal_services = Vec::new();
    let mut seen_tags = std::collections::HashSet::new();

    for entry in &parsed.entries {
        let Some(gemma) = &entry.gemma else {
            continue;
        };
        let backend = &gemma.backend;
        let Some((kind, backend_ref, routable)) = backend_kind_and_ref(backend) else {
            continue;
        };

        let tag = entry
            .tags
            .first()
            .cloned()
            .unwrap_or_else(|| entry.subject.clone());

        if !seen_tags.insert(tag.clone()) {
            // First declaration wins (same as op-gemma).
            continue;
        }

        if !routable {
            let reason = backend
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("internal")
                .to_string();
            internal_services.push(InternalService {
                tag,
                service: entry.subject.clone(),
                reason,
                routable: false,
            });
            continue;
        }

        subdomains.push(SubdomainRoute {
            tag,
            hosts: entry.subdomains.clone(),
            service: entry.subject.clone(),
            backend_kind: kind,
            backend_ref,
            backend: backend.clone(),
            public: true,
        });
    }

    let status = if subdomains.is_empty() && internal_services.is_empty() {
        "empty"
    } else if subdomains.is_empty() {
        "partial"
    } else {
        "ready"
    }
    .to_string();

    Ok(RoutingState {
        identity_injection: IdentityInjection::default(),
        subdomains,
        internal_services,
        catalog_path: XRAY_ROUTES_PATH.to_string(),
        status,
    })
}

fn default_privacy_chain() -> Vec<OpenFlowRuleOut> {
    vec![
        OpenFlowRuleOut {
            priority: 100,
            match_fields: HashMap::from([("in_port".to_string(), "gbr_wg".to_string())]),
            actions: vec!["output:gbr_warp".to_string()],
            description: Some("gbr_wg -> gbr_warp".to_string()),
        },
        OpenFlowRuleOut {
            priority: 100,
            match_fields: HashMap::from([("in_port".to_string(), "gbr_warp".to_string())]),
            actions: vec!["output:gbr_xray".to_string()],
            description: Some("gbr_warp -> gbr_xray".to_string()),
        },
        OpenFlowRuleOut {
            priority: 100,
            match_fields: HashMap::from([("in_port".to_string(), "gbr_xray".to_string())]),
            actions: vec!["output:gbr_warp".to_string()],
            description: Some("gbr_xray -> gbr_warp".to_string()),
        },
        OpenFlowRuleOut {
            priority: 200,
            match_fields: HashMap::from([("dl_type".to_string(), "0x0806".to_string())]),
            actions: vec!["arp_responder".to_string()],
            description: Some("ARP Responder for privacy network".to_string()),
        },
    ]
}

fn backend_to_openflow(tag: &str, backend: &serde_json::Value) -> Option<OpenFlowRuleOut> {
    let ty = backend.get("type")?.as_str()?;
    let (host, port) = match ty {
        "tcp" | "grpc" => (
            backend.get("host")?.as_str()?.to_string(),
            backend.get("port")?.as_u64()? as u16,
        ),
        _ => return None,
    };
    let mut match_fields = HashMap::new();
    match_fields.insert("dl_type".to_string(), "0x0800".to_string());
    match_fields.insert("nw_proto".to_string(), "6".to_string());
    match_fields.insert("nw_dst".to_string(), host.clone());
    match_fields.insert("tp_dst".to_string(), port.to_string());
    Some(OpenFlowRuleOut {
        priority: 150,
        match_fields,
        actions: vec!["output:ovsbr0-mgmt".to_string()],
        description: Some(format!("routing: {tag} -> {host}:{port}")),
    })
}

/// Build SHM catalog from authoritative plugin state.
pub fn state_to_catalog(state: &RoutingState) -> XrayRoutingMap {
    let mut openflow_rules = default_privacy_chain();
    let mut xray_routes = Vec::new();
    for route in &state.subdomains {
        if let Some(rule) = backend_to_openflow(&route.tag, &route.backend) {
            openflow_rules.push(rule);
        }
        xray_routes.push(XrayRouteOut {
            tag: route.tag.clone(),
            subdomains: route.hosts.clone(),
            backend: route.backend.clone(),
        });
    }
    XrayRoutingMap {
        version: 1,
        xray_routes,
        openflow_rules,
    }
}

/// Atomic write of JSON to path (tmp + rename).
pub fn write_json_atomic(path: &str, value: &impl Serialize) -> Result<()> {
    let tmp = format!("{path}.tmp");
    let mut f = fs::File::create(&tmp).with_context(|| format!("create tmp {tmp}"))?;
    let json = serde_json::to_string_pretty(value).context("serialize routing catalog")?;
    f.write_all(json.as_bytes())
        .with_context(|| format!("write tmp {tmp}"))?;
    f.sync_data().ok();
    fs::rename(&tmp, path).with_context(|| format!("rename {tmp} -> {path}"))?;
    Ok(())
}

/// Publish secondary SHM projections from D-Bus state.
pub fn publish_catalog(state: &RoutingState) -> Result<XrayRoutingMap> {
    let catalog = state_to_catalog(state);
    let path = if state.catalog_path.is_empty() {
        XRAY_ROUTES_PATH
    } else {
        state.catalog_path.as_str()
    };
    write_json_atomic(path, &catalog)?;
    // Companion openflow file (same map) for legacy consumers.
    if Path::new("/dev/shm").exists() {
        let _ = write_json_atomic(OPENFLOW_ROUTES_PATH, &catalog);
    }
    Ok(catalog)
}

// ── Plugin ───────────────────────────────────────────────────────────────────

pub struct RoutingPlugin;

impl Default for RoutingPlugin {
    fn default() -> Self {
        Self
    }
}

impl RoutingPlugin {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl StatePlugin for RoutingPlugin {
    fn name(&self) -> &str {
        "routing"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<PluginSchema> {
        let mut schema = routing_schema();
        super::common::oscal::ensure_category_metadata_fields(&mut schema);
        Some(schema)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "schema-declared".to_string(),
                desired_hash: "schema-declared".to_string(),
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
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: simd_json::serde::to_owned_value(RoutingState::default())?,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
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

/// Canonical `routing` schema.
pub fn routing_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(RoutingState))
        .expect("schemars schema serializes");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "routing",
        "1.0.0",
        "Routing authority: identity injection, subdomain→backend map, and internal (non-routable) services. D-Bus object is the payload; SHM catalog is optional secondary.",
        &root,
    );

    use super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;
    use op_state_store::SideEffect;

    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    struct GetStateOutput {
        state: RoutingState,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    struct ListSubdomainsOutput {
        subdomains: Vec<SubdomainRoute>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    struct ListInternalOutput {
        internal_services: Vec<InternalService>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    struct GetIdentityInjectionOutput {
        identity_injection: IdentityInjection,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    struct ListTagsOutput {
        tags: Vec<String>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    struct DeriveOutput {
        state: RoutingState,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    struct PublishOutput {
        catalog_path: String,
        route_count: u32,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    struct UpsertSubdomainInput {
        route: SubdomainRoute,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    struct RemoveSubdomainInput {
        tag: String,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    struct AckRouteOutput {
        state: RoutingState,
    }

    schema.methods.insert(
        "get_state".to_string(),
        method_decl_from_schemars_with_output::<(), GetStateOutput>(
            "get_state",
            SideEffect::Read,
            true,
            "routing.read",
            "obs.network.routing.state.get@v1",
        ),
    );
    schema.methods.insert(
        "list_subdomains".to_string(),
        method_decl_from_schemars_with_output::<(), ListSubdomainsOutput>(
            "list_subdomains",
            SideEffect::Read,
            true,
            "routing.read",
            "obs.network.routing.subdomain.list@v1",
        ),
    );
    schema.methods.insert(
        "list_internal".to_string(),
        method_decl_from_schemars_with_output::<(), ListInternalOutput>(
            "list_internal",
            SideEffect::Read,
            true,
            "routing.read",
            "obs.network.routing.internal-service.list@v1",
        ),
    );
    schema.methods.insert(
        "get_identity_injection".to_string(),
        method_decl_from_schemars_with_output::<(), GetIdentityInjectionOutput>(
            "get_identity_injection",
            SideEffect::Read,
            true,
            "routing.read",
            "obs.network.routing.identity-injection.get@v1",
        ),
    );
    schema.methods.insert(
        "list_tags".to_string(),
        method_decl_from_schemars_with_output::<(), ListTagsOutput>(
            "list_tags",
            SideEffect::Read,
            true,
            "routing.read",
            "obs.network.routing.tag.list@v1",
        ),
    );
    schema.methods.insert(
        "derive".to_string(),
        method_decl_from_schemars_with_output::<(), DeriveOutput>(
            "derive",
            SideEffect::Mutation,
            false,
            "routing.invoke",
            "mut.network.routing.catalog.derive@v1",
        ),
    );
    schema.methods.insert(
        "publish".to_string(),
        method_decl_from_schemars_with_output::<(), PublishOutput>(
            "publish",
            SideEffect::Mutation,
            false,
            "routing.invoke",
            "mut.network.routing.catalog.publish@v1",
        ),
    );
    schema.methods.insert(
        "upsert_subdomain".to_string(),
        method_decl_from_schemars_with_output::<UpsertSubdomainInput, AckRouteOutput>(
            "upsert_subdomain",
            SideEffect::Mutation,
            false,
            "routing.invoke",
            "mut.network.routing.subdomain.upsert@v1",
        ),
    );
    schema.methods.insert(
        "remove_subdomain".to_string(),
        method_decl_from_schemars_with_output::<RemoveSubdomainInput, AckRouteOutput>(
            "remove_subdomain",
            SideEffect::Mutation,
            false,
            "routing.invoke",
            "mut.network.routing.subdomain.remove@v1",
        ),
    );

    schema
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state_plugins::common::oscal::validate_subid;

    #[test]
    fn schema_has_core_methods() {
        let schema = routing_schema();
        assert_eq!(schema.name, "routing");
        for m in [
            "get_state",
            "list_subdomains",
            "list_internal",
            "get_identity_injection",
            "list_tags",
            "derive",
            "publish",
            "upsert_subdomain",
            "remove_subdomain",
        ] {
            assert!(schema.methods.contains_key(m), "missing method {m}");
        }
    }

    #[test]
    fn all_subids_are_valid() {
        let raw = serde_json::to_value(schemars::schema_for!(RoutingState)).unwrap();
        let mut stack = vec![&raw];
        while let Some(node) = stack.pop() {
            if let Some(subid) = node.get("x-oscal-subid").and_then(|v| v.as_str()) {
                validate_subid(subid).unwrap_or_else(|e| panic!("bad subid {subid}: {e}"));
            }
            if let Some(props) = node.get("properties").and_then(|v| v.as_object()) {
                for (_, v) in props {
                    stack.push(v);
                }
            }
            if let Some(defs) = node
                .get("$defs")
                .or_else(|| node.get("definitions"))
                .and_then(|v| v.as_object())
            {
                for (_, v) in defs {
                    stack.push(v);
                }
            }
        }
    }

    #[test]
    fn derive_skips_file_and_internal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("registry.json");
        fs::write(
            &path,
            r#"{
              "version": 1,
              "entries": [
                {
                  "uuid": "a", "subid": "src.service.q@v1", "category": "src",
                  "component_type": "service", "subject": "qdrant", "verb": "serve",
                  "version": 1, "tags": ["qdrant"], "subdomains": ["q.example"],
                  "gemma": {"backend": {"type": "tcp", "host": "127.0.0.1", "port": 6333}}
                },
                {
                  "uuid": "b", "subid": "src.service.f@v1", "category": "src",
                  "component_type": "service", "subject": "file-thing", "verb": "serve",
                  "version": 1, "tags": ["file-thing"], "subdomains": [],
                  "gemma": {"backend": {"type": "file", "path": "/tmp/x"}}
                },
                {
                  "uuid": "c", "subid": "src.service.i@v1", "category": "src",
                  "component_type": "service", "subject": "internal-thing", "verb": "serve",
                  "version": 1, "tags": ["internal-thing"], "subdomains": [],
                  "gemma": {"backend": {"type": "internal"}}
                }
              ]
            }"#,
        )
        .unwrap();
        let state = derive_from_registry(path.to_str().unwrap()).unwrap();
        assert_eq!(state.subdomains.len(), 1);
        assert_eq!(state.subdomains[0].tag, "qdrant");
        assert_eq!(state.internal_services.len(), 2);
        let catalog = state_to_catalog(&state);
        assert_eq!(catalog.xray_routes.len(), 1);
    }
}

inventory::submit! {
    crate::default_registry::PluginReg::new("routing", |_ctx| {
        std::sync::Arc::new(RoutingPlugin::new())
    })
}
