//! Gallery fill-loop runner.
//!
//! Orchestrates the generation session: assembles context, runs the inference
//! loop over empty slots, validates specs, and reports progress. Designed to
//! be spawned as a background tokio task from the HTTP handler.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::Result;

use crate::admission::{try_admit, GalleryStore};
use crate::context::GenerationContext;
use crate::inference::InferenceLoop;
use crate::validator::SpecValidator;
use crate::GalleryGenConfig;

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

/// Run the gallery fill loop.
///
/// This is the core async function spawned by the HTTP handler. It:
/// 1. Iterates over empty slots up to `config.target_count`
/// 2. Calls the inference loop for each slot (up to 3 retries per slot)
/// 3. Validates each generated spec
/// 4. Admits valid specs to the gallery store (with signature dedup)
/// 5. Reports progress via `progress` atomics
/// 6. Respects `stop_signal` for operator cancellation
///
/// The `store` parameter is the gallery store implementation — either the real
/// `CatalogStore` wrapper (from op-web) or `InMemoryGalleryStore` for testing.
///
/// Returns the number of specs successfully generated.
pub async fn run_gallery_fill(
    config: &GalleryGenConfig,
    ctx: GenerationContext,
    store: Arc<dyn GalleryStore>,
    progress: Arc<RunProgress>,
) -> Result<usize> {
    let inference = InferenceLoop::new(config.zeroclaw_endpoint.clone(), config.max_turns);
    let validator = SpecValidator::new();

    let target = config.target_count;
    let max_retries_per_slot: usize = 3;
    let max_total_attempts = target * max_retries_per_slot;

    let mut generated: usize = 0;
    let mut total_attempts: usize = 0;

    let stats = store.stats();
    tracing::info!(
        "Starting gallery fill: target={}, endpoint={}, mcp={}, qdrant={}, catalog_hash={}, gallery={}/{} ({} stable core)",
        target,
        config.zeroclaw_endpoint,
        config.enable_mcp,
        config.enable_qdrant,
        &ctx.catalog_hash[..ctx.catalog_hash.len().min(12)],
        stats.current_size,
        stats.max_size,
        stats.stable_core_count,
    );

    while generated < target && total_attempts < max_total_attempts {
        // Check cancellation
        if progress.is_cancelled() {
            tracing::info!("Generation cancelled by operator after {} specs", generated);
            break;
        }

        total_attempts += 1;
        progress.attempts.store(total_attempts, Ordering::SeqCst);

        tracing::debug!(
            "Slot attempt {}/{} (generated {}/{})",
            total_attempts,
            max_total_attempts,
            generated,
            target
        );

        // Call inference
        match inference.generate(&ctx).await {
            Ok(gen_spec) => {
                // Validate
                let validation = validator.validate(&gen_spec.spec);

                if validation.valid {
                    // Attempt admission (handles dedup + capacity + retirement)
                    let admit_result = try_admit(store.as_ref(), &validation, gen_spec.spec);

                    if admit_result.admitted {
                        generated += 1;
                        progress.generated.store(generated, Ordering::SeqCst);
                        tracing::info!(
                            "Spec {}/{} admitted as '{}' (signature: {}..)",
                            generated,
                            target,
                            admit_result.element_id.as_deref().unwrap_or("?"),
                            &validation.signature[..validation.signature.len().min(12)]
                        );
                    } else {
                        tracing::warn!(
                            "Spec admission rejected (attempt {}): {}",
                            total_attempts,
                            admit_result.rejection_reason.as_deref().unwrap_or("unknown")
                        );
                    }
                } else {
                    tracing::warn!(
                        "Spec validation failed (attempt {}): {}",
                        total_attempts,
                        validation
                            .errors
                            .iter()
                            .map(|e| format!("[{}] {}", e.code, e.message))
                            .collect::<Vec<_>>()
                            .join("; ")
                    );
                }
            }
            Err(e) => {
                tracing::warn!("Inference failed (attempt {}): {}", total_attempts, e);
            }
        }
    }

    progress.finish();

    tracing::info!(
        "Gallery fill complete: {}/{} specs generated in {} attempts",
        generated,
        target,
        total_attempts
    );

    Ok(generated)
}
