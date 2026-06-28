//! Embedded plugin schema (single source of truth).
//!
//! The Zeroclaw plugin in `op-plugins` is the **one** schema for LLM providers,
//! model routes, tools, and the selector contract — the plugin IS the schema.
//! op-llm does NOT declare a second, divergent schema. It **includes** the
//! plugin's schema here so the same definition is used in both places: the
//! plugin (authority, D-Bus/gRPC projection) and op-llm (provider adapters).
//!
//! Anything in op-llm that needs the provider/model/tool/route contract should
//! import it from this module, never re-define it.

pub use op_plugins::state_plugins::common::errors::ZeroclawError;
pub use op_plugins::state_plugins::common::llm_projection::{
    ConfigSchema, LlmProjection, LlmTool, LlmTransport, ModelRoute, Provider, Router,
    SelectionEvent, SelectionInput, SelectionOutput, SelectorPolicy, StructuredOutput, UiSurface,
};
pub use op_plugins::state_plugins::common::selector::select_model;
pub use op_plugins::state_plugins::zeroclaw::{zeroclaw_plugin_schema, ZeroclawPlugin, ZeroclawState};

/// The single Zeroclaw `PluginSchema`, included from the plugin. op-llm reads
/// the provider/model/tool contract from here — never a separate copy.
pub fn embedded_plugin_schema() -> op_state_store::PluginSchema {
    zeroclaw_plugin_schema()
}

/// The in-memory typed projection state (declared providers, model routes,
/// tools, selector policy) from the plugin — the schema-backed authority.
pub fn embedded_projection() -> LlmProjection {
    ZeroclawPlugin::current_state().projection
}
