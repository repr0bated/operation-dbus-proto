//! Gallery admission for validated specs.
//!
//! Admits validated specs to the catalog store:
//! - Signature deduplication
//! - Stable-core protection
//! - Gallery size management (200 slots)
//! - Immutable versioning

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::validator::ValidationResult;

/// Gallery admission controller.
pub struct GalleryAdmission {
    /// Maximum gallery size (default: 200)
    max_size: usize,

    /// Maximum stable-core elements to protect (default: 40)
    stable_core_max: usize,

    /// Existing signatures for deduplication
    existing_signatures: HashSet<String>,
}

/// Admission result.
#[derive(Debug, Clone)]
pub struct AdmissionResult {
    /// Whether the spec was admitted
    pub admitted: bool,

    /// Reason for rejection (if not admitted)
    pub rejection_reason: Option<String>,

    /// Slot number assigned (if admitted)
    pub slot: Option<usize>,
}

impl GalleryAdmission {
    /// Create a new gallery admission controller.
    pub fn new(max_size: usize, stable_core_max: usize) -> Self {
        Self {
            max_size,
            stable_core_max,
            existing_signatures: HashSet::new(),
        }
    }

    /// Load existing signatures from catalog store.
    pub fn load_existing_signatures(&mut self, signatures: HashSet<String>) {
        self.existing_signatures = signatures;
    }

    /// Attempt to admit a validated spec to the gallery.
    pub fn admit(&self, validation: &ValidationResult) -> AdmissionResult {
        // Check validation
        if !validation.valid {
            return AdmissionResult {
                admitted: false,
                rejection_reason: Some("Spec failed validation".to_string()),
                slot: None,
            };
        }

        // Check signature deduplication
        if self.existing_signatures.contains(&validation.signature) {
            return AdmissionResult {
                admitted: false,
                rejection_reason: Some("Spec signature already exists in gallery".to_string()),
                slot: None,
            };
        }

        // TODO: Check stable-core protection
        // TODO: Find available slot
        // TODO: Admit to catalog store

        AdmissionResult {
            admitted: true,
            rejection_reason: None,
            slot: Some(0), // Placeholder
        }
    }

    /// Find the next available slot in the gallery.
    pub fn find_available_slot(&self) -> Option<usize> {
        // TODO: Query catalog store for available slots
        // For now, return a placeholder
        Some(0)
    }

    /// Check if a slot is in the stable-core protected range.
    pub fn is_stable_core_slot(&self, slot: usize) -> bool {
        slot < self.stable_core_max
    }

    /// Get gallery statistics.
    pub fn stats(&self) -> GalleryStats {
        GalleryStats {
            max_size: self.max_size,
            stable_core_max: self.stable_core_max,
            current_size: self.existing_signatures.len(),
            available_slots: self.max_size.saturating_sub(self.existing_signatures.len()),
        }
    }
}

/// Gallery statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GalleryStats {
    pub max_size: usize,
    pub stable_core_max: usize,
    pub current_size: usize,
    pub available_slots: usize,
}

impl Default for GalleryAdmission {
    fn default() -> Self {
        Self::new(200, 40)
    }
}
