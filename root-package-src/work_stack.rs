//! Work stack model with Vector Clock for causality tracking.
//!
//! Work stacks are the runtime container for tool node execution.
//! They are immutable once committed, hash-footprinted, and replayable.
//! Vector clocks provide causality guarantees across distributed/concurrent execution.

use serde::{Deserialize, Serialize};
use simd_json::OwnedValue as Value;
use simd_json::OwnedValue;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use uuid::Uuid;

use crate::constants::WORK_STACK_PROMOTION_THRESHOLD;
use op_execution_tracker::ExecutionRecord;

/// Vector clock for causality tracking
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct VectorClock {
    pub clocks: HashMap<String, u64>,
}

impl VectorClock {
    pub fn new() -> Self {
        Self { clocks: HashMap::new() }
    }

    /// Increment this node's clock
    pub fn increment(&mut self, node_id: &str) {
        *self.clocks.entry(node_id.to_string()).or_insert(0) += 1;
    }

    /// Merge with another vector clock (take max of each component)
    pub fn merge(&mut self, other: &VectorClock) {
        for (node, &time) in &other.clocks {
            let current = self.clocks.entry(node.clone()).or_insert(0);
            *current = (*current).max(time);
        }
    }

    /// Check if this clock happens-before another
    pub fn happens_before(&self, other: &VectorClock) -> bool {
        let mut dominated = false;
        for (node, &time) in &self.clocks {
            let other_time = other.clocks.get(node).copied().unwrap_or(0);
            if time > other_time {
                return false;
            }
            if time < other_time {
                dominated = true;
            }
        }
        for (node, &other_time) in &other.clocks {
            let time = self.clocks.get(node).copied().unwrap_or(0);
            if time < other_time {
                dominated = true;
            }
        }
        dominated
    }

    /// Check if two clocks are concurrent (neither happens-before the other)
    pub fn concurrent_with(&self, other: &VectorClock) -> bool {
        !self.happens_before(other) && !other.happens_before(self)
    }

    /// Serialize for hashing
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).unwrap_or_default()
    }
}

/// Work stack node types
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type")]
pub enum WorkStackNode {
    /// Tool execution node - a single JSON-RPC tool invocation
    Tool {
        tool_name: String,
        input: Value,
        /// References to outputs from previous nodes (node_id -> output_path)
        input_bindings: HashMap<String, String>,
        /// Number of retries on failure
        retry_count: u32,
    },
    /// Service verification node - verify service availability/state
    Service {
        service_name: String,
        required_state: ServiceState,
    },
}

/// Required service state for service nodes
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ServiceState {
    Running,
    Available,
    Ready,
}

/// Edge in the work stack DAG
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct WorkStackEdge {
    pub from_node: String,
    pub to_node: String,
    /// JSON path to output value from source node
    pub output_path: Option<String>,
    /// JSON path where value is bound in target node
    pub input_path: Option<String>,
}

/// Immutable work stack definition
///
/// A work stack is the runtime container for executing a workflow.
/// It binds identity, permissions, timing, and evidence.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WorkStack {
    /// Unique stack identifier
    pub stack_id: String,
    /// Nodes in the stack (tool and service nodes)
    pub nodes: HashMap<String, WorkStackNode>,
    /// Execution order (topologically sorted node IDs)
    pub execution_order: Vec<String>,
    /// Edges defining data flow
    pub edges: Vec<WorkStackEdge>,
    /// Policy governing this execution
    pub policy_id: String,
    /// Plugin versions in scope
    pub plugin_versions: HashMap<String, String>,
    /// Content hash of the canonical work stack (for caching)
    pub content_hash: String,
    /// Frequency count (how often this pattern has been executed)
    #[serde(default)]
    pub frequency_count: u64,
    /// Cache key if promoted to Btrfs cache
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_key: Option<String>,
    /// Vector clock for causality
    #[serde(default)]
    pub vector_clock: VectorClock,
    /// Monotonic reference for ordering
    pub monotonic_ns: u128,
}

impl WorkStack {
    /// Create a new work stack builder
    pub fn builder() -> WorkStackBuilder {
        WorkStackBuilder::new()
    }

    /// Compute the content hash for this work stack
    fn compute_content_hash(&self) -> String {
        let mut hasher = Sha256::new();

        // Hash nodes in deterministic order
        let mut node_ids: Vec<_> = self.nodes.keys().collect();
        node_ids.sort();
        for id in node_ids {
            hasher.update(id.as_bytes());
            hasher.update(serde_json::to_vec(&self.nodes[id]).unwrap_or_default());
        }

        // Hash edges
        for edge in &self.edges {
            hasher.update(serde_json::to_vec(edge).unwrap_or_default());
        }

        // Hash policy
        hasher.update(self.policy_id.as_bytes());

        // Hash vector clock
        hasher.update(self.vector_clock.to_bytes());

        hex::encode(hasher.finalize())
    }

    /// Check if this work stack is a single tool invocation
    pub fn is_single_tool(&self) -> bool {
        self.nodes.len() == 1
            && self.nodes.values().all(|n| matches!(n, WorkStackNode::Tool { .. }))
    }

    /// Get the single tool name if this is a single-tool stack
    pub fn single_tool_name(&self) -> Option<&str> {
        if self.is_single_tool() {
            self.nodes.values().next().and_then(|n| match n {
                WorkStackNode::Tool { tool_name, .. } => Some(tool_name.as_str()),
                _ => None,
            })
        } else {
            None
        }
    }

    /// Increment frequency and check for cache promotion
    pub fn increment_frequency(&mut self) -> bool {
        self.frequency_count += 1;

        if self.frequency_count >= WORK_STACK_PROMOTION_THRESHOLD && self.cache_key.is_none() {
            self.cache_key = Some(self.content_hash.clone());
            true // Indicates promotion occurred
        } else {
            false
        }
    }

    /// Check if this stack should be promoted to cache
    pub fn should_promote(&self) -> bool {
        self.frequency_count >= WORK_STACK_PROMOTION_THRESHOLD && self.cache_key.is_none()
    }

    /// Check if this stack is cached
    pub fn is_cached(&self) -> bool {
        self.cache_key.is_some()
    }

    /// Update vector clock for a node execution
    pub fn record_node_execution(&mut self, node_id: &str) {
        self.vector_clock.increment(node_id);
    }
}

/// Builder for constructing work stacks
pub struct WorkStackBuilder {
    stack_id: String,
    nodes: HashMap<String, WorkStackNode>,
    edges: Vec<WorkStackEdge>,
    policy_id: String,
    plugin_versions: HashMap<String, String>,
    vector_clock: VectorClock,
}

impl WorkStackBuilder {
    pub fn new() -> Self {
        Self {
            stack_id: Uuid::new_v4().to_string(),
            nodes: HashMap::new(),
            edges: Vec::new(),
            policy_id: "policy-default".to_string(),
            plugin_versions: HashMap::new(),
            vector_clock: VectorClock::new(),
        }
    }

    pub fn stack_id(mut self, id: impl Into<String>) -> Self {
        self.stack_id = id.into();
        self
    }

    pub fn policy_id(mut self, id: impl Into<String>) -> Self {
        self.policy_id = id.into();
        self
    }

    /// Add a tool node
    pub fn add_tool(
        mut self,
        node_id: impl Into<String>,
        tool_name: impl Into<String>,
        input: Value,
    ) -> Self {
        self.nodes.insert(
            node_id.into(),
            WorkStackNode::Tool {
                tool_name: tool_name.into(),
                input,
                input_bindings: HashMap::new(),
                retry_count: 0,
            },
        );
        self
    }

    /// Add a tool node with input bindings from other nodes
    pub fn add_tool_with_bindings(
        mut self,
        node_id: impl Into<String>,
        tool_name: impl Into<String>,
        input: Value,
        bindings: HashMap<String, String>,
    ) -> Self {
        self.nodes.insert(
            node_id.into(),
            WorkStackNode::Tool {
                tool_name: tool_name.into(),
                input,
                input_bindings: bindings,
                retry_count: 0,
            },
        );
        self
    }

    /// Add a service verification node
    pub fn add_service(
        mut self,
        node_id: impl Into<String>,
        service_name: impl Into<String>,
        required_state: ServiceState,
    ) -> Self {
        self.nodes.insert(
            node_id.into(),
            WorkStackNode::Service {
                service_name: service_name.into(),
                required_state,
            },
        );
        self
    }

    /// Add an edge between nodes
    pub fn add_edge(
        mut self,
        from: impl Into<String>,
        to: impl Into<String>,
    ) -> Self {
        self.edges.push(WorkStackEdge {
            from_node: from.into(),
            to_node: to.into(),
            output_path: None,
            input_path: None,
        });
        self
    }

    /// Add a data-flow edge between nodes
    pub fn add_data_edge(
        mut self,
        from: impl Into<String>,
        to: impl Into<String>,
        output_path: impl Into<String>,
        input_path: impl Into<String>,
    ) -> Self {
        self.edges.push(WorkStackEdge {
            from_node: from.into(),
            to_node: to.into(),
            output_path: Some(output_path.into()),
            input_path: Some(input_path.into()),
        });
        self
    }

    /// Register a plugin version used by this stack
    pub fn plugin_version(
        mut self,
        plugin: impl Into<String>,
        version: impl Into<String>,
    ) -> Self {
        self.plugin_versions.insert(plugin.into(), version.into());
        self
    }

    /// Set retry count for a tool node
    pub fn set_retry(mut self, node_id: &str, count: u32) -> Self {
        if let Some(WorkStackNode::Tool { retry_count, .. }) = self.nodes.get_mut(node_id) {
            *retry_count = count;
        }
        self
    }

    /// Merge with existing vector clock
    pub fn with_vector_clock(mut self, clock: VectorClock) -> Self {
        self.vector_clock.merge(&clock);
        self
    }

    /// Build the work stack with topological sort
    pub fn build(self) -> WorkStack {
        let execution_order = self.topological_sort();
        let monotonic_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();

        let mut stack = WorkStack {
            stack_id: self.stack_id,
            nodes: self.nodes,
            execution_order,
            edges: self.edges,
            policy_id: self.policy_id,
            plugin_versions: self.plugin_versions,
            content_hash: String::new(),
            frequency_count: 0,
            cache_key: None,
            vector_clock: self.vector_clock,
            monotonic_ns,
        };

        stack.content_hash = stack.compute_content_hash();
        stack
    }

    /// Topological sort of nodes based on edges
    fn topological_sort(&self) -> Vec<String> {
        use std::collections::{HashSet, VecDeque};

        let mut in_degree: HashMap<&str, usize> = HashMap::new();
        let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();

        // Initialize
        for node_id in self.nodes.keys() {
            in_degree.insert(node_id.as_str(), 0);
            adjacency.insert(node_id.as_str(), Vec::new());
        }

        // Build graph
        for edge in &self.edges {
            if let Some(adj) = adjacency.get_mut(edge.from_node.as_str()) {
                adj.push(edge.to_node.as_str());
            }
            if let Some(degree) = in_degree.get_mut(edge.to_node.as_str()) {
                *degree += 1;
            }
        }

        // Kahn's algorithm
        let mut queue: VecDeque<&str> = in_degree
            .iter()
            .filter(|(_, &d)| d == 0)
            .map(|(&id, _)| id)
            .collect();

        let mut result = Vec::new();

        while let Some(node) = queue.pop_front() {
            result.push(node.to_string());

            if let Some(neighbors) = adjacency.get(node) {
                for &neighbor in neighbors {
                    if let Some(degree) = in_degree.get_mut(neighbor) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push_back(neighbor);
                        }
                    }
                }
            }
        }

        result
    }
}

impl Default for WorkStackBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Work stack execution state
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WorkStackExecution {
    pub stack_id: String,
    pub execution_id: String,
    pub node_results: HashMap<String, ExecutionRecord>,
    pub started_at_ns: u128,
    pub completed_at_ns: Option<u128>,
    pub status: WorkStackStatus,
    pub vector_clock: VectorClock,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum WorkStackStatus {
    Pending,
    Running,
    Completed,
    Failed(String),
}

impl WorkStackExecution {
    pub fn new(stack_id: String) -> Self {
        Self {
            stack_id,
            execution_id: Uuid::new_v4().to_string(),
            node_results: HashMap::new(),
            started_at_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            completed_at_ns: None,
            status: WorkStackStatus::Pending,
            vector_clock: VectorClock::new(),
        }
    }

    pub fn record_node_result(&mut self, node_id: String, record: ExecutionRecord) {
        self.vector_clock.increment(&node_id);
        self.node_results.insert(node_id, record);
    }

    pub fn complete(&mut self) {
        self.completed_at_ns = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
        );
        self.status = WorkStackStatus::Completed;
    }

    pub fn fail(&mut self, reason: String) {
        self.completed_at_ns = Some(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
        );
        self.status = WorkStackStatus::Failed(reason);
    }

    /// Get the final execution record (last in chain)
    pub fn final_record(&self) -> Option<&ExecutionRecord> {
        self.node_results
            .values()
            .max_by_key(|r| r.timing.monotonic_ns)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_clock_happens_before() {
        let mut a = VectorClock::new();
        a.increment("node1");

        let mut b = a.clone();
        b.increment("node1");
        b.increment("node2");

        assert!(a.happens_before(&b));
        assert!(!b.happens_before(&a));
    }

    #[test]
    fn test_vector_clock_concurrent() {
        let mut a = VectorClock::new();
        a.increment("node1");

        let mut b = VectorClock::new();
        b.increment("node2");

        assert!(a.concurrent_with(&b));
    }

    #[test]
    fn test_single_tool_stack() {
        let stack = WorkStack::builder()
            .add_tool("node1", "test.tool", simd_json::json!({}))
            .build();

        assert!(stack.is_single_tool());
        assert_eq!(stack.single_tool_name(), Some("test.tool"));
    }
}
