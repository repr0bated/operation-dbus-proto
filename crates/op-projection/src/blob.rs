//! Blob packaging for schema-driven projections (SCH.Blob v1)
//! Implements the "Blob Architecture" discussed in agent conversation history:
//! - Per-plugin (or per-subject) sealed artifacts instead of monolithic live-schema.
//! - Zero-copy friendly serialization (postcard primary; rkyv path ready).
//! - BTRFS subvolume mountable units for complete deployments (e.g. zeroclaw+gemma4 ollama).
//!
//! See docs/BLOB_ARCHITECTURE_SYNTHESIS.md for full gathered rationale + history excerpts.
//! Goal: gemma4 full functional deploy as zeroclaw "blob" that exposes native dbus/grpc surfaces.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use simd_json::OwnedValue as Value;

/// The sealed projection blob - the unit of deployment and reflection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginObjectBlob {
    /// Canonical schema JSON (canonical PluginSchema serialized)
    pub schema_json: String,
    /// Fast hash (for cache/mount validation & dedup)
    pub schema_hash: u64,
    /// D-Bus path for activation (e.g. /org/opdbus/v1/plugins/zeroclaw)
    pub dbus_identity: String,
    /// Serialized FileDescriptorSet bytes for gRPC reflection (from blobify)
    pub grpc_fd_set: Vec<u8>,
    /// Method names surface (for manifest / discovery)
    pub methods: Vec<String>,
    /// Payload (live-state projection or sub-blob)
    pub data: Value,
    /// Optional: model reference, btrfs subvol root, etc for Complete blobs
    pub blob_meta: Option<BlobMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BlobMeta {
    /// e.g. "zeroclaw-gemma4-ollama"
    pub kind: String,
    /// path to btrfs subvol when materialized as deployable unit
    pub btrfs_subvol: Option<PathBuf>,
    /// model tag (gemma4:12b) or store ref
    pub model_ref: Option<String>,
    /// provider (ollama/local via zeroclaw)
    pub provider_ref: Option<String>,
}

/// Simple blobify helper: given the schema + fd + state produce blob.
pub fn blobify_plugin_schema(
    schema_json: &str,
    schema_hash: u64,
    dbus_path: &str,
    fd_set: Vec<u8>,
    methods: Vec<String>,
    projected_data: Value,
) -> Result<PluginObjectBlob> {
    Ok(PluginObjectBlob {
        schema_json: schema_json.to_string(),
        schema_hash,
        dbus_identity: dbus_path.to_string(),
        grpc_fd_set: fd_set,
        methods,
        data: projected_data,
        blob_meta: None,
    })
}

/// Mark this blob as a full gemma4 deployment (the target for user request).
pub fn with_gemma_blob_meta(mut blob: PluginObjectBlob, subvol: Option<PathBuf>) -> PluginObjectBlob {
    blob.blob_meta = Some(BlobMeta {
        kind: "zeroclaw-gemma4-ollama".to_string(),
        btrfs_subvol: subvol,
        model_ref: Some("gemma4:12b".to_string()),
        provider_ref: Some("ollama".to_string()),
    });
    blob
}

/// Example shm path convention (per conversation: hash in name)
pub fn shm_blob_path(blob: &PluginObjectBlob) -> PathBuf {
    PathBuf::from(format!(
        "/dev/shm/opdbus/blobs/{:016x}.blob",
        blob.schema_hash
    ))
}

// Integration hook suggestions:
//  - In op-grpc-bridge or op-projection call blobify_plugin_schema for zeroclaw etc.
//  - Push active PluginObjectBlob refs into an ActiveReflectionCatalog.
//  - Use deploy/btrfs-layout style scripts to "materialize" a gemma blob (subvol + copy the serialized blob + record).

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_smoke() {
        let data = simd_json::json!({ "status": "declared", "selected_model": "gemma4" });
        let b = blobify_plugin_schema(
            r#"{"plugin":"zeroclaw"}"#,
            0xdeadbeef,
            "/org/opdbus/v1/plugins/zeroclaw",
            vec![],
            vec!["chat".into()],
            data,
        ).unwrap();
        let p = shm_blob_path(&b);
        assert!(p.to_string_lossy().contains("deadbeef"));
    }
}
