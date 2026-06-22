//! Proto Generator - Generate protobuf definitions from PluginSchema
//!
//! Converts operation-dbus plugin schemas to protobuf message and service
//! definitions, enabling dynamic schema-driven gRPC.

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

/// Generate protobuf definitions from plugin schemas
pub struct ProtoGenerator {
    config: ProtoGenConfig,
}

impl ProtoGenerator {
    pub fn new(config: ProtoGenConfig) -> Self {
        Self { config }
    }

    /// Generate proto file content for a single plugin schema
    pub fn generate_for_schema(&self, schema: &PluginSchema) -> String {
        let mut output = String::new();

        // Header
        writeln!(output, "syntax = \"proto3\";").unwrap();
        writeln!(output).unwrap();
        writeln!(output, "package {};", self.config.package_name).unwrap();
        writeln!(output).unwrap();

        // Imports
        writeln!(output, "import \"google/protobuf/struct.proto\";").unwrap();
        writeln!(output, "import \"google/protobuf/timestamp.proto\";").unwrap();
        writeln!(output).unwrap();

        // Generate message for the schema
        self.generate_message(&mut output, schema);

        // Generate request/response messages
        self.generate_crud_messages(&mut output, schema);

        // Generate service if enabled
        if self.config.generate_services {
            self.generate_service(&mut output, schema);
        }

        output
    }

    /// Generate proto file content for all schemas in a catalog.
    pub fn generate_for_catalog(&self, catalog: &SchemaCatalog) -> String {
        let mut output = String::new();
        let mut schema_names: Vec<&str> = catalog.list();
        schema_names.sort_unstable();

        // Header
        writeln!(output, "syntax = \"proto3\";").unwrap();
        writeln!(output).unwrap();
        writeln!(output, "package {};", self.config.package_name).unwrap();
        writeln!(output).unwrap();

        // Imports
        writeln!(output, "import \"google/protobuf/struct.proto\";").unwrap();
        writeln!(output, "import \"google/protobuf/timestamp.proto\";").unwrap();
        writeln!(output, "import \"google/protobuf/any.proto\";").unwrap();
        writeln!(output).unwrap();

        // Generate messages for each schema
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

        // Add unified service
        self.generate_unified_service(&mut output, catalog);

        output
    }

    /// Compatibility wrapper for older call sites that still say `registry`.
    pub fn generate_for_registry(&self, registry: &SchemaRegistry) -> String {
        self.generate_for_catalog(registry)
    }

    pub fn generate_message(&self, output: &mut String, schema: &PluginSchema) {
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
            let optional_marker = if field_schema.required {
                ""
            } else {
                "optional "
            };
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

        // Get request
        writeln!(output, "message Get{}Request {{", message_name).unwrap();
        writeln!(output, "  string object_path = 1;").unwrap();
        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();

        // Get response
        writeln!(output, "message Get{}Response {{", message_name).unwrap();
        writeln!(output, "  {} state = 1;", message_name).unwrap();
        writeln!(output, "  string error = 2;").unwrap();
        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();

        // Set request
        writeln!(output, "message Set{}Request {{", message_name).unwrap();
        writeln!(output, "  string object_path = 1;").unwrap();
        writeln!(output, "  {} state = 2;", message_name).unwrap();
        writeln!(output, "  string actor_id = 3;").unwrap();
        writeln!(output, "  string capability_id = 4;").unwrap();
        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();

        // Set response
        writeln!(output, "message Set{}Response {{", message_name).unwrap();
        writeln!(output, "  bool success = 1;").unwrap();
        writeln!(output, "  string event_id = 2;").unwrap();
        writeln!(output, "  string effective_hash = 3;").unwrap();
        writeln!(output, "  string error = 4;").unwrap();
        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();

        // List request
        writeln!(output, "message List{}Request {{", message_name).unwrap();
        writeln!(output, "  string path_prefix = 1;").unwrap();
        writeln!(output, "  int32 limit = 2;").unwrap();
        writeln!(output, "  string cursor = 3;").unwrap();
        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();

        // List response
        writeln!(output, "message List{}Response {{", message_name).unwrap();
        writeln!(output, "  repeated {} items = 1;", message_name).unwrap();
        writeln!(output, "  string next_cursor = 2;").unwrap();
        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();

        // Watch update (for streaming)
        if self.config.generate_streams {
            writeln!(output, "message {}Update {{", message_name).unwrap();
            writeln!(output, "  string object_path = 1;").unwrap();
            writeln!(output, "  {} state = 2;", message_name).unwrap();
            writeln!(output, "  string event_id = 3;").unwrap();
            writeln!(output, "  repeated string tags_touched = 4;").unwrap();
            writeln!(output, "  google.protobuf.Timestamp timestamp = 5;").unwrap();
            writeln!(output, "}}").unwrap();
            writeln!(output).unwrap();

            writeln!(output, "message Watch{}Request {{", message_name).unwrap();
            writeln!(output, "  string path_filter = 1;").unwrap();
            writeln!(output, "  repeated string tag_filters = 2;").unwrap();
            writeln!(output, "}}").unwrap();
            writeln!(output).unwrap();
        }
    }

    fn generate_service(&self, output: &mut String, schema: &PluginSchema) {
        let service_name = to_pascal_case(&schema.name);

        writeln!(output, "service {}Service {{", service_name).unwrap();
        writeln!(output, "  // Get {} state", schema.name).unwrap();
        writeln!(
            output,
            "  rpc Get(Get{}Request) returns (Get{}Response);",
            service_name, service_name
        )
        .unwrap();
        writeln!(output).unwrap();
        writeln!(output, "  // Set {} state", schema.name).unwrap();
        writeln!(
            output,
            "  rpc Set(Set{}Request) returns (Set{}Response);",
            service_name, service_name
        )
        .unwrap();
        writeln!(output).unwrap();
        writeln!(output, "  // List {} objects", schema.name).unwrap();
        writeln!(
            output,
            "  rpc List(List{}Request) returns (List{}Response);",
            service_name, service_name
        )
        .unwrap();

        if self.config.generate_streams {
            writeln!(output).unwrap();
            writeln!(output, "  // Watch for {} changes", schema.name).unwrap();
            writeln!(
                output,
                "  rpc Watch(Watch{}Request) returns (stream {}Update);",
                service_name, service_name
            )
            .unwrap();
        }

        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();
    }

    fn generate_unified_service(&self, output: &mut String, catalog: &SchemaCatalog) {
        writeln!(output, "// =============================================").unwrap();
        writeln!(output, "// Unified Operation Service").unwrap();
        writeln!(output, "// =============================================").unwrap();
        writeln!(output).unwrap();

        // Generic state messages
        writeln!(output, "message GenericGetRequest {{").unwrap();
        writeln!(output, "  string plugin_id = 1;").unwrap();
        writeln!(output, "  string object_path = 2;").unwrap();
        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();

        writeln!(output, "message GenericGetResponse {{").unwrap();
        writeln!(output, "  google.protobuf.Struct state = 1;").unwrap();
        writeln!(output, "  string schema_version = 2;").unwrap();
        writeln!(output, "  string effective_hash = 3;").unwrap();
        writeln!(output, "  string error = 4;").unwrap();
        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();

        writeln!(output, "message GenericSetRequest {{").unwrap();
        writeln!(output, "  string plugin_id = 1;").unwrap();
        writeln!(output, "  string object_path = 2;").unwrap();
        writeln!(output, "  google.protobuf.Struct state = 3;").unwrap();
        writeln!(output, "  string actor_id = 4;").unwrap();
        writeln!(output, "  string capability_id = 5;").unwrap();
        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();

        writeln!(output, "message GenericSetResponse {{").unwrap();
        writeln!(output, "  bool success = 1;").unwrap();
        writeln!(output, "  string event_id = 2;").unwrap();
        writeln!(output, "  string effective_hash = 3;").unwrap();
        writeln!(output, "  string error = 4;").unwrap();
        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();

        writeln!(output, "service OperationService {{").unwrap();
        writeln!(
            output,
            "  rpc Get(GenericGetRequest) returns (GenericGetResponse);"
        )
        .unwrap();
        writeln!(
            output,
            "  rpc Set(GenericSetRequest) returns (GenericSetResponse);"
        )
        .unwrap();

        let mut schema_names: Vec<&str> = catalog.list();
        schema_names.sort_unstable();

        for schema_name in schema_names {
            let name = to_pascal_case(schema_name);
            writeln!(
                output,
                "  rpc Get{}(Get{}Request) returns (Get{}Response);",
                name, name, name
            )
            .unwrap();
            writeln!(
                output,
                "  rpc Set{}(Set{}Request) returns (Set{}Response);",
                name, name, name
            )
            .unwrap();
        }
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
fn to_pascal_case(s: &str) -> String {
    s.split(['_', '-', ' '])
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
    use op_state_store::SchemaCatalog;

    #[test]
    fn test_to_pascal_case() {
        assert_eq!(to_pascal_case("lxc"), "Lxc");
        assert_eq!(to_pascal_case("network_interface"), "NetworkInterface");
        assert_eq!(to_pascal_case("ovs-bridge"), "OvsBridge");
    }

    #[test]
    fn test_to_snake_case() {
        assert_eq!(to_snake_case("containerID"), "container_i_d");
        assert_eq!(to_snake_case("objectPath"), "object_path");
    }

    #[test]
    fn test_generate_for_catalog() {
        let catalog = SchemaCatalog::with_builtin_schemas();
        let generator = ProtoGenerator::new(ProtoGenConfig::default());
        let proto = generator.generate_for_catalog(&catalog);

        assert!(proto.contains("syntax = \"proto3\";"));
        assert!(proto.contains("service OperationService"));
    }
}
