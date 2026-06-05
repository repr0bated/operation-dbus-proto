use op_state_store::{Constraint, FieldSchema, FieldType, PluginSchema, ReadOnlyCondition};
use simd_json::{json, OwnedValue as Value};
use std::collections::HashMap;

/// Format a plugin's live state into a `PluginSchema`.
///
/// This is a *formatter*, not a schema authority — nothing here is indexed. The
/// plugin's `query_current_state()` (its `*State` struct) is the single source
/// of truth; field types are inferred from that state, so the schema can never
/// drift from what the plugin actually reports. To add a field, add it to the
/// plugin state — never hand-write a field definition.
pub(crate) fn schema_from_state(
    name: &str,
    category: &str,
    version: &str,
    description: &str,
    state: &Value,
) -> PluginSchema {
    use simd_json::prelude::*;

    let mut builder = PluginSchema::builder(name)
        .version(version)
        .category(category)
        .description(description);

    if let Some(obj) = state.as_object() {
        for (key, value) in obj.iter() {
            builder = builder.field(&key.to_string(), field_from_value(value));
        }
    }

    builder.example(state.clone()).build()
}

/// Infer a `FieldSchema` (type + example) from a live state value.
fn field_from_value(value: &Value) -> FieldSchema {
    FieldSchema {
        field_type: infer_field_type(value),
        required: false,
        description: String::new(),
        default: None,
        example: Some(value.clone()),
        constraints: Vec::new(),
        read_only: false,
        read_only_when: None,
    }
}

/// Infer a `FieldType` from a live state value, recursing into arrays/objects.
fn infer_field_type(value: &Value) -> FieldType {
    use simd_json::prelude::*;

    if value.is_null() {
        FieldType::Any
    } else if value.as_bool().is_some() {
        FieldType::Boolean
    } else if value.as_i64().is_some() || value.as_u64().is_some() {
        FieldType::Integer
    } else if value.as_f64().is_some() {
        FieldType::Float
    } else if value.as_str().is_some() {
        FieldType::String
    } else if let Some(arr) = value.as_array() {
        let inner = arr.first().map(infer_field_type).unwrap_or(FieldType::Any);
        FieldType::Array(Box::new(inner))
    } else if let Some(obj) = value.as_object() {
        let fields = obj
            .iter()
            .map(|(k, v)| (k.to_string(), field_from_value(v)))
            .collect();
        FieldType::Object(fields)
    } else {
        FieldType::Any
    }
}

fn any_field(required: bool, description: &str, default: Option<Value>) -> FieldSchema {
    FieldSchema {
        field_type: FieldType::Any,
        required,
        description: description.to_string(),
        default,
        example: None,
        constraints: Vec::new(),
        read_only: false,
        read_only_when: None,
    }
}

fn simple_schema(
    name: &str,
    description: &str,
    dependencies: &[&str],
    fields: Vec<(&str, FieldSchema)>,
) -> PluginSchema {
    let mut builder = PluginSchema::builder(name)
        .version("1.0.0")
        .description(description);
    for dep in dependencies {
        builder = builder.dependency(dep);
    }
    for (field_name, schema) in fields {
        builder = builder.field(field_name, schema);
    }
    builder.build()
}

pub(crate) fn adc_plugin_schema() -> PluginSchema {
    simple_schema(
        "adc",
        "Application default credentials state",
        &[],
        vec![(
            "configured",
            FieldSchema {
                field_type: FieldType::Boolean,
                required: true,
                description: "Whether ADC is configured".to_string(),
                default: Some(json!(false)),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )],
    )
}

pub(crate) fn agent_config_plugin_schema() -> PluginSchema {
    simple_schema(
        "agent_config",
        "Agent configuration and tool assignments",
        &[],
        vec![(
            "agents",
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Any)),
                required: true,
                description: "List of agent configurations".to_string(),
                default: Some(json!([])),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )],
    )
}

pub(crate) fn endpoint_plugin_schema() -> PluginSchema {
    simple_schema(
        "endpoint",
        "Endpoint configuration",
        &["net"],
        vec![(
            "endpoints",
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: true,
                description: "Declared endpoints".to_string(),
                default: Some(json!([])),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )],
    )
}

pub(crate) fn gcloud_adc_plugin_schema() -> PluginSchema {
    simple_schema(
        "gcloud_adc",
        "Google Cloud ADC state",
        &[],
        vec![
            ("account", any_field(false, "Authenticated account", None)),
            ("project_id", any_field(false, "Project id", None)),
            (
                "authenticated",
                FieldSchema {
                    field_type: FieldType::Boolean,
                    required: true,
                    description: "Authentication status".to_string(),
                    default: Some(json!(false)),
                    example: None,
                    constraints: Vec::new(),
                    read_only: false,
                    read_only_when: None,
                },
            ),
        ],
    )
}

pub(crate) fn hardware_plugin_schema() -> PluginSchema {
    simple_schema(
        "hardware",
        "Hardware inventory snapshot",
        &[],
        vec![
            ("cpu", any_field(true, "CPU info", Some(json!({})))),
            ("memory", any_field(true, "Memory info", Some(json!({})))),
            ("disks", any_field(true, "Disk list", Some(json!([])))),
        ],
    )
}

pub(crate) fn keypair_plugin_schema() -> PluginSchema {
    simple_schema(
        "keypair",
        "Keypair declaration state",
        &[],
        vec![(
            "keypairs",
            any_field(true, "Managed keypairs", Some(json!([]))),
        )],
    )
}

pub(crate) fn ovsdb_bridge_plugin_schema() -> PluginSchema {
    simple_schema(
        "ovsdb_bridge",
        "OVS bridge declarations",
        &["net"],
        vec![(
            "bridges",
            any_field(true, "Bridge declarations", Some(json!([]))),
        )],
    )
}

pub(crate) fn proxmox_plugin_schema() -> PluginSchema {
    simple_schema(
        "proxmox",
        "Proxmox container declarations",
        &["net"],
        vec![(
            "containers",
            any_field(true, "Container declarations", Some(json!([]))),
        )],
    )
}

pub(crate) fn proxy_server_plugin_schema() -> PluginSchema {
    simple_schema(
        "proxy_server",
        "Proxy server runtime config",
        &["net"],
        vec![
            (
                "enabled",
                FieldSchema {
                    field_type: FieldType::Boolean,
                    required: true,
                    description: "Enable proxy".to_string(),
                    default: Some(json!(false)),
                    example: None,
                    constraints: Vec::new(),
                    read_only: false,
                    read_only_when: None,
                },
            ),
            (
                "port",
                FieldSchema {
                    field_type: FieldType::Integer,
                    required: true,
                    description: "Proxy port".to_string(),
                    default: Some(json!(8080)),
                    example: None,
                    constraints: vec![
                        Constraint::Min { value: 1.0 },
                        Constraint::Max { value: 65535.0 },
                    ],
                    read_only: false,
                    read_only_when: None,
                },
            ),
        ],
    )
}

pub(crate) fn service_plugin_schema() -> PluginSchema {
    simple_schema(
        "service",
        "Service definition declarations",
        &["net"],
        vec![("services", any_field(true, "Service map", Some(json!({}))))],
    )
}

pub(crate) fn sess_decl_plugin_schema() -> PluginSchema {
    simple_schema(
        "sess_decl",
        "Session declaration state",
        &["users"],
        vec![(
            "sessions",
            any_field(true, "Session declarations", Some(json!([]))),
        )],
    )
}

pub(crate) fn software_plugin_schema() -> PluginSchema {
    simple_schema(
        "software",
        "Software package inventory",
        &[],
        vec![("packages", any_field(true, "Package list", Some(json!([]))))],
    )
}

pub(crate) fn users_plugin_schema() -> PluginSchema {
    simple_schema(
        "users",
        "User account declarations",
        &[],
        vec![("users", any_field(true, "Users list", Some(json!([]))))],
    )
}

pub(crate) fn web_ui_plugin_schema() -> PluginSchema {
    simple_schema(
        "web_ui",
        "Web UI tunables",
        &["mcp"],
        vec![
            (
                "enabled",
                FieldSchema {
                    field_type: FieldType::Boolean,
                    required: true,
                    description: "Enable UI".to_string(),
                    default: Some(json!(true)),
                    example: None,
                    constraints: Vec::new(),
                    read_only: false,
                    read_only_when: None,
                },
            ),
            (
                "cors_origins",
                any_field(false, "Allowed CORS origins", Some(json!([]))),
            ),
            (
                "compression",
                FieldSchema {
                    field_type: FieldType::Boolean,
                    required: true,
                    description: "Enable compression".to_string(),
                    default: Some(json!(true)),
                    example: None,
                    constraints: Vec::new(),
                    read_only: false,
                    read_only_when: None,
                },
            ),
            (
                "cache_ttl",
                FieldSchema {
                    field_type: FieldType::Integer,
                    required: true,
                    description: "Cache TTL seconds".to_string(),
                    default: Some(json!(86400)),
                    example: None,
                    constraints: vec![Constraint::Min { value: 0.0 }],
                    read_only: false,
                    read_only_when: None,
                },
            ),
            (
                "theme",
                any_field(true, "Theme name", Some(json!("default"))),
            ),
            (
                "feature_flags",
                any_field(false, "Feature flag map", Some(json!({}))),
            ),
        ],
    )
}

pub(crate) fn wireguard_plugin_schema() -> PluginSchema {
    simple_schema(
        "wireguard",
        "WireGuard interface state",
        &["net"],
        vec![(
            "interfaces",
            any_field(true, "WireGuard interfaces", Some(json!([]))),
        )],
    )
}

pub(crate) fn incus_plugin_schema() -> PluginSchema {
    let instance_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Instance name".to_string(),
                default: None,
                example: Some(json!("privacy-user-123")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "status".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "Running".to_string(),
                    "Stopped".to_string(),
                    "Frozen".to_string(),
                ]),
                required: true,
                description: "Instance status".to_string(),
                default: Some(json!("Stopped")),
                example: Some(json!("Running")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "type".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "container".to_string(),
                    "virtual-machine".to_string(),
                ]),
                required: true,
                description: "Instance type".to_string(),
                default: Some(json!("container")),
                example: Some(json!("container")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "image".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Source image reference".to_string(),
                default: None,
                example: Some(json!("images:debian/13")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "storage_pool".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Preferred Incus storage pool for initial creation".to_string(),
                default: None,
                example: Some(json!("registration")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "profiles".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: false,
                description: "Applied Incus profiles".to_string(),
                default: Some(json!(["default"])),
                example: Some(json!(["default"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "config".to_string(),
            FieldSchema {
                field_type: FieldType::Any,
                required: false,
                description: "Instance configuration map".to_string(),
                default: Some(json!({})),
                example: Some(json!({"limits.cpu": "2"})),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "devices".to_string(),
            FieldSchema {
                field_type: FieldType::Any,
                required: false,
                description: "Instance device definitions".to_string(),
                default: Some(json!({})),
                example: Some(json!({
                    "eth0": {
                        "type": "nic",
                        "nictype": "bridged",
                        "parent": "ovsbr0"
                    }
                })),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("incus")
        .version("1.0.0")
        .description("Incus instance management")
        .array_field(
            "instances",
            FieldType::Object(instance_fields),
            true,
            "List of Incus instances",
        )
        .example(json!({
            "instances": [
                {
                    "name": "privacy-user-123",
                    "status": "Running",
                    "type": "container",
                    "image": "images:debian/13",
                    "storage_pool": "registration",
                    "profiles": ["default"],
                    "config": {
                        "limits.cpu": "2"
                    },
                    "devices": {
                        "eth0": {
                            "type": "nic",
                            "nictype": "bridged",
                            "parent": "ovsbr0"
                        }
                    }
                }
            ]
        }))
        .build()
}

pub(crate) fn net_plugin_schema() -> PluginSchema {
    let interface_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Interface name".to_string(),
                default: None,
                example: Some(json!("eth0")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "type".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "ethernet".to_string(),
                    "bridge".to_string(),
                    "veth".to_string(),
                    "vlan".to_string(),
                    "bond".to_string(),
                ]),
                required: true,
                description: "Interface type".to_string(),
                default: Some(json!("ethernet")),
                example: Some(json!("ethernet")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "state".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["up".to_string(), "down".to_string()]),
                required: false,
                description: "Interface state".to_string(),
                default: Some(json!("up")),
                example: Some(json!("up")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "addresses".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: false,
                description: "IP addresses".to_string(),
                default: Some(json!([])),
                example: Some(json!(["192.168.1.100/24"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("net")
        .version("1.0.0")
        .description("Network interface management via rtnetlink")
        .array_field(
            "interfaces",
            FieldType::Object(interface_fields),
            true,
            "List of network interfaces",
        )
        .example(json!({
            "interfaces": [
                {
                    "name": "eth0",
                    "type": "ethernet",
                    "state": "up",
                    "addresses": ["192.168.1.100/24"]
                }
            ]
        }))
        .build()
}

pub(crate) fn rtnetlink_plugin_schema() -> PluginSchema {
    let interface_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Interface name".to_string(),
                default: None,
                example: Some(json!("eth0")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "state".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["up".to_string(), "down".to_string()]),
                required: false,
                description: "Administrative interface state".to_string(),
                default: Some(json!("up")),
                example: Some(json!("up")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "addresses".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: false,
                description: "Interface IP addresses in CIDR form".to_string(),
                default: Some(json!([])),
                example: Some(json!(["10.0.0.2/24"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "mac_address".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Optional MAC address override".to_string(),
                default: None,
                example: Some(json!("02:00:00:00:00:01")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "default_gateway".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Default gateway for this interface".to_string(),
                default: None,
                example: Some(json!("10.0.0.1")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("rtnetlink")
        .version("1.0.0")
        .description("Native kernel rtnetlink interface management")
        .array_field(
            "interfaces",
            FieldType::Object(interface_fields),
            true,
            "Desired rtnetlink-managed interfaces",
        )
        .example(json!({
            "interfaces": [
                {
                    "name": "ovsbr0",
                    "state": "up",
                    "addresses": ["10.10.0.1/24"],
                    "default_gateway": "10.10.0.254"
                }
            ]
        }))
        .build()
}

pub(crate) fn openflow_plugin_schema() -> PluginSchema {
    let bridge_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Bridge name".to_string(),
                default: None,
                example: Some(json!("ovs-br0")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "datapath_id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Datapath ID".to_string(),
                default: None,
                example: Some(json!("0000000000000001")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "protocols".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: false,
                description: "Supported OpenFlow protocols".to_string(),
                default: Some(json!(["OpenFlow13"])),
                example: Some(json!(["OpenFlow10", "OpenFlow13"])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "flows".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object({
                    let mut fields = HashMap::new();
                    fields.insert(
                        "table".to_string(),
                        FieldSchema {
                            field_type: FieldType::Integer,
                            required: true,
                            description: "OpenFlow table number".to_string(),
                            default: Some(json!(0)),
                            example: Some(json!(0)),
                            constraints: vec![
                                Constraint::Min { value: 0.0 },
                                Constraint::Max { value: 254.0 },
                            ],
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "priority".to_string(),
                        FieldSchema {
                            field_type: FieldType::Integer,
                            required: true,
                            description: "Flow priority".to_string(),
                            default: Some(json!(100)),
                            example: Some(json!(22000)),
                            constraints: vec![
                                Constraint::Min { value: 0.0 },
                                Constraint::Max { value: 65535.0 },
                            ],
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "match_fields".to_string(),
                        FieldSchema {
                            field_type: FieldType::Any,
                            required: true,
                            description: "OpenFlow match fields".to_string(),
                            default: None,
                            example: Some(
                                json!({"in_port": "ovsbr0-sock", "nw_src": "10.100.0.2"}),
                            ),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "actions".to_string(),
                        FieldSchema {
                            field_type: FieldType::Array(Box::new(FieldType::Any)),
                            required: true,
                            description: "OpenFlow actions".to_string(),
                            default: None,
                            example: Some(json!([{"type": "output", "port": "priv_wg"}])),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "cookie".to_string(),
                        FieldSchema {
                            field_type: FieldType::Integer,
                            required: false,
                            description: "Flow cookie for idempotent route ownership".to_string(),
                            default: None,
                            example: Some(json!(5787125521171081216u64)),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "idle_timeout".to_string(),
                        FieldSchema {
                            field_type: FieldType::Integer,
                            required: false,
                            description: "Idle timeout in seconds".to_string(),
                            default: Some(json!(0)),
                            example: Some(json!(0)),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "hard_timeout".to_string(),
                        FieldSchema {
                            field_type: FieldType::Integer,
                            required: false,
                            description: "Hard timeout in seconds".to_string(),
                            default: Some(json!(0)),
                            example: Some(json!(0)),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields
                }))),
                required: false,
                description: "Flows managed for this bridge".to_string(),
                default: Some(json!([])),
                example: Some(json!([])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "socket_ports".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::Object({
                    let mut fields = HashMap::new();
                    fields.insert(
                        "name".to_string(),
                        FieldSchema {
                            field_type: FieldType::String,
                            required: true,
                            description: "OVS socket port name".to_string(),
                            default: None,
                            example: Some(json!("ovsbr0-sock")),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "container_name".to_string(),
                        FieldSchema {
                            field_type: FieldType::String,
                            required: false,
                            description: "Optional legacy container name bound to this port"
                                .to_string(),
                            default: None,
                            example: Some(json!("privacy-user-abc")),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "port_type".to_string(),
                        FieldSchema {
                            field_type: FieldType::String,
                            required: true,
                            description: "Socket port role".to_string(),
                            default: Some(json!("SharedIngress")),
                            example: Some(json!("SharedIngress")),
                            constraints: Vec::new(),
                            read_only: false,
                            read_only_when: None,
                        },
                    );
                    fields.insert(
                        "ofport".to_string(),
                        FieldSchema {
                            field_type: FieldType::Integer,
                            required: false,
                            description: "Resolved OpenFlow port number".to_string(),
                            default: None,
                            example: Some(json!(7)),
                            constraints: Vec::new(),
                            read_only: true,
                            read_only_when: None,
                        },
                    );
                    fields
                }))),
                required: false,
                description: "Managed OVS socket ports for the bridge".to_string(),
                default: Some(json!([])),
                example: Some(json!([])),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("openflow")
        .version("1.0.0")
        .description("OpenFlow flow table management")
        .dependency("net")
        .dependency("privacy_routes")
        .array_field(
            "bridges",
            FieldType::Object(bridge_fields),
            true,
            "OVS bridges",
        )
        .string_field("controller_endpoint", false, "OpenFlow controller endpoint")
        .boolean_field(
            "auto_discover_containers",
            false,
            "Auto-create flows from discovered legacy container sockets",
        )
        .boolean_field(
            "enable_security_flows",
            false,
            "Inject hardening flows before route flows",
        )
        .integer_field("obfuscation_level", false, "Traffic obfuscation level for generated flows")
        .example(json!({
            "bridges": [
                {
                    "name": "ovsbr0",
                    "protocols": ["OpenFlow13"],
                    "socket_ports": [
                        {
                            "name": "ovsbr0-sock",
                            "port_type": "SharedIngress"
                        }
                    ],
                    "flows": [
                        {
                            "table": 0,
                            "priority": 22000,
                            "match_fields": {"in_port": "ovsbr0-sock", "ip": "", "nw_src": "10.100.0.2"},
                            "actions": [{"type": "output", "port": "priv_wg"}],
                            "cookie": 5787125521171081216u64,
                            "idle_timeout": 0,
                            "hard_timeout": 0
                        }
                    ]
                }
            ],
            "auto_discover_containers": false,
            "enable_security_flows": false,
            "obfuscation_level": 0
        }))
        .build()
}

pub(crate) fn s6_plugin_schema() -> PluginSchema {
    let unit_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Unit name".to_string(),
                default: None,
                example: Some(json!("nginx.service")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "state".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "active".to_string(),
                    "inactive".to_string(),
                    "failed".to_string(),
                ]),
                required: false,
                description: "Desired unit state".to_string(),
                default: Some(json!("active")),
                example: Some(json!("active")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "enabled".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: false,
                description: "Whether unit is enabled at boot".to_string(),
                default: Some(json!(true)),
                example: Some(json!(true)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("s6")
        .version("1.0.0")
        .description("s6 service management")
        .array_field("units", FieldType::Object(unit_fields), true, "s6 services")
        .example(json!({
            "units": [
                {
                    "name": "nginx",
                    "state": "active",
                    "enabled": true
                }
            ]
        }))
        .build()
}

pub(crate) fn privacy_router_plugin_schema() -> PluginSchema {
    let wireguard_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "enabled".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: true,
                description: "Enable WireGuard tunnel".to_string(),
                default: Some(json!(true)),
                example: Some(json!(true)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "container_id".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "Container VMID for WireGuard".to_string(),
                default: Some(json!(100)),
                example: Some(json!(100)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: Some(ReadOnlyCondition {
                    property: "enabled".to_string(),
                    value: "true".to_string(),
                }),
            },
        );
        fields.insert(
            "listen_port".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "WireGuard listen port".to_string(),
                default: Some(json!(51820)),
                example: Some(json!(51820)),
                constraints: vec![
                    Constraint::Min { value: 1.0 },
                    Constraint::Max { value: 65535.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "socket_port".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Host-side bridge port name for the WireGuard ingress container"
                    .to_string(),
                default: Some(json!("priv_wg")),
                example: Some(json!("priv_wg")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let warp_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "enabled".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: true,
                description: "Enable Cloudflare WARP tunnel".to_string(),
                default: Some(json!(true)),
                example: Some(json!(true)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "bridge_interface".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Host WireGuard interface bridged into OVS for WARP egress"
                    .to_string(),
                default: Some(json!("wgcf")),
                example: Some(json!("wgcf")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "wgcf_config".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Path to wgcf WireGuard config used to create the host interface"
                    .to_string(),
                default: Some(json!("/etc/wireguard/wgcf.conf")),
                example: Some(json!("/etc/wireguard/wgcf.conf")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let xray_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "enabled".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: true,
                description: "Enable system XRay client tunnel".to_string(),
                default: Some(json!(true)),
                example: Some(json!(true)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "container_id".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "Container VMID for the local XRay client".to_string(),
                default: Some(json!(101)),
                example: Some(json!(101)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: Some(ReadOnlyCondition {
                    property: "enabled".to_string(),
                    value: "true".to_string(),
                }),
            },
        );
        fields.insert(
            "socket_port".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Host-side bridge port for the local XRay client".to_string(),
                default: Some(json!("priv_xray")),
                example: Some(json!("priv_xray")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "socks_port".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "SOCKS listener port exposed by the local XRay client".to_string(),
                default: Some(json!(1080)),
                example: Some(json!(1080)),
                constraints: vec![
                    Constraint::Min { value: 1.0 },
                    Constraint::Max { value: 65535.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "vps_address".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Remote XRay server hostname or IP".to_string(),
                default: Some(json!("vps.example.com")),
                example: Some(json!("vps.example.com")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "vps_port".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "Remote XRay server port".to_string(),
                default: Some(json!(443)),
                example: Some(json!(443)),
                constraints: vec![
                    Constraint::Min { value: 1.0 },
                    Constraint::Max { value: 65535.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let vps_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "xray_server".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Remote XRay server hostname or IP".to_string(),
                default: Some(json!("vps.example.com")),
                example: Some(json!("vps.example.com")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "xray_port".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: true,
                description: "Remote XRay server port".to_string(),
                default: Some(json!(443)),
                example: Some(json!(443)),
                constraints: vec![
                    Constraint::Min { value: 1.0 },
                    Constraint::Max { value: 65535.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("privacy_router")
        .version("1.1.0")
        .description("System privacy fabric (WireGuard/XRay ingress, WARP bridge, XRay egress)")
        .dependency("incus")
        .dependency("openflow")
        .dependency("privacy_routes")
        .string_field("bridge_name", true, "OVS bridge for privacy network")
        .object_field(
            "wireguard",
            wireguard_fields,
            true,
            "WireGuard tunnel config",
        )
        .object_field("warp", warp_fields, true, "Cloudflare WARP bridge config")
        .object_field(
            "xray",
            xray_fields,
            true,
            "XRay REALITY egress client config",
        )
        .object_field(
            "vps",
            vps_fields,
            true,
            "Remote XRay server endpoint config",
        )
        .example(json!({
            "bridge_name": "ovsbr0",
            "wireguard": {
                "enabled": true,
                "container_id": 100,
                "socket_port": "priv_wg",
                "listen_port": 51820
            },
            "warp": {
                "enabled": true,
                "bridge_interface": "wgcf",
                "wgcf_config": "/etc/wireguard/wgcf.conf"
            },
            "xray": {
                "enabled": true,
                "container_id": 101,
                "socket_port": "priv_xray",
                "socks_port": 1080,
                "vps_address": "vps.example.com",
                "vps_port": 443
            },
            "vps": {
                "xray_server": "vps.example.com",
                "xray_port": 443
            }
        }))
        .build()
}

pub(crate) fn unix_socket_plugin_schema() -> PluginSchema {
    let mut socket_fields = HashMap::new();
    socket_fields.insert(
        "path".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Filesystem path of the unix domain socket".to_string(),
            default: None,
            example: Some(json!("/run/qdrant.sock")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    socket_fields.insert(
        "port".to_string(),
        FieldSchema {
            field_type: FieldType::Integer,
            required: true,
            description: "Local TCP port xray listens on and proxies into this socket".to_string(),
            default: None,
            example: Some(json!(6334)),
            constraints: vec![
                Constraint::Min { value: 1.0 },
                Constraint::Max { value: 65535.0 },
            ],
            read_only: false,
            read_only_when: None,
        },
    );
    socket_fields.insert(
        "protocol".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Transport protocol carried over the socket (grpc, jsonrpc, …)"
                .to_string(),
            default: Some(json!("grpc")),
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );
    socket_fields.insert(
        "label".to_string(),
        FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Human-readable service label used as the xray outbound tag".to_string(),
            default: None,
            example: Some(json!("qdrant-grpc")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        },
    );

    PluginSchema::builder("unix_socket")
        .version("1.0.0")
        .description("Unix domain socket endpoints proxied into xray outbounds")
        .array_field(
            "sockets",
            FieldType::Object(socket_fields),
            true,
            "Declared unix socket endpoints",
        )
        .example(json!({
            "sockets": [
                {
                    "path": "/run/qdrant.sock",
                    "port": 6334,
                    "protocol": "grpc",
                    "label": "qdrant-grpc"
                }
            ]
        }))
        .build()
}

pub(crate) fn privacy_routes_plugin_schema() -> PluginSchema {
    let route_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Stable route object identifier".to_string(),
                default: None,
                example: Some(json!(
                    "4f5e7f1a2d3c4b5a6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5"
                )),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "route_id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Derived route ID from WireGuard public key and shared secret"
                    .to_string(),
                default: None,
                example: Some(json!(
                    "4f5e7f1a2d3c4b5a6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5"
                )),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "user_id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Internal privacy user identifier".to_string(),
                default: None,
                example: Some(json!("550e8400-e29b-41d4-a716-446655440000")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "email".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "User email for audit and publication context".to_string(),
                default: None,
                example: Some(json!("user@example.com")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "wireguard_public_key".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "WireGuard public key backing this route identity".to_string(),
                default: None,
                example: Some(json!("P8c9Kjnv4B3r6C4+J4Q6VQ2sY4bXn4XWz0P2r5s6t7U=")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "assigned_ip".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Assigned WireGuard tunnel address".to_string(),
                default: None,
                example: Some(json!("10.100.0.2/32")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "selector_ip".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Packet-visible selector used for OpenFlow matching".to_string(),
                default: None,
                example: Some(json!("10.100.0.2")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "container_name".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Associated Incus instance name".to_string(),
                default: None,
                example: Some(json!("privacy-user-550e8400")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "ingress_port".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Shared OVS ingress port for route matching".to_string(),
                default: Some(json!("ovsbr0-sock")),
                example: Some(json!("ovsbr0-sock")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "next_hop".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "First logical next hop for this route".to_string(),
                default: Some(json!("priv_wg")),
                example: Some(json!("priv_wg")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "enabled".to_string(),
            FieldSchema {
                field_type: FieldType::Boolean,
                required: true,
                description: "Whether this route should be active".to_string(),
                default: Some(json!(true)),
                example: Some(json!(true)),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "created_at".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Creation timestamp".to_string(),
                default: None,
                example: Some(json!("2026-01-01T00:00:00Z")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "updated_at".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Last update timestamp".to_string(),
                default: None,
                example: Some(json!("2026-01-01T00:05:00Z")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("privacy_routes")
        .version("1.0.0")
        .description("Per-user privacy route objects keyed by WireGuard identity")
        .dependency("wireguard")
        .dependency("privacy_router")
        .array_field(
            "routes",
            FieldType::Object(route_fields),
            true,
            "Published privacy route objects",
        )
        .example(json!({
            "routes": [
                {
                    "name": "4f5e7f1a2d3c4b5a6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5",
                    "route_id": "4f5e7f1a2d3c4b5a6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f8091a2b3c4d5",
                    "user_id": "550e8400-e29b-41d4-a716-446655440000",
                    "email": "user@example.com",
                    "wireguard_public_key": "P8c9Kjnv4B3r6C4+J4Q6VQ2sY4bXn4XWz0P2r5s6t7U=",
                    "assigned_ip": "10.100.0.2/32",
                    "selector_ip": "10.100.0.2",
                    "container_name": "privacy-user-550e8400",
                    "ingress_port": "ovsbr0-sock",
                    "next_hop": "priv_wg",
                    "enabled": true,
                    "created_at": "2026-01-01T00:00:00Z",
                    "updated_at": "2026-01-01T00:00:00Z"
                }
            ]
        }))
        .build()
}

pub(crate) fn mail_server_plugin_schema() -> PluginSchema {
    use op_state_store::FieldType;

    let endpoint_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "smtp_submission".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "SMTP submission endpoint (port 587)".to_string(),
                default: Some(json!("0.0.0.0:587")),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "smtp_tls".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "SMTP TLS endpoint (port 465)".to_string(),
                default: Some(json!("0.0.0.0:465")),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "imap".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "IMAP endpoint (port 143)".to_string(),
                default: Some(json!("0.0.0.0:143")),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "imaps".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "IMAPS endpoint (port 993)".to_string(),
                default: Some(json!("0.0.0.0:993")),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "dovecot_lmtp".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Dovecot LMTP unix socket path inside container".to_string(),
                default: Some(json!("/var/spool/postfix/private/dovecot-lmtp")),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "postfix_pickup".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Postfix pickup unix socket path inside container".to_string(),
                default: Some(json!("/var/spool/postfix/private/pickup")),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("mail_server")
        .version("1.0.0")
        .description("Mail server container state and D-Bus registration for 3tched.com")
        .dependency("incus")
        .dependency("unix_socket")
        .field(
            "container_name",
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Incus container name running the mail stack".to_string(),
                default: Some(json!("mail-3tched")),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "container_status",
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Container runtime status".to_string(),
                default: Some(json!("Unknown")),
                example: None,
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        )
        .field(
            "domain",
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Primary mail domain".to_string(),
                default: Some(json!("3tched.com")),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "xray_socket_path",
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Unix socket path for Xray naive routing integration".to_string(),
                default: Some(json!("/run/xray/mail-naive.sock")),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "dbus_service_name",
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "D-Bus service name registered for this mail instance".to_string(),
                default: Some(json!("org.opdbus.MailServer.3tched")),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "endpoints",
            FieldSchema {
                field_type: FieldType::Object(endpoint_fields),
                required: true,
                description: "Active mail service endpoints".to_string(),
                default: Some(json!({})),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        )
        .field(
            "container_ip",
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Container IPv4 address".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        )
        .field(
            "healthy",
            FieldSchema {
                field_type: FieldType::Boolean,
                required: true,
                description: "Whether the mail stack is healthy".to_string(),
                default: Some(json!(false)),
                example: None,
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        )
        .field(
            "last_error",
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Last error message if unhealthy".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        )
        .example(json!({
            "container_name": "mail-3tched",
            "container_status": "Running",
            "domain": "3tched.com",
            "xray_socket_path": "/run/xray/mail-naive.sock",
            "dbus_service_name": "org.opdbus.MailServer.3tched",
            "endpoints": {
                "smtp_submission": "0.0.0.0:587",
                "smtp_tls": "0.0.0.0:465",
                "imap": "0.0.0.0:143",
                "imaps": "0.0.0.0:993",
                "dovecot_lmtp": "/var/spool/postfix/private/dovecot-lmtp",
                "postfix_pickup": "/var/spool/postfix/private/pickup"
            },
            "container_ip": "10.200.0.2",
            "healthy": true,
            "last_error": null
        }))
        .build()
}

pub(crate) fn cognitive_mcp_plugin_schema() -> PluginSchema {
    let citation_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "text".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Cited text passage".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "source".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Source document identifier".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "page".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Page or location within source".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let source_info_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Unique source identifier".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "title".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Source title".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "source_type".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "url".to_string(),
                    "text".to_string(),
                    "file".to_string(),
                ]),
                required: true,
                description: "Source transport type".to_string(),
                default: None,
                example: Some(json!("url")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "tags".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: false,
                description: "Tags attached to this source".to_string(),
                default: Some(json!([])),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "created_at".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "ISO-8601 creation timestamp".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields
    };

    let gemini_query_request_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "query".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Natural-language query".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "context".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Optional grounding context".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "mode".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec!["query".to_string(), "deep_research".to_string()]),
                required: false,
                description: "Query mode".to_string(),
                default: Some(json!("query")),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "depth".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "Deep-research depth (1-5, default 3)".to_string(),
                default: Some(json!(3)),
                example: None,
                constraints: vec![
                    Constraint::Min { value: 1.0 },
                    Constraint::Max { value: 5.0 },
                ],
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    let memory_tool_input_fields = {
        let mut fields = HashMap::new();
        fields.insert(
            "operation".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "store".to_string(),
                    "retrieve".to_string(),
                    "query".to_string(),
                    "delete".to_string(),
                    "list_namespaces".to_string(),
                    "stats".to_string(),
                ]),
                required: true,
                description: "Memory operation to perform".to_string(),
                default: None,
                example: Some(json!("store")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "namespace".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Namespace name (e.g. project:op-dbus, session:abc)".to_string(),
                default: None,
                example: Some(json!("project:op-dbus")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "namespace_kind".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "project".to_string(),
                    "session".to_string(),
                    "database".to_string(),
                    "workflow".to_string(),
                    "agent".to_string(),
                    "cron".to_string(),
                    "custom".to_string(),
                ]),
                required: false,
                description: "Kind of namespace (used when creating)".to_string(),
                default: None,
                example: Some(json!("project")),
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "key".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Entry key within namespace".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "value".to_string(),
            FieldSchema {
                field_type: FieldType::Any,
                required: false,
                description: "Value to store (any JSON)".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "tags".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: false,
                description: "Tags for the entry".to_string(),
                default: Some(json!([])),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "key_pattern".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Substring pattern for key search (used in query)".to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields.insert(
            "limit".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: false,
                description: "Max results (default 50)".to_string(),
                default: Some(json!(50)),
                example: None,
                constraints: Vec::new(),
                read_only: false,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("cognitive_mcp")
        .version("2.0.0")
        .description("Cognitive MCP server — memory, gRPC CognitiveToolService. THE PLUGIN IS THE SCHEMA: every method, tool, property, and field is declared here. Downstream inherits.")
        .field("http", FieldSchema {
            field_type: FieldType::String, required: false,
            description: "HTTP/SSE bind address for the MCP protocol endpoint".to_string(),
            default: Some(json!("0.0.0.0:3003")), example: Some(json!("100.90.37.254:3003")),
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        .field("grpc", FieldSchema {
            field_type: FieldType::String, required: false,
            description: "gRPC bind address for the CognitiveToolService endpoint".to_string(),
            default: Some(json!("0.0.0.0:50052")), example: Some(json!("100.90.37.254:50052")),
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        .field("db_path", FieldSchema {
            field_type: FieldType::String, required: false,
            description: "CozoDB database path for persistent memory storage".to_string(),
            default: Some(json!("/var/lib/op-dbus/cognitive.db")), example: Some(json!("/var/lib/op-dbus/cognitive.db")),
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        .field("wg_interface", FieldSchema {
            field_type: FieldType::String, required: false,
            description: "WireGuard interface to read identity from".to_string(),
            default: Some(json!("netmaker")), example: Some(json!("netmaker")),
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        .field("http_enabled", FieldSchema {
            field_type: FieldType::Boolean, required: false,
            description: "Enable the HTTP/SSE MCP transport".to_string(),
            default: Some(json!(true)), example: None, constraints: Vec::new(),
            read_only: false, read_only_when: None,
        })
        .field("grpc_enabled", FieldSchema {
            field_type: FieldType::Boolean, required: false,
            description: "Enable the gRPC CognitiveToolService transport".to_string(),
            default: Some(json!(true)), example: None, constraints: Vec::new(),
            read_only: false, read_only_when: None,
        })
        .field("dbus_enabled", FieldSchema {
            field_type: FieldType::Boolean, required: false,
            description: "Register on D-Bus as org.opdbus.CognitiveMcp".to_string(),
            default: Some(json!(true)), example: None, constraints: Vec::new(),
            read_only: false, read_only_when: None,
        })
        .field("running", FieldSchema {
            field_type: FieldType::Boolean, required: false,
            description: "Whether the s6 service is currently running".to_string(),
            default: Some(json!(false)), example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .field("healthy", FieldSchema {
            field_type: FieldType::Boolean, required: false,
            description: "Last known health status from GetHealth".to_string(),
            default: Some(json!(false)), example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .field("auth_status", FieldSchema {
            field_type: FieldType::Enum(vec![
                "none".to_string(), "chrome_profile".to_string(),
                "cookie".to_string(), "api_key".to_string(),
            ]),
            required: false,
            description: "Current authentication method".to_string(),
            default: Some(json!("none")), example: Some(json!("chrome_profile")),
            constraints: Vec::new(), read_only: true, read_only_when: None,
        })
        .field("queries_remaining", FieldSchema {
            field_type: FieldType::Integer, required: false,
            description: "Queries remaining in current quota period".to_string(),
            default: Some(json!(0)), example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .field("queries_limit", FieldSchema {
            field_type: FieldType::Integer, required: false,
            description: "Total queries allowed per quota period".to_string(),
            default: Some(json!(50)), example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .field("notebook_count", FieldSchema {
            field_type: FieldType::Integer, required: false,
            description: "Number of notebooks in the library".to_string(),
            default: Some(json!(0)), example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .field("gemini_query_request", FieldSchema {
            field_type: FieldType::Object(gemini_query_request_fields), required: false,
            description: "R12: Gemini fallback query (requires GEMINI_API_KEY)".to_string(),
            default: None, example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .field("memory_tool", FieldSchema {
            field_type: FieldType::Object(memory_tool_input_fields), required: false,
            description: "MCP MemoryTool: key-value memory store with operations store/retrieve/query/delete/list_namespaces/stats".to_string(),
            default: None, example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .field("citation", FieldSchema {
            field_type: FieldType::Object(citation_fields), required: false,
            description: "Citation sub-object: text, source, page. Inherited by grounded query responses.".to_string(),
            default: None, example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .field("source_info", FieldSchema {
            field_type: FieldType::Object(source_info_fields), required: false,
            description: "SourceInfo sub-object: id, title, source_type, tags, created_at. Inherited by source CRUD responses.".to_string(),
            default: None, example: None, constraints: Vec::new(),
            read_only: true, read_only_when: None,
        })
        .build()
}
pub(crate) fn compact_mcp_plugin_schema() -> PluginSchema {
    PluginSchema::builder("compact_mcp")
        .version("1.0.0")
        .description("op-mcp-server — multi-mode MCP server (compact/full/agents) with stdio, HTTP, and WebSocket transports")
        .field("mode", FieldSchema {
            field_type: FieldType::Enum(vec![
                "compact".into(), "full".into(), "agents".into(),
            ]),
            required: false,
            description: "Server mode: compact (5 meta-tools), full (all tools), agents (D-Bus agents)".into(),
            default: Some(json!("compact")),
            example: Some(json!("compact")),
            constraints: vec![],
            read_only: false,
            read_only_when: None,
        })
        .field("http", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "HTTP/SSE bind address (empty = not started)".into(),
            default: Some(json!("127.0.0.1:11436")),
            example: Some(json!("100.90.37.254:3001")),
            constraints: vec![],
            read_only: false,
            read_only_when: None,
        })
        .field("ws", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "WebSocket bind address (empty = not started)".into(),
            default: Some(json!(null)),
            example: Some(json!("100.90.37.254:3002")),
            constraints: vec![],
            read_only: false,
            read_only_when: None,
        })
        .field("wg_interface", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "WireGuard interface for identity sled".into(),
            default: Some(json!("netmaker")),
            example: Some(json!("netmaker")),
            constraints: vec![],
            read_only: false,
            read_only_when: None,
        })
        .field("stdio", FieldSchema {
            field_type: FieldType::Boolean,
            required: false,
            description: "Run stdio transport (default for Claude Desktop)".into(),
            default: Some(json!(true)),
            example: None,
            constraints: vec![],
            read_only: false,
            read_only_when: None,
        })
        .field("log_level", FieldSchema {
            field_type: FieldType::Enum(vec![
                "trace".into(), "debug".into(), "info".into(), "warn".into(), "error".into(),
            ]),
            required: false,
            description: "Log verbosity".into(),
            default: Some(json!("info")),
            example: None,
            constraints: vec![],
            read_only: false,
            read_only_when: None,
        })
        .field("running", FieldSchema {
            field_type: FieldType::Boolean,
            required: false,
            description: "Whether the s6 service is currently running".into(),
            default: Some(json!(false)),
            example: None,
            constraints: vec![],
            read_only: true,
            read_only_when: None,
        })
        .build()
}

pub(crate) fn zeroclaw_plugin_schema() -> PluginSchema {
    // The plugin IS the schema: derive directly from ZeroclawPlugin's live
    // state. Nothing is indexed here — adding a field to ZeroclawState is the
    // only way to change this schema.
    let state = simd_json::serde::to_owned_value(super::zeroclaw::ZeroclawPlugin::current_state())
        .unwrap_or_else(|_| json!({}));
    schema_from_state(
        "zeroclaw",
        "llm",
        "1.0.0",
        "Zeroclaw schema/RPC-native model router for Antigravity UI, CLI providers, and structured JSON output",
        &state,
    )
}

pub(crate) fn ctl_plane_chatbot_plugin_schema() -> PluginSchema {
    // ── REQ-2: Reasoning Episode Record sub-object ──────────────────────────
    let reasoning_episode_fields = {
        let mut fields = HashMap::new();
        // Core identity
        fields.insert(
            "episode_id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "Unique ID (UUID v7 for time-ordering)".to_string(),
                default: None,
                example: Some(json!("01912abc-def0-7abc-8def-0123456789ab")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "started_at".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "ISO-8601 timestamp of reasoning entry".to_string(),
                default: None,
                example: Some(json!("2025-05-29T14:30:00Z")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "ended_at".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description: "ISO-8601 timestamp of reasoning exit".to_string(),
                default: None,
                example: Some(json!("2025-05-29T14:30:05Z")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "duration_ms".to_string(),
            FieldSchema {
                field_type: FieldType::Integer,
                required: true,
                description: "Wall-clock duration in milliseconds".to_string(),
                default: None,
                example: Some(json!(5000)),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        // Lifecycle
        fields.insert(
            "trigger".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "goal".to_string(),
                    "tool_result".to_string(),
                    "interrupt".to_string(),
                    "replan".to_string(),
                    "system_event".to_string(),
                ]),
                required: true,
                description: "What caused reasoning to start".to_string(),
                default: None,
                example: Some(json!("goal")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "exit_reason".to_string(),
            FieldSchema {
                field_type: FieldType::Enum(vec![
                    "tool_call".to_string(),
                    "response_emitted".to_string(),
                    "direction_change".to_string(),
                    "goal_achieved".to_string(),
                    "config_set".to_string(),
                    "task_scheduled".to_string(),
                    "interrupt".to_string(),
                ]),
                required: true,
                description: "What ended reasoning".to_string(),
                default: None,
                example: Some(json!("tool_call")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        // Content — PII-tagged per REQ-8
        fields.insert(
            "goal_text".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "High-level goal or prompt active at episode start [PII]".to_string(),
                default: None,
                example: Some(json!("Configure VLAN isolation for tenant-3")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "reasoning_summary".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description:
                    "Compact natural-language summary of reasoning — primary embedding input [PII]"
                        .to_string(),
                default: None,
                example: Some(json!(
                    "Evaluated 3 bridge configs, chose br-tenant3 for isolation"
                )),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "tools_consulted".to_string(),
            FieldSchema {
                field_type: FieldType::Array(Box::new(FieldType::String)),
                required: false,
                description: "Ordered list of tools/plugins/MCP calls made during the episode"
                    .to_string(),
                default: Some(json!([])),
                example: Some(json!(["ovs_list_bridges", "ovs_create_bridge"])),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "decision_output".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "The decision, plan, or action the episode produced [PII]".to_string(),
                default: None,
                example: Some(json!("Create br-tenant3 with VLAN 103 tagged ports")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        // Outcome
        fields.insert("outcome_class".to_string(), FieldSchema {
            field_type: FieldType::Enum(vec![
                "goal_achieved".to_string(), "config_set".to_string(),
                "task_scheduled".to_string(), "delegated".to_string(),
                "interrupted".to_string(), "direction_changed".to_string(),
                "inconclusive".to_string(),
            ]),
            required: true,
            description: "Classification of episode outcome. goal_achieved/config_set/task_scheduled => Signal significance".to_string(),
            default: None, example: Some(json!("config_set")),
            constraints: Vec::new(), read_only: true, read_only_when: None,
        });
        fields.insert(
            "confidence".to_string(),
            FieldSchema {
                field_type: FieldType::Float,
                required: false,
                description: "Optional confidence 0.0-1.0 if the model emits one".to_string(),
                default: None,
                example: Some(json!(0.87)),
                constraints: vec![
                    Constraint::Min { value: 0.0 },
                    Constraint::Max { value: 1.0 },
                ],
                read_only: true,
                read_only_when: None,
            },
        );
        // Grouping
        fields.insert(
            "plugin_id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Plugin that owns the context being reasoned about".to_string(),
                default: None,
                example: Some(json!("ovsdb_bridge")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert(
            "conversation_id".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Groups episodes belonging to the same high-level task chain"
                    .to_string(),
                default: None,
                example: Some(json!("vlan-isolation-task-3")),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        // Integrity + Privacy
        fields.insert(
            "content_hash".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: true,
                description:
                    "SHA-256 of canonical serialized record — for exact dedup before upsert (REQ-7)"
                        .to_string(),
                default: None,
                example: None,
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields.insert("pii_flagged".to_string(), FieldSchema {
            field_type: FieldType::Boolean, required: false,
            description: "If true, reasoning_summary and decision_output are redacted before vectorization (REQ-8)".to_string(),
            default: Some(json!(false)), example: None,
            constraints: Vec::new(), read_only: true, read_only_when: None,
        });
        fields
    };

    // ── Significance classification sub-object (REQ-3) ───────────────────────
    let significance_fields = {
        let mut fields = HashMap::new();
        fields.insert("level".to_string(), FieldSchema {
            field_type: FieldType::Enum(vec!["Contextual".to_string(), "Signal".to_string()]),
            required: true,
            description: "Reasoning episodes are always at least Contextual. goal_achieved/config_set/task_scheduled => Signal".to_string(),
            default: Some(json!("Contextual")), example: Some(json!("Signal")),
            constraints: Vec::new(), read_only: true, read_only_when: None,
        });
        fields.insert(
            "rule".to_string(),
            FieldSchema {
                field_type: FieldType::String,
                required: false,
                description: "Significance rule that was evaluated".to_string(),
                default: None,
                example: Some(json!(
                    "outcome_class in [goal_achieved, config_set, task_scheduled]"
                )),
                constraints: Vec::new(),
                read_only: true,
                read_only_when: None,
            },
        );
        fields
    };

    PluginSchema::builder("ctl_plane_chatbot")
        .version("1.0.0")
        .description("Control-plane chatbot reasoning episodes — THE PLUGIN IS THE SCHEMA. Declares every episode field (REQ-2), PII tagging (REQ-8), significance classification (REQ-3), and vectorization pipeline config (REQ-4/5/6/7). Downstream (Qdrant, CozoDB, Accountability UI, EventChainService) inherits.")
        // ── Pipeline config (tunable) ──────────────────────────────────────
        .field("voyage_model", FieldSchema {
            field_type: FieldType::Enum(vec![
                "voyage-4-lite".to_string(), "voyage-4".to_string(),
            ]),
            required: false,
            description: "Voyage embedding model for reasoning episodes (REQ-4). POC target: voyage-4-lite".to_string(),
            default: Some(json!("voyage-4-lite")), example: Some(json!("voyage-4")),
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        .field("qdrant_collection", FieldSchema {
            field_type: FieldType::String, required: false,
            description: "Qdrant collection name (REQ-5). Separate from mutation/schema footprints".to_string(),
            default: Some(json!("ctl_plane_reasoning_episodes")), example: None,
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        .field("vector_dims", FieldSchema {
            field_type: FieldType::Integer, required: false,
            description: "Vector dimensions (1024 for voyage-4-lite, flexible post-POC)".to_string(),
            default: Some(json!(1024)), example: None,
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        .field("dedup_window_hrs", FieldSchema {
            field_type: FieldType::Integer, required: false,
            description: "Content-hash dedup collision window in hours (REQ-7, default 24)".to_string(),
            default: Some(json!(24)), example: None,
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        .field("queue_alert_threshold", FieldSchema {
            field_type: FieldType::Integer, required: false,
            description: "Alert if embedding queue depth exceeds this (REQ-10, default 50)".to_string(),
            default: Some(json!(50)), example: None,
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        .field("nesting_policy", FieldSchema {
            field_type: FieldType::Enum(vec!["flat".to_string(), "nested".to_string()]),
            required: false,
            description: "REQ-1: flat = new trigger extends current episode; nested = opens new episode".to_string(),
            default: Some(json!("flat")), example: None,
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        .field("vectorization_enabled", FieldSchema {
            field_type: FieldType::Boolean, required: false,
            description: "Enable Voyage embedding pipeline for reasoning episodes".to_string(),
            default: Some(json!(true)), example: None,
            constraints: Vec::new(), read_only: false, read_only_when: None,
        })
        // ── Observed state (read-only from pipeline) ───────────────────────
        .field("running", FieldSchema {
            field_type: FieldType::Boolean, required: false,
            description: "Whether the chatbot is currently active".to_string(),
            default: Some(json!(true)), example: None,
            constraints: Vec::new(), read_only: true, read_only_when: None,
        })
        .field("reasoning_active", FieldSchema {
            field_type: FieldType::Boolean, required: false,
            description: "Whether the chatbot is currently in reasoning state (REQ-1)".to_string(),
            default: Some(json!(false)), example: None,
            constraints: Vec::new(), read_only: true, read_only_when: None,
        })
        .field("embedding_queue_depth", FieldSchema {
            field_type: FieldType::Integer, required: false,
            description: "Current Voyage embedding queue depth (alert at queue_alert_threshold)".to_string(),
            default: Some(json!(0)), example: None,
            constraints: Vec::new(), read_only: true, read_only_when: None,
        })
        .field("last_vectorized_at", FieldSchema {
            field_type: FieldType::String, required: false,
            description: "ISO-8601 timestamp of last successful Qdrant upsert".to_string(),
            default: None, example: None,
            constraints: Vec::new(), read_only: true, read_only_when: None,
        })
        // ── Vector ID on sled (identity-bound) ───────────────────────────
        .field("vector_id", FieldSchema {
            field_type: FieldType::String, required: false,
            description: "Qdrant vector UUID on the identity sled — binds every vectorized episode to this identity".to_string(),
            default: None, example: Some(json!("a1b2c3d4-e5f6-7890-abcd-ef0123456789")),
            constraints: Vec::new(), read_only: true, read_only_when: None,
        })
        // ── REQ-2: Reasoning Episode Record ────────────────────────────────
        .field("reasoning_episode", FieldSchema {
            field_type: FieldType::Object(reasoning_episode_fields), required: false,
            description: "REQ-2: Structured record produced at reasoning exit. Primary unit of vectorization.".to_string(),
            default: None, example: None,
            constraints: Vec::new(), read_only: true, read_only_when: None,
        })
        // ── REQ-3: Significance classification ─────────────────────────────
        .field("significance", FieldSchema {
            field_type: FieldType::Object(significance_fields), required: false,
            description: "REQ-3: Always at least Contextual. goal_achieved/config_set/task_scheduled => Signal".to_string(),
            default: None, example: None,
            constraints: Vec::new(), read_only: true, read_only_when: None,
        })
        .build()
}

// ── OSCAL Subid Registry ─────────────────────────────────────────────────────
//
// Every D-Bus object, plugin, schema, mutation, event, and tool carries two
// identifiers: a `uuid` (machine identity) and a `subid` (human-readable
// operational taxonomy key).  This schema defines the canonical shape of one
// registry entry.  Compliance refs live in metadata arrays — never inside
// the subid string itself.

pub(crate) fn oscal_subid_registry_plugin_schema() -> PluginSchema {
    PluginSchema::builder("oscal_subid_registry")
        .version("1.0.0")
        .description("OSCAL subid registry — dual-identifier model for every system artifact. uuid = machine identity, subid = operational taxonomy key.")
        .category("compliance")

        // ── Core identity ─────────────────────────────────────────────────
        .field("uuid", FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Machine identity UUID (RFC 4122). Never replaced by subid.".to_string(),
            default: None,
            example: Some(json!("a1b2c3d4-e5f6-7890-abcd-ef0123456789")),
            constraints: Vec::new(),
            read_only: true,
            read_only_when: None,
        })
        .field("subid", FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Human-readable operational taxonomy key. Format: <category>.<component-type>.<subject>.<verb>[.<facet>][@vN]. Immutable per subject.".to_string(),
            default: None,
            example: Some(json!("mut.service.state-sync.apply-patch@v1")),
            constraints: vec![
                Constraint::Pattern {
                    regex: "^(src|prj|sch|mut|obs|evt|exp)\\.(this-system|system|interconnection|software|hardware|service|policy|physical|process-procedure|plan|guidance|standard|validation|network)\\.[a-z0-9]+(?:-[a-z0-9]+)*\\.[a-z0-9]+(?:-[a-z0-9]+)*(?:\\.[a-z0-9]+(?:-[a-z0-9]+)*){0,2}(?:@v[1-9][0-9]*)?$".to_string()
                },
            ],
            read_only: false,
            read_only_when: None,
        })

        // ── Taxonomy axes ─────────────────────────────────────────────────
        .field("category", FieldSchema {
            field_type: FieldType::Enum(vec![
                "src".to_string(),  // authoritative source / ingress
                "prj".to_string(),  // D-Bus projection / mirror publication
                "sch".to_string(),  // schema, contract, vocabulary
                "mut".to_string(),  // write-path state mutation
                "obs".to_string(),  // read / query / discovery
                "evt".to_string(),  // signal, audit event, proof, tag provenance
                "exp".to_string(),  // consumer-facing render (MCP tool, UI, gRPC view)
            ]),
            required: true,
            description: "Operational category. Determines which additional fields are required.".to_string(),
            default: None,
            example: Some(json!("mut")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("component_type", FieldSchema {
            field_type: FieldType::Enum(vec![
                "software".to_string(),
                "service".to_string(),
                "network".to_string(),
                "hardware".to_string(),
                "process-procedure".to_string(),
                "standard".to_string(),
                "validation".to_string(),
                "policy".to_string(),
                "plan".to_string(),
                "guidance".to_string(),
                "physical".to_string(),
                "this-system".to_string(),
                "system".to_string(),
                "interconnection".to_string(),
            ]),
            required: true,
            description: "OSCAL component-type vocabulary. Reuse OSCAL nouns — do not invent new types.".to_string(),
            default: None,
            example: Some(json!("service")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("subject", FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Stable noun identifying the artifact (e.g. state-sync, plugin-schema, event-chain). Lowercase hyphenated.".to_string(),
            default: None,
            example: Some(json!("state-sync")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("verb", FieldSchema {
            field_type: FieldType::String,
            required: true,
            description: "Action performed on the subject (e.g. apply-patch, resolve, monitor). Lowercase hyphenated.".to_string(),
            default: None,
            example: Some(json!("apply-patch")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("facet", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Optional additional qualifier (up to two segments). Lowercase hyphenated.".to_string(),
            default: None,
            example: Some(json!("rollback")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("version", FieldSchema {
            field_type: FieldType::Integer,
            required: false,
            description: "Schema version of this subid (the @vN suffix). Increment only when subject meaning changes materially.".to_string(),
            default: Some(json!(1)),
            example: Some(json!(1)),
            constraints: vec![Constraint::Min { value: 1.0 }],
            read_only: false,
            read_only_when: None,
        })

        // ── Compliance refs (metadata — never in the subid string) ────────
        .field("control_source", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "URI of the OSCAL catalog or profile that provides the control baseline (e.g. NIST SP 800-53 Rev 5).".to_string(),
            default: None,
            example: Some(json!("https://csrc.nist.gov/projects/oscal")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("control_refs", FieldSchema {
            field_type: FieldType::Array(Box::new(FieldType::String)),
            required: false,
            description: "OSCAL control IDs satisfied by this artifact (e.g. [\"AC-1\", \"CM-3\"]). Compliance detail belongs here, not in the subid string.".to_string(),
            default: Some(json!([])),
            example: Some(json!(["AC-1", "CM-3"])),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("statement_refs", FieldSchema {
            field_type: FieldType::Array(Box::new(FieldType::String)),
            required: false,
            description: "Optional fine-grained OSCAL statement-level references within the controls (e.g. [\"AC-1_smt.a\"]]).".to_string(),
            default: Some(json!([])),
            example: Some(json!(["AC-1_smt.a"])),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })

        // ── Category-specific required fields ─────────────────────────────
        // mut.* — write-path fields
        .field("actor_id", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for mut.* entries. Identity of the actor that performed the mutation.".to_string(),
            default: None,
            example: Some(json!("user:jeremy")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("capability_id", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for mut.* entries. Capability that authorized the mutation.".to_string(),
            default: None,
            example: Some(json!("cap:state-write")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("idempotency_key", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for mut.* entries. Deduplication key for the mutation operation.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })

        // evt.* — event / audit fields
        .field("event_id", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for evt.* entries. Unique identifier for this event in the audit chain.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: true,
            read_only_when: None,
        })
        .field("event_hash", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for evt.* entries. Content hash of the event for chain verification.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: true,
            read_only_when: None,
        })
        .field("tags_touched", FieldSchema {
            field_type: FieldType::Array(Box::new(FieldType::String)),
            required: false,
            description: "Required for evt.* entries. Tags whose immutability is affected by this event.".to_string(),
            default: Some(json!([])),
            example: None,
            constraints: Vec::new(),
            read_only: true,
            read_only_when: None,
        })
        .field("proof_root", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Optional for evt.* entries. Merkle proof root for chain verification.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: true,
            read_only_when: None,
        })

        // src.* — source authority fields
        .field("source_system", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for src.* entries. Name of the authoritative source system (e.g. ovsdb, netmaker).".to_string(),
            default: None,
            example: Some(json!("ovsdb")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("source_locator", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for src.* entries. Socket path, URL, or address of the source.".to_string(),
            default: None,
            example: Some(json!("unix:/var/run/openvswitch/db.sock")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("authority_rank", FieldSchema {
            field_type: FieldType::Integer,
            required: false,
            description: "Optional for src.* entries. Precedence when multiple sources provide the same subject (lower = higher authority).".to_string(),
            default: Some(json!(100)),
            example: Some(json!(1)),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })

        // prj.* — projection fields
        .field("dbus_path", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for prj.* entries. D-Bus object path of the projected artifact.".to_string(),
            default: None,
            example: Some(json!("/opdbus/v1/plugins/wireguard")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("service_name", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for prj.* entries. D-Bus service name hosting the object.".to_string(),
            default: None,
            example: Some(json!("org.opdbus.v1")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("source_subid", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Optional for prj.* entries. Subid of the src.* record this projection was derived from.".to_string(),
            default: None,
            example: Some(json!("src.network.ovsdb.monitor@v1")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })

        // sch.* — schema / contract fields
        .field("schema_id", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for sch.* entries. Canonical name of the schema (matches plugin_schema_defs.rs entry).".to_string(),
            default: None,
            example: Some(json!("wireguard")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("schema_hash", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for sch.* entries. Content hash of the schema at this version.".to_string(),
            default: None,
            example: None,
            constraints: Vec::new(),
            read_only: true,
            read_only_when: None,
        })

        // exp.* — exposure / render fields
        .field("consumer_surface", FieldSchema {
            field_type: FieldType::Enum(vec![
                "mcp-tool".to_string(),
                "dbus-method".to_string(),
                "grpc-method".to_string(),
                "ui-field".to_string(),
                "ui-page".to_string(),
                "api-endpoint".to_string(),
            ]),
            required: false,
            description: "Required for exp.* entries. The consumer-facing surface this artifact is rendered on.".to_string(),
            default: None,
            example: Some(json!("mcp-tool")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })
        .field("tool_name", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for exp.mcp-tool entries. The MCP tool name as registered.".to_string(),
            default: None,
            example: Some(json!("cognitive_memory")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })

        // obs.* — observation / query fields
        .field("query_scope", FieldSchema {
            field_type: FieldType::String,
            required: false,
            description: "Required for obs.* entries. D-Bus path pattern or scope expression for this observation.".to_string(),
            default: None,
            example: Some(json!("/opdbus/v1/plugins/*")),
            constraints: Vec::new(),
            read_only: false,
            read_only_when: None,
        })

        .build()
}

pub(crate) fn factory_plugin_schema() -> PluginSchema {
    let state = simd_json::serde::to_owned_value(super::factory::FactoryPlugin::current_state())
        .unwrap_or_else(|_| json!({}));
    schema_from_state(
        "factory",
        "llm",
        "1.0.0",
        "Factory Droid agent platform — computers, sessions, models, autonomy controls",
        &state,
    )
}

pub(crate) fn fail2ban_plugin_schema() -> PluginSchema {
    let state = simd_json::serde::to_owned_value(super::fail2ban::Fail2banPlugin::current_state())
        .unwrap_or_else(|_| json!({}));
    schema_from_state(
        "fail2ban",
        "security",
        "1.0.0",
        "Fail2ban intrusion prevention — jails, bans, filters, actions",
        &state,
    )
}
pub(crate) fn cron_plugin_schema() -> PluginSchema {
    let state = simd_json::serde::to_owned_value(super::cron::CronPlugin::current_state())
        .unwrap_or_else(|_| json!({}));
    schema_from_state(
        "cron",
        "infrastructure",
        "1.0.0",
        "Cron scheduler — jobs, schedules, execution history",
        &state,
    )
}
pub(crate) fn memory_plugin_schema() -> PluginSchema {
    let state =
        simd_json::serde::to_owned_value(super::memory_plugin::MemoryPlugin::current_state())
            .unwrap_or_else(|_| json!({}));
    schema_from_state(
        "memory",
        "data",
        "1.0.0",
        "Cognitive memory — namespaces, embeddings, search",
        &state,
    )
}
pub(crate) fn workflows_plugin_schema() -> PluginSchema {
    let state =
        simd_json::serde::to_owned_value(super::workflows_plugin::WorkflowsPlugin::current_state())
            .unwrap_or_else(|_| json!({}));
    schema_from_state(
        "workflows",
        "automation",
        "1.0.0",
        "Workflow automation — pipelines, triggers, execution",
        &state,
    )
}
pub(crate) fn btrfs_plugin_schema() -> PluginSchema {
    let state = simd_json::serde::to_owned_value(super::btrfs_plugin::BtrfsPlugin::current_state())
        .unwrap_or_else(|_| json!({}));
    schema_from_state(
        "btrfs",
        "infrastructure",
        "1.0.0",
        "Btrfs filesystem — subvolumes, snapshots, send/receive, DR",
        &state,
    )
}
pub(crate) fn knowledge_plugin_schema() -> PluginSchema {
    let state =
        simd_json::serde::to_owned_value(super::knowledge_plugin::KnowledgePlugin::current_state())
            .unwrap_or_else(|_| json!({}));
    schema_from_state(
        "knowledge",
        "data",
        "1.0.0",
        "Knowledge stores — Qdrant, CozoDB, Sled, embedding pipeline",
        &state,
    )
}
pub(crate) fn antigravity_chat_plugin_schema() -> PluginSchema {
    let state = simd_json::serde::to_owned_value(
        super::antigravity_chat::AntigravityChatPlugin::current_state(),
    )
    .unwrap_or_else(|_| json!({}));
    schema_from_state(
        "antigravity_chat",
        "llm",
        "1.0.0",
        "Antigravity Chat — OAuth bridge, Gemini models, headless IDE",
        &state,
    )
}
pub(crate) fn schema_renderer_plugin_schema() -> PluginSchema {
    let state = simd_json::serde::to_owned_value(
        super::schema_renderer::SchemaRendererPlugin::current_state(),
    )
    .unwrap_or_else(|_| json!({}));
    schema_from_state(
        "schema_renderer",
        "ui",
        "1.0.0",
        "Schema Renderer - dynamic JSON Schema to React form generation with auto-gallery",
        &state,
    )
}
pub(crate) fn antigravity_plugin_schema() -> PluginSchema {
    let state =
        simd_json::serde::to_owned_value(super::antigravity::AntigravityPlugin::current_state())
            .unwrap_or_else(|_| json!({"status":"error"}));
    schema_from_state("antigravity", "llm", "1.0.0", "Google Antigravity SDK provider — Vertex AI Gemini models, OAuth auth, structured output, OSCAL compliance routing", &state)
}
