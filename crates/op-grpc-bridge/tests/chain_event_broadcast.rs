//! `EventChainService.SubscribeEvents` is fed by the mutation pipeline itself.
//!
//! Both doors that append to the event chain — `dispatch_method_call` and
//! `process_authoritative_change` — must publish what they appended on the
//! engine's chain broadcast. If they don't, the audit stream is silently empty
//! while the ledger behind `GetEvents` keeps filling up, which is exactly the
//! failure these tests exist to catch.

use std::sync::Arc;
use std::time::Duration;

use op_grpc_bridge::{ChangeSource, ChangeType, MutationEngine};

fn engine() -> Arc<MutationEngine> {
    let event_chain = Arc::new(tokio::sync::RwLock::new(op_state_store::EventChain::new(
        op_state_store::ChainConfig::default(),
    )));
    let ovsdb = Arc::new(op_network::rovs_proxy::OvsdbDbusClient::new());
    Arc::new(MutationEngine::new(event_chain, ovsdb))
}

async fn next_event(
    rx: &mut tokio::sync::broadcast::Receiver<op_state_store::ChainEvent>,
) -> op_state_store::ChainEvent {
    tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("a recorded event reaches chain subscribers")
        .expect("chain broadcast stayed open")
}

#[tokio::test]
async fn dispatched_method_call_reaches_chain_subscribers() {
    let engine = engine();
    let mut rx = engine.chain_tx().subscribe();

    engine
        .dispatch_method_call(
            "cognitive_mcp",
            "get_health",
            "{\"probe\":1}",
            Some("cognitive.read"),
            "test-actor",
        )
        .await
        .expect("dispatch records an event");

    let event = next_event(&mut rx).await;
    assert_eq!(event.plugin_id, "cognitive_mcp");
    assert_eq!(event.actor_id, "test-actor");
    assert_eq!(event.capability_id.as_deref(), Some("cognitive.read"));
    assert!(!event.event_hash.is_empty());

    // The broadcast event is the same one GetEvents serves, not a reconstruction.
    let chain = engine.event_chain.read().await;
    let recorded = chain.events().last().expect("chain holds the event");
    assert_eq!(recorded.event_id, event.event_id);
    assert_eq!(recorded.event_hash, event.event_hash);
}

#[tokio::test]
async fn native_cognitive_mutation_uses_the_same_audit_chain_and_grpc_provenance() {
    let engine = engine();
    let mut rx = engine.chain_tx().subscribe();
    let mut changes = engine.change_tx().subscribe();
    let arguments = r#"{"notebook_id":"project:3tched-cognative","content_sha256":"redacted"}"#;

    let receipt = engine
        .audit_cognitive_mcp_mutation(
            "add_source",
            arguments,
            "cognitive_mcp.invoke",
            "authenticated-session",
        )
        .await
        .expect("native cognitive mutation receives an audit receipt");

    let event = next_event(&mut rx).await;
    assert_eq!(event.event_id, receipt.event_id);
    assert_eq!(event.event_hash, receipt.event_hash);
    assert_eq!(event.plugin_id, "cognitive_mcp");
    assert_eq!(event.method_name.as_deref(), Some("add_source"));
    assert_eq!(event.actor_id, "authenticated-session");
    assert_eq!(event.capability_id.as_deref(), Some("cognitive_mcp.invoke"));
    let expected_footprint = blake3::hash(arguments.as_bytes()).to_hex().to_string();
    assert_eq!(
        event.json_args_footprint.as_deref(),
        Some(expected_footprint.as_str())
    );

    let change = tokio::time::timeout(Duration::from_secs(5), changes.recv())
        .await
        .expect("native cognitive mutation reaches state subscribers")
        .expect("state broadcast stayed open");
    assert_eq!(change.change_type, ChangeType::MethodCall);
    assert_eq!(change.source, ChangeSource::Grpc);
    let chain = engine.event_chain.read().await;
    assert!(chain
        .events()
        .last()
        .is_some_and(|recorded| recorded.verify()));
}

#[tokio::test]
async fn cognitive_model_route_rejects_mutation_capability_before_recording_or_execution() {
    let engine = engine();
    let error = engine
        .execute_cognitive_model_route(
            "generate_data_table",
            "Return a JSON array.",
            "cognitive_mcp.invoke",
            "authenticated-session",
        )
        .await
        .expect_err("a model read cannot use the mutation capability");
    assert!(error.to_string().contains("cognitive_mcp.read"));
    assert!(
        engine.event_chain.read().await.events().is_empty(),
        "rejected model calls must not create an allowed audit event"
    );
}

#[tokio::test]
async fn authoritative_change_reaches_chain_subscribers() {
    let engine = engine();
    let mut rx = engine.chain_tx().subscribe();

    engine
        .process_authoritative_change(
            "incus".to_string(),
            "/org/opdbus/v1/plugins/incus".to_string(),
            ChangeType::PropertySet,
            Some("containers".to_string()),
            None,
            simd_json::json!({"containers": []}),
            vec!["state".to_string()],
            "test-actor".to_string(),
            None,
            ChangeSource::Grpc,
        )
        .await
        .expect("authoritative change records an event");

    let event = next_event(&mut rx).await;
    assert_eq!(event.plugin_id, "incus");
    assert_eq!(event.actor_id, "test-actor");
    assert!(event.tags_touched.contains(&"state".to_string()));
    assert!(!event.event_hash.is_empty());
}
