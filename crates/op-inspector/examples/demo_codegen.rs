//! Generate a real plugin scaffold from real inferred data and print it,
//! so it can be dropped into op-plugins to verify the OUTPUT actually
//! compiles (not just this generator function).

use op_inspector::{generate_plugin_scaffold, InspectionInput, InspectionSource, IntrospectiveGadget, KnowledgeBase};
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let kb = Arc::new(RwLock::new(KnowledgeBase::default()));
    let gadget = IntrospectiveGadget::new(kb).await?;

    // Simulates genuinely unknown data -- e.g. a webhook payload from a
    // service this repo has no plugin for yet.
    let sample = r#"{
        "event_type": "deployment.completed",
        "service_name": "example-svc",
        "region": "us-east-1",
        "replica_count": 3,
        "healthy": true,
        "metadata": {"trigger": "manual"}
    }"#;

    let input = InspectionInput {
        source: InspectionSource::RawData {
            format_hint: Some("json".to_string()),
            description: "unknown_webhook_payload".to_string(),
        },
        data: Some(sample.to_string()),
        metadata: Default::default(),
    };

    let result = gadget.inspect_object(input).await?;
    let scaffold = generate_plugin_scaffold("unknown_webhook", "Unknown webhook payload, auto-inferred by op-inspector", &result.schema);

    std::fs::write(
        "/tmp/claude-1000/-home-jeremy-git-operation-dbus-proto/88ffa2c0-8dc5-4914-bd79-4e1098ff688d/scratchpad/unknown_webhook.rs",
        &scaffold,
    )?;
    println!("{scaffold}");

    Ok(())
}
