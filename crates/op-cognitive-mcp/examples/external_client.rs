//! Cognitive MCP External Client Example
//!
//! Demonstrates how external clients (Droid, Cursor, Codex, NotebookLM, Gemini CLI)
//! should connect to cognitive-mcp on port 3003 per AGENTS.md specification.
//!
//! ## Architecture Compliance
//!
//! Per AGENTS.md Section 4b:
//! - **cognitive-mcp** (`:3003`, Netmaker WG IP `100.90.37.254`) is the **universal
//!   gateway for ALL external clients**
//! - **compact-mcp** (`127.0.0.1:11436`) is **loopback/chatbot only**
//!
//! ## Usage
//!
//! ```bash
//! # Run with external client configuration (correct)
//! cargo run --example external_client -- --client-type droid
//!
//! # Run with local chatbot configuration (loopback only)
//! cargo run --example external_client -- --client-type local
//! ```

use clap::Parser;
use op_cognitive_mcp::client_config::{
    ClientConfig, CognitiveMcpClient, CognitiveMcpClientFactory,
};
use serde_json::json;
use tracing::{error, info, warn};

#[derive(Parser)]
#[command(name = "cognitive-mcp-client-example")]
#[command(about = "Example external client for cognitive-mcp gateway")]
struct Cli {
    /// Client type: droid, cursor, codex, gemini, notebooklm, or local
    #[arg(short, long, default_value = "droid")]
    client_type: String,

    /// Custom client identifier
    #[arg(short, long)]
    client_id: Option<String>,

    /// Enable debug logging
    #[arg(long)]
    debug: bool,

    /// Demonstrate batched operations
    #[arg(long)]
    batch: bool,

    /// Number of parallel requests for batch demo
    #[arg(long, default_value = "5")]
    batch_count: usize,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Initialize logging
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(if cli.debug {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Generate or use provided client ID
    let client_id = cli
        .client_id
        .unwrap_or_else(|| format!("{}-{}", cli.client_type, uuid::Uuid::new_v4()));

    info!(client_id = %client_id, client_type = %cli.client_type, "Initializing Cognitive MCP client");

    // Create configuration based on client type
    let config = match cli.client_type.as_str() {
        "droid" => {
            info!("Creating Droid-optimized client configuration");
            ClientConfig::for_external_client(&client_id)
        }
        "cursor" => {
            info!("Creating Cursor low-latency configuration");
            CognitiveMcpClientFactory::low_latency(&client_id)
        }
        "codex" => {
            info!("Creating Codex high-throughput configuration");
            CognitiveMcpClientFactory::high_throughput(&client_id)
        }
        "gemini" | "gemini_cli" => {
            info!("Creating Gemini CLI configuration");
            ClientConfig::for_external_client(&client_id)
        }
        "notebooklm" => {
            info!("Creating NotebookLM configuration (50 query daily limit)");
            CognitiveMcpClientFactory::for_notebooklm(&client_id)
        }
        "local" => {
            info!("Creating local chatbot configuration (loopback only)");
            ClientConfig::for_local_chatbot(&client_id)
        }
        _ => {
            warn!(
                "Unknown client type '{}', using default external client",
                cli.client_type
            );
            ClientConfig::for_external_client(&client_id)
        }
    };

    // Log configuration details
    info!(
        endpoint = %config.endpoint,
        client_type = ?config.client_type,
        daily_quota = config.quota_tier.daily_limit,
        "Client configuration"
    );

    // Validate configuration
    if let Err(e) = config.validate() {
        error!(error = %e, "Configuration validation failed");
        return Err(e.into());
    }

    // Connect to cognitive-mcp
    let client = CognitiveMcpClient::connect(config).await?;
    info!("Connected to cognitive-mcp successfully");

    // Show quota status
    let (remaining, limit) = client.quota_status().await;
    info!(remaining = remaining, limit = limit, "Initial quota status");

    // Demo 1: Cached tool discovery
    demo_tool_discovery(&client).await?;

    // Demo 2: Memory operations
    demo_memory_operations(&client).await?;

    // Demo 3: Batch operations (if requested)
    if cli.batch {
        demo_batch_operations(&client, cli.batch_count).await?;
    }

    // Demo 4: Statistics
    let stats = client.stats().await;
    info!(?stats, "Client statistics");

    info!("External client example completed successfully");
    Ok(())
}

async fn demo_tool_discovery(
    client: &CognitiveMcpClient,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("=== Tool Discovery Demo ===");

    // First call - fetches from server
    let start = std::time::Instant::now();
    let tools = client.list_tools_cached().await?;
    info!(
        count = tools.len(),
        elapsed_ms = start.elapsed().as_millis(),
        "First tool list fetch (from server)"
    );

    // Second call - returns from cache
    let start = std::time::Instant::now();
    let _cached_tools = client.list_tools_cached().await?;
    info!(
        elapsed_ms = start.elapsed().as_micros(),
        "Second tool list fetch (from cache)"
    );

    // List available tools
    for tool in tools.iter().take(5) {
        if let Some(name) = tool.get("name").and_then(|n| n.as_str()) {
            info!(tool_name = name, "Available tool");
        }
    }

    Ok(())
}

async fn demo_memory_operations(
    client: &CognitiveMcpClient,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("=== Memory Operations Demo ===");

    let namespace = "demo:external-client";
    let key = "test-entry";

    // Store a value
    let value = json!({
        "client_type": "external_demo",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "message": "Hello from cognitive-mcp client!"
    });

    info!(namespace = namespace, key = key, "Storing memory entry");
    let result = client
        .store_memory(
            namespace,
            key,
            value.clone(),
            vec!["demo".into(), "external".into()],
        )
        .await;

    match result {
        Ok(_) => info!("Memory entry stored successfully"),
        Err(e) => warn!(error = %e, "Failed to store memory (server may be unavailable)"),
    }

    // Retrieve the value
    info!(namespace = namespace, key = key, "Retrieving memory entry");
    let retrieved = client.retrieve_memory(namespace, key).await;

    match retrieved {
        Ok(Some(value)) => info!(value = %value, "Retrieved memory entry"),
        Ok(None) => info!("No entry found"),
        Err(e) => warn!(error = %e, "Failed to retrieve memory"),
    }

    // Show updated quota
    let (remaining, limit) = client.quota_status().await;
    info!(remaining = remaining, limit = limit, "Updated quota status");

    Ok(())
}

async fn demo_batch_operations(
    client: &CognitiveMcpClient,
    count: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("=== Batch Operations Demo ===");
    info!(batch_size = count, "Executing parallel memory stores");

    let namespace = "demo:batch";
    let start = std::time::Instant::now();

    // Prepare batch calls
    let calls: Vec<(&str, serde_json::Value)> = (0..count)
        .map(|i| {
            (
                "memory/store",
                json!({
                    "namespace": namespace,
                    "key": format!("batch-item-{}", i),
                    "value": {
                        "index": i,
                        "timestamp": chrono::Utc::now().to_rfc3339()
                    },
                    "tags": ["batch", "demo"]
                }),
            )
        })
        .collect();

    // Execute in parallel
    let results = client.call_tools_batch(calls).await?;

    let elapsed_ms = start.elapsed().as_millis();
    let success_count = results.iter().filter(|(_, r)| r.is_ok()).count();
    let fail_count = results.len() - success_count;

    info!(
        total = results.len(),
        success = success_count,
        failed = fail_count,
        elapsed_ms = elapsed_ms,
        avg_ms_per_op = elapsed_ms / results.len() as u128,
        "Batch operations complete"
    );

    Ok(())
}
