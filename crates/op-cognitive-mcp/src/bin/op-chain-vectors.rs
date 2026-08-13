//! Operator CLI for the blockchain vector pipeline (`blockchain_footprints` only).
//!
//! Strict automatic Qdrant policy applies **only** to this collection — not to
//! `blob_vectors`, user_memory, or RAG shuttle traffic.
//!
//! Origin host (has a Voyage key):
//! ```sh
//! op-chain-vectors status
//! op-chain-vectors project --limit 50              # embed → btrfs vectors only (no Qdrant)
//! op-chain-vectors project --limit 50 --upsert-qdrant  # manual: also write Qdrant directly
//! op-chain-vectors replicate --host offsite.example --remote-path /var/lib/opdbus/chain-replica
//! ```
//!
//! Replica host (no Voyage key needed):
//! ```sh
//! op-chain-vectors ingest                  # Qdrant upsert after btrfs receive (automatic path)
//! ```
//!
//! Configuration is env-driven and shared with the rest of the workspace:
//! `OPDBUS_BLOCKCHAIN_PATH`, `OPDBUS_QDRANT_URL` (or `COGNITIVE_MCP_QDRANT_URL`),
//! `OPDBUS_QDRANT_BLOCKCHAIN_COLLECTION`, `OPDBUS_QDRANT_BLOCKCHAIN_DIM`.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use op_blockchain::{btrfs_delta, StreamingBlockchain};
use op_cognitive_mcp::ChainVectorIndex;

#[derive(Parser)]
#[command(
    name = "op-chain-vectors",
    about = "Embed blockchain blocks into the chain's vector subvolume and index them in Qdrant"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Report chain, vector-coverage and replication state without changing anything.
    Status,
    /// Origin: embed pending blocks into the chain `vectors` subvolume.
    ///
    /// Default does **not** write Qdrant (`blockchain_footprints` is filled by
    /// replica `ingest` after `btrfs receive`). Pass `--upsert-qdrant` for a
    /// manual direct index write that bypasses send/receive.
    Project {
        /// Cap this pass (default: all pending blocks).
        #[arg(long)]
        limit: Option<usize>,
        /// Manual exception: also upsert `blockchain_footprints` on this host.
        /// Does not affect other Qdrant collections.
        #[arg(long, default_value_t = false)]
        upsert_qdrant: bool,
    },
    /// Manual: rebuild `blockchain_footprints` from chain vectors (no Voyage, no receive).
    Reindex,
    /// Replica: index the vectors that arrived since the last indexed block.
    Ingest {
        /// Override the persisted watermark (0 reindexes everything present).
        #[arg(long)]
        since_block: Option<u64>,
    },
    /// Snapshot all three subvolumes under one aligned counter, without sending.
    Snapshot,
    /// Origin: snapshot the chain and send it offsite, incrementally when possible.
    Replicate {
        /// Target host, e.g. `opc@oracle-host`.
        #[arg(long)]
        host: String,
        /// Absolute path of a btrfs directory on the target that receives the subvolumes.
        #[arg(long)]
        remote_path: String,
        /// Absolute path of a program on the target, run as `<program> <counter>`
        /// once the whole triple lands (see deploy/chain-replica-index).
        #[arg(long)]
        on_receive: Option<String>,
    },
    /// Origin: semantic search over indexed blocks.
    Search {
        query: String,
        #[arg(long, default_value_t = 5)]
        limit: u64,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.cmd {
        Cmd::Status => status().await,
        Cmd::Project {
            limit,
            upsert_qdrant,
        } => {
            let index = ChainVectorIndex::open().await?;
            let summary = index.project_pending(limit, upsert_qdrant).await?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
            Ok(())
        }
        Cmd::Reindex => {
            let index = ChainVectorIndex::open_replica().await?;
            let summary = index.reindex_from_chain().await?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
            Ok(())
        }
        Cmd::Ingest { since_block } => {
            let index = ChainVectorIndex::open_replica().await?;
            let summary = index.ingest_received(since_block).await?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
            Ok(())
        }
        Cmd::Snapshot => {
            let chain = open_chain().await?;
            let counter = chain.create_snapshot_aligned().await?;
            for label in op_blockchain::SNAPSHOT_LABELS {
                println!(
                    "{}",
                    op_blockchain::StreamingBlockchain::snapshot_name(label, counter)
                );
            }
            Ok(())
        }
        Cmd::Replicate {
            host,
            remote_path,
            on_receive,
        } => {
            let chain = open_chain().await?;
            let report = chain
                .replicate(&host, &remote_path, on_receive.as_deref())
                .await?;
            println!(
                "counter {} (parent {:?})\n  sent: {}\n  failed: {}",
                report.counter,
                report.parent,
                report.sent.join(", "),
                if report.failed.is_empty() {
                    "none".to_string()
                } else {
                    report
                        .failed
                        .iter()
                        .map(|(name, err)| format!("{name}: {err}"))
                        .collect::<Vec<_>>()
                        .join("; ")
                }
            );
            if let Some(err) = &report.hook_error {
                eprintln!("warning: data landed but the remote index lagged: {err}");
            }
            anyhow::ensure!(
                report.failed.is_empty(),
                "replication incomplete; parent pointer not advanced"
            );
            Ok(())
        }
        Cmd::Search { query, limit } => {
            let index = ChainVectorIndex::open().await?;
            for point in index.search(&query, limit).await? {
                let block = point
                    .payload
                    .get("block_num")
                    .map(|v| v.to_string())
                    .unwrap_or_default();
                let plugin = point
                    .payload
                    .get("plugin_id")
                    .and_then(|v| v.as_str())
                    .cloned()
                    .unwrap_or_default();
                let action = point
                    .payload
                    .get("action")
                    .and_then(|v| v.as_str())
                    .cloned()
                    .unwrap_or_default();
                println!("{:.4}  block {block}  {plugin}/{action}", point.score);
            }
            Ok(())
        }
    }
}

async fn open_chain() -> Result<StreamingBlockchain> {
    let path = std::env::var("OPDBUS_BLOCKCHAIN_PATH")
        .unwrap_or_else(|_| "/var/lib/opdbus/blockchain".to_string());
    StreamingBlockchain::new(&path)
        .await
        .with_context(|| format!("failed to open chain at {path}"))
}

async fn status() -> Result<()> {
    let chain = open_chain().await?;
    let blocks = chain.blocks().await?;
    let with_vectors = blocks.iter().filter(|block| block.has_vector).count();
    let vectors_subvol = chain.vector_subvolume_path();

    println!("chain:            {}", chain.base_path().display());
    println!("blocks:           {}", blocks.len());
    println!("with vectors:     {with_vectors}");
    println!("missing vectors:  {}", blocks.len() - with_vectors);

    match btrfs_delta::generation(vectors_subvol).await {
        Ok(gen) => println!("vectors gen:      {gen}"),
        Err(err) => println!("vectors gen:      unavailable ({err})"),
    }
    match btrfs_delta::received_uuid(vectors_subvol).await {
        Ok(Some(uuid)) => println!("role:             replica (received {uuid})"),
        Ok(None) => println!("role:             origin (locally produced)"),
        Err(err) => println!("role:             unknown ({err})"),
    }

    if let Some(last) = blocks.last() {
        println!(
            "latest block:     {} {}/{}",
            last.block_num, last.category, last.action
        );
    }

    // Index state needs Qdrant; report the chain half regardless of whether it
    // is reachable, so `status` stays useful during an outage.
    match ChainVectorIndex::open_replica().await {
        Ok(index) => {
            println!(
                "indexed through:  block {}",
                index.read_indexed_block().await?
            );
            println!("collection:       {}", index.collection());
        }
        Err(err) => println!("index:            unavailable ({err:#})"),
    }
    Ok(())
}
