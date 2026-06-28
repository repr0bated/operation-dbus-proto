//! Proto descriptor → JSON Schema for MCP tool inputSchema.
//! Field names use proto3 JSON (lowerCamelCase) names, matching what
//! prost-reflect's serde accepts on tools/call.

use prost_reflect::{FieldDescriptor, Kind, MessageDescriptor};
use serde_json::{json, Map, Value};

pub fn message_schema(desc: &MessageDescriptor) -> Value {
    let mut visited = Vec::new();
    message_schema_inner(desc, &mut visited)
}

fn message_schema_inner(desc: &MessageDescriptor, visited: &mut Vec<String>) -> Value {
    if visited.iter().any(|n| n == desc.full_name()) {
        return json!({
            "type": "object",
            "description": format!("recursive {}", desc.full_name())
        });
    }
    visited.push(desc.full_name().to_string());
    let mut props = Map::new();
    for field in desc.fields() {
        props.insert(field.json_name().to_string(), field_schema(&field, visited));
    }
    visited.pop();
    json!({
        "type": "object",
        "properties": props,
        "additionalProperties": false
    })
}

fn field_schema(field: &FieldDescriptor, visited: &mut Vec<String>) -> Value {
    if field.is_map() {
        let value_schema = match field.kind() {
            Kind::Message(entry) => kind_schema(&entry.map_entry_value_field().kind(), visited),
            _ => json!({}),
        };
        return json!({"type": "object", "additionalProperties": value_schema});
    }
    let base = kind_schema(&field.kind(), visited);
    if field.is_list() {
        json!({"type": "array", "items": base})
    } else {
        base
    }
}

fn kind_schema(kind: &Kind, visited: &mut Vec<String>) -> Value {
    match kind {
        Kind::Double | Kind::Float => json!({"type": "number"}),
        Kind::Int32 | Kind::Sint32 | Kind::Sfixed32 | Kind::Uint32 | Kind::Fixed32 => {
            json!({"type": "integer"})
        }
        Kind::Int64 | Kind::Sint64 | Kind::Sfixed64 | Kind::Uint64 | Kind::Fixed64 => {
            json!({"type": ["integer", "string"], "description": "64-bit integer (string form accepted)"})
        }
        Kind::Bool => json!({"type": "boolean"}),
        Kind::String => json!({"type": "string"}),
        Kind::Bytes => json!({"type": "string", "description": "base64-encoded bytes"}),
        Kind::Enum(e) => json!({
            "type": "string",
            "enum": e.values().map(|v| v.name().to_string()).collect::<Vec<_>>()
        }),
        Kind::Message(m) => message_schema_inner(m, visited),
    }
}
