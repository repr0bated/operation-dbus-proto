//! Spec validator for gallery admission.
//!
//! Validates generated specs against the json-render.dev grammar:
//! - Structure checks (root, elements, references)
//! - Type checks (known components)
//! - Prop schema validation
//! - Children validation
//! - Bind path validation
//! - Cycle detection
//! - Signature deduplication

use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

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
    /// Known component types
    stable_core: HashSet<&'static str>,
}

impl SpecValidator {
    /// Create a new spec validator.
    pub fn new() -> Self {
        Self {
            stable_core: [
                // Layout
                "stack", "card", "separator", "space",
                // Text
                "heading", "label", "muted",
                // Status
                "status_pill",
                // Action
                "button", "button_group",
                // Data Display
                "kv_pair", "table", "log_stream", "flow_table", "metric_card",
                // Form
                "text_input", "number_input", "select", "toggle",
                // Dynamic
                "repeat", "schema_form",
            ].iter().cloned().collect(),
        }
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
        if let Err(e) = self.check_references(root_id, elements) {
            errors.push(e);
        }
        
        // 3. Type check
        for (id, element) in elements {
            if let Err(e) = self.check_type(id, element) {
                errors.push(e);
            }
        }
        
        // 4. Prop schema check
        for (id, element) in elements {
            if let Err(e) = self.check_props(id, element) {
                errors.push(e);
            }
        }
        
        // 5. Children check
        for (id, element) in elements {
            if let Err(e) = self.check_children(id, element, elements) {
                errors.push(e);
            }
        }
        
        // 6. Bind path check
        for (id, element) in elements {
            if let Err(e) = self.check_bind_paths(id, element) {
                errors.push(e);
            }
        }
        
        // 7. Cycle check
        if let Err(e) = self.check_cycles(root_id, elements) {
            errors.push(e);
        }
        
        // 8. Generate signature
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
        
        let root_id = spec.get("root")
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
    
    /// Check that element type is known.
    fn check_type(&self, id: &str, element: &serde_json::Value) -> Result<(), ValidationError> {
        let type_name = element.get("type")
            .and_then(|t| t.as_str())
            .ok_or_else(|| ValidationError {
                code: "E_MISSING_TYPE".to_string(),
                message: format!("Element '{}' missing 'type' field", id),
            })?;
        
        if !self.stable_core.contains(type_name) {
            // Allow novelty types with a warning (not an error)
            tracing::warn!("Element '{}' uses novelty type: {}", id, type_name);
        }
        
        Ok(())
    }
    
    /// Check props against component schemas.
    fn check_props(&self, id: &str, element: &serde_json::Value) -> Result<(), ValidationError> {
        let type_name = element.get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("");
        
        let props = element.get("props").and_then(|p| p.as_object());
        
        match type_name {
            "heading" => {
                if let Some(p) = props {
                    if !p.contains_key("text") && !p.contains_key("bind") {
                        return Err(ValidationError {
                            code: "E_PROP_SCHEMA".to_string(),
                            message: format!("Element '{}' (heading) requires 'text' or 'bind' prop", id),
                        });
                    }
                }
            }
            "label" => {
                if let Some(p) = props {
                    if !p.contains_key("text") && !p.contains_key("bind") {
                        return Err(ValidationError {
                            code: "E_PROP_SCHEMA".to_string(),
                            message: format!("Element '{}' (label) requires 'text' or 'bind' prop", id),
                        });
                    }
                }
            }
            "status_pill" => {
                if let Some(p) = props {
                    if !p.contains_key("bind") {
                        return Err(ValidationError {
                            code: "E_PROP_SCHEMA".to_string(),
                            message: format!("Element '{}' (status_pill) requires 'bind' prop", id),
                        });
                    }
                }
            }
            "button" => {
                if let Some(p) = props {
                    if !p.contains_key("label") {
                        return Err(ValidationError {
                            code: "E_PROP_SCHEMA".to_string(),
                            message: format!("Element '{}' (button) requires 'label' prop", id),
                        });
                    }
                }
            }
            "table" => {
                if let Some(p) = props {
                    if !p.contains_key("bind") {
                        return Err(ValidationError {
                            code: "E_PROP_SCHEMA".to_string(),
                            message: format!("Element '{}' (table) requires 'bind' prop", id),
                        });
                    }
                    if !p.contains_key("columns") {
                        return Err(ValidationError {
                            code: "E_PROP_SCHEMA".to_string(),
                            message: format!("Element '{}' (table) requires 'columns' prop", id),
                        });
                    }
                }
            }
            _ => {}
        }
        
        Ok(())
    }
    
    /// Check children are valid for each component.
    fn check_children(
        &self,
        id: &str,
        element: &serde_json::Value,
        elements: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), ValidationError> {
        let type_name = element.get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("");
        
        let children = element.get("children").and_then(|c| c.as_array());
        
        // Components that don't accept children
        let no_children = [
            "heading", "label", "muted", "status_pill", "button",
            "separator", "space", "kv_pair", "table", "log_stream",
            "flow_table", "metric_card", "text_input", "number_input",
            "select", "toggle",
        ];
        
        if no_children.contains(&type_name) && children.is_some() {
            return Err(ValidationError {
                code: "E_CHILDREN_NOT_ALLOWED".to_string(),
                message: format!("Component '{}' does not accept children", type_name),
            });
        }
        
        // Components that require children
        let requires_children = ["stack", "card"];
        
        if requires_children.contains(&type_name) {
            match children {
                Some(arr) if arr.is_empty() => {
                    return Err(ValidationError {
                        code: "E_CHILDREN_REQUIRED".to_string(),
                        message: format!("Component '{}' requires non-empty children", type_name),
                    });
                }
                None => {
                    return Err(ValidationError {
                        code: "E_CHILDREN_REQUIRED".to_string(),
                        message: format!("Component '{}' requires children", type_name),
                    });
                }
                _ => {}
            }
        }
        
        Ok(())
    }
    
    /// Check bind paths start with '/'.
    fn check_bind_paths(&self, id: &str, element: &serde_json::Value) -> Result<(), ValidationError> {
        if let Some(props) = element.get("props").and_then(|p| p.as_object()) {
            if let Some(bind) = props.get("bind").and_then(|b| b.as_str()) {
                if !bind.starts_with('/') {
                    return Err(ValidationError {
                        code: "E_BIND_PATH".to_string(),
                        message: format!(
                            "Invalid bind path '{}' in element '{}' (must start with '/')",
                            bind, id
                        ),
                    });
                }
            }
        }
        
        Ok(())
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
        use sha2::{Sha256, Digest};
        
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
