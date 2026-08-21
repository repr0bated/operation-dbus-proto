//! Axum gRPC/gRPC-Web host for the tched_router plugin schema.
//!
//! Serves the plugin-owned schema JSON on:
//!   - host native gRPC over Unix socket `/run/opdbus/grpc.sock`
//!   - shared container UDS `/run/ghostbridge/container.sock` (same routes)
//!   - one TCP door: tonic-web on `:8090` (HTTP + native gRPC demux)
//!
//! The schema itself is never generated here; it is read from the plugin's
//! sealed blob in the SHM catalog (`/dev/shm/opdbus/plugin-blobs`).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use async_stream::stream;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{header, Response, StatusCode};
use axum::routing::get;
use axum::Router;
use hyper::client::conn::http1;
use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;
use tonic::transport::{Identity, ServerTlsConfig};
use tonic::{Request as TonicRequest, Response as TonicResponse, Status};
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use crate::grpc_server::OperationGrpcServer;
use crate::mutation_engine::MutationEngine;
use crate::schema_loader::{SchemaLoader, SchemaReloadEvent};
use crate::schema_router::SchemaRouter;
use crate::tracing::{GhostbridgeTraceLayer, TraceContext};

use crate::grpc_server::{
    attach_cognitive_tool_service, build_operation_routes, init_cognitive_mcp,
};
use crate::proto::tched_router::{
    tched_router_service_server::{TchedRouterService, TchedRouterServiceServer},
    GetSchemaRequest, SchemaEvent, SchemaResponse, WatchSchemaRequest,
};

/// Host-local native gRPC UDS (op-web, zbusctl, operators on the host).
const DEFAULT_UNIX_SOCKET: &str = "/run/opdbus/grpc.sock";
/// Shared container socket — bind-mounted into NIC-less CTs as `/run/ghostbridge`.
const DEFAULT_SHARED_SOCKET: &str = crate::shared_socket::DEFAULT_SOCKET_PATH;
/// EMQX's MQTT-over-WebSocket payload socket. `/mqtt` on the one TCP door is
/// reverse-upgraded through this socket; port 8083 remains container-internal.
const DEFAULT_EMQX_BROKER_SOCKET: &str = op_plugins::state_plugins::emqx::BROKER_SOCKET;
// One TCP door: tonic-web on :8090 demuxes HTTP and native gRPC.
// CognitiveToolService is mounted on that same surface (and the sockets).
// Bind loopback only. Mesh/svc0 publications forward to this listener so every
// client traverses the same HTTP/gRPC/MQTT demux and boot does not depend on
// netclient creating the mesh address first.
// Live override: ZEROCLAW_BIND_ADDR / deploy/runit/op-grpc-bridge/run.
const DEFAULT_BIND_ADDR: &str = "127.0.0.1:8090";
/// Default schema source: the sealed blob catalog dir. When `schema_path`
/// is a directory the loader reads the plugin's own blob from it (a blob in
/// the catalog IS the plugin); a file path is still accepted for tests and
/// plugin-owned schema files.
const DEFAULT_SCHEMA_PATH: &str = op_blob::catalog::DEFAULT_SHM_DIR;

/// Runtime configuration for the tched_router Axum host.
#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub plugin_id: String,
    pub schema_path: PathBuf,
    /// Host-local gRPC UDS (`ZEROCLAW_UNIX_SOCKET`, default `/run/opdbus/grpc.sock`).
    pub unix_socket: PathBuf,
    /// Shared container UDS (`GHOSTBRIDGE_SOCKET_PATH`, default
    /// `/run/ghostbridge/container.sock`). Served with the same route set.
    /// When equal to `unix_socket`, only one listener is opened.
    pub shared_socket: PathBuf,
    /// EMQX MQTT/WebSocket payload UDS used by the `/mqtt` demux on `:8090`.
    pub emqx_broker_socket: PathBuf,
    pub bind_addr: String,
    pub tls_identity: Option<Identity>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            plugin_id: "tched_router".to_string(),
            schema_path: PathBuf::from(DEFAULT_SCHEMA_PATH),
            unix_socket: PathBuf::from(DEFAULT_UNIX_SOCKET),
            shared_socket: PathBuf::from(DEFAULT_SHARED_SOCKET),
            emqx_broker_socket: PathBuf::from(DEFAULT_EMQX_BROKER_SOCKET),
            bind_addr: DEFAULT_BIND_ADDR.to_string(),
            tls_identity: None,
        }
    }
}

impl ServerConfig {
    /// Load configuration from environment variables, falling back to defaults.
    pub fn from_env() -> Self {
        Self {
            plugin_id: std::env::var("OP_DBUS_SCHEMA_PLUGIN_ID")
                .or_else(|_| std::env::var("ZEROCLAW_PLUGIN_ID"))
                .unwrap_or_else(|_| "tched_router".to_string()),
            schema_path: PathBuf::from(
                std::env::var("ZEROCLAW_SCHEMA_PATH")
                    .unwrap_or_else(|_| DEFAULT_SCHEMA_PATH.to_string()),
            ),
            unix_socket: PathBuf::from(
                std::env::var("ZEROCLAW_UNIX_SOCKET")
                    .unwrap_or_else(|_| DEFAULT_UNIX_SOCKET.to_string()),
            ),
            shared_socket: PathBuf::from(
                std::env::var("GHOSTBRIDGE_SOCKET_PATH")
                    .unwrap_or_else(|_| DEFAULT_SHARED_SOCKET.to_string()),
            ),
            emqx_broker_socket: PathBuf::from(
                std::env::var("EMQX_BROKER_SOCKET")
                    .unwrap_or_else(|_| DEFAULT_EMQX_BROKER_SOCKET.to_string()),
            ),
            bind_addr: std::env::var("ZEROCLAW_BIND_ADDR")
                .ok()
                .filter(|s| !s.trim().is_empty())
                .or_else(|| {
                    std::env::var("GRPC_BIND")
                        .ok()
                        .filter(|s| !s.trim().is_empty())
                })
                .unwrap_or_else(|| DEFAULT_BIND_ADDR.to_string()),
            tls_identity: Self::load_tls_identity(),
        }
    }

    /// Load the TLS identity used by the primary TCP listener.
    ///
    /// Env: `ZEROCLAW_TLS_CERT_FILE`/`ZEROCLAW_TLS_KEY_FILE` (preferred) or
    /// `ZEROCLAW_TLS_CERT`/`ZEROCLAW_TLS_KEY` (PEM). If neither pair is set,
    /// generate a self-signed development identity. Production runit wiring
    /// always supplies stable certificate files.
    fn load_tls_identity() -> Option<Identity> {
        let cert_file = std::env::var("ZEROCLAW_TLS_CERT_FILE")
            .ok()
            .filter(|s| !s.trim().is_empty());
        let key_file = std::env::var("ZEROCLAW_TLS_KEY_FILE")
            .ok()
            .filter(|s| !s.trim().is_empty());

        match (cert_file, key_file) {
            (Some(cert_path), Some(key_path)) => {
                let cert = std::fs::read(&cert_path).unwrap_or_else(|error| {
                    panic!("failed to read TLS certificate {cert_path}: {error}")
                });
                let key = std::fs::read(&key_path).unwrap_or_else(|error| {
                    panic!("failed to read TLS private key {key_path}: {error}")
                });
                tracing::info!(
                    cert_path,
                    key_path,
                    "TLS identity loaded from certificate files"
                );
                return Some(Identity::from_pem(cert, key));
            }
            (Some(_), None) | (None, Some(_)) => {
                panic!(
                    "ZEROCLAW_TLS_CERT_FILE and ZEROCLAW_TLS_KEY_FILE must be configured together"
                );
            }
            (None, None) => {}
        }

        let cert_pem = std::env::var("ZEROCLAW_TLS_CERT").ok();
        let key_pem = std::env::var("ZEROCLAW_TLS_KEY").ok();

        match (cert_pem, key_pem) {
            (Some(cert), Some(key)) => {
                tracing::info!(
                    "TLS identity loaded from ZEROCLAW_TLS_CERT/ZEROCLAW_TLS_KEY env vars"
                );
                Some(Identity::from_pem(cert, key))
            }
            (Some(_), None) | (None, Some(_)) => {
                panic!("ZEROCLAW_TLS_CERT and ZEROCLAW_TLS_KEY must be configured together");
            }
            (None, None) => {
                let ck = rcgen::generate_simple_self_signed(vec![
                    "localhost".to_string(),
                    "ghostbridge.tech".to_string(),
                    "3tched.com".to_string(),
                ])
                .expect("failed to generate self-signed TLS cert");
                let cert_pem = ck.cert.pem();
                let key_pem = ck.key_pair.serialize_pem();
                tracing::info!("TLS identity auto-generated (self-signed for localhost, ghostbridge.tech, 3tched.com)");
                Some(Identity::from_pem(cert_pem, key_pem))
            }
        }
    }
}

/// Shared state for the gRPC service handlers.
#[derive(Clone)]
struct TchedRouterGrpcService {
    loader: Arc<SchemaLoader>,
}

#[tonic::async_trait]
impl TchedRouterService for TchedRouterGrpcService {
    type WatchSchemaStream =
        Pin<Box<dyn tokio_stream::Stream<Item = Result<SchemaEvent, Status>> + Send>>;

    async fn get_schema(
        &self,
        request: TonicRequest<GetSchemaRequest>,
    ) -> Result<TonicResponse<SchemaResponse>, Status> {
        let context = request
            .extensions()
            .get::<TraceContext>()
            .cloned()
            .unwrap_or_else(|| TraceContext::from_tonic_metadata(request.metadata()));

        let schema_json = self.loader.get().await.to_string();
        let mut response = TonicResponse::new(SchemaResponse {
            schema_json,
            trace_id: context.trace_id.clone(),
            footprint: context.genesis.clone().unwrap_or_default(),
        });
        inject_trace_metadata(&mut response, &context);
        Ok(response)
    }

    async fn watch_schema(
        &self,
        _request: TonicRequest<WatchSchemaRequest>,
    ) -> Result<TonicResponse<Self::WatchSchemaStream>, Status> {
        let mut reload_rx = self.loader.reload_tx().subscribe();
        let loader = self.loader.clone();

        let initial = loader.get().await;
        let stream = stream! {
            yield Ok(SchemaEvent {
                schema_json: initial.to_string(),
                event_type: "initial".to_string(),
            });

            while let Ok(SchemaReloadEvent { event_type }) = reload_rx.recv().await {
                let schema_json = loader.get().await.to_string();
                yield Ok(SchemaEvent {
                    schema_json,
                    event_type,
                });
            }
        };

        Ok(TonicResponse::new(Box::pin(stream)))
    }
}

fn inject_trace_metadata<T>(response: &mut TonicResponse<T>, context: &TraceContext) {
    if let Ok(trace_id) = context.trace_id.parse() {
        response
            .metadata_mut()
            .insert("x-ghostbridge-trace-id", trace_id);
    }
    if let Some(genesis) = context.genesis.as_deref() {
        if let Ok(genesis) = genesis.parse() {
            response
                .metadata_mut()
                .insert("x-ghostbridge-genesis", genesis);
        }
    }
}

/// Assemble the gRPC service set for the tched_router bridge.
///
/// The full backplane surface (StateSync, PluginService, DbusPassthrough,
/// OvsdbMirror, RuntimeMirror, EventChainService, ComponentRegistry, Mail,
/// Privacy, Registration, McpService, ChatService, reflection) comes from the
/// shared [`build_operation_routes`] — identical to what `run_grpc_server` mounts
/// on op-dbus :50051, so reflection never advertises an unmounted service. On top
/// of that the tched_router bridge adds its endpoint-specific `TchedRouterService`
/// (plugin-owned schema get/watch).
///
/// Every container shares this one surface over `container.sock`; the assistant
/// container is provisioned exactly like the others (its socket created via the
/// unix-socket plugin's createsocket through PluginService, not a raw Incus proxy
/// device).
fn build_routes(loader: Arc<SchemaLoader>, server: OperationGrpcServer) -> tonic::service::Routes {
    let tched_router_svc =
        crate::grpc_web::enable(TchedRouterServiceServer::new(TchedRouterGrpcService {
            loader,
        }));
    build_operation_routes(server).add_service(tched_router_svc)
}

#[derive(Clone)]
struct MqttProxyState {
    broker_socket: PathBuf,
}

fn mqtt_proxy_error(status: StatusCode, message: &'static str) -> Response<Body> {
    Response::builder()
        .status(status)
        .body(Body::from(message))
        .expect("static MQTT proxy error response")
}

fn header_has_token(
    headers: &axum::http::HeaderMap,
    name: header::HeaderName,
    token: &str,
) -> bool {
    headers
        .get_all(name)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .any(|value| value.trim().eq_ignore_ascii_case(token))
}

/// Reverse-proxy an MQTT-over-WebSocket upgrade from the one TCP door to the
/// broker's Unix socket. EMQX performs the real HTTP handshake; after both
/// peers return `101`, the bridge copies the upgraded streams as opaque bytes.
async fn mqtt_websocket_proxy(
    State(state): State<MqttProxyState>,
    mut request: Request,
) -> Response<Body> {
    if request.method() != axum::http::Method::GET
        || !header_has_token(request.headers(), header::CONNECTION, "upgrade")
        || !header_has_token(request.headers(), header::UPGRADE, "websocket")
    {
        return mqtt_proxy_error(
            StatusCode::BAD_REQUEST,
            "the /mqtt route requires a WebSocket upgrade",
        );
    }

    let downstream_upgrade = hyper::upgrade::on(&mut request);
    let broker = match UnixStream::connect(&state.broker_socket).await {
        Ok(stream) => stream,
        Err(error) => {
            tracing::warn!(
                path = %state.broker_socket.display(),
                %error,
                "MQTT demux could not connect to EMQX broker socket"
            );
            return mqtt_proxy_error(StatusCode::BAD_GATEWAY, "EMQX broker socket unavailable");
        }
    };

    let (mut sender, connection) = match http1::handshake(TokioIo::new(broker)).await {
        Ok(parts) => parts,
        Err(error) => {
            tracing::warn!(%error, "MQTT demux could not open upstream HTTP connection");
            return mqtt_proxy_error(StatusCode::BAD_GATEWAY, "EMQX broker handshake unavailable");
        }
    };
    tokio::spawn(async move {
        if let Err(error) = connection.with_upgrades().await {
            tracing::debug!(%error, "MQTT upstream HTTP connection ended");
        }
    });

    let mut response = match sender.send_request(request).await {
        Ok(response) => response,
        Err(error) => {
            tracing::warn!(%error, "MQTT demux request to EMQX failed");
            return mqtt_proxy_error(StatusCode::BAD_GATEWAY, "EMQX broker request failed");
        }
    };

    if response.status() == StatusCode::SWITCHING_PROTOCOLS {
        let upstream_upgrade = hyper::upgrade::on(&mut response);
        tokio::spawn(async move {
            let (downstream, upstream) =
                match tokio::try_join!(downstream_upgrade, upstream_upgrade) {
                    Ok(upgrades) => upgrades,
                    Err(error) => {
                        tracing::debug!(%error, "MQTT WebSocket upgrade did not complete");
                        return;
                    }
                };
            let mut downstream = TokioIo::new(downstream);
            let mut upstream = TokioIo::new(upstream);
            match tokio::io::copy_bidirectional(&mut downstream, &mut upstream).await {
                Ok((to_broker, to_client)) => {
                    tracing::debug!(to_broker, to_client, "MQTT WebSocket demux stream closed")
                }
                Err(error) => tracing::debug!(%error, "MQTT WebSocket demux stream failed"),
            }
        });
    }

    response.map(Body::new)
}

/// Build the Axum router for the one TCP door. Exact `/mqtt` WebSocket traffic
/// goes to EMQX; every other route remains on the tonic gRPC/gRPC-Web surface.
pub fn build_axum_app_with_mqtt_socket(
    loader: Arc<SchemaLoader>,
    server: OperationGrpcServer,
    broker_socket: PathBuf,
) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
        .expose_headers([
            "grpc-status".parse().unwrap(),
            "grpc-message".parse().unwrap(),
            "grpc-status-details-bin".parse().unwrap(),
        ]);

    build_routes(loader, server)
        .into_axum_router()
        .route(
            "/mqtt",
            get(mqtt_websocket_proxy).with_state(MqttProxyState { broker_socket }),
        )
        .layer(cors)
        .layer(GhostbridgeTraceLayer::new())
}

/// Build the production Axum app using the canonical NetMaker broker socket.
pub fn build_axum_app(loader: Arc<SchemaLoader>, server: OperationGrpcServer) -> Router {
    build_axum_app_with_mqtt_socket(loader, server, PathBuf::from(DEFAULT_EMQX_BROKER_SOCKET))
}

/// Build the tonic `Routes` used for the native-gRPC Unix socket side.
fn build_tonic_routes(
    loader: Arc<SchemaLoader>,
    server: OperationGrpcServer,
) -> tonic::service::Routes {
    build_routes(loader, server)
}

/// Run the tched_router Axum host.
///
/// Returns only when both listeners exit (which should not happen under normal
/// operation).
pub async fn run_tched_router_server(config: ServerConfig) -> anyhow::Result<()> {
    // PluginService (PluginService.CallMethod) backed by the authoritative
    // MutationEngine. This is how createunixsocket is invoked: a CallMethod with
    // plugin_id="unix_socket" routes through MutationEngine.mutate →
    // UnixSocketPlugin::create_unix_socket. The sled identity is resolved as
    // the actor_id inside mutate when the caller omits it.
    let event_chain = Arc::new(tokio::sync::RwLock::new(op_state_store::EventChain::new(
        op_state_store::ChainConfig::default(),
    )));
    let ovsdb = Arc::new(op_network::rovs_proxy::OvsdbDbusClient::new());
    let mutation_engine = Arc::new(MutationEngine::new(event_chain, ovsdb));
    let cognitive = init_cognitive_mcp().await;
    if let Some(ref handle) = cognitive {
        mutation_engine
            .attach_cognitive_mcp(Some(handle.tool_registry()), Some(handle.context_state()));
    }
    // Open the durable audit sink and replay the pre-restart trail before any
    // surface accepts traffic, so an immediate query sees prior history.
    let replayed = mutation_engine.init_audit_durability().await;
    if replayed > 0 {
        tracing::info!(replayed, "audit trail restored from durable storage");
    }
    mutation_engine.seed_missing_plugin_projections().await?;

    // Seeding can replace a stale sealed schema with the plugin's current
    // projection. Load only after that write so the later reflection freeze
    // cannot persist an older in-memory schema back over the fresh blob.
    let loader = Arc::new(SchemaLoader::new_for_plugin(
        &config.schema_path,
        config.plugin_id.clone(),
    )?);

    // SIGHUP reload watcher.
    let _sighup_handle = loader.clone().watch_sighup();

    // Start the OVSDB → process_authoritative_change feed after seed (same as run_grpc_server).
    match mutation_engine.clone().start().await {
        Ok(()) => tracing::info!("MutationEngine OVSDB monitor started"),
        Err(error) => tracing::warn!(
            %error,
            "MutationEngine::start failed; continuing without OVSDB feed"
        ),
    }
    let mutation_engine_for_dbus = mutation_engine.clone();
    let operation_server = OperationGrpcServer::new(mutation_engine);
    // The sealed SHM blob catalog IS the plugin set: hydrate reflection from
    // it so a bridge restart advertises every sealed plugin immediately.
    operation_server.hydrate_reflection_from_shm().await;

    // Activate the frozen per-method descriptors.
    //
    // Hydrating the catalog above only makes the sealed blobs *discoverable*; it
    // does not mount them. This turns each method's frozen descriptor into a live
    // typed gRPC service (one service per method, e.g.
    // `operation.method.cognitive_mcp.invoke_tool.InvokeToolService`) and registers
    // it with the per-method reflection registry.
    //
    // Must run before `build_axum_app` below: tonic-reflection is immutable once
    // mounted, so a service activated after route construction can never be served.
    // `run_grpc_server` already did this; omitting it here meant the
    // tched_router bridge advertised sealed plugins while serving none of their typed
    // per-method services.
    operation_server.freeze_plugin_method_reflection().await;

    // Mount CognitiveToolService on the same 8090 / socket surface.
    let operation_server = attach_cognitive_tool_service(operation_server, cognitive).await;

    // ── D-Bus plugin object registration ──────────────────────────────────
    // Register all plugin objects from the SHM blob catalog on the session bus
    // so `busctl tree org.opdbus.v1.plugins` shows the full plugin tree.
    // SchemaRouter reads sealed blobs and creates SchemaBackedInterface objects
    // with real methods/properties matching each plugin's schema, dispatching
    // calls through the same MutationEngine as the gRPC path.
    tokio::spawn(async move {
        let addr = std::env::var("DBUS_SESSION_BUS_ADDRESS")
            .unwrap_or_else(|_| op_core::config::SESSION_BUS_ADDRESS.to_string());
        let conn = match zbus::connection::Builder::address(addr.as_str()) {
            Ok(builder) => match builder.build().await {
                Ok(c) => c,
                Err(e) => {
                    tracing::error!(error = %e, "Failed to connect to session bus for D-Bus plugin objects");
                    return;
                }
            },
            Err(e) => {
                tracing::error!(error = %e, "Invalid session bus address for D-Bus plugin objects");
                return;
            }
        };

        let dbus_conn = Arc::new(tokio::sync::OnceCell::new());
        let _ = dbus_conn.set(conn.clone());
        let engine_for_signal = mutation_engine_for_dbus.clone();
        let router = SchemaRouter::with_engine(dbus_conn, mutation_engine_for_dbus);

        if let Err(e) = router.register_objects().await {
            tracing::error!(error = %e, "Failed to register authoritative D-Bus plugin objects");
        } else {
            // Store the signal bus on the MutationEngine so emit_updated_signal works.
            engine_for_signal.set_signal_bus(conn.clone());
            // Request the canonical bus name
            match conn
                .request_name(op_plugins::canonical::BASE_SERVICE_NAME)
                .await
            {
                Ok(_) => {
                    let count = router.list_plugin_ids().await.len();
                    tracing::info!(
                        count,
                        "D-Bus plugin objects registered under org.opdbus.v1.plugins"
                    );
                }
                Err(e) => {
                    tracing::warn!(error = %e, "D-Bus name request failed");
                }
            }
        }

        // Keep connection alive (it's moved into the spawned task)
        // The connection drops when the server shuts down.
        std::future::pending::<()>().await;
    });
    // Attach the semantic shuttle for parity with run_grpc_server so
    // SearchSemanticTrace behaves identically on both endpoints (best-effort:
    // if Qdrant is unavailable the method returns failed_precondition).
    let operation_server = match op_cognitive_mcp::QdrantSemanticShuttle::new().await {
        Ok(shuttle) => operation_server.with_semantic_shuttle(Arc::new(shuttle)),
        Err(error) => {
            tracing::warn!(%error, "semantic shuttle unavailable; SearchSemanticTrace will return failed_precondition");
            operation_server
        }
    };
    match serde_json::from_value::<op_state_store::PluginSchema>(loader.get().await) {
        Ok(schema) => {
            let blob = crate::tched_router_object_blob::from_schema(schema.clone());
            tracing::info!(
                plugin_id = %blob.manifest.plugin_id,
                schema_hash = %blob.manifest.schema_hash,
                methods = blob.manifest.methods.len(),
                dbus_path = %blob.manifest.dbus.object_path,
                grpc_service = ?blob.manifest.grpc.services,
                "tched_router D-Bus/gRPC object blob frozen"
            );
            if let Err(error) = operation_server
                .register_plugin_methods(config.plugin_id.clone(), &schema)
                .await
            {
                tracing::warn!(
                    plugin_id = %config.plugin_id,
                    %error,
                    "failed to freeze tched_router method descriptors for gRPC reflection"
                );
            }
        }
        Err(error) => {
            tracing::warn!(
                plugin_id = %config.plugin_id,
                %error,
                "tched_router schema could not be parsed for gRPC reflection"
            );
        }
    }

    // Host-local Unix socket (operators / host clients).
    let unix_socket = config.unix_socket.clone();
    let unix_incoming = bind_unix_listener(&unix_socket).await?;
    info!(path = %unix_socket.display(), "tched_router native gRPC listening on Unix socket");

    // Shared container socket — same gRPC surface, path bind-mounted into CTs.
    // Skip a second bind when env points both knobs at the same path.
    let shared_socket = config.shared_socket.clone();
    let shared_incoming = if shared_socket != unix_socket {
        let incoming = bind_unix_listener(&shared_socket).await?;
        info!(
            path = %shared_socket.display(),
            "shared container.sock listening (ghostbridge UDS fabric)"
        );
        Some(incoming)
    } else {
        info!(
            path = %shared_socket.display(),
            "shared container socket coincides with host UDS; single listener"
        );
        None
    };

    // accept_http1(true): tonic-web serves gRPC-Web over HTTP/1.1, which is what
    // browsers (and xray's tls-h1 fallback forwarding to this socket) speak. Without
    // it the socket is h2-only and rejects browser gRPC-Web. Matches the TCP/axum side.
    let tonic_server = tonic::transport::Server::builder()
        .accept_http1(true)
        .layer(tonic::service::interceptor(
            crate::shared_socket::uds_identity_interceptor,
        ))
        .add_routes(build_tonic_routes(loader.clone(), operation_server.clone()))
        .serve_with_incoming(unix_incoming);

    let shared_server = shared_incoming.map(|incoming| {
        tonic::transport::Server::builder()
            .accept_http1(true)
            .layer(tonic::service::interceptor(
                crate::shared_socket::uds_identity_interceptor,
            ))
            .add_routes(build_tonic_routes(loader.clone(), operation_server.clone()))
            .serve_with_incoming(incoming)
    });

    // TLS TCP listeners for MQTT/WSS + gRPC + gRPC-Web. REST is served by
    // op-web on :8080. The Axum router is converted back into tonic Routes so
    // ServerTlsConfig protects the actual one-door surface rather than an
    // unrelated second listener.
    let bind_addrs: Vec<&str> = config
        .bind_addr
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let tls_identity = config
        .tls_identity
        .clone()
        .ok_or_else(|| anyhow::anyhow!("TLS identity is required for TCP gRPC ingress"))?;
    let mut tcp_tasks = Vec::new();
    for bind_addr_str in &bind_addrs {
        let bind_addr: SocketAddr = bind_addr_str.parse()?;
        info!(addr = %bind_addr, "tched_router MQTT/gRPC/gRPC-Web TLS demux listening on TCP");
        let app = build_axum_app_with_mqtt_socket(
            loader.clone(),
            operation_server.clone(),
            config.emqx_broker_socket.clone(),
        );
        let routes: tonic::service::Routes = app.into();
        let tls_config = ServerTlsConfig::new().identity(tls_identity.clone());
        let server = tonic::transport::Server::builder()
            .accept_http1(true)
            .tls_config(tls_config)?
            .add_routes(routes)
            .serve(bind_addr);
        tcp_tasks.push(tokio::spawn(async move { server.await }));
    }

    // Drive all listeners concurrently. Absent shared servers are no-ops.
    let shared_fut = async {
        match shared_server {
            Some(s) => s
                .await
                .map_err(|e| anyhow::anyhow!("Shared container.sock: {e}")),
            None => Ok(()),
        }
    };
    let unix_fut = async {
        tonic_server
            .await
            .map_err(|e| anyhow::anyhow!("Unix server error: {e}"))
    };
    let tcp_fut = async {
        if tcp_tasks.is_empty() {
            Ok(())
        } else {
            let (result, _idx, _rest) = futures::future::select_all(tcp_tasks).await;
            match result {
                Ok(inner) => inner.map_err(|e| anyhow::anyhow!("TCP server error: {e}")),
                Err(join_err) => Err(anyhow::anyhow!("TCP server task panicked: {join_err}")),
            }
        }
    };

    let (u, s, t) = tokio::join!(unix_fut, shared_fut, tcp_fut);
    u?;
    s?;
    t?;
    Ok(())
}

/// Bind a Unix domain socket for tonic, creating parents and replacing stale files.
async fn bind_unix_listener(
    path: &std::path::Path,
) -> anyhow::Result<tokio_stream::wrappers::UnixListenerStream> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
        // Containers mount this directory; world-executable so CT UIDs can traverse.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o755));
        }
    }
    if let Err(e) = tokio::fs::remove_file(path).await {
        if e.kind() != std::io::ErrorKind::NotFound {
            return Err(e.into());
        }
    }
    let listener = tokio::net::UnixListener::bind(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // World-readable+writable+executable: NIC-less containers (mapped UIDs) must connect.
        // Auth is enforced by the GhostBridge footprint header, not socket ACLs.
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o777));
    }
    Ok(tokio_stream::wrappers::UnixListenerStream::new(listener))
}
