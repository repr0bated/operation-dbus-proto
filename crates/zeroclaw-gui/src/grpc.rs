//! Tonic + tonic-web client with **server reflection** + **dynamic unary invocation**.
//!
//! - `bootstrap`: open a Channel, hit `grpc.reflection.v1.ServerReflection`,
//!   load every returned `FileDescriptorProto` into a `prost_reflect::DescriptorPool`.
//! - `invoke_unary`: JSON in → encode via DynamicMessage → unary RPC over a
//!   bytes codec → decode response DynamicMessage → JSON out.
//! - `template_for_request`: walk a method's input MessageDescriptor and emit
//!   a JSON skeleton with zero-value fields, used to seed the Explorer editor.

use anyhow::{anyhow, Context, Result};
use bytes::{Buf, BufMut};
use prost::Message;
use prost_reflect::{
    DescriptorPool, DynamicMessage, Kind, MessageDescriptor, MethodDescriptor, ServiceDescriptor,
};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};
use tokio::task::AbortHandle;
use tonic::codec::{Codec, DecodeBuf, Decoder, EncodeBuf, Encoder};
use tonic::transport::Channel;
use tonic::{Request, Status};

/// What kind of RPC a reflected method is. The Explorer branches on this.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MethodKind {
    Unary,
    ServerStreaming,
    ClientStreaming,
    BidiStreaming,
}

impl MethodKind {
    pub fn label(self) -> &'static str {
        match self {
            MethodKind::Unary => "unary",
            MethodKind::ServerStreaming => "server-streaming",
            MethodKind::ClientStreaming => "client-streaming",
            MethodKind::BidiStreaming => "bidi-streaming",
        }
    }
}

use tonic_reflection::pb::v1::{
    server_reflection_client::ServerReflectionClient, server_reflection_request::MessageRequest,
    server_reflection_response::MessageResponse, ServerReflectionRequest,
};

#[derive(Clone, Default)]
pub struct ReflectionRegistry {
    pool: Arc<RwLock<Option<DescriptorPool>>>,
    channel: Arc<RwLock<Option<Channel>>>,
    /// Descriptor pools decoded from sealed plugin blobs, keyed by plugin id.
    /// Populated by [`ReflectionRegistry::refresh_from_blobs`]; independent of
    /// the server-reflection `pool` above, which needs a live connection.
    blob_pools: Arc<RwLock<HashMap<String, DescriptorPool>>>,
    /// `generation` of the blob catalog manifest the pools were built from.
    last_generation: Arc<RwLock<u64>>,
}

impl ReflectionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn pool(&self) -> Option<DescriptorPool> {
        self.pool.read().ok().and_then(|g| g.clone())
    }
    pub fn channel(&self) -> Option<Channel> {
        self.channel.read().ok().and_then(|g| g.clone())
    }

    pub fn services(&self) -> Vec<String> {
        let pool = match self.pool() {
            Some(p) => p,
            None => return vec![],
        };
        let svcs: Vec<_> = pool.services().collect();
        svcs.into_iter()
            .map(|s| s.full_name().to_string())
            .collect()
    }
    pub fn methods(&self, service: &str) -> Vec<String> {
        let pool = match self.pool() {
            Some(p) => p,
            None => return vec![],
        };
        let svcs: Vec<_> = pool.services().collect();
        svcs.into_iter()
            .find(|s| s.full_name() == service)
            .map(|s| s.methods().map(|m| m.name().to_string()).collect())
            .unwrap_or_default()
    }

    pub fn method_kind(&self, service: &str, method: &str) -> Option<MethodKind> {
        let pool = self.pool()?;
        let m = pool
            .services()
            .find(|s| s.full_name() == service)?
            .methods()
            .find(|m| m.name() == method)?;
        Some(match (m.is_client_streaming(), m.is_server_streaming()) {
            (false, false) => MethodKind::Unary,
            (false, true) => MethodKind::ServerStreaming,
            (true, false) => MethodKind::ClientStreaming,
            (true, true) => MethodKind::BidiStreaming,
        })
    }

    fn set_pool(&self, pool: DescriptorPool) {
        if let Ok(mut g) = self.pool.write() {
            *g = Some(pool);
        }
    }
    fn set_channel(&self, ch: Channel) {
        if let Ok(mut g) = self.channel.write() {
            *g = Some(ch);
        }
    }
}

// ----------------- blob-backed descriptor pools -----------------

/// Outcome of a [`ReflectionRegistry::refresh_from_blobs`] pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BlobRefresh {
    /// Manifest generation was unchanged; pools left alone.
    Unchanged { generation: u64 },
    /// Pools rebuilt. `plugins` counts blobs whose descriptor set decoded.
    Reloaded { generation: u64, plugins: usize },
}

impl ReflectionRegistry {
    /// Rebuild [`Self::blob_pools`] from the default SHM blob catalog.
    ///
    /// Unlike [`bootstrap`], this needs no connection: the sealed blobs carry
    /// their own `FileDescriptorSet`, so the console can populate an explorer
    /// tree before (or without) reaching a server.
    pub fn refresh_from_blobs(&self) -> Result<BlobRefresh> {
        self.refresh_from_blobs_in(Path::new(op_blob::catalog::DEFAULT_SHM_DIR))
    }

    /// `refresh_from_blobs`, against an explicit catalog directory.
    pub fn refresh_from_blobs_in(&self, dir: &Path) -> Result<BlobRefresh> {
        let generation = read_manifest_generation(dir);

        // Cheap staleness gate: the catalog bumps `generation` on every write,
        // so an unchanged value means the sealed bytes are the ones we hold.
        if let Ok(last) = self.last_generation.read() {
            if *last == generation && generation != 0 {
                return Ok(BlobRefresh::Unchanged { generation });
            }
        }

        let store = op_blob::catalog::BlobStore::open(dir)
            .with_context(|| format!("opening blob catalog at {}", dir.display()))?;

        let mut pools = HashMap::new();
        for blob in store.plugin_object_blobs() {
            let plugin_id = blob.manifest.plugin_id.clone();
            match DescriptorPool::decode(blob.descriptor_set.as_slice()) {
                Ok(pool) => {
                    pools.insert(plugin_id, pool);
                }
                Err(e) => {
                    // One malformed blob must not blank the whole explorer.
                    eprintln!("[zeroclaw] blob {plugin_id}: descriptor decode failed: {e}");
                }
            }
        }

        let plugins = pools.len();
        if let Ok(mut g) = self.blob_pools.write() {
            *g = pools;
        }
        if let Ok(mut g) = self.last_generation.write() {
            *g = generation;
        }
        Ok(BlobRefresh::Reloaded {
            generation,
            plugins,
        })
    }

    #[allow(dead_code)] // surfaced by the task 10 reflection tree
    pub fn blob_generation(&self) -> u64 {
        self.last_generation.read().map(|g| *g).unwrap_or(0)
    }

    #[allow(dead_code)] // surfaced by the task 10 reflection tree
    pub fn blob_plugin_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .blob_pools
            .read()
            .map(|g| g.keys().cloned().collect())
            .unwrap_or_default();
        ids.sort();
        ids
    }

    /// Every service across the reflection pool and all blob pools, de-duped
    /// by fully-qualified name and sorted.
    pub fn all_services(&self) -> Vec<ServiceDescriptor> {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for pool in self.pools() {
            for svc in pool.services() {
                if seen.insert(svc.full_name().to_string()) {
                    out.push(svc);
                }
            }
        }
        out.sort_by(|a, b| a.full_name().cmp(b.full_name()));
        out
    }

    /// Every method across every service, for tree population.
    #[allow(dead_code)] // populates the task 10 reflection tree
    pub fn all_methods(&self) -> Vec<MethodDescriptor> {
        let mut out: Vec<MethodDescriptor> = self
            .all_services()
            .into_iter()
            // `methods()` borrows the service, so each service's methods are
            // collected before the descriptor goes out of scope.
            .flat_map(|svc| svc.methods().collect::<Vec<_>>())
            .collect();
        out.sort_by(|a, b| a.full_name().cmp(b.full_name()));
        out
    }

    /// Resolve a method by path. Accepts `pkg.Service/Method`,
    /// `/pkg.Service/Method`, and the fully-qualified `pkg.Service.Method`.
    pub fn resolve_method(&self, path: &str) -> Option<MethodDescriptor> {
        let trimmed = path.trim_start_matches('/');
        let (service, method) = match trimmed.split_once('/') {
            Some((s, m)) => (s.to_string(), m.to_string()),
            // Fully-qualified: split the final segment off as the method name.
            None => {
                let (s, m) = trimmed.rsplit_once('.')?;
                (s.to_string(), m.to_string())
            }
        };
        self.all_services()
            .into_iter()
            .find(|s| s.full_name() == service)?
            .methods()
            .find(|m| m.name() == method)
    }

    /// Reflection pool first (it reflects what the server actually serves),
    /// then blob pools.
    fn pools(&self) -> Vec<DescriptorPool> {
        let mut pools = Vec::new();
        if let Some(pool) = self.pool() {
            pools.push(pool);
        }
        if let Ok(g) = self.blob_pools.read() {
            let mut ids: Vec<&String> = g.keys().collect();
            ids.sort();
            for id in ids {
                pools.push(g[id].clone());
            }
        }
        pools
    }
}

/// `generation` from a blob catalog `.manifest.json`, or 0 when absent.
fn read_manifest_generation(dir: &Path) -> u64 {
    std::fs::read(dir.join(".manifest.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|v| v.get("generation").and_then(Value::as_u64))
        .unwrap_or(0)
}

// ----------------- bootstrap -----------------

/// Load the full reflection pool: ListServices → FileContainingSymbol per
/// active blob-backed service → accumulate FileDescriptorSet bytes.
pub async fn bootstrap(endpoint: &str, reg: ReflectionRegistry) -> Result<()> {
    use prost_types::FileDescriptorProto;
    use tokio::sync::mpsc;
    use tonic::codegen::tokio_stream::wrappers::ReceiverStream;

    let channel: Channel = crate::conn::connect_channel(endpoint).await?;
    reg.set_channel(channel.clone());

    let mut client = ServerReflectionClient::new(channel);
    let (req_tx, req_rx) = mpsc::channel::<ServerReflectionRequest>(64);
    let outbound = ReceiverStream::new(req_rx);
    let mut inbound = client.server_reflection_info(outbound).await?.into_inner();

    req_tx
        .send(ServerReflectionRequest {
            host: String::new(),
            message_request: Some(MessageRequest::ListServices(String::new())),
        })
        .await?;

    let mut pool = DescriptorPool::new();
    let mut pending_files = 0usize;

    while let Some(msg) = inbound.message().await? {
        match msg.message_response {
            Some(MessageResponse::ListServicesResponse(list)) => {
                for svc in list.service {
                    pending_files += 1;
                    req_tx
                        .send(ServerReflectionRequest {
                            host: String::new(),
                            message_request: Some(MessageRequest::FileContainingSymbol(svc.name)),
                        })
                        .await?;
                }
                if pending_files == 0 {
                    break;
                }
            }
            Some(MessageResponse::FileDescriptorResponse(fds)) => {
                for raw in fds.file_descriptor_proto {
                    if let Ok(fdp) = FileDescriptorProto::decode(&*raw) {
                        let _ = pool.add_file_descriptor_proto(fdp);
                    }
                }
                pending_files = pending_files.saturating_sub(1);
                if pending_files == 0 {
                    break;
                }
            }
            Some(MessageResponse::ErrorResponse(e)) => {
                eprintln!("[zeroclaw] reflection bootstrap: {}", e.error_message);
            }
            _ => {}
        }
    }
    drop(req_tx);
    reg.set_pool(pool);
    Ok(())
}

// ----------------- blob-backed method resolution -----------------

/// Canonical plugin id: lowercase, hyphens → underscores (matches op-blob).
pub fn canonical_plugin_id(name: &str) -> String {
    name.to_lowercase().replace('-', "_")
}

/// Snake-case a schema method name (matches op-blob descriptor synthesis).
pub fn snake_case(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 && !out.ends_with('_') {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else if c == '-' || c == '.' {
            out.push('_');
        } else {
            out.push(c);
        }
    }
    out
}

/// PascalCase a schema method name (matches op-blob descriptor synthesis).
pub fn pascal_case(s: &str) -> String {
    let mut out = String::new();
    let mut upper = true;
    for c in s.chars() {
        if c == '_' || c == '-' || c == '.' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

/// Resolve a plugin schema method to a reflected gRPC service + RPC name.
///
/// Primary match: per-method typed services sealed in plugin blobs:
/// `operation.method.<plugin>.<snake>.<Pascal>Service` / `<Pascal>`.
pub fn resolve_plugin_method(
    reg: &ReflectionRegistry,
    plugin_id: &str,
    method_name: &str,
) -> Option<(String, String)> {
    let pool = reg.pool()?;
    let plugin = canonical_plugin_id(plugin_id);
    let snake = snake_case(method_name);
    let pascal = pascal_case(method_name);
    let expected = format!("operation.method.{plugin}.{snake}.{pascal}Service");

    for svc in pool.services() {
        let full = svc.full_name();
        if full == expected {
            for m in svc.methods() {
                if m.name() == pascal {
                    return Some((full.to_string(), pascal));
                }
            }
            return svc
                .methods()
                .next()
                .map(|m| (full.to_string(), m.name().to_string()));
        }
    }

    let prefix = format!("operation.method.{plugin}.{snake}.");
    for svc in pool.services() {
        let full = svc.full_name();
        if full.starts_with(&prefix) {
            return svc
                .methods()
                .next()
                .map(|m| (full.to_string(), m.name().to_string()));
        }
    }

    // Legacy build.rs aggregate: operation.plugin.v1.<Plugin>PluginMethods
    let legacy = format!("operation.plugin.v1.{}PluginMethods", pascal_case(&plugin));
    for svc in pool.services() {
        if svc.full_name() == legacy {
            for m in svc.methods() {
                if m.name() == pascal || snake_case(m.name()) == snake {
                    return Some((legacy.clone(), m.name().to_string()));
                }
            }
        }
    }

    None
}

/// Resolve by explicit gRPC path, subid hint, or plugin+method against reflection.
pub fn resolve_method(
    reg: &ReflectionRegistry,
    grpc_service: Option<&str>,
    grpc_method: Option<&str>,
    subid: Option<&str>,
    plugin_id: &str,
    method_name: &str,
) -> Result<(String, String)> {
    if let (Some(svc), Some(method)) = (grpc_service, grpc_method) {
        return Ok((svc.to_string(), method.to_string()));
    }

    if let Some(subid) = subid {
        if let Some(pair) = resolve_by_subid(reg, subid) {
            return Ok(pair);
        }
    }

    resolve_plugin_method(reg, plugin_id, method_name).ok_or_else(|| {
        anyhow!(
            "no reflected gRPC service for {plugin_id}.{method_name} \
             (plugin may be inactive — blob not in SHM catalog)"
        )
    })
}

/// Best-effort subid match: scan reflected method names under the plugin prefix.
fn resolve_by_subid(reg: &ReflectionRegistry, subid: &str) -> Option<(String, String)> {
    let pool = reg.pool()?;
    let verb = subid.rsplit('.').next()?.split('@').next()?;
    let snake = snake_case(verb);
    for svc in pool.services() {
        let full = svc.full_name();
        if !full.starts_with("operation.method.") {
            continue;
        }
        if !full.contains(&format!(".{snake}.")) {
            continue;
        }
        return svc
            .methods()
            .next()
            .map(|m| (full.to_string(), m.name().to_string()));
    }
    None
}

/// Build a typed request JSON body and invoke a plugin method via reflection.
pub async fn invoke_plugin_method(
    reg: &ReflectionRegistry,
    grpc_service: Option<&str>,
    grpc_method: Option<&str>,
    subid: Option<&str>,
    plugin_id: &str,
    method_name: &str,
    args: &Value,
) -> Result<Value> {
    let (service, method) = resolve_method(
        reg,
        grpc_service,
        grpc_method,
        subid,
        plugin_id,
        method_name,
    )?;

    let mut body = template_for_request(reg, &service, &method)?;
    merge_request_args(&mut body, args);

    invoke_unary(reg, &service, &method, &body).await
}

fn merge_request_args(body: &mut Value, args: &Value) {
    match args {
        Value::Object(overrides) => {
            if let Value::Object(fields) = body {
                for (k, v) in overrides {
                    fields.insert(k.clone(), v.clone());
                }
            }
        }
        Value::Array(items) if !items.is_empty() => {
            if let Value::Object(fields) = body {
                let keys: Vec<String> = fields.keys().cloned().collect();
                for (i, val) in items.iter().enumerate() {
                    if let Some(key) = keys.get(i) {
                        fields.insert(key.clone(), val.clone());
                    }
                }
            }
        }
        _ => {}
    }
}

// ----------------- decoding helpers -----------------

pub fn decode_to_json(reg: &ReflectionRegistry, msg_fqn: &str, bytes: &[u8]) -> Result<Value> {
    let pool = reg
        .pool()
        .ok_or_else(|| anyhow!("reflection pool not loaded"))?;
    let descriptor = pool
        .get_message_by_name(msg_fqn)
        .ok_or_else(|| anyhow!("unknown message: {msg_fqn}"))?;
    let dynamic = DynamicMessage::decode(descriptor, bytes)?;
    Ok(serde_json::to_value(&dynamic)?)
}

/// Walk a request message descriptor and emit a JSON skeleton (zero values).
pub fn template_for_request(
    reg: &ReflectionRegistry,
    service: &str,
    method: &str,
) -> Result<Value> {
    let pool = reg
        .pool()
        .ok_or_else(|| anyhow!("reflection pool not loaded"))?;
    let svc = pool
        .services()
        .find(|s| s.full_name() == service)
        .ok_or_else(|| anyhow!("unknown service {service}"))?;
    let m = svc
        .methods()
        .find(|m| m.name() == method)
        .ok_or_else(|| anyhow!("unknown method {service}/{method}"))?;
    Ok(skeleton_for(&m.input(), 0))
}

fn skeleton_for(desc: &MessageDescriptor, depth: usize) -> Value {
    if depth > 4 {
        return json!({});
    }
    let mut obj = Map::new();
    for field in desc.fields() {
        let v = if field.is_list() {
            json!([])
        } else if field.is_map() {
            json!({})
        } else {
            match field.kind() {
                Kind::Double | Kind::Float => json!(0.0),
                Kind::Int32
                | Kind::Sint32
                | Kind::Sfixed32
                | Kind::Int64
                | Kind::Sint64
                | Kind::Sfixed64
                | Kind::Uint32
                | Kind::Fixed32
                | Kind::Uint64
                | Kind::Fixed64 => json!(0),
                Kind::Bool => json!(false),
                Kind::String => json!(""),
                Kind::Bytes => json!(""),
                Kind::Enum(e) => json!(e
                    .values()
                    .next()
                    .map(|v| v.name().to_string())
                    .unwrap_or_default()),
                Kind::Message(m) => skeleton_for(&m, depth + 1),
            }
        };
        obj.insert(field.name().to_string(), v);
    }
    Value::Object(obj)
}

// ----------------- dynamic unary invocation -----------------

/// JSON in → encoded protobuf → unary RPC → JSON out.
pub async fn invoke_unary(
    reg: &ReflectionRegistry,
    service: &str,
    method: &str,
    request_json: &Value,
) -> Result<Value> {
    let pool = reg
        .pool()
        .ok_or_else(|| anyhow!("reflection pool not loaded"))?;
    let channel = reg.channel().ok_or_else(|| anyhow!("no active channel"))?;

    let svc = pool
        .services()
        .find(|s| s.full_name() == service)
        .ok_or_else(|| anyhow!("unknown service {service}"))?;
    let m = svc
        .methods()
        .find(|m| m.name() == method)
        .ok_or_else(|| anyhow!("unknown method {service}/{method}"))?;

    invoke_unary_on(channel, &m, request_json).await
}

/// Open a server-streaming RPC against an explicit channel and descriptor.
///
/// Returns the raw framed stream; the caller decodes each frame against
/// `m.output()`. Used by the `grpc.stream_subscribe` action handler, which
/// needs to own the read loop so it can cancel it by stream id.
pub async fn open_server_stream(
    channel: Channel,
    m: &MethodDescriptor,
    request_json: &Value,
) -> Result<tonic::Streaming<Vec<u8>>> {
    if !m.is_server_streaming() || m.is_client_streaming() {
        return Err(anyhow!(
            "{} is not server-streaming",
            m.full_name()
        ));
    }

    let req_msg = DynamicMessage::deserialize(m.input(), request_json)
        .context("request JSON does not match input schema")?;
    let req_bytes = req_msg.encode_to_vec();

    let path = format!("/{}/{}", m.parent_service().full_name(), m.name());
    let mut grpc = tonic::client::Grpc::new(channel);
    grpc.ready()
        .await
        .map_err(|e| anyhow!("channel not ready: {e}"))?;
    let mut request = Request::new(req_bytes);
    attach_ghostbridge_identity(&mut request);
    let resp = grpc
        .server_streaming(request, path.parse()?, BytesCodec)
        .await
        .map_err(|s: Status| anyhow!("gRPC {}: {}", s.code(), s.message()))?;
    Ok(resp.into_inner())
}

/// Unary invocation against an explicit channel and an already-resolved
/// method descriptor.
///
/// Split out of [`invoke_unary`] so the action bus can dial through
/// [`crate::conn::ConnectionPool`] and resolve through blob pools, neither of
/// which the registry's single `channel`/`pool` pair covers.
pub async fn invoke_unary_on(
    channel: Channel,
    m: &MethodDescriptor,
    request_json: &Value,
) -> Result<Value> {
    let service = m.parent_service().full_name().to_string();
    let method = m.name().to_string();
    if m.is_client_streaming() || m.is_server_streaming() {
        return Err(anyhow!("only unary methods are supported by the Explorer"));
    }

    let input_desc = m.input();
    let output_desc = m.output();
    let req_msg = DynamicMessage::deserialize(input_desc, request_json)
        .context("request JSON does not match input schema")?;
    let req_bytes = req_msg.encode_to_vec();

    let path = format!("/{}/{}", service, method);
    let mut grpc = tonic::client::Grpc::new(channel);
    grpc.ready()
        .await
        .map_err(|e| anyhow!("channel not ready: {e}"))?;
    let mut request = Request::new(req_bytes);
    attach_ghostbridge_identity(&mut request);
    let resp = grpc
        .unary(request, path.parse()?, BytesCodec)
        .await
        .map_err(|s: Status| anyhow!("gRPC {}: {}", s.code(), s.message()))?;

    let resp_bytes = resp.into_inner();
    let resp_msg = DynamicMessage::decode(output_desc, resp_bytes.as_slice())?;
    Ok(serde_json::to_value(&resp_msg)?)
}

// ----------------- ghostbridge identity sled -----------------

/// op-grpc-bridge rejects calls without the Ghostbridge Identity Sled headers.
/// The sled is a shared-memory blob (`/dev/shm/plugin_schema.dat`, same layout
/// `bin/zcall` reads): footprint at bytes 40..72, trace id at 72..88.
fn ghostbridge_identity() -> Option<(String, String)> {
    let path =
        std::env::var("ZCALL_SLED_PATH").unwrap_or_else(|_| "/dev/shm/plugin_schema.dat".into());
    let data = std::fs::read(path).ok()?;
    if data.len() < 88 {
        return None;
    }
    let footprint = &data[40..72];
    let trace_id = &data[72..88];
    if footprint.iter().all(|&b| b == 0) || trace_id.iter().all(|&b| b == 0) {
        return None;
    }
    let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    Some((hex(footprint), hex(trace_id)))
}

pub fn attach_ghostbridge_identity<T>(request: &mut Request<T>) {
    if let Some((footprint, trace_id)) = ghostbridge_identity() {
        let md = request.metadata_mut();
        if let Ok(v) = footprint.parse() {
            md.insert("x-ghostbridge-footprint", v);
        }
        if let Ok(v) = trace_id.parse() {
            md.insert("x-ghostbridge-trace-id", v);
        }
    }
}

// ----------------- BytesCodec: passthrough Vec<u8> in/out -----------------

#[derive(Default, Clone, Copy)]
pub struct BytesCodec;

impl Codec for BytesCodec {
    type Encode = Vec<u8>;
    type Decode = Vec<u8>;
    type Encoder = BytesCodec;
    type Decoder = BytesCodec;
    fn encoder(&mut self) -> Self::Encoder {
        *self
    }
    fn decoder(&mut self) -> Self::Decoder {
        *self
    }
}

impl Encoder for BytesCodec {
    type Item = Vec<u8>;
    type Error = Status;
    fn encode(&mut self, item: Self::Item, dst: &mut EncodeBuf<'_>) -> Result<(), Self::Error> {
        dst.put_slice(&item);
        Ok(())
    }
}

impl Decoder for BytesCodec {
    type Item = Vec<u8>;
    type Error = Status;
    fn decode(&mut self, src: &mut DecodeBuf<'_>) -> Result<Option<Self::Item>, Self::Error> {
        let n = src.remaining();
        let mut out = vec![0u8; n];
        src.copy_to_slice(&mut out);
        Ok(Some(out))
    }
}

// ----------------- background invocation handle -----------------

/// Stored on the app; the UI polls it once per frame.
#[derive(Clone, Default)]
pub struct InvokeHandle {
    inner: Arc<Mutex<InvokeState>>,
}

#[derive(Default)]
struct InvokeState {
    in_flight: bool,
    result: Option<Result<Value, String>>,
}

impl InvokeHandle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn in_flight(&self) -> bool {
        self.inner.lock().map(|g| g.in_flight).unwrap_or(false)
    }

    pub fn take_result(&self) -> Option<Result<Value, String>> {
        self.inner.lock().ok().and_then(|mut g| g.result.take())
    }

    pub fn spawn(
        &self,
        reg: ReflectionRegistry,
        service: String,
        method: String,
        body: Value,
        ctx: egui::Context,
    ) {
        {
            let mut g = self.inner.lock().unwrap();
            if g.in_flight {
                return;
            }
            g.in_flight = true;
            g.result = None;
        }
        let inner = self.inner.clone();
        tokio::spawn(async move {
            let res = invoke_unary(&reg, &service, &method, &body)
                .await
                .map_err(|e| format!("{e:#}"));
            if let Ok(mut g) = inner.lock() {
                g.in_flight = false;
                g.result = Some(res);
            }
            ctx.request_repaint();
        });
    }
}

// ----------------- request validation -----------------

/// Validate raw JSON text against a method's input descriptor.
/// Returns `Ok(())` if the JSON parses AND deserialises into a DynamicMessage
/// of the method's input type. Used for live editor feedback.
pub fn validate_request(
    reg: &ReflectionRegistry,
    service: &str,
    method: &str,
    raw: &str,
) -> Result<()> {
    let pool = reg
        .pool()
        .ok_or_else(|| anyhow!("reflection pool not loaded"))?;
    let svc = pool
        .services()
        .find(|s| s.full_name() == service)
        .ok_or_else(|| anyhow!("unknown service {service}"))?;
    let m = svc
        .methods()
        .find(|m| m.name() == method)
        .ok_or_else(|| anyhow!("unknown method {service}/{method}"))?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("request body is empty"));
    }
    let value: Value = serde_json::from_str(trimmed).map_err(|e| {
        anyhow!(
            "invalid JSON at line {} col {}: {}",
            e.line(),
            e.column(),
            e
        )
    })?;
    DynamicMessage::deserialize(m.input(), value)
        .context("JSON does not match input message schema")?;
    Ok(())
}

// ----------------- server-streaming invocation -----------------

/// Shared, frame-polled state for a single server-streaming RPC.
#[derive(Clone, Default)]
pub struct StreamHandle {
    inner: Arc<Mutex<StreamState>>,
}

#[derive(Default)]
struct StreamState {
    running: bool,
    items: Vec<Value>,
    error: Option<String>,
    closed: bool,
    abort: Option<AbortHandle>,
}

#[derive(Clone, Debug)]
pub struct StreamSnapshot {
    pub running: bool,
    pub closed: bool,
    pub error: Option<String>,
    pub items: Vec<Value>,
}

impl StreamHandle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self) -> StreamSnapshot {
        let g = self.inner.lock().unwrap();
        StreamSnapshot {
            running: g.running,
            closed: g.closed,
            error: g.error.clone(),
            items: g.items.clone(),
        }
    }

    pub fn clear(&self) {
        if let Ok(mut g) = self.inner.lock() {
            if let Some(a) = g.abort.take() {
                a.abort();
            }
            *g = StreamState::default();
        }
    }

    pub fn cancel(&self) {
        if let Ok(mut g) = self.inner.lock() {
            if let Some(a) = g.abort.take() {
                a.abort();
            }
            g.running = false;
            g.closed = true;
        }
    }

    pub fn spawn(
        &self,
        reg: ReflectionRegistry,
        service: String,
        method: String,
        body: Value,
        ctx: egui::Context,
    ) {
        {
            let mut g = self.inner.lock().unwrap();
            if g.running {
                return;
            }
            if let Some(a) = g.abort.take() {
                a.abort();
            }
            *g = StreamState {
                running: true,
                ..Default::default()
            };
        }
        let inner = self.inner.clone();
        let handle = tokio::spawn(async move {
            let res =
                run_server_streaming(reg, service, method, body, inner.clone(), ctx.clone()).await;
            if let Ok(mut g) = inner.lock() {
                g.running = false;
                g.closed = true;
                if let Err(e) = res {
                    g.error = Some(format!("{e:#}"));
                }
            }
            ctx.request_repaint();
        });
        if let Ok(mut g) = self.inner.lock() {
            g.abort = Some(handle.abort_handle());
        }
    }
}

async fn run_server_streaming(
    reg: ReflectionRegistry,
    service: String,
    method: String,
    body: Value,
    inner: Arc<Mutex<StreamState>>,
    ctx: egui::Context,
) -> Result<()> {
    let pool = reg
        .pool()
        .ok_or_else(|| anyhow!("reflection pool not loaded"))?;
    let channel = reg.channel().ok_or_else(|| anyhow!("no active channel"))?;

    let svc = pool
        .services()
        .find(|s| s.full_name() == service)
        .ok_or_else(|| anyhow!("unknown service {service}"))?;
    let m = svc
        .methods()
        .find(|m| m.name() == method)
        .ok_or_else(|| anyhow!("unknown method {service}/{method}"))?;
    if !m.is_server_streaming() || m.is_client_streaming() {
        return Err(anyhow!("method is not server-streaming"));
    }

    let input_desc = m.input();
    let output_desc = m.output();
    let req_msg = DynamicMessage::deserialize(input_desc, &body)
        .context("request JSON does not match input schema")?;
    let req_bytes = req_msg.encode_to_vec();

    let path = format!("/{}/{}", service, method);
    let mut grpc = tonic::client::Grpc::new(channel);
    grpc.ready()
        .await
        .map_err(|e| anyhow!("channel not ready: {e}"))?;
    let mut request = Request::new(req_bytes);
    attach_ghostbridge_identity(&mut request);
    let resp = grpc
        .server_streaming(request, path.parse()?, BytesCodec)
        .await
        .map_err(|s: Status| anyhow!("gRPC {}: {}", s.code(), s.message()))?;

    let mut stream = resp.into_inner();
    loop {
        match stream.message().await {
            Ok(Some(bytes)) => {
                let msg = DynamicMessage::decode(output_desc.clone(), bytes.as_slice())
                    .context("failed to decode streamed item")?;
                let val = serde_json::to_value(&msg)?;
                if let Ok(mut g) = inner.lock() {
                    g.items.push(val);
                }
                ctx.request_repaint();
            }
            Ok(None) => break,
            Err(s) => return Err(anyhow!("gRPC {}: {}", s.code(), s.message())),
        }
    }
    Ok(())
}

// ----------------- internal -----------------

fn once_stream<T: Send + 'static>(
    item: T,
) -> impl tonic::codegen::tokio_stream::Stream<Item = T> + Send + 'static {
    use tonic::codegen::tokio_stream::wrappers::ReceiverStream;
    let (tx, rx) = tokio::sync::mpsc::channel(1);
    tokio::spawn(async move {
        let _ = tx.send(item).await;
    });
    ReceiverStream::new(rx)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A catalog directory containing one real sealed blob, removed on drop.
    struct TempCatalog {
        dir: std::path::PathBuf,
    }

    impl TempCatalog {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "zeroclaw-gui-blobs-{}-{}-{}",
                tag,
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Self { dir }
        }

        fn seal_demo_plugin(&self) {
            let schema = op_blob::demo::unix_socket_schema();
            let blob = op_blob::blob::blobify(&schema);
            let mut store = op_blob::catalog::BlobStore::open(&self.dir).unwrap();
            store.write(&blob).unwrap();
        }
    }

    impl Drop for TempCatalog {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn refresh_from_blobs_lists_services_without_a_connection() {
        let catalog = TempCatalog::new("services");
        catalog.seal_demo_plugin();

        let reg = ReflectionRegistry::new();
        let refresh = reg.refresh_from_blobs_in(&catalog.dir).unwrap();

        assert!(
            matches!(refresh, BlobRefresh::Reloaded { plugins: 1, .. }),
            "unexpected refresh outcome: {refresh:?}"
        );
        assert_eq!(reg.blob_plugin_ids().len(), 1);
        // No `bootstrap` ran, so anything listed came from the sealed blob.
        assert!(reg.pool().is_none());
        assert!(
            !reg.all_services().is_empty(),
            "expected services from the blob descriptor set"
        );
        assert!(!reg.all_methods().is_empty());
    }

    #[test]
    fn resolve_method_finds_a_known_path() {
        let catalog = TempCatalog::new("resolve");
        catalog.seal_demo_plugin();
        let reg = ReflectionRegistry::new();
        reg.refresh_from_blobs_in(&catalog.dir).unwrap();

        let method = reg.all_methods().into_iter().next().unwrap();
        let service = method.parent_service().full_name().to_string();
        let name = method.name().to_string();

        // `service/Method`
        let found = reg.resolve_method(&format!("{service}/{name}")).unwrap();
        assert_eq!(found.full_name(), method.full_name());
        // Leading slash, as it appears in a gRPC path.
        let found = reg.resolve_method(&format!("/{service}/{name}")).unwrap();
        assert_eq!(found.full_name(), method.full_name());
        // Fully-qualified dotted form.
        let found = reg.resolve_method(&format!("{service}.{name}")).unwrap();
        assert_eq!(found.full_name(), method.full_name());
    }

    #[test]
    fn resolve_method_returns_none_for_unknown_paths() {
        let reg = ReflectionRegistry::new();
        assert!(reg.resolve_method("no.Such/Method").is_none());
        assert!(reg.resolve_method("garbage").is_none());
        assert!(reg.resolve_method("").is_none());
    }

    #[test]
    fn unchanged_generation_skips_the_reload() {
        let catalog = TempCatalog::new("generation");
        catalog.seal_demo_plugin();
        let reg = ReflectionRegistry::new();

        let first = reg.refresh_from_blobs_in(&catalog.dir).unwrap();
        let generation = match first {
            BlobRefresh::Reloaded { generation, .. } => generation,
            other => panic!("expected a reload, got {other:?}"),
        };
        assert!(generation > 0, "catalog should have written a generation");

        let second = reg.refresh_from_blobs_in(&catalog.dir).unwrap();
        assert_eq!(second, BlobRefresh::Unchanged { generation });
        assert_eq!(reg.blob_generation(), generation);

        // Re-sealing bumps the manifest generation, which un-gates the reload.
        catalog.seal_demo_plugin();
        let third = reg.refresh_from_blobs_in(&catalog.dir).unwrap();
        assert!(
            matches!(third, BlobRefresh::Reloaded { .. }),
            "a bumped generation must force a reload, got {third:?}"
        );
    }

    #[test]
    fn refresh_on_an_empty_directory_is_not_an_error() {
        let catalog = TempCatalog::new("empty");
        let reg = ReflectionRegistry::new();

        let refresh = reg.refresh_from_blobs_in(&catalog.dir).unwrap();

        assert!(matches!(refresh, BlobRefresh::Reloaded { plugins: 0, .. }));
        assert!(reg.all_services().is_empty());
    }
}
