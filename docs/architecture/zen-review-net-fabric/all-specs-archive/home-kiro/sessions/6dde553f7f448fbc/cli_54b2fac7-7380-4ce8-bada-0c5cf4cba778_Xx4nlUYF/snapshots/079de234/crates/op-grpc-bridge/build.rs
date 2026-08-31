use std::env;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let generated_proto = out_dir.join("plugin_methods.proto");
    let generated_routes = out_dir.join("plugin_method_routes.rs");
    let plugin_methods = collect_plugin_methods();

    std::fs::write(
        &generated_proto,
        generate_plugin_methods_proto(&plugin_methods),
    )?;
    std::fs::write(
        &generated_routes,
        generate_plugin_method_routes(&plugin_methods),
    )?;

    // Compile all domain protos into a single combined FileDescriptorSet so
    // that tonic-reflection exposes every service in one query.
    //
    // Adding a new domain proto:
    //   1. Add the .proto file under proto/
    //   2. Add it to the compile_protos list below
    //   3. Add rerun-if-changed below
    //   4. Add the generated server/client to grpc_server.rs
    tonic_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(out_dir.join("operation_descriptor.bin"))
        .compile_protos(
            &[
                "proto/operation.proto",
                "proto/mail.proto",
                "proto/privacy_network.proto",
                "proto/registration.proto",
                "proto/registry.proto",
                "proto/emqx_exhook_v2.proto",
                "../op-chat/proto/chat.proto",
                "src/grpc/zeroclaw.proto",
                generated_proto
                    .to_str()
                    .ok_or("generated plugin proto path is not valid UTF-8")?,
            ],
            &[
                "proto",
                "../op-chat/proto",
                "src/grpc",
                out_dir
                    .to_str()
                    .ok_or("generated proto include path is not valid UTF-8")?,
            ],
        )?;

    println!("cargo:rerun-if-changed=proto/operation.proto");
    println!("cargo:rerun-if-changed=proto/mail.proto");
    println!("cargo:rerun-if-changed=proto/privacy_network.proto");
    println!("cargo:rerun-if-changed=proto/registration.proto");
    println!("cargo:rerun-if-changed=proto/registry.proto");
    println!("cargo:rerun-if-changed=proto/emqx_exhook_v2.proto");
    println!("cargo:rerun-if-changed=../op-chat/proto/chat.proto");
    println!("cargo:rerun-if-changed=src/grpc/zeroclaw.proto");
    println!("cargo:rerun-if-changed=../op-plugins/src/state_plugins");
    println!("cargo:rerun-if-changed=build.rs");

    Ok(())
}

#[derive(Debug, Clone)]
struct PluginMethodSet {
    plugin_id: String,
    service_name: String,
    server_module: String,
    server_type: String,
    trait_name: String,
    methods: Vec<PluginMethod>,
}

#[derive(Debug, Clone)]
struct PluginMethod {
    rpc_name: String,
    rust_name: String,
    schema_name: String,
    input_name: String,
    output_name: String,
    args: simd_json::OwnedValue,
    returns: simd_json::OwnedValue,
}

fn collect_plugin_methods() -> Vec<PluginMethodSet> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build plugin schema collection runtime");
    rt.block_on(async {
        let store = Arc::new(op_state_store::MemoryStore::new());
        let registry = op_plugins::DefaultPluginRegistry::new(store);
        let mut sets = Vec::new();

        for plugin_id in op_plugins::DefaultPluginRegistry::available_plugins() {
            let Ok(plugin) = registry.load_plugin(&plugin_id).await else {
                continue;
            };
            let Some(schema) = plugin.schema() else {
                continue;
            };
            if schema.methods.is_empty() {
                continue;
            }

            let mut method_names = schema.methods.keys().cloned().collect::<Vec<_>>();
            method_names.sort();
            let methods = method_names
                .into_iter()
                .map(|schema_name| {
                    let method_decl = schema
                        .methods
                        .get(&schema_name)
                        .expect("method declaration must exist");
                    let rpc_name = to_pascal_ident(&schema_name);
                    let message_prefix = to_pascal_ident(&plugin_id);
                    PluginMethod {
                        rust_name: to_snake_ident(&rpc_name),
                        rpc_name: rpc_name.clone(),
                        schema_name,
                        input_name: format!("{message_prefix}{rpc_name}Request"),
                        output_name: format!("{message_prefix}{rpc_name}Response"),
                        args: method_decl.args.clone(),
                        returns: method_decl
                            .returns
                            .clone()
                            .unwrap_or_else(|| simd_json::json!({})),
                    }
                })
                .collect::<Vec<_>>();

            let trait_name = format!("{}PluginMethods", to_pascal_ident(&plugin_id));
            sets.push(PluginMethodSet {
                plugin_id: plugin_id.clone(),
                service_name: trait_name.clone(),
                server_module: format!("{}_server", to_snake_ident(&trait_name)),
                server_type: format!("{}Server", trait_name),
                trait_name,
                methods,
            });
        }

        sets.sort_by(|left, right| left.plugin_id.cmp(&right.plugin_id));
        sets
    })
}

fn generate_plugin_methods_proto(sets: &[PluginMethodSet]) -> String {
    let mut proto = String::new();
    writeln!(proto, "syntax = \"proto3\";").unwrap();
    writeln!(proto).unwrap();
    writeln!(proto, "package operation.plugin.v1;").unwrap();
    writeln!(proto).unwrap();
    writeln!(proto, "import \"google/protobuf/struct.proto\";").unwrap();
    writeln!(proto).unwrap();

    for set in sets {
        writeln!(proto, "// PluginSchema source: {}", set.plugin_id).unwrap();
        for method in &set.methods {
            writeln!(
                proto,
                "{}",
                schema_message_proto(&method.input_name, &method.args)
            )
            .unwrap();
            writeln!(
                proto,
                "{}",
                schema_message_proto(&method.output_name, &method.returns)
            )
            .unwrap();
        }
        writeln!(proto, "service {} {{", set.service_name).unwrap();
        for method in &set.methods {
            writeln!(
                proto,
                "  rpc {}({}) returns ({});",
                method.rpc_name, method.input_name, method.output_name
            )
            .unwrap();
        }
        writeln!(proto, "}}").unwrap();
        writeln!(proto).unwrap();
    }

    proto
}

fn generate_plugin_method_routes(sets: &[PluginMethodSet]) -> String {
    let mut rust = String::new();
    writeln!(rust, "#[allow(clippy::all)]").unwrap();
    writeln!(rust, "#[allow(unused_qualifications)]").unwrap();
    writeln!(rust, "mod generated_plugin_method_routes {{").unwrap();
    writeln!(rust, "    use crate::grpc_server::OperationGrpcServer;").unwrap();
    writeln!(rust, "    use tonic::{{Request, Response, Status}};").unwrap();
    writeln!(rust).unwrap();

    for set in sets {
        writeln!(
            rust,
            "    #[tonic::async_trait]\n    impl crate::proto::plugin_methods::{}::{} for OperationGrpcServer {{",
            set.server_module, set.trait_name
        )
        .unwrap();
        for method in &set.methods {
            writeln!(
                rust,
                "        async fn {}(&self, request: Request<crate::proto::plugin_methods::{}>) -> Result<Response<crate::proto::plugin_methods::{}>, Status> {{",
                method.rust_name, method.input_name, method.output_name
            )
            .unwrap();
            writeln!(
                rust,
                "            self.call_generated_plugin_method_typed({:?}, {:?}, {:?}, {:?}, request).await",
                set.plugin_id, method.schema_name, method.input_name, method.output_name
            )
            .unwrap();
            writeln!(rust, "        }}").unwrap();
        }
        writeln!(rust, "    }}").unwrap();
        writeln!(rust).unwrap();
    }

    writeln!(
        rust,
        "    pub(crate) fn add_routes(mut routes: tonic::service::Routes, server: OperationGrpcServer) -> tonic::service::Routes {{"
    )
    .unwrap();
    for set in sets {
        writeln!(
            rust,
            "        routes = routes.add_service(tonic_web::enable(crate::proto::plugin_methods::{}::{}::new(server.clone())));",
            set.server_module, set.server_type
        )
        .unwrap();
    }
    writeln!(rust, "        routes").unwrap();
    writeln!(rust, "    }}").unwrap();
    writeln!(rust).unwrap();

    // Emitted from the same `sets` that produced `add_routes`, so the names
    // reflection advertises can never drift from the ones actually mounted.
    writeln!(
        rust,
        "    pub(crate) const LEGACY_PLUGIN_METHOD_SERVICES: &[(&str, &str)] = &["
    )
    .unwrap();
    for set in sets {
        writeln!(
            rust,
            "        ({:?}, {:?}),",
            set.plugin_id,
            format!("operation.plugin.v1.{}", set.service_name)
        )
        .unwrap();
    }
    writeln!(rust, "    ];").unwrap();
    writeln!(rust, "}}").unwrap();
    writeln!(
        rust,
        "pub(crate) use generated_plugin_method_routes::{{add_routes, LEGACY_PLUGIN_METHOD_SERVICES}};"
    )
    .unwrap();
    rust
}

/// Nested message names already emitted into the current generated file.
///
/// Definitions are shared across methods (e.g. `IncusInstance` is referenced by
/// many incus methods), and every generated message lands in one flat
/// `operation.plugin.v1` package — so emission must be de-duplicated at *file*
/// scope, not per message, or protoc rejects the redefinitions. build.rs is a
/// short-lived single-run process, so a process-global set is the whole lifetime.
fn emitted_nested() -> &'static std::sync::Mutex<std::collections::BTreeSet<String>> {
    static EMITTED: std::sync::OnceLock<std::sync::Mutex<std::collections::BTreeSet<String>>> =
        std::sync::OnceLock::new();
    EMITTED.get_or_init(|| std::sync::Mutex::new(std::collections::BTreeSet::new()))
}

fn schema_message_proto(message_name: &str, schema: &simd_json::OwnedValue) -> String {
    let (fields, nested) = schema_fields_with_nested(schema);
    let mut proto = String::new();

    // Emit referenced object definitions as real nested messages first, so the
    // descriptor carries their full field detail instead of collapsing them to
    // `google.protobuf.Struct`/`Value`. Consumers generating clients from these
    // descriptors (the dashboard) then see typed shapes, not opaque blobs.
    {
        let mut already = emitted_nested().lock().expect("emitted-nested lock");
        for (nested_name, nested_fields) in &nested {
            if !already.insert(nested_name.clone()) {
                continue; // already emitted earlier in this file
            }
            proto.push_str(&format!("message {} {{\n", nested_name));
            if nested_fields.is_empty() {
                proto.push_str("  // Empty schema object.\n");
            }
            for field in nested_fields {
                let repeated = if field.repeated { "repeated " } else { "" };
                proto.push_str(&format!(
                    "  {}{} {} = {};\n",
                    repeated, field.proto_type, field.name, field.number
                ));
            }
            proto.push_str("}\n\n");
        }
    }

    proto.push_str(&format!("message {} {{\n", message_name));

    if fields.is_empty() {
        proto.push_str("  // Empty schema object.\n");
    }

    for field in fields {
        let repeated = if field.repeated { "repeated " } else { "" };
        proto.push_str(&format!(
            "  {}{} {} = {};\n",
            repeated, field.proto_type, field.name, field.number
        ));
    }

    proto.push_str("}\n");
    proto
}

#[derive(Debug, Clone)]
struct ProtoField {
    name: String,
    proto_type: String,
    repeated: bool,
    number: i32,
}

fn schema_fields(schema: &simd_json::OwnedValue) -> Vec<ProtoField> {
    schema_fields_with_nested(schema).0
}

/// Field list for `schema`, plus every object definition it references, in
/// declaration order and de-duplicated.
///
/// schemars emits nested types as `$ref: "#/definitions/<Name>"` (or `$defs`)
/// with the bodies in a sibling map. Resolving those refs is what lets a nested
/// struct become a real proto message rather than an opaque
/// `google.protobuf.Struct`/`Value`.
type NestedMessages = Vec<(String, Vec<ProtoField>)>;

fn schema_fields_with_nested(schema: &simd_json::OwnedValue) -> (Vec<ProtoField>, NestedMessages) {
    let Ok(json_schema) = serde_json::to_value(schema) else {
        return (
            vec![fallback_field("payload", "google.protobuf.Value", false)],
            Vec::new(),
        );
    };

    let defs = collect_definitions(&json_schema);
    let mut nested: NestedMessages = Vec::new();
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();

    let Some(properties) = json_schema
        .get("properties")
        .and_then(|value| value.as_object())
    else {
        return (Vec::new(), Vec::new());
    };

    let mut props = properties.iter().collect::<Vec<_>>();
    props.sort_by_key(|(name, _)| *name);

    let fields = props
        .into_iter()
        .map(|(name, property)| {
            let (proto_type, repeated) =
                resolve_proto_type(property, &defs, &mut nested, &mut seen, 0);
            ProtoField {
                name: sanitize_proto_ident(name),
                proto_type,
                repeated,
                number: stable_field_number(name),
            }
        })
        .collect();

    (fields, nested)
}

/// Gather `$defs` and `definitions` (schemars uses either depending on version)
/// into one lookup table.
fn collect_definitions(
    root: &serde_json::Value,
) -> std::collections::BTreeMap<String, serde_json::Value> {
    let mut out = std::collections::BTreeMap::new();
    for key in ["$defs", "definitions"] {
        if let Some(map) = root.get(key).and_then(|v| v.as_object()) {
            for (name, body) in map {
                out.insert(name.clone(), body.clone());
            }
        }
    }
    out
}

/// Extract the definition name from a `$ref` like `#/definitions/NodeSummary`.
fn ref_target_name(schema: &serde_json::Value) -> Option<String> {
    schema
        .get("$ref")
        .and_then(|v| v.as_str())
        .and_then(|r| r.rsplit('/').next())
        .map(str::to_string)
}

/// Depth cap guards against self-referential schemas producing infinite nesting.
const MAX_NESTED_DEPTH: usize = 6;

fn resolve_proto_type(
    schema: &serde_json::Value,
    defs: &std::collections::BTreeMap<String, serde_json::Value>,
    nested: &mut NestedMessages,
    seen: &mut std::collections::BTreeSet<String>,
    depth: usize,
) -> (String, bool) {
    // `$ref` to a named object definition becomes a real message type.
    if let Some(name) = ref_target_name(schema) {
        if depth < MAX_NESTED_DEPTH {
            if let Some(body) = defs.get(&name) {
                let ident = sanitize_message_ident(&name);
                if seen.insert(ident.clone()) {
                    // Reserve the slot before recursing so a cycle sees it as visited.
                    nested.push((ident.clone(), Vec::new()));
                    let idx = nested.len() - 1;
                    let inner = object_fields(body, defs, nested, seen, depth + 1);
                    nested[idx].1 = inner;
                }
                return (ident, false);
            }
        }
        return ("google.protobuf.Value".to_string(), false);
    }

    // `allOf`/`oneOf`/`anyOf` wrapping a single ref (schemars Option<T>, newtypes).
    for key in ["allOf", "oneOf", "anyOf"] {
        if let Some(arr) = schema.get(key).and_then(|v| v.as_array()) {
            let candidates: Vec<&serde_json::Value> = arr
                .iter()
                .filter(|v| v.get("type").and_then(|t| t.as_str()) != Some("null"))
                .collect();
            if candidates.len() == 1 {
                return resolve_proto_type(candidates[0], defs, nested, seen, depth);
            }
        }
    }

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
                    .map(|items| resolve_proto_type(items, defs, nested, seen, depth).0)
                    .unwrap_or_else(|| "google.protobuf.Value".to_string());
                (item_type, true)
            }
            "object" => {
                // Inline object with properties: only representable as Struct,
                // since it has no name to mint a message from.
                if schema.get("properties").is_some() {
                    ("google.protobuf.Struct".to_string(), false)
                } else {
                    ("google.protobuf.Struct".to_string(), false)
                }
            }
            _ => ("google.protobuf.Value".to_string(), false),
        },
        Some(serde_json::Value::Array(kinds)) => {
            let first_non_null = kinds
                .iter()
                .filter_map(|value| value.as_str())
                .find(|kind| *kind != "null")
                .unwrap_or("object");
            let mut reduced = schema.clone();
            if let Some(obj) = reduced.as_object_mut() {
                obj.insert(
                    "type".to_string(),
                    serde_json::Value::String(first_non_null.to_string()),
                );
            }
            resolve_proto_type(&reduced, defs, nested, seen, depth)
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

/// Field list for a resolved object definition body.
fn object_fields(
    body: &serde_json::Value,
    defs: &std::collections::BTreeMap<String, serde_json::Value>,
    nested: &mut NestedMessages,
    seen: &mut std::collections::BTreeSet<String>,
    depth: usize,
) -> Vec<ProtoField> {
    let Some(properties) = body.get("properties").and_then(|v| v.as_object()) else {
        return Vec::new();
    };
    let mut props = properties.iter().collect::<Vec<_>>();
    props.sort_by_key(|(name, _)| *name);
    props
        .into_iter()
        .map(|(name, property)| {
            let (proto_type, repeated) = resolve_proto_type(property, defs, nested, seen, depth);
            ProtoField {
                name: sanitize_proto_ident(name),
                proto_type,
                repeated,
                number: stable_field_number(name),
            }
        })
        .collect()
}

/// Definition names arrive as Rust type identifiers already; keep them but
/// strip anything not valid in a proto message name.
fn sanitize_message_ident(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if cleaned.is_empty() {
        "Value".to_string()
    } else {
        cleaned
    }
}

fn fallback_field(name: &str, proto_type: &str, repeated: bool) -> ProtoField {
    ProtoField {
        name: name.to_string(),
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

fn stable_field_number(name: &str) -> i32 {
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

fn sanitize_proto_ident(value: &str) -> String {
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

fn to_pascal_ident(value: &str) -> String {
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
    } else if ident.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        format!("P{}", ident)
    } else {
        ident
    }
}

fn to_snake_ident(value: &str) -> String {
    let mut ident = String::new();
    let mut previous_was_underscore = false;
    for (idx, ch) in value.chars().enumerate() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() {
                if idx > 0 && !previous_was_underscore {
                    ident.push('_');
                }
                ident.push(ch.to_ascii_lowercase());
                previous_was_underscore = false;
            } else {
                ident.push(ch);
                previous_was_underscore = false;
            }
        } else if !previous_was_underscore && !ident.is_empty() {
            ident.push('_');
            previous_was_underscore = true;
        }
    }
    while ident.ends_with('_') {
        ident.pop();
    }
    if ident.is_empty() {
        "generated".to_string()
    } else if ident.chars().next().is_some_and(|ch| ch.is_ascii_digit()) {
        format!("p_{}", ident)
    } else {
        ident
    }
}
