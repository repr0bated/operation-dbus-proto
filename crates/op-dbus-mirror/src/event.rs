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
    /// Get the target path for this event (mirror namespace)
    pub fn target_path(&self) -> Option<String> {
        match self {
            MirrorEvent::OvsdbRow {
                table_name, uuid, ..
            } => Some(format!(
                "/org/opdbus/v1/mirror/ovsdb/{}/{}",
                table_name, uuid
            )),
            MirrorEvent::NonNet { key, .. } => {
                Some(format!("/org/opdbus/v1/mirror/nonnet/{}", key))
            }
            MirrorEvent::ProcMem { .. } => Some("/org/opdbus/v1/mirror/host/meminfo".to_string()),
            MirrorEvent::ProcLoad { .. } => Some("/org/opdbus/v1/mirror/host/loadavg".to_string()),
            MirrorEvent::ProcStatic { section, .. } => {
                Some(format!("/org/opdbus/v1/mirror/host/{}", section))
            }
        }
    }

    /// Get the sequence number for this event
    pub fn sequence(&self) -> u64 {
        match self {
            MirrorEvent::OvsdbRow { sequence, .. }
            | MirrorEvent::NonNet { sequence, .. }
            | MirrorEvent::ProcMem { sequence, .. }
            | MirrorEvent::ProcLoad { sequence, .. }
            | MirrorEvent::ProcStatic { sequence, .. } => *sequence,
        }
    }
}
