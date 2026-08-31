//! Gallery fill-loop runner.
//!
//! Orchestrates the generation session: assembles context, runs the inference
//! loop over empty slots, validates specs, and reports progress. Designed to
//! be spawned as a background tokio task from the HTTP handler.
//!
//! Features:
//! - Up to 4 concurrent inference calls per batch (design spec parallelism)
//! - Catalog hash freshness check (NFR-4: log if catalog changes mid-run)
//! - Operator cancellation via stop_signal
//! - Sequential admission to prevent signature-hash races

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::Result;
use tokio::task::JoinSet;

use crate::admission::{try_admit, GalleryStore};
use crate::context::{read_catalog_hash_from_shm, GenerationContext};
use crate::inference::InferenceLoop;
use crate::validator::SpecValidator;
use crate::GalleryGenConfig;

/// Maximum concurrent inference calls per batch.
const MAX_CONCURRENCY: usize = 4;

/// Check catalog freshness every N attempts.
const FRESHNESS_CHECK_INTERVAL: usize = 20;

/// Progress state shared between the runner and the SSE stream.
///
/// All fields are atomics so the SSE poller can read them without locking.
pub struct RunProgress {
    /// Whether the run is currently active.
    pub running: AtomicBool,
    /// Number of specs successfully admitted to the gallery.
    pub generated: AtomicUsize,
    /// Total inference attempts made (includes retries).
    pub attempts: AtomicUsize,
    /// Target number of specs to generate.
    pub target: AtomicUsize,
    /// Operator cancel signal — when true, the loop exits gracefully.
    pub stop_signal: AtomicBool,
}

impl RunProgress {
    pub fn new() -> Self {
        Self {
            running: AtomicBool::new(false),
            generated: AtomicUsize::new(0),
            attempts: AtomicUsize::new(0),
            target: AtomicUsize::new(0),
            stop_signal: AtomicBool::new(false),
        }
    }

    /// Reset all counters for a new run.
    pub fn reset(&self, target: usize) {
        self.running.store(true, Ordering::SeqCst);
        self.generated.store(0, Ordering::SeqCst);
        self.attempts.store(0, Ordering::SeqCst);
        self.target.store(target, Ordering::SeqCst);
        self.stop_signal.store(false, Ordering::SeqCst);
    }

    /// Mark the run as complete.
    pub fn finish(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Check if the operator has requested cancellation.
    pub fn is_cancelled(&self) -> bool {
        self.stop_signal.load(Ordering::SeqCst)
    }
}

impl Default for RunProgress {
    fn default() -> Self {
        Self::new()
    }
}

/// Run the gallery fill loop with parallelism and freshness checks.
///
/// This is the core async function spawned by the HTTP handler. It:
/// 1. Dispatches up to 4 concurrent inference calls per batch
/// 2. Collects results and admits sequentially (prevents signature-hash races)
/// 3. Checks catalog freshness every 20 attempts (NFR-4)
/// 4. Respects `stop_signal` for operator cancellation
///
/// Returns the number of specs successfully generated.
pub async fn run_gallery_fill(
    config: &GalleryGenConfig,
    ctx: GenerationContext,
    store: Arc<dyn GalleryStore>,
    progress: Arc<RunProgress>,
) -> Result<usize> {
    let inference = Arc::new(InferenceLoop::new(
        config.zeroclaw_endpoint.clone(),
        config.max_turns,
    ));
    let validator = SpecValidator::new();

    let target = config.target_count;
    let max_retries_per_slot: usize = 3;
    let max_total_attempts = target * max_retries_per_slot;

    let mut generated: usize = 0;
    let mut total_attempts: usize = 0;
    let mut catalog_changed_logged = false;

    let stats = store.stats();
    tracing::info!(
        "Starting gallery fill: target={}, concurrency={}, endpoint={}, mcp={}, qdrant={}, \
         catalog_hash={}, gallery={}/{} ({} stable core)",
        target,
        MAX_CONCURRENCY,
        config.zeroclaw_endpoint,
        config.enable_mcp,
        config.enable_qdrant,
        &ctx.catalog_hash[..ctx.catalog_hash.len().min(12)],
        stats.current_size,
        stats.max_size,
        stats.stable_core_count,
    );

    // Main fill loop — dispatches batches of concurrent inference calls
    while generated < target && total_attempts < max_total_attempts {
        // Check cancellation before each batch
        if progress.is_cancelled() {
            tracing::info!("Generation cancelled by operator after {} specs", generated);
            break;
        }

        // Catalog freshness check (NFR-4)
        if !catalog_changed_logged
            && total_attempts > 0
            && total_attempts % FRESHNESS_CHECK_INTERVAL == 0
        {
            if let Some(current_hash) = read_catalog_hash_from_shm() {
                if current_hash != ctx.catalog_hash {
                    tracing::warn!(
                        "Catalog changed mid-run (started={}, now={}). \
                         Continuing with original catalog per NFR-4. \
                         Next run will pick up the new catalog.",
                        &ctx.catalog_hash[..ctx.catalog_hash.len().min(12)],
                        &current_hash[..current_hash.len().min(12)],
                    );
                    catalog_changed_logged = true;
                }
            }
        }

        // Determine batch size: min of (remaining target, remaining attempts, concurrency cap)
        let remaining_target = target - generated;
        let remaining_attempts = max_total_attempts - total_attempts;
        let batch_size = remaining_target
            .min(remaining_attempts)
            .min(MAX_CONCURRENCY);

        if batch_size == 0 {
            break;
        }

        // Dispatch concurrent inference calls
        let mut join_set = JoinSet::new();
        let ctx_arc = Arc::new(ctx.clone());

        for _ in 0..batch_size {
            let inf = Arc::clone(&inference);
            let ctx_clone = Arc::clone(&ctx_arc);

            join_set.spawn(async move { inf.generate(&ctx_clone).await });
        }

        total_attempts += batch_size;
        progress.attempts.store(total_attempts, Ordering::SeqCst);

        // Collect results and admit SEQUENTIALLY (prevents signature-hash races)
        while let Some(result) = join_set.join_next().await {
            // Check cancellation between admissions
            if progress.is_cancelled() {
                tracing::info!(
                    "Generation cancelled by operator during batch (after {} specs)",
                    generated
                );
                // Abort remaining tasks in the join set
                join_set.abort_all();
                progress.finish();
                return Ok(generated);
            }

            match result {
                Ok(Ok(gen_spec)) => {
                    let validation = validator.validate(&gen_spec.spec);

                    if validation.valid {
                        let admit_result =
                            try_admit(store.as_ref(), &validation, gen_spec.spec);

                        if admit_result.admitted {
                            generated += 1;
                            progress.generated.store(generated, Ordering::SeqCst);
                            tracing::info!(
                                "Spec {}/{} admitted as '{}' (sig: {}..)",
                                generated,
                                target,
                                admit_result.element_id.as_deref().unwrap_or("?"),
                                &validation.signature[..validation.signature.len().min(12)]
                            );

                            // Stop early if we've hit the target
                            if generated >= target {
                                join_set.abort_all();
                                break;
                            }
                        } else {
                            tracing::warn!(
                                "Admission rejected: {}",
                                admit_result
                                    .rejection_reason
                                    .as_deref()
                                    .unwrap_or("unknown")
                            );
                        }
                    } else {
                        tracing::warn!(
                            "Validation failed: {}",
                            validation
                                .errors
                                .iter()
                                .map(|e| format!("[{}] {}", e.code, e.message))
                                .collect::<Vec<_>>()
                                .join("; ")
                        );
                    }
                }
                Ok(Err(e)) => {
                    tracing::warn!("Inference failed: {}", e);
                }
                Err(e) => {
                    // JoinError — task panicked or was cancelled
                    tracing::error!("Inference task failed: {}", e);
                }
            }
        }
    }

    progress.finish();

    tracing::info!(
        "Gallery fill complete: {}/{} specs generated in {} attempts{}",
        generated,
        target,
        total_attempts,
        if catalog_changed_logged {
            " (catalog changed mid-run)"
        } else {
            ""
        }
    );

    Ok(generated)
}
