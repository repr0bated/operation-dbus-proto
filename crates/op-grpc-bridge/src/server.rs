//! Axum gRPC/gRPC-Web host for the zeroclaw plugin schema.
//!
//! Serves the plugin-owned schema JSON on:
//!   - host native gRPC over Unix socket `/run/opdbus/grpc.sock`
//!   - shared container UDS `/run/ghostbridge/container.sock` (same routes)
//!   - gRPC + gRPC-Web on TCP `0.0.0.0:8090` (configurable by environment)
//!
//! The schema itself is never generated here; it is read from the plugin's
//! sealed blob in the SHM catalog (`/dev/shm/opdbus/plugin-blobs`).

use std::net::SocketAddr;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use async_stream::stream;
use axum::Router;
use tonic::transport::{Identity, ServerTlsConfig};
use tonic::{Request as TonicRequest, Response as TonicResponse, Status};
use tower_http::cors::CorsLayer;
use tracing::info;

use crate::grpc_server::{build_operation_routes_with_validator, OperationGrpcServer};
use crate::mutation_engine::MutationEngine;
use crate::oracle_assertion::{AssertionValidator, DecoyTrustStore};
use crate::schema_loader::{SchemaLoader, SchemaReloadEvent};
use crate::schema_router::SchemaRouter;
use crate::tracing::{GhostbridgeTraceLayer, TraceContext};

use crate::proto::zeroclaw::{
    zeroclaw_service_server::{ZeroclawService, ZeroclawServiceServer},
    GetSchemaRequest, SchemaEvent, SchemaResponse, WatchSchemaRequest,
};

/// Host-local native gRPC UDS (op-web, zbusctl, operators on the host).
const DEFAULT_UNIX_SOCKET: &str = "/run/opdbus/grpc.sock";
/// Shared container socket — bind-mounted into NIC-less CTs as `/run/ghostbridge`.
const DEFAULT_SHARED_SOCKET: &str = crate::shared_socket::DEFAULT_SOCKET_PATH;
const DEFAULT_SCHEMA_PLUGIN_ID: &str = "tched_router";
/// The authenticated shared TCP door. Cognitive methods are in-process routes
/// on this listener; they do not own a separate bind address.
const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8090";
/// Default schema source: the sealed blob catalog dir. When `schema_path`
/// is a directory the loader reads the plugin's own blob from it (a blob in
/// the catalog IS the plugin); a file path is still accepted for tests and
/// plugin-owned schema files.
const DEFAULT_SCHEMA_PATH: &str = op_blob::catalog::DEFAULT_SHM_DIR;

/// Runtime configuration for the zeroclaw Axum host.
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
    /// TCP bind addresses (comma-separated). **Always TLS** — zero-trust transport
    /// policy forbids plaintext gRPC on TCP, even on loopback.
    /// Set via `ZEROCLAW_BIND_ADDR` or `GRPC_BIND`; default `0.0.0.0:8090`.
    pub bind_addr: String,
    /// TLS identity for the TCP door. `None` aborts startup with a clear error
    /// unless `ZEROCLAW_DEV_SELF_SIGNED=1` is set (dev/CI only — never production).
    pub tls_identity: Option<Identity>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            plugin_id: DEFAULT_SCHEMA_PLUGIN_ID.to_string(),
            schema_path: PathBuf::from(DEFAULT_SCHEMA_PATH),
            unix_socket: PathBuf::from(DEFAULT_UNIX_SOCKET),
            shared_socket: PathBuf::from(DEFAULT_SHARED_SOCKET),
            bind_addr: DEFAULT_BIND_ADDR.to_string(),
            tls_identity: None,
        }
    }
}

impl ServerConfig {
    /// Load configuration from environment variables, falling back to defaults.
    pub fn from_env() -> Self {
        Self {
            plugin_id: configured_schema_plugin_id(|key| std::env::var(key).ok()),
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
            bind_addr: std::env::var("ZEROCLAW_BIND_ADDR")
                .or_else(|_| std::env::var("GRPC_BIND"))
                .unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_string()),
            tls_identity: Self::load_tls_identity(),
        }
    }

    /// Load TLS identity for the TCP door.
    ///
    /// Priority:
    ///   1. `ZEROCLAW_TLS_CERT` + `ZEROCLAW_TLS_KEY` env vars (PEM contents).
    ///   2. `ZEROCLAW_TLS_CERT_FILE` + `ZEROCLAW_TLS_KEY_FILE` env vars pointing
    ///      at PEM files on disk — this is what the runit run script exports
    ///      (`/etc/op-dbus/tls/tonic-svc0.crt` / `.key`). Dropping this branch is
    ///      how the TCP door has regressed repeatedly (2026-08-22, -08-23,
    ///      -08-24): the binary aborts with "TCP fabric requires TLS" and the
    ///      mesh :8090 never binds.
    ///   3. Self-signed via rcgen **only** if `ZEROCLAW_DEV_SELF_SIGNED=1` is set
    ///      (dev/CI use only — never set on production; op-web and mesh peers will
    ///      reject an unverified cert and connections will fail at the TLS handshake).
    ///   4. `None` — startup will abort with a clear error rather than silently
    ///      serving unencrypted gRPC (zero-trust: TLS is mandatory on TCP).
    fn load_tls_identity() -> Option<Identity> {
        let cert_pem = std::env::var("ZEROCLAW_TLS_CERT").ok();
        let key_pem = std::env::var("ZEROCLAW_TLS_KEY").ok();

        if let (Some(cert), Some(key)) = (cert_pem, key_pem) {
            tracing::info!("TLS identity loaded from ZEROCLAW_TLS_CERT/ZEROCLAW_TLS_KEY");
            return Some(Identity::from_pem(cert, key));
        }

        // Certificate *files* on disk (the deployment path). Both must be set;
        // a half-configured pair falls through to the loud error below instead
        // of guessing.
        let cert_file = std::env::var("ZEROCLAW_TLS_CERT_FILE").ok();
        let key_file = std::env::var("ZEROCLAW_TLS_KEY_FILE").ok();
        if let (Some(cert_path), Some(key_path)) = (cert_file, key_file) {
            match (
                std::fs::read_to_string(&cert_path),
                std::fs::read_to_string(&key_path),
            ) {
                (Ok(cert), Ok(key)) => {
                    tracing::info!(cert = %cert_path, key = %key_path,
                        "TLS identity loaded from certificate files");
                    return Some(Identity::from_pem(cert, key));
                }
                (cert_res, key_res) => {
                    tracing::error!(
                        cert_path = %cert_path,
                        key_path = %key_path,
                        cert_error = ?cert_res.err(),
                        key_error = ?key_res.err(),
                        "failed to read ZEROCLAW_TLS_CERT_FILE/ZEROCLAW_TLS_KEY_FILE"
                    );
                    return None;
                }
            }
        }

        // Dev/CI escape hatch: self-signed cert. Never set this in production.
        if std::env::var("ZEROCLAW_DEV_SELF_SIGNED").as_deref() == Ok("1") {
            match rcgen::generate_simple_self_signed(vec![
                "localhost".to_string(),
                "ghostbridge.tech".to_string(),
                "3tched.com".to_string(),
            ]) {
                Ok(ck) => {
                    tracing::warn!(
                        "ZEROCLAW_DEV_SELF_SIGNED=1: using ephemeral self-signed TLS cert. \
                         DO NOT USE IN PRODUCTION — peers will reject this cert."
                    );
                    return Some(Identity::from_pem(
                        ck.cert.pem(),
                        ck.key_pair.serialize_pem(),
                    ));
                }
                Err(e) => {
                    tracing::error!(error = %e, "failed to generate self-signed TLS cert");
                    return None;
                }
            }
        }

        tracing::error!(
            "No TLS identity configured for TCP door. \
             Set ZEROCLAW_TLS_CERT+ZEROCLAW_TLS_KEY (PEM), \
             ZEROCLAW_TLS_CERT_FILE+ZEROCLAW_TLS_KEY_FILE (paths), \
             or ZEROCLAW_DEV_SELF_SIGNED=1 (dev only). \
             TCP listeners will not start."
        );
        None
    }
}

fn configured_schema_plugin_id(getenv: impl Fn(&str) -> Option<String>) -> String {
    getenv("OP_DBUS_SCHEMA_PLUGIN_ID")
        .or_else(|| getenv("ZEROCLAW_PLUGIN_ID"))
        .unwrap_or_else(|| DEFAULT_SCHEMA_PLUGIN_ID.to_string())
}

/// Shared state for the gRPC service handlers.
#[derive(Clone)]
struct ZeroclawGrpcService {
    loader: Arc<SchemaLoader>,
}

#[tonic::async_trait]
impl ZeroclawService for ZeroclawGrpcService {
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
            footprint: context.footprint.clone(),
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
    if let Ok(footprint) = context
        .footprint
        .parse::<tonic::metadata::MetadataValue<tonic::metadata::Ascii>>()
    {
        response
            .metadata_mut()
            .insert("x-ghostbridge-genesis", footprint.clone());
        response
            .metadata_mut()
            .insert("x-ghostbridge-footprint", footprint);
    }
}

/// Assemble the gRPC service set for the zeroclaw bridge.
///
/// The full backplane surface (StateSync, PluginService, DbusPassthrough,
/// OvsdbMirror, RuntimeMirror, EventChainService, ComponentRegistry, Mail,
/// Privacy, Registration, McpService, ChatService, reflection) comes from the
/// shared [`build_operation_routes`] — identical to what `run_grpc_server` mounts
/// on op-dbus :50051, so reflection never advertises an unmounted service. On top
/// of that the zeroclaw bridge adds its endpoint-specific `ZeroclawService`
/// (plugin-owned schema get/watch).
///
/// Every container shares this one surface over `container.sock`; the assistant
/// container is provisioned exactly like the others (its socket created via the
/// unix-socket plugin's createsocket through PluginService, not a raw Incus proxy
/// device).
fn build_routes(
    loader: Arc<SchemaLoader>,
    server: OperationGrpcServer,
    validator: Arc<AssertionValidator>,
) -> tonic::service::Routes {
    let zeroclaw_svc =
        crate::grpc_web::enable(ZeroclawServiceServer::new(ZeroclawGrpcService { loader }));
    build_operation_routes_with_validator(server, validator).add_service(zeroclaw_svc)
}

/// Build the shared TCP ingress: native MCP HTTP plus gRPC and gRPC-Web.
/// The router owns no listener; production serves it through the existing
/// tonic TLS acceptor on `:8090`.
pub fn build_axum_app(loader: Arc<SchemaLoader>, server: OperationGrpcServer) -> Router {
    let validator = Arc::new(AssertionValidator::from_env(DecoyTrustStore::load()));
    build_axum_app_with_validator(loader, server, validator)
}

fn build_axum_app_with_validator(
    loader: Arc<SchemaLoader>,
    server: OperationGrpcServer,
    validator: Arc<AssertionValidator>,
) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(crate::mcp_frontend::configured_allow_origin())
        .allow_credentials(true)
        .allow_methods([axum::http::Method::POST])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            "x-grpc-web".parse().unwrap(),
            "x-user-agent".parse().unwrap(),
            "grpc-timeout".parse().unwrap(),
            crate::mcp_frontend::HTTP_ASSERTION_HEADER.parse().unwrap(),
            crate::mcp_frontend::MCP_VERSION_HEADER.parse().unwrap(),
            crate::mcp_frontend::MCP_METHOD_HEADER.parse().unwrap(),
            crate::mcp_frontend::MCP_NAME_HEADER.parse().unwrap(),
            crate::grpc_server::DECLARED_CAPABILITY_HEADER
                .parse()
                .unwrap(),
        ])
        .expose_headers([
            "grpc-status".parse().unwrap(),
            "grpc-message".parse().unwrap(),
            "grpc-status-details-bin".parse().unwrap(),
        ]);

    let engine = server.mutation_engine();
    crate::mcp_frontend::build_mcp_router(engine, validator.clone())
        .merge(build_routes(loader, server, validator).into_axum_router())
        .layer(cors)
        .layer(GhostbridgeTraceLayer::new())
}

/// Build the tonic `Routes` used for the native-gRPC Unix socket side.
fn build_tonic_routes(
    loader: Arc<SchemaLoader>,
    server: OperationGrpcServer,
    validator: Arc<AssertionValidator>,
) -> tonic::service::Routes {
    build_routes(loader, server, validator)
}

/// Run the zeroclaw Axum host.
///
/// Returns only when both listeners exit (which should not happen under normal
/// operation).
pub async fn run_zeroclaw_server(config: ServerConfig) -> anyhow::Result<()> {
    let loader = Arc::new(SchemaLoader::new_for_plugin(
        &config.schema_path,
        config.plugin_id.clone(),
    )?);

    // SIGHUP reload watcher.
    let _sighup_handle = loader.clone().watch_sighup();

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
    // One validator is shared by every projection/listener so an assertion
    // consumed through MCP cannot be replayed through gRPC (or vice versa).
    let assertion_validator = Arc::new(AssertionValidator::from_env(DecoyTrustStore::load()));
    // Open the durable audit sink and replay the pre-restart trail before any
    // surface accepts traffic, so an immediate query sees prior history.
    let replayed = mutation_engine.init_audit_durability().await;
    if replayed > 0 {
        tracing::info!(replayed, "audit trail restored from durable storage");
    }
    mutation_engine.seed_missing_plugin_projections().await?;
    let mutation_engine_for_dbus = mutation_engine.clone();
    let operation_server = OperationGrpcServer::new(mutation_engine.clone());
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
    // `run_grpc_server` (op-dbus :50051) already did this; omitting it here meant the
    // zeroclaw bridge advertised sealed plugins while serving none of their typed
    // per-method services.
    operation_server.freeze_plugin_method_reflection().await;

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
            let blob = crate::zeroclaw_object_blob::from_schema(schema.clone());
            tracing::info!(
                plugin_id = %blob.manifest.plugin_id,
                schema_hash = %blob.manifest.schema_hash,
                methods = blob.manifest.methods.len(),
                dbus_path = %blob.manifest.dbus.object_path,
                grpc_service = ?blob.manifest.grpc.services,
                "zeroclaw D-Bus/gRPC object blob frozen"
            );
            if let Err(error) = operation_server
                .register_plugin_methods(config.plugin_id.clone(), &schema)
                .await
            {
                tracing::warn!(
                    plugin_id = %config.plugin_id,
                    %error,
                    "failed to freeze zeroclaw method descriptors for gRPC reflection"
                );
            }
        }
        Err(error) => {
            tracing::warn!(
                plugin_id = %config.plugin_id,
                %error,
                "zeroclaw schema could not be parsed for gRPC reflection"
            );
        }
    }

    // Host-local Unix socket (operators / host clients).
    let unix_socket = config.unix_socket.clone();
    let unix_incoming = bind_unix_listener(&unix_socket).await?;
    info!(path = %unix_socket.display(), "zeroclaw native gRPC listening on Unix socket");

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
        .add_routes(build_tonic_routes(
            loader.clone(),
            operation_server.clone(),
            assertion_validator.clone(),
        ))
        .serve_with_incoming(unix_incoming);

    let shared_server = shared_incoming.map(|incoming| {
        tonic::transport::Server::builder()
            .accept_http1(true)
            .add_routes(build_tonic_routes(
                loader.clone(),
                operation_server.clone(),
                assertion_validator.clone(),
            ))
            .serve_with_incoming(incoming)
    });

    // TCP door: always TLS (zero-trust transport policy).
    // bind_addr is comma-separated; each address gets its own TLS listener.
    let identity = config.tls_identity.ok_or_else(|| {
        anyhow::anyhow!(
            "TCP fabric requires TLS — set ZEROCLAW_TLS_CERT+ZEROCLAW_TLS_KEY, \
             ZEROCLAW_TLS_CERT_FILE+ZEROCLAW_TLS_KEY_FILE, \
             or ZEROCLAW_DEV_SELF_SIGNED=1 (dev only)"
        )
    })?;
    let bind_addrs: Vec<&str> = config
        .bind_addr
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if bind_addrs.is_empty() {
        anyhow::bail!("ZEROCLAW_BIND_ADDR is empty — no TCP addresses to bind");
    }
    let mut tcp_tasks = Vec::new();
    for bind_addr_str in &bind_addrs {
        let bind_addr: SocketAddr = bind_addr_str.parse()?;
        info!(addr = %bind_addr, "zeroclaw TLS gRPC/gRPC-Web listening on TCP");
        let tls_config = ServerTlsConfig::new().identity(identity.clone());
        let ingress = build_axum_app_with_validator(
            loader.clone(),
            operation_server.clone(),
            assertion_validator.clone(),
        );
        let server = tonic::transport::Server::builder()
            .accept_http1(true)
            .tls_config(tls_config)
            .map_err(|e| anyhow::anyhow!("invalid TLS config for {bind_addr}: {e}"))?
            .add_routes(tonic::service::Routes::from(ingress))
            .serve(bind_addr);
        tcp_tasks.push(tokio::spawn(async move { server.await }));
    }

    // Drive all listeners concurrently.
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
        let (result, _idx, _rest) = futures::future::select_all(tcp_tasks).await;
        match result {
            Ok(inner) => inner.map_err(|e| anyhow::anyhow!("TCP TLS server error: {e}")),
            Err(join_err) => Err(anyhow::anyhow!("TCP TLS server task panicked: {join_err}")),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_server_loads_the_canonical_router_blob() {
        assert_eq!(ServerConfig::default().plugin_id, "tched_router");
    }

    #[test]
    fn env_free_server_startup_loads_the_canonical_router_blob() {
        assert_eq!(configured_schema_plugin_id(|_| None), "tched_router");
    }
}
