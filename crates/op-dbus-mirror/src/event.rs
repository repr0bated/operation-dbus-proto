//! MirrorEvent module for unified event enum

use serde_json::Value;

/// Unified event enum representing all data source changes
#[derive(Debug, Clone)]
pub enum MirrorEvent {
    /// OVSDB row change event
    OvsdbRow {
        table_name: String,
        uuid: String,
        delta: Value,
        sequence: u64,
    },
    /// NonNetDb key change event
    NonNet {
        key: String,
        delta: Value,
        sequence: u64,
    },
    /// StateManager plugin event
    Plugin {
        plugin_id: String,
        delta: Value,
        sequence: u64,
    },
    /// ComponentRegistry event
    Registry {
        event: Box<op_grpc_bridge::proto::registry::RegistryEvent>,
        sequence: u64,
    },
    /// Procfs memory info event
    ProcMem { delta: Value, sequence: u64 },
    /// Procfs load average event
    ProcLoad { delta: Value, sequence: u64 },
    /// Procfs static section event
    ProcStatic {
        section: String,
        data: Value,
        sequence: u64,
    },
}

impl MirrorEvent {
    /// Get the target path for this event
    pub fn target_path(&self) -> Option<String> {
        match self {
            MirrorEvent::OvsdbRow {
                table_name, uuid, ..
            } => Some(format!("/org/opdbus/v1/ovsdb/{}/{}", table_name, uuid)),
            MirrorEvent::NonNet { key, .. } => Some(format!("/org/opdbus/v1/nonnet/{}", key)),
            MirrorEvent::Plugin { plugin_id, .. } => {
                Some(format!("/org/opdbus/v1/plugin/plugins/{}", plugin_id))
            }
            MirrorEvent::Registry { event, .. } => {
                let component = event.component.as_ref()?;
                let safe = component.component_id.replace(['.', '-', ':'], "_");
                Some(format!("/org/opdbus/v1/registry/{}", safe))
            }
            MirrorEvent::ProcMem { .. } => Some("/org/opdbus/v1/host/meminfo".to_string()),
            MirrorEvent::ProcLoad { .. } => Some("/org/opdbus/v1/host/loadavg".to_string()),
            MirrorEvent::ProcStatic { section, .. } => {
                Some(format!("/org/opdbus/v1/host/{}", section))
            }
        }
    }

    /// Get the sequence number for this event
    pub fn sequence(&self) -> u64 {
        match self {
            MirrorEvent::OvsdbRow { sequence, .. }
            | MirrorEvent::NonNet { sequence, .. }
            | MirrorEvent::Plugin { sequence, .. }
            | MirrorEvent::Registry { sequence, .. }
            | MirrorEvent::ProcMem { sequence, .. }
            | MirrorEvent::ProcLoad { sequence, .. }
            | MirrorEvent::ProcStatic { sequence, .. } => *sequence,
        }
    }
}
