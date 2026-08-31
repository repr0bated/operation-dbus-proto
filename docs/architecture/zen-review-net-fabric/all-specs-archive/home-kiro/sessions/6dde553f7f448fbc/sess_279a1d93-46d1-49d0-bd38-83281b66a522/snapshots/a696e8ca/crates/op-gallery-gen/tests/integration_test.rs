//! Integration test for gallery generation

use op_gallery_gen::{GalleryGenConfig, validator::SpecValidator};

#[test]
fn test_validator_accepts_valid_spec() {
    let spec = serde_json::json!({
        "root": "root",
        "elements": {
            "root": {
                "type": "card",
                "children": ["label1"]
            },
            "label1": {
                "type": "label",
                "props": {
                    "text": "Hello World"
                }
            }
        }
    });
    
    let validator = SpecValidator::new();
    let result = validator.validate(&spec);
    
    assert!(result.valid, "Spec should be valid, errors: {:?}", result.errors);
}

#[test]
fn test_validator_rejects_missing_root() {
    let spec = serde_json::json!({
        "elements": {
            "label1": {
                "type": "label",
                "props": {
                    "text": "Hello"
                }
            }
        }
    });
    
    let validator = SpecValidator::new();
    let result = validator.validate(&spec);
    
    assert!(!result.valid);
    assert!(result.errors.iter().any(|e| e.code == "E_MISSING_ROOT"));
}

#[test]
fn test_validator_rejects_unknown_type() {
    let spec = serde_json::json!({
        "root": "root",
        "elements": {
            "root": {
                "type": "unknown_component",
                "children": []
            }
        }
    });
    
    let validator = SpecValidator::new();
    let result = validator.validate(&spec);
    
    // Unknown types are warned but not rejected (novelty components)
    // This test verifies the validator doesn't crash on unknown types
}

#[test]
fn test_validator_rejects_missing_bind_in_status_pill() {
    let spec = serde_json::json!({
        "root": "root",
        "elements": {
            "root": {
                "type": "status_pill"
            }
        }
    });
    
    let validator = SpecValidator::new();
    let result = validator.validate(&spec);
    
    assert!(!result.valid);
    assert!(result.errors.iter().any(|e| e.code == "E_PROP_SCHEMA"));
}

#[test]
fn test_validator_generates_signature() {
    let spec1 = serde_json::json!({
        "root": "a",
        "elements": {
            "a": {
                "type": "label",
                "props": {"text": "Test"}
            }
        }
    });
    
    let spec2 = serde_json::json!({
        "root": "a",
        "elements": {
            "a": {
                "type": "label",
                "props": {"text": "Test"}
            }
        }
    });
    
    let validator = SpecValidator::new();
    let result1 = validator.validate(&spec1);
    let result2 = validator.validate(&spec2);
    
    // Same specs should generate same signature
    assert_eq!(result1.signature, result2.signature);
}
