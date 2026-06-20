//! Plugin schema aggregator — re-exports only.
//!
//! THE PLUGIN IS THE SCHEMA. Every schema is defined in its own plugin file.
//! This module only re-exports them for backward compatibility and
//! cross-plugin discovery.

// ── Re-exports from plugin files ──────────────────────────────────────────────

pub(crate) use super::adc::adc_schema as adc_plugin_schema;
pub(crate) use super::agent_config::agent_config_schema as agent_config_plugin_schema;
pub(crate) use super::antigravity::antigravity_schema as antigravity_plugin_schema;
pub(crate) use super::antigravity_chat::antigravity_chat_schema as antigravity_chat_plugin_schema;
pub(crate) use super::btrfs_plugin::btrfs_schema as btrfs_plugin_schema;
pub(crate) use super::cognitive_mcp::cognitive_mcp_schema as cognitive_mcp_plugin_schema;
pub(crate) use super::compact_mcp::compact_mcp_schema as compact_mcp_plugin_schema;
pub(crate) use super::cron::cron_schema as cron_plugin_schema;
pub(crate) use super::ctl_plane_chatbot::ctl_plane_chatbot_schema as ctl_plane_chatbot_plugin_schema;
pub(crate) use super::endpoint::endpoint_schema as endpoint_plugin_schema;
pub(crate) use super::factory::factory_schema as factory_plugin_schema;
pub(crate) use super::fail2ban::fail2ban_schema as fail2ban_plugin_schema;
pub(crate) use super::gcloud_adc::gcloud_adc_schema as gcloud_adc_plugin_schema;
pub(crate) use super::hardware::hardware_schema as hardware_plugin_schema;
pub(crate) use super::incus::incus_schema as incus_plugin_schema;
pub(crate) use super::keypair::keypair_schema as keypair_plugin_schema;
pub(crate) use super::knowledge_plugin::knowledge_schema as knowledge_plugin_schema;
pub(crate) use super::mail_server::mail_server_schema as mail_server_plugin_schema;
pub(crate) use super::memory_plugin::memory_schema as memory_plugin_schema;
pub(crate) use super::net::net_schema as net_plugin_schema;
pub(crate) use super::netmaker::netmaker_schema as netmaker_plugin_schema;
pub(crate) use super::oci::oci_schema;
pub(crate) use super::openflow::openflow_schema as openflow_plugin_schema;
pub(crate) use super::oscal_subid_registry::oscal_subid_registry_schema;
pub(crate) use super::ovsdb_bridge::ovsdb_bridge_schema as ovsdb_bridge_plugin_schema;
pub(crate) use super::ovsdb_daemon::ovsdb_daemon_schema as ovsdb_daemon_plugin_schema;
pub(crate) use super::privacy_router::privacy_router_schema as privacy_router_plugin_schema;
pub(crate) use super::privacy_routes::privacy_routes_schema as privacy_routes_plugin_schema;
pub(crate) use super::proxmox::proxmox_schema as proxmox_plugin_schema;
pub(crate) use super::proxy_server::proxy_server_schema as proxy_server_plugin_schema;
pub(crate) use super::rovs_commands::rovs_commands_schema as rovs_commands_plugin_schema;
pub(crate) use super::rtnetlink::rtnetlink_schema as rtnetlink_plugin_schema;
pub(crate) use super::s6::s6_schema as s6_plugin_schema;
pub(crate) use super::schema_renderer::schema_renderer_schema as schema_renderer_plugin_schema;
pub(crate) use super::service::service_schema as service_plugin_schema;
pub(crate) use super::sessdecl::sess_decl_schema as sess_decl_plugin_schema;
pub(crate) use super::software::software_schema as software_plugin_schema;
pub(crate) use super::unix_socket::unix_socket_schema_derived as unix_socket_plugin_schema;
pub(crate) use super::users::users_schema as users_plugin_schema;
pub(crate) use super::web_ui::web_ui_schema as web_ui_plugin_schema;
pub(crate) use super::wgcf::wgcf_schema as wgcf_plugin_schema;
pub(crate) use super::wireguard::wireguard_schema as wireguard_plugin_schema;
pub(crate) use super::workflows_plugin::workflows_schema as workflows_plugin_schema;
pub(crate) use super::xray::xray_schema as xray_plugin_schema;
pub(crate) use super::zeroclaw::zeroclaw_schema as zeroclaw_plugin_schema;

// ── Shared utility functions (not schemas) ─────────────────────────────────────

use op_state_store::{FieldSchema, FieldType, PluginSchema};
use simd_json::OwnedValue as Value;

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
pub(crate) fn field_from_value(value: &Value) -> FieldSchema {
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
pub(crate) fn infer_field_type(value: &Value) -> FieldType {
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

pub(crate) fn any_field(required: bool, description: &str, default: Option<Value>) -> FieldSchema {
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

pub(crate) fn simple_schema(
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
