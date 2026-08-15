//! Spec validator for gallery admission.
//!
//! Two layers, deliberately separable:
//!
//! - **Grammar** (always): root exists, `elements` is an object, every child
//!   reference resolves, no cycles, plus the dedup signature. These hold for any
//!   json-render spec regardless of which components a host app declares.
//! - **Vocabulary** (when a [`CatalogGuard`] is attached): which components
//!   exist, which props they take, which accept children. This crate cannot know
//!   that on its own — it belongs to the app's catalog — so it is loaded from the
//!   catalog's own export rather than restated here. See [`crate::catalog_guard`].
//!
//! A validator built with [`SpecValidator::new`] checks grammar only, and will
//! admit a spec naming a component no renderer has. Admission paths must attach
//! a catalog.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::catalog_guard::CatalogGuard;

/// Spec validation result.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the spec is valid
    pub valid: bool,

    /// Validation errors (if any)
    pub errors: Vec<ValidationError>,

    /// Spec signature for deduplication
    pub signature: String,
}

/// Validation error.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub code: String,
    pub message: String,
}

/// Spec validator.
pub struct SpecValidator {
    /// The component vocabulary, when one has been supplied. `None` means
    /// grammar-only validation.
    catalog: Option<CatalogGuard>,
}

impl SpecValidator {
    /// Create a grammar-only validator.
    ///
    /// Structure, references and cycles are checked; component names and props
    /// are not, because without a catalog there is nothing to check them
    /// against. Use [`Self::with_catalog`] for admission.
    pub fn new() -> Self {
        Self { catalog: None }
    }

    /// Create a validator that also enforces a catalog's vocabulary.
    pub fn with_catalog(catalog: CatalogGuard) -> Self {
        Self {
            catalog: Some(catalog),
        }
    }

    /// Digest of the catalog artifact in force, if any.
    ///
    /// Worth logging next to a spec's signature: it records which vocabulary the
    /// spec was admitted against.
    pub fn catalog_hash(&self) -> Option<&str> {
        self.catalog.as_ref().map(CatalogGuard::hash)
    }

    /// Validate a spec.
    pub fn validate(&self, spec: &serde_json::Value) -> ValidationResult {
        let mut errors = Vec::new();

        // 1. Structure check
        let root_id = match self.check_structure(spec) {
            Ok(id) => id,
            Err(e) => {
                errors.push(e);
                return ValidationResult {
                    valid: false,
                    errors,
                    signature: String::new(),
                };
            }
        };

        let elements = match spec.get("elements").and_then(|e| e.as_object()) {
            Some(obj) => obj,
            None => {
                errors.push(ValidationError {
                    code: "E_MISSING_ELEMENTS".to_string(),
                    message: "Spec missing 'elements' object".to_string(),
                });
                return ValidationResult {
                    valid: false,
                    errors,
                    signature: String::new(),
                };
            }
        };

        // 2. Reference check
        if let Err(e) = self.check_references(root_id.clone(), elements) {
            errors.push(e);
        }

        // 3. Element shape: `type` is required by the grammar itself.
        for (id, element) in elements {
            if let Err(e) = self.check_type_present(id, element) {
                errors.push(e);
            }
        }

        // 4. Vocabulary: component names, props, slots, binds, visibility.
        if let Some(catalog) = &self.catalog {
            for (id, element) in elements {
                catalog.check_element(id, element, &mut errors);
            }
        }

        // 5. Cycle check
        if let Err(e) = self.check_cycles(&root_id, elements) {
            errors.push(e);
        }

        // 6. Generate signature
        let signature = self.compute_signature(spec);

        ValidationResult {
            valid: errors.is_empty(),
            errors,
            signature,
        }
    }

    /// Check top-level structure.
    fn check_structure(&self, spec: &serde_json::Value) -> Result<String, ValidationError> {
        if !spec.is_object() {
            return Err(ValidationError {
                code: "E_INVALID_SPEC".to_string(),
                message: "Spec must be a JSON object".to_string(),
            });
        }

        let root_id = spec
            .get("root")
            .and_then(|r| r.as_str())
            .ok_or_else(|| ValidationError {
                code: "E_MISSING_ROOT".to_string(),
                message: "Spec missing 'root' field".to_string(),
            })?
            .to_string();

        if !spec.get("elements").map_or(false, |e| e.is_object()) {
            return Err(ValidationError {
                code: "E_MISSING_ELEMENTS".to_string(),
                message: "Spec missing 'elements' object".to_string(),
            });
        }

        Ok(root_id)
    }

    /// Check that all element references exist.
    fn check_references(
        &self,
        root_id: String,
        elements: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), ValidationError> {
        // Check root exists
        if !elements.contains_key(&root_id) {
            return Err(ValidationError {
                code: "E_MISSING_ROOT".to_string(),
                message: format!("Root element '{}' not found in elements", root_id),
            });
        }

        // Check all children references
        for (id, element) in elements {
            if let Some(children) = element.get("children").and_then(|c| c.as_array()) {
                for child_id in children {
                    if let Some(child_str) = child_id.as_str() {
                        if !elements.contains_key(child_str) {
                            return Err(ValidationError {
                                code: "E_DANGLING_REF".to_string(),
                                message: format!(
                                    "Element '{}' references non-existent child '{}'",
                                    id, child_str
                                ),
                            });
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Check that an element declares a `type`.
    ///
    /// Whether that type exists is a catalog question, answered by
    /// [`CatalogGuard::check_element`]; that it is present at all is grammar.
    fn check_type_present(
        &self,
        id: &str,
        element: &serde_json::Value,
    ) -> Result<(), ValidationError> {
        match element.get("type").and_then(|t| t.as_str()) {
            Some(_) => Ok(()),
            None => Err(ValidationError {
                code: "E_MISSING_TYPE".to_string(),
                message: format!("Element '{}' missing 'type' field", id),
            }),
        }
    }

    /// Check for cycles in element tree.
    fn check_cycles(
        &self,
        root_id: &str,
        elements: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), ValidationError> {
        let mut visited = HashSet::new();
        let mut path = Vec::new();

        self.detect_cycle(root_id, elements, &mut visited, &mut path)
    }

    /// Recursive cycle detection.
    fn detect_cycle(
        &self,
        current_id: &str,
        elements: &serde_json::Map<String, serde_json::Value>,
        visited: &mut HashSet<String>,
        path: &mut Vec<String>,
    ) -> Result<(), ValidationError> {
        if path.contains(&current_id.to_string()) {
            let cycle_path = path.join(" -> ");
            return Err(ValidationError {
                code: "E_CYCLE".to_string(),
                message: format!("Cycle detected: {} -> {}", cycle_path, current_id),
            });
        }

        if visited.contains(current_id) {
            return Ok(());
        }

        visited.insert(current_id.to_string());
        path.push(current_id.to_string());

        if let Some(element) = elements.get(current_id) {
            if let Some(children) = element.get("children").and_then(|c| c.as_array()) {
                for child_id in children {
                    if let Some(child_str) = child_id.as_str() {
                        self.detect_cycle(child_str, elements, visited, path)?;
                    }
                }
            }
        }

        path.pop();
        Ok(())
    }

    /// Compute signature for deduplication.
    fn compute_signature(&self, spec: &serde_json::Value) -> String {
        use sha2::{Digest, Sha256};

        // Normalize spec by sorting keys
        let normalized = self.normalize_json(spec);
        let json_str = serde_json::to_string(&normalized).unwrap_or_default();

        let mut hasher = Sha256::new();
        hasher.update(json_str.as_bytes());
        let hash = hasher.finalize();

        hex::encode(hash)
    }

    /// Normalize JSON for consistent hashing.
    fn normalize_json(&self, value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::Object(obj) => {
                let mut sorted: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
                let mut keys: Vec<_> = obj.keys().collect();
                keys.sort();

                for key in keys {
                    sorted.insert(key.clone(), self.normalize_json(&obj[key]));
                }

                serde_json::Value::Object(sorted)
            }
            serde_json::Value::Array(arr) => {
                serde_json::Value::Array(arr.iter().map(|v| self.normalize_json(v)).collect())
            }
            _ => value.clone(),
        }
    }
}

impl Default for SpecValidator {
    fn default() -> Self {
        Self::new()
    }
}
