// Integration tests for the zeroclaw Axum HTTP/gRPC-Web host.
//
// These tests spin up the TCP side of the server on an ephemeral port and
// exercise the generated ZeroclawService through a native tonic client. The
// server is configured with `tonic_web::enable`, so the same listener also
// accepts gRPC-Web/HTTP/1.1 clients.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::Query;
use axum::routing::get;
use axum::{Json, Router};
use op_grpc_bridge::proto::zeroclaw::{
    zeroclaw_service_client::ZeroclawServiceClient, GetSchemaRequest, WatchSchemaRequest,
};
use op_grpc_bridge::schema_loader::SchemaLoader;
use op_grpc_bridge::server::build_axum_app;
use serde_json::json;
use tokio::time::timeout;
use tonic::metadata::MetadataValue;

const TEST_TRACE_HEADER: &str = "x-ghostbridge-trace-id";
const TEST_FOOTPRINT_HEADER: &str = "x-ghostbridge-footprint";

fn write_schema(path: &PathBuf, value: &serde_json::Value) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, serde_json::to_string(value).unwrap()).unwrap();
}

async fn start_test_server() -> (SocketAddr, Arc<SchemaLoader>, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("live-schema.json");
    let schema = json!({
        "name": "zeroclaw",
        "version": "1.0.0",
        "kind": "llm",
        "description": "test schema"
    });
    write_schema(&path, &json!({ "zeroclaw": [schema] }));

    let loader = Arc::new(SchemaLoader::new(&path).unwrap());

    // Build the intercepted plugin service exactly as `run_zeroclaw_server`
    // does, so the Axum app mounts both the zeroclaw service and the plugin
    // service behind the Ghostbridge interceptor.
    let event_chain = Arc::new(tokio::sync::RwLock::new(op_state_store::EventChain::new(
        op_state_store::ChainConfig::default(),
    )));
    let ovsdb = Arc::new(op_network::rovs_proxy::OvsdbDbusClient::new());
    let mutation_engine = Arc::new(op_grpc_bridge::MutationEngine::new(event_chain, ovsdb));
    let operation_server = op_grpc_bridge::grpc_server::OperationGrpcServer::new(mutation_engine);
    let app = build_axum_app(loader.clone(), operation_server);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Give the server a moment to start accepting connections.
    tokio::time::sleep(Duration::from_millis(100)).await;

    (addr, loader, path)
}

async fn start_zeroclaw_runtime() -> SocketAddr {
    async fn health() -> Json<serde_json::Value> {
        Json(json!({ "status": "ok" }))
    }

    async fn config_prop(
        Query(query): Query<std::collections::HashMap<String, String>>,
    ) -> Json<serde_json::Value> {
        assert_eq!(
            query.get("path").map(String::as_str),
            Some("agents.dashboard.model_provider")
        );
        Json(json!({ "value": "opencode.go" }))
    }

    async fn config_list() -> Json<serde_json::Value> {
        Json(json!({
            "entries": [{
                "path": "providers.models.opencode.go.model",
                "value": "deepseek-v4-flash-free"
            }]
        }))
    }

    async fn model_catalog(
        Query(query): Query<std::collections::HashMap<String, String>>,
    ) -> Json<serde_json::Value> {
        assert_eq!(
            query.get("model_provider").map(String::as_str),
            Some("opencode")
        );
        Json(json!({
            "models": ["deepseek-v4-flash-free", "runtime-discovered-test-model"],
            "live": true
        }))
    }

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/config/prop", get(config_prop))
        .route("/api/config/list", get(config_list))
        .route("/api/config/catalog/models", get(model_catalog));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    addr
}

#[tokio::test]
async fn should_serve_schema_over_grpc_web() {
    let (addr, _loader, _path) = start_test_server().await;

    let endpoint = format!("http://{}", addr);
    let mut client = ZeroclawServiceClient::connect(endpoint).await.unwrap();

    let mut request = tonic::Request::new(GetSchemaRequest {});
    request.metadata_mut().insert(
        TEST_TRACE_HEADER,
        MetadataValue::from_static("integration-test-trace"),
    );
    request.metadata_mut().insert(
        TEST_FOOTPRINT_HEADER,
        MetadataValue::from_static("integration-test-footprint"),
    );

    let response = client.get_schema(request).await.unwrap();
    let inner = response.into_inner();

    let parsed: serde_json::Value = serde_json::from_str(&inner.schema_json).unwrap();
    assert_eq!(parsed["name"], "zeroclaw");
    assert_eq!(parsed["version"], "1.0.0");
    assert_eq!(inner.trace_id, "integration-test-trace");
    assert_eq!(inner.footprint, "integration-test-footprint");
}

#[tokio::test]
async fn should_stream_reload_on_sighup() {
    let (addr, loader, path) = start_test_server().await;

    let endpoint = format!("http://{}", addr);
    let mut client = ZeroclawServiceClient::connect(endpoint).await.unwrap();

    let mut stream = client
        .watch_schema(tonic::Request::new(WatchSchemaRequest {}))
        .await
        .unwrap()
        .into_inner();

    let initial = timeout(Duration::from_secs(5), stream.message())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(initial.event_type, "initial");

    // Simulate a SIGHUP-triggered reload by reloading the file and broadcasting
    // the event through the loader's channel. Tests do not send real SIGHUP to
    // the process to avoid interfering with other tests.
    let updated = json!({
        "name": "zeroclaw",
        "version": "2.0.0",
        "kind": "llm",
        "description": "reloaded schema"
    });
    write_schema(&path, &updated);
    loader.load().unwrap();
    let _ = loader
        .reload_tx()
        .send(op_grpc_bridge::schema_loader::SchemaReloadEvent {
            event_type: "reload".to_string(),
        });

    let event = timeout(Duration::from_secs(5), stream.message())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(event.event_type, "reload");

    let parsed: serde_json::Value = serde_json::from_str(&event.schema_json).unwrap();
    assert_eq!(parsed["version"], "2.0.0");
}

#[tokio::test]
async fn list_models_uses_the_audited_zeroclaw_method_dispatcher() {
    let runtime_addr = start_zeroclaw_runtime().await;
    let event_chain = Arc::new(tokio::sync::RwLock::new(op_state_store::EventChain::new(
        op_state_store::ChainConfig::default(),
    )));
    let ovsdb = Arc::new(op_network::rovs_proxy::OvsdbDbusClient::new());
    let engine = op_grpc_bridge::MutationEngine::new_with_zeroclaw_runtime(
        event_chain.clone(),
        ovsdb,
        format!("http://{runtime_addr}"),
        "dashboard".to_string(),
        None,
    );

    let result = engine
        .dispatch_method_call(
            "zeroclaw",
            "ListModels",
            r#"{"provider":"opencode"}"#,
            Some("cap.software.zeroclaw.models.read@v1"),
            "integration-test-actor",
        )
        .await
        .unwrap();

    assert_eq!(result["success"], true);
    assert_eq!(result["plugin_id"], "zeroclaw");
    assert_eq!(result["method"], "ListModels");
    let routes = result["result"]["model_routes"].as_array().unwrap();
    assert!(!routes.is_empty());
    assert!(routes
        .iter()
        .all(|route| { route["provider"] == "opencode" }));
    assert!(routes
        .iter()
        .any(|route| route["model"] == "runtime-discovered-test-model"));

    let chain = event_chain.read().await;
    let event = chain.events().last().unwrap();
    assert_eq!(event.plugin_id, "zeroclaw");
    assert_eq!(event.method_name.as_deref(), Some("ListModels"));
    assert_eq!(
        event.capability_id.as_deref(),
        Some("cap.software.zeroclaw.models.read@v1")
    );
    assert_eq!(event.actor_id, "integration-test-actor");
    assert!(event.json_args_footprint.is_some());
}
