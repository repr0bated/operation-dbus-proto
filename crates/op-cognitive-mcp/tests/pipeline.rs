use op_blockchain::PluginFootprint;
use op_blockchain::btrfs_numa_integration::OptimizedBlockchain;
use op_cognitive_mcp::graph_store::KnowledgeGraphStore;
use simd_json::prelude::ValueAsScalar;
use simd_json::json;
use tempfile::tempdir;
use tokio::sync::mpsc;

fn extract_payload_string(payload: &simd_json::OwnedValue, key: &str) -> String {
    payload[key]
        .as_str()
        .unwrap_or_else(|| panic!("missing string payload key: {key}"))
        .to_string()
}

#[tokio::test]
async fn should_route_blockchain_footprint_into_embed_request_and_graph_projection() {
    let blockchain_dir = tempdir().unwrap();
    let cache_dir = tempdir().unwrap();
    let (embed_tx, mut embed_rx) = mpsc::channel(4);

    let blockchain = OptimizedBlockchain::new(blockchain_dir.path(), cache_dir.path())
        .await
        .unwrap()
        .with_embed_channel(embed_tx);

    let metadata = json!({
        "namespace": "work:project-x",
        "conversation_id": "conv-123",
        "user_id": "user-7"
    });
    let mut footprint = PluginFootprint::new(
        "ctl-plane-chatbot",
        "decision",
        &json!({"decision": "use cozo"}),
    );
    let metadata_map = simd_json::serde::from_owned_value(metadata).unwrap();
    footprint.metadata = metadata_map;
    footprint.vector_features = vec![0.42, 0.13, 0.07];

    let graph = KnowledgeGraphStore::new_in_memory().unwrap();
    let block_hash = blockchain.add_footprint(footprint.clone()).await.unwrap();

    let embed_req = embed_rx.recv().await.expect("expected embed request");
    assert_eq!(embed_req.block_hash, block_hash);
    assert_eq!(embed_req.collection, "ctl_plane_reasoning_episodes");
    assert_eq!(
        extract_payload_string(&embed_req.payload, "plugin_id"),
        "ctl-plane-chatbot"
    );
    assert_eq!(
        extract_payload_string(&embed_req.payload, "operation"),
        "decision"
    );

    graph.project_footprint(&block_hash, &footprint).unwrap();
    let events = graph.list_projected_events().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].block_hash, block_hash);
    assert_eq!(events[0].plugin_id, "ctl-plane-chatbot");
    assert_eq!(events[0].namespace, "work:project-x");

    let links = graph.list_links().unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].source, block_hash);
    assert_eq!(links[0].target, "work:project-x");
}
