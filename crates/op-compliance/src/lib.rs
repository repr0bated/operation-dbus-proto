//! The Compliance Engine: "The Law Firm"
//! Responsible for validating the PluginSchema against legal and security frameworks.

use serde_json::Value;

/// Strongly-typed error for compliance validation operations.
///
/// Keeps `anyhow` out of the public API surface while preserving
/// ergonomic `?` propagation inside the crate.
#[derive(Debug, thiserror::Error)]
pub enum ComplianceError {
    #[error("validation failed: {0}")]
    Validation(String),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub mod attorneys {
    use super::ComplianceError;
    use serde_json::Value;

    /// Olivia Scal: Managing Partner / OSCAL (Open Security Controls Assessment Language)
    pub struct OliviaScal;
    impl OliviaScal {
        pub fn validate_controls(schema: &Value) -> std::result::Result<(), ComplianceError> {
            // Check for security control metadata
            if let Some(caps) = schema.get("capabilities") {
                if caps.get("requires_root").and_then(|v| v.as_bool()) == Some(true) {
                    tracing::warn!("Plugin requires root; OSCAL assessment recommended");
                }
            }
            Ok(())
        }
    }

    /// E.U.gene Risk: EU AI Act Counsel
    pub struct EugeneRisk;
    impl EugeneRisk {
        pub fn validate_ai_risk(schema: &Value) -> std::result::Result<(), ComplianceError> {
            // If it's an AI/ML plugin, check for transparency requirements
            if schema.get("plugin_type").and_then(|v| v.as_str()) == Some("custom") {
                if let Some(meta) = schema.get("schema") {
                    if meta.get("model_name").is_some()
                        && meta.get("training_data_source").is_none()
                    {
                        return Err(ComplianceError::Validation(
                            "EU AI Act violation: Training data source must be declared for models"
                                .into(),
                        ));
                    }
                }
            }
            Ok(())
        }
    }

    /// Penny Privacy: GDPR Engine
    pub struct PennyPrivacy;
    impl PennyPrivacy {
        pub fn validate_privacy(schema: &Value) -> std::result::Result<(), ComplianceError> {
            // Check for PII handling without retention policy
            if let Some(s) = schema.get("schema") {
                let schema_str = s.to_string().to_lowercase();
                if (schema_str.contains("email")
                    || schema_str.contains("user_id")
                    || schema_str.contains("phone"))
                    && !schema_str.contains("retention")
                {
                    return Err(ComplianceError::Validation(
                        "GDPR violation: PII fields detected without retention policy".into(),
                    ));
                }
            }
            Ok(())
        }
    }

    /// Reggie O.P.A.: Cloud Prosecutor
    pub struct ReggieOpa;
    impl ReggieOpa {
        pub fn validate_policy(schema: &Value) -> std::result::Result<(), ComplianceError> {
            // Basic policy check: Versioning must be present
            if schema.get("version").is_none() {
                return Err(ComplianceError::Validation(
                    "OPA Policy failure: Missing version field".into(),
                ));
            }
            Ok(())
        }
    }
}


pub struct LawFirm;

impl LawFirm {
    pub fn review_schema(schema_json: &str) -> std::result::Result<(), ComplianceError> {
        let v: Value = serde_json::from_str(schema_json)?;

        // 1. Structural Validation via JSON Schema
        let meta_schema = include_str!("../../../schemas/opdbus-plugin-schema.json");
        let meta_v: Value = serde_json::from_str(meta_schema)?;

        let compiled = jsonschema::validator_for(&meta_v)
            .map_err(|e| ComplianceError::Validation(format!("Schema error: {e}")))?;

        // jsonschema >= 0.20: `validate` yields one error; `iter_errors` is the
        // all-errors iterator the old `JSONSchema::validate` returned.
        let error_msgs: Vec<String> = compiled.iter_errors(&v).map(|e| e.to_string()).collect();
        if !error_msgs.is_empty() {
            return Err(ComplianceError::Validation(format!(
                "Structural validation failed: {}",
                error_msgs.join(", ")
            )));
        }

        // 2. Attorney Reviews
        attorneys::OliviaScal::validate_controls(&v)?;
        attorneys::EugeneRisk::validate_ai_risk(&v)?;
        attorneys::PennyPrivacy::validate_privacy(&v)?;
        attorneys::ReggieOpa::validate_policy(&v)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_valid_schema_passes() {
        let schema = json!({
            "name": "test-plugin",
            "version": "1.0.0",
            "plugin_type": "service",
            "capabilities": {
                "can_read": true
            }
        });
        assert!(LawFirm::review_schema(&schema.to_string()).is_ok());
    }

    #[test]
    fn test_invalid_version_fails() {
        let schema = json!({
            "name": "test-plugin",
            "version": "invalid",
            "plugin_type": "service"
        });
        assert!(LawFirm::review_schema(&schema.to_string()).is_err());
    }

    #[test]
    fn test_gdpr_violation() {
        let schema = json!({
            "name": "pii-plugin",
            "version": "1.1.0",
            "plugin_type": "custom",
            "schema": {
                "user_email": "string"
            }
        });
        let result = LawFirm::review_schema(&schema.to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("GDPR"));
    }

    #[test]
    fn test_ai_act_violation() {
        let schema = json!({
            "name": "ai-plugin",
            "version": "2.0.0",
            "plugin_type": "custom",
            "schema": {
                "model_name": "gpt-4"
            }
        });
        let result = LawFirm::review_schema(&schema.to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("AI Act"));
    }
}
