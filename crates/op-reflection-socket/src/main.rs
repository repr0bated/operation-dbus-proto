//! op-reflection-socket
//!
//! Standalone gRPC dispatcher for the per-method typed services that
//! op-blob freezes into every sealed plugin blob (`operation.method.<plugin>.
//! <method>.<Method>Service`). Those services are already fully typed and
//! reflectable (descriptors are generated at blob-seal time by
//! `op_blob::descriptor`) but were never mounted anywhere callable — see
//! SIGNALS.md 2026-07-12 for the full trace. This process is the mount
//! point: it reads the blob catalog directly from SHM (same source
//! `op-dbus`'s own reflection hydrates from), and for each incoming call it
//! decodes the request via the blob's own descriptor, proxies the actual
//! execution to the existing, working `operation.v1.PluginService/CallMethod`
//! on op-dbus, and encodes the result back into the caller's typed shape.
//!
//! Deliberately a separate process on its own Unix socket (modeled on
//! `op-grpc-bridge::shared_socket`) rather than a change to op-dbus itself:
//! it can be built, run, and iterated on without ever restarting the daemon
//! everything else depends on.

use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Body;
use axum::http;
use bytes::{Buf, BufMut, Bytes, BytesMut};
use http_body_util::BodyExt;
use prost::Message as _;
use prost_reflect::{DescriptorPool, DynamicMessage, MessageDescriptor};
use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::{Channel, Server};
use tonic::{Request, Status};
use tower::Service;
use tracing::{error, info, warn};

#[allow(dead_code)]
mod operation {
    pub mod v1 {
        tonic::include_proto!("operation.v1");
    }
}

use operation::v1::plugin_service_client::PluginServiceClient;
use operation::v1::CallMethodRequest;

const DEFAULT_SOCKET_PATH: &str = "/run/ghostbridge/reflection.sock";
const DEFAULT_UPSTREAM: &str = "http://10.200.0.1:50051";

/// One resolved route: everything needed to decode a request, dispatch it,
/// and encode the response, keyed by the exact gRPC path the blob declared.
#[derive(Clone)]
struct RouteEntry {
    plugin_id: String,
    method_name: String,
    object_path: String,
    interface_name: String,
    required_capability: Option<String>,
    input: MessageDescriptor,
    output: MessageDescriptor,
}

#[derive(Clone)]
struct RouteTable {
    routes: Arc<HashMap<String, RouteEntry>>,
    /// Raw `FileDescriptorSet` bytes per plugin, as sealed into its blob —
    /// this is exactly what a per-plugin reflection mount needs to answer
    /// `ServerReflection` for that plugin alone.
    descriptor_sets: Arc<HashMap<String, Vec<u8>>>,
}

impl RouteTable {
    /// Load every sealed plugin blob from SHM and index its methods by
    /// their declared `grpc_path` ("/package.Service/Method"). Re-run this
    /// (a fresh RouteTable) to pick up newly sealed/reseal plugins; nothing
    /// here mutates state, so it's safe to rebuild on a timer or on demand.
    fn load() -> anyhow::Result<Self> {
        let store = op_blob::BlobStore::open_default()?;
        let mut routes = HashMap::new();
        let mut descriptor_sets = HashMap::new();

        for blob in store.plugin_object_blobs() {
            descriptor_sets.insert(blob.manifest.plugin_id.clone(), blob.descriptor_set.clone());
            let pool = match DescriptorPool::decode(blob.descriptor_set.as_slice()) {
                Ok(pool) => pool,
                Err(error) => {
                    warn!(
                        plugin_id = %blob.manifest.plugin_id,
                        %error,
                        "skipping blob: cannot decode its descriptor set"
                    );
                    continue;
                }
            };

            for method in &blob.manifest.methods {
                let (Some(input), Some(output)) = (
                    pool.get_message_by_name(&method.input_message),
                    pool.get_message_by_name(&method.output_message),
                ) else {
                    warn!(
                        plugin_id = %blob.manifest.plugin_id,
                        method = %method.schema_name,
                        "skipping method: input/output message not found in its own descriptor set"
                    );
                    continue;
                };

                routes.insert(
                    method.grpc_path.clone(),
                    RouteEntry {
                        plugin_id: blob.manifest.plugin_id.clone(),
                        method_name: method.schema_name.clone(),
                        object_path: format!("/org/opdbus/v1/plugins/{}", blob.manifest.plugin_id),
                        interface_name: "org.opdbus.v1.plugins.ProjectedObject".to_string(),
                        required_capability: method.required_capability.clone(),
                        input,
                        output,
                    },
                );
            }
        }

        info!(
            routes = routes.len(),
            "reflection route table loaded from SHM blob catalog"
        );
        Ok(Self {
            routes: Arc::new(routes),
            descriptor_sets: Arc::new(descriptor_sets),
        })
    }

    /// A view of this table restricted to one plugin's routes — what a
    /// per-plugin mount should serve, so a socket named for plugin X can
    /// never dispatch to plugin Y.
    fn filtered_by_plugin(&self, plugin_id: &str) -> Self {
        let routes = self
            .routes
            .iter()
            .filter(|(_, entry)| entry.plugin_id == plugin_id)
            .map(|(path, entry)| (path.clone(), entry.clone()))
            .collect::<HashMap<_, _>>();
        let mut descriptor_sets = HashMap::new();
        if let Some(set) = self.descriptor_sets.get(plugin_id) {
            descriptor_sets.insert(plugin_id.to_string(), set.clone());
        }
        Self {
            routes: Arc::new(routes),
            descriptor_sets: Arc::new(descriptor_sets),
        }
    }
}

const DEFAULT_MOUNT_DIR: &str = "/run/reflection";

/// Spawns a dedicated reflection+dispatch socket per plugin, the first time
/// that plugin is actually called through the main socket — "arrival
/// triggers action," not a pre-built index of all 66 plugins at startup.
/// Once mounted, `/run/reflection/<plugin_id>.sock` stays up for the life of
/// this process and answers `ServerReflection` for just that plugin.
#[derive(Clone)]
struct PluginMounter {
    mounted: Arc<tokio::sync::Mutex<std::collections::HashSet<String>>>,
    routes: RouteTable,
    upstream: Channel,
}

impl PluginMounter {
    fn new(routes: RouteTable, upstream: Channel) -> Self {
        Self {
            mounted: Arc::new(tokio::sync::Mutex::new(std::collections::HashSet::new())),
            routes,
            upstream,
        }
    }

    async fn ensure_mounted(&self, plugin_id: &str) {
        let mut mounted = self.mounted.lock().await;
        if !mounted.insert(plugin_id.to_string()) {
            return;
        }
        if let Err(error) = self.spawn_mount(plugin_id).await {
            mounted.remove(plugin_id);
            error!(plugin_id, %error, "failed to mount per-plugin reflection socket");
        }
    }

    async fn spawn_mount(&self, plugin_id: &str) -> anyhow::Result<()> {
        let descriptor_set = self
            .routes
            .descriptor_sets
            .get(plugin_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("no descriptor set for plugin {plugin_id}"))?;
        let plugin_routes = self.routes.filtered_by_plugin(plugin_id);

        let mount_dir = PathBuf::from(
            std::env::var("REFLECTION_MOUNT_DIR").unwrap_or_else(|_| DEFAULT_MOUNT_DIR.to_string()),
        );
        std::fs::create_dir_all(&mount_dir)?;
        let socket_path = mount_dir.join(format!("{plugin_id}.sock"));
        let _ = std::fs::remove_file(&socket_path);
        let listener = UnixListener::bind(&socket_path)?;
        std::os::unix::fs::chown(&socket_path, Some(1000), Some(1000))?;
        std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o660))?;

        let reflection_v1 = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(&descriptor_set)
            .build_v1()?;
        let reflection_v1alpha = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(&descriptor_set)
            .build_v1alpha()?;

        let dispatch = DispatchService {
            routes: plugin_routes,
            upstream: self.upstream.clone(),
            mounter: None,
        };
        let router = axum::Router::new().fallback_service(dispatch);
        let dispatch_routes = tonic::service::Routes::from(router);

        let incoming = UnixListenerStream::new(listener);
        let plugin_id_owned = plugin_id.to_string();
        let socket_path_owned = socket_path.clone();
        tokio::spawn(async move {
            info!(
                plugin_id = %plugin_id_owned,
                path = %socket_path_owned.display(),
                "mounted per-plugin reflection socket"
            );
            if let Err(error) = Server::builder()
                .add_routes(dispatch_routes)
                .add_service(reflection_v1)
                .add_service(reflection_v1alpha)
                .serve_with_incoming(incoming)
                .await
            {
                error!(plugin_id = %plugin_id_owned, %error, "per-plugin reflection mount exited");
            }
        });

        Ok(())
    }
}

#[derive(Clone)]
struct DispatchService {
    routes: RouteTable,
    upstream: Channel,
    /// `Some` only on the main socket's dispatcher — the per-plugin mounts
    /// it spawns are handed `None` so they don't recursively spawn further
    /// mounts for calls that arrive on their own already-scoped socket.
    mounter: Option<PluginMounter>,
}

impl Service<http::Request<Body>> for DispatchService {
    type Response = http::Response<Body>;
    type Error = std::convert::Infallible;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: http::Request<Body>) -> Self::Future {
        let this = self.clone();
        Box::pin(async move { Ok(this.dispatch(req).await) })
    }
}

impl DispatchService {
    async fn dispatch(&self, req: http::Request<Body>) -> http::Response<Body> {
        let path = req.uri().path().to_string();
        let Some(route) = self.routes.routes.get(&path) else {
            return grpc_status_response(tonic::Code::Unimplemented, "no such method");
        };

        if let Some(mounter) = &self.mounter {
            mounter.ensure_mounted(&route.plugin_id).await;
        }

        let body_bytes = match read_body(req.into_body()).await {
            Ok(bytes) => bytes,
            Err(status) => return grpc_status_response(status.code(), status.message()),
        };

        let request_msg = match decode_grpc_frame(&body_bytes) {
            Ok(payload) => payload,
            Err(status) => return grpc_status_response(status.code(), status.message()),
        };

        let dynamic_request = match DynamicMessage::decode(route.input.clone(), request_msg) {
            Ok(msg) => msg,
            Err(error) => {
                return grpc_status_response(
                    tonic::Code::InvalidArgument,
                    &format!(
                        "failed to decode request against {}: {error}",
                        route.input.full_name()
                    ),
                )
            }
        };

        let args_json = match serde_json::to_value(&dynamic_request) {
            Ok(json) => json,
            Err(error) => {
                return grpc_status_response(
                    tonic::Code::Internal,
                    &format!("failed to serialize decoded request: {error}"),
                )
            }
        };

        let call_request = CallMethodRequest {
            plugin_id: route.plugin_id.clone(),
            object_path: route.object_path.clone(),
            interface_name: route.interface_name.clone(),
            method_name: route.method_name.clone(),
            arguments: vec![json_to_prost_value(&args_json)],
            actor_id: "grpc:operation.method".to_string(),
            capability_id: route.required_capability.clone().unwrap_or_default(),
        };

        let mut client = PluginServiceClient::new(self.upstream.clone());
        let mut proxied_request = Request::new(call_request);
        inject_ghostbridge_identity(proxied_request.metadata_mut());
        let response = match client.call_method(proxied_request).await {
            Ok(response) => response.into_inner(),
            Err(status) => return grpc_status_response(status.code(), status.message()),
        };

        if !response.success {
            let message = response
                .error
                .map(|e| e.message)
                .unwrap_or_else(|| "call failed".to_string());
            return grpc_status_response(tonic::Code::Internal, &message);
        }

        let result_json = prost_value_to_json(&response.result.unwrap_or_default());

        let dynamic_response = match DynamicMessage::deserialize(route.output.clone(), result_json)
        {
            Ok(msg) => msg,
            Err(error) => {
                return grpc_status_response(
                    tonic::Code::Internal,
                    &format!(
                        "upstream result does not fit {}: {error}",
                        route.output.full_name()
                    ),
                )
            }
        };

        let mut encoded = Vec::new();
        if let Err(error) = dynamic_response.encode(&mut encoded) {
            return grpc_status_response(
                tonic::Code::Internal,
                &format!("failed to encode typed response: {error}"),
            );
        }

        grpc_unary_response(encoded)
    }
}

/// Read the gRPC-framed body: `[compressed:1][length:4 BE][message bytes]`.
async fn read_body(mut body: Body) -> Result<Bytes, Status> {
    let collected = body
        .frame()
        .await
        .transpose()
        .map_err(|_| Status::internal("failed to read request body"))?;
    let mut all = BytesMut::new();
    if let Some(frame) = collected {
        if let Ok(data) = frame.into_data() {
            all.put(data);
        }
    }
    // Some transports deliver the frame across multiple polls; drain the rest.
    while let Some(frame) = body
        .frame()
        .await
        .transpose()
        .map_err(|_| Status::internal("failed to read request body"))?
    {
        if let Ok(data) = frame.into_data() {
            all.put(data);
        }
    }
    Ok(all.freeze())
}

fn decode_grpc_frame(bytes: &Bytes) -> Result<Bytes, Status> {
    if bytes.len() < 5 {
        return Err(Status::invalid_argument(
            "request frame shorter than 5-byte gRPC header",
        ));
    }
    let mut cursor = bytes.clone();
    let compressed = cursor.get_u8();
    if compressed != 0 {
        return Err(Status::unimplemented(
            "compressed gRPC frames are not supported",
        ));
    }
    let len = cursor.get_u32() as usize;
    if cursor.remaining() < len {
        return Err(Status::invalid_argument(
            "gRPC frame length exceeds body size",
        ));
    }
    Ok(cursor.slice(0..len))
}

fn grpc_frame(payload: Vec<u8>) -> Bytes {
    let mut framed = BytesMut::with_capacity(5 + payload.len());
    framed.put_u8(0);
    framed.put_u32(payload.len() as u32);
    framed.put_slice(&payload);
    framed.freeze()
}

/// A response with a real body must carry `grpc-status` as an actual HTTP/2
/// trailer (a second HEADERS frame after the data, with END_STREAM) — not a
/// leading header. `Body::from(bytes)` sets `content-length` and has no
/// trailers support at all, which left clients waiting for a terminating
/// frame that never arrived (confirmed live: grpcurl reported "server
/// closed the stream without sending trailers"). Only the empty-body error
/// path (`grpc_status_response`) can put the status in leading headers,
/// because a headers-only h2 response IS gRPC's defined Trailers-Only case.
fn grpc_unary_response(payload: Vec<u8>) -> http::Response<Body> {
    use http_body::Frame;
    use http_body_util::{combinators::UnsyncBoxBody, BodyExt, StreamBody};

    let mut trailers = http::HeaderMap::new();
    trailers.insert("grpc-status", http::HeaderValue::from_static("0"));

    let frames = vec![
        Ok::<_, std::convert::Infallible>(Frame::data(grpc_frame(payload))),
        Ok(Frame::trailers(trailers)),
    ];
    let stream_body = StreamBody::new(tokio_stream::iter(frames));
    let boxed: UnsyncBoxBody<Bytes, std::convert::Infallible> = stream_body.boxed_unsync();

    http::Response::builder()
        .status(200)
        .header("content-type", "application/grpc")
        .body(Body::new(boxed))
        .expect("static response builder never fails")
}

fn grpc_status_response(code: tonic::Code, message: &str) -> http::Response<Body> {
    http::Response::builder()
        .status(200)
        .header("content-type", "application/grpc")
        .header("grpc-status", (code as i32).to_string())
        .header("grpc-message", message)
        .body(Body::empty())
        .expect("static response builder never fails")
}

/// Proxying to op-dbus's PluginService is itself a call that must pass the
/// Ghostbridge zero-trust gate — mirrors zcall's `read_headers()`, which
/// reads this host's own identity sled and forwards it the same way.
fn inject_ghostbridge_identity(metadata: &mut tonic::metadata::MetadataMap) {
    let Ok((sled_ptr, _mmap)) = op_identity::read_sled() else {
        warn!("no local identity sled; proxied call will be unauthenticated");
        return;
    };
    let sled = unsafe { &*sled_ptr };
    if sled.hashed_footprint == [0u8; 32] || sled.trace_id == [0u8; 16] {
        warn!("identity sled present but zeroed; proxied call will be unauthenticated");
        return;
    }
    if let Ok(value) = hex::encode(sled.hashed_footprint).parse() {
        metadata.insert("x-ghostbridge-footprint", value);
    }
    if let Ok(value) = sled.trace_id_hex().parse() {
        metadata.insert("x-ghostbridge-trace-id", value);
    }
}

fn json_to_prost_value(v: &serde_json::Value) -> prost_types::Value {
    use prost_types::value::Kind;
    let kind = match v {
        serde_json::Value::Null => Kind::NullValue(0),
        serde_json::Value::Bool(b) => Kind::BoolValue(*b),
        serde_json::Value::Number(n) => Kind::NumberValue(n.as_f64().unwrap_or(0.0)),
        serde_json::Value::String(s) => Kind::StringValue(s.clone()),
        serde_json::Value::Array(arr) => Kind::ListValue(prost_types::ListValue {
            values: arr.iter().map(json_to_prost_value).collect(),
        }),
        serde_json::Value::Object(obj) => Kind::StructValue(prost_types::Struct {
            fields: obj
                .iter()
                .map(|(k, v)| (k.clone(), json_to_prost_value(v)))
                .collect(),
        }),
    };
    prost_types::Value { kind: Some(kind) }
}

fn prost_value_to_json(v: &prost_types::Value) -> serde_json::Value {
    use prost_types::value::Kind;
    match &v.kind {
        None | Some(Kind::NullValue(_)) => serde_json::Value::Null,
        Some(Kind::BoolValue(b)) => serde_json::Value::Bool(*b),
        Some(Kind::NumberValue(n)) => serde_json::json!(n),
        Some(Kind::StringValue(s)) => serde_json::Value::String(s.clone()),
        Some(Kind::ListValue(list)) => {
            serde_json::Value::Array(list.values.iter().map(prost_value_to_json).collect())
        }
        Some(Kind::StructValue(s)) => serde_json::Value::Object(
            s.fields
                .iter()
                .map(|(k, v)| (k.clone(), prost_value_to_json(v)))
                .collect(),
        ),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let socket_path = PathBuf::from(
        std::env::var("REFLECTION_SOCKET_PATH").unwrap_or_else(|_| DEFAULT_SOCKET_PATH.to_string()),
    );
    let upstream =
        std::env::var("REFLECTION_UPSTREAM").unwrap_or_else(|_| DEFAULT_UPSTREAM.to_string());

    let routes = RouteTable::load()?;
    let channel = Channel::from_shared(upstream.clone())?.connect_lazy();

    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let _ = std::fs::remove_file(&socket_path);
    let listener = UnixListener::bind(&socket_path)?;
    std::os::unix::fs::chown(&socket_path, Some(1000), Some(1000))?;
    std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o660))?;
    info!(
        path = %socket_path.display(),
        upstream = %upstream,
        routes = routes.routes.len(),
        "op-reflection-socket listening"
    );
    let incoming = UnixListenerStream::new(listener);

    let mounter = PluginMounter::new(routes.clone(), channel.clone());
    let service = DispatchService {
        routes,
        upstream: channel,
        mounter: Some(mounter),
    };

    // DispatchService handles an unbounded set of dynamically-discovered
    // paths, so it can't implement tonic's `NamedService` (one fixed name).
    // Mount it as an axum fallback instead — matches every path not claimed
    // by another service — then hand the resulting Router to tonic's Routes
    // via `From<axum::Router>`, which erases that constraint.
    let router = axum::Router::new().fallback_service(service);
    let routes = tonic::service::Routes::from(router);

    if let Err(error) = Server::builder()
        .add_routes(routes)
        .serve_with_incoming(incoming)
        .await
    {
        error!(%error, "op-reflection-socket exited");
    }

    Ok(())
}
