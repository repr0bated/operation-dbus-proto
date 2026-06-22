//! Axum HTTP/gRPC-Web host for the zeroclaw plugin schema.
//!
//! Serves the plugin-owned schema JSON on:
//!   - native gRPC over Unix socket `/run/opdbus/zeroclaw-grpc.sock`
//!   - HTTP/1.1 + gRPC-Web on TCP `0.0.0.0:8090` (configurable via D-Bus)
//!
//! The schema itself is never generated here; it is read from
//! `/dev/shm/opdbus/schemas/zeroclaw.json` written by the zeroclaw plugin.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use async_stream::stream;
use axum::Router;
use tokio::sync::mpsc;
use tonic::{Request as TonicRequest, Response as TonicResponse, Status};
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use crate::dbus_object::{register_zeroclaw_host_object, ZeroclawAxumHostObject};
use crate::grpc_server::OperationGrpcServer;
use crate::mutation_engine::MutationEngine;
use crate::schema_loader::{SchemaLoader, SchemaReloadEvent};
use crate::tracing::{GhostbridgeTraceLayer, TraceContext};

use crate::proto::plugin_service_server::PluginServiceServer;
use crate::proto::zeroclaw::{
    zeroclaw_service_server::{ZeroclawService, ZeroclawServiceServer},
    GetSchemaRequest, SchemaEvent, SchemaResponse, WatchSchemaRequest,
};
use tonic::codegen::InterceptedService;

const DEFAULT_UNIX_SOCKET: &str = "/run/opdbus/zeroclaw-grpc.sock";
const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8090";
const DEFAULT_SCHEMA_PATH: &str = "/dev/shm/opdbus/schemas/zeroclaw.json";

/// Runtime configuration for the zeroclaw Axum host.
#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub schema_path: PathBuf,
    pub unix_socket: PathBuf,
    pub bind_addr: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            schema_path: PathBuf::from(DEFAULT_SCHEMA_PATH),
            unix_socket: PathBuf::from(DEFAULT_UNIX_SOCKET),
            bind_addr: DEFAULT_BIND_ADDR.to_string(),
        }
    }
}

impl ServerConfig {
    /// Load configuration from environment variables, falling back to defaults.
    pub fn from_env() -> Self {
        Self {
            schema_path: PathBuf::from(
                std::env::var("ZEROCLAW_SCHEMA_PATH")
                    .unwrap_or_else(|_| DEFAULT_SCHEMA_PATH.to_string()),
            ),
            unix_socket: PathBuf::from(
                std::env::var("ZEROCLAW_UNIX_SOCKET")
                    .unwrap_or_else(|_| DEFAULT_UNIX_SOCKET.to_string()),
            ),
            bind_addr: std::env::var("ZEROCLAW_BIND_ADDR")
                .unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_string()),
        }
    }
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
    if let (Ok(trace_id), Ok(footprint)) = (context.trace_id.parse(), context.footprint.parse()) {
        response
            .metadata_mut()
            .insert("x-ghostbridge-trace-id", trace_id);
        response
            .metadata_mut()
            .insert("x-ghostbridge-footprint", footprint);
    }
}

type PluginSvcIntercepted = InterceptedService<
    PluginServiceServer<OperationGrpcServer>,
    crate::interceptor::GhostbridgeInterceptor,
>;

/// Build the axum `Router` that serves the gRPC service over HTTP/gRPC-Web.
pub fn build_axum_app(loader: Arc<SchemaLoader>, plugin_service: PluginSvcIntercepted) -> Router {
    let service = ZeroclawGrpcService { loader };
    let zeroclaw_svc = tonic_web::enable(ZeroclawServiceServer::new(service));
    let plugin_svc = tonic_web::enable(plugin_service);
    let routes = tonic::service::Routes::new(zeroclaw_svc).add_service(plugin_svc);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    routes
        .into_axum_router()
        .layer(cors)
        .layer(GhostbridgeTraceLayer::new())
}

/// Build the tonic `Routes` used for the native-gRPC Unix socket side.
fn build_tonic_routes(
    loader: Arc<SchemaLoader>,
    plugin_service: PluginSvcIntercepted,
) -> tonic::service::Routes {
    let service = ZeroclawGrpcService { loader };
    let zeroclaw_svc = tonic_web::enable(ZeroclawServiceServer::new(service));
    let plugin_svc = tonic_web::enable(plugin_service);
    tonic::service::Routes::new(zeroclaw_svc).add_service(plugin_svc)
}

/// Run the zeroclaw Axum host.
///
/// Returns only when both listeners exit (which should not happen under normal
/// operation).
pub async fn run_zeroclaw_server(config: ServerConfig) -> anyhow::Result<()> {
    let loader = Arc::new(SchemaLoader::new(&config.schema_path)?);

    // Channel for D-Bus-triggered rebind requests. Only `SchemaPath` changes are
    // applied live; `BindAddr` changes are recorded and honored on the next
    // restart.
    let (rebind_tx, mut rebind_rx) = mpsc::channel::<String>(4);

    // D-Bus object for runtime configuration.
    let dbus_object = ZeroclawAxumHostObject::new(loader.clone(), rebind_tx)?;
    let _dbus_connection = register_zeroclaw_host_object(dbus_object).await?;

    // SIGHUP reload watcher.
    let _sighup_handle = loader.clone().watch_sighup();

    // PluginService (PluginService.CallMethod) backed by the authoritative
    // MutationEngine. This is how createunixsocket is invoked: a CallMethod with
    // plugin_id="unix_socket" routes through MutationEngine.mutate →
    // UnixSocketPlugin::create_unix_socket. The sled identity is resolved as
    // the actor_id inside mutate when the caller omits it.
    let event_chain = Arc::new(tokio::sync::RwLock::new(
        op_state_store::EventChain::new(op_state_store::ChainConfig::default()),
    ));
    let ovsdb = Arc::new(op_network::rovs_proxy::OvsdbDbusClient::new());
    let nonnet = Arc::new(op_jsonrpc::nonnet::NonNetDb::new());
    let mutation_engine = Arc::new(MutationEngine::new(event_chain, ovsdb, nonnet));
    let operation_server = OperationGrpcServer::new(mutation_engine);
    let plugin_service = PluginServiceServer::with_interceptor(
        operation_server,
        crate::interceptor::GhostbridgeInterceptor,
    );

    // Unix socket listener for native gRPC.
    let unix_socket = config.unix_socket.clone();
    if let Some(parent) = unix_socket.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::remove_file(&unix_socket);
    let unix_listener = tokio::net::UnixListener::bind(&unix_socket)?;
    info!(path = %unix_socket.display(), "zeroclaw native gRPC listening on Unix socket");

    let unix_incoming = tokio_stream::wrappers::UnixListenerStream::new(unix_listener);

    let tonic_server = tonic::transport::Server::builder()
        .add_routes(build_tonic_routes(loader.clone(), plugin_service.clone()))
        .serve_with_incoming(unix_incoming);

    // TCP listener for HTTP + gRPC-Web.
    let bind_addr: SocketAddr = config.bind_addr.parse()?;
    let tcp_listener = tokio::net::TcpListener::bind(bind_addr).await?;
    info!(addr = %bind_addr, "zeroclaw HTTP/gRPC-Web listening on TCP");

    let axum_app = build_axum_app(loader.clone(), plugin_service);
    let tcp_server = axum::serve(tcp_listener, axum_app);

    // Rebind task: when a new bind address is requested, restart the TCP server.
    // The current implementation starts the initial TCP server; subsequent
    // rebinds require a restart to keep the listener lifecycle simple and safe.
    let rebind_task = tokio::spawn(async move {
        while let Some(new_bind) = rebind_rx.recv().await {
            info!(new_bind = %new_bind, "D-Bus rebind request received; will be honored on next restart");
        }
    });

    let (unix_result, tcp_result) = tokio::join!(tonic_server, tcp_server);
    let _ = rebind_task.await;
    unix_result.map_err(|e| anyhow::anyhow!("Unix server error: {}", e))?;
    tcp_result.map_err(|e| anyhow::anyhow!("TCP server error: {}", e))
}
