# Standalone EMQX Identity MCP — Design

## Overview

This document provides the detailed technical design for integrating EMQX as a standalone local service into the OP-DBUS identity pipeline. The design follows the PluginSchema contract pattern established by the grpc-expert skill and maintains the existing identity/capability model. The same bridge-owned endpoint also applies two independent projections: caller audience (singleton chatbot versus external agent) and execution temperature (HOT versus selected WARM/COLD tool set).

**Unified Fabric Surface:** `10.0.0.3:8090` is the bridge-owned unified fabric
surface itself—not merely one endpoint on a larger fabric. It carries the canonical
MCP path `https://10.0.0.3:8090/mcp`, native gRPC/gRPC-Web, generated plugin methods,
and mutation/control capabilities through one authority. EMQX remains internal on
loopback MQTT/ExHook behind that surface and never exposes a second external endpoint.

---

## 1. Plugin State Redesign

### 1.1 Current State (To Be Removed)

The existing `EmqxState` contains Netmaker-specific fields that must be removed:

```rust
// REMOVE these Netmaker-specific constants
pub const CONTAINER_NAME: &str = "NetMaker";
pub const BROKER_SOCKET: &str = "/run/ghostbridge/NetMaker/broker.sock";
pub const API_SOCKET: &str = "/run/ghostbridge/NetMaker/api.sock";
pub const HOST_BROKER_ALIAS: &str = "/run/ghostbridge/netmaker-broker.sock";
pub const EXHOOK_TARGET: &str = "unix:/run/ghostbridge/container.sock";
pub const MQTT_WS_URL: &str = "ws://100.69.0.1:8090/mqtt";

// REMOVE these Netmaker-specific fields from EmqxState
pub container_name: String,      // Was "NetMaker"
pub nic: bool,                   // "Containers have no NIC"
pub broker_socket: String,       // Netmaker path
pub api_socket: String,          // Netmaker REST API
```

### 1.2 New Standalone State Design

```rust
//! EMQX present-state plugin — GB.Emqx.
//!
//! Present-state for standalone local EMQX broker: service status, listeners,
//! hooks, and unified-fabric attachment. ExHook RPCs stay on HookProvider
//! (`emqx.exhook.v3`).
//! EMQX is NOT a Netmaker dependency.

use super::plugin_scaffold_helpers::{method_decl_from_schemars_with_output, AckOutput};
use anyhow::Result;
use op_state_store::{CapabilityDecl, MethodDecl, PluginSchema, SideEffect};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const PLUGIN_NAME: &str = "emqx";
const PLUGIN_VERSION: &str = "2.0.0";  // Major bump for breaking change
const PLUGIN_CATEGORY: &str = "net";
const PLUGIN_DESCRIPTION: &str =
    "Standalone EMQX broker: status, listeners, hooks, fabric attachment, lifecycle management";
const PLUGIN_DISPLAY_NAME: &str = "GB.Emqx";

// Standalone EMQX paths (loopback/UDS only — no public bindings)
pub const EMQX_DATA_DIR: &str = "/var/lib/emqx";
pub const EMQX_CONFIG_DIR: &str = "/etc/emqx";
pub const EMQX_LOG_DIR: &str = "/var/log/emqx";
pub const EMQX_DASHBOARD_BIND: &str = "127.0.0.1:18083";
pub const EMQX_MQTT_TCP_BIND: &str = "127.0.0.1:1883";
pub const EMQX_API_BIND: &str = "127.0.0.1:18083";

// ExHook configuration (dedicated local listener)
pub const EXHOOK_PROTO: &str = "emqx.exhook.v3";
pub const EXHOOK_SERVICE: &str = "emqx.exhook.v3.HookProvider";
pub const EXHOOK_UDS: &str = "/run/opdbus/emqx-exhook.sock";  // Dedicated UDS
```

These declarations are schema constants, not probes. `emqx_schema()` must be pure and
deterministic: it never calls `current_state()`, checks a socket, invokes `sv`, or
contacts the EMQX API while building defaults/examples. Live state is obtained only
inside bounded dispatch functions. Credentials are held by the runtime secret/config
boundary and are never fields in `EmqxState` or generated examples.

---

## 2. Method Definitions

### 2.1 Output Type Policy

Per FR-2.5, `AckOutput` (from `plugin_scaffold_helpers.rs:369`) MAY be used for acknowledgment-only methods. However, lifecycle methods MUST return richer results:

```rust
// AckOutput is acceptable for simple confirmations
use super::plugin_scaffold_helpers::AckOutput;

// Lifecycle methods require richer output
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StartOutput {
    pub started: bool,
    pub pid: Option<u32>,
    pub uptime_seconds: Option<u64>,
    pub node_name: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct StopOutput {
    pub stopped: bool,
    pub was_running: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RestartOutput {
    pub restarted: bool,
    pub pid: Option<u32>,
    pub uptime_seconds: Option<u64>,
    pub node_name: String,
    pub previous_uptime: Option<u64>,
    pub message: String,
}
```

### 2.2 Complete I/O Types

```rust
// ─────────────────────────────────────────────────────────────────────────────
// get_status
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct GetStatusInput {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetStatusOutput {
    pub running: bool,
    pub pid: Option<u32>,
    pub uptime_seconds: Option<u64>,
    pub version: String,
    pub node_name: String,
    pub cluster_status: String,
    pub data_dir: String,
    pub config_dir: String,
    pub listeners_active: usize,
    pub connections_current: u64,
}

// ─────────────────────────────────────────────────────────────────────────────
// get_fabric_status
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct GetFabricStatusInput {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetFabricStatusOutput {
    pub mqtt_loopback_ready: bool,
    pub exhook_ready: bool,
    pub exhook_authenticated: bool,
    /// EMQX is behind the fabric surface and exposes no MCP endpoint.
    pub internal_only: bool,
    pub fabric_surface: String, // fixed discovery value: https://10.0.0.3:8090
}

// ─────────────────────────────────────────────────────────────────────────────
// list_listeners
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EmqxListener {
    pub id: String,
    pub r#type: String,           // tcp, ssl, ws, wss, quic
    pub bind: String,             // Must be loopback or UDS
    pub running: bool,
    pub current_connections: u64,
    pub max_connections: u64,
    pub is_loopback: bool,        // Validation field
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ListListenersInput {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListListenersOutput {
    pub listeners: Vec<EmqxListener>,
    pub total_connections: u64,
    pub all_loopback_or_uds: bool,  // Must be true
}

// ─────────────────────────────────────────────────────────────────────────────
// list_hooks / get_hook_status / configure_hooks
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EmqxExhookServer {
    pub name: String,
    pub url: String,
    pub enable: bool,
    pub status: String,
    pub hooks: Vec<String>,
    /// Must be local UDS or loopback
    pub is_local: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ListHooksInput {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListHooksOutput {
    pub exhook_servers: Vec<EmqxExhookServer>,
    pub total_hooks: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetHookStatusInput {
    pub server_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetHookStatusOutput {
    pub server: Option<EmqxExhookServer>,
    pub found: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConfigureHooksInput {
    /// Must resolve to the single configured OP-DBUS ExHook server.
    pub server_name: String,
    /// A closed enum selected after the pinned-release compatibility probe;
    /// callers cannot supply an arbitrary URL.
    pub target: ExhookTarget,
    pub enable: bool,
    pub hooks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExhookTarget {
    LocalUds,
    LoopbackMtls,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ConfigureHooksOutput {
    pub configured: bool,
    pub server_name: String,
    pub is_local: bool,           // Validation result
    pub message: String,
}
```

---

## 3. ExHook Security Design

### 3.1 Current Problem (grpc_server.rs:867)

```rust
// CURRENT (UNSAFE): ExHook on shared route without interceptor
.add_service(
    crate::proto::emqx_exhook::hook_provider_server::HookProviderServer::new(
        crate::emqx_hook_provider::HookProviderService::new(server.mutation_engine.clone()),
    ),
)
```

### 3.2 Required Solution

Phase 0 first exercises the pinned EMQX release against a real HookProvider and records
the supported target schemes. The following UDS sketch is conditional, not a claim
that EMQX accepts `unix://` on every release. If that probe fails, the required design
is a loopback-only tonic listener with mutual TLS and a dedicated, pinned EMQX client
certificate. Neither option shares the client-facing `:8090` route.

When the pinned release supports UDS, ExHook is mounted on a dedicated local listener
with peer validation:

```rust
// Option A: Dedicated UDS listener with peer credential verification
pub async fn run_exhook_listener(
    mutation_engine: Arc<MutationEngine>,
) -> Result<(), tonic::transport::Error> {
    let uds_path = std::path::Path::new(EXHOOK_UDS);
    
    // Remove stale socket
    let _ = std::fs::remove_file(uds_path);
    
    let uds = tokio::net::UnixListener::bind(uds_path)?;
    let uds_stream = tokio_stream::wrappers::UnixListenerStream::new(uds);
    
    // Illustrative: install as root:emqx, 0660, under a non-world-traversable
    // /run/opdbus directory. Deployment code, not this sketch, performs chown.
    std::fs::set_permissions(uds_path, std::fs::Permissions::from_mode(0o660))?;
    
    let hook_service = HookProviderService::new(mutation_engine);
    
    Server::builder()
        .add_service(HookProviderServer::with_interceptor(
            hook_service,
            peer_credential_interceptor,
        ))
        .serve_with_incoming(uds_stream)
        .await
}

// Peer credential interceptor for UDS
fn peer_credential_interceptor(req: Request<()>) -> Result<Request<()>, Status> {
    // Verify peer credentials via SO_PEERCRED
    // Accept only the dedicated EMQX UID/GID and bind PID + process start time
    // + executable/cgroup provenance. UID alone is not authority.
    // Reject all others
    todo!("Implement peer credential verification")
}
```

The production UDS listener wraps each accepted `UnixStream` in a type implementing
tonic's `Connected` contract. It captures `SO_PEERCRED` from that stream and attaches an
immutable peer record to request extensions; the interceptor reads only that record.
gRPC metadata, request fields, and a caller-claimed PID are never peer evidence.

### 3.3 EMQX Configuration

```hocon
# This form is installed ONLY if the pinned-release compatibility test proves it.
exhook {
  servers = [{
    name = "opdbus"
    enable = true
    # Connect to dedicated UDS, not shared gRPC port
    url = "unix:///run/opdbus/emqx-exhook.sock"
    request_timeout = "5s"
    failed_action = ignore
    hooks = [
      {name = "client.connect"}
      {name = "client.connected"}
      {name = "client.disconnected"}
      {name = "client.authenticate"}
      {name = "client.authorize"}
      {name = "message.publish", topics = ["#"]}
    ]
  }]
}

# Anonymous MQTT MUST be denied
authentication = [
  {
    mechanism = password_based
    backend = built_in_database
    # No anonymous fallback
  }
]

# No MCP topic transport. Deny $mcp-* outright; tightly ACL internal
# $opdbus/fabric/* health/catalog/lifecycle events.
authorization {
  sources = [
    {
      type = built_in_database
    }
  ]
  # Default deny
  no_match = deny
}
```

The loopback fallback uses `https://127.0.0.1:<dedicated-port>` (or the exact scheme
required by the pinned EMQX release), server and client certificates issued solely for
this channel, certificate pinning, and firewall rejection outside loopback. Hook
configuration is rendered to a temporary file, validated, atomically renamed, and
reloaded through the authorized service-management boundary. On failed validation or
reload, the previous configuration is restored. Only an allowlist of hook names from
FR-5.8 is accepted, and secrets are redacted from errors and audit records.

---

## 4. Identity Credential Design

### 4.1 Selected-Sled SID1 Flow

```text
MutationEngine mints/recover session genesis
        │
        ├─ derive registered principal_id from the authoritative WireGuard identity
        ├─ seal immutable SID1 once
        └─ write `sid1:<canonical-unpadded-base64url>` into selected sled `sealed_id`
                                      │
Codex native Streamable HTTP          │
`http_headers_helper` per request     ▼
        │                    op-identity-headers --session <id>
        │                    ├─ read private root:secrets credential projection only
        │                    ├─ never fall back to public/legacy state
        │                    ├─ require current, active, anchored, unambiguous sled
        │                    ├─ validate SID1 claims against that projection
        │                    └─ stdout only:
        │                       {"x-opdbus-sealed-id-bin":"<exact SID1>"}
        ▼
direct HTTPS POST https://10.0.0.3:8090/mcp
        │
        └─ bridge decodes SID1, exact-matches bytes and every claim against the
           authoritative active sled, checks revocation and exact principal grants,
           then applies projection/schema/MutationEngine/audit
```

`MutationEngine` is the only SID1 author. The helper forwards the canonical encoded
payload unchanged; it never reconstructs claims, signs, hashes a footprint, or mints a
credential. It is a short-lived header JSON producer, not an MCP shim, proxy, listener,
broker, transport adapter, or daemon. SID1 is stable for the immutable session term and
may be read for each request; retry does not remint it. Sled deactivation, expiration,
revocation, replacement, or any exact-match failure invalidates it immediately.

The MutationEngine atomically publishes two different identity views. The full sled is
durable only under the root-only identity Cozo tree and projected at runtime only to
`/dev/shm/opdbus/credentials/identity_sled.json` (`0640 root:secrets`, parent `0750`).
The ordinary state-tree file is a separately serialized public view with `sealed_id`
removed recursively. Generic D-Bus reads, StateSync get/subscribe, snapshots, method
results, Snowball/vector payloads, schemas, web responses, and logs use only that public
view. The helper is the bounded private reader; it is not granted database access.

### 4.2 Optional OIA1 Flow for Other Callers

OIA1 remains available as an alternative credential for dashboard or other callers:

```text
authenticated caller → op-decoy-issuer → fresh source/target-bound OIA1
                    → direct HTTPS request to the same :8090/mcp endpoint
                    → signature/time/binding validation → nonce consume
                    → exact principal grants → common dispatch path
```

OIA1 stays single-use and replay-protected; an OIA-authenticated retry obtains a new
assertion. A request must contain exactly one credential—SID1 or OIA1, never both.
Neither credential is published to MQTT or logged. After identity resolution the two
paths converge on the same `VerifiedIdentity`, capability, projection, validation,
MutationEngine, and audit stages.

### 4.3 Direct Codex Configuration

The project configuration is direct and contains no secret material:

```toml
[mcp_servers.op-dbus-unified]
url = "https://10.0.0.3:8090/mcp"
enabled = true
required = true
startup_timeout_sec = 30
tool_timeout_sec = 120
http_headers_helper = "/usr/local/bin/op-identity-headers --session <selected-session-id>"
```

Codex invokes `http_headers_helper` before each native Streamable HTTP request and
adds the returned header object to that direct request. No `command`/stdio MCP server,
MCP shim, HTTP proxy, bearer token, static assertion header, OAuth metadata, login,
callback, authorization route, or token route is introduced. The selector is a bounded
session handle, not a credential; the helper fails closed if it resolves no current
session or more than one current session.

This is a locally installed Codex capability and is verified against the deployed
binary/config schema, not inferred from generic Streamable HTTP documentation. The
release pins and runs the actual client (currently `codex-cli 0.151.0`). If a future
client stops recognizing or invoking `http_headers_helper`, startup fails; no OAuth,
static-header, or MCP-shim fallback is permitted.

### 4.4 EMQX Internal Broker Boundary and MCP Lifecycle

EMQX is a standalone internal broker. Its MQTT listener and authenticated ExHook
listener are loopback/UDS only and carry schema-versioned health, catalog-invalidation,
hook, and lifecycle events. EMQX does not expose or forward MCP, carry SID1/OIA1, or
become an identity/authorization authority. The complete external fabric is the
bridge-owned `10.0.0.3:8090` surface; `/mcp` is its sole MCP route alongside the
same surface's gRPC, plugin, mutation, and control capabilities.

The canonical MCP path is stateless: optional `server/discover`, then direct
`tools/list` and `tools/call`. The same router also accepts native Codex's
`initialize`/`notifications/initialized` startup sequence as protocol compatibility;
it issues no standing authorization or second listener, and every request still
supplies exactly one credential. Method and target come from the parsed JSON-RPC body;
native Codex is not required to synthesize custom `Mcp-Method` or `Mcp-Name` headers.
`MCP-Protocol-Version` follows standard initialize/subsequent-request behavior.
Tool-set selection is a validated `_meta` selector or principal-bound projection
handle and never server-held authority.

---

## 5. D-Bus Identity Resolution Design

### 5.1 Required Implementation

D-Bus does NOT use the gRPC interceptor. A separate mechanism is required:

```rust
/// D-Bus sender identity resolver.
/// 
/// Maps the current D-Bus message sender to an identity through bus-owned
/// credentials and an already-authenticated Ghostbridge session.
pub struct DbusIdentityResolver {
    active_bindings: RwLock<HashMap<String, DbusBinding>>,
    exit_monitor: ProcessExitMonitor,
}

#[derive(Clone)]
struct DbusBinding {
    unique_name: String,
    uid: u32,
    gid: u32,
    pid: u32,
    process_start_ticks: u64,
    principal_id: String,
    session_id: String,
    registered_at: chrono::DateTime<chrono::Utc>,
}

impl DbusIdentityResolver {
    pub async fn resolve(&self, header: &zbus::message::Header<'_>) -> Option<ResolvedIdentity> {
        let sender = header.sender()?.to_string();
        let binding = self.active_bindings.read().await.get(&sender)?.clone();
        let peer = self.read_bus_peer_credentials(&sender).await?;
        let start_ticks = read_proc_start_ticks(peer.pid).ok()?;

        if peer.uid != binding.uid
            || peer.gid != binding.gid
            || peer.pid != binding.pid
            || start_ticks != binding.process_start_ticks
        {
            self.revoke(&sender).await;
            return None;
        }

        let session = self.session_store.lookup_authenticated(&binding.session_id).await?;
        let principal = self.principal_registry.resolve_session(&session).await?;
        if principal.principal_id != binding.principal_id
            || principal.is_revoked()
            || session.is_revoked_or_expired()
        {
            self.revoke(&sender).await;
            return None;
        }

        Some(ResolvedIdentity {
            principal_id: binding.principal_id,
            session_id: binding.session_id,
            session_genesis: session.genesis,
        })
    }

    /// Called only by a capability-gated registration method. The sender is taken
    /// from the message header and the principal/session are resolved from the
    /// authenticated proof. No identity value is accepted as caller input.
    pub async fn register_current_sender(
        &self,
        header: &zbus::message::Header<'_>,
        session_id: &str,
        registration_proof: RegistrationProof,
    ) -> Result<(), RegistrationError> {
        let unique_name = header.sender().ok_or(RegistrationError::NoSender)?.to_string();
        let peer = self.read_bus_peer_credentials(&unique_name).await
            .ok_or(RegistrationError::CannotGetPeerCredentials)?;
        let session = self.session_store
            .verify_registration(session_id, registration_proof, &peer)
            .await?;
        let process_start_ticks = read_proc_start_ticks(peer.pid)?;

        let binding = DbusBinding {
            unique_name: unique_name.clone(),
            uid: peer.uid,
            gid: peer.gid,
            pid: peer.pid,
            process_start_ticks,
            principal_id: session.principal_id,
            session_id: session_id.to_string(),
            registered_at: chrono::Utc::now(),
        };

        self.active_bindings.write().await.insert(unique_name.clone(), binding);
        self.exit_monitor.watch(peer.pid, process_start_ticks, unique_name);
        Ok(())
    }

    pub async fn revoke(&self, unique_name: &str) {
        self.active_bindings.write().await.remove(unique_name);
    }
}
```

`NameOwnerChanged` revokes the matching unique name. Explicit logout is likewise a
current-sender operation and cannot name another owner. The concrete
`RegistrationProof` reuses the authoritative session/OIA machinery; it is not a new
self-asserted token format.

### 5.2 Integration with SchemaBackedInterface

```rust
// In schema_router.rs, update the call method:

async fn call(
    &self,
    method: String,
    json_args: String,
    #[zbus(header)] header: zbus::message::Header<'_>,
) -> zbus::fdo::Result<String> {
    // Resolve identity (fail-closed if no binding)
    let identity = self.identity_resolver.resolve(&header).await
        .ok_or_else(|| zbus::fdo::Error::AccessDenied(
            "No valid D-Bus identity binding for sender".to_string()
        ))?;
    
    self.dispatch_call_for_identity(method, json_args, Some(identity)).await
}
```

### 5.3 Principal, Session, Sled, Footprint, and Snowball Flow

The implementation keeps identity and accountability data in separate types. The
current `HumanPrincipalIdentity.footprint`, `GhostbridgeIdentity.footprint`, and
footprint-keyed MCP caller/grant fields are legacy overloads and are deleted rather
than renamed into another digest.

```rust
struct VerifiedIdentity {
    principal_id: PrincipalId,       // stable authn/authz key
    session_id: SessionId,           // correlation/session lookup
    session_genesis: SessionGenesis, // immutable stamp for this session term
}

struct PluginFootprintPayload {
    actor_id: PrincipalId,           // copied metadata, not derived or hashed identity
    session_id: SessionId,
    session_genesis: SessionGenesis,
    plugin_id: String,
    method_name: String,
    capability_id: String,
    payload: CanonicalRedactedPayload,
}

struct SnowballReceipt {
    event_id: EventId,
    event_hash: EventHash,
}
```

```text
exact selected-sled SID1 or fresh OIA1
  → resolve registered principal_id + current session/genesis
  → grants/audience lookup by principal_id
  → canonical MutationEngine admission
  → sled emits one canonical PluginFootprintPayload with identity metadata
  → Shuttle delivers the same payload
       ├─ Snowball append: H(domain || previous_event_hash || canonical_payload_bytes)
       │                  exactly once → event_id/event_hash receipt
       └─ async vectorization: deterministic text from canonical payload
                              + event receipt as reference
```

`PluginFootprintPayload` is an envelope, not an identity and not a precomputed digest.
The Snowball append path is the sole chain-hash author. The current mutation is not
reduced to `json_args_footprint`, `data_hash`, or `content_hash` and then hashed again;
the current `ChainEvent.json_args_footprint` field is deleted. Vectorization likewise
receives meaningful canonical payload text rather than a hash-only surrogate. The
previous event hash is used only as the normal chain link. The resulting `event_hash`
is a receipt and is never fed back into principal derivation, grants, audience, SID1,
D-Bus registration, or tool-set selection.

### 5.4 A.N.N.A. Scribe Preservation Boundary

A.N.N.A. Scribe (Axon Network Notary Arbitrator) remains the named Scribe persona,
including its established email identity. The identity cleanup removes only the
unsafe duplicate reader that directly opened/memory-mapped
`/dev/shm/plugin_schema.dat` and any duplicate derivation performed from that mapping.
If Scribe needs session identity, it consumes the canonical selected-sled projection;
it does not author SID1, genesis, principal IDs, footprints, or chain hashes. A prior
revision that deleted the whole Scribe module/persona must restore the presentation
and email contract without restoring the mmap reader. This active design supersedes
the broader `anna_scribe` deletion instruction in `session-genesis-identity`.

---

## 6. MutationEngine Wiring

Add to `crates/op-grpc-bridge/src/mutation_engine.rs` in `dispatch_method_call`:

```rust
// In the match statement for plugin_id:
"emqx" => {
    let args = serde_json::to_value(&parsed_value)?;
    op_plugins::state_plugins::emqx::dispatch_emqx_method(method, &args).await?
}
```

Read methods use bounded, credential-redacted calls to the loopback EMQX API. Lifecycle
methods call the existing authorized D-Bus runit manager with the literal service name
`emqx`; there is no `service_name` input and no shell execution in the plugin. Hook
configuration uses validate → temporary render → atomic replace → authorized reload →
health check, restoring the prior file on failure.

---

## 7. OSCAL Subid CI Validation

### 7.1 Test Implementation

Extend the existing global registry uniqueness gate in
`crates/op-plugins/src/default_registry.rs` so it also walks every method subid. Do
not create a second, partially overlapping test in `state_plugins/mod.rs`. The sketch
below is illustrative; the existing registry enumerator remains authoritative.

```rust
// In crates/op-plugins/src/state_plugins/mod.rs or a dedicated test file

#[cfg(test)]
mod subid_uniqueness_tests {
    use std::collections::HashSet;
    
    #[test]
    fn all_plugin_subids_are_unique() {
        let mut all_subids = HashSet::new();
        let mut duplicates = Vec::new();
        
        // Collect subids from all plugins
        for plugin in crate::default_registry::available_plugins() {
            if let Some(schema) = plugin.schema() {
                for (method_name, method_decl) in &schema.methods {
                    let subid = &method_decl.subid;
                    if !all_subids.insert(subid.clone()) {
                        duplicates.push(format!(
                            "Duplicate subid '{}' in {}.{}",
                            subid, schema.name, method_name
                        ));
                    }
                }
            }
        }
        
        assert!(
            duplicates.is_empty(),
            "Duplicate OSCAL subids found:\n{}",
            duplicates.join("\n")
        );
    }
}
```

---

## 8. Deployment Artifact Tracking

### 8.1 Repository Structure

```
deploy/
├── runit/
│   ├── emqx/
│   │   └── run                     # Tracked service definition
│   └── build-golden.sh             # Handles registration
├── config/
│   ├── emqx.version                # Pinned version + checksum
│   └── emqx-integration.version    # Retained ExHook/MQTT integration, if separate
└── checksums/
    ├── emqx-5.x.x.sha256           # Binary checksum
    └── emqx-integration.sha256     # Internal integration checksum, if separate
```

### 8.2 Version Pinning Format

```
# deploy/config/emqx.version
VERSION=<phase-0-compatible-version>
SHA256=<full-distribution-sha256>
ARTIFACT=deploy/artifacts/emqx/emqx-${VERSION}.tar.gz
SOURCE_URL=<provenance-only-url>

# deploy/config/emqx-integration.version (only if ExHook/MQTT integration is separate)
VERSION=<compatible-plugin-version>
SHA256=<plugin-sha256>
ARTIFACT=deploy/artifacts/emqx/emqx-integration-${VERSION}.tar.gz
```

Phase 0 owns the actual pin. A placeholder release such as `5.8.0` must not survive
approval, and the locally available EMQX checkout/package is not assumed compatible.
The decision record includes edition/license, ExHook target support, loopback MQTT
compatibility, confirmation that no MCP gateway/listener is installed, and checksums. `ARTIFACT` is a deterministic offline build
input available before deployment; `SOURCE_URL` is provenance only and is never
fetched at boot or by the runit service.

### 8.3 build-golden.sh Integration

`build-golden.sh` is POSIX `/bin/sh`; additions use `var=value`, `[ ... ]`, and its
existing `run`/`install` helpers—never Bash-only `local` or `[[ ... ]]` syntax.

Integration uses the mechanisms already present in the script:

1. Add `emqx` to `deploy/runit/managed-services` and
   `deploy/runit/enabled-services` so the generic service-definition and enablement
   paths own `/etc/runit/sv/emqx` and its symlink.
2. Read the pinned manifest, resolve the explicit offline `ARTIFACT`, and verify its
   checksum before changing either target.
3. Extract/stage the same verified distribution into the golden target and live target
   selected by the script. Never verify an unrelated pre-existing `/usr/bin/emqx` as a
   substitute for the release input.
4. Materialize immutable runtime/config into the release target while mounting or
   linking persistent `/var/lib/emqx` from the data subvolume defined by
   `deploy/btrfs-layout.sh`.
5. Let the generic managed-service path install `deploy/runit/emqx/run`; do not create
   service symlinks in a custom function.
6. `--dry-run`, `--golden-only`, and `--live-only` exercise the same manifest and
   checksum logic. EMQX is reported for deliberate restart; network-critical services
   remain untouched.

---

## 9. Audience Projection and Tool Sets

### 9.1 Server-Side Audience Policy

The bridge loads one exact `singleton_chatbot_principal_id` from protected server-side
configuration. Rotation is an audited two-step operation (install replacement grant,
then retire the old principal binding); zero or multiple active singleton values fail closed for
the chatbot projection. No request-supplied field participates in this decision.

`deploy/config/mcp-audience-policy.json`:

```json
{
  "version": 1,
  "rotation_epoch": 1,
  "singleton_chatbot_principal_id": "<registered-principal-id>"
}
```

The loader validates the principal against the authoritative principal registry plus
file ownership/mode and rejects
client-name globs. Production startup also rejects a `"*"` identity in the capability
grant source, as well as any 64-hex footprint/genesis/hash key. The current broad
wildcard and legacy digest-keyed entry are migrated to registered `principal_id`
entries before projection rollout; audience configuration never creates those grants.

```rust
const CHATBOT_HOT: [&str; 5] = [
    "memory_recall",
    "memory_store",
    "workflow_query",
    "workflow_run",
    "toolsets",
];

enum Audience {
    SingletonChatbot,
    ExternalAgent,
}

fn audience(identity: &VerifiedIdentity, policy: &AudiencePolicy) -> Audience {
    if policy.has_exact_singleton()
        && identity.principal_id() == policy.singleton_chatbot_principal_id()
    {
        Audience::SingletonChatbot
    } else {
        Audience::ExternalAgent
    }
}
```

The singleton chatbot's initial discovery is exactly `CHATBOT_HOT`, filtered by its
exact grants; deployment provisions all five grants for that principal. The former
`list_tools`, `search_tools`, `get_tool_schema`, `execute_tool`, and `invoke_tool`
surface is removed from the active catalog and denied by name for every identity.
`toolsets` is the only drill-down contract and exposes real typed tools, never a
generic executor.

### 9.2 Stateless Tool-Set Selection

`toolsets` is a typed HOT tool with operations equivalent to `list` and `select`.
Selection returns:

```rust
struct ToolsetSelectionOutput {
    selected_set: String,
    catalog_generation: u64,
    projection_handle: String, // signed, opaque, principal+client-bound, short-lived
    expires_at_unix_ms: u64,
    relist_required: bool,
}
```

The client sends the handle in canonical request `_meta` when it re-lists and calls a
tool. The handle contains no authority: validation recovers only the requested set,
principal/client binding, generation, and expiry. The bridge recomputes capabilities
and health on every request. A plain validated `toolset_id` in `_meta` may replace the
signed handle when the client and catalog protocol make it equally deterministic; it
still cannot grant authority.

For the singleton chatbot and other identities:

```text
default visible = exact grants ∩ CHATBOT_HOT
selected visible = exact grants ∩ audience policy
                   ∩ (CHATBOT_HOT ∪ exactly one selected typed set)
                   ∩ provider health
```

HOT tools ignore provider-health filtering. Unknown, expired, forged,
cross-principal, or stale-generation selection never expands to the full registry; it
returns a typed selector error or the deterministic HOT view. The same projection is
checked at `tools/call`, so guessing a hidden name does not bypass discovery. A valid
selection triggers `notifications/tools/list_changed` only when negotiated; otherwise
the selection output explicitly requires a new `tools/list`.

This re-list is performed by the MCP host client, not by the model. A standard model
cannot issue `tools/list`, and reading schemas returned by `toolsets` does not install
them as callable tools. The host must capture the opaque selector, re-list with it in
request `_meta`, update the model-visible typed catalog, and preserve the selector on
calls. Acceptance therefore drives the actual installed Codex and Grok clients. If a
client lacks selector-aware re-list support, it remains on the exact HOT five and the
implementation reports drill-down unsupported for that client; it never claims the
returned schemas became callable.

Tool-set manifests are versioned deterministic configuration containing only typed
tool names, temperature, provider identity, and health policy. They cannot contain
capability grants. An `emqx_ops` set, for example, exposes the generated typed EMQX
methods for which that exact caller already has grants; no caller receives
`invoke_tool`, `execute_tool`, or another generic executor.

`deploy/config/mcp-toolsets.json` (abridged):

```json
{
  "generation": 1,
  "hot": ["memory_recall", "memory_store", "workflow_query", "workflow_run", "toolsets"],
  "sets": [
    {
      "id": "emqx_ops",
      "temperature": "warm",
      "provider": "emqx",
      "requires_provider_health": true,
      "tools": ["plugin.emqx.get_status", "plugin.emqx.list_hooks", "plugin.emqx.restart"]
    }
  ]
}
```

Activation validates every referenced canonical name, rejects duplicates/legacy alias
collisions, and publishes the generation atomically.

### 9.3 Projection Order

```text
exact SID1 or fresh OIA1 → registered principal_id → principal grants → audience
          → HOT/default or validated selected set → provider health
          → tools/list projection or tools/call projection check
          → typed schema validation → canonical MutationEngine path once → audit
```

The ordering prevents `clientInfo`, catalog messages, tool-set selection, or provider
health from becoming identity or authorization inputs.

## 10. HOT/WARM/COLD Runtime and EMQX

### 10.1 HOT Lane

Before the listener reports ready, the bridge constructs the schemas and adapters for
the singleton chatbot's five HOT contracts and `toolsets`. The stable HOT names
are projection facades over existing canonical memory/workflow implementations where
available; they do not construct a second registry, store, or workflow engine. The
facade resolves its target before catalog activation, fails on a canonical/legacy name
collision, and enters canonical dispatch exactly once. Memory/workflow indexes may be
resident, but Cozo/Qdrant or the configured durable stores remain the authoritative
copy. After the common admission pipeline, a HOT call performs no MQTT, EMQX, provider
discovery, or cold initialization round trip.

HOT is a latency/residency classification, not a trust class. Exact active-sled SID1
or fresh OIA1, exact grants, projection checks, typed validation, MutationEngine admission,
and inline immutable audit remain mandatory. Phase 0 records p50/p95 admission-to-dispatch
baselines and the rollout record sets the permitted regression budget.

### 10.2 WARM/COLD Sets

WARM and COLD tools remain real typed tools in the single registry/generated-plugin
surface. Explicit selection narrows discovery to one domain plus the HOT core. If a
provider is not ready, the bridge returns a typed `provider_unavailable` or
`activation_pending` result; it does not fall back to the full catalog or to generic
execution. Promotion/demotion is a reviewed manifest change with a catalog generation,
never an automatic frequency-based mutation.

### 10.3 EMQX Async Role and Failure Mode

EMQX may distribute authenticated, schema-versioned messages for provider health,
catalog invalidation, hook events, and WARM/COLD activation coordination. The bridge
checks producer identity and monotonic catalog generation, reconciles the hint with
authoritative local configuration, and updates a bounded local snapshot. OIA bytes are
never published.

The synchronous execution route remains HTTP/gRPC/D-Bus → canonical admission →
MutationEngine. Audit commits inline; MQTT mirroring happens afterward and is
best-effort. With EMQX stopped:

- `https://10.0.0.3:8090/mcp` remains healthy and authenticated;
- the singleton chatbot's five HOT tools and `toolsets` remain callable;
- EMQX-backed WARM/COLD tools report typed unavailability; and
- no second endpoint or authorization fallback appears.

---

## 11. File Changes Summary

### Files to Create

| File | Purpose |
|------|---------|
| `deploy/runit/emqx/run` | Tracked runit service definition |
| `deploy/config/emqx.version` | Pinned EMQX version + checksum |
| `deploy/config/mcp-audience-policy.json` | Exact registered singleton chatbot `principal_id` and audited rotation epoch; no client-name authority |
| `deploy/config/mcp-toolsets.json` | Versioned HOT/WARM/COLD membership and provider metadata; no grants |
| `crates/op-identity/src/sealed_id.rs` | Bounded deterministic SID1 envelope authored by MutationEngine and exact-matched to one sled |
| `crates/op-identity/src/bin/op-identity-headers.rs` | Short-lived selected-sled reader that emits header JSON only |
| Tests for subid uniqueness | CI validation |
| Tests for ExHook peer validation | Security tests |

### Files to Modify

| File | Changes |
|------|---------|
| `crates/op-plugins/src/state_plugins/emqx.rs` | Complete rewrite for standalone |
| `crates/op-grpc-bridge/src/mutation_engine.rs` | Add `"emqx"` match arm and author one SID1 when the selected identity sled is anchored |
| `crates/op-grpc-bridge/src/grpc_server.rs` | Move ExHook to dedicated listener |
| `crates/op-grpc-bridge/src/schema_router.rs` | Add D-Bus identity resolver integration |
| `crates/op-grpc-bridge/src/mcp_frontend.rs` | Exact-match SID1 or validate optional OIA1; apply five-HOT/tool-set projection; support direct Codex initialize in the same router |
| `crates/op-grpc-bridge/src/interceptor.rs`, `oracle_assertion.rs`, and `mcp_frontend.rs` | Remove derived human/Ghostbridge footprints from auth; carry `principal_id`, session, and genesis separately; key grants/audience by principal |
| `crates/op-grpc-bridge/src/cognitive_mcp.rs` and registry wiring | Remove the four generic compact meta-tools; register the five typed HOT tools and `toolsets` projection |
| `crates/op-state-store/src/event_chain.rs`, `crates/op-snowball/src/footprint.rs`, legacy `plugin_footprint.rs`, and Snowball append wiring | Replace `json_args_footprint`/digest-only audit input with canonical bounded/redacted payload; consolidate one sled-emitted envelope; remove hash-of-hash generators; make Snowball the sole event-hash author and feed payload text to vectorization |
| Project `.codex/config.toml` | Keep the direct `https://10.0.0.3:8090/mcp` URL and configure native per-request `http_headers_helper`; no OAuth or stdio MCP command |
| `.kiro/specs/README.md` | Update topology lock to `10.0.0.3:8090` |
| `deploy/runit/build-golden.sh` | Add EMQX service handling |
| `deploy/runit/managed-services`, `deploy/runit/enabled-services` | Register and enable EMQX through existing generic logic |

### Files NOT to Modify

| File | Reason |
|------|--------|
| Any generated `.proto` or route files | Build artifacts |
| `/run/runit/service/*` | Supervisor runtime view |
