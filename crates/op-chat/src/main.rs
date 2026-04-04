//! Standalone op-chat MCP server binary

use op_chat::actor::ChatActorConfig;
use op_chat::mcp_server::run_chat_mcp_server;
use op_chat::ChatActor;
use op_llm::chat::ChatManager;
use op_llm::provider::LlmProvider;
use std::net::SocketAddr;
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let addr: SocketAddr = std::env::var("OP_CHAT_LISTEN")
        .unwrap_or_else(|_| "0.0.0.0:50052".to_string())
        .parse()?;

    // Create LLM provider (auto-detects from env)
    let provider: Arc<dyn LlmProvider> = Arc::new(ChatManager::new());
    tracing::info!("LLM provider: {:?}", provider.provider_type());

    let config = ChatActorConfig::default();
    let (mut actor, handle) = ChatActor::new(config, provider).await?;

    // We need an Arc<ChatActor> for MCP server, but actor also needs to run.
    // Create a second ChatActor just for the MCP server's Arc reference,
    // or better: wrap actor in Arc and use a separate task.
    // Since run() now takes &mut self, we can't share via Arc easily.
    // Solution: run actor in main task, MCP server in spawned task.

    // Keep handle alive
    let _handle = handle;

    // Start MCP server in background — it references builtin_workstacks, not the actor directly
    // Create a minimal ChatActor for the MCP server (it only uses tool_registry)
    let mcp_provider: Arc<dyn LlmProvider> = Arc::new(ChatManager::new());
    let mcp_config = ChatActorConfig::default();
    let (mcp_actor, _mcp_handle) = ChatActor::new(mcp_config, mcp_provider).await?;
    let mcp_actor = Arc::new(mcp_actor);

    tokio::spawn(async move {
        tracing::info!("Starting op-chat MCP server on {}", addr);
        if let Err(e) = run_chat_mcp_server(addr, mcp_actor).await {
            tracing::error!("MCP server error: {}", e);
        }
    });

    // Run the main actor event loop (blocks until all handles dropped)
    actor.run().await;

    Ok(())
}
