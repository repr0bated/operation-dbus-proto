//! Background embedding worker
//!
//! Consumes EmbedRequests from the channel produced by OptimizedBlockchain::add_footprint(),
//! calls the embedding provider, and upserts vectors into Qdrant.
//!
//! This is plain runtime processing — not audit flow, not persistent memory.
//! If the process dies with requests in-flight, they are lost. That is acceptable:
//! the blockchain timing subvolume has the full history for any future reindex.

use anyhow::Result;
use op_blockchain::EmbedRequest;
use op_llm::provider::{EmbeddingIntent, EmbeddingProvider};
use qdrant_client::qdrant::{
    CreateCollectionBuilder, Distance, PointStruct, UpsertPointsBuilder, VectorParamsBuilder,
};
use qdrant_client::Payload;
use qdrant_client::Qdrant;
use simd_json::prelude::{TypedScalarValue, ValueAsContainer, ValueAsScalar};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

const MAX_RETRIES: u32 = 5;
const RETRY_BASE_MS: u64 = 500;

/// Spawn the background embedding worker. Returns the sender end of the channel.
/// Channel capacity: 1024 — try_send in add_footprint() drops silently if full.
pub fn spawn_embedding_worker(
    embedder: Arc<dyn EmbeddingProvider>,
    qdrant: Arc<Qdrant>,
) -> mpsc::Sender<EmbedRequest> {
    let (tx, rx) = mpsc::channel::<EmbedRequest>(1024);

    tokio::spawn(run_worker(rx, embedder, qdrant));

    info!("Embedding worker spawned (capacity=1024)");
    tx
}

async fn run_worker(
    mut rx: mpsc::Receiver<EmbedRequest>,
    embedder: Arc<dyn EmbeddingProvider>,
    qdrant: Arc<Qdrant>,
) {
    info!("Embedding worker started");

    while let Some(req) = rx.recv().await {
        debug!("Embedding worker: processing block {}", req.block_hash);

        if let Err(e) = process_with_retry(&req, &embedder, &qdrant).await {
            warn!(
                "Embedding worker: failed to process block {} after retries: {}",
                req.block_hash, e
            );
        }
    }

    info!("Embedding worker stopped — channel closed");
}

async fn process_with_retry(
    req: &EmbedRequest,
    embedder: &Arc<dyn EmbeddingProvider>,
    qdrant: &Arc<Qdrant>,
) -> Result<()> {
    let mut last_err = None;

    for attempt in 0..MAX_RETRIES {
        if attempt > 0 {
            let delay = RETRY_BASE_MS * (1 << (attempt - 1).min(4));
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }

        match embed_and_upsert(req, embedder, qdrant).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                warn!(
                    "Embedding attempt {}/{} failed for {}: {}",
                    attempt + 1,
                    MAX_RETRIES,
                    req.block_hash,
                    e
                );
                last_err = Some(e);
            }
        }
    }

    Err(last_err.unwrap())
}

async fn embed_and_upsert(
    req: &EmbedRequest,
    embedder: &Arc<dyn EmbeddingProvider>,
    qdrant: &Arc<Qdrant>,
) -> Result<()> {
    // Embed the text
    let result = embedder
        .embed(req.embedding_text.clone(), EmbeddingIntent::Document)
        .await?;

    // Ensure collection exists with the correct dimensions
    ensure_collection_exists(qdrant, &req.collection, result.vector.len()).await?;

    // Build Qdrant payload from the EmbedRequest payload JSON
    let qdrant_payload = json_to_payload(&req.payload);

    // Upsert — use block_hash as the point ID (string UUID-style)
    let point = PointStruct::new(req.block_hash.clone(), result.vector, qdrant_payload);

    qdrant
        .upsert_points(UpsertPointsBuilder::new(&req.collection, vec![point]))
        .await?;

    info!(
        "Embedded and upserted block {} into {}",
        req.block_hash, req.collection
    );

    Ok(())
}

fn json_to_payload(val: &simd_json::OwnedValue) -> Payload {
    use qdrant_client::qdrant::value::Kind;
    use qdrant_client::qdrant::Value as QValue;

    let mut map = std::collections::HashMap::new();
    if let Some(obj) = val.as_object() {
        for (k, v) in obj.iter() {
            let kind = if v.is_null() {
                Kind::NullValue(0)
            } else if let Some(b) = v.as_bool() {
                Kind::BoolValue(b)
            } else if let Some(n) = v.as_i64() {
                Kind::IntegerValue(n)
            } else if let Some(n) = v.as_u64() {
                Kind::IntegerValue(n as i64)
            } else if let Some(f) = v.as_f64() {
                Kind::DoubleValue(f)
            } else if let Some(s) = v.as_str() {
                Kind::StringValue(s.to_string())
            } else {
                continue;
            };
            map.insert(k.clone(), QValue { kind: Some(kind) });
        }
    }
    Payload::from(map)
}

async fn ensure_collection_exists(client: &Qdrant, collection: &str, dims: usize) -> Result<()> {
    match client.collection_exists(collection).await {
        Ok(true) => return Ok(()),
        Ok(false) => {}
        Err(err) => {
            warn!(
                "failed to check Qdrant collection existence for '{}': {}",
                collection, err
            );
        }
    }

    client
        .create_collection(
            CreateCollectionBuilder::new(collection)
                .vectors_config(VectorParamsBuilder::new(dims as u64, Distance::Cosine)),
        )
        .await?;

    Ok(())
}
