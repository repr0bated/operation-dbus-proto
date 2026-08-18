//! Integration tests for the audit-trail query surface
//! (`.kiro/specs/accountability-audit-trail`).
//!
//! These exercise `MutationEngine::dispatch_method_call` directly, which is the
//! single dispatch path behind both `PluginV1.Call` over D-Bus and the generated
//! per-plugin gRPC methods. They assert the two things the spec's acceptance
//! criteria hinge on:
//!
//!   1. `blockchain.query_events` / `blockchain.verify_chain` return real audit
//!      data — not the catch-all echo of their input.
//!   2. The blockchain plugin's seven pre-existing methods still echo, proving
//!      the scope boundary was not crossed.

use std::sync::Arc;

use op_grpc_bridge::MutationEngine;

fn engine() -> Arc<MutationEngine> {
    let event_chain = Arc::new(tokio::sync::RwLock::new(op_state_store::EventChain::new(
        op_state_store::ChainConfig::default(),
    )));
    let ovsdb = Arc::new(op_network::rovs_proxy::OvsdbDbusClient::new());
    Arc::new(MutationEngine::new(event_chain, ovsdb))
}

/// Every dispatch records an event, so calling a method is how we populate the
/// chain under test.
async fn record_calls(engine: &MutationEngine, count: usize) {
    for i in 0..count {
        engine
            .dispatch_method_call(
                "cognitive_mcp",
                "get_health",
                &format!("{{\"probe\":{i}}}"),
                Some("cognitive.read"),
                "test-actor",
            )
            .await
            .expect("dispatch records an event");
    }
}

#[tokio::test]
async fn query_events_returns_real_audit_data_not_echo() {
    let engine = engine();
    record_calls(&engine, 3).await;

    let response = engine
        .dispatch_method_call(
            "blockchain",
            "query_events",
            "{\"limit\":10}",
            Some("blockchain.read"),
            "test-actor",
        )
        .await
        .expect("query_events dispatches");

    let result = &response["result"];

    // An echo would surface the input key `limit` and carry no `events` array.
    assert!(
        result.get("limit").is_none(),
        "query_events echoed its input instead of dispatching: {result}"
    );
    let events = result["events"]
        .as_array()
        .unwrap_or_else(|| panic!("expected an events array, got: {result}"));

    // 3 recorded calls + the query_events call itself, which is also audited.
    assert_eq!(events.len(), 4, "unexpected page: {result}");
    assert_eq!(result["has_more"], serde_json::json!(false));
    assert_eq!(result["total_in_chain"], serde_json::json!(4));

    // Records carry the accountability surface, not just ids.
    let first = &events[0];
    assert_eq!(first["plugin_id"], serde_json::json!("cognitive_mcp"));
    assert_eq!(first["method_name"], serde_json::json!("get_health"));
    assert_eq!(first["actor_id"], serde_json::json!("test-actor"));
    assert_eq!(first["capability_id"], serde_json::json!("cognitive.read"));
    assert_eq!(first["decision"], serde_json::json!("Allow"));
    assert!(!first["event_hash"].as_str().unwrap_or_default().is_empty());
    assert!(!first["timestamp"].as_str().unwrap_or_default().is_empty());
}

#[tokio::test]
async fn query_events_filters_and_clamps_limit() {
    let engine = engine();
    record_calls(&engine, 5).await;

    // limit=2 must return exactly 2 rows and report more available.
    let paged = engine
        .dispatch_method_call(
            "blockchain",
            "query_events",
            "{\"limit\":2}",
            Some("blockchain.read"),
            "test-actor",
        )
        .await
        .expect("dispatch");
    let paged = &paged["result"];
    assert_eq!(paged["events"].as_array().map(Vec::len), Some(2));
    assert_eq!(paged["has_more"], serde_json::json!(true));

    // An over-max limit is clamped silently rather than rejected (FR-4).
    let clamped = engine
        .dispatch_method_call(
            "blockchain",
            "query_events",
            "{\"limit\":100000}",
            Some("blockchain.read"),
            "test-actor",
        )
        .await
        .expect("dispatch");
    assert!(
        clamped["result"]["events"]
            .as_array()
            .map(Vec::len)
            .unwrap()
            <= 100,
        "limit was not clamped to 100"
    );

    // plugin_id filter excludes other plugins' events.
    let filtered = engine
        .dispatch_method_call(
            "blockchain",
            "query_events",
            "{\"plugin_id\":\"cognitive_mcp\",\"limit\":50}",
            Some("blockchain.read"),
            "test-actor",
        )
        .await
        .expect("dispatch");
    let rows = filtered["result"]["events"].as_array().cloned().unwrap();
    assert!(!rows.is_empty(), "plugin filter returned nothing");
    assert!(
        rows.iter()
            .all(|e| e["plugin_id"] == serde_json::json!("cognitive_mcp")),
        "plugin filter leaked other plugins"
    );

    // decision filter maps onto the chain's Decision enum.
    let denied = engine
        .dispatch_method_call(
            "blockchain",
            "query_events",
            "{\"decision\":\"deny\",\"limit\":50}",
            Some("blockchain.read"),
            "test-actor",
        )
        .await
        .expect("dispatch");
    assert_eq!(
        denied["result"]["events"].as_array().map(Vec::len),
        Some(0),
        "no events were denied, so the deny filter must return an empty page"
    );
}

#[tokio::test]
async fn verify_chain_reports_integrity() {
    let engine = engine();
    record_calls(&engine, 4).await;

    let response = engine
        .dispatch_method_call(
            "blockchain",
            "verify_chain",
            "{}",
            Some("blockchain.read"),
            "test-actor",
        )
        .await
        .expect("verify_chain dispatches");
    let result = &response["result"];

    assert_eq!(result["valid"], serde_json::json!(true), "chain: {result}");
    // 4 recorded calls + this verify_chain call, which is itself audited.
    assert_eq!(result["events_verified"], serde_json::json!(5));
    assert_eq!(result["errors"], serde_json::json!([]));
}

#[tokio::test]
async fn existing_blockchain_methods_still_echo() {
    let engine = engine();

    // Scope boundary (FR-8): the seven pre-existing methods stay un-wired and
    // fall through to the catch-all echo.
    for method in [
        "list_snapshots",
        "get_snapshot",
        "create_snapshot",
        "rollback",
        "get_current_state",
        "set_retention",
        "get_stats",
    ] {
        let response = engine
            .dispatch_method_call(
                "blockchain",
                method,
                "{\"scope_marker\":true}",
                Some("blockchain.read"),
                "test-actor",
            )
            .await
            .unwrap_or_else(|e| panic!("{method} dispatch failed: {e}"));

        assert_eq!(
            response["result"]["scope_marker"],
            serde_json::json!(true),
            "{method} no longer echoes — the scope boundary was crossed"
        );
        assert!(
            response["result"].get("events").is_none(),
            "{method} unexpectedly returned audit data"
        );
    }
}

#[tokio::test]
async fn audit_methods_are_declared_in_the_plugin_schema() {
    use op_state::StatePlugin;
    use op_state_store::SideEffect;

    let schema = op_plugins::state_plugins::blockchain_plugin::BlockchainPlugin::new()
        .schema()
        .expect("blockchain plugin publishes a schema");

    for name in ["query_events", "verify_chain"] {
        let decl = schema
            .methods
            .get(name)
            .unwrap_or_else(|| panic!("{name} missing from schema.methods"));
        assert!(matches!(decl.side_effect, SideEffect::Read));
        assert_eq!(decl.required_capability.as_deref(), Some("blockchain.read"));
        assert!(decl.returns.is_some(), "{name} must declare a typed output");
    }

    // The seven originals are still declared.
    for name in [
        "create_snapshot",
        "list_snapshots",
        "get_snapshot",
        "rollback",
        "get_current_state",
        "set_retention",
        "get_stats",
    ] {
        assert!(
            schema.methods.contains_key(name),
            "{name} disappeared from the schema"
        );
    }
}

/// FR-6: events are written to the `timing_subvol` as they happen, and a fresh
/// engine over the same path rebuilds the chain from disk — the "survives a
/// restart" claim, exercised without touching a live service.
///
/// The chain path must be on Btrfs (`StreamingBlockchain` creates subvolumes).
/// If subvolume creation is unavailable the durability sink stays disabled by
/// design (NFR-4), and this test says so rather than silently passing.
#[tokio::test]
async fn audit_trail_persists_and_survives_a_restart() {
    let base = std::env::temp_dir().join(format!("opdbus-audit-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let base_path = base.to_string_lossy().to_string();

    // ── First "process": record events with durability enabled ──────────────
    let first = engine();
    first.init_audit_durability_at(&base_path).await;
    record_calls(&first, 3).await;

    let timing = base.join("timing");
    let files: Vec<_> = std::fs::read_dir(&timing)
        .map(|dir| {
            dir.filter_map(Result::ok)
                .filter(|e| e.path().extension().and_then(|x| x.to_str()) == Some("json"))
                .collect()
        })
        .unwrap_or_default();

    if files.is_empty() {
        // Sink unavailable (no Btrfs / no permission). Assert the documented
        // degraded behaviour instead: dispatch still succeeds and the in-memory
        // chain is intact.
        let live = first
            .dispatch_method_call(
                "blockchain",
                "query_events",
                "{\"limit\":10}",
                Some("blockchain.read"),
                "test-actor",
            )
            .await
            .expect("dispatch must succeed even without a durable sink");
        assert!(
            live["result"]["events"]
                .as_array()
                .map(Vec::len)
                .unwrap_or(0)
                >= 3,
            "in-memory chain must still hold the events"
        );
        eprintln!(
            "durable sink unavailable at {base_path}; verified degraded (RAM-only) behaviour"
        );
        return;
    }

    // One file per event: 3 dispatches, each audited.
    assert_eq!(
        files.len(),
        3,
        "expected one timing record per event, found {}",
        files.len()
    );

    // ── Second "process": fresh empty chain, same path ──────────────────────
    let second = engine();
    let replayed = second.init_audit_durability_at(&base_path).await;
    assert_eq!(replayed, 3, "rebuild did not replay every persisted event");

    let restored = second
        .dispatch_method_call(
            "blockchain",
            "query_events",
            "{\"limit\":10}",
            Some("blockchain.read"),
            "test-actor",
        )
        .await
        .expect("dispatch");
    let restored = &restored["result"];
    let events = restored["events"].as_array().expect("events array");

    // 3 replayed from disk + this query_events call.
    assert_eq!(events.len(), 4, "restored page: {restored}");
    assert_eq!(events[0]["method_name"], serde_json::json!("get_health"));
    assert_eq!(events[0]["plugin_id"], serde_json::json!("cognitive_mcp"));

    // Hash linkage survived persistence, so the rebuilt chain verifies and the
    // new event continues the chain instead of restarting the ids.
    let verified = second
        .dispatch_method_call(
            "blockchain",
            "verify_chain",
            "{}",
            Some("blockchain.read"),
            "test-actor",
        )
        .await
        .expect("dispatch");
    assert_eq!(
        verified["result"]["valid"],
        serde_json::json!(true),
        "rebuilt chain failed verification: {}",
        verified["result"]
    );
    assert_eq!(events[3]["event_id"], serde_json::json!(4));

    let _ = std::fs::remove_dir_all(&base);
}
