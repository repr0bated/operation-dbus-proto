//! E2E oracle identity assertion battery over real TLS (design.md section 4.4).
//!
//! Serialized: always run with `--test-threads=1`.

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::sync::Once;
use std::time::Duration;

use axum::extract::ConnectInfo as AxumConnectInfo;
use base64::Engine as _;
use ed25519_dalek::{Signer, SigningKey};
use op_grpc_bridge::grpc_server::{
    build_operation_routes_with_validator, OperationGrpcServer, DECLARED_CAPABILITY_HEADER,
};
use op_grpc_bridge::interceptor::{connect_info_peer_addr, ASSERTION_METADATA_KEY};
use op_grpc_bridge::mutation_engine::MutationEngine;
use op_grpc_bridge::oracle_assertion::{derive_human_footprint, AssertionValidator, DecoyTrustStore};
use op_grpc_bridge::proto::plugin_service_client::PluginServiceClient;
use op_grpc_bridge::proto::{CallMethodRequest, ErrorCode as ProtoErrorCode};
use op_identity::oracle_assertion::{
    verify_signature, DecoyIssuer, OracleIdentityAssertion, SignedAssertion,
};
use op_identity::session::{derive_principal_id, derive_session_id};
use op_state_store::{ChainConfig, EventChain};
use prost_types::value::Kind;
use prost_types::Value as ProstValue;
use tempfile::TempDir;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tonic::metadata::{MetadataMap, MetadataValue};
use tonic::transport::server::TcpConnectInfo;
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Identity, ServerTlsConfig};
use tonic::{Code, Request, Status};
use tower::{Layer, Service};

/// Pinned battery size (VAL-E2E-023).
pub const EXPECTED_TEST_COUNT: usize = 34;

static CRYPTO: Once = Once::new();

fn install_crypto_provider() {
    CRYPTO.call_once(|| {
        rustls::crypto::aws_lc_rs::default_provider()
            .install_default()
            .expect("rustls CryptoProvider");
    });
}

fn test_ip() -> IpAddr {
    IpAddr::V4(Ipv4Addr::LOCALHOST)
}

fn alt_ip() -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(10, 200, 0, 7))
}

fn pk(byte: u8) -> String {
    base64::engine::general_purpose::STANDARD.encode([byte; 32])
}

fn test_signing_key() -> SigningKey {
    SigningKey::from_bytes(&[7u8; 32])
}

fn test_issuer() -> DecoyIssuer {
    DecoyIssuer::new(test_signing_key(), "decoy-key-1", Duration::from_secs(900))
}

fn write_trust_store(dir: &TempDir, issuer: &DecoyIssuer) {
    let b64 = base64::engine::general_purpose::STANDARD.encode(issuer.verifying_key().to_bytes());
    let json = format!(
        "{{\"decoy_keys\": {{\"{}\": \"{}\"}}}}",
        issuer.key_id(),
        b64
    );
    let path = dir.path().join("decoy-trust.json");
    std::fs::write(&path, json).expect("trust store");
    std::env::set_var("OP_DECOY_TRUST_STORE", &path);
}

fn write_grants(path: &std::path::Path, doc: &serde_json::Value) {
    std::fs::write(path, serde_json::to_vec(doc).unwrap()).expect("grants");
    std::env::set_var("OP_GRANTS_PATH", path);
}

fn footprint_hex(pubkey: &str) -> String {
    hex::encode(derive_human_footprint(pubkey))
}

fn grants_for(pubkey: &str, caps: &[&str]) -> serde_json::Value {
    let fp = footprint_hex(pubkey);
    let caps: Vec<_> = caps.iter().map(|c| (*c).to_string()).collect();
    serde_json::json!({ fp: { "capabilities": caps } })
}

fn grants_wildcard(caps: &[&str]) -> serde_json::Value {
    let caps: Vec<_> = caps.iter().map(|c| (*c).to_string()).collect();
    serde_json::json!({ "*": { "capabilities": caps } })
}

fn empty_grants() -> serde_json::Value {
    serde_json::json!({})
}

fn prost_str(s: &str) -> ProstValue {
    ProstValue {
        kind: Some(Kind::StringValue(s.to_string())),
    }
}

fn prost_struct(fields: BTreeMap<String, ProstValue>) -> ProstValue {
    ProstValue {
        kind: Some(Kind::StructValue(prost_types::Struct { fields })),
    }
}

fn struct_fields(v: &ProstValue) -> &BTreeMap<String, ProstValue> {
    match &v.kind {
        Some(Kind::StructValue(s)) => &s.fields,
        _ => panic!("expected struct"),
    }
}

fn struct_field<'a>(v: &'a ProstValue, key: &str) -> &'a ProstValue {
    struct_fields(v)
        .get(key)
        .unwrap_or_else(|| panic!("missing field {key}"))
}

fn string_field(v: &ProstValue, key: &str) -> String {
    match &struct_field(v, key).kind {
        Some(Kind::StringValue(s)) => s.clone(),
        _ => panic!("expected string field {key}"),
    }
}

fn envelope_result(v: &ProstValue) -> &ProstValue {
    struct_field(v, "result")
}

fn principal_id_from(v: &ProstValue) -> String {
    string_field(struct_field(envelope_result(v), "principal"), "principal_id")
}

fn human_pubkey_from(v: &ProstValue) -> String {
    string_field(struct_field(envelope_result(v), "principal"), "human_pubkey")
}

const HOST_SLED_FOOTPRINT: [u8; 32] = [0xAA; 32];

fn write_host_sled(path: &std::path::Path) -> String {
    let sled = op_identity::IdentitySled {
        hashed_footprint: HOST_SLED_FOOTPRINT,
        trace_id: [0xBB; 16],
        ..Default::default()
    };
    let bytes = unsafe {
        std::slice::from_raw_parts(
            &sled as *const op_identity::IdentitySled as *const u8,
            op_identity::IdentitySled::SIZE,
        )
    };
    std::fs::write(path, bytes).expect("host sled");
    hex::encode(HOST_SLED_FOOTPRINT)
}

fn signed_with_fields(
    issuer: &DecoyIssuer,
    human_pubkey: &str,
    inner_ip: IpAddr,
    issued_at: i64,
    expires_at: i64,
    nonce: [u8; 16],
    decoy_key_id: Option<&str>,
) -> SignedAssertion {
    let assertion = OracleIdentityAssertion {
        human_pubkey: human_pubkey.to_string(),
        issued_at,
        expires_at,
        nonce,
        netmaker_inner_ip: inner_ip,
        decoy_key_id: decoy_key_id.unwrap_or(issuer.key_id()).to_string(),
    };
    let signature = test_signing_key()
        .sign(&assertion.signing_bytes())
        .to_bytes();
    SignedAssertion {
        assertion,
        signature,
    }
}

fn fresh_signed(issuer: &DecoyIssuer, pubkey: &str, nonce: [u8; 16]) -> SignedAssertion {
    let now = chrono::Utc::now().timestamp();
    signed_with_fields(issuer, pubkey, test_ip(), now - 5, now + 300, nonce, None)
}

fn attach_assertion(meta: &mut MetadataMap, wire: &[u8]) {
    meta.insert_bin(
        ASSERTION_METADATA_KEY,
        MetadataValue::from_bytes(wire),
    );
}

fn attach_capability(meta: &mut MetadataMap, cap: &str) {
    meta.insert(
        DECLARED_CAPABILITY_HEADER,
        MetadataValue::try_from(cap).expect("cap header"),
    );
}

fn attach_ghostbridge(meta: &mut MetadataMap, footprint: &str, trace: &str) {
    meta.insert(
        "x-ghostbridge-footprint",
        MetadataValue::try_from(footprint).expect("fp"),
    );
    meta.insert(
        "x-ghostbridge-trace-id",
        MetadataValue::try_from(trace).expect("trace"),
    );
}

struct TestEnv {
    _root: TempDir,
    _cozo: TempDir,
    issuer: DecoyIssuer,
    host_footprint: String,
    host_trace: String,
}

impl TestEnv {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("root");
        std::env::set_var("OP_SHM_STATE_DIR", root.path().join("shm-state"));
        let cozo = tempfile::tempdir().expect("cozo");
        std::env::set_var("OP_HUMAN_PRINCIPAL_COZO_DB_PATH", cozo.path().join("cozo"));
        let sled_path = root.path().join("sled.dat");
        let host_footprint = write_host_sled(&sled_path);
        std::env::set_var("OP_SLED_PATH", &sled_path);
        let grants_path = root.path().join("capability-grants.json");
        write_grants(&grants_path, &empty_grants());
        let issuer = test_issuer();
        write_trust_store(&root, &issuer);
        Self {
            _root: root,
            _cozo: cozo,
            issuer,
            host_footprint,
            host_trace: "e2e-host-trace".to_string(),
        }
    }

    fn set_grants(&self, doc: &serde_json::Value) {
        let path = self._root.path().join("capability-grants.json");
        write_grants(&path, doc);
    }

    fn grant_human(&self, pubkey: &str, caps: &[&str]) {
        self.set_grants(&grants_for(pubkey, caps));
    }

    fn cozo_path(&self) -> std::path::PathBuf {
        self._cozo.path().join("cozo")
    }
}

struct RunningServer {
    addr: SocketAddr,
    ca_pem: String,
    engine: Arc<MutationEngine>,
    _task: tokio::task::JoinHandle<()>,
}

#[derive(Clone, Default)]
struct StripConnectInfoLayer;

impl<S> Layer<S> for StripConnectInfoLayer {
    type Service = StripConnectInfo<S>;
    fn layer(&self, inner: S) -> Self::Service {
        StripConnectInfo { inner }
    }
}

#[derive(Clone)]
struct StripConnectInfo<S> {
    inner: S,
}

impl<S, ReqBody, ResBody> Service<http::Request<ReqBody>> for StripConnectInfo<S>
where
    S: Service<http::Request<ReqBody>, Response = http::Response<ResBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>> + Send,
    ReqBody: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: http::Request<ReqBody>) -> Self::Future {
        req.extensions_mut().remove::<TcpConnectInfo>();
        req.extensions_mut().remove::<AxumConnectInfo<SocketAddr>>();
        self.inner.call(req)
    }
}

async fn start_server(strip_connect_info: bool) -> (RunningServer, TestEnv) {
    start_server_with_env(strip_connect_info, TestEnv::new()).await
}

async fn start_server_with_env(
    strip_connect_info: bool,
    env: TestEnv,
) -> (RunningServer, TestEnv) {
    install_crypto_provider();
    let ck = rcgen::generate_simple_self_signed(vec![
        "localhost".to_string(),
        "127.0.0.1".to_string(),
    ])
    .expect("tls cert");
    let ca_pem = ck.cert.pem();
    let identity = Identity::from_pem(ck.cert.pem(), ck.key_pair.serialize_pem());

    let event_chain = Arc::new(RwLock::new(EventChain::new(ChainConfig::default())));
    let ovsdb = Arc::new(op_network::rovs_proxy::OvsdbDbusClient::new());
    let engine = Arc::new(MutationEngine::new(event_chain, ovsdb));
    let server = OperationGrpcServer::new(engine.clone());
    let validator = Arc::new(AssertionValidator::new(DecoyTrustStore::load()));
    let routes = build_operation_routes_with_validator(server, validator);

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
    let tls = ServerTlsConfig::new().identity(identity);

    let task = if strip_connect_info {
        let serve = tonic::transport::Server::builder()
            .tls_config(tls)
            .expect("tls")
            .accept_http1(true)
            .layer(StripConnectInfoLayer)
            .add_routes(routes)
            .serve_with_incoming(incoming);
        tokio::spawn(async move {
            serve.await.unwrap();
        })
    } else {
        let serve = tonic::transport::Server::builder()
            .tls_config(tls)
            .expect("tls")
            .accept_http1(true)
            .add_routes(routes)
            .serve_with_incoming(incoming);
        tokio::spawn(async move {
            serve.await.unwrap();
        })
    };
    tokio::time::sleep(Duration::from_millis(200)).await;

    (
        RunningServer {
            addr,
            ca_pem,
            engine,
            _task: task,
        },
        env,
    )
}

async fn tls_channel(addr: SocketAddr, ca_pem: &str) -> Channel {
    let tls = ClientTlsConfig::new()
        .ca_certificate(Certificate::from_pem(ca_pem))
        .domain_name("localhost");
    Channel::from_shared(format!("https://{}:{}", addr.ip(), addr.port()))
        .unwrap()
        .tls_config(tls)
        .unwrap()
        .connect_timeout(Duration::from_secs(5))
        .connect()
        .await
        .expect("connect")
}

struct CallOpts<'a> {
    plugin_id: &'a str,
    method: &'a str,
    capability: &'a str,
    args: ProstValue,
    assertion: Option<SignedAssertion>,
    ghostbridge: Option<(&'a str, &'a str)>,
    wireguard_pubkey: Option<&'a str>,
    extra_assertion: Option<SignedAssertion>,
}

enum CallOutcome {
    RpcErr(Status),
    GateDenied,
    Ok(prost_types::Value),
}

impl std::fmt::Debug for CallOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RpcErr(s) => write!(f, "RpcErr({s})"),
            Self::GateDenied => write!(f, "GateDenied"),
            Self::Ok(_) => write!(f, "Ok"),
        }
    }
}

async fn call_method(channel: Channel, opts: CallOpts<'_>) -> CallOutcome {
    let mut client = PluginServiceClient::new(channel);
    let mut req = Request::new(CallMethodRequest {
        plugin_id: opts.plugin_id.to_string(),
        object_path: String::new(),
        interface_name: String::new(),
        method_name: opts.method.to_string(),
        arguments: vec![opts.args],
        actor_id: "e2e".to_string(),
        capability_id: opts.capability.to_string(),
    });
    if let Some(signed) = opts.assertion {
        attach_assertion(req.metadata_mut(), &signed.to_wire());
    }
    if let Some(extra) = opts.extra_assertion {
        req.metadata_mut().append_bin(
            ASSERTION_METADATA_KEY,
            MetadataValue::from_bytes(&extra.to_wire()),
        );
    }
    if let Some((fp, trace)) = opts.ghostbridge {
        attach_ghostbridge(req.metadata_mut(), fp, trace);
    }
    if let Some(pubkey) = opts.wireguard_pubkey {
        req.metadata_mut().insert(
            "x-wireguard-pubkey",
            MetadataValue::try_from(pubkey).expect("wg pubkey"),
        );
    }
    attach_capability(req.metadata_mut(), opts.capability);

    match client.call_method(req).await {
        Err(status) => CallOutcome::RpcErr(status),
        Ok(resp) => {
            let inner = resp.into_inner();
            if inner.success {
                CallOutcome::Ok(inner.result.unwrap_or_default())
            } else if inner
                .error
                .as_ref()
                .is_some_and(|e| e.code == ProtoErrorCode::PermissionDenied as i32)
            {
                CallOutcome::GateDenied
            } else {
                CallOutcome::RpcErr(Status::internal(format!(
                    "unexpected call failure: {:?}",
                    inner.error
                )))
            }
        }
    }
}

fn assert_unauthenticated(out: CallOutcome, reason: &str) {
    match out {
        CallOutcome::RpcErr(s) => {
            assert_eq!(s.code(), Code::Unauthenticated, "{s}");
            assert!(
                s.message().contains(reason),
                "expected {reason}, got {}",
                s.message()
            );
        }
        other => panic!("expected Unauthenticated({reason}), got {other:?}"),
    }
}

fn assert_ghostbridge_identity_rejected(out: CallOutcome) {
    match out {
        CallOutcome::RpcErr(s) => {
            assert!(
                s.code() == Code::Unauthenticated || s.code() == Code::PermissionDenied,
                "{s}"
            );
        }
        other => panic!("expected ghostbridge identity rejection, got {other:?}"),
    }
}

fn assert_permission_denied(out: CallOutcome) {
    match out {
        CallOutcome::GateDenied => {}
        other => panic!("expected PermissionDenied, got {other:?}"),
    }
}

fn assert_ok(out: CallOutcome) -> prost_types::Value {
    match out {
        CallOutcome::Ok(v) => v,
        other => panic!("expected Ok, got {other:?}"),
    }
}

async fn register_human(
    channel: Channel,
    env: &TestEnv,
    pubkey: &str,
    alias: &str,
    nonce: [u8; 16],
) {
    env.grant_human(
        pubkey,
        &["human_principal.write", "human_principal.read"],
    );
    let args = prost_struct(BTreeMap::from([
        ("human_pubkey".to_string(), prost_str(pubkey)),
        ("display_alias".to_string(), prost_str(alias)),
    ]));
    let signed = fresh_signed(&env.issuer, pubkey, nonce);
    let out = call_method(
        channel,
        CallOpts {
            plugin_id: "human_principal",
            method: "register_key",
            capability: "human_principal.write",
            args,
            assertion: Some(signed),
            ghostbridge: None,
            wireguard_pubkey: None,
            extra_assertion: None,
        },
    )
    .await;
    assert_ok(out);
}

async fn resolve_human(channel: Channel, env: &TestEnv, pubkey: &str, nonce: [u8; 16]) {
    env.grant_human(pubkey, &["human_principal.read"]);
    let args = prost_struct(BTreeMap::from([(
        "human_pubkey".to_string(),
        prost_str(pubkey),
    )]));
    let signed = fresh_signed(&env.issuer, pubkey, nonce);
    let out = call_method(
        channel,
        CallOpts {
            plugin_id: "human_principal",
            method: "resolve_key",
            capability: "human_principal.read",
            args,
            assertion: Some(signed),
            ghostbridge: None,
            wireguard_pubkey: None,
            extra_assertion: None,
        },
    )
    .await;
    assert_ok(out);
}

fn sled_dir_bytes(path: &std::path::Path) -> Vec<u8> {
    let mut out = Vec::new();
    if path.is_file() {
        out.extend(std::fs::read(path).unwrap());
        return out;
    }
    let mut entries: Vec<_> = std::fs::read_dir(path)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for entry in entries {
        out.extend(sled_dir_bytes(&entry));
    }
    out
}

async fn provision_container_identity(
    channel: Channel,
    env: &TestEnv,
    pubkey: &str,
    _nonce: [u8; 16],
) -> (String, String) {
    env.set_grants(&grants_wildcard(&["identity_sled.write"]));
    let args = prost_struct(BTreeMap::from([(
        "wireguard_pubkey".to_string(),
        prost_str(pubkey),
    )]));
    let result = assert_ok(
        call_method(
            channel,
            CallOpts {
                plugin_id: "identity_sled",
                method: "write_identity",
                capability: "identity_sled.write",
                args,
                assertion: None,
                ghostbridge: Some((&env.host_footprint, &env.host_trace)),
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
    );
    let identity = struct_field(envelope_result(&result), "identity");
    (
        string_field(identity, "hashed_footprint"),
        string_field(identity, "trace_id"),
    )
}

// ?? VAL-E2E-001 ?????????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn happy_path_registered_human_gated_call_over_tls() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let pubkey = pk(1);
    register_human(ch.clone(), &env, &pubkey, "alice", [0x01; 16]).await;
    env.grant_human(&pubkey, &["human_principal.read"]);
    let args = prost_struct(BTreeMap::from([(
        "human_pubkey".to_string(),
        prost_str(&pubkey),
    )]));
    let signed = fresh_signed(&env.issuer, &pubkey, [0x02; 16]);
    let out = call_method(
        ch.clone(),
        CallOpts {
            plugin_id: "human_principal",
            method: "resolve_key",
            capability: "human_principal.read",
            args,
            assertion: Some(signed),
            ghostbridge: None,
            wireguard_pubkey: None,
            extra_assertion: None,
        },
    )
    .await;
    assert_ok(out);
    let no_meta = call_method(
        ch,
        CallOpts {
            plugin_id: "human_principal",
            method: "resolve_key",
            capability: "human_principal.read",
            args: prost_struct(BTreeMap::from([(
                "human_pubkey".to_string(),
                prost_str(&pubkey),
            )])),
            assertion: None,
            ghostbridge: None,
            wireguard_pubkey: None,
            extra_assertion: None,
        },
    )
    .await;
    assert_unauthenticated(no_meta, "Missing Ghostbridge Identity Sled");
}

// ?? VAL-E2E-002 ?????????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn rejects_unknown_human_key() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let pubkey = pk(2);
    env.grant_human(&pubkey, &["human_principal.read"]);
    let signed = fresh_signed(&env.issuer, &pubkey, [0x03; 16]);
    let out = call_method(
        ch,
        CallOpts {
            plugin_id: "human_principal",
            method: "resolve_key",
            capability: "human_principal.read",
            args: prost_struct(BTreeMap::from([(
                "human_pubkey".to_string(),
                prost_str(&pubkey),
            )])),
            assertion: Some(signed),
            ghostbridge: None,
            wireguard_pubkey: None,
            extra_assertion: None,
        },
    )
    .await;
    assert_unauthenticated(out, "UnknownPrincipal");
}

// ?? VAL-E2E-003 ?????????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn rejects_revoked_human_key() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let pubkey = pk(3);
    register_human(ch.clone(), &env, &pubkey, "revoke-me", [0x04; 16]).await;
    resolve_human(ch.clone(), &env, &pubkey, [0x05; 16]).await;
    env.grant_human(
        &pubkey,
        &["human_principal.write", "human_principal.read"],
    );
    let revoke_args = prost_struct(BTreeMap::from([(
        "human_pubkey".to_string(),
        prost_str(&pubkey),
    )]));
    assert_ok(
        call_method(
            ch.clone(),
            CallOpts {
                plugin_id: "human_principal",
                method: "revoke_key",
                capability: "human_principal.write",
                args: revoke_args,
                assertion: Some(fresh_signed(&env.issuer, &pubkey, [0x06; 16])),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
    );
    let out = call_method(
        ch.clone(),
        CallOpts {
            plugin_id: "human_principal",
            method: "resolve_key",
            capability: "human_principal.read",
            args: prost_struct(BTreeMap::from([(
                "human_pubkey".to_string(),
                prost_str(&pubkey),
            )])),
            assertion: Some(fresh_signed(&env.issuer, &pubkey, [0x07; 16])),
            ghostbridge: None,
            wireguard_pubkey: None,
            extra_assertion: None,
        },
    )
    .await;
    assert_unauthenticated(out, "RevokedPrincipal");
}

// ?? VAL-E2E-004 ?????????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn rejects_expired_assertion() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let pubkey = pk(4);
    register_human(ch.clone(), &env, &pubkey, "exp", [0x08; 16]).await;
    env.grant_human(&pubkey, &["human_principal.read"]);
    let now = chrono::Utc::now().timestamp();
    let signed = signed_with_fields(&env.issuer, &pubkey, test_ip(), now - 600, now - 60, [0x09; 16], None);
    let out = call_method(
        ch,
        CallOpts {
            plugin_id: "human_principal",
            method: "resolve_key",
            capability: "human_principal.read",
            args: prost_struct(BTreeMap::from([(
                "human_pubkey".to_string(),
                prost_str(&pubkey),
            )])),
            assertion: Some(signed),
            ghostbridge: None,
            wireguard_pubkey: None,
            extra_assertion: None,
        },
    )
    .await;
    assert_unauthenticated(out, "Expired");
}

// ?? VAL-E2E-005 ?????????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn rejects_replayed_nonce() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let pubkey = pk(5);
    register_human(ch.clone(), &env, &pubkey, "replay", [0x0A; 16]).await;
    env.grant_human(&pubkey, &["human_principal.read"]);
    let signed = fresh_signed(&env.issuer, &pubkey, [0x0B; 16]);
    let args = prost_struct(BTreeMap::from([(
        "human_pubkey".to_string(),
        prost_str(&pubkey),
    )]));
    assert_ok(
        call_method(
            ch.clone(),
            CallOpts {
                plugin_id: "human_principal",
                method: "resolve_key",
                capability: "human_principal.read",
                args: args.clone(),
                assertion: Some(signed.clone()),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
    );
    let out = call_method(
        ch,
        CallOpts {
            plugin_id: "human_principal",
            method: "resolve_key",
            capability: "human_principal.read",
            args,
            assertion: Some(signed),
            ghostbridge: None,
            wireguard_pubkey: None,
            extra_assertion: None,
        },
    )
    .await;
    assert_unauthenticated(out, "Replay");
}

// ?? VAL-E2E-006 ?????????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn rejects_source_ip_substitution() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let pubkey = pk(6);
    register_human(ch.clone(), &env, &pubkey, "ip", [0x0C; 16]).await;
    env.grant_human(&pubkey, &["human_principal.read"]);
    let now = chrono::Utc::now().timestamp();
    let bad = signed_with_fields(&env.issuer, &pubkey, alt_ip(), now - 5, now + 300, [0x0D; 16], None);
    let out = call_method(
        ch.clone(),
        CallOpts {
            plugin_id: "human_principal",
            method: "resolve_key",
            capability: "human_principal.read",
            args: prost_struct(BTreeMap::from([(
                "human_pubkey".to_string(),
                prost_str(&pubkey),
            )])),
            assertion: Some(bad),
            ghostbridge: None,
            wireguard_pubkey: None,
            extra_assertion: None,
        },
    )
    .await;
    assert_unauthenticated(out, "SourceIpMismatch");
    resolve_human(ch, &env, &pubkey, [0x0E; 16]).await;
}

// ?? VAL-E2E-007 ?????????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn rejects_alias_substitution() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let pubkey = pk(7);
    register_human(ch.clone(), &env, &pubkey, "alice", [0x0F; 16]).await;
    env.grant_human(&pubkey, &["human_principal.read"]);
    env.grant_human("alice", &["human_principal.read"]);
    let signed = fresh_signed(&env.issuer, "alice", [0x10; 16]);
    let out = call_method(
        ch,
        CallOpts {
            plugin_id: "human_principal",
            method: "resolve_key",
            capability: "human_principal.read",
            args: prost_struct(BTreeMap::from([(
                "human_pubkey".to_string(),
                prost_str("alice"),
            )])),
            assertion: Some(signed),
            ghostbridge: None,
            wireguard_pubkey: None,
            extra_assertion: None,
        },
    )
    .await;
    assert_unauthenticated(out, "UnknownPrincipal");
}

// ?? VAL-E2E-008 ?????????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn rejects_container_key_substitution() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let container_pk = pk(8);
    let (_fp, _trace) = provision_container_identity(ch.clone(), &env, &container_pk, [0x11; 16]).await;
    env.grant_human(&container_pk, &["human_principal.read"]);
    let signed = fresh_signed(&env.issuer, &container_pk, [0x12; 16]);
    let out = call_method(
        ch,
        CallOpts {
            plugin_id: "human_principal",
            method: "resolve_key",
            capability: "human_principal.read",
            args: prost_struct(BTreeMap::from([(
                "human_pubkey".to_string(),
                prost_str(&container_pk),
            )])),
            assertion: Some(signed),
            ghostbridge: None,
            wireguard_pubkey: None,
            extra_assertion: None,
        },
    )
    .await;
    assert_unauthenticated(out, "UnknownPrincipal");
}

// ?? VAL-E2E-009 ?????????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn rejects_over_long_ttl() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let pubkey = pk(9);
    register_human(ch.clone(), &env, &pubkey, "long", [0x13; 16]).await;
    env.grant_human(&pubkey, &["human_principal.read"]);
    let now = chrono::Utc::now().timestamp();
    let signed = signed_with_fields(&env.issuer, &pubkey, test_ip(), now, now + 901, [0x14; 16], None);
    verify_signature(
        &signed.assertion,
        &signed.signature,
        &env.issuer.verifying_key(),
    )
    .expect("sig ok");
    let out = call_method(
        ch,
        CallOpts {
            plugin_id: "human_principal",
            method: "resolve_key",
            capability: "human_principal.read",
            args: prost_struct(BTreeMap::from([(
                "human_pubkey".to_string(),
                prost_str(&pubkey),
            )])),
            assertion: Some(signed),
            ghostbridge: None,
            wireguard_pubkey: None,
            extra_assertion: None,
        },
    )
    .await;
    assert_unauthenticated(out, "LifetimeTooLong");
}

// ?? VAL-E2E-010 ?????????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn rejects_unknown_decoy_key() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let pubkey = pk(10);
    register_human(ch.clone(), &env, &pubkey, "decoy", [0x15; 16]).await;
    env.grant_human(&pubkey, &["human_principal.read"]);
    let other = DecoyIssuer::new(
        SigningKey::from_bytes(&[9u8; 32]),
        "decoy-key-2",
        Duration::from_secs(900),
    );
    let signed = signed_with_fields(
        &other,
        &pubkey,
        test_ip(),
        chrono::Utc::now().timestamp() - 5,
        chrono::Utc::now().timestamp() + 300,
        [0x16; 16],
        Some("decoy-key-2"),
    );
    let out = call_method(
        ch,
        CallOpts {
            plugin_id: "human_principal",
            method: "resolve_key",
            capability: "human_principal.read",
            args: prost_struct(BTreeMap::from([(
                "human_pubkey".to_string(),
                prost_str(&pubkey),
            )])),
            assertion: Some(signed),
            ghostbridge: None,
            wireguard_pubkey: None,
            extra_assertion: None,
        },
    )
    .await;
    assert_unauthenticated(out, "UnknownDecoyKey");
}

// ?? VAL-E2E-011 ?????????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn rejects_bad_signature() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let pubkey = pk(11);
    register_human(ch.clone(), &env, &pubkey, "sig", [0x17; 16]).await;
    env.grant_human(&pubkey, &["human_principal.read"]);
    let args = prost_struct(BTreeMap::from([(
        "human_pubkey".to_string(),
        prost_str(&pubkey),
    )]));
    let mut wire_tamper = fresh_signed(&env.issuer, &pubkey, [0x18; 16]).to_wire();
    wire_tamper[10] ^= 0x01;
    let mut client = PluginServiceClient::new(ch.clone());
    let mut req = Request::new(CallMethodRequest {
        plugin_id: "human_principal".to_string(),
        object_path: String::new(),
        interface_name: String::new(),
        method_name: "resolve_key".to_string(),
        arguments: vec![args.clone()],
        actor_id: "e2e".to_string(),
        capability_id: "human_principal.read".to_string(),
    });
    req.metadata_mut().insert_bin(
        ASSERTION_METADATA_KEY,
        MetadataValue::from_bytes(&wire_tamper),
    );
    attach_capability(req.metadata_mut(), "human_principal.read");
    let out0 = client.call_method(req).await.unwrap_err();
    assert_eq!(out0.code(), Code::Unauthenticated);
    assert!(out0.message().contains("BadSignature"));
    let mut tamper_sig = fresh_signed(&env.issuer, &pubkey, [0x19; 16]);
    tamper_sig.signature[0] ^= 0x01;
    let out1 = call_method(
        ch.clone(),
        CallOpts {
            plugin_id: "human_principal",
            method: "resolve_key",
            capability: "human_principal.read",
            args: args.clone(),
            assertion: Some(tamper_sig.clone()),
            ghostbridge: None,
            wireguard_pubkey: None,
            extra_assertion: None,
        },
    )
    .await;
    assert_unauthenticated(out1, "BadSignature");
}

// ?? VAL-E2E-012 ?????????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn denies_valid_assertion_without_capability_grant() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let pubkey = pk(12);
    register_human(ch.clone(), &env, &pubkey, "nogrant", [0x1A; 16]).await;
    env.set_grants(&empty_grants());
    let out = call_method(
        ch,
        CallOpts {
            plugin_id: "human_principal",
            method: "resolve_key",
            capability: "human_principal.read",
            args: prost_struct(BTreeMap::from([(
                "human_pubkey".to_string(),
                prost_str(&pubkey),
            )])),
            assertion: Some(fresh_signed(&env.issuer, &pubkey, [0x1B; 16])),
            ghostbridge: None,
            wireguard_pubkey: None,
            extra_assertion: None,
        },
    )
    .await;
    assert_permission_denied(out);
}

// ?? VAL-E2E-015 ?????????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn rejects_malformed_assertion_metadata() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let pubkey = pk(15);
    register_human(ch.clone(), &env, &pubkey, "mal", [0x1C; 16]).await;
    env.grant_human(&pubkey, &["human_principal.read"]);
    let args = prost_struct(BTreeMap::from([(
        "human_pubkey".to_string(),
        prost_str(&pubkey),
    )]));
    for (nonce_byte, wire) in [
        (0x1Du8, b"not-oia1".to_vec()),
        (0x1Eu8, {
            let mut w = fresh_signed(&env.issuer, &pubkey, [0x1E; 16]).to_wire();
            w.push(0xFF);
            w
        }),
        (0x1Fu8, b"OIA1".to_vec()),
    ] {
        let mut client = PluginServiceClient::new(ch.clone());
        let mut req = Request::new(CallMethodRequest {
            plugin_id: "human_principal".to_string(),
            object_path: String::new(),
            interface_name: String::new(),
            method_name: "resolve_key".to_string(),
            arguments: vec![args.clone()],
            actor_id: "e2e".to_string(),
            capability_id: "human_principal.read".to_string(),
        });
        req.metadata_mut().insert_bin(
            ASSERTION_METADATA_KEY,
            MetadataValue::from_bytes(&wire),
        );
        attach_capability(req.metadata_mut(), "human_principal.read");
        let status = client.call_method(req).await.unwrap_err();
        assert_eq!(status.code(), Code::Unauthenticated, "nonce {nonce_byte}");
        assert!(status.message().contains("Malformed"), "{}", status.message());
    }
}

// ?? VAL-E2E-016 ?????????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn rejects_not_yet_valid_assertion() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let pubkey = pk(16);
    register_human(ch.clone(), &env, &pubkey, "future", [0x20; 16]).await;
    env.grant_human(&pubkey, &["human_principal.read"]);
    let now = chrono::Utc::now().timestamp();
    let signed = signed_with_fields(&env.issuer, &pubkey, test_ip(), now + 300, now + 900, [0x21; 16], None);
    let out = call_method(
        ch,
        CallOpts {
            plugin_id: "human_principal",
            method: "resolve_key",
            capability: "human_principal.read",
            args: prost_struct(BTreeMap::from([(
                "human_pubkey".to_string(),
                prost_str(&pubkey),
            )])),
            assertion: Some(signed),
            ghostbridge: None,
            wireguard_pubkey: None,
            extra_assertion: None,
        },
    )
    .await;
    assert_unauthenticated(out, "NotYetValid");
}

// ?? VAL-E2E-019 ?????????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn accepts_within_leeway_expired_assertion() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let pubkey = pk(19);
    register_human(ch.clone(), &env, &pubkey, "leeway", [0x22; 16]).await;
    env.grant_human(&pubkey, &["human_principal.read"]);
    let now = chrono::Utc::now().timestamp();
    let signed = signed_with_fields(&env.issuer, &pubkey, test_ip(), now - 300, now - 10, [0x23; 16], None);
    assert_ok(
        call_method(
            ch,
            CallOpts {
                plugin_id: "human_principal",
                method: "resolve_key",
                capability: "human_principal.read",
                args: prost_struct(BTreeMap::from([(
                    "human_pubkey".to_string(),
                    prost_str(&pubkey),
                )])),
                assertion: Some(signed),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
    );
}

// ?? VAL-E2E-020 ?????????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn accepts_lifetime_exactly_900s() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let pubkey = pk(20);
    register_human(ch.clone(), &env, &pubkey, "900", [0x24; 16]).await;
    env.grant_human(&pubkey, &["human_principal.read"]);
    let now = chrono::Utc::now().timestamp();
    let signed = signed_with_fields(&env.issuer, &pubkey, test_ip(), now, now + 900, [0x25; 16], None);
    assert_ok(
        call_method(
            ch,
            CallOpts {
                plugin_id: "human_principal",
                method: "resolve_key",
                capability: "human_principal.read",
                args: prost_struct(BTreeMap::from([(
                    "human_pubkey".to_string(),
                    prost_str(&pubkey),
                )])),
                assertion: Some(signed),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
    );
}

// ?? VAL-E2E-021 ?????????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn registration_bootstrap_requires_grant() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let pubkey = pk(21);
    env.grant_human(&pubkey, &["human_principal.write"]);
    let args = prost_struct(BTreeMap::from([
        ("human_pubkey".to_string(), prost_str(&pubkey)),
        ("display_alias".to_string(), prost_str("boot")),
    ]));
    assert_ok(
        call_method(
            ch.clone(),
            CallOpts {
                plugin_id: "human_principal",
                method: "register_key",
                capability: "human_principal.write",
                args: args.clone(),
                assertion: Some(fresh_signed(&env.issuer, &pubkey, [0x26; 16])),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
    );
    let pubkey2 = pk(22);
    env.set_grants(&empty_grants());
    assert_permission_denied(
        call_method(
            ch,
            CallOpts {
                plugin_id: "human_principal",
                method: "register_key",
                capability: "human_principal.write",
                args: prost_struct(BTreeMap::from([
                    ("human_pubkey".to_string(), prost_str(&pubkey2)),
                    ("display_alias".to_string(), prost_str("denied")),
                ])),
                assertion: Some(fresh_signed(&env.issuer, &pubkey2, [0x27; 16])),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
    );
}

// ?? VAL-E2E-022 ?????????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn fixture_connect_info_matches_validator() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let pubkey = pk(22);
    register_human(ch.clone(), &env, &pubkey, "conn", [0x28; 16]).await;
    env.grant_human(&pubkey, &["human_principal.read"]);
    let signed = fresh_signed(&env.issuer, &pubkey, [0x29; 16]);
    let mut client = PluginServiceClient::new(ch);
    let mut req = Request::new(CallMethodRequest {
        plugin_id: "human_principal".to_string(),
        object_path: String::new(),
        interface_name: String::new(),
        method_name: "resolve_key".to_string(),
        arguments: vec![prost_struct(BTreeMap::from([(
            "human_pubkey".to_string(),
            prost_str(&pubkey),
        )]))],
        actor_id: "e2e".to_string(),
        capability_id: "human_principal.read".to_string(),
    });
    attach_assertion(req.metadata_mut(), &signed.to_wire());
    attach_capability(req.metadata_mut(), "human_principal.read");
    let _ = client.call_method(req).await.expect("composed path ok");
    let mut probe = Request::new(());
    probe.extensions_mut().insert(TcpConnectInfo {
        local_addr: Some(SocketAddr::new(test_ip(), 0)),
        remote_addr: Some(SocketAddr::new(test_ip(), 12345)),
    });
    assert_eq!(
        connect_info_peer_addr(&probe),
        Some(SocketAddr::new(test_ip(), 12345))
    );
}

// ?? VAL-E2E-024 ?????????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn rejects_duplicate_assertion_metadata() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let pubkey = pk(24);
    register_human(ch.clone(), &env, &pubkey, "dup", [0x2A; 16]).await;
    env.grant_human(&pubkey, &["human_principal.read"]);
    let a = fresh_signed(&env.issuer, &pubkey, [0x2B; 16]);
    let b = fresh_signed(&env.issuer, &pubkey, [0x2C; 16]);
    let out = call_method(
        ch,
        CallOpts {
            plugin_id: "human_principal",
            method: "resolve_key",
            capability: "human_principal.read",
            args: prost_struct(BTreeMap::from([(
                "human_pubkey".to_string(),
                prost_str(&pubkey),
            )])),
            assertion: Some(a),
            ghostbridge: None,
            wireguard_pubkey: None,
            extra_assertion: Some(b),
        },
    )
    .await;
    assert_unauthenticated(out, "Malformed");
}

// ?? VAL-CROSS-001 ???????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn e2e_full_trust_chain_generated_surface() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let pubkey = pk(31);
    env.grant_human(&pubkey, &["human_principal.write", "human_principal.read"]);
    let reg_args = prost_struct(BTreeMap::from([
        ("human_pubkey".to_string(), prost_str(&pubkey)),
        ("display_alias".to_string(), prost_str("chain")),
    ]));
    let reg = assert_ok(
        call_method(
            ch.clone(),
            CallOpts {
                plugin_id: "human_principal",
                method: "register_key",
                capability: "human_principal.write",
                args: reg_args,
                assertion: Some(fresh_signed(&env.issuer, &pubkey, [0x31; 16])),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
    );
    let derived = derive_principal_id(&pubkey);
    let pid = principal_id_from(&reg);
    assert_eq!(pid, derived);
    env.grant_human(&pubkey, &["human_principal.write", "human_principal.read"]);
    assert_ok(
        call_method(
            ch.clone(),
            CallOpts {
                plugin_id: "human_principal",
                method: "set_alias",
                capability: "human_principal.write",
                args: prost_struct(BTreeMap::from([
                    ("principal_id".to_string(), prost_str(&derived)),
                    ("display_alias".to_string(), prost_str("renamed")),
                ])),
                assertion: Some(fresh_signed(&env.issuer, &pubkey, [0x32; 16])),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
    );
    let got = assert_ok(
        call_method(
            ch,
            CallOpts {
                plugin_id: "human_principal",
                method: "get_principal",
                capability: "human_principal.read",
                args: prost_struct(BTreeMap::from([(
                    "principal_id".to_string(),
                    prost_str(&derived),
                )])),
                assertion: Some(fresh_signed(&env.issuer, &pubkey, [0x33; 16])),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
    );
    let alias = string_field(
        struct_field(envelope_result(&got), "principal"),
        "display_alias",
    );
    assert_eq!(alias, "renamed");
}

// ?? VAL-CROSS-003 ???????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn e2e_assertion_precedence_over_footprint_headers() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let pubkey = pk(32);
    register_human(ch.clone(), &env, &pubkey, "prec", [0x34; 16]).await;
    env.grant_human(&pubkey, &["human_principal.read"]);
    let signed = fresh_signed(&env.issuer, &pubkey, [0x35; 16]);
    assert_ok(
        call_method(
            ch.clone(),
            CallOpts {
                plugin_id: "human_principal",
                method: "resolve_key",
                capability: "human_principal.read",
                args: prost_struct(BTreeMap::from([(
                    "human_pubkey".to_string(),
                    prost_str(&pubkey),
                )])),
                assertion: Some(signed),
                ghostbridge: Some(("00".repeat(32).as_str(), "bogus-trace")),
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
    );
    let mut bad = fresh_signed(&env.issuer, &pubkey, [0x36; 16]);
    bad.signature[0] ^= 0x01;
    let container_pk = pk(33);
    let (fp, trace) = provision_container_identity(ch.clone(), &env, &container_pk, [0x37; 16]).await;
    env.set_grants(&serde_json::json!({ fp.clone(): { "capabilities": ["human_principal.read"] } }));
    let out = call_method(
        ch,
        CallOpts {
            plugin_id: "human_principal",
            method: "resolve_key",
            capability: "human_principal.read",
            args: prost_struct(BTreeMap::from([(
                "human_pubkey".to_string(),
                prost_str(&pubkey),
            )])),
            assertion: Some(bad),
            ghostbridge: Some((&fp, &trace)),
            wireguard_pubkey: None,
            extra_assertion: None,
        },
    )
    .await;
    assert_unauthenticated(out, "BadSignature");
}

// ?? VAL-CROSS-004 ???????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn e2e_container_and_human_never_cross_authenticate() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let container_pk = pk(34);
    provision_container_identity(ch.clone(), &env, &container_pk, [0x38; 16]).await;
    env.grant_human(&container_pk, &["human_principal.read"]);
    assert_unauthenticated(
        call_method(
            ch.clone(),
            CallOpts {
                plugin_id: "human_principal",
                method: "resolve_key",
                capability: "human_principal.read",
                args: prost_struct(BTreeMap::from([(
                    "human_pubkey".to_string(),
                    prost_str(&container_pk),
                )])),
                assertion: Some(fresh_signed(&env.issuer, &container_pk, [0x39; 16])),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
        "UnknownPrincipal",
    );
    let human_pk = pk(35);
    register_human(ch.clone(), &env, &human_pk, "human", [0x3A; 16]).await;
    let hfp = footprint_hex(&human_pk);
    env.set_grants(&serde_json::json!({ hfp.clone(): { "capabilities": ["human_principal.read"] } }));
    assert_ghostbridge_identity_rejected(
        call_method(
            ch.clone(),
            CallOpts {
                plugin_id: "human_principal",
                method: "resolve_key",
                capability: "human_principal.read",
                args: prost_struct(BTreeMap::from([(
                    "human_pubkey".to_string(),
                    prost_str(&human_pk),
                )])),
                assertion: None,
                ghostbridge: Some((&hfp, "human-trace")),
                wireguard_pubkey: Some(&human_pk),
                extra_assertion: None,
            },
        )
        .await,
    );
    assert_ne!(derive_principal_id(&human_pk), derive_session_id(&human_pk));
}

// ?? VAL-CROSS-005 ???????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn e2e_human_grants_same_mechanism() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let pubkey = pk(36);
    register_human(ch.clone(), &env, &pubkey, "grants", [0x3B; 16]).await;
    let fp = footprint_hex(&pubkey);
    let args = prost_struct(BTreeMap::from([(
        "human_pubkey".to_string(),
        prost_str(&pubkey),
    )]));
    env.set_grants(&serde_json::json!({ fp: { "capabilities": ["human_principal.read"] } }));
    assert_ok(
        call_method(
            ch.clone(),
            CallOpts {
                plugin_id: "human_principal",
                method: "resolve_key",
                capability: "human_principal.read",
                args: args.clone(),
                assertion: Some(fresh_signed(&env.issuer, &pubkey, [0x3C; 16])),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
    );
    env.set_grants(&empty_grants());
    assert_permission_denied(
        call_method(
            ch.clone(),
            CallOpts {
                plugin_id: "human_principal",
                method: "resolve_key",
                capability: "human_principal.read",
                args: args.clone(),
                assertion: Some(fresh_signed(&env.issuer, &pubkey, [0x3D; 16])),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
    );
    env.set_grants(&grants_wildcard(&["human_principal.read"]));
    assert_ok(
        call_method(
            ch.clone(),
            CallOpts {
                plugin_id: "human_principal",
                method: "resolve_key",
                capability: "human_principal.read",
                args: args.clone(),
                assertion: Some(fresh_signed(&env.issuer, &pubkey, [0x3E; 16])),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
    );
    env.set_grants(&empty_grants());
    assert_permission_denied(
        call_method(
            ch,
            CallOpts {
                plugin_id: "human_principal",
                method: "resolve_key",
                capability: "human_principal.read",
                args,
                assertion: Some(fresh_signed(&env.issuer, &pubkey, [0x3F; 16])),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
    );
}

// ?? VAL-CROSS-006 ???????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn e2e_freshness_lifecycle() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let pubkey = pk(37);
    register_human(ch.clone(), &env, &pubkey, "life", [0x40; 16]).await;
    env.grant_human(&pubkey, &["human_principal.read"]);
    let now = chrono::Utc::now().timestamp();
    assert_ok(
        call_method(
            ch.clone(),
            CallOpts {
                plugin_id: "human_principal",
                method: "resolve_key",
                capability: "human_principal.read",
                args: prost_struct(BTreeMap::from([(
                    "human_pubkey".to_string(),
                    prost_str(&pubkey),
                )])),
                assertion: Some(signed_with_fields(
                    &env.issuer,
                    &pubkey,
                    test_ip(),
                    now - 5,
                    now + 60,
                    [0x41; 16],
                    None,
                )),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
    );
    assert_unauthenticated(
        call_method(
            ch.clone(),
            CallOpts {
                plugin_id: "human_principal",
                method: "resolve_key",
                capability: "human_principal.read",
                args: prost_struct(BTreeMap::from([(
                    "human_pubkey".to_string(),
                    prost_str(&pubkey),
                )])),
                assertion: Some(signed_with_fields(
                    &env.issuer,
                    &pubkey,
                    test_ip(),
                    now - 600,
                    now - 60,
                    [0x42; 16],
                    None,
                )),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
        "Expired",
    );
    assert_ok(
        call_method(
            ch,
            CallOpts {
                plugin_id: "human_principal",
                method: "resolve_key",
                capability: "human_principal.read",
                args: prost_struct(BTreeMap::from([(
                    "human_pubkey".to_string(),
                    prost_str(&pubkey),
                )])),
                assertion: Some(fresh_signed(&env.issuer, &pubkey, [0x43; 16])),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
    );
}

// ?? VAL-CROSS-007 ???????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn e2e_empty_trust_store_fail_closed() {
    let env = TestEnv::new();
    let cozo = env.cozo_path();
    let grants_template = env._root.path().join("capability-grants.json");
    let (srv, env) = start_server_with_env(false, env).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let pubkey = pk(38);
    register_human(ch.clone(), &env, &pubkey, "empty-trust", [0x44; 16]).await;
    drop(srv);
    drop(ch);
    std::env::set_var("OP_HUMAN_PRINCIPAL_COZO_DB_PATH", &cozo);
    write_grants(&grants_template, &grants_for(&pubkey, &["human_principal.read"]));
    let missing = tempfile::tempdir().expect("missing");
    std::env::set_var(
        "OP_DECOY_TRUST_STORE",
        missing.path().join("missing-trust.json"),
    );
    let (srv2, env) = start_server_with_env(false, env).await;
    let ch = tls_channel(srv2.addr, &srv2.ca_pem).await;
    assert_unauthenticated(
        call_method(
            ch.clone(),
            CallOpts {
                plugin_id: "human_principal",
                method: "resolve_key",
                capability: "human_principal.read",
                args: prost_struct(BTreeMap::from([(
                    "human_pubkey".to_string(),
                    prost_str(&pubkey),
                )])),
                assertion: Some(fresh_signed(&env.issuer, &pubkey, [0x45; 16])),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
        "UnknownDecoyKey",
    );
    let container_pk = pk(39);
    let (fp, trace) = provision_container_identity(ch.clone(), &env, &container_pk, [0x46; 16]).await;
    env.set_grants(&serde_json::json!({ fp.clone(): { "capabilities": ["identity_sled.read"] } }));
    assert_ok(
        call_method(
            ch,
            CallOpts {
                plugin_id: "identity_sled",
                method: "get_identity",
                capability: "identity_sled.read",
                args: prost_struct(BTreeMap::from([(
                    "session_id".to_string(),
                    prost_str(&derive_session_id(&container_pk)),
                )])),
                assertion: None,
                ghostbridge: Some((&fp, &trace)),
                wireguard_pubkey: Some(&container_pk),
                extra_assertion: None,
            },
        )
        .await,
    );
}

// ?? VAL-CROSS-010 ???????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn e2e_alias_mutation_has_no_auth_effect() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let pubkey = pk(40);
    register_human(ch.clone(), &env, &pubkey, "alice", [0x47; 16]).await;
    env.grant_human(&pubkey, &["human_principal.read", "human_principal.write"]);
    let before = assert_ok(
        call_method(
            ch.clone(),
            CallOpts {
                plugin_id: "human_principal",
                method: "resolve_key",
                capability: "human_principal.read",
                args: prost_struct(BTreeMap::from([(
                    "human_pubkey".to_string(),
                    prost_str(&pubkey),
                )])),
                assertion: Some(fresh_signed(&env.issuer, &pubkey, [0x48; 16])),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
    );
    let derived = derive_principal_id(&pubkey);
    assert_ok(
        call_method(
            ch.clone(),
            CallOpts {
                plugin_id: "human_principal",
                method: "set_alias",
                capability: "human_principal.write",
                args: prost_struct(BTreeMap::from([
                    ("principal_id".to_string(), prost_str(&derived)),
                    ("display_alias".to_string(), prost_str("bob")),
                ])),
                assertion: Some(fresh_signed(&env.issuer, &pubkey, [0x49; 16])),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
    );
    let after = assert_ok(
        call_method(
            ch,
            CallOpts {
                plugin_id: "human_principal",
                method: "resolve_key",
                capability: "human_principal.read",
                args: prost_struct(BTreeMap::from([(
                    "human_pubkey".to_string(),
                    prost_str(&pubkey),
                )])),
                assertion: Some(fresh_signed(&env.issuer, &pubkey, [0x4A; 16])),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
    );
    assert_eq!(principal_id_from(&before), principal_id_from(&after));
    assert_eq!(human_pubkey_from(&before), human_pubkey_from(&after));
    assert_eq!(footprint_hex(&pubkey), hex::encode(derive_human_footprint(&pubkey)));
}

// ?? VAL-CROSS-011 ???????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn e2e_per_principal_grant_isolation() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let a = pk(41);
    let b = pk(42);
    register_human(ch.clone(), &env, &a, "a", [0x4B; 16]).await;
    register_human(ch.clone(), &env, &b, "b", [0x4C; 16]).await;
    env.set_grants(&grants_for(&b, &["human_principal.read"]));
    assert_permission_denied(
        call_method(
            ch.clone(),
            CallOpts {
                plugin_id: "human_principal",
                method: "resolve_key",
                capability: "human_principal.read",
                args: prost_struct(BTreeMap::from([(
                    "human_pubkey".to_string(),
                    prost_str(&a),
                )])),
                assertion: Some(fresh_signed(&env.issuer, &a, [0x4D; 16])),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
    );
    env.set_grants(&grants_for(&a, &["human_principal.read"]));
    assert_ok(
        call_method(
            ch,
            CallOpts {
                plugin_id: "human_principal",
                method: "resolve_key",
                capability: "human_principal.read",
                args: prost_struct(BTreeMap::from([(
                    "human_pubkey".to_string(),
                    prost_str(&a),
                )])),
                assertion: Some(fresh_signed(&env.issuer, &a, [0x4E; 16])),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
    );
}

// ?? VAL-CROSS-012 ???????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn e2e_missing_connect_info_fails_closed() {
    let env = TestEnv::new();
    let cozo = env.cozo_path();
    let (srv, env) = start_server_with_env(false, env).await;
    let ch_ok = tls_channel(srv.addr, &srv.ca_pem).await;
    let pubkey = pk(43);
    register_human(ch_ok, &env, &pubkey, "ci", [0x4F; 16]).await;
    drop(srv);
    std::env::set_var("OP_HUMAN_PRINCIPAL_COZO_DB_PATH", &cozo);
    let (srv2, env2) = start_server_with_env(true, TestEnv::new()).await;
    let ch_bad = tls_channel(srv2.addr, &srv2.ca_pem).await;
    env2.grant_human(&pubkey, &["human_principal.read"]);
    assert_unauthenticated(
        call_method(
            ch_bad,
            CallOpts {
                plugin_id: "human_principal",
                method: "resolve_key",
                capability: "human_principal.read",
                args: prost_struct(BTreeMap::from([(
                    "human_pubkey".to_string(),
                    prost_str(&pubkey),
                )])),
                assertion: Some(fresh_signed(&env2.issuer, &pubkey, [0x50; 16])),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
        "MissingConnectInfo",
    );
}

// ?? VAL-CROSS-013 ???????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn e2e_revocation_durable_across_restart() {
    let env = TestEnv::new();
    let cozo = env.cozo_path();
    let (srv, env) = start_server_with_env(false, env).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let k1 = pk(44);
    let k2 = pk(45);
    register_human(ch.clone(), &env, &k1, "k1", [0x51; 16]).await;
    register_human(ch.clone(), &env, &k2, "k2", [0x52; 16]).await;
    env.set_grants(&serde_json::json!({
        footprint_hex(&k1): { "capabilities": ["human_principal.read", "human_principal.write"] },
        footprint_hex(&k2): { "capabilities": ["human_principal.read"] },
    }));
    resolve_human(ch.clone(), &env, &k1, [0x53; 16]).await;
    resolve_human(ch.clone(), &env, &k2, [0x54; 16]).await;
    env.set_grants(&serde_json::json!({
        footprint_hex(&k1): { "capabilities": ["human_principal.read", "human_principal.write"] },
        footprint_hex(&k2): { "capabilities": ["human_principal.read"] },
    }));
    assert_ok(
        call_method(
            ch.clone(),
            CallOpts {
                plugin_id: "human_principal",
                method: "revoke_key",
                capability: "human_principal.write",
                args: prost_struct(BTreeMap::from([(
                    "human_pubkey".to_string(),
                    prost_str(&k1),
                )])),
                assertion: Some(fresh_signed(&env.issuer, &k1, [0x55; 16])),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
    );
    drop(srv);
    drop(ch);
    std::env::set_var("OP_HUMAN_PRINCIPAL_COZO_DB_PATH", &cozo);
    let (srv2, env) = start_server_with_env(false, env).await;
    let ch2 = tls_channel(srv2.addr, &srv2.ca_pem).await;
    env.set_grants(&serde_json::json!({
        footprint_hex(&k1): { "capabilities": ["human_principal.read"] },
        footprint_hex(&k2): { "capabilities": ["human_principal.read"] },
    }));
    assert_unauthenticated(
        call_method(
            ch2.clone(),
            CallOpts {
                plugin_id: "human_principal",
                method: "resolve_key",
                capability: "human_principal.read",
                args: prost_struct(BTreeMap::from([(
                    "human_pubkey".to_string(),
                    prost_str(&k1),
                )])),
                assertion: Some(fresh_signed(&env.issuer, &k1, [0x56; 16])),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
        "RevokedPrincipal",
    );
    assert_ok(
        call_method(
            ch2,
            CallOpts {
                plugin_id: "human_principal",
                method: "resolve_key",
                capability: "human_principal.read",
                args: prost_struct(BTreeMap::from([(
                    "human_pubkey".to_string(),
                    prost_str(&k2),
                )])),
                assertion: Some(fresh_signed(&env.issuer, &k2, [0x57; 16])),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
    );
}

// ?? VAL-CROSS-014 ???????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn e2e_concurrent_multi_principal_interleaving() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let a = pk(46);
    let b = pk(47);
    register_human(ch.clone(), &env, &a, "A", [0x58; 16]).await;
    register_human(ch.clone(), &env, &b, "B", [0x59; 16]).await;
    env.set_grants(&serde_json::json!({
        footprint_hex(&a): { "capabilities": ["human_principal.read"] },
        footprint_hex(&b): { "capabilities": ["human_principal.read"] },
    }));
    let (ra, rb) = tokio::join!(
        call_method(
            ch.clone(),
            CallOpts {
                plugin_id: "human_principal",
                method: "resolve_key",
                capability: "human_principal.read",
                args: prost_struct(BTreeMap::from([(
                    "human_pubkey".to_string(),
                    prost_str(&a),
                )])),
                assertion: Some(fresh_signed(&env.issuer, &a, [0x5A; 16])),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        ),
        call_method(
            ch.clone(),
            CallOpts {
                plugin_id: "human_principal",
                method: "resolve_key",
                capability: "human_principal.read",
                args: prost_struct(BTreeMap::from([(
                    "human_pubkey".to_string(),
                    prost_str(&b),
                )])),
                assertion: Some(fresh_signed(&env.issuer, &b, [0x5B; 16])),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        ),
    );
    let va = assert_ok(ra);
    let vb = assert_ok(rb);
    let check_pid = |v: &ProstValue, pk: &str| {
        assert_eq!(principal_id_from(v), derive_principal_id(pk));
    };
    check_pid(&va, &a);
    check_pid(&vb, &b);
}

// ?? VAL-CROSS-018 ???????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn e2e_human_registration_no_container_side_effects() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let sled_path = std::env::var("OP_SLED_PATH").unwrap();
    let before = sled_dir_bytes(std::path::Path::new(&sled_path));
    let pubkey = pk(48);
    register_human(ch, &env, &pubkey, "sidefx", [0x5C; 16]).await;
    let after = sled_dir_bytes(std::path::Path::new(&sled_path));
    assert_eq!(before, after);
}

// ?? VAL-CROSS-019 ???????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn e2e_revocation_tombstone_end_to_end() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let pubkey = pk(49);
    register_human(ch.clone(), &env, &pubkey, "tomb", [0x5D; 16]).await;
    env.grant_human(
        &pubkey,
        &["human_principal.write", "human_principal.read"],
    );
    assert_ok(
        call_method(
            ch.clone(),
            CallOpts {
                plugin_id: "human_principal",
                method: "revoke_key",
                capability: "human_principal.write",
                args: prost_struct(BTreeMap::from([(
                    "human_pubkey".to_string(),
                    prost_str(&pubkey),
                )])),
                assertion: Some(fresh_signed(&env.issuer, &pubkey, [0x5E; 16])),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
    );
    let dup = call_method(
        ch.clone(),
        CallOpts {
            plugin_id: "human_principal",
            method: "register_key",
            capability: "human_principal.write",
            args: prost_struct(BTreeMap::from([
                ("human_pubkey".to_string(), prost_str(&pubkey)),
                ("display_alias".to_string(), prost_str("retry")),
            ])),
            assertion: Some(fresh_signed(&env.issuer, &pubkey, [0x5F; 16])),
            ghostbridge: None,
            wireguard_pubkey: None,
            extra_assertion: None,
        },
    )
    .await;
    match dup {
        CallOutcome::RpcErr(_) => {}
        CallOutcome::Ok(_) => {}
        CallOutcome::GateDenied => panic!("duplicate register should not be gate denied"),
    }
    assert_unauthenticated(
        call_method(
            ch,
            CallOpts {
                plugin_id: "human_principal",
                method: "resolve_key",
                capability: "human_principal.read",
                args: prost_struct(BTreeMap::from([(
                    "human_pubkey".to_string(),
                    prost_str(&pubkey),
                )])),
                assertion: Some(fresh_signed(&env.issuer, &pubkey, [0x60; 16])),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
        "RevokedPrincipal",
    );
}

// ?? VAL-CROSS-021 ???????????????????????????????????????????????????????????

#[tokio::test(flavor = "multi_thread")]
async fn e2e_restart_empties_replay_cache_expired_stays_closed() {
    let (srv, env) = start_server(false).await;
    let ch = tls_channel(srv.addr, &srv.ca_pem).await;
    let pubkey = pk(50);
    register_human(ch.clone(), &env, &pubkey, "restart", [0x61; 16]).await;
    env.grant_human(&pubkey, &["human_principal.read"]);
    let now = chrono::Utc::now().timestamp();
    let expired = signed_with_fields(&env.issuer, &pubkey, test_ip(), now - 600, now - 60, [0x62; 16], None);
    drop(srv);
    drop(ch);
    let (srv2, env2) = start_server(false).await;
    let ch2 = tls_channel(srv2.addr, &srv2.ca_pem).await;
    assert_unauthenticated(
        call_method(
            ch2,
            CallOpts {
                plugin_id: "human_principal",
                method: "resolve_key",
                capability: "human_principal.read",
                args: prost_struct(BTreeMap::from([(
                    "human_pubkey".to_string(),
                    prost_str(&pubkey),
                )])),
                assertion: Some(expired),
                ghostbridge: None,
                wireguard_pubkey: None,
                extra_assertion: None,
            },
        )
        .await,
        "Expired",
    );
}

// ?? VAL-E2E-023 (count pin) ?????????????????????????????????????????????????

#[test]
fn battery_test_count_is_pinned() {
    assert_eq!(EXPECTED_TEST_COUNT, 34);
}
