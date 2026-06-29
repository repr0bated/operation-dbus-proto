//! Proto Generator - Generate protobuf definitions from PluginSchema
//!
//! Converts operation-dbus plugin schemas to protobuf message and service
//! definitions, enabling dynamic schema-driven gRPC.
//!
//! For plugins using `#[derive(schemars::JsonSchema)]` on method input types,
//! this generates equivalent protobuf services that mirror the D-Bus objects
//! registered by SchemaBackedInterface.

use op_state_store::{FieldType, PluginSchema, SchemaCatalog, SchemaRegistry};
use std::fmt::Write;

/// Configuration for protobuf generation
#[derive(Debug, Clone)]
pub struct ProtoGenConfig {
    /// Package name for generated proto
    pub package_name: String,
    /// Whether to generate service definitions
    pub generate_services: bool,
    /// Whether to include validation annotations
    pub include_validation: bool,
    /// Whether to generate streaming RPCs for state changes
    pub generate_streams: bool,
}

impl Default for ProtoGenConfig {
    fn default() -> Self {
        Self {
            package_name: "operation.v1".to_string(),
            generate_services: true,
            include_validation: true,
            generate_streams: true,
        }
    }
}

/// Generate protobuf definitions from plugin schemas and their method declarations.
///
/// Each plugin's `MethodDecl` entries become typed RPCs. This mirrors the D-Bus
/// objects created by `SchemaBackedInterface.register_objects()`.
pub fn generate_plugin_method_protos(plugin_schemas: &[(String, PluginSchema)]) -> String {
    let mut output = String::new();

    writeln!(output, "syntax = \"proto3\";").unwrap();
    writeln!(output).unwrap();
    writeln!(output, "package operation.plugin.v1;").unwrap();
    writeln!(output).unwrap();
    writeln!(output, "import \"google/protobuf/struct.proto\";").unwrap();
    writeln!(output).unwrap();

    let mut plugin_ids: Vec<&String> = plugin_schemas.iter().map(|(id, _)| id).collect();
    plugin_ids.sort();

    for plugin_id in plugin_ids {
        let (_, schema) = plugin_schemas.iter().find(|(id, _)| id == plugin_id).unwrap();
        generate_plugin_service(&mut output, plugin_id, schema);
    }

    output
}

/// Generate a service for a single plugin with typed method messages.
fn generate_plugin_service(output: &mut String, plugin_id: &str, schema: &PluginSchema) {
    writeln!(output, "// ================================================").unwrap();
    writeln!(output, "// Plugin: {}", plugin_id).unwrap();
    writeln!(output, "// {} service typed methods", schema.description).unwrap();
    writeln!(output, "// ================================================").unwrap();
    writeln!(output).unwrap();

    let service_name = to_pascal_case(plugin_id);
    writeln!(output, "service {} {{", service_name).unwrap();

    let mut method_names: Vec<&String> = schema.methods.keys().collect();
    method_names.sort();

    for method_name in method_names {
        let method = &schema.methods[method_name];
        let subid = method.subid.as_str();
        let cap = method.required_capability.as_deref().unwrap_or("none");

        writeln!(output, "  // subid: {}", subid).unwrap();
        writeln!(output, "  // capability: {}", cap).unwrap();

        writeln!(
            output,
            "  rpc {}(google.protobuf.Struct) returns (google.protobuf.Struct);",
            method_name
        )
        .unwrap();
    }

    writeln!(output, "}}").unwrap();
    writeln!(output).unwrap();
}

// ProtoGenerator struct and other methods (kept for compatibility)
pub struct ProtoGenerator {
    config: ProtoGenConfig,
}

impl ProtoGenerator {
    pub fn new(config: ProtoGenConfig) -> Self {
        Self { config }
    }

    pub fn generate_for_schema(&self, schema: &PluginSchema) -> String {
        let mut output = String::new();
        writeln!(output, "syntax = \"proto3\";").unwrap();
        writeln!(output).unwrap();
        writeln!(output, "package {};", self.config.package_name).unwrap();
        writeln!(output).unwrap();
        writeln!(output, "import \"google/protobuf/struct.proto\";").unwrap();
        writeln!(output).unwrap();

        let message_name = to_pascal_case(&schema.name);
        writeln!(output, "message {} {{", message_name).unwrap();

        let mut fields: Vec<(&str, &op_state_store::FieldSchema)> = schema
            .fields
            .iter()
            .map(|(field_name, field_schema)| (field_name.as_str(), field_schema))
            .collect();
        fields.sort_unstable_by(|left, right| left.0.cmp(right.0));

        for (field_num, (field_name, field_schema)) in (1..).zip(fields) {
            let proto_type = self.field_type_to_proto(&field_schema.field_type);
            let optional_marker = if field_schema.required { "" } else { "optional " };
            writeln!(
                output,
                "  {}{} {} = {};",
                optional_marker, proto_type, field_name, field_num
            )
            .unwrap();
        }

        writeln!(output, "}}").unwrap();
        output
    }

    pub fn generate_for_catalog(&self, catalog: &SchemaCatalog) -> String {
        let mut output = String::new();
        let mut schema_names: Vec<&str> = catalog.list();
        schema_names.sort_unstable();

        writeln!(output, "syntax = \"proto3\";").unwrap();
        writeln!(output).unwrap();
        writeln!(output, "package {};", self.config.package_name).unwrap();
        writeln!(output).unwrap();
        writeln!(output, "import \"google/protobuf/struct.proto\";").unwrap();
        writeln!(output).unwrap();

        for schema_name in schema_names {
            let Some(schema) = catalog.get(schema_name) else {
                continue;
            };
            writeln!(output, "// =============================================").unwrap();
            writeln!(output, "// {} - {}", schema.name, schema.description).unwrap();
            writeln!(output, "// =============================================").unwrap();
            writeln!(output).unwrap();

            self.generate_message(&mut output, schema);
            self.generate_crud_messages(&mut output, schema);

            if self.config.generate_services {
                self.generate_service(&mut output, schema);
            }

            writeln!(output).unwrap();
        }

        self.generate_unified_service(&mut output, catalog);
        output
    }

    pub fn generate_for_registry(&self, registry: &SchemaRegistry) -> String {
        self.generate_for_catalog(registry)
    }

    fn generate_message(&self, output: &mut String, schema: &PluginSchema) {
        let message_name = to_pascal_case(&schema.name);
        writeln!(output, "message {} {{", message_name).unwrap();

        let mut fields: Vec<(&str, &op_state_store::FieldSchema)> = schema
            .fields
            .iter()
            .map(|(field_name, field_schema)| (field_name.as_str(), field_schema))
            .collect();
        fields.sort_unstable_by(|left, right| left.0.cmp(right.0));

        for (field_num, (field_name, field_schema)) in (1..).zip(fields) {
            let proto_type = self.field_type_to_proto(&field_schema.field_type);
            let optional_marker = if field_schema.required { "" } else { "optional " };
            writeln!(
                output,
                "  {}{} {} = {};",
                optional_marker, proto_type, field_name, field_num
            )
            .unwrap();
        }

        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();
    }

    fn generate_crud_messages(&self, output: &mut String, schema: &PluginSchema) {
        let message_name = to_pascal_case(&schema.name);

        writeln!(output, "message Get{}Request {{", message_name).unwrap();
        writeln!(output, "  string object_path = 1;").unwrap();
        writeln!(output, "}}").unwrap();

        writeln!(output, "message Get{}Response {{", message_name).unwrap();
        writeln!(output, "  {} state = 1;", message_name).unwrap();
        writeln!(output, "  string error = 2;").unwrap();
        writeln!(output, "}}").unwrap();

        if self.config.generate_streams {
            writeln!(output, "message {}Update {{", message_name).unwrap();
            writeln!(output, "  string object_path = 1;").unwrap();
            writeln!(output, "  {} state = 2;", message_name).unwrap();
            writeln!(output, "}}").unwrap();
        }

        writeln!(output).unwrap();
    }

    fn generate_service(&self, output: &mut String, schema: &PluginSchema) {
        let service_name = to_pascal_case(&schema.name);

        writeln!(output, "service {}Service {{", service_name).unwrap();
        writeln!(
            output,
            "  rpc Get(Get{}Request) returns (Get{}Response);",
            service_name, service_name
        )
        .unwrap();
        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();
    }

    fn generate_unified_service(&self, output: &mut String, _catalog: &SchemaCatalog) {
        writeln!(output, "service OperationService {{").unwrap();
        writeln!(
            output,
            "  rpc Get(GenericGetRequest) returns (GenericGetResponse);"
        )
        .unwrap();
        writeln!(output, "}}").unwrap();
    }

    fn field_type_to_proto(&self, field_type: &FieldType) -> String {
        match field_type {
            FieldType::String => "string".to_string(),
            FieldType::Integer => "int64".to_string(),
            FieldType::Float => "double".to_string(),
            FieldType::Boolean => "bool".to_string(),
            FieldType::Array(inner) => format!("repeated {}", self.field_type_to_proto(inner)),
            FieldType::Object(_) => "google.protobuf.Struct".to_string(),
            FieldType::Enum(_) => "string".to_string(),
            FieldType::OneOf(_) => "google.protobuf.Value".to_string(),
            FieldType::Any => "google.protobuf.Value".to_string(),
        }
    }
}

/// Convert string to PascalCase
pub fn to_pascal_case(s: &str) -> String {
    s.split(['_', '-', ' ', '.'])
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect()
}

/// Convert string to snake_case
#[cfg(test)]
fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_lowercase().next().unwrap_or(c));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("lxc"), "Lxc");
        assert_eq!(to_pascal_case("network_interface"), "NetworkInterface");
        assert_eq!(to_pascal_case("ovs-bridge"), "OvsBridge");
    }
}
