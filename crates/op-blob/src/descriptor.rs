//! Typed gRPC reflection descriptors synthesized from `PluginSchema.methods`
//! (REQ-6.1): per-method `FileDescriptorProto`s with real field types derived
//! from `MethodDecl.args` / `MethodDecl.returns` JSON Schemas — never
//! `google.protobuf.Struct`.
//!
//! Naming follows the spec's acceptance criterion (Task 3.2):
//!
//! ```text
//! file:    operation/method/<plugin>/<method>.proto
//! package: operation.method.<plugin>.<method>
//! service: <Method>Service   rpc <Method>(<Method>Input) returns (<Method>Output)
//! ```
//!
//! The protobuf wire encoding is written by hand (varint + length-delimited
//! fields of descriptor.proto messages) so the crate stays dependency-free.
//! The bytes are a valid `google.protobuf.FileDescriptorSet` consumable by
//! tonic-reflection / grpcurl.

use crate::schema::{MethodDecl, PluginSchema};
use serde_json::Value as JVal;

// FieldDescriptorProto.Type values (descriptor.proto)
const T_DOUBLE: u64 = 1;
const T_INT64: u64 = 3;
const T_BOOL: u64 = 8;
const T_STRING: u64 = 9;
const T_MESSAGE: u64 = 11;
// FieldDescriptorProto.Label values
const L_OPTIONAL: u64 = 1;
const L_REPEATED: u64 = 3;

/// One synthesized per-method descriptor file plus its reflection index keys.
#[derive(Debug, Clone)]
pub struct MethodDescriptor {
    pub method_name: String,
    pub file_name: String,
    pub package: String,
    /// Fully-qualified service name, e.g. `operation.method.unix_socket.bind.BindService`.
    pub service_full: String,
    /// `/package.Service/Rpc` — the callable route path.
    pub grpc_path: String,
    pub input_full: String,
    pub output_full: String,
    /// All fully-qualified symbols this file defines (for symbol lookup).
    pub symbols: Vec<String>,
    /// Encoded `FileDescriptorProto` bytes.
    pub file_descriptor_proto: Vec<u8>,
}

/// Synthesize the full `FileDescriptorSet` for one plugin, one file per
/// method, sorted by method name for byte determinism (NFR-2).
pub fn descriptor_set_for_plugin(
    plugin_id: &str,
    schema: &PluginSchema,
) -> (Vec<u8>, Vec<MethodDescriptor>) {
    let mut names: Vec<&String> = schema.methods.keys().collect();
    names.sort();
    let mut infos = Vec::new();
    let mut set = Vec::new();
    for name in names {
        let info = method_file_descriptor(plugin_id, &schema.methods[name]);
        // FileDescriptorSet.file = 1 (repeated message)
        put_bytes(&mut set, 1, &info.file_descriptor_proto);
        infos.push(info);
    }
    (set, infos)
}

/// Build one per-method `FileDescriptorProto` with typed Input/Output messages.
pub fn method_file_descriptor(plugin_id: &str, method: &MethodDecl) -> MethodDescriptor {
    let snake = snake_case(&method.name);
    let pascal = pascal_case(&method.name);
    let package = format!("operation.method.{plugin_id}.{snake}");
    let file_name = format!("operation/method/{plugin_id}/{snake}.proto");

    let input_msg = message_from_json_schema(&format!("{pascal}Input"), &method.args);
    let empty = serde_json::json!({"type": "object"});
    let output_schema = method.returns.as_ref().unwrap_or(&empty);
    let output_msg = message_from_json_schema(&format!("{pascal}Output"), output_schema);

    let input_full = format!("{package}.{pascal}Input");
    let output_full = format!("{package}.{pascal}Output");
    let service_full = format!("{package}.{pascal}Service");

    // MethodDescriptorProto
    let mut m = Vec::new();
    put_str(&mut m, 1, &pascal);
    put_str(&mut m, 2, &format!(".{input_full}"));
    put_str(&mut m, 3, &format!(".{output_full}"));

    // ServiceDescriptorProto
    let mut svc = Vec::new();
    put_str(&mut svc, 1, &format!("{pascal}Service"));
    put_bytes(&mut svc, 2, &m);

    // FileDescriptorProto
    let mut file = Vec::new();
    put_str(&mut file, 1, &file_name);
    put_str(&mut file, 2, &package);
    put_bytes(&mut file, 4, &encode_message(&input_msg));
    put_bytes(&mut file, 4, &encode_message(&output_msg));
    put_bytes(&mut file, 6, &svc);
    put_str(&mut file, 12, "proto3");

    let mut symbols = vec![
        service_full.clone(),
        input_full.clone(),
        output_full.clone(),
    ];
    symbols.push(format!("{service_full}.{pascal}"));
    collect_nested_symbols(
        &input_msg,
        &format!("{package}.{}", input_msg.name),
        &mut symbols,
    );
    collect_nested_symbols(
        &output_msg,
        &format!("{package}.{}", output_msg.name),
        &mut symbols,
    );

    MethodDescriptor {
        method_name: method.name.clone(),
        grpc_path: format!("/{service_full}/{pascal}"),
        file_name,
        package,
        service_full,
        input_full,
        output_full,
        symbols,
        file_descriptor_proto: file,
    }
}

// ---------------------------------------------------------------------------
// JSON Schema -> DescriptorProto message synthesis
// ---------------------------------------------------------------------------

struct MessageDef {
    name: String,
    fields: Vec<FieldDef>,
    nested: Vec<MessageDef>,
}

struct FieldDef {
    name: String,
    number: u64,
    repeated: bool,
    proto_type: u64,
    /// Relative type name for `T_MESSAGE` fields (a nested message).
    type_name: Option<String>,
}

/// Walk a schemars JSON Schema document into a proto message definition.
/// Objects with properties become nested messages; scalars map directly;
/// enums/unknown/any degrade to string (JSON-encoded).
fn message_from_json_schema(msg_name: &str, schema: &JVal) -> MessageDef {
    let defs = schema
        .get("$defs")
        .or_else(|| schema.get("definitions"))
        .and_then(JVal::as_object);
    build_message(msg_name, resolve(schema, defs), defs)
}

fn build_message(
    name: &str,
    node: &JVal,
    defs: Option<&serde_json::Map<String, JVal>>,
) -> MessageDef {
    let mut fields = Vec::new();
    let mut nested = Vec::new();
    if let Some(props) = node.get("properties").and_then(JVal::as_object) {
        // serde_json Map is BTreeMap-backed: iteration (and thus field
        // numbering) is name-sorted and deterministic.
        for (i, (fname, fnode)) in props.iter().enumerate() {
            let number = (i + 1) as u64;
            let (repeated, item) = match unwrap_array(fnode, defs) {
                Some(item) => (true, item),
                None => (false, resolve(fnode, defs).clone()),
            };
            let (proto_type, type_name) = proto_type_of(&item, defs, fname, &mut nested);
            fields.push(FieldDef {
                name: fname.clone(),
                number,
                repeated,
                proto_type,
                type_name,
            });
        }
    }
    MessageDef {
        name: name.to_string(),
        fields,
        nested,
    }
}

/// If `node` is an array schema, return its resolved item schema.
fn unwrap_array(node: &JVal, defs: Option<&serde_json::Map<String, JVal>>) -> Option<JVal> {
    let node = resolve(node, defs);
    if json_type(node) == Some("array") {
        let item = node.get("items").map(|i| resolve(i, defs).clone());
        Some(item.unwrap_or(serde_json::json!({})))
    } else {
        None
    }
}

fn proto_type_of(
    node: &JVal,
    defs: Option<&serde_json::Map<String, JVal>>,
    field_name: &str,
    nested: &mut Vec<MessageDef>,
) -> (u64, Option<String>) {
    let node = resolve(node, defs);

    // Option<T>: collapse anyOf [T, null] to T.
    if let Some(alts) = node
        .get("anyOf")
        .or_else(|| node.get("oneOf"))
        .and_then(JVal::as_array)
    {
        let non_null: Vec<&JVal> = alts
            .iter()
            .filter(|a| json_type(a) != Some("null"))
            .collect();
        if let [single] = non_null.as_slice() {
            return proto_type_of(single, defs, field_name, nested);
        }
        // Multi-branch union: JSON-encoded string.
        return (T_STRING, None);
    }

    match json_type(node) {
        Some("string") => (T_STRING, None),
        Some("integer") => (T_INT64, None),
        Some("number") => (T_DOUBLE, None),
        Some("boolean") => (T_BOOL, None),
        Some("object") if node.get("properties").is_some() => {
            let msg_name = format!("{}Msg", pascal_case(field_name));
            let def = build_message(&msg_name, node, defs);
            nested.push(def);
            (T_MESSAGE, Some(msg_name))
        }
        // enum / free-form object / any -> JSON-encoded string
        _ => (T_STRING, None),
    }
}

fn resolve<'a>(node: &'a JVal, defs: Option<&'a serde_json::Map<String, JVal>>) -> &'a JVal {
    if let (Some(r), Some(d)) = (node.get("$ref").and_then(JVal::as_str), defs) {
        if let Some(key) = r.rsplit('/').next() {
            if let Some(target) = d.get(key) {
                return resolve(target, defs);
            }
        }
    }
    node
}

fn json_type(node: &JVal) -> Option<&str> {
    match node.get("type") {
        Some(JVal::String(s)) => Some(s.as_str()),
        Some(JVal::Array(a)) => a.iter().filter_map(JVal::as_str).find(|t| *t != "null"),
        _ => None,
    }
}

fn collect_nested_symbols(msg: &MessageDef, full: &str, out: &mut Vec<String>) {
    for n in &msg.nested {
        let child = format!("{full}.{}", n.name);
        out.push(child.clone());
        collect_nested_symbols(n, &child, out);
    }
}

// ---------------------------------------------------------------------------
// descriptor.proto wire encoding
// ---------------------------------------------------------------------------

fn encode_message(def: &MessageDef) -> Vec<u8> {
    // DescriptorProto: name=1, field=2, nested_type=3
    let mut out = Vec::new();
    put_str(&mut out, 1, &def.name);
    for f in &def.fields {
        // FieldDescriptorProto: name=1, number=3, label=4, type=5, type_name=6, json_name=10
        let mut fd = Vec::new();
        put_str(&mut fd, 1, &f.name);
        put_varint_field(&mut fd, 3, f.number);
        put_varint_field(&mut fd, 4, if f.repeated { L_REPEATED } else { L_OPTIONAL });
        put_varint_field(&mut fd, 5, f.proto_type);
        if let Some(t) = &f.type_name {
            // Relative reference resolves against the enclosing scope.
            put_str(&mut fd, 6, t);
        }
        put_str(&mut fd, 10, &json_name(&f.name));
        put_bytes(&mut out, 2, &fd);
    }
    for n in &def.nested {
        put_bytes(&mut out, 3, &encode_message(n));
    }
    out
}

fn put_varint(out: &mut Vec<u8>, mut v: u64) {
    loop {
        let b = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            out.push(b);
            break;
        }
        out.push(b | 0x80);
    }
}

fn put_tag(out: &mut Vec<u8>, field: u32, wire: u8) {
    put_varint(out, ((field as u64) << 3) | wire as u64);
}

fn put_varint_field(out: &mut Vec<u8>, field: u32, v: u64) {
    put_tag(out, field, 0);
    put_varint(out, v);
}

fn put_str(out: &mut Vec<u8>, field: u32, s: &str) {
    put_bytes(out, field, s.as_bytes());
}

fn put_bytes(out: &mut Vec<u8>, field: u32, b: &[u8]) {
    put_tag(out, field, 2);
    put_varint(out, b.len() as u64);
    out.extend_from_slice(b);
}

fn json_name(snake: &str) -> String {
    let mut out = String::new();
    let mut upper = false;
    for c in snake.chars() {
        if c == '_' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

pub fn pascal_case(s: &str) -> String {
    let mut out = String::new();
    let mut upper = true;
    for c in s.chars() {
        if c == '_' || c == '-' || c == '.' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    out
}

pub fn snake_case(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 && !out.ends_with('_') {
                out.push('_');
            }
            out.extend(c.to_lowercase());
        } else if c == '-' || c == '.' {
            out.push('_');
        } else {
            out.push(c);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Minimal FileDescriptorSet decoding — enough for reflection indexing,
// catalog `file_by_filename`, and tests. Returns borrowed subslices of the
// input (zero-copy).
// ---------------------------------------------------------------------------

/// Summary of one file entry inside an encoded `FileDescriptorSet`, borrowing
/// the underlying descriptor bytes.
#[derive(Debug)]
pub struct FileSummary<'a> {
    pub name: String,
    pub package: String,
    pub messages: Vec<String>,
    pub services: Vec<String>,
    /// Borrowed `FileDescriptorProto` bytes within the set.
    pub bytes: &'a [u8],
}

/// Iterate the `file` entries of an encoded `FileDescriptorSet`.
pub fn decode_file_summaries(set: &[u8]) -> Result<Vec<FileSummary<'_>>, String> {
    let mut out = Vec::new();
    let mut r = Reader { buf: set, pos: 0 };
    while !r.done() {
        let (field, wire) = r.tag()?;
        if field == 1 && wire == 2 {
            let bytes = r.len_delim()?;
            out.push(decode_file_summary(bytes)?);
        } else {
            r.skip(wire)?;
        }
    }
    Ok(out)
}

fn decode_file_summary(bytes: &[u8]) -> Result<FileSummary<'_>, String> {
    let mut s = FileSummary {
        name: String::new(),
        package: String::new(),
        messages: Vec::new(),
        services: Vec::new(),
        bytes,
    };
    let mut r = Reader { buf: bytes, pos: 0 };
    while !r.done() {
        let (field, wire) = r.tag()?;
        match (field, wire) {
            (1, 2) => s.name = r.string()?,
            (2, 2) => s.package = r.string()?,
            (4, 2) => s.messages.push(first_string_field(r.len_delim()?)?),
            (6, 2) => s.services.push(first_string_field(r.len_delim()?)?),
            _ => r.skip(wire)?,
        }
    }
    Ok(s)
}

/// Read field #1 (name) of a DescriptorProto/ServiceDescriptorProto.
fn first_string_field(bytes: &[u8]) -> Result<String, String> {
    let mut r = Reader { buf: bytes, pos: 0 };
    while !r.done() {
        let (field, wire) = r.tag()?;
        if field == 1 && wire == 2 {
            return r.string();
        }
        r.skip(wire)?;
    }
    Ok(String::new())
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn done(&self) -> bool {
        self.pos >= self.buf.len()
    }

    fn varint(&mut self) -> Result<u64, String> {
        let mut v = 0u64;
        let mut shift = 0;
        loop {
            let b = *self
                .buf
                .get(self.pos)
                .ok_or_else(|| "varint overruns buffer".to_string())?;
            self.pos += 1;
            v |= ((b & 0x7f) as u64) << shift;
            if b & 0x80 == 0 {
                return Ok(v);
            }
            shift += 7;
            if shift > 63 {
                return Err("varint too long".into());
            }
        }
    }

    fn tag(&mut self) -> Result<(u32, u8), String> {
        let t = self.varint()?;
        Ok(((t >> 3) as u32, (t & 7) as u8))
    }

    fn len_delim(&mut self) -> Result<&'a [u8], String> {
        let len = self.varint()? as usize;
        let end = self.pos + len;
        if end > self.buf.len() {
            return Err("length-delimited field overruns buffer".into());
        }
        let s = &self.buf[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    fn string(&mut self) -> Result<String, String> {
        Ok(String::from_utf8_lossy(self.len_delim()?).into_owned())
    }

    fn skip(&mut self, wire: u8) -> Result<(), String> {
        match wire {
            0 => {
                self.varint()?;
            }
            1 => self.pos += 8,
            2 => {
                self.len_delim()?;
            }
            5 => self.pos += 4,
            w => return Err(format!("unsupported wire type {w}")),
        }
        if self.pos > self.buf.len() {
            return Err("skip overruns buffer".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{method_decl_from_schemars_with_output, AckOutput};
    use crate::schema::SideEffect;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, schemars::JsonSchema)]
    struct SetDeviceInput {
        interface: String,
        private_key: String,
        listen_port: Option<u16>,
        fwmark: Option<u32>,
    }

    #[test]
    fn typed_descriptor_matches_spec_naming() {
        let decl = method_decl_from_schemars_with_output::<SetDeviceInput, AckOutput>(
            "set_device",
            SideEffect::Mutation,
            false,
            "wireguard.write",
            "mut.network.wireguard.set-device@v1",
        );
        let d = method_file_descriptor("wireguard", &decl);
        assert_eq!(d.package, "operation.method.wireguard.set_device");
        assert_eq!(
            d.service_full,
            "operation.method.wireguard.set_device.SetDeviceService"
        );
        assert_eq!(
            d.input_full,
            "operation.method.wireguard.set_device.SetDeviceInput"
        );
        assert_eq!(
            d.grpc_path,
            "/operation.method.wireguard.set_device.SetDeviceService/SetDevice"
        );

        // Round-trip through the wire decoder: real typed messages, no Struct.
        let mut set = Vec::new();
        put_bytes(&mut set, 1, &d.file_descriptor_proto);
        let files = decode_file_summaries(&set).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "operation/method/wireguard/set_device.proto");
        assert_eq!(files[0].services, vec!["SetDeviceService"]);
        assert_eq!(files[0].messages, vec!["SetDeviceInput", "SetDeviceOutput"]);
    }

    #[test]
    fn case_conversions() {
        assert_eq!(pascal_case("set_device"), "SetDevice");
        assert_eq!(snake_case("SetDevice"), "set_device");
        assert_eq!(json_name("listen_port"), "listenPort");
    }
}
