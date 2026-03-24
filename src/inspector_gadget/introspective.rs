//! Introspective Gadget logic
//!
//! Universal object inspector that analyzes data structures and generates schemas.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use base64::{Engine as _, engine::general_purpose};
use crate::error::Result;

// ============================================================================
// DATA STRUCTURES
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectionInput {
    pub source: InspectionSource,
    pub data: Option<String>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InspectionSource {
    File(String),
    Url(String),
    RawData {
        format_hint: Option<String>,
        description: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectionResult {
    pub detected_format: String,
    pub parsed_data: Value,
    pub schema: ObjectSchema,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectSchema {
    pub schema_type: String,
    pub properties: HashMap<String, SchemaProperty>,
    pub required: Vec<String>,
    pub array_items: Option<Box<ObjectSchema>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaProperty {
    pub data_type: String,
    pub description: Option<String>,
}

// ============================================================================
// INTROSPECTIVE GADGET
// ============================================================================

pub struct IntrospectiveGadget;

impl IntrospectiveGadget {
    pub async fn inspect(&self, input: InspectionInput) -> Result<InspectionResult> {
        // Simple auto-detection logic
        let format = self.detect_format(&input);
        
        match format.as_str() {
            "json" => self.parse_json(&input).await,
            "text" => self.parse_text(&input).await,
            _ => self.parse_binary(&input).await,
        }
    }

    fn detect_format(&self, input: &InspectionInput) -> String {
        if let InspectionSource::RawData { format_hint: Some(hint), .. } = &input.source {
            return hint.clone();
        }
        
        if let Some(data) = &input.data {
            if data.trim().starts_with('{') || data.trim().starts_with('[') {
                return "json".to_string();
            }
        }
        
        "text".to_string()
    }

    async fn parse_json(&self, input: &InspectionInput) -> Result<InspectionResult> {
        let data = input.data.as_ref().ok_or_else(|| crate::error::OpDbusError::General("No data".into()))?;
        let parsed: Value = serde_json::from_str(data).map_err(|e| crate::error::OpDbusError::General(e.to_string()))?;
        
        let schema = self.analyze_json_schema(&parsed);
        
        Ok(InspectionResult {
            detected_format: "json".to_string(),
            parsed_data: parsed,
            schema,
        })
    }

    async fn parse_text(&self, input: &InspectionInput) -> Result<InspectionResult> {
        let data = input.data.as_ref().unwrap_or(&String::new()).clone();
        
        Ok(InspectionResult {
            detected_format: "text".to_string(),
            parsed_data: json!({ "content": data }),
            schema: ObjectSchema {
                schema_type: "object".to_string(),
                properties: HashMap::new(),
                required: vec![],
                array_items: None,
            },
        })
    }

    async fn parse_binary(&self, input: &InspectionInput) -> Result<InspectionResult> {
         let data = input.data.as_ref().unwrap_or(&String::new()).clone();
         let bytes = data.as_bytes();
         
         Ok(InspectionResult {
            detected_format: "binary".to_string(),
            parsed_data: json!({
                "size": bytes.len(),
                "base64": general_purpose::STANDARD.encode(bytes)
            }),
            schema: ObjectSchema {
                schema_type: "object".to_string(),
                properties: HashMap::new(),
                required: vec![],
                array_items: None,
            },
         })
    }

    fn analyze_json_schema(&self, value: &Value) -> ObjectSchema {
        match value {
            Value::Object(obj) => {
                let mut properties = HashMap::new();
                let mut required = Vec::new();

                for (key, val) in obj {
                    properties.insert(key.clone(), SchemaProperty {
                        data_type: self.json_type(val),
                        description: None,
                    });
                    required.push(key.clone());
                }

                ObjectSchema {
                    schema_type: "object".to_string(),
                    properties,
                    required,
                    array_items: None,
                }
            }
            Value::Array(arr) => {
                let item_schema = arr.first().map(|first| Box::new(self.analyze_json_schema(first)));

                ObjectSchema {
                    schema_type: "array".to_string(),
                    properties: HashMap::new(),
                    required: vec![],
                    array_items: item_schema,
                }
            }
            _ => ObjectSchema {
                schema_type: self.json_type(value),
                properties: HashMap::new(),
                required: vec![],
                array_items: None,
            },
        }
    }

    fn json_type(&self, value: &Value) -> String {
        match value {
            Value::String(_) => "string".to_string(),
            Value::Number(_) => "number".to_string(),
            Value::Bool(_) => "boolean".to_string(),
            Value::Object(_) => "object".to_string(),
            Value::Array(_) => "array".to_string(),
            Value::Null => "null".to_string(),
        }
    }
}
