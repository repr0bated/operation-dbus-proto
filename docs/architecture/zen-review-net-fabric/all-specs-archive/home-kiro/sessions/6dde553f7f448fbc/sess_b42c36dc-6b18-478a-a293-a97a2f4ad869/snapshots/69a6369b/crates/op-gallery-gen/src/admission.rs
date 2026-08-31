//! Gallery admission for validated specs.
//!
//! Defines the `GalleryStore` trait that abstracts over the actual catalog
//! store implementation. The HTTP handler provides a concrete implementation
//! wrapping `CatalogStore`; the generation loop uses the trait.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::validator::ValidationResult;

/// Trait abstracting the gallery store for spec admission.
///
/// The generation crate cannot depend on `op-web` (circular), so we define
/// this trait here and the HTTP handler provides a concrete implementation.
pub trait GalleryStore: Send + Sync {
    /// Admit a validated spec to the gallery.
    ///
    /// Returns `Ok(element_id)` if admitted, or `Err(reason)` if rejected
    /// (duplicate signature, gallery full with no retirable novelty, etc.).
    fn admit_spec(
        &self,
        spec: serde_json::Value,
        signature: &str,
        title: String,
    ) -> Result<String, String>;

    /// Check if a signature already exists in the gallery.
    fn has_signature(&self, signature: &str) -> bool;

    /// Get current gallery statistics.
    fn stats(&self) -> GalleryStats;

    /// Retire the oldest novelty element to make room.
    /// Returns the ID of the retired element, or None if no retirable element exists.
    fn retire_oldest_novelty(&self) -> Option<String>;
}

/// Gallery statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GalleryStats {
    /// Maximum gallery size (200).
    pub max_size: usize,
    /// Maximum stable-core elements (40).
    pub stable_core_max: usize,
    /// Current number of active elements.
    pub current_size: usize,
    /// Number of stable-core elements.
    pub stable_core_count: usize,
    /// Number of novelty elements.
    pub novelty_count: usize,
    /// Available slots (max_size - current_size).
    pub available_slots: usize,
}

/// A no-op gallery store for testing or when CatalogStore is unavailable.
///
/// Accepts all specs and tracks signatures in memory.
pub struct InMemoryGalleryStore {
    signatures: std::sync::RwLock<HashSet<String>>,
    admitted_count: std::sync::atomic::AtomicUsize,
}

impl InMemoryGalleryStore {
    pub fn new() -> Self {
        Self {
            signatures: std::sync::RwLock::new(HashSet::new()),
            admitted_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

impl Default for InMemoryGalleryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl GalleryStore for InMemoryGalleryStore {
    fn admit_spec(
        &self,
        _spec: serde_json::Value,
        signature: &str,
        _title: String,
    ) -> Result<String, String> {
        let mut sigs = self.signatures.write().unwrap();
        if sigs.contains(signature) {
            return Err("E_DUPLICATE_SIGNATURE: Spec signature already exists".to_string());
        }
        sigs.insert(signature.to_string());
        let count = self
            .admitted_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let id = format!("gen-spec-{:04}", count + 1);
        Ok(id)
    }

    fn has_signature(&self, signature: &str) -> bool {
        self.signatures.read().unwrap().contains(signature)
    }

    fn stats(&self) -> GalleryStats {
        let count = self
            .admitted_count
            .load(std::sync::atomic::Ordering::SeqCst);
        GalleryStats {
            max_size: 200,
            stable_core_max: 40,
            current_size: count,
            stable_core_count: 0,
            novelty_count: count,
            available_slots: 200_usize.saturating_sub(count),
        }
    }

    fn retire_oldest_novelty(&self) -> Option<String> {
        // In-memory store doesn't track order, so just decrement
        let count = self
            .admitted_count
            .load(std::sync::atomic::Ordering::SeqCst);
        if count > 0 {
            self.admitted_count
                .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            Some(format!("retired-gen-spec-{:04}", count))
        } else {
            None
        }
    }
}

/// Attempt admission of a validated spec through the gallery store.
///
/// This is the high-level function called by the fill loop:
/// 1. Check signature dedup
/// 2. Check gallery capacity (retire oldest novelty if full)
/// 3. Admit the spec
pub fn try_admit(
    store: &dyn GalleryStore,
    validation: &ValidationResult,
    spec: serde_json::Value,
) -> AdmissionResult {
    if !validation.valid {
        return AdmissionResult {
            admitted: false,
            element_id: None,
            rejection_reason: Some("Spec failed validation".to_string()),
        };
    }

    // Signature dedup
    if store.has_signature(&validation.signature) {
        return AdmissionResult {
            admitted: false,
            element_id: None,
            rejection_reason: Some("E_DUPLICATE_SIGNATURE: Spec signature already exists in gallery".to_string()),
        };
    }

    // Check capacity
    let stats = store.stats();
    if stats.available_slots == 0 {
        // Try to retire oldest novelty
        if store.retire_oldest_novelty().is_none() {
            return AdmissionResult {
                admitted: false,
                element_id: None,
                rejection_reason: Some("Gallery full — no retirable novelty elements".to_string()),
            };
        }
    }

    // Generate a title from the spec (use root element type or fallback)
    let title = spec
        .get("elements")
        .and_then(|e| e.as_object())
        .and_then(|elems| {
            let root_id = spec.get("root").and_then(|r| r.as_str())?;
            elems.get(root_id)
        })
        .and_then(|root_el| root_el.get("type"))
        .and_then(|t| t.as_str())
        .map(|t| format!("gen-{}", t))
        .unwrap_or_else(|| "gen-spec".to_string());

    // Admit
    match store.admit_spec(spec, &validation.signature, title) {
        Ok(id) => AdmissionResult {
            admitted: true,
            element_id: Some(id),
            rejection_reason: None,
        },
        Err(reason) => AdmissionResult {
            admitted: false,
            element_id: None,
            rejection_reason: Some(reason),
        },
    }
}

/// Result of an admission attempt.
#[derive(Debug, Clone)]
pub struct AdmissionResult {
    /// Whether the spec was admitted.
    pub admitted: bool,
    /// Element ID assigned (if admitted).
    pub element_id: Option<String>,
    /// Reason for rejection (if not admitted).
    pub rejection_reason: Option<String>,
}
