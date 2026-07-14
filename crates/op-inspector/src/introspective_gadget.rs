//! Introspective Gadget - Universal Object Inspector
//!
//! Like Inspector Gadget, but for data structures! 🕵️‍♂️
//!
//! This is the universal object inspector that can analyze ANY data structure
//! and add it to the knowledge base for schema generation and understanding.
//!
//! Examples:
//! - Docker containers: Inspect running containers, extract configurations
//! - XML data: Parse unknown XML structures, generate schemas
//! - JSON objects: Analyze complex nested structures
//! - Binary data: Attempt to reverse engineer structures
//! - Legacy formats: Handle old/obscure data formats (like Apple Lisa disks)
//!
//! The gadget can:
//! 1. Accept any input source (file, URL, data stream, etc.)
//! 2. Attempt multiple parsing strategies
//! 3. Extract structural information
//! 4. Generate validation schemas
//! 5. Add to knowledge base for future use
//! 6. Create templates for similar objects

use anyhow::{Context, Result};
use base64::engine::general_purpose;
use base64::Engine;
use regex::Regex;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;
use std::path::Path;

// Stub types for KnowledgeBase and SchemaDefinition until op-mcp is built
#[derive(Debug, Clone, Default)]
pub struct KnowledgeBase {
    pub schemas: HashMap<String, SchemaDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaDefinition {
    pub name: String,
    pub object_type: String,
    pub source_type: String,
    pub source_data: Option<String>,
    pub schema: Value,
    pub generated_schemas: HashMap<String, String>,
    pub validation_rules: Vec<String>,
    pub examples: Vec<Value>,
    pub metadata: HashMap<String, String>,
}

/// Cap on retained examples per knowledge-base entry, so repeated inspection
/// of a high-traffic shape doesn't grow `examples` unbounded.
const MAX_RETAINED_EXAMPLES: usize = 10;

/// Merge a freshly-inspected `SchemaDefinition` into an existing entry of the
/// same name, narrowing `required` to the intersection across every sample
/// seen so far and unioning `properties` (a field absent from `new` but
/// present in `existing` — or vice versa — is optional, not missing).
fn merge_schema_definitions(existing: &SchemaDefinition, new: &SchemaDefinition) -> SchemaDefinition {
    let merged_schema = merge_schema_values(&existing.schema, &new.schema);

    let mut validation_rules = existing.validation_rules.clone();
    for rule in &new.validation_rules {
        if !validation_rules.contains(rule) {
            validation_rules.push(rule.clone());
        }
    }

    let mut examples = existing.examples.clone();
    examples.extend(new.examples.iter().cloned());
    if examples.len() > MAX_RETAINED_EXAMPLES {
        let overflow = examples.len() - MAX_RETAINED_EXAMPLES;
        examples.drain(0..overflow);
    }

    let mut metadata = existing.metadata.clone();
    metadata.extend(new.metadata.clone());
    metadata.insert(
        "sample_count".to_string(),
        (existing
            .metadata
            .get("sample_count")
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(1)
            + 1)
        .to_string(),
    );

    SchemaDefinition {
        name: existing.name.clone(),
        object_type: new.object_type.clone(),
        source_type: new.source_type.clone(),
        source_data: new.source_data.clone(),
        schema: merged_schema,
        generated_schemas: existing.generated_schemas.clone(),
        validation_rules,
        examples,
        metadata,
    }
}

/// Merge two `ObjectSchema::to_value()` JSON shapes: union `properties`,
/// intersect `required` (a field must appear in every sample to stay
/// required), and keep the newer `type`/`array_items`/`object_patterns`.
fn merge_schema_values(old: &Value, new: &Value) -> Value {
    use std::collections::HashSet;

    let old_props = old
        .get("properties")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();
    let new_props = new
        .get("properties")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    let str_set = |v: &Value| -> HashSet<String> {
        v.as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default()
    };
    let old_required = old.get("required").map(str_set).unwrap_or_default();
    let new_required = new.get("required").map(str_set).unwrap_or_default();

    let mut merged_required: Vec<String> = old_required
        .intersection(&new_required)
        .cloned()
        .collect();
    merged_required.sort();

    let mut merged_props = old_props;
    for (k, v) in new_props {
        merged_props.entry(k).or_insert(v);
    }

    json!({
        "type": new.get("type").cloned().unwrap_or_else(|| Value::from("object")),
        "properties": merged_props,
        "required": merged_required,
        "array_items": new.get("array_items").cloned(),
        "object_patterns": new.get("object_patterns").cloned().unwrap_or_else(|| Value::from(Vec::<Value>::new())),
    })
}

// ============================================================================
// INTROSPECTIVE GADGET - THE OBJECT INSPECTOR
// ============================================================================

/// The main Introspective Gadget - universal object inspector
#[derive(Clone)]
pub struct IntrospectiveGadget {
    knowledge_base: std::sync::Arc<tokio::sync::RwLock<KnowledgeBase>>,
    parsers: std::sync::Arc<
        std::sync::RwLock<HashMap<String, std::sync::Arc<dyn ObjectParser + Send + Sync>>>,
    >,
}

impl IntrospectiveGadget {
    /// Create a new Introspective Gadget
    pub async fn new(
        knowledge_base: std::sync::Arc<tokio::sync::RwLock<KnowledgeBase>>,
    ) -> Result<Self> {
        let mut parsers: HashMap<String, std::sync::Arc<dyn ObjectParser + Send + Sync>> =
            HashMap::new();

        // Register all built-in parsers
        parsers.insert("json".to_string(), std::sync::Arc::new(JsonParser));
        parsers.insert("xml".to_string(), std::sync::Arc::new(XmlParser));
        parsers.insert("yaml".to_string(), std::sync::Arc::new(YamlParser));
        parsers.insert("docker".to_string(), std::sync::Arc::new(DockerParser));
        parsers.insert("binary".to_string(), std::sync::Arc::new(BinaryParser));
        parsers.insert("text".to_string(), std::sync::Arc::new(TextParser));
        parsers.insert("auto".to_string(), std::sync::Arc::new(AutoParser));

        Ok(Self {
            knowledge_base,
            parsers: std::sync::Arc::new(std::sync::RwLock::new(parsers)),
        })
    }

    /// Inspect any object and add to knowledge base
    ///
    /// This is the main "Go-Go-Gadget" method that can handle anything!
    pub async fn inspect_object(&self, input: InspectionInput) -> Result<InspectionResult> {
        let start_time = std::time::Instant::now();

        // Attempt to determine the format
        let detected_format = self.detect_format(&input).await?;

        // Try multiple parsing strategies
        let mut results = Vec::new();
        let mut errors = Vec::new();

        // Try the detected format first
        let parser_opt = self.parsers.read().unwrap().get(&detected_format).cloned();
        if let Some(parser) = parser_opt {
            match parser.parse(&input).await {
                Ok(result) => results.push(result),
                Err(e) => errors.push(format!("{} parser failed: {}", detected_format, e)),
            }
        }

        // If that didn't work, try auto-detection
        if results.is_empty() {
            let auto_parser_opt = self.parsers.read().unwrap().get("auto").cloned();
            if let Some(auto_parser) = auto_parser_opt {
                match auto_parser.parse(&input).await {
                    Ok(result) => results.push(result),
                    Err(e) => errors.push(format!("Auto parser failed: {}", e)),
                }
            }
        }

        // Try all parsers if still no results
        if results.is_empty() {
            let all_parsers: Vec<(String, std::sync::Arc<dyn ObjectParser + Send + Sync>)> = self
                .parsers
                .read()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();

            for (format_name, parser) in all_parsers {
                if format_name != detected_format && format_name != "auto" {
                    if let Ok(result) = parser.parse(&input).await {
                        results.push(result);
                    }
                }
            }
        }

        if results.is_empty() {
            return Err(anyhow::anyhow!(
                "Could not parse object with any available parser. Errors: {:?}",
                errors
            ));
        }

        // Use the best result (most complete schema)
        let best_result = results
            .into_iter()
            .max_by_key(|r| r.schema.complexity_score())
            .unwrap();

        // Generate knowledge base entry
        let kb_entry = self
            .generate_knowledge_base_entry(&best_result, &input)
            .await?;

        // Add to knowledge base, merging with any prior entry of the same
        // name instead of overwriting it. A single sample cannot tell which
        // fields are truly required vs. just present in that one example;
        // merging narrows `required` to the intersection across every
        // sample seen so far, and unions `properties` so fields that only
        // appear in some samples are still captured (as optional).
        let merged_entry = {
            let mut kb = self.knowledge_base.write().await;
            let merged = match kb.schemas.get(&kb_entry.name) {
                Some(existing) => merge_schema_definitions(existing, &kb_entry),
                None => kb_entry.clone(),
            };
            kb.schemas.insert(merged.name.clone(), merged.clone());
            merged
        };

        let inspection_time = start_time.elapsed().as_millis();

        Ok(InspectionResult {
            input_info: input,
            detected_format,
            parsed_data: best_result.data,
            schema: best_result.schema,
            knowledge_base_entry: merged_entry.name,
            inspection_time_ms: inspection_time,
            parsing_errors: errors,
        })
    }

    /// Inspect a Docker container (specialized method)
    pub async fn inspect_docker_container(
        &self,
        container_name: &str,
    ) -> Result<ContainerInspectionWithKnowledge> {
        // Get container info
        let inspect_output = tokio::process::Command::new("docker")
            .args(["inspect", container_name])
            .output()
            .await
            .context("Failed to run docker inspect")?;

        let mut inspect_json = String::from_utf8_lossy(&inspect_output.stdout).to_string();

        // Parse the JSON
        let container_data: Value = unsafe { simd_json::from_str(&mut inspect_json) }
            .context("Failed to parse docker inspect JSON")?;

        // Extract key information
        let config = container_data[0]["Config"].clone();
        let network_settings = container_data[0]["NetworkSettings"].clone();
        let mounts = container_data[0]["Mounts"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|m| ContainerMount {
                source: m["Source"].as_str().unwrap_or("").to_string(),
                destination: m["Destination"].as_str().unwrap_or("").to_string(),
                mode: m["Mode"].as_str().unwrap_or("").to_string(),
                rw: m["RW"].as_bool().unwrap_or(false),
            })
            .collect();

        // Get running processes
        let top_output = tokio::process::Command::new("docker")
            .args(["top", container_name])
            .output()
            .await;

        let processes = if let Ok(output) = top_output {
            let top_text = String::from_utf8_lossy(&output.stdout);
            self.parse_docker_top(&top_text)
        } else {
            vec![]
        };

        let inspection = ContainerInspection {
            name: container_name.to_string(),
            id: container_data[0]["Id"].as_str().unwrap_or("").to_string(),
            image: container_data[0]["Config"]["Image"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            status: container_data[0]["State"]["Status"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            config: config.clone(),
            network_settings: network_settings.clone(),
            mounts,
            processes,
            ports: self.extract_container_ports(&network_settings),
            environment: self.extract_container_env(&config),
            labels: self.extract_container_labels(&config),
        };

        // Add to knowledge base
        let input = InspectionInput {
            source: InspectionSource::DockerContainer(container_name.to_string()),
            data: Some(inspect_json.to_string()),
            metadata: HashMap::new(),
        };

        let result = self.inspect_object(input).await?;
        let kb_entry_name = result.knowledge_base_entry;

        Ok(ContainerInspectionWithKnowledge {
            inspection,
            knowledge_base_entry: kb_entry_name,
        })
    }

    /// Inspect random XML data (as mentioned)
    pub async fn inspect_xml_data(
        &self,
        xml_data: &str,
        source_description: &str,
    ) -> Result<XmlInspection> {
        let input = InspectionInput {
            source: InspectionSource::RawData {
                format_hint: Some("xml".to_string()),
                description: source_description.to_string(),
            },
            data: Some(xml_data.to_string()),
            metadata: HashMap::new(),
        };

        let result = self.inspect_object(input).await?;

        // Try to understand the XML structure
        let root_element = self.extract_xml_root(xml_data);
        let namespaces = self.extract_xml_namespaces(xml_data);
        let elements = self.analyze_xml_elements(xml_data);

        Ok(XmlInspection {
            source_description: source_description.to_string(),
            root_element,
            namespaces,
            elements,
            schema_generated: result.schema,
            knowledge_base_entry: result.knowledge_base_entry,
        })
    }

    /// Inspect legacy/binary data (like Apple Lisa disks)
    pub async fn inspect_legacy_data(
        &self,
        data: &[u8],
        description: &str,
    ) -> Result<LegacyInspection> {
        let input = InspectionInput {
            source: InspectionSource::RawData {
                format_hint: Some("binary".to_string()),
                description: description.to_string(),
            },
            data: Some(String::from_utf8_lossy(data).to_string()),
            metadata: HashMap::from([
                ("original_size".to_string(), data.len().to_string()),
                (
                    "entropy".to_string(),
                    self.calculate_entropy(data).to_string(),
                ),
            ]),
        };

        let result = self.inspect_object(input).await?;

        // Analyze binary structure
        let file_header = if data.len() >= 16 {
            Some(data[0..16].to_vec())
        } else {
            None
        };

        let strings_found = self.extract_strings_from_binary(data);
        let patterns = self.analyze_binary_patterns(data);

        Ok(LegacyInspection {
            description: description.to_string(),
            file_size: data.len(),
            file_header,
            strings_found,
            patterns,
            entropy: self.calculate_entropy(data),
            schema_generated: result.schema,
            knowledge_base_entry: result.knowledge_base_entry,
        })
    }

    // ============================================================================
    // HELPER METHODS
    // ============================================================================

    async fn detect_format(&self, input: &InspectionInput) -> Result<String> {
        match &input.source {
            InspectionSource::File(path) => {
                if let Some(ext) = Path::new(path).extension() {
                    match ext.to_str().unwrap_or("") {
                        "json" => Ok("json".to_string()),
                        "xml" => Ok("xml".to_string()),
                        "yaml" | "yml" => Ok("yaml".to_string()),
                        _ => Ok("auto".to_string()),
                    }
                } else {
                    Ok("auto".to_string())
                }
            }
            InspectionSource::DockerContainer(_) => Ok("docker".to_string()),
            InspectionSource::RawData { format_hint, .. } => {
                Ok(format_hint.clone().unwrap_or_else(|| "auto".to_string()))
            }
            _ => Ok("auto".to_string()),
        }
    }

    async fn generate_knowledge_base_entry(
        &self,
        result: &ParsedObject,
        input: &InspectionInput,
    ) -> Result<SchemaDefinition> {
        let name = match &input.source {
            InspectionSource::File(path) => format!(
                "file_{}",
                Path::new(path)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
            ),
            InspectionSource::DockerContainer(name) => format!("docker_container_{}", name),
            InspectionSource::RawData { description, .. } => {
                format!("raw_data_{}", description.replace(" ", "_"))
            }
            InspectionSource::Url(url) => {
                format!("url_{}", url.replace("/", "_").replace(":", "_"))
            }
        };

        let source_type = match &input.source {
            InspectionSource::File(_) => "file".to_string(),
            InspectionSource::DockerContainer(_) => "docker".to_string(),
            InspectionSource::RawData { .. } => "raw_data".to_string(),
            InspectionSource::Url(_) => "url".to_string(),
        };

        Ok(SchemaDefinition {
            name: name.clone(),
            object_type: result.schema.schema_type.clone(),
            source_type,
            source_data: Some(simd_json::to_string(&result.data)?),
            schema: result.schema.to_value(),
            generated_schemas: HashMap::new(),
            validation_rules: result.schema.generate_validation_rules(),
            examples: vec![result.data.clone()],
            metadata: HashMap::new(),
        })
    }

    fn extract_xml_root(&self, xml: &str) -> Option<String> {
        let re = Regex::new(r#"<\s*([^\s>]+)"#).ok()?;
        re.captures(xml)?.get(1).map(|m| m.as_str().to_string())
    }

    fn extract_xml_namespaces(&self, xml: &str) -> HashMap<String, String> {
        let mut namespaces = HashMap::new();
        let re = Regex::new(r#"xmlns(?::([^\s=]+))?\s*=\s*["']([^"']+)["']"#).unwrap();

        for cap in re.captures_iter(xml) {
            let prefix = cap.get(1).map(|m| m.as_str()).unwrap_or("default");
            let uri = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            namespaces.insert(prefix.to_string(), uri.to_string());
        }

        namespaces
    }

    fn analyze_xml_elements(&self, xml: &str) -> Vec<XmlElementInfo> {
        // Element regex was already compiled once here (fine); the bug was
        // one level down, in parse_xml_attributes, which used to compile a
        // FRESH regex on every single call -- i.e. once per matched element.
        // On a large real file (16MB repomix dump, tens of thousands of
        // `<...>`-shaped matches once you count false positives from
        // embedded source code -- Rust generics, comparisons, etc.) that
        // was thousands of redundant regex compilations, taking minutes.
        // Confirmed by profiling: killed after 2:46 wall-clock still
        // running on the same input that completes in ~1s once fixed.
        static ELEMENT_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
        let re = ELEMENT_RE.get_or_init(|| Regex::new(r#"<([^\s>/]+)([^>]*)>"#).unwrap());

        let mut elements = Vec::new();
        for cap in re.captures_iter(xml) {
            let name = cap
                .get(1)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            let attrs = cap.get(2).map(|m| m.as_str()).unwrap_or("");

            let attributes = self.parse_xml_attributes(attrs);
            elements.push(XmlElementInfo { name, attributes });
        }

        elements
    }

    fn parse_xml_attributes(&self, attrs: &str) -> HashMap<String, String> {
        static ATTR_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
        let re = ATTR_RE.get_or_init(|| Regex::new(r#"(\w+)\s*=\s*["']([^"']*)["']"#).unwrap());

        let mut attributes = HashMap::new();
        for cap in re.captures_iter(attrs) {
            if let (Some(key), Some(value)) = (cap.get(1), cap.get(2)) {
                attributes.insert(key.as_str().to_string(), value.as_str().to_string());
            }
        }

        attributes
    }

    fn calculate_entropy(&self, data: &[u8]) -> f64 {
        let mut counts = [0u64; 256];
        for &byte in data {
            counts[byte as usize] += 1;
        }

        let len = data.len() as f64;
        let mut entropy = 0.0;

        for &count in &counts {
            if count > 0 {
                let p = count as f64 / len;
                entropy -= p * p.log2();
            }
        }

        entropy
    }

    fn extract_strings_from_binary(&self, data: &[u8]) -> Vec<String> {
        let mut strings = Vec::new();
        let mut current_string = Vec::new();

        for &byte in data {
            if byte.is_ascii_alphanumeric() || byte.is_ascii_punctuation() || byte == b' ' {
                current_string.push(byte);
            } else {
                if current_string.len() >= 4 {
                    if let Ok(s) = String::from_utf8(current_string.clone()) {
                        strings.push(s);
                    }
                }
                current_string.clear();
            }
        }

        strings
    }

    fn analyze_binary_patterns(&self, data: &[u8]) -> Vec<BinaryPattern> {
        // Previously O(n^2)-ish: for every starting offset, a fresh linear
        // scan of the whole buffer looking for that window's repeats -- on a
        // full multi-KB+ blob (not just a truncated demo prefix) this took
        // minutes. Single linear pass with a frequency map instead: O(n)
        // hashing every 8-byte window once, only quadratic in `data.len()`
        // going away entirely.
        let mut counts: HashMap<&[u8], (usize, usize)> = HashMap::new(); // window -> (count, first_offset)

        if data.len() >= 8 {
            for i in 0..=data.len() - 8 {
                let window = &data[i..i + 8];
                let entry = counts.entry(window).or_insert((0, i));
                entry.0 += 1;
            }
        }

        let mut patterns: Vec<BinaryPattern> = counts
            .into_iter()
            .filter(|(_, (count, _))| *count > 1)
            .map(|(window, (count, offset))| BinaryPattern {
                pattern: window.to_vec(),
                count,
                offset,
            })
            .collect();

        patterns.sort_by_key(|a| std::cmp::Reverse(a.count));
        patterns.truncate(10); // Top 10 patterns

        patterns
    }

    fn parse_docker_top(&self, top_output: &str) -> Vec<ContainerProcess> {
        let mut processes = Vec::new();
        let lines: Vec<&str> = top_output.lines().collect();

        if lines.len() < 2 {
            return processes;
        }

        for line in &lines[1..] {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 8 {
                processes.push(ContainerProcess {
                    user: parts[0].to_string(),
                    pid: parts[1].parse().unwrap_or(0),
                    ppid: parts[2].parse().unwrap_or(0),
                    cpu: parts[3].to_string(),
                    memory: parts[4].to_string(),
                    vsz: parts[5].parse().unwrap_or(0),
                    rss: parts[6].parse().unwrap_or(0),
                    tty: parts[7].to_string(),
                    stat: parts.get(8).map_or("", |v| v).to_string(),
                    start: parts.get(9).map_or("", |v| v).to_string(),
                    time: parts.get(10).map_or("", |v| v).to_string(),
                    command: parts[11..].join(" "),
                });
            }
        }

        processes
    }

    fn extract_container_ports(&self, network_settings: &Value) -> HashMap<String, Vec<String>> {
        let mut ports = HashMap::new();

        if let Some(ports_obj) = network_settings["Ports"].as_object() {
            for (container_port, host_bindings) in ports_obj {
                if let Some(bindings) = host_bindings.as_array() {
                    let hosts = bindings
                        .iter()
                        .filter_map(|b| {
                            if let (Some(host_ip), Some(host_port)) =
                                (b["HostIp"].as_str(), b["HostPort"].as_str())
                            {
                                Some(format!("{}:{}", host_ip, host_port))
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>();

                    if !hosts.is_empty() {
                        ports.insert(container_port.clone(), hosts);
                    }
                }
            }
        }

        ports
    }

    fn extract_container_env(&self, config: &Value) -> HashMap<String, String> {
        let mut env = HashMap::new();

        if let Some(env_array) = config["Env"].as_array() {
            for env_var in env_array {
                if let Some(env_str) = env_var.as_str() {
                    if let Some(eq_pos) = env_str.find('=') {
                        let key = &env_str[..eq_pos];
                        let value = &env_str[eq_pos + 1..];
                        env.insert(key.to_string(), value.to_string());
                    }
                }
            }
        }

        env
    }

    fn extract_container_labels(&self, config: &Value) -> HashMap<String, String> {
        let mut labels = HashMap::new();

        if let Some(labels_obj) = config["Labels"].as_object() {
            for (key, value) in labels_obj {
                if let Some(val_str) = value.as_str() {
                    labels.insert(key.clone(), val_str.to_string());
                }
            }
        }

        labels
    }
}

// ============================================================================
// DATA STRUCTURES
// ============================================================================

/// Input for inspection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectionInput {
    pub source: InspectionSource,
    pub data: Option<String>,
    pub metadata: HashMap<String, String>,
}

/// Source of the data to inspect
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InspectionSource {
    File(String),
    Url(String),
    DockerContainer(String),
    RawData {
        format_hint: Option<String>,
        description: String,
    },
}

/// Result of an inspection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectionResult {
    pub input_info: InspectionInput,
    pub detected_format: String,
    pub parsed_data: Value,
    pub schema: ObjectSchema,
    pub knowledge_base_entry: String,
    pub inspection_time_ms: u128,
    pub parsing_errors: Vec<String>,
}

/// Parsed object result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedObject {
    pub data: Value,
    pub schema: ObjectSchema,
}

/// Object schema extracted from inspection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectSchema {
    pub schema_type: String,
    pub properties: HashMap<String, SchemaProperty>,
    pub required: Vec<String>,
    pub array_items: Option<Box<ObjectSchema>>,
    pub object_patterns: Vec<String>,
}

impl ObjectSchema {
    fn complexity_score(&self) -> usize {
        self.properties.len() * 10 + self.required.len() * 5 + self.object_patterns.len() * 3
    }

    fn to_value(&self) -> Value {
        json!({
            "type": self.schema_type,
            "properties": self.properties.iter().map(|(k, v)| (k.clone(), v.to_value())).collect::<HashMap<_, _>>(),
            "required": self.required,
            "array_items": self.array_items.as_ref().map(|s| s.to_value()),
            "object_patterns": self.object_patterns
        })
    }

    fn generate_validation_rules(&self) -> Vec<String> {
        let mut rules = Vec::new();

        for (prop_name, prop) in &self.properties {
            match prop.data_type.as_str() {
                "string" if prop.pattern.is_some() => {
                    rules.push(format!("{}_format", prop_name));
                }
                "string" => {}
                "number" => {
                    if let Some(min) = prop.minimum {
                        rules.push(format!("{}_min_{}", prop_name, min));
                    }
                    if let Some(max) = prop.maximum {
                        rules.push(format!("{}_max_{}", prop_name, max));
                    }
                }
                _ => {}
            }
        }

        rules
    }
}

/// Schema property
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchemaProperty {
    pub data_type: String,
    pub description: Option<String>,
    pub pattern: Option<String>,
    pub minimum: Option<f64>,
    pub maximum: Option<f64>,
    pub enum_values: Option<Vec<Value>>,
    pub nested_schema: Option<Box<ObjectSchema>>,
}

impl SchemaProperty {
    fn to_value(&self) -> Value {
        // Build via as_object_mut().insert(...) rather than index-assignment
        // (`obj["key"] = ...`) -- the latter panics ("index out of bounds")
        // on simd_json::OwnedValue in practice; confirmed by running this
        // path on a real 16MB input (repomix-output.xml) where it crashed
        // every time a SchemaProperty had a `description` set.
        let mut obj = json!({
            "type": self.data_type
        });
        let map = obj
            .as_object_mut()
            .expect("json!({\"type\": ..}) always builds an object");

        if let Some(desc) = &self.description {
            map.insert("description".into(), json!(desc));
        }
        if let Some(pattern) = &self.pattern {
            map.insert("pattern".into(), json!(pattern));
        }
        if let Some(min) = self.minimum {
            map.insert("minimum".into(), json!(min));
        }
        if let Some(max) = self.maximum {
            map.insert("maximum".into(), json!(max));
        }
        if let Some(enum_vals) = &self.enum_values {
            map.insert("enum".into(), json!(enum_vals));
        }
        if let Some(nested) = &self.nested_schema {
            map.insert("properties".into(), nested.to_value());
        }

        obj
    }
}

// ============================================================================
// SPECIALIZED INSPECTION RESULTS
// ============================================================================

/// Docker container inspection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInspectionWithKnowledge {
    pub inspection: ContainerInspection,
    pub knowledge_base_entry: String,
}

/// Docker container inspection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerInspection {
    pub name: String,
    pub id: String,
    pub image: String,
    pub status: String,
    pub config: Value,
    pub network_settings: Value,
    pub mounts: Vec<ContainerMount>,
    pub processes: Vec<ContainerProcess>,
    pub ports: HashMap<String, Vec<String>>,
    pub environment: HashMap<String, String>,
    pub labels: HashMap<String, String>,
}

/// Container mount
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerMount {
    pub source: String,
    pub destination: String,
    pub mode: String,
    pub rw: bool,
}

/// Container process
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerProcess {
    pub user: String,
    pub pid: u32,
    pub ppid: u32,
    pub cpu: String,
    pub memory: String,
    pub vsz: u64,
    pub rss: u64,
    pub tty: String,
    pub stat: String,
    pub start: String,
    pub time: String,
    pub command: String,
}

/// XML inspection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XmlInspection {
    pub source_description: String,
    pub root_element: Option<String>,
    pub namespaces: HashMap<String, String>,
    pub elements: Vec<XmlElementInfo>,
    pub schema_generated: ObjectSchema,
    pub knowledge_base_entry: String,
}

/// XML element information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XmlElementInfo {
    pub name: String,
    pub attributes: HashMap<String, String>,
}

/// Legacy/binary inspection result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyInspection {
    pub description: String,
    pub file_size: usize,
    pub file_header: Option<Vec<u8>>,
    pub strings_found: Vec<String>,
    pub patterns: Vec<BinaryPattern>,
    pub entropy: f64,
    pub schema_generated: ObjectSchema,
    pub knowledge_base_entry: String,
}

/// Binary pattern found in data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinaryPattern {
    pub pattern: Vec<u8>,
    pub count: usize,
    pub offset: usize,
}

// ============================================================================
// PARSERS
// ============================================================================

/// Trait for object parsers
#[async_trait::async_trait]
trait ObjectParser: Send + Sync {
    async fn parse(&self, input: &InspectionInput) -> Result<ParsedObject>;
}

/// JSON parser
struct JsonParser;

#[async_trait::async_trait]
impl ObjectParser for JsonParser {
    async fn parse(&self, input: &InspectionInput) -> Result<ParsedObject> {
        let data = input
            .data
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No data provided for JSON parsing"))?;

        let mut data_mut = data.clone();
        let parsed: Value = unsafe { simd_json::from_str(&mut data_mut)? };
        let schema = self.analyze_json_schema(&parsed);

        Ok(ParsedObject {
            data: parsed,
            schema,
        })
    }
}

impl JsonParser {
    fn analyze_json_schema(&self, value: &Value) -> ObjectSchema {
        if let Some(obj) = value.as_object() {
            let mut properties = HashMap::new();
            let mut required = Vec::new();

            for (key, val) in obj {
                properties.insert(key.clone(), self.analyze_json_value(val));
                required.push(key.clone());
            }

            ObjectSchema {
                schema_type: "object".to_string(),
                properties,
                required,
                array_items: None,
                object_patterns: vec![],
            }
        } else if let Some(arr) = value.as_array() {
            let item_schema = arr
                .first()
                .map(|first| Box::new(self.analyze_json_schema(first)));

            ObjectSchema {
                schema_type: "array".to_string(),
                properties: HashMap::new(),
                required: vec![],
                array_items: item_schema,
                object_patterns: vec![],
            }
        } else {
            ObjectSchema {
                schema_type: self.json_value_type(value),
                properties: HashMap::new(),
                required: vec![],
                array_items: None,
                object_patterns: vec![],
            }
        }
    }

    fn analyze_json_value(&self, value: &Value) -> SchemaProperty {
        SchemaProperty {
            data_type: self.json_value_type(value),
            description: None,
            pattern: None,
            minimum: None,
            maximum: None,
            enum_values: None,
            nested_schema: None,
        }
    }

    fn json_value_type(&self, value: &Value) -> String {
        if value.is_str() {
            "string".to_string()
        } else if value.is_i64() || value.is_u64() || value.is_f64() {
            "number".to_string()
        } else if value.is_bool() {
            "boolean".to_string()
        } else if value.is_object() {
            "object".to_string()
        } else if value.is_array() {
            "array".to_string()
        } else {
            "null".to_string()
        }
    }
}

/// XML parser
struct XmlParser;

#[async_trait::async_trait]
impl ObjectParser for XmlParser {
    async fn parse(&self, input: &InspectionInput) -> Result<ParsedObject> {
        let data = input
            .data
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No data provided for XML parsing"))?;

        // Simple XML parsing - extract structure
        let properties = HashMap::from([(
            "xml_content".to_string(),
            SchemaProperty {
                data_type: "string".to_string(),
                description: Some("Raw XML content".to_string()),
                pattern: Some(r#"^<.*>$"#.to_string()),
                minimum: None,
                maximum: None,
                enum_values: None,
                nested_schema: None,
            },
        )]);

        Ok(ParsedObject {
            data: json!({ "xml": data }),
            schema: ObjectSchema {
                schema_type: "object".to_string(),
                properties,
                required: vec!["xml_content".to_string()],
                array_items: None,
                object_patterns: vec!["xml_structure".to_string()],
            },
        })
    }
}

/// Docker parser
struct DockerParser;

#[async_trait::async_trait]
impl ObjectParser for DockerParser {
    async fn parse(&self, input: &InspectionInput) -> Result<ParsedObject> {
        if let InspectionSource::DockerContainer(name) = &input.source {
            // Run docker inspect
            let output = tokio::process::Command::new("docker")
                .args(["inspect", name])
                .output()
                .await?;

            let mut json_str = String::from_utf8_lossy(&output.stdout).to_string();
            let parsed: Value = unsafe { simd_json::from_str(&mut json_str)? };

            Ok(ParsedObject {
                data: parsed,
                schema: ObjectSchema {
                    schema_type: "object".to_string(),
                    properties: HashMap::new(), // Would analyze Docker schema
                    required: vec![],
                    array_items: None,
                    object_patterns: vec!["docker_container".to_string()],
                },
            })
        } else {
            Err(anyhow::anyhow!(
                "Docker parser requires DockerContainer source"
            ))
        }
    }
}

/// Binary parser for unknown data
struct BinaryParser;

#[async_trait::async_trait]
impl ObjectParser for BinaryParser {
    async fn parse(&self, input: &InspectionInput) -> Result<ParsedObject> {
        let data = input
            .data
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No data provided for binary parsing"))?;

        let bytes = data.as_bytes();

        Ok(ParsedObject {
            data: json!({
                "binary_data": general_purpose::STANDARD.encode(bytes),
                "size": bytes.len(),
                "entropy": calculate_entropy(bytes),
            }),
            schema: ObjectSchema {
                schema_type: "object".to_string(),
                properties: HashMap::new(),
                required: vec![],
                array_items: None,
                object_patterns: vec!["binary_blob".to_string()],
            },
        })
    }
}

/// YAML parser
struct YamlParser;

#[async_trait::async_trait]
impl ObjectParser for YamlParser {
    async fn parse(&self, input: &InspectionInput) -> Result<ParsedObject> {
        let data = input
            .data
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No data provided for YAML parsing"))?;

        let parsed: Value = serde_yaml::from_str(data)?;
        let schema = JsonParser.analyze_json_schema(&parsed); // Reuse JSON analyzer

        Ok(ParsedObject {
            data: parsed,
            schema,
        })
    }
}

/// Text parser for plain text
struct TextParser;

#[async_trait::async_trait]
impl ObjectParser for TextParser {
    async fn parse(&self, input: &InspectionInput) -> Result<ParsedObject> {
        let data = input
            .data
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No data provided for text parsing"))?;

        Ok(ParsedObject {
            data: json!({ "text": data }),
            schema: ObjectSchema {
                schema_type: "object".to_string(),
                properties: HashMap::new(),
                required: vec![],
                array_items: None,
                object_patterns: vec!["plain_text".to_string()],
            },
        })
    }
}

/// Auto-detecting parser
struct AutoParser;

#[async_trait::async_trait]
impl ObjectParser for AutoParser {
    async fn parse(&self, input: &InspectionInput) -> Result<ParsedObject> {
        let _data = input
            .data
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No data provided for auto parsing"))?;

        // Try JSON first
        if let Ok(result) = JsonParser.parse(input).await {
            return Ok(result);
        }

        // Try XML
        if let Ok(result) = XmlParser.parse(input).await {
            return Ok(result);
        }

        // Try YAML
        if let Ok(result) = YamlParser.parse(input).await {
            return Ok(result);
        }

        // Fall back to binary
        BinaryParser.parse(input).await
    }
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

fn calculate_entropy(data: &[u8]) -> f64 {
    let mut counts = [0u64; 256];
    for &byte in data {
        counts[byte as usize] += 1;
    }

    let len = data.len() as f64;
    let mut entropy = 0.0;

    for &count in &counts {
        if count > 0 {
            let p = count as f64 / len;
            entropy -= p * p.log2();
        }
    }

    entropy
}
