//! identity_sled — the container IS the sled IS the identity.
//!
//! Each record is one provisioned container's identity, seeded by its sealed
//! blob: `session_id` (= the container name, derived from the WireGuard
//! pubkey) keys the record; the host itself is "container zero" — the same
//! kind of object, not a special case. This replaces the bespoke pre-schema
//! `#[repr(C)]` sled at `/dev/shm/plugin_schema.dat` as the schema'd,
//! reflectable identity surface; the raw sled file remains a compatibility
//! projection written by the dispatcher until AnnaScribe migrates.
//!
//! The session event ledger ("snowball") methods replace the flat
//! `/dev/shm/snowball_session.log`; every call is already notarized in the
//! immutable event chain by MutationEngine, and rows persist to the Cozo
//! `sessions` / `session_memories` relations via the cognitive-mcp store.

use anyhow::Result;
use async_trait::async_trait;
use op_state::{ApplyResult, Checkpoint, DiffMetadata, PluginCapabilities, StateDiff, StatePlugin};
use op_state_store::PluginSchema;
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;

/// One container's identity sled. The container is the sled is the identity:
/// `session_id` == container name == derived session identity.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.identity-sled.record.schema@v1"))]
pub struct ContainerIdentitySled {
    /// Container identity: session_id = container name, derived from the
    /// WireGuard pubkey. The host is "container zero" under its own id.
    #[schemars(extend("x-oscal-subid" = "src.service.identity-sled.session-id@v1"))]
    pub session_id: String,
    /// WireGuard public key (base64) — the cryptographic root of identity.
    #[schemars(extend("x-oscal-subid" = "src.network.identity-sled.wireguard-pubkey@v1"))]
    pub wireguard_pubkey: String,
    /// WireGuard interface the identity is anchored to.
    #[serde(default)]
    pub interface: String,
    /// Peer IP on the WG mesh, when known (identity anchor for sub-HTTP protocols).
    #[serde(default)]
    pub peer_ip: Option<String>,
    /// Monotonic mutation index (accountability loop position).
    #[serde(default)]
    pub mutation_index: u64,
    /// Hex Blake3 session footprint (pubkey + mutation index + source port).
    #[serde(default)]
    pub hashed_footprint: String,
    /// Hex trace id propagated across the session.
    #[serde(default)]
    pub trace_id: String,
    /// Published schema catalog version the footprint was etched against.
    #[serde(default)]
    pub schema_version: u32,
    /// Semantic vector id for the session, when embedded.
    #[serde(default)]
    pub vector_id: String,
    /// Sealed identity blob backing this sled in the SHM catalog
    /// (`<plugin_id>.<schema_hash16>.blob`). The blob seeds the container.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.service.identity-sled.blob-ref@v1"))]
    pub blob_ref: Option<String>,
    /// The sled's persistence: an immutable sealed btrfs image, registered as
    /// a first-class btrfs device record in Cozo and attached via
    /// `btrfs device add` — no overlay layers, no subvolumes.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.service.identity-sled.btrfs-device@v1"))]
    pub btrfs_device: Option<SledBtrfsDevice>,
    /// The container's full Incus definition (official `shared/api` shape) —
    /// the sled inherits the Incus schema; provisioning the container IS
    /// writing this sled. The instance `name` is always the session_id.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.service.identity-sled.instance@v1"))]
    pub instance: Option<super::incus::IncusInstance>,
    /// Unix seconds when the session began.
    #[serde(default)]
    pub session_started_at: i64,
    /// Unix seconds of the last heartbeat / arrival.
    #[serde(default)]
    pub last_seen_at: i64,
    /// Whether the identity is currently live (container running / host up).
    #[serde(default)]
    pub active: bool,
    /// Unix seconds the *current visit/term* stops being valid. The account
    /// (session_id) can be lifelong (host/user/chatbot) while still having a
    /// bounded term — lifelong accounts get this renewed on every touch
    /// (`touch_identity_sled`); temporary consumer identities (e.g. external
    /// frontends like Lovable) don't, so their term actually lapses.
    /// `None` before a term has ever been issued.
    #[serde(default)]
    #[schemars(extend("x-oscal-subid" = "src.service.identity-sled.expires-at@v1"))]
    pub expires_at: Option<i64>,
}

/// The btrfs device that IS the sled's persistence. The record of truth lives
/// in Cozo (the device registry); the physical attach is `btrfs device add`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.identity-sled.btrfs-device.schema@v1"))]
pub struct SledBtrfsDevice {
    /// Path of the sealed image / block device (e.g. loop-backed image file).
    pub device_path: String,
    /// Mounted btrfs filesystem the device is added TO (`btrfs device add
    /// <device_path> <mount_point>`).
    #[serde(default)]
    pub mount_point: String,
    /// btrfs filesystem UUID of the sealed image.
    #[serde(default)]
    pub btrfs_uuid: String,
    /// Cozo row id of the device record (the registry of truth).
    #[serde(default)]
    pub cozo_id: String,
    /// Whether the device is currently attached (`btrfs device add` done).
    #[serde(default)]
    pub attached: bool,
}

/// One entry in a session's append-only "snowball" ledger.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.identity-sled.session-event.schema@v1"))]
pub struct SessionEvent {
    /// Session the event belongs to.
    pub session_id: String,
    /// Monotonic sequence within the session.
    pub seq: u64,
    /// Event kind (e.g. "arrival", "mutation", "handshake").
    pub kind: String,
    /// OSCAL subid of the acting surface, when applicable.
    #[serde(default)]
    pub subid: String,
    /// JSON payload of the event.
    #[serde(default)]
    pub content: String,
    /// Unix seconds the event was recorded.
    pub created_at: i64,
}

/// Runtime state: every container identity sled, host ("container zero") included.
#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
#[schemars(extend("x-oscal-subid" = "sch.service.plugin.identity-sled.schema@v1"))]
#[schemars(extend("x-oscal-category" = "service"))]
pub struct IdentitySledState {
    /// All known container identity sleds, keyed by `session_id`, sorted.
    #[schemars(extend("x-oscal-subid" = "obs.service.identity-sled.sleds@v1"))]
    pub sleds: Vec<ContainerIdentitySled>,
}

pub struct IdentitySledPlugin;

impl IdentitySledPlugin {
    pub fn new() -> Self {
        Self
    }

    /// This plugin's present state, read from its own sealed blob in the SHM
    /// catalog (the blob IS the plugin).
    fn read_state() -> IdentitySledState {
        let Some(schema) = op_blob::catalog::read_plugin_schema_shm("identity_sled") else {
            return IdentitySledState::default();
        };
        let Some(state) = schema.example else {
            return IdentitySledState::default();
        };
        simd_json::serde::from_owned_value::<IdentitySledState>(state).unwrap_or_default()
    }
}

impl Default for IdentitySledPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl StatePlugin for IdentitySledPlugin {
    fn name(&self) -> &str {
        "identity_sled"
    }

    fn version(&self) -> &str {
        "1.0.0"
    }

    fn schema(&self) -> Option<PluginSchema> {
        let mut schema = super::plugin_scaffold_helpers::identity_sled_plugin_schema();
        super::common::oscal::ensure_category_metadata_fields(&mut schema);
        Some(schema)
    }

    async fn calculate_diff(&self, _current: &Value, _desired: &Value) -> Result<StateDiff> {
        Ok(StateDiff {
            plugin: self.name().to_string(),
            actions: vec![],
            metadata: DiffMetadata {
                timestamp: chrono::Utc::now().timestamp(),
                current_hash: "schema".to_string(),
                desired_hash: "schema".to_string(),
            },
        })
    }

    async fn apply_state(&self, _diff: &StateDiff) -> Result<ApplyResult> {
        // Mutations go through the identity_sled dispatcher (MutationEngine);
        // reconciliation has nothing to converge here.
        Ok(ApplyResult {
            success: true,
            changes_applied: vec![],
            errors: vec![],
            checkpoint: None,
        })
    }

    async fn verify_state(&self, _desired: &Value) -> Result<bool> {
        Ok(true)
    }

    async fn create_checkpoint(&self) -> Result<Checkpoint> {
        Ok(Checkpoint {
            id: uuid::Uuid::new_v4().to_string(),
            plugin: self.name().to_string(),
            timestamp: chrono::Utc::now().timestamp(),
            state_snapshot: simd_json::serde::to_owned_value(Self::read_state())?,
            backend_checkpoint: None,
        })
    }

    async fn rollback(&self, _checkpoint: &Checkpoint) -> Result<()> {
        Ok(())
    }

    fn capabilities(&self) -> PluginCapabilities {
        PluginCapabilities {
            supports_rollback: false,
            supports_checkpoints: true,
            supports_verification: true,
            atomic_operations: false,
        }
    }
}

/// Canonical `identity_sled` schema derived from [`IdentitySledState`] via
/// schemars, plus the callable method surface.
pub(crate) fn identity_sled_schema() -> PluginSchema {
    let root = serde_json::to_value(schemars::schema_for!(IdentitySledState))
        .expect("schemars schema serializes to JSON");
    let mut schema = super::schemars_adapter::plugin_schema_from_json(
        "identity_sled",
        "1.0.0",
        "Container identity sleds: the container is the sled is the identity; host is container zero; each record is seeded by its sealed blob",
        &root,
    );

    // Typed method inputs / outputs.
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetIdentityInput {
        /// Session (container) id to read; empty = the host ("container zero").
        #[serde(default)]
        pub session_id: String,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetIdentityOutput {
        pub identity: ContainerIdentitySled,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct WriteIdentityInput {
        /// WireGuard public key (base64). session_id is DERIVED from this —
        /// never supplied — so an identity cannot be written under a foreign id.
        pub wireguard_pubkey: String,
        #[serde(default)]
        pub interface: String,
        #[serde(default)]
        pub peer_ip: Option<String>,
        /// Sealed identity blob reference seeding this container.
        #[serde(default)]
        pub blob_ref: Option<String>,
        /// btrfs device (Cozo-registered, `btrfs device add`-attached) that is
        /// this sled's persistence.
        #[serde(default)]
        pub btrfs_device: Option<SledBtrfsDevice>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct ProvisionContainerInput {
        /// WireGuard public key (base64). session_id — and therefore the
        /// container name — is DERIVED from this, never supplied.
        pub wireguard_pubkey: String,
        /// Provision-time PSK (base64); when present the session_id derivation
        /// is Argon2(PSK, salt=pubkey).
        #[serde(default)]
        pub psk: Option<String>,
        /// Sealed identity blob reference seeding this container.
        #[serde(default)]
        pub blob_ref: Option<String>,
        /// btrfs persistence device to register for this sled.
        #[serde(default)]
        pub btrfs_device: Option<SledBtrfsDevice>,
        /// Incus instance definition overrides (image, profiles, config,
        /// devices…). `name` is ignored and forced to the derived session_id;
        /// `nic`-type devices are rejected (containers get no NIC or IP).
        #[serde(default)]
        pub instance: Option<super::incus::IncusInstance>,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct AttachBtrfsDeviceInput {
        pub session_id: String,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct TouchSessionInput {
        pub session_id: String,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct RecordSessionEventInput {
        pub session_id: String,
        /// Event kind (e.g. "arrival", "mutation", "handshake").
        pub kind: String,
        #[serde(default)]
        pub subid: String,
        /// JSON payload of the event.
        #[serde(default)]
        pub content: String,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetSessionHistoryInput {
        pub session_id: String,
        /// Maximum entries to return, newest first (0 = all).
        #[serde(default)]
        pub limit: u32,
    }
    #[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
    pub struct GetSessionHistoryOutput {
        pub events: Vec<SessionEvent>,
    }

    use super::plugin_scaffold_helpers::method_decl_from_schemars_with_output;
    use super::plugin_scaffold_helpers::AckOutput;
    use op_state_store::SideEffect;

    schema.methods.insert(
        "get_identity".to_string(),
        method_decl_from_schemars_with_output::<GetIdentityInput, GetIdentityOutput>(
            "get_identity",
            SideEffect::Read,
            true,
            "identity_sled.read",
            "obs.service.identity-sled.identity.get@v1",
        ),
    );
    schema.methods.insert(
        "write_identity".to_string(),
        method_decl_from_schemars_with_output::<WriteIdentityInput, GetIdentityOutput>(
            "write_identity",
            SideEffect::Mutation,
            true,
            "identity_sled.write",
            "mut.service.identity-sled.identity.write@v1",
        ),
    );
    schema.methods.insert(
        "provision_container".to_string(),
        method_decl_from_schemars_with_output::<ProvisionContainerInput, GetIdentityOutput>(
            "provision_container",
            SideEffect::Mutation,
            false,
            "identity_sled.write",
            "mut.service.identity-sled.container.provision@v1",
        ),
    );
    schema.methods.insert(
        "attach_btrfs_device".to_string(),
        method_decl_from_schemars_with_output::<AttachBtrfsDeviceInput, GetIdentityOutput>(
            "attach_btrfs_device",
            SideEffect::Mutation,
            false,
            "identity_sled.write",
            "mut.service.identity-sled.btrfs-device.attach@v1",
        ),
    );
    schema.methods.insert(
        "touch_session".to_string(),
        method_decl_from_schemars_with_output::<TouchSessionInput, AckOutput>(
            "touch_session",
            SideEffect::Mutation,
            true,
            "identity_sled.write",
            "mut.service.identity-sled.session.touch@v1",
        ),
    );
    schema.methods.insert(
        "record_session_event".to_string(),
        method_decl_from_schemars_with_output::<RecordSessionEventInput, AckOutput>(
            "record_session_event",
            SideEffect::Mutation,
            false,
            "identity_sled.write",
            "mut.service.identity-sled.session.event.record@v1",
        ),
    );
    schema.methods.insert(
        "get_session_history".to_string(),
        method_decl_from_schemars_with_output::<GetSessionHistoryInput, GetSessionHistoryOutput>(
            "get_session_history",
            SideEffect::Read,
            true,
            "identity_sled.read",
            "obs.service.identity-sled.session.history.get@v1",
        ),
    );

    schema
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_has_method_surface() {
        let schema = identity_sled_schema();
        for method in [
            "get_identity",
            "write_identity",
            "provision_container",
            "attach_btrfs_device",
            "touch_session",
            "record_session_event",
            "get_session_history",
        ] {
            assert!(schema.methods.contains_key(method), "missing {method}");
        }
    }

    #[test]
    fn all_subids_are_valid() {
        let raw = serde_json::to_value(schemars::schema_for!(IdentitySledState)).unwrap();
        let mut subids = Vec::new();
        collect_subids(&raw, &mut subids);
        assert!(!subids.is_empty());
        for subid in subids {
            assert!(
                crate::state_plugins::common::oscal::validate_subid(&subid).is_ok(),
                "invalid subid: {subid}"
            );
        }
    }

    fn collect_subids(value: &serde_json::Value, out: &mut Vec<String>) {
        if let Some(obj) = value.as_object() {
            if let Some(subid) = obj.get("x-oscal-subid").and_then(|v| v.as_str()) {
                out.push(subid.to_string());
            }
            for v in obj.values() {
                collect_subids(v, out);
            }
        }
        if let Some(arr) = value.as_array() {
            for v in arr {
                collect_subids(v, out);
            }
        }
    }
}

// Self-registration: the plugin registry discovers this via inventory
// (single source of the catalog; no central dispatch list).
inventory::submit! {
    crate::default_registry::PluginReg::new("identity_sled", |_ctx| std::sync::Arc::new(IdentitySledPlugin::new()))
}
