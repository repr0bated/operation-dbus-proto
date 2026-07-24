//! Standalone op-chat MCP server binary

use op_chat::actor::ChatActorConfig;
use op_chat::mcp_server::run_chat_mcp_server;
use op_chat::memory_loop::MemoryLoop;
use op_chat::ChatActor;
use op_cognitive_mcp::{CognitiveMemoryStore, CozoGraphShuttle, QdrantSemanticShuttle};
use op_llm::chat::ChatManager;
use op_llm::provider::LlmProvider;
use std::net::SocketAddr;
use std::path::PathBuf;
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

    // Wire memory loop — CozoDB (durable) + optional Qdrant (semantic)
    let db_path = std::env::var("OP_MEMORY_DB")
        .unwrap_or_else(|_| "/var/lib/op-dbus/chat-memory.db".to_string());
    match CozoGraphShuttle::new_persistent(PathBuf::from(&db_path)) {
        Ok(cozo) => {
            let cozo_arc = Arc::new(cozo);
            let mem_store = Arc::new(CognitiveMemoryStore::new(cozo_arc.clone()).await?);

            // The container account is pre-assigned from the persistent wg0
            // registry. WG terminates upstream; this process never probes a
            // local interface or identity file. Registering the account here
            // seeds Cozo's durable pubkey -> account-session mapping, while
            // request authentication remains exclusively Ghostbridge metadata.
            if let Ok(pubkey) = std::env::var("OP_ACCOUNT_WG_PUBKEY") {
                let pubkey = pubkey.trim();
                if !pubkey.is_empty() {
                    let session_id = op_identity::session::derive_session_id(pubkey);
                    let proof = op_identity::session::session_proof(&session_id);
                    cozo_arc.upsert_user(pubkey)?;
                    cozo_arc.upsert_account_session(pubkey, &session_id, &proof)?;
                    cozo_arc.create_session(&session_id, pubkey, None)?;
                    tracing::info!("Registered pre-assigned container account in CozoDB");
                }
            }

            let qdrant = QdrantSemanticShuttle::new().await.ok().map(Arc::new);
            let mut mem_loop = MemoryLoop::new(mem_store);
            if let Some(q) = qdrant {
                tracing::info!("Qdrant semantic shuttle attached to memory loop");
                mem_loop = mem_loop.with_qdrant(q);
            }
            actor.set_memory_loop(Arc::new(mem_loop));
            tracing::info!("Memory loop active (db: {})", db_path);
        }
        Err(e) => {
            tracing::warn!(
                "Memory loop unavailable, chat will run without persistence: {}",
                e
            );
        }
    }

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
