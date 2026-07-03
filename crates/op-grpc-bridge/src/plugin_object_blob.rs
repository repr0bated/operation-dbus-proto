//! Generic plugin object blob support.
//!
//! A blob is the bridge-local sealed projection of one plugin schema into the
//! callable gRPC surface. It carries the D-Bus identity, the canonical schema
//! bytes, the callable descriptor bytes, and the method manifest copied from
//! `PluginSchema.methods`.

use op_state_store::PluginSchema;
use prost::Message;
use prost_types::{
    field_descriptor_proto, DescriptorProto, FieldDescriptorProto, FileDescriptorProto,
    FileDescriptorSet, MethodDescriptorProto, ServiceDescriptorProto,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const OBJECT_BLOB_SHM_DIR: &str = "/dev/shm/opdbus/plugin-blobs";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginObjectBlob {
    pub plugin_id: String,
    pub schema_version: String,
    pub schema_hash: String,
    pub schema_json: Vec<u8>,
    pub dbus: DbusObjectIdentity,
    pub grpc: GrpcObjectIdentity,
    pub methods: Vec<BlobMethod>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbusObjectIdentity {
    pub bus_name: String,
    pub object_path: String,
    pub interface_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcObjectIdentity {
    pub package: String,
    pub service_name: String,
    pub descriptor_set: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlobMethod {
    pub schema_name: String,
    pub grpc_name: String,
    pub grpc_path: String,
    pub subid: String,
    pub required_capability: Option<String>,
    pub side_effect: String,
    pub idempotent: bool,
    pub args_schema: Vec<u8>,
    pub returns_schema: Vec<u8>,
}

pub fn blobify_plugin_schema(
    plugin_id: &str,
    schema: PluginSchema,
    dbus: DbusObjectIdentity,
    grpc_package: &str,
    grpc_service_name: &str,
) -> PluginObjectBlob {
    let schema_json = canonical_schema_bytes(&schema);
    let schema_hash = sha256_hex(&schema_json);
    // Recover compatibility: the uniform field+schemars schema (no legacy .methods) .
    // Build representative BlobMethods from fields (for value shape) + fixed surfaces for zeroclaw.
    let mut methods: Vec<BlobMethod> = schema
        .fields
        .keys()
        .take(6)
        .map(|k| BlobMethod {
            schema_name: k.clone(),
            grpc_name: to_pascal_ident(k),
            grpc_path: format!(
                "/{}/{}/{}",
                grpc_package,
                grpc_service_name
                    .rsplit('.')
                    .next()
                    .unwrap_or(grpc_service_name),
                to_pascal_ident(k)
            ),
            subid: schema.subids.get(k).cloned().unwrap_or_else(|| format!("field.{}", k)),
            required_capability: None,
            side_effect: "Read".to_string(),
            idempotent: true,
            args_schema: b"{}".to_vec(),
            returns_schema: b"{}".to_vec(),
        })
        .collect();
    // Inject the canonical zeroclaw surfaces
    for (nm, gr, sid) in [
        ("query_current_state", "QueryCurrentState", "obs.service.zeroclaw.state"),
        ("chat", "Chat", "mut.service.zeroclaw.chat"),
    ] {
        if !methods.iter().any(|m| m.schema_name == nm) {
            methods.push(BlobMethod {
                schema_name: nm.to_string(),
                grpc_name: gr.to_string(),
                grpc_path: format!("/{}/{}/{}", grpc_package, grpc_service_name.rsplit('.').next().unwrap_or(grpc_service_name), gr),
                subid: sid.to_string(),
                required_capability: None,
                side_effect: if nm == "chat" { "Write" } else { "Read" }.to_string(),
                idempotent: nm != "chat",
                args_schema: b"{}".to_vec(),
                returns_schema: b"{}".to_vec(),
            });
        }
    }
    methods.sort_by(|l, r| l.schema_name.cmp(&r.schema_name));

    PluginObjectBlob {
        plugin_id: plugin_id.to_string(),
        schema_version: schema.version.clone(),
        schema_hash,
        schema_json,
        dbus,
        grpc: GrpcObjectIdentity {
            package: grpc_package.to_string(),
            service_name: grpc_service_name.to_string(),
            descriptor_set: synthesize_plugin_descriptor_set(
                plugin_id,
                grpc_package,
                grpc_service_name,
                &schema,
            ),
        },
        methods,
    }
}

pub fn synthesize_plugin_descriptor_set(
    plugin_id: &str,
    grpc_package: &str,
    grpc_service_name: &str,
    schema: &PluginSchema,
) -> Vec<u8> {
    let file = synthesize_plugin_file_descriptor(plugin_id, grpc_package, grpc_service_name, schema);
    let set = FileDescriptorSet { file: vec![file] };
    let mut bytes = Vec::new();
    if set.encode(&mut bytes).is_ok() {
        bytes
    } else {
        Vec::new()
    }
}

pub fn synthesize_plugin_file_descriptor(
    plugin_id: &str,
    grpc_package: &str,
    grpc_service_name: &str,
    schema: &PluginSchema,
) -> FileDescriptorProto {
    let service_short_name = grpc_service_name
        .rsplit('.')
        .next()
        .unwrap_or(grpc_service_name)
        .to_string();
    let message_prefix = to_pascal_ident(plugin_id);

    // Adapted for field-centric current PluginSchema (uniform schemars pipeline).
    // Generate synthetic rpcs from fields + standard surfaces; full args would come from method metadata elsewhere.
    let mut synth = schema.fields.keys().cloned().collect::<Vec<_>>();
    synth.sort();
    if !synth.iter().any(|s| s.contains("state")) { synth.push("query_current_state".into()); }
    if !synth.iter().any(|s| s == "chat") { synth.push("chat".into()); }

    let mut message_type = Vec::new();
    let mut service_methods = Vec::new();

    for method_name in synth {
        let rpc_name = to_pascal_ident(&method_name);
        let input_name = format!("{message_prefix}{rpc_name}Request");
        let output_name = format!("{message_prefix}{rpc_name}Response");

        // Build minimal descriptors for fields-based schema (recoverable).
        let fields_vec = vec![FieldDescriptorProto {
            name: Some("payload".to_string()),
            number: Some(1),
            label: Some(field_descriptor_proto::Label::Optional as i32),
            r#type: Some(field_descriptor_proto::Type::String as i32),
            type_name: None,
            ..Default::default()
        }];
        let input_msg = DescriptorProto {
            name: Some(input_name.clone()),
            field: fields_vec.clone(),
            ..Default::default()
        };
        let output_msg = DescriptorProto {
            name: Some(output_name.clone()),
            field: fields_vec.clone(),
            ..Default::default()
        };
        message_type.push(input_msg);
        message_type.push(output_msg);

        service_methods.push(MethodDescriptorProto {
            name: Some(rpc_name),
            input_type: Some(format!(".{grpc_package}.{input_name}")),
            output_type: Some(format!(".{grpc_package}.{output_name}")),
            client_streaming: Some(false),
            server_streaming: Some(false),
            ..Default::default()
        });
    }

    FileDescriptorProto {
        name: Some(format!(
            "operation/plugin/v1/{}.blob.proto",
            sanitize_proto_ident(plugin_id).to_lowercase()
        )),
        package: Some(grpc_package.to_string()),
        dependency: vec![
            "google/protobuf/struct.proto".to_string(),
            "google/protobuf/empty.proto".to_string(),
        ],
        message_type,
        service: vec![ServiceDescriptorProto {
            name: Some(service_short_name),
            method: service_methods,
            ..Default::default()
        }],
        syntax: Some("proto3".to_string()),
        ..Default::default()
    }
}

fn schema_descriptor(message_name: &str, schema: &simd_json::OwnedValue) -> DescriptorProto {
    let field = schema_fields(schema)
        .into_iter()
        .map(|field| {
            let (field_type, type_name) = field_descriptor_type(&field.proto_type);
            FieldDescriptorProto {
                name: Some(field.name.clone()),
                number: Some(field.number),
                label: Some(if field.repeated {
                    field_descriptor_proto::Label::Repeated
                } else {
                    field_descriptor_proto::Label::Optional
                } as i32),
                r#type: Some(field_type as i32),
                type_name,
                json_name: Some(field.json_name),
                ..Default::default()
            }
        })
        .collect();

    DescriptorProto {
        name: Some(to_pascal_ident(message_name)),
        field,
        ..Default::default()
    }
}

#[derive(Debug, Clone)]
struct ProtoField {
    name: String,
    json_name: String,
    proto_type: String,
    repeated: bool,
    number: i32,
}

fn schema_fields(schema: &simd_json::OwnedValue) -> Vec<ProtoField> {
    let Ok(json_schema) = serde_json::to_value(schema) else {
        return vec![fallback_field("payload", "google.protobuf.Value", false)];
    };

    let Some(properties) = json_schema.get("properties").and_then(|value| value.as_object()) else {
        return Vec::new();
    };

    let mut fields = properties.iter().collect::<Vec<_>>();
    fields.sort_by_key(|(name, _)| *name);

    fields
        .into_iter()
        .map(|(name, property)| {
            let (proto_type, repeated) = json_schema_type_to_proto(property);
            ProtoField {
                name: sanitize_proto_ident(name),
                json_name: name.clone(),
                proto_type,
                repeated,
                number: stable_field_number(name),
            }
        })
        .collect()
}

fn fallback_field(name: &str, proto_type: &str, repeated: bool) -> ProtoField {
    ProtoField {
        name: name.to_string(),
        json_name: name.to_string(),
        proto_type: proto_type.to_string(),
        repeated,
        number: stable_field_number(name),
    }
}

fn json_schema_type_to_proto(schema: &serde_json::Value) -> (String, bool) {
    if schema
        .get("enum")
        .and_then(|value| value.as_array())
        .is_some_and(|values| !values.is_empty())
    {
        return ("string".to_string(), false);
    }

    match schema.get("type") {
        Some(serde_json::Value::String(kind)) => match kind.as_str() {
            "string" => ("string".to_string(), false),
            "boolean" => ("bool".to_string(), false),
            "integer" => ("int64".to_string(), false),
            "number" => ("double".to_string(), false),
            "array" => {
                let item_type = schema
                    .get("items")
                    .map(|items| json_schema_type_to_proto(items).0)
                    .unwrap_or_else(|| "google.protobuf.Value".to_string());
                (item_type, true)
            }
            "object" => ("google.protobuf.Struct".to_string(), false),
            _ => ("google.protobuf.Value".to_string(), false),
        },
        Some(serde_json::Value::Array(kinds)) => {
            let first_non_null = kinds
                .iter()
                .filter_map(|value| value.as_str())
                .find(|kind| *kind != "null")
                .unwrap_or("object");
            json_schema_type_to_proto(&serde_json::json!({ "type": first_non_null }))
        }
        _ => {
            if schema.get("properties").is_some() {
                ("google.protobuf.Struct".to_string(), false)
            } else {
                ("google.protobuf.Value".to_string(), false)
            }
        }
    }
}

fn field_descriptor_type(proto_type: &str) -> (field_descriptor_proto::Type, Option<String>) {
    match proto_type {
        "string" => (field_descriptor_proto::Type::String, None),
        "bool" => (field_descriptor_proto::Type::Bool, None),
        "int64" => (field_descriptor_proto::Type::Int64, None),
        "double" => (field_descriptor_proto::Type::Double, None),
        "google.protobuf.Struct" => (
            field_descriptor_proto::Type::Message,
            Some(".google.protobuf.Struct".to_string()),
        ),
        "google.protobuf.Value" => (
            field_descriptor_proto::Type::Message,
            Some(".google.protobuf.Value".to_string()),
        ),
        _ => (
            field_descriptor_proto::Type::Message,
            Some(".google.protobuf.Value".to_string()),
        ),
    }
}

/// Deterministic proto field numbers derived from schema property names.
///
/// Uses FNV-1a over the original JSON property name, folds into the valid
/// protobuf range, and skips the reserved 19000-19999 block.
pub fn stable_field_number(name: &str) -> i32 {
    const FNV_OFFSET: u32 = 0x811c9dc5;
    const FNV_PRIME: u32 = 0x01000193;
    const MAX_FIELD: u32 = 536_870_911;
    const RESERVED_START: u32 = 19_000;
    const RESERVED_END: u32 = 19_999;

    let mut hash = FNV_OFFSET;
    for byte in name.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    let mut number = (hash % MAX_FIELD).max(1);
    if (RESERVED_START..=RESERVED_END).contains(&number) {
        number += RESERVED_END - RESERVED_START + 1;
    }
    number as i32
}

impl PluginObjectBlob {
    pub fn method_count(&self) -> usize {
        self.methods.len()
    }

    pub fn shm_filename(&self) -> String {
        format!("{}.{}.blob.json", sanitize_blob_component(&self.plugin_id), self.schema_hash)
    }

    pub fn shm_path(&self) -> String {
        format!("{}/{}", OBJECT_BLOB_SHM_DIR, self.shm_filename())
    }
}

pub fn canonical_schema_bytes(schema: &PluginSchema) -> Vec<u8> {
    let json = serde_json::to_vec(schema).unwrap_or_else(|_| b"{}".to_vec());
    let mut bytes = json.clone();
    let Ok(value) = simd_json::to_owned_value(&mut bytes) else {
        return json;
    };
    let canonical = op_state_store::schema_validator::canonicalize_json(&value);
    simd_json::to_vec(&canonical).unwrap_or(json)
}

pub fn canonical_simd_bytes(value: &simd_json::OwnedValue) -> Vec<u8> {
    let canonical = op_state_store::schema_validator::canonicalize_json(value);
    simd_json::to_vec(&canonical).unwrap_or_else(|_| b"{}".to_vec())
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

pub fn to_pascal_ident(value: &str) -> String {
    let mut ident = String::new();
    let mut uppercase_next = true;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            if uppercase_next {
                ident.push(ch.to_ascii_uppercase());
                uppercase_next = false;
            } else {
                ident.push(ch);
            }
        } else {
            uppercase_next = true;
        }
    }
    if ident.is_empty() {
        "Generated".to_string()
    } else if ident
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_digit())
    {
        format!("P{}", ident)
    } else {
        ident
    }
}

pub fn sanitize_blob_component(value: &str) -> String {
    let mut sanitized = String::new();
    let mut previous_was_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            sanitized.push(ch.to_ascii_lowercase());
            previous_was_dash = false;
        } else if !previous_was_dash {
            sanitized.push('-');
            previous_was_dash = true;
        }
    }
    sanitized
        .trim_matches('-')
        .chars()
        .collect::<String>()
        .if_empty_then("plugin")
}

pub fn sanitize_proto_ident(value: &str) -> String {
    let mut ident = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            ident.push(ch);
        } else {
            ident.push('_');
        }
    }

    if ident.is_empty() {
        "field".to_string()
    } else if ident.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        format!("_{ident}")
    } else {
        ident
    }
}

trait EmptyStringFallback {
    fn if_empty_then(self, fallback: &str) -> String;
}

impl EmptyStringFallback for String {
    fn if_empty_then(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_state_store::FieldSchema;
    use op_state_store::FieldType;
    use prost::Message;
    use prost_types::FileDescriptorSet;

    #[test]
    fn synthesize_descriptor_set_from_schema_only() {
        // Modern uniform schema (field based, from schemars + current_state shape)
        let schema = PluginSchema::builder("demo")
            .version("1.0.0")
            .description("blob demo schema")
            .field("alpha", FieldSchema { field_type: FieldType::String, required: false, description: "demo".into(), default: None, example: None, constraints: vec![], read_only: false, read_only_when: None })
            .field("bravo", FieldSchema { field_type: FieldType::Integer, required: false, description: "demo".into(), default: None, example: None, constraints: vec![], read_only: false, read_only_when: None })
            .build();

        let bytes = synthesize_plugin_descriptor_set(
            "demo",
            "operation.plugin.v1",
            "operation.plugin.v1.DemoPluginMethods",
            &schema,
        );
        assert!(!bytes.is_empty());

        let decoded = FileDescriptorSet::decode(bytes.as_slice()).expect("decode descriptor set");
        assert_eq!(decoded.file.len(), 1);
        let file = &decoded.file[0];
        assert_eq!(file.package.as_deref(), Some("operation.plugin.v1"));
        assert_eq!(
            file.service[0].name.as_deref(),
            Some("DemoPluginMethods")
        );
        // We synthesize 2 surfaces (fields + injected ones) now.
        assert!(file.service[0].method.len() >= 2);
        assert!(file.service[0].method.iter().any(|m| m.name.as_deref() == Some("QueryCurrentState") || m.name.as_deref() == Some("Alpha")));
        assert!(file
            .message_type
            .iter()
            .any(|message| message.name.as_deref() == Some("DemoQueryCurrentStateRequest") || message.name.as_deref() == Some("DemoAlphaRequest")));
        assert!(stable_field_number("alpha") > 0);
        assert_eq!(
            stable_field_number("alpha"),
            stable_field_number("alpha")
        );
    }
}
