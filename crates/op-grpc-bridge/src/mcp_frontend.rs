//! Bridge-owned MCP Streamable HTTP frontend.
//!
//! This module contributes an Axum `/mcp` route to the existing tonic TLS
//! acceptor. It does not bind a socket and it never dispatches over HTTP: tool
//! discovery and execution use the same `MutationEngine` and in-process
//! cognitive tool registry as the generated plugin routes.

use std::collections::{BTreeMap, HashSet};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use axum::body::{to_bytes, Body};
use axum::extract::State;
use axum::http::{HeaderMap, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::Router;
use base64::Engine as _;
use serde::Deserialize;
use serde_json::{json, Value};
use tonic::transport::server::{TcpConnectInfo, TlsConnectInfo};

use crate::grpc_server::DECLARED_CAPABILITY_HEADER;
#[cfg(test)]
use crate::interceptor::capabilities_for_principal;
use crate::interceptor::load_capability_grants;
use crate::mcp_policy::{McpProjectionPolicy, ToolsetDefinition, HOT_TOOL_NAMES};
use crate::mutation_engine::MutationEngine;
use crate::oracle_assertion::AssertionValidator;

pub const MCP_PATH: &str = "/mcp";
pub const MCP_PROTOCOL_VERSION: &str = "2026-07-28";
/// Official MCP spec dates this frontend can speak, plus the local canonical
/// revision. Codex's rmcp client sends `2025-06-18` and will disconnect if
/// initialize errors or returns a date it does not know (`2026-07-28`).
const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &[
    MCP_PROTOCOL_VERSION,
    "2025-06-18",
    "2025-03-26",
    "2024-11-05",
];
/// Latest official MCP spec date this frontend implements. Used when a client
/// requests an unknown revision so stock SDKs can continue.
const LATEST_OFFICIAL_PROTOCOL_VERSION: &str = "2025-06-18";
pub const MCP_VERSION_HEADER: &str = "mcp-protocol-version";
pub const MCP_METHOD_HEADER: &str = "mcp-method";
pub const MCP_NAME_HEADER: &str = "mcp-name";
/// Raw HTTP uses the same canonical header name as gRPC metadata. Its value is
/// the OIA1 wire envelope encoded as unpadded canonical base64url.
pub const HTTP_ASSERTION_HEADER: &str = "x-oracle-identity-assertion-bin";
/// MutationEngine-authored SID1 envelope read from the caller's selected sled.
/// The HTTP spelling is canonical unpadded base64url; the bridge reconstructs
/// the inline `sid1:` value and exact-matches it against the authoritative
/// session record before accepting any claims.
pub const HTTP_SEALED_ID_HEADER: &str = op_identity::sealed_id::HTTP_HEADER_NAME;

const MAX_BODY_BYTES: usize = 1024 * 1024;
const MAX_ASSERTION_BYTES: usize = 16 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(90);
const DEFAULT_PAGE_SIZE: usize = 100;
const MAX_PAGE_SIZE: usize = 500;
const ALLOWED_ORIGINS_ENV: &str = "OP_MCP_ALLOWED_ORIGINS";

#[derive(Clone, Debug)]
struct AuthenticatedCaller {
    principal_id: String,
    session_id: String,
    session_genesis: String,
}

#[derive(Debug)]
struct McpAuthError(String);

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct ToolsetSelection {
    id: String,
    generation: u64,
}

#[async_trait]
trait McpAuthenticator: Send + Sync {
    async fn authenticate(
        &self,
        headers: &HeaderMap,
        peer: Option<SocketAddr>,
    ) -> Result<AuthenticatedCaller, McpAuthError>;
}

#[derive(Clone)]
struct OracleHttpAuthenticator {
    validator: Arc<AssertionValidator>,
    engine: Arc<MutationEngine>,
}

#[async_trait]
impl McpAuthenticator for OracleHttpAuthenticator {
    async fn authenticate(
        &self,
        headers: &HeaderMap,
        peer: Option<SocketAddr>,
    ) -> Result<AuthenticatedCaller, McpAuthError> {
        let assertion =
            one_optional_header(headers, HTTP_ASSERTION_HEADER).map_err(McpAuthError)?;
        let sealed_id =
            one_optional_header(headers, HTTP_SEALED_ID_HEADER).map_err(McpAuthError)?;
        match (assertion, sealed_id) {
            (Some(_), Some(_)) => Err(McpAuthError(
                "send exactly one identity credential, not both OIA1 and SID1".into(),
            )),
            (Some(encoded), None) => self.authenticate_assertion(encoded, peer).await,
            (None, Some(encoded)) => self.authenticate_sealed_id(encoded).await,
            (None, None) => Err(McpAuthError(format!(
                "missing {HTTP_ASSERTION_HEADER} or {HTTP_SEALED_ID_HEADER} header"
            ))),
        }
    }
}

impl OracleHttpAuthenticator {
    async fn authenticate_assertion(
        &self,
        encoded: &str,
        peer: Option<SocketAddr>,
    ) -> Result<AuthenticatedCaller, McpAuthError> {
        if encoded.len() > encoded_len_upper_bound(MAX_ASSERTION_BYTES) {
            return Err(McpAuthError(
                "Oracle identity assertion is too large".into(),
            ));
        }
        let wire = decode_http_assertion(encoded)?;

        let mut pending = self
            .validator
            .validate_pending_with_bootstrap(&wire, peer, chrono::Utc::now().timestamp(), false)
            .map_err(|error| McpAuthError(error.to_string()))?;
        // Match native gRPC ordering: validate signature/binding and resolve the
        // principal, anchor the durable human session, then atomically consume
        // the nonce. A missing session must not burn an otherwise valid nonce.
        let session = self
            .engine
            .ensure_verified_human_session_context(&pending.identity().human_pubkey)
            .await
            .ok_or_else(|| {
                McpAuthError("Assertion identity has no anchored session genesis".into())
            })?;
        pending.identity_mut().session_id = session.session_id;
        pending.identity_mut().session_genesis = session.genesis_hex;
        let identity = self
            .validator
            .consume_pending(pending)
            .map_err(|error| McpAuthError(error.to_string()))?;
        Ok(AuthenticatedCaller {
            principal_id: identity.principal_id,
            session_id: identity.session_id,
            session_genesis: identity.session_genesis,
        })
    }

    async fn authenticate_sealed_id(
        &self,
        encoded: &str,
    ) -> Result<AuthenticatedCaller, McpAuthError> {
        if encoded.len() > encoded_len_upper_bound(MAX_ASSERTION_BYTES) {
            return Err(McpAuthError("sealed identity is too large".into()));
        }
        let wire = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(encoded.as_bytes())
            .map_err(|_| McpAuthError("malformed sealed identity".into()))?;
        if wire.len() > MAX_ASSERTION_BYTES
            || base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&wire) != encoded
        {
            return Err(McpAuthError(
                "sealed identity is not canonical base64url".into(),
            ));
        }
        let claims = op_identity::sealed_id::SealedId::open(&wire)
            .map_err(|error| McpAuthError(error.to_string()))?;
        if claims.principal_kind != "wireguard-principal"
            || !claims
                .transport_scope
                .split(',')
                .any(|scope| scope.trim() == "mcp")
        {
            return Err(McpAuthError(
                "sealed identity is not valid for the MCP transport".into(),
            ));
        }
        if op_identity::session::derive_session_id(&claims.wireguard_pubkey) != claims.session_id
            || op_identity::session::derive_principal_id(&claims.wireguard_pubkey)
                != claims.principal_id
        {
            return Err(McpAuthError(
                "sealed identity identifiers do not match its WireGuard identity".into(),
            ));
        }

        let sled =
            crate::identity_sled_dispatch::stored_session(self.engine.as_ref(), &claims.session_id)
                .await
                .ok_or_else(|| McpAuthError("sealed identity session was not found".into()))?;
        let now = chrono::Utc::now().timestamp();
        if !sled.is_anchored() || !sled.active {
            return Err(McpAuthError(
                "sealed identity session is not active and anchored".into(),
            ));
        }
        if sled
            .expires_at
            .is_some_and(|expires_at| expires_at != 0 && expires_at <= now)
        {
            return Err(McpAuthError("sealed identity session has expired".into()));
        }
        let inline = format!("{}{}", op_identity::sealed_id::INLINE_PREFIX, encoded);
        if sled.sealed_id.as_deref() != Some(inline.as_str())
            || sled.wireguard_pubkey != claims.wireguard_pubkey
            || sled.genesis.as_deref() != Some(claims.session_genesis.as_str())
            || sled.trace_id != claims.trace_id
            || sled.schema_version != claims.schema_version
            || sled.expires_at.unwrap_or(0) != claims.expires_at
            || sled.arrival_timestamp != claims.arrival_timestamp
            || claims.issued_at != claims.arrival_timestamp
            || sled.chain_head_at_arrival != claims.chain_head_at_arrival
            || sled.catalog_hash_at_arrival != claims.catalog_hash_at_arrival
            || sled.head_timestamp_at_arrival != claims.head_timestamp_at_arrival
        {
            return Err(McpAuthError(
                "sealed identity does not match the authoritative sled".into(),
            ));
        }

        // A local sled can exist without MCP authority.  Exact principal-only
        // grants remain the final admission source; no sealed ID/genesis/hash is
        // ever looked up as a grant key.
        if load_exact_capability_grants(&claims.principal_id).is_empty() {
            return Err(McpAuthError(
                "sealed identity principal has no MCP capabilities".into(),
            ));
        }
        match crate::human_principal_dispatch::resolve_key_for_assertion(&claims.wireguard_pubkey)
            .await
        {
            Ok(record) => validate_registered_sealed_id_principal(
                record.as_ref(),
                &claims.principal_id,
                &claims.wireguard_pubkey,
            )?,
            Err(error) => {
                return Err(McpAuthError(format!(
                    "principal registry unavailable: {error:?}"
                )))
            }
        }

        Ok(AuthenticatedCaller {
            principal_id: claims.principal_id,
            session_id: claims.session_id,
            session_genesis: claims.session_genesis,
        })
    }
}

/// SID1 is a possession credential for an already-registered principal, not
/// a registration mechanism.  Keep this check independent and testable so a
/// missing row can never drift back into a fail-open wildcard match.
fn validate_registered_sealed_id_principal(
    record: Option<&op_cozo_store::HumanPrincipalRecord>,
    expected_principal_id: &str,
    expected_wireguard_pubkey: &str,
) -> Result<(), McpAuthError> {
    let record =
        record.ok_or_else(|| McpAuthError("sealed identity principal is not registered".into()))?;
    if record.revoked_at != 0 {
        return Err(McpAuthError("sealed identity principal is revoked".into()));
    }
    if record.principal_id != expected_principal_id
        || record.human_pubkey != expected_wireguard_pubkey
    {
        return Err(McpAuthError(
            "sealed identity principal registry binding changed".into(),
        ));
    }
    Ok(())
}

#[async_trait]
trait McpBackend: Send + Sync {
    async fn list_tools(
        &self,
        caller: &AuthenticatedCaller,
        selection: Option<&ToolsetSelection>,
    ) -> anyhow::Result<Vec<Value>>;

    async fn call_tool(
        &self,
        caller: &AuthenticatedCaller,
        name: &str,
        arguments: Value,
        selection: Option<&ToolsetSelection>,
    ) -> anyhow::Result<Value>;

    async fn list_resources(&self, caller: &AuthenticatedCaller) -> anyhow::Result<Vec<Value>>;
    async fn read_resource(&self, caller: &AuthenticatedCaller, uri: &str)
        -> anyhow::Result<Value>;
}

#[derive(Clone)]
struct MutationEngineMcpBackend {
    engine: Arc<MutationEngine>,
    blob_catalog_dir: PathBuf,
    policy: Arc<McpProjectionPolicy>,
    plugin_registry: Arc<op_plugins::DefaultPluginRegistry>,
}

impl MutationEngineMcpBackend {
    fn new(engine: Arc<MutationEngine>, policy: McpProjectionPolicy) -> Self {
        let blob_catalog_dir = std::env::var("OP_BLOB_CATALOG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(op_blob::catalog::DEFAULT_SHM_DIR));
        let plugin_registry =
            op_plugins::DefaultPluginRegistry::new(Arc::new(op_state_store::MemoryStore::new()));
        Self {
            engine,
            blob_catalog_dir,
            policy: Arc::new(policy),
            plugin_registry: Arc::new(plugin_registry),
        }
    }

    fn authorize(caller: &AuthenticatedCaller, required_capability: &str) -> anyhow::Result<()> {
        let grants = load_exact_capability_grants(&caller.principal_id);
        authorize_with_grants(&grants, required_capability)
    }

    async fn dispatch_plugin(
        &self,
        caller: &AuthenticatedCaller,
        plugin_id: &str,
        method: &str,
        arguments: Value,
        required_capability: &str,
    ) -> anyhow::Result<Value> {
        Self::authorize(caller, required_capability)?;
        // Stable principal identity is the audit actor. Session identity stays
        // in the authenticated request context and never becomes authority.
        let actor = caller.principal_id.clone();
        let result = self
            .engine
            .dispatch_method_call_with_identity(
                plugin_id,
                method,
                &serde_json::to_string(&arguments)?,
                Some(required_capability),
                &actor,
                Some(&caller.session_id),
                Some(&caller.session_genesis),
            )
            .await?;
        Ok(result.get("result").cloned().unwrap_or(result))
    }

    async fn cognitive_admission(
        &self,
        caller: &AuthenticatedCaller,
        method: &str,
        arguments: Value,
        required_capability: &str,
    ) -> anyhow::Result<Value> {
        self.dispatch_plugin(
            caller,
            "cognitive_mcp",
            method,
            arguments,
            required_capability,
        )
        .await
    }

    fn provider_ready(set: &ToolsetDefinition) -> bool {
        !set.requires_provider_health
            || Path::new("/run/opdbus/runit-ready")
                .join(&set.provider)
                .is_file()
    }

    async fn typed_tool_descriptor(&self, public_name: &str) -> anyhow::Result<Value> {
        let (plugin_id, method_name) = parse_typed_tool_name(public_name)?;
        if !op_plugins::DefaultPluginRegistry::available_plugins()
            .iter()
            .any(|candidate| candidate == plugin_id)
        {
            anyhow::bail!("AccessDenied: tool '{public_name}' names an unknown plugin");
        }
        let plugin = self.plugin_registry.load_plugin(plugin_id).await?;
        let schema = plugin
            .schema()
            .ok_or_else(|| anyhow::anyhow!("plugin '{plugin_id}' has no sealed schema"))?;
        method_descriptor(&schema, method_name, public_name)
    }

    fn hot_tool_descriptor(public_name: &str) -> anyhow::Result<Value> {
        let schema = op_plugins::cognitive_mcp_plugin_schema();
        method_descriptor(&schema, public_name, public_name)
    }

    async fn authorized_hot_catalog(&self, grants: &HashSet<String>) -> anyhow::Result<Vec<Value>> {
        let mut tools = Vec::with_capacity(HOT_TOOL_NAMES.len());
        for name in HOT_TOOL_NAMES {
            let descriptor = Self::hot_tool_descriptor(name)?;
            if descriptor_authority(&descriptor, Some(name))
                .is_ok_and(|(capability, _)| grants.contains(capability))
            {
                tools.push(descriptor);
            }
        }
        Ok(tools)
    }

    async fn authorized_set_catalog(
        &self,
        set: &ToolsetDefinition,
        grants: &HashSet<String>,
    ) -> anyhow::Result<Vec<Value>> {
        if !Self::provider_ready(set) {
            anyhow::bail!("provider_unavailable: {}", set.provider);
        }
        let mut tools = Vec::with_capacity(set.tools.len());
        for name in &set.tools {
            let descriptor = self.typed_tool_descriptor(name).await?;
            if descriptor_authority(&descriptor, Some(name))
                .is_ok_and(|(capability, _)| grants.contains(capability))
            {
                tools.push(descriptor);
            }
        }
        Ok(tools)
    }

    fn selected_set<'a>(
        &'a self,
        selection: &ToolsetSelection,
    ) -> anyhow::Result<&'a ToolsetDefinition> {
        if selection.generation != self.policy.toolsets.generation {
            anyhow::bail!(
                "toolset_generation_changed: requested {}, current {}; relist required",
                selection.generation,
                self.policy.toolsets.generation
            );
        }
        self.policy
            .toolset(&selection.id)
            .ok_or_else(|| anyhow::anyhow!("unknown toolset '{}'", selection.id))
    }

    async fn external_catalog(
        &self,
        caller: &AuthenticatedCaller,
        selection: Option<&ToolsetSelection>,
    ) -> anyhow::Result<Vec<Value>> {
        let grants = load_exact_capability_grants(&caller.principal_id);
        let mut by_name = BTreeMap::new();
        for descriptor in self.authorized_hot_catalog(&grants).await? {
            by_name.insert(tool_name(&descriptor).to_string(), descriptor);
        }
        if let Some(selection) = selection {
            let set = self.selected_set(selection)?;
            for descriptor in self.authorized_set_catalog(set, &grants).await? {
                by_name.insert(tool_name(&descriptor).to_string(), descriptor);
            }
        }
        Ok(by_name.into_values().collect())
    }

    async fn toolsets_result(
        &self,
        caller: &AuthenticatedCaller,
        arguments: &Value,
    ) -> anyhow::Result<Value> {
        let grants = load_exact_capability_grants(&caller.principal_id);
        let operation = arguments
            .get("operation")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("toolsets requires operation"))?;
        let mut projected = Vec::new();
        for set in &self.policy.toolsets.sets {
            let authorized_count = self
                .authorized_set_catalog_without_health(set, &grants)
                .await?
                .len();
            if authorized_count == 0 {
                continue;
            }
            projected.push(json!({
                "id": set.id,
                "temperature": set.temperature,
                "provider": set.provider,
                "available": Self::provider_ready(set),
                "authorizedToolCount": authorized_count
            }));
        }

        match operation {
            "list" => Ok(json!({
                "operation": "list",
                "catalog_generation": self.policy.toolsets.generation,
                "relist_required": false,
                "result": {"sets": projected}
            })),
            "select" => {
                let id = arguments
                    .get("toolset_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow::anyhow!("toolsets select requires toolset_id"))?;
                let set = self
                    .policy
                    .toolset(id)
                    .ok_or_else(|| anyhow::anyhow!("unknown toolset '{id}'"))?;
                if !Self::provider_ready(set) {
                    anyhow::bail!("provider_unavailable: {}", set.provider);
                }
                let tools = self.authorized_set_catalog(set, &grants).await?;
                if tools.is_empty() {
                    anyhow::bail!("AccessDenied: no authorized tools in toolset '{id}'");
                }
                Ok(json!({
                    "operation": "select",
                    "catalog_generation": self.policy.toolsets.generation,
                    "relist_required": true,
                    "result": {
                        "selector": {
                            "id": id,
                            "generation": self.policy.toolsets.generation
                        },
                        "tools": tools.into_iter().map(normalize_tool_definition).collect::<Vec<_>>()
                    }
                }))
            }
            _ => anyhow::bail!("toolsets operation must be list or select"),
        }
    }

    async fn authorized_set_catalog_without_health(
        &self,
        set: &ToolsetDefinition,
        grants: &HashSet<String>,
    ) -> anyhow::Result<Vec<Value>> {
        let mut tools = Vec::with_capacity(set.tools.len());
        for name in &set.tools {
            let descriptor = self.typed_tool_descriptor(name).await?;
            if descriptor_authority(&descriptor, Some(name))
                .is_ok_and(|(capability, _)| grants.contains(capability))
            {
                tools.push(descriptor);
            }
        }
        Ok(tools)
    }

    async fn dispatch_projected_tool(
        &self,
        caller: &AuthenticatedCaller,
        descriptor: &Value,
        name: &str,
        arguments: Value,
    ) -> anyhow::Result<Value> {
        let grants = load_exact_capability_grants(&caller.principal_id);
        authorize_and_validate_tool_call(descriptor, name, &arguments, &grants)?;
        let required_capability = descriptor_authority(descriptor, Some(name))?.0;
        let (plugin_id, method_name) = if HOT_TOOL_NAMES.contains(&name) {
            ("cognitive_mcp", name)
        } else {
            parse_typed_tool_name(name)?
        };
        let admitted = self
            .dispatch_plugin(
                caller,
                plugin_id,
                method_name,
                arguments.clone(),
                required_capability,
            )
            .await?;
        if name == "toolsets" {
            self.toolsets_result(caller, &arguments).await
        } else {
            Ok(admitted)
        }
    }
}

#[async_trait]
impl McpBackend for MutationEngineMcpBackend {
    async fn list_tools(
        &self,
        caller: &AuthenticatedCaller,
        selection: Option<&ToolsetSelection>,
    ) -> anyhow::Result<Vec<Value>> {
        self.cognitive_admission(caller, "list_tools", json!({}), "cognitive_mcp.read")
            .await?;
        if selection.is_some() {
            self.cognitive_admission(
                caller,
                "toolsets",
                json!({"operation": "select"}),
                "cognitive_mcp.read",
            )
            .await?;
        }
        Ok(self
            .external_catalog(caller, selection)
            .await?
            .into_iter()
            .map(normalize_tool_definition)
            .collect())
    }

    async fn call_tool(
        &self,
        caller: &AuthenticatedCaller,
        name: &str,
        arguments: Value,
        selection: Option<&ToolsetSelection>,
    ) -> anyhow::Result<Value> {
        let descriptor = self
            .external_catalog(caller, selection)
            .await?
            .into_iter()
            .find(|tool| tool_name(tool) == name)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "AccessDenied: tool '{name}' is not in the current HOT/toolset projection"
                )
            })?;
        self.dispatch_projected_tool(caller, &descriptor, name, arguments)
            .await
            .map(normalize_tool_result)
    }

    async fn list_resources(&self, caller: &AuthenticatedCaller) -> anyhow::Result<Vec<Value>> {
        Self::authorize(caller, "cognitive_mcp.read")?;
        let dir = self.blob_catalog_dir.clone();
        tokio::task::spawn_blocking(move || list_blob_resources(&dir))
            .await
            .map_err(|error| anyhow::anyhow!("blob catalog task failed: {error}"))?
    }

    async fn read_resource(
        &self,
        caller: &AuthenticatedCaller,
        uri: &str,
    ) -> anyhow::Result<Value> {
        Self::authorize(caller, "cognitive_mcp.read")?;
        let dir = self.blob_catalog_dir.clone();
        let uri = uri.to_string();
        tokio::task::spawn_blocking(move || read_blob_resource(&dir, &uri))
            .await
            .map_err(|error| anyhow::anyhow!("blob resource task failed: {error}"))?
    }
}

#[derive(Clone)]
struct McpFrontendState {
    authenticator: Arc<dyn McpAuthenticator>,
    backend: Arc<dyn McpBackend>,
    allowed_origins: Arc<HashSet<String>>,
}

/// Build the raw HTTP projection. The returned router owns no listener.
pub fn build_mcp_router(engine: Arc<MutationEngine>, validator: Arc<AssertionValidator>) -> Router {
    let policy = McpProjectionPolicy::load_from_env()
        .expect("protected MCP audience/toolset policy must be valid before binding :8090");
    let state = McpFrontendState {
        authenticator: Arc::new(OracleHttpAuthenticator {
            validator,
            engine: engine.clone(),
        }),
        backend: Arc::new(MutationEngineMcpBackend::new(engine, policy)),
        allowed_origins: Arc::new(configured_allowed_origins()),
    };
    router_with_state(state)
}

fn router_with_state(state: McpFrontendState) -> Router {
    Router::new()
        .route(MCP_PATH, post(handle_mcp))
        .with_state(state)
}

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

async fn handle_mcp(State(state): State<McpFrontendState>, request: Request<Body>) -> Response {
    let peer = peer_addr(request.extensions());
    let headers = request.headers().clone();
    if let Err(error) = validate_origin(&headers, &state.allowed_origins) {
        return (
            StatusCode::FORBIDDEN,
            axum::Json(json!({"error": "origin_rejected", "message": error})),
        )
            .into_response();
    }
    let caller = match state.authenticator.authenticate(&headers, peer).await {
        Ok(caller) => caller,
        Err(error) => {
            return (
                StatusCode::UNAUTHORIZED,
                axum::Json(json!({"error": "unauthenticated", "message": error.0})),
            )
                .into_response();
        }
    };

    let bytes = match to_bytes(request.into_body(), MAX_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => {
            return jsonrpc_error(
                StatusCode::PAYLOAD_TOO_LARGE,
                None,
                -32600,
                "body too large",
            )
        }
    };
    let rpc: JsonRpcRequest = match serde_json::from_slice(&bytes) {
        Ok(request) => request,
        Err(error) => {
            return jsonrpc_error(
                StatusCode::BAD_REQUEST,
                None,
                -32700,
                &format!("parse error: {error}"),
            )
        }
    };

    if rpc.jsonrpc != "2.0" {
        return jsonrpc_error(
            StatusCode::BAD_REQUEST,
            rpc.id,
            -32600,
            "jsonrpc must be 2.0",
        );
    }
    if let Err(error) = validate_protocol_headers(&headers, &rpc) {
        return jsonrpc_error(StatusCode::BAD_REQUEST, rpc.id, -32600, &error);
    }
    match one_optional_header(&headers, DECLARED_CAPABILITY_HEADER) {
        Ok(None) => {}
        Ok(Some(_)) => {
            return jsonrpc_error(
                StatusCode::BAD_REQUEST,
                rpc.id,
                -32600,
                "x-opdbus-capability is not accepted on MCP; authority is resolved server-side",
            )
        }
        Err(error) => return jsonrpc_error(StatusCode::BAD_REQUEST, rpc.id, -32600, &error),
    }

    let id = rpc.id.clone();
    let outcome = tokio::time::timeout(
        REQUEST_TIMEOUT,
        dispatch_rpc(state.backend.as_ref(), &caller, &rpc.method, rpc.params),
    )
    .await;

    match outcome {
        Ok(Ok(result)) => match id {
            Some(id) => (
                StatusCode::OK,
                axum::Json(json!({"jsonrpc": "2.0", "id": id, "result": result})),
            )
                .into_response(),
            None => StatusCode::ACCEPTED.into_response(),
        },
        Ok(Err(error)) => jsonrpc_error(StatusCode::OK, id, error.code, &error.message),
        Err(_) => jsonrpc_error(StatusCode::REQUEST_TIMEOUT, id, -32000, "request timed out"),
    }
}

#[derive(Debug)]
struct RpcDispatchError {
    code: i64,
    message: String,
}

impl RpcDispatchError {
    fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("method not found: {method}"),
        }
    }

    fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: message.into(),
        }
    }

    fn internal(error: impl std::fmt::Display) -> Self {
        Self {
            code: -32603,
            message: error.to_string(),
        }
    }
}

async fn dispatch_rpc(
    backend: &dyn McpBackend,
    caller: &AuthenticatedCaller,
    method: &str,
    params: Value,
) -> Result<Value, RpcDispatchError> {
    match method {
        // Streamable HTTP remains stateless, but the standard MCP handshake is
        // still required.  No shim or server-side session is introduced here.
        "initialize" => {
            let requested = params
                .get("protocolVersion")
                .and_then(Value::as_str)
                .ok_or_else(|| RpcDispatchError::invalid_params("missing protocolVersion"))?;
            Ok(initialize_result(negotiate_protocol_version(requested)))
        }
        "notifications/initialized" => Ok(json!({})),
        "ping" => Ok(json!({})),
        "server/discover" => Ok(discovery_result()),
        "tools/list" => {
            let selection = selection_from_params(&params)?;
            let mut tools = backend
                .list_tools(caller, selection.as_ref())
                .await
                .map_err(RpcDispatchError::internal)?;
            tools.sort_by(|left, right| tool_name(left).cmp(tool_name(right)));
            Ok(paginate("tools", tools, &params)?)
        }
        "tools/call" => {
            let name = required_string_param(&params, "name")?;
            let mut arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let selection = merged_toolset_selection(&params, &mut arguments)?;
            backend
                .call_tool(caller, name, arguments, selection.as_ref())
                .await
                .map_err(RpcDispatchError::internal)
        }
        "resources/list" => {
            let mut resources = backend
                .list_resources(caller)
                .await
                .map_err(RpcDispatchError::internal)?;
            resources.sort_by(|left, right| resource_uri(left).cmp(resource_uri(right)));
            Ok(paginate("resources", resources, &params)?)
        }
        "resources/read" => {
            let uri = required_string_param(&params, "uri")?;
            backend
                .read_resource(caller, uri)
                .await
                .map_err(RpcDispatchError::internal)
        }
        other => Err(RpcDispatchError::method_not_found(other)),
    }
}

fn negotiate_protocol_version(requested: &str) -> &'static str {
    SUPPORTED_PROTOCOL_VERSIONS
        .iter()
        .copied()
        .find(|version| *version == requested)
        .unwrap_or(LATEST_OFFICIAL_PROTOCOL_VERSION)
}

fn protocol_version_supported(version: &str) -> bool {
    SUPPORTED_PROTOCOL_VERSIONS.contains(&version)
}

fn initialize_result(protocol_version: &str) -> Value {
    json!({
        "protocolVersion": protocol_version,
        "capabilities": {
            "tools": {"listChanged": false},
            "resources": {"subscribe": false, "listChanged": false}
        },
        "serverInfo": {
            "name": "op-grpc-bridge",
            "version": env!("CARGO_PKG_VERSION")
        },
        "instructions": "Each request requires exactly one authenticated identity: a fresh OIA1 assertion or the exact active SID1 stored in the caller's identity sled."
    })
}

fn discovery_result() -> Value {
    json!({
        "protocolVersions": SUPPORTED_PROTOCOL_VERSIONS,
        "endpoint": MCP_PATH,
        "transport": "streamable-http",
        "sessionMode": "stateless",
        "requiredHeaders": ["MCP-Protocol-Version"],
        "identityHeaderAlternatives": [HTTP_ASSERTION_HEADER, HTTP_SEALED_ID_HEADER],
        "optionalIntegrityHeaders": ["Mcp-Method", "Mcp-Name"],
        "capabilities": initialize_result(MCP_PROTOCOL_VERSION)["capabilities"].clone()
    })
}

fn validate_protocol_headers(headers: &HeaderMap, rpc: &JsonRpcRequest) -> Result<(), String> {
    // Per Streamable HTTP, the initialize request negotiates the protocol and
    // therefore need not carry MCP-Protocol-Version yet. All later requests
    // must carry a negotiated value this frontend can speak.
    match one_optional_header(headers, MCP_VERSION_HEADER)? {
        Some(version) if !protocol_version_supported(&version) => {
            if rpc.method != "initialize" {
                return Err(format!("unsupported MCP protocol version: {version}"));
            }
        }
        None if rpc.method != "initialize" => {
            return Err(format!("missing {MCP_VERSION_HEADER} header"));
        }
        _ => {}
    }
    if one_optional_header(headers, MCP_METHOD_HEADER)?.is_some_and(|method| method != rpc.method) {
        return Err("Mcp-Method header does not match JSON-RPC method".into());
    }

    let expected_name = match rpc.method.as_str() {
        "tools/call" => {
            Some(required_string_param(&rpc.params, "name").map_err(|error| error.message)?)
        }
        "resources/read" => {
            Some(required_string_param(&rpc.params, "uri").map_err(|error| error.message)?)
        }
        _ => None,
    };
    match expected_name {
        Some(expected) => {
            if one_optional_header(headers, MCP_NAME_HEADER)?
                .is_some_and(|actual| actual != expected)
            {
                return Err("Mcp-Name header does not match JSON-RPC target".into());
            }
        }
        None => {
            if one_optional_header(headers, MCP_NAME_HEADER)?.is_some() {
                return Err("Mcp-Name is not valid for this method".into());
            }
        }
    }
    Ok(())
}

fn one_required_header<'a>(headers: &'a HeaderMap, name: &str) -> Result<&'a str, String> {
    one_optional_header(headers, name)?.ok_or_else(|| format!("missing {name} header"))
}

fn one_optional_header<'a>(headers: &'a HeaderMap, name: &str) -> Result<Option<&'a str>, String> {
    let mut values = headers.get_all(name).iter();
    let Some(first) = values.next() else {
        return Ok(None);
    };
    if values.next().is_some() {
        return Err(format!("duplicate {name} header"));
    }
    let value = first
        .to_str()
        .map_err(|_| format!("malformed {name} header"))?
        .trim();
    if value.is_empty() {
        return Err(format!("empty {name} header"));
    }
    Ok(Some(value))
}

/// Exact browser origin allowlist shared by MCP HTTP and gRPC-Web CORS.
/// Invalid entries are ignored, which fails closed for those origins.
pub(crate) fn configured_allowed_origins() -> HashSet<String> {
    std::env::var(ALLOWED_ORIGINS_ENV)
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|origin| valid_origin(origin))
        .map(str::to_string)
        .collect()
}

pub(crate) fn configured_allow_origin() -> tower_http::cors::AllowOrigin {
    let origins = configured_allowed_origins()
        .into_iter()
        .filter_map(|origin| origin.parse::<axum::http::HeaderValue>().ok())
        .collect::<Vec<_>>();
    tower_http::cors::AllowOrigin::list(origins)
}

fn valid_origin(origin: &str) -> bool {
    if origin.is_empty() || origin == "*" || origin.eq_ignore_ascii_case("null") {
        return false;
    }
    let Ok(uri) = origin.parse::<axum::http::Uri>() else {
        return false;
    };
    matches!(uri.scheme_str(), Some("https") | Some("http"))
        && uri.authority().is_some()
        && (uri.path().is_empty() || uri.path() == "/")
        && uri.query().is_none()
}

fn validate_origin(headers: &HeaderMap, allowed_origins: &HashSet<String>) -> Result<(), String> {
    let origin = one_optional_header(headers, axum::http::header::ORIGIN.as_str())?;
    if let Some(origin) = origin {
        if !valid_origin(origin) || !allowed_origins.contains(origin) {
            return Err("Origin is not on the exact MCP allowlist".into());
        }
        return Ok(());
    }

    let browser_marked = ["sec-fetch-mode", "sec-fetch-site", "sec-fetch-dest"]
        .iter()
        .any(|name| headers.contains_key(*name));
    if browser_marked {
        return Err("browser MCP requests require an Origin header".into());
    }
    Ok(())
}

fn required_string_param<'a>(params: &'a Value, name: &str) -> Result<&'a str, RpcDispatchError> {
    params
        .get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| RpcDispatchError::invalid_params(format!("missing {name}")))
}

fn selection_from_params(params: &Value) -> Result<Option<ToolsetSelection>, RpcDispatchError> {
    selection_from_meta(params.get("_meta"))
}

fn merged_toolset_selection(
    params: &Value,
    arguments: &mut Value,
) -> Result<Option<ToolsetSelection>, RpcDispatchError> {
    let from_params = selection_from_params(params)?;
    let from_arguments = take_argument_toolset_selection(arguments)?;
    match (from_params, from_arguments) {
        (Some(left), Some(right)) if left != right => Err(RpcDispatchError::invalid_params(
            "conflicting _meta.opdbus_toolset selectors",
        )),
        (Some(selection), _) | (_, Some(selection)) => Ok(Some(selection)),
        (None, None) => Ok(None),
    }
}

fn selection_from_meta(meta: Option<&Value>) -> Result<Option<ToolsetSelection>, RpcDispatchError> {
    let Some(raw) = meta.and_then(|value| value.get("opdbus_toolset")) else {
        return Ok(None);
    };
    let selection: ToolsetSelection = serde_json::from_value(raw.clone()).map_err(|error| {
        RpcDispatchError::invalid_params(format!("invalid toolset selector: {error}"))
    })?;
    if selection.id.is_empty() || selection.generation == 0 {
        return Err(RpcDispatchError::invalid_params(
            "toolset selector requires a non-empty id and positive generation",
        ));
    }
    Ok(Some(selection))
}

fn take_argument_toolset_selection(
    arguments: &mut Value,
) -> Result<Option<ToolsetSelection>, RpcDispatchError> {
    let Some(object) = arguments.as_object_mut() else {
        return Ok(None);
    };
    let mut remove_meta = false;
    let raw = if let Some(meta) = object.get_mut("_meta").and_then(Value::as_object_mut) {
        let value = meta.remove("opdbus_toolset");
        remove_meta = meta.is_empty();
        value
    } else {
        None
    };
    if remove_meta {
        object.remove("_meta");
    }
    match raw {
        Some(value) => selection_from_meta(Some(&json!({"opdbus_toolset": value}))),
        None => Ok(None),
    }
}

fn paginate(key: &str, values: Vec<Value>, params: &Value) -> Result<Value, RpcDispatchError> {
    let offset = match params.get("cursor").and_then(Value::as_str) {
        Some(cursor) => cursor
            .parse::<usize>()
            .map_err(|_| RpcDispatchError::invalid_params("invalid cursor"))?,
        None => 0,
    };
    let limit = params
        .get("limit")
        .and_then(Value::as_u64)
        .map(|value| value as usize)
        .unwrap_or(DEFAULT_PAGE_SIZE)
        .clamp(1, MAX_PAGE_SIZE);
    if offset > values.len() {
        return Err(RpcDispatchError::invalid_params(
            "cursor is past the catalog",
        ));
    }
    let end = offset.saturating_add(limit).min(values.len());
    let mut result = serde_json::Map::new();
    result.insert(key.to_string(), Value::Array(values[offset..end].to_vec()));
    if end < values.len() {
        result.insert("nextCursor".into(), Value::String(end.to_string()));
    }
    Ok(Value::Object(result))
}

fn parse_typed_tool_name(name: &str) -> anyhow::Result<(&str, &str)> {
    let mut parts = name.split('.');
    if parts.next() != Some("plugin") {
        anyhow::bail!("AccessDenied: '{name}' is not a canonical typed plugin tool");
    }
    let plugin_id = parts
        .next()
        .filter(|part| !part.is_empty())
        .ok_or_else(|| anyhow::anyhow!("AccessDenied: '{name}' has no plugin id"))?;
    let method_name = parts
        .next()
        .filter(|part| !part.is_empty())
        .ok_or_else(|| anyhow::anyhow!("AccessDenied: '{name}' has no method"))?;
    if parts.next().is_some()
        || !plugin_id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        || !method_name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        anyhow::bail!("AccessDenied: '{name}' is not canonical");
    }
    Ok((plugin_id, method_name))
}

fn method_descriptor(
    schema: &op_state_store::PluginSchema,
    method_name: &str,
    public_name: &str,
) -> anyhow::Result<Value> {
    let method = schema.methods.get(method_name).ok_or_else(|| {
        anyhow::anyhow!(
            "AccessDenied: sealed plugin '{}' has no method '{}'",
            schema.name,
            method_name
        )
    })?;
    let required_capability = method.required_capability.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
            "AccessDenied: {}.{} has no sealed capability",
            schema.name,
            method_name
        )
    })?;
    if !schema.capabilities.contains_key(required_capability) {
        anyhow::bail!(
            "AccessDenied: {}.{} capability '{}' is outside schema closure",
            schema.name,
            method_name,
            required_capability
        );
    }
    let mut descriptor = json!({
        "name": public_name,
        "description": format!("{} — {}", schema.description, method.name),
        "inputSchema": serde_json::to_value(&method.args)?,
        "required_capability": required_capability,
        "subid": method.subid,
        "authority_method": method.name
    });
    if let Some(output) = &method.returns {
        descriptor["outputSchema"] = serde_json::to_value(output)?;
    }
    Ok(descriptor)
}

fn normalize_tool_definition(mut tool: Value) -> Value {
    let Some(object) = tool.as_object_mut() else {
        return tool;
    };
    if let Some(schema) = object.remove("input_schema") {
        object.insert("inputSchema".into(), schema);
    }
    for internal in [
        "category",
        "namespace",
        "schema_version",
        "tags",
        "required_capability",
        "subid",
        "authority_method",
    ] {
        object.remove(internal);
    }
    tool
}

fn normalize_tool_result(result: Value) -> Value {
    if result.as_object().is_some_and(|object| {
        object.contains_key("content") || object.contains_key("structuredContent")
    }) {
        return result;
    }
    let text = serde_json::to_string(&result).unwrap_or_else(|_| "null".into());
    json!({
        "content": [{"type": "text", "text": text}],
        "structuredContent": result,
        "isError": false
    })
}

fn tool_name(value: &Value) -> &str {
    value.get("name").and_then(Value::as_str).unwrap_or("")
}

fn authorize_with_grants(
    grants: &HashSet<String>,
    required_capability: &str,
) -> anyhow::Result<()> {
    if grants.contains(required_capability) {
        Ok(())
    } else {
        anyhow::bail!("AccessDenied: principal lacks required capability {required_capability}")
    }
}

fn descriptor_authority<'a>(
    descriptor: &'a Value,
    expected_name: Option<&str>,
) -> anyhow::Result<(&'a str, &'a str)> {
    let name = descriptor
        .get("name")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("AccessDenied: tool descriptor has no name"))?;
    if expected_name.is_some_and(|expected| expected != name) {
        anyhow::bail!("AccessDenied: resolved tool descriptor name mismatch");
    }
    let capability = descriptor
        .get("required_capability")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "AccessDenied: target tool '{name}' has no required_capability descriptor"
            )
        })?;
    let subid = descriptor
        .get("subid")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            anyhow::anyhow!("AccessDenied: target tool '{name}' has no subid descriptor")
        })?;
    Ok((capability, subid))
}

fn filter_authorized_tools(tools: Vec<Value>, grants: &HashSet<String>) -> Vec<Value> {
    tools
        .into_iter()
        .filter(|tool| {
            descriptor_authority(tool, None).is_ok_and(|(capability, _)| {
                capability != "cognitive_mcp.invoke" && grants.contains(capability)
            })
        })
        .collect()
}

fn validate_tool_arguments(descriptor: &Value, arguments: &Value) -> anyhow::Result<()> {
    let name = descriptor
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("<unknown>");
    let schema = descriptor
        .get("input_schema")
        .or_else(|| descriptor.get("inputSchema"))
        .ok_or_else(|| anyhow::anyhow!("AccessDenied: target tool '{name}' has no input schema"))?;
    let validator = jsonschema::Validator::new(schema)
        .map_err(|error| anyhow::anyhow!("invalid input schema for tool '{name}': {error}"))?;
    validator
        .validate(arguments)
        .map_err(|error| anyhow::anyhow!("invalid arguments for tool '{name}': {error}"))
}

fn authorize_and_validate_tool_call(
    descriptor: &Value,
    expected_name: &str,
    arguments: &Value,
    grants: &HashSet<String>,
) -> anyhow::Result<()> {
    let (required_capability, _) = descriptor_authority(descriptor, Some(expected_name))?;
    if required_capability == "cognitive_mcp.invoke" {
        anyhow::bail!(
            "AccessDenied: target tool '{expected_name}' has no independent sealed capability"
        );
    }
    authorize_with_grants(grants, required_capability)?;
    validate_tool_arguments(descriptor, arguments)
}

fn resource_uri(value: &Value) -> &str {
    value.get("uri").and_then(Value::as_str).unwrap_or("")
}

fn load_exact_capability_grants(principal_id: &str) -> HashSet<String> {
    load_capability_grants(principal_id)
}

fn list_blob_resources(dir: &Path) -> anyhow::Result<Vec<Value>> {
    let entries = manifest_plugins(dir)?;
    Ok(entries
        .keys()
        .filter(|plugin_id| !op_plugins::default_registry::is_retired_plugin(plugin_id))
        .map(|plugin_id| {
            json!({
                "uri": format!("blob://{plugin_id}"),
                "name": format!("{plugin_id} schema"),
                "description": format!("Sanitized sealed PluginSchema for {plugin_id}"),
                "mimeType": "application/json"
            })
        })
        .collect())
}

fn read_blob_resource(dir: &Path, uri: &str) -> anyhow::Result<Value> {
    let plugin_id = uri
        .strip_prefix("blob://")
        .filter(|value| valid_plugin_id(value))
        .ok_or_else(|| anyhow::anyhow!("unsupported resource URI: {uri}"))?;
    if op_plugins::default_registry::is_retired_plugin(plugin_id) {
        anyhow::bail!("resource not found: {uri}");
    }
    let entries = manifest_plugins(dir)?;
    let schema_hash = entries
        .get(plugin_id)
        .ok_or_else(|| anyhow::anyhow!("resource not found: {uri}"))?;
    if schema_hash.len() < 16 {
        anyhow::bail!("catalog hash for {plugin_id} is malformed");
    }
    let path = dir.join(format!("{plugin_id}.{}.blob", &schema_hash[..16]));
    let bytes = std::fs::read(&path)?;
    let blob = op_blob::BlobRef::new(&bytes)
        .map_err(|error| anyhow::anyhow!("invalid sealed blob {}: {error}", path.display()))?;
    if blob.schema_hash_hex() != *schema_hash {
        anyhow::bail!("manifest/blob schema hash mismatch for {plugin_id}");
    }
    let mut schema: Value = serde_json::from_str(blob.schema_json())?;
    sanitize_schema(&mut schema);
    let text = serde_json::to_string(&schema)?;
    Ok(json!({
        "contents": [{"uri": uri, "mimeType": "application/json", "text": text}]
    }))
}

fn manifest_plugins(dir: &Path) -> anyhow::Result<BTreeMap<String, String>> {
    let bytes = std::fs::read(dir.join(op_blob::catalog::MANIFEST_FILENAME))?;
    let value: Value = serde_json::from_slice(&bytes)?;
    serde_json::from_value(
        value
            .get("plugins")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("blob catalog manifest has no plugins map"))?,
    )
    .map_err(Into::into)
}

fn valid_plugin_id(plugin_id: &str) -> bool {
    !plugin_id.is_empty()
        && plugin_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
}

fn sanitize_schema(value: &mut Value) {
    match value {
        Value::Object(object) => {
            object.retain(|key, _| !private_projection_key(key));
            for nested in object.values_mut() {
                sanitize_schema(nested);
            }
        }
        Value::Array(values) => {
            for nested in values {
                sanitize_schema(nested);
            }
        }
        _ => {}
    }
}

fn private_projection_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase().replace('-', "_");
    matches!(
        normalized.as_str(),
        "capability_grants"
            | "principal_membership"
            | "principal_memberships"
            | "authorized_principals"
            | "member_ids"
            | "internal_path"
            | "socket_path"
            | "db_path"
            | "key_path"
            | "cert_path"
            | "key_id"
            | "decoy_key_id"
            | "approval_verifier"
            | "approval_verifier_config"
    ) || normalized.ends_with("_secret")
        || normalized.ends_with("_token")
        || normalized.ends_with("_password")
        || normalized.ends_with("_private_key")
        || normalized.ends_with("_api_key")
        || normalized.ends_with("_key_id")
}

fn encoded_len_upper_bound(decoded_len: usize) -> usize {
    decoded_len.saturating_add(2) / 3 * 4
}

fn decode_http_assertion(encoded: &str) -> Result<Vec<u8>, McpAuthError> {
    let wire = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded.as_bytes())
        .map_err(|_| McpAuthError("Malformed Oracle identity assertion".into()))?;
    if wire.len() > MAX_ASSERTION_BYTES
        || base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&wire) != encoded
    {
        return Err(McpAuthError(
            "Oracle identity assertion is not canonical base64url".into(),
        ));
    }
    Ok(wire)
}

fn peer_addr(extensions: &axum::http::Extensions) -> Option<SocketAddr> {
    if let Some(info) = extensions.get::<TcpConnectInfo>() {
        if let Some(addr) = info.remote_addr() {
            return Some(addr);
        }
    }
    extensions
        .get::<TlsConnectInfo<TcpConnectInfo>>()
        .and_then(|info| info.get_ref().remote_addr())
}

fn jsonrpc_error(status: StatusCode, id: Option<Value>, code: i64, message: &str) -> Response {
    (
        status,
        axum::Json(json!({
            "jsonrpc": "2.0",
            "id": id.unwrap_or(Value::Null),
            "error": {"code": code, "message": message}
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::header::CONTENT_TYPE;
    use tower::ServiceExt;

    #[derive(Clone)]
    struct TestAuthenticator;

    #[async_trait]
    impl McpAuthenticator for TestAuthenticator {
        async fn authenticate(
            &self,
            headers: &HeaderMap,
            _peer: Option<SocketAddr>,
        ) -> Result<AuthenticatedCaller, McpAuthError> {
            if headers
                .get("x-test-auth")
                .and_then(|value| value.to_str().ok())
                != Some("ok")
            {
                return Err(McpAuthError("missing test assertion".into()));
            }
            Ok(AuthenticatedCaller {
                principal_id: "test-principal".into(),
                session_id: "test-session".into(),
                session_genesis: "test-genesis".into(),
            })
        }
    }

    #[derive(Clone)]
    struct TestBackend;

    #[async_trait]
    impl McpBackend for TestBackend {
        async fn list_tools(
            &self,
            _caller: &AuthenticatedCaller,
            _selection: Option<&ToolsetSelection>,
        ) -> anyhow::Result<Vec<Value>> {
            Ok(vec![json!({
                "name": "echo",
                "description": "Echo input",
                "inputSchema": {"type": "object"}
            })])
        }

        async fn call_tool(
            &self,
            _caller: &AuthenticatedCaller,
            name: &str,
            arguments: Value,
            _selection: Option<&ToolsetSelection>,
        ) -> anyhow::Result<Value> {
            Ok(json!({
                "content": [{"type": "text", "text": arguments.to_string()}],
                "structuredContent": {"tool": name, "arguments": arguments},
                "isError": false
            }))
        }

        async fn list_resources(
            &self,
            _caller: &AuthenticatedCaller,
        ) -> anyhow::Result<Vec<Value>> {
            Ok(vec![json!({
                "uri": "blob://cognitive_mcp",
                "name": "cognitive_mcp schema",
                "mimeType": "application/json"
            })])
        }

        async fn read_resource(
            &self,
            _caller: &AuthenticatedCaller,
            uri: &str,
        ) -> anyhow::Result<Value> {
            Ok(json!({
                "contents": [{"uri": uri, "mimeType": "application/json", "text": "{}"}]
            }))
        }
    }

    fn test_router() -> Router {
        router_with_state(McpFrontendState {
            authenticator: Arc::new(TestAuthenticator),
            backend: Arc::new(TestBackend),
            allowed_origins: Arc::new(HashSet::from(["https://dashboard.example".into()])),
        })
    }

    async fn call(method: &str, params: Value, name: Option<&str>) -> Response {
        let mut builder = Request::builder()
            .method("POST")
            .uri(MCP_PATH)
            .header(CONTENT_TYPE, "application/json")
            .header("x-test-auth", "ok")
            .header(MCP_VERSION_HEADER, MCP_PROTOCOL_VERSION)
            .header(MCP_METHOD_HEADER, method);
        if let Some(name) = name {
            builder = builder.header(MCP_NAME_HEADER, name);
        }
        test_router()
            .oneshot(
                builder
                    .body(Body::from(
                        json!({"jsonrpc": "2.0", "id": 1, "method": method, "params": params})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    async fn response_json(response: Response) -> Value {
        let bytes = to_bytes(response.into_body(), MAX_BODY_BYTES)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn native_initialize_succeeds_without_a_shim() {
        let response = call(
            "initialize",
            json!({"protocolVersion": MCP_PROTOCOL_VERSION}),
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["result"]["protocolVersion"], MCP_PROTOCOL_VERSION);
        assert_eq!(body["result"]["serverInfo"]["name"], "op-grpc-bridge");
    }

    #[tokio::test]
    async fn native_initialized_notification_is_accepted_without_a_shim() {
        let response = call("notifications/initialized", json!({}), None).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["result"], json!({}));
    }

    #[tokio::test]
    async fn initialize_negotiates_before_protocol_header_exists() {
        let response = test_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(MCP_PATH)
                    .header(CONTENT_TYPE, "application/json")
                    .header("x-test-auth", "ok")
                    .body(Body::from(
                        json!({
                            "jsonrpc": "2.0",
                            "id": 1,
                            "method": "initialize",
                            "params": {"protocolVersion": MCP_PROTOCOL_VERSION}
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response_json(response).await["result"]["protocolVersion"],
            MCP_PROTOCOL_VERSION
        );
    }

    #[tokio::test]
    async fn initialize_echoes_codex_official_protocol_version() {
        let response = test_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(MCP_PATH)
                    .header(CONTENT_TYPE, "application/json")
                    .header("x-test-auth", "ok")
                    .body(Body::from(
                        json!({
                            "jsonrpc": "2.0",
                            "id": 1,
                            "method": "initialize",
                            "params": {"protocolVersion": "2025-06-18"}
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert!(
            body.get("error").is_none(),
            "Codex handshake must not fail: {body}"
        );
        assert_eq!(body["result"]["protocolVersion"], "2025-06-18");
    }

    #[tokio::test]
    async fn subsequent_request_accepts_codex_protocol_header() {
        let response = test_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(MCP_PATH)
                    .header(CONTENT_TYPE, "application/json")
                    .header("x-test-auth", "ok")
                    .header(MCP_VERSION_HEADER, "2025-06-18")
                    .header(MCP_METHOD_HEADER, "tools/list")
                    .body(Body::from(
                        json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert!(
            body.get("error").is_none(),
            "post-initialize Codex requests must be accepted: {body}"
        );
        assert_eq!(body["result"]["tools"][0]["name"], "echo");
    }

    #[tokio::test]
    async fn unknown_protocol_version_down_negotiates_instead_of_erroring() {
        let response = test_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(MCP_PATH)
                    .header(CONTENT_TYPE, "application/json")
                    .header("x-test-auth", "ok")
                    .body(Body::from(
                        json!({
                            "jsonrpc": "2.0",
                            "id": 1,
                            "method": "initialize",
                            "params": {"protocolVersion": "2025-11-25"}
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert!(
            body.get("error").is_none(),
            "MCP lifecycle requires down-negotiation, not -32602: {body}"
        );
        assert_eq!(body["result"]["protocolVersion"], "2025-06-18");
    }

    fn principal_record(
        principal_id: &str,
        pubkey: &str,
        revoked_at: i64,
    ) -> op_cozo_store::HumanPrincipalRecord {
        op_cozo_store::HumanPrincipalRecord {
            principal_id: principal_id.into(),
            human_pubkey: pubkey.into(),
            display_alias: String::new(),
            registered_at: 1,
            revoked_at,
        }
    }

    #[test]
    fn sealed_id_requires_an_exact_active_registry_binding() {
        assert!(validate_registered_sealed_id_principal(None, "p", "k").is_err());
        assert!(validate_registered_sealed_id_principal(
            Some(&principal_record("p", "k", 2)),
            "p",
            "k"
        )
        .is_err());
        assert!(validate_registered_sealed_id_principal(
            Some(&principal_record("other", "k", 0)),
            "p",
            "k"
        )
        .is_err());
        assert!(validate_registered_sealed_id_principal(
            Some(&principal_record("p", "other", 0)),
            "p",
            "k"
        )
        .is_err());
        assert!(validate_registered_sealed_id_principal(
            Some(&principal_record("p", "k", 0)),
            "p",
            "k"
        )
        .is_ok());
    }

    #[tokio::test]
    async fn tools_list_uses_shared_catalog_backend() {
        let body = response_json(call("tools/list", json!({}), None).await).await;
        assert_eq!(body["result"]["tools"][0]["name"], "echo");
    }

    #[tokio::test]
    async fn resources_list_survives_as_blob_catalog() {
        let body = response_json(call("resources/list", json!({}), None).await).await;
        assert_eq!(
            body["result"]["resources"][0]["uri"],
            "blob://cognitive_mcp"
        );
    }

    #[tokio::test]
    async fn resources_read_returns_content() {
        let uri = "blob://cognitive_mcp";
        let body =
            response_json(call("resources/read", json!({"uri": uri}), Some(uri)).await).await;
        assert_eq!(body["result"]["contents"][0]["uri"], uri);
    }

    #[tokio::test]
    async fn tools_call_dispatches_named_tool() {
        let body = response_json(
            call(
                "tools/call",
                json!({"name": "echo", "arguments": {"value": 7}}),
                Some("echo"),
            )
            .await,
        )
        .await;
        assert_eq!(body["result"]["structuredContent"]["tool"], "echo");
        assert_eq!(body["result"]["structuredContent"]["arguments"]["value"], 7);
    }

    #[test]
    fn argument_toolset_selector_is_stripped_before_schema_validation() {
        let mut arguments = json!({
            "query": "identity",
            "_meta": {
                "opdbus_toolset": {"id": "context_code", "generation": 3}
            }
        });
        let selection = merged_toolset_selection(&json!({}), &mut arguments)
            .unwrap()
            .unwrap();
        assert_eq!(selection.id, "context_code");
        assert_eq!(selection.generation, 3);
        assert_eq!(arguments, json!({"query": "identity"}));
    }

    #[tokio::test]
    async fn unauthenticated_request_is_rejected_before_dispatch() {
        let response = test_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(MCP_PATH)
                    .header(MCP_VERSION_HEADER, MCP_PROTOCOL_VERSION)
                    .header(MCP_METHOD_HEADER, "tools/list")
                    .body(Body::from(
                        json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn browser_origin_is_exact_allowlist_only() {
        let allowed_origin = Request::builder()
            .method("POST")
            .uri(MCP_PATH)
            .header("origin", "https://dashboard.example")
            .header("sec-fetch-mode", "cors")
            .header("x-test-auth", "ok")
            .header(MCP_VERSION_HEADER, MCP_PROTOCOL_VERSION)
            .header(MCP_METHOD_HEADER, "tools/list")
            .body(Body::from(
                json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}).to_string(),
            ))
            .unwrap();
        let response = test_router().oneshot(allowed_origin).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let wrong_origin = Request::builder()
            .method("POST")
            .uri(MCP_PATH)
            .header("origin", "https://evil.example")
            .header("x-test-auth", "ok")
            .body(Body::empty())
            .unwrap();
        let response = test_router().oneshot(wrong_origin).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        let missing_browser_origin = Request::builder()
            .method("POST")
            .uri(MCP_PATH)
            .header("sec-fetch-mode", "cors")
            .header("x-test-auth", "ok")
            .body(Body::empty())
            .unwrap();
        let response = test_router().oneshot(missing_browser_origin).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn caller_supplied_capability_header_is_rejected() {
        let response = test_router()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(MCP_PATH)
                    .header("x-test-auth", "ok")
                    .header(MCP_VERSION_HEADER, MCP_PROTOCOL_VERSION)
                    .header(MCP_METHOD_HEADER, "tools/list")
                    .header(DECLARED_CAPABILITY_HEADER, "cognitive_mcp.invoke")
                    .body(Body::from(
                        json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn target_tool_without_authority_metadata_is_fail_closed() {
        let grants = HashSet::from([
            "cognitive_mcp.invoke".to_string(),
            "cognitive_mcp.read".to_string(),
        ]);
        let missing_capability = json!({
            "name": "safe_read",
            "subid": "obs.service.cognitive-mcp.test.read@v1",
            "input_schema": {"type": "object"}
        });
        assert!(authorize_and_validate_tool_call(
            &missing_capability,
            "safe_read",
            &json!({}),
            &grants
        )
        .is_err());

        let missing_subid = json!({
            "name": "safe_read",
            "required_capability": "cognitive_mcp.read",
            "input_schema": {"type": "object"}
        });
        assert!(
            authorize_and_validate_tool_call(&missing_subid, "safe_read", &json!({}), &grants)
                .is_err()
        );
    }

    #[test]
    fn nested_tool_arguments_are_validated_before_dispatch() {
        let descriptor = json!({
            "name": "safe_read",
            "required_capability": "cognitive_mcp.read",
            "subid": "obs.service.cognitive-mcp.test.read@v1",
            "input_schema": {
                "type": "object",
                "required": ["filter"],
                "properties": {
                    "filter": {
                        "type": "object",
                        "required": ["limit"],
                        "properties": {"limit": {"type": "integer", "minimum": 1}}
                    }
                }
            }
        });
        let grants = HashSet::from([
            "cognitive_mcp.invoke".to_string(),
            "cognitive_mcp.read".to_string(),
        ]);
        assert!(authorize_and_validate_tool_call(
            &descriptor,
            "safe_read",
            &json!({"filter": {"limit": 2}}),
            &grants
        )
        .is_ok());
        assert!(authorize_and_validate_tool_call(
            &descriptor,
            "safe_read",
            &json!({"filter": {"limit": 0}}),
            &grants
        )
        .is_err());
        assert!(authorize_and_validate_tool_call(
            &descriptor,
            "safe_read",
            &json!({"filter": {}}),
            &grants
        )
        .is_err());
    }

    #[test]
    fn target_capability_is_required_in_addition_to_outer_invoke() {
        let descriptor = json!({
            "name": "safe_read",
            "required_capability": "cognitive_mcp.read",
            "subid": "obs.service.cognitive-mcp.test.read@v1",
            "input_schema": {"type": "object"}
        });
        let outer_only = HashSet::from(["cognitive_mcp.invoke".to_string()]);
        let error =
            authorize_and_validate_tool_call(&descriptor, "safe_read", &json!({}), &outer_only)
                .unwrap_err();
        assert!(error.to_string().contains("cognitive_mcp.read"));
    }

    #[test]
    fn tool_list_is_filtered_by_exact_target_capability() {
        let tools = vec![
            json!({
                "name": "safe_read",
                "required_capability": "cognitive_mcp.read",
                "subid": "obs.service.cognitive-mcp.test.read@v1"
            }),
            json!({
                "name": "mutate",
                "required_capability": "cognitive_mcp.invoke",
                "subid": "mut.service.cognitive-mcp.test.write@v1"
            }),
            json!({"name": "missing_descriptor"}),
        ];
        let grants = HashSet::from(["cognitive_mcp.read".to_string()]);
        let filtered = filter_authorized_tools(tools, &grants);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0]["name"], "safe_read");
    }

    #[test]
    fn broad_outer_invoke_is_not_accepted_as_target_authority() {
        let descriptor = json!({
            "name": "code_index",
            "required_capability": "cognitive_mcp.invoke",
            "subid": "mut.service.code-rag.index@v1",
            "input_schema": {"type": "object"}
        });
        let grants = HashSet::from(["cognitive_mcp.invoke".to_string()]);
        let error =
            authorize_and_validate_tool_call(&descriptor, "code_index", &json!({}), &grants)
                .unwrap_err();
        assert!(error
            .to_string()
            .contains("no independent sealed capability"));
    }

    #[test]
    fn public_tool_projection_strips_authority_metadata() {
        let projected = normalize_tool_definition(json!({
            "name": "code_search",
            "description": "search",
            "input_schema": {"type": "object"},
            "required_capability": "cognitive_mcp.read",
            "subid": "obs.service.code-rag.search@v1",
            "authority_method": "code_search"
        }));
        assert!(projected.get("inputSchema").is_some());
        for internal in ["required_capability", "subid", "authority_method"] {
            assert!(projected.get(internal).is_none());
        }
    }

    #[test]
    fn safe_read_tool_with_both_grants_passes_policy_and_schema() {
        let descriptor = json!({
            "name": "safe_read",
            "required_capability": "cognitive_mcp.read",
            "subid": "obs.service.cognitive-mcp.test.read@v1",
            "input_schema": {
                "type": "object",
                "required": ["key"],
                "properties": {"key": {"type": "string"}},
                "additionalProperties": false
            }
        });
        let grants = HashSet::from([
            "cognitive_mcp.invoke".to_string(),
            "cognitive_mcp.read".to_string(),
        ]);
        authorize_with_grants(&grants, "cognitive_mcp.invoke").unwrap();
        authorize_and_validate_tool_call(
            &descriptor,
            "safe_read",
            &json!({"key": "status"}),
            &grants,
        )
        .unwrap();
    }

    #[test]
    fn http_assertion_requires_canonical_unpadded_base64url() {
        // Eight bytes force standard base64 to append one `=` padding byte.
        let wire = b"OIA1wire";
        let canonical = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(wire);
        assert_eq!(decode_http_assertion(&canonical).unwrap(), wire);

        let padded = base64::engine::general_purpose::URL_SAFE.encode(wire);
        assert!(decode_http_assertion(&padded).is_err());
        assert!(decode_http_assertion("%%%not-base64url%%%").is_err());
    }

    #[test]
    fn wildcard_grant_never_authorizes_mcp() {
        let document = json!({
            "*": {"capabilities": ["cognitive_mcp.read", "cognitive_mcp.invoke"]},
            "known": {"capabilities": ["cognitive_mcp.read"]}
        });
        assert!(capabilities_for_principal(&document, "unknown").is_empty());
        assert!(capabilities_for_principal(&document, "known").is_empty());
    }

    #[test]
    fn legacy_footprint_key_invalidates_entire_grant_document() {
        let document = json!({
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef": {
                "capabilities": ["cognitive_mcp.invoke"]
            },
            "937d6d2b-ecae-ed53-f3a2-d7bd09f544ff": {
                "capabilities": ["cognitive_mcp.read"]
            }
        });
        assert!(
            capabilities_for_principal(&document, "937d6d2b-ecae-ed53-f3a2-d7bd09f544ff")
                .is_empty()
        );
    }

    #[test]
    fn schema_sanitizer_removes_embedded_identity_grants() {
        let mut schema = json!({
            "capability_grants": {"*": ["mcp.write"]},
            "principal_membership": ["did:op:human:secret"],
            "internal": {
                "socket_path": "/run/private.sock",
                "decoy_key_id": "issuer-1",
                "oauth_token": "secret",
                "approval_verifier_config": {"key": "value"}
            },
            "methods": {"read": {
                "required_capability": "mcp.read",
                "args": {"properties": {"path": {"type": "string"}}}
            }}
        });
        sanitize_schema(&mut schema);
        assert!(schema.get("capability_grants").is_none());
        assert!(schema.get("principal_membership").is_none());
        assert!(schema["internal"].get("socket_path").is_none());
        assert!(schema["internal"].get("decoy_key_id").is_none());
        assert!(schema["internal"].get("oauth_token").is_none());
        assert!(schema["internal"].get("approval_verifier_config").is_none());
        assert_eq!(schema["methods"]["read"]["required_capability"], "mcp.read");
        assert_eq!(
            schema["methods"]["read"]["args"]["properties"]["path"]["type"],
            "string"
        );
    }
}
