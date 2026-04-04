//! Event Chain - Blockchain-style Compliance and Reproducibility Layer
//!
//! Provides tamper-evident audit trail through:
//! - Hash-linked events for every state transition
//! - Merkle tree batching for scale
//! - Schema-aware canonical hashing
//! - Tag-scoped proofs for compliance
//! - Reproducible replay from footprints
//!
//! Each event record contains:
//! - `event_id` (monotonic or UUID)
//! - `prev_hash` (hash of previous event)
//! - `event_hash` = H(prev_hash || canonical_event_payload)
//! - `timestamp`
//! - `actor_id` + `capability_id`
//! - `plugin_id` + `schema_version`
//! - `op` (operation type)
//! - `target` (object path / selector)
//! - `tags_touched` (computed from schema)
//! - `decision` (allow/deny) + `deny_reason`
//! - `input_patch_hash`
//! - `result_effective_hash` (post-compile)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use simd_json::prelude::*;
use simd_json::OwnedValue as Value;
use simd_json::ValueBuilder;
use std::collections::HashMap;

use crate::schema_validator::canonicalize_json;

/// Operation types for state transitions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum OperationType {
    /// Apply an immutable wrapper to a plugin state
    ApplyImmutableWrapper,
    /// Apply a tunable patch
    ApplyTunablePatch,
    /// Schema migration
    Migrate,
    /// Reconcile state with reality
    Reconcile,
    /// Emit a D-Bus signal
    EmitSignal,
    /// Property read
    PropertyGet,
    /// Property write
    PropertySet,
    /// Method invocation
    MethodCall,
    /// Snapshot creation
    CreateSnapshot,
    /// State import
    Import,
    /// State export
    Export,
    /// Custom operation
    Custom(String),
}

/// Decision result for an operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Allow,
    Deny,
}

/// How an action came to be initiated — the autonomy provenance dimension.
///
/// Every model action must declare its origin so auditors can distinguish
/// human-instructed execution from autonomous model decisions. This is
/// what makes the trust boundary enforceable and verifiable, not just policy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActionOrigin {
    /// A human, user, or parent agent explicitly requested this action.
    Instructed {
        /// Identity of the instructing party (user ID, agent ID, etc.)
        by: String,
        /// Session or conversation context reference.
        session_id: Option<String>,
        /// Hash of the prompt/instruction that triggered this, for traceability.
        prompt_ref: Option<String>,
    },
    /// The model reasoned and decided to act without explicit instruction.
    /// Autonomous actions are subject to stricter policy enforcement.
    Autonomous {
        /// Reference to the vector ID in Qdrant capturing the semantic
        /// context that drove the decision ("why it acted alone").
        reasoning_ref: Option<String>,
        /// Model's self-reported confidence in the decision (0.0–1.0).
        confidence: Option<f32>,
    },
    /// A system event triggered the action (no human or model decision involved).
    Reactive {
        /// Description of the trigger: D-Bus signal path, cron expression,
        /// state change event ID, etc.
        trigger: String,
    },
}

/// Reason for denial
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DenyReason {
    /// Tag was locked by immutable wrapper
    TagLock { tag: String, wrapper_id: String },
    /// Constraint validation failed
    ConstraintFail { constraint: String, message: String },
    /// Missing required capability
    CapabilityMissing { capability: String },
    /// Schema validation failed
    SchemaValidation { errors: Vec<String> },
    /// Read-only field modification attempted
    ReadOnlyViolation { field: String },
    /// Custom denial reason
    Custom { reason: String },
}

/// A single event in the chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainEvent {
    /// Monotonic event ID
    pub event_id: u64,
    /// Hash of the previous event
    pub prev_hash: String,
    /// Hash of this event: H(prev_hash || canonical_payload)
    pub event_hash: String,
    /// Event timestamp
    pub timestamp: DateTime<Utc>,
    /// Actor who initiated the operation
    pub actor_id: String,
    /// Capability used for the operation
    pub capability_id: Option<String>,
    /// Plugin that owns the state
    pub plugin_id: String,
    /// Schema version at time of event
    pub schema_version: String,
    /// Type of operation
    pub op: OperationType,
    /// Target object path or selector
    pub target: String,
    /// Tags touched by this operation (computed from schema)
    pub tags_touched: Vec<String>,
    /// Decision: allow or deny
    pub decision: Decision,
    /// Reason for denial (if denied)
    pub deny_reason: Option<DenyReason>,
    /// Hash of the input patch/payload
    pub input_patch_hash: String,
    /// Hash of the resulting effective state (if allowed)
    pub result_effective_hash: Option<String>,
    /// Optional delta hash for incremental verification
    pub db_delta_hash: Option<String>,
    /// Reference to a snapshot (if this event creates one)
    pub snapshot_ref: Option<String>,
    /// Autonomy provenance: was this instructed, autonomous, or reactive?
    /// None = legacy event predating this field; treat as unknown.
    pub action_origin: Option<ActionOrigin>,
    /// The user who initiated the conversation that led to this event.
    /// None for purely system/reactive events with no human context.
    pub user_id: Option<String>,
    /// The conversation (chat session) this event belongs to.
    /// Groups the full why→what→who chain for a single session.
    /// Indexed for efficient per-conversation audit queries.
    pub conversation_id: Option<String>,
}

impl ChainEvent {
    /// Create a new event with computed hash
    pub fn new(
        event_id: u64,
        prev_hash: String,
        actor_id: String,
        plugin_id: String,
        schema_version: String,
        op: OperationType,
        target: String,
        tags_touched: Vec<String>,
        decision: Decision,
        input_patch: &Value,
    ) -> Self {
        let timestamp = Utc::now();
        let input_patch_hash = compute_hash(&canonicalize_json(input_patch));

        let mut event = Self {
            event_id,
            prev_hash: prev_hash.clone(),
            event_hash: String::new(), // Computed below
            timestamp,
            actor_id,
            capability_id: None,
            plugin_id,
            schema_version,
            op,
            target,
            tags_touched,
            decision,
            deny_reason: None,
            input_patch_hash,
            result_effective_hash: None,
            db_delta_hash: None,
            snapshot_ref: None,
            action_origin: None,
            user_id: None,
            conversation_id: None,
        };

        // Compute event hash
        event.event_hash = event.compute_hash();
        event
    }

    /// Compute the hash of this event
    fn compute_hash(&self) -> String {
        let payload = CanonicalEventPayload {
            event_id: self.event_id,
            prev_hash: &self.prev_hash,
            timestamp: self.timestamp,
            actor_id: &self.actor_id,
            capability_id: self.capability_id.as_deref(),
            plugin_id: &self.plugin_id,
            schema_version: &self.schema_version,
            op: &self.op,
            target: &self.target,
            tags_touched: &self.tags_touched,
            decision: &self.decision,
            deny_reason: self.deny_reason.as_ref(),
            input_patch_hash: &self.input_patch_hash,
            result_effective_hash: self.result_effective_hash.as_deref(),
        };

        let canonical = simd_json::serde::to_owned_value(&payload).unwrap_or_default();
        let canonical = canonicalize_json(&canonical);
        compute_hash(&canonical)
    }

    /// Set the result effective hash after successful operation
    pub fn with_result_hash(mut self, hash: String) -> Self {
        self.result_effective_hash = Some(hash);
        self.event_hash = self.compute_hash();
        self
    }

    /// Set deny reason
    pub fn with_deny_reason(mut self, reason: DenyReason) -> Self {
        self.deny_reason = Some(reason);
        self.event_hash = self.compute_hash();
        self
    }

    /// Set capability ID
    pub fn with_capability(mut self, capability: String) -> Self {
        self.capability_id = Some(capability);
        self.event_hash = self.compute_hash();
        self
    }

    /// Verify this event's hash against its content
    pub fn verify(&self) -> bool {
        let computed = self.compute_hash();
        computed == self.event_hash
    }
}

/// Canonical payload structure for consistent hashing
#[derive(Serialize)]
struct CanonicalEventPayload<'a> {
    event_id: u64,
    prev_hash: &'a str,
    timestamp: DateTime<Utc>,
    actor_id: &'a str,
    capability_id: Option<&'a str>,
    plugin_id: &'a str,
    schema_version: &'a str,
    op: &'a OperationType,
    target: &'a str,
    tags_touched: &'a [String],
    decision: &'a Decision,
    deny_reason: Option<&'a DenyReason>,
    input_patch_hash: &'a str,
    result_effective_hash: Option<&'a str>,
}

/// Merkle tree node for batch proofs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleNode {
    pub hash: String,
    pub left: Option<Box<MerkleNode>>,
    pub right: Option<Box<MerkleNode>>,
}

/// A batch of events with Merkle root
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventBatch {
    /// Merkle root of all event hashes in this batch
    pub batch_root: String,
    /// Range of event IDs in this batch
    pub first_event_id: u64,
    pub last_event_id: u64,
    /// Hash of the previous batch root (for chaining batches)
    pub prev_batch_root: Option<String>,
    /// Timestamp when batch was finalized
    pub timestamp: DateTime<Utc>,
    /// Number of events in this batch
    pub event_count: usize,
}

impl EventBatch {
    /// Create a new batch from a list of event hashes
    pub fn from_events(events: &[ChainEvent], prev_batch_root: Option<String>) -> Option<Self> {
        if events.is_empty() {
            return None;
        }

        let hashes: Vec<&str> = events.iter().map(|e| e.event_hash.as_str()).collect();
        let batch_root = compute_merkle_root(&hashes);

        Some(Self {
            batch_root,
            first_event_id: events.first().unwrap().event_id,
            last_event_id: events.last().unwrap().event_id,
            prev_batch_root,
            timestamp: Utc::now(),
            event_count: events.len(),
        })
    }

    /// Generate a Merkle proof for a specific event
    pub fn generate_proof(events: &[ChainEvent], event_id: u64) -> Option<MerkleProof> {
        let idx = events.iter().position(|e| e.event_id == event_id)?;
        let hashes: Vec<&str> = events.iter().map(|e| e.event_hash.as_str()).collect();

        let siblings = compute_merkle_proof(&hashes, idx);
        let root = compute_merkle_root(&hashes);

        Some(MerkleProof {
            event_hash: events[idx].event_hash.clone(),
            event_id,
            siblings,
            root,
        })
    }
}

/// Merkle proof for a single event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MerkleProof {
    /// Hash of the event being proved
    pub event_hash: String,
    /// Event ID
    pub event_id: u64,
    /// Sibling hashes needed to reconstruct root
    pub siblings: Vec<(String, bool)>, // (hash, is_right)
    /// Expected root hash
    pub root: String,
}

impl MerkleProof {
    /// Verify this proof
    pub fn verify(&self) -> bool {
        let mut current = self.event_hash.clone();

        for (sibling, is_right) in &self.siblings {
            current = if *is_right {
                compute_hash_pair(&current, sibling)
            } else {
                compute_hash_pair(sibling, &current)
            };
        }

        current == self.root
    }
}

/// Snapshot of plugin state for fast rebuild
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    /// Snapshot ID (hash of content)
    pub snapshot_id: String,
    /// Event ID at which this snapshot was taken
    pub at_event_id: u64,
    /// Plugin ID
    pub plugin_id: String,
    /// Schema version
    pub schema_version: String,
    /// Stub hash
    pub stub_hash: String,
    /// Immutable wrappers hash (or list of wrapper hashes)
    pub immutable_wrappers_hash: String,
    /// Tunable patch hash
    pub tunable_patch_hash: String,
    /// Effective state hash (computed from above)
    pub effective_hash: String,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// The actual state data
    pub state: Value,
}

impl StateSnapshot {
    /// Create a new snapshot
    pub fn new(at_event_id: u64, plugin_id: String, schema_version: String, state: Value) -> Self {
        let canonical = canonicalize_json(&state);
        let effective_hash = compute_hash(&canonical);

        // For now, stub/wrapper/tunable hashes are derived from effective
        // In a real implementation, these would be tracked separately
        let stub_hash = effective_hash.clone();
        let immutable_wrappers_hash = compute_hash(&simd_json::json!([]));
        let tunable_patch_hash = compute_hash(&simd_json::json!({}));

        let mut snapshot = Self {
            snapshot_id: String::new(),
            at_event_id,
            plugin_id,
            schema_version,
            stub_hash,
            immutable_wrappers_hash,
            tunable_patch_hash,
            effective_hash,
            timestamp: Utc::now(),
            state,
        };

        // Compute snapshot ID from all hashes
        let id_input = format!(
            "{}:{}:{}:{}:{}",
            snapshot.at_event_id,
            snapshot.stub_hash,
            snapshot.immutable_wrappers_hash,
            snapshot.tunable_patch_hash,
            snapshot.effective_hash
        );
        snapshot.snapshot_id = compute_hash_str(&id_input);
        snapshot
    }

    /// Verify snapshot integrity
    pub fn verify(&self) -> bool {
        let canonical = canonicalize_json(&self.state);
        let computed = compute_hash(&canonical);
        computed == self.effective_hash
    }
}

/// The event chain - append-only ledger
pub struct EventChain {
    /// All events in order
    events: Vec<ChainEvent>,
    /// Finalized batches
    batches: Vec<EventBatch>,
    /// Snapshots for fast rebuild
    snapshots: HashMap<String, StateSnapshot>,
    /// Configuration
    config: ChainConfig,
    /// Genesis hash (first prev_hash)
    genesis_hash: String,
}

/// Configuration for the event chain
#[derive(Debug, Clone)]
pub struct ChainConfig {
    /// Number of events per batch
    pub batch_size: usize,
    /// Whether to auto-batch when batch_size is reached
    pub auto_batch: bool,
}

impl Default for ChainConfig {
    fn default() -> Self {
        Self {
            batch_size: 1000,
            auto_batch: true,
        }
    }
}

impl EventChain {
    /// Create a new event chain
    pub fn new(config: ChainConfig) -> Self {
        Self {
            events: Vec::new(),
            batches: Vec::new(),
            snapshots: HashMap::new(),
            config,
            genesis_hash: compute_hash_str("genesis"),
        }
    }

    /// Get the hash of the last event (or genesis)
    pub fn last_hash(&self) -> &str {
        self.events
            .last()
            .map(|e| e.event_hash.as_str())
            .unwrap_or(&self.genesis_hash)
    }

    /// Get the next event ID
    pub fn next_event_id(&self) -> u64 {
        self.events.last().map(|e| e.event_id + 1).unwrap_or(1)
    }

    /// Append a new event to the chain
    pub fn append(&mut self, mut event: ChainEvent) -> &ChainEvent {
        // Ensure prev_hash matches
        event.prev_hash = self.last_hash().to_string();
        event.event_id = self.next_event_id();
        event.event_hash = event.compute_hash();

        self.events.push(event);

        // Auto-batch if configured
        if self.config.auto_batch && self.unbatched_count() >= self.config.batch_size {
            self.create_batch();
        }

        self.events.last().unwrap()
    }

    /// Create a new event and append it
    pub fn record(
        &mut self,
        actor_id: String,
        plugin_id: String,
        schema_version: String,
        op: OperationType,
        target: String,
        tags_touched: Vec<String>,
        decision: Decision,
        input_patch: &Value,
    ) -> &ChainEvent {
        let event = ChainEvent::new(
            self.next_event_id(),
            self.last_hash().to_string(),
            actor_id,
            plugin_id,
            schema_version,
            op,
            target,
            tags_touched,
            decision,
            input_patch,
        );
        self.append(event)
    }

    /// Get number of unbatched events
    fn unbatched_count(&self) -> usize {
        let last_batched = self.batches.last().map(|b| b.last_event_id).unwrap_or(0);
        self.events
            .iter()
            .filter(|e| e.event_id > last_batched)
            .count()
    }

    /// Create a batch from unbatched events
    pub fn create_batch(&mut self) -> Option<&EventBatch> {
        let last_batched = self.batches.last().map(|b| b.last_event_id).unwrap_or(0);
        let unbatched: Vec<_> = self
            .events
            .iter()
            .filter(|e| e.event_id > last_batched)
            .cloned()
            .collect();

        if unbatched.is_empty() {
            return None;
        }

        let prev_root = self.batches.last().map(|b| b.batch_root.clone());
        let batch = EventBatch::from_events(&unbatched, prev_root)?;
        self.batches.push(batch);
        self.batches.last()
    }

    /// Create a snapshot at the current state
    pub fn create_snapshot(
        &mut self,
        plugin_id: String,
        schema_version: String,
        state: Value,
    ) -> &StateSnapshot {
        let event_id = self.events.last().map(|e| e.event_id).unwrap_or(0);
        let snapshot = StateSnapshot::new(event_id, plugin_id, schema_version, state);
        let id = snapshot.snapshot_id.clone();
        self.snapshots.insert(id.clone(), snapshot);
        self.snapshots.get(&id).unwrap()
    }

    /// Verify the entire chain
    pub fn verify_chain(&self) -> ChainVerificationResult {
        let mut result = ChainVerificationResult {
            valid: true,
            events_verified: 0,
            batches_verified: 0,
            errors: Vec::new(),
        };

        // Verify event chain
        let mut expected_prev = self.genesis_hash.clone();
        for event in &self.events {
            if event.prev_hash != expected_prev {
                result.valid = false;
                result.errors.push(format!(
                    "Event {} has wrong prev_hash: expected {}, got {}",
                    event.event_id, expected_prev, event.prev_hash
                ));
            }

            if !event.verify() {
                result.valid = false;
                result
                    .errors
                    .push(format!("Event {} hash verification failed", event.event_id));
            }

            expected_prev = event.event_hash.clone();
            result.events_verified += 1;
        }

        // Verify batch chain
        for batch in &self.batches {
            // Get events in this batch
            let batch_events: Vec<_> = self
                .events
                .iter()
                .filter(|e| e.event_id >= batch.first_event_id && e.event_id <= batch.last_event_id)
                .collect();

            let hashes: Vec<&str> = batch_events.iter().map(|e| e.event_hash.as_str()).collect();
            let computed_root = compute_merkle_root(&hashes);

            if computed_root != batch.batch_root {
                result.valid = false;
                result.errors.push(format!(
                    "Batch {}-{} root mismatch: expected {}, computed {}",
                    batch.first_event_id, batch.last_event_id, batch.batch_root, computed_root
                ));
            }

            result.batches_verified += 1;
        }

        result
    }

    /// Query events by tag
    pub fn events_touching_tag(&self, tag: &str) -> Vec<&ChainEvent> {
        self.events
            .iter()
            .filter(|e| e.tags_touched.contains(&tag.to_string()))
            .collect()
    }

    /// Query events by plugin
    pub fn events_for_plugin(&self, plugin_id: &str) -> Vec<&ChainEvent> {
        self.events
            .iter()
            .filter(|e| e.plugin_id == plugin_id)
            .collect()
    }

    /// Prove that a tag was never touched by tunable writes
    pub fn prove_tag_immutability(&self, tag: &str) -> TagImmutabilityProof {
        let tunable_touches: Vec<_> = self
            .events
            .iter()
            .filter(|e| {
                matches!(e.op, OperationType::ApplyTunablePatch)
                    && e.tags_touched.contains(&tag.to_string())
                    && e.decision == Decision::Allow
            })
            .collect();

        TagImmutabilityProof {
            tag: tag.to_string(),
            is_immutable: tunable_touches.is_empty(),
            violations: tunable_touches.iter().map(|e| e.event_id).collect(),
            total_events_checked: self.events.len(),
        }
    }

    /// Get all events
    pub fn events(&self) -> &[ChainEvent] {
        &self.events
    }

    /// Get all batches
    pub fn batches(&self) -> &[EventBatch] {
        &self.batches
    }

    /// Get a snapshot by ID
    pub fn get_snapshot(&self, id: &str) -> Option<&StateSnapshot> {
        self.snapshots.get(id)
    }
}

/// Result of chain verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainVerificationResult {
    pub valid: bool,
    pub events_verified: usize,
    pub batches_verified: usize,
    pub errors: Vec<String>,
}

/// Proof that a tag was never modified by tunable writes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagImmutabilityProof {
    pub tag: String,
    pub is_immutable: bool,
    pub violations: Vec<u64>,
    pub total_events_checked: usize,
}

// =============================================================================
// Hash utilities
// =============================================================================

/// Compute hash of a JSON value
fn compute_hash(value: &Value) -> String {
    let canonical_str = simd_json::to_string(value).unwrap_or_default();
    format!("{:x}", md5::compute(canonical_str.as_bytes()))
}

/// Compute hash of a string
fn compute_hash_str(s: &str) -> String {
    format!("{:x}", md5::compute(s.as_bytes()))
}

/// Compute hash of two hashes concatenated
fn compute_hash_pair(left: &str, right: &str) -> String {
    compute_hash_str(&format!("{}{}", left, right))
}

/// Compute Merkle root from a list of hashes
fn compute_merkle_root(hashes: &[&str]) -> String {
    if hashes.is_empty() {
        return compute_hash_str("empty");
    }
    if hashes.len() == 1 {
        return hashes[0].to_string();
    }

    let mut level: Vec<String> = hashes.iter().map(|s| s.to_string()).collect();

    while level.len() > 1 {
        let mut next_level = Vec::new();
        for chunk in level.chunks(2) {
            if chunk.len() == 2 {
                next_level.push(compute_hash_pair(&chunk[0], &chunk[1]));
            } else {
                // Odd number: duplicate last hash
                next_level.push(compute_hash_pair(&chunk[0], &chunk[0]));
            }
        }
        level = next_level;
    }

    level.into_iter().next().unwrap_or_default()
}

/// Compute Merkle proof siblings for a specific index
fn compute_merkle_proof(hashes: &[&str], index: usize) -> Vec<(String, bool)> {
    if hashes.len() <= 1 {
        return Vec::new();
    }

    let mut siblings = Vec::new();
    let mut level: Vec<String> = hashes.iter().map(|s| s.to_string()).collect();
    let mut idx = index;

    while level.len() > 1 {
        let sibling_idx = if idx % 2 == 0 { idx + 1 } else { idx - 1 };
        let is_right = idx % 2 == 0;

        if sibling_idx < level.len() {
            siblings.push((level[sibling_idx].clone(), is_right));
        } else {
            // Odd number: duplicate
            siblings.push((level[idx].clone(), is_right));
        }

        // Build next level
        let mut next_level = Vec::new();
        for chunk in level.chunks(2) {
            if chunk.len() == 2 {
                next_level.push(compute_hash_pair(&chunk[0], &chunk[1]));
            } else {
                next_level.push(compute_hash_pair(&chunk[0], &chunk[0]));
            }
        }

        idx /= 2;
        level = next_level;
    }

    siblings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_chain_basic() {
        let mut chain = EventChain::new(ChainConfig::default());

        chain.record(
            "user1".to_string(),
            "lxc".to_string(),
            "2.0.0".to_string(),
            OperationType::ApplyTunablePatch,
            "/containers/100".to_string(),
            vec!["container".to_string()],
            Decision::Allow,
            &simd_json::json!({"memory": 1024}),
        );

        assert_eq!(chain.events().len(), 1);
        assert!(chain.verify_chain().valid);
    }

    #[test]
    fn test_event_chain_integrity() {
        let mut chain = EventChain::new(ChainConfig::default());

        for i in 0..5 {
            chain.record(
                "user1".to_string(),
                "lxc".to_string(),
                "2.0.0".to_string(),
                OperationType::PropertySet,
                format!("/containers/{}", i),
                vec!["container".to_string()],
                Decision::Allow,
                &simd_json::json!({"value": i}),
            );
        }

        let result = chain.verify_chain();
        assert!(result.valid);
        assert_eq!(result.events_verified, 5);
    }

    #[test]
    fn test_merkle_root() {
        let hashes = vec!["a", "b", "c", "d"];
        let root = compute_merkle_root(&hashes);
        assert!(!root.is_empty());

        // Same hashes should produce same root
        let root2 = compute_merkle_root(&hashes);
        assert_eq!(root, root2);
    }

    #[test]
    fn test_merkle_proof() {
        let hashes = vec!["a", "b", "c", "d"];
        let root = compute_merkle_root(&hashes);

        let proof_siblings = compute_merkle_proof(&hashes, 2);

        // Verify proof manually
        let mut current = "c".to_string();
        for (sibling, is_right) in &proof_siblings {
            current = if *is_right {
                compute_hash_pair(&current, sibling)
            } else {
                compute_hash_pair(sibling, &current)
            };
        }

        assert_eq!(current, root);
    }

    #[test]
    fn test_tag_immutability_proof() {
        let mut chain = EventChain::new(ChainConfig::default());

        // Record events with different tags
        chain.record(
            "user1".to_string(),
            "lxc".to_string(),
            "2.0.0".to_string(),
            OperationType::ApplyTunablePatch,
            "/containers/100".to_string(),
            vec!["container".to_string()],
            Decision::Allow,
            &simd_json::json!({}),
        );

        chain.record(
            "user1".to_string(),
            "lxc".to_string(),
            "2.0.0".to_string(),
            OperationType::ApplyImmutableWrapper,
            "/containers/100".to_string(),
            vec!["security".to_string()],
            Decision::Allow,
            &simd_json::json!({}),
        );

        // Security tag should be immutable (no tunable touches)
        let proof = chain.prove_tag_immutability("security");
        assert!(proof.is_immutable);

        // Container tag was touched by tunable
        let proof = chain.prove_tag_immutability("container");
        assert!(!proof.is_immutable);
    }

    #[test]
    fn test_batch_creation() {
        let config = ChainConfig {
            batch_size: 3,
            auto_batch: false,
        };
        let mut chain = EventChain::new(config);

        for i in 0..5 {
            chain.record(
                "user1".to_string(),
                "lxc".to_string(),
                "2.0.0".to_string(),
                OperationType::PropertySet,
                format!("/test/{}", i),
                vec![],
                Decision::Allow,
                &simd_json::json!({}),
            );
        }

        let batch = chain.create_batch().unwrap();
        assert_eq!(batch.event_count, 5);
        assert_eq!(batch.first_event_id, 1);
        assert_eq!(batch.last_event_id, 5);
    }

    #[test]
    fn test_snapshot() {
        let mut chain = EventChain::new(ChainConfig::default());

        chain.record(
            "user1".to_string(),
            "lxc".to_string(),
            "2.0.0".to_string(),
            OperationType::ApplyTunablePatch,
            "/containers".to_string(),
            vec![],
            Decision::Allow,
            &simd_json::json!({}),
        );

        let state = simd_json::json!({
            "containers": [{"id": "100", "running": true}]
        });

        let snapshot = chain.create_snapshot("lxc".to_string(), "2.0.0".to_string(), state);

        assert!(snapshot.verify());
        assert_eq!(snapshot.at_event_id, 1);
    }
}
