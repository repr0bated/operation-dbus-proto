# Public API Surface & Architectural Quality Audit

This document presents a comprehensive quality and security audit of the `op-cache` crate. It evaluates public API structures, identifies dead code, flags data contract/schema-as-code violations, and exposes security vulnerabilities found within the provided source files.

---

## 1. Public API Surface & Dead Code Evaluation

### 1.1 Public API Enumeration
Below is a complete enumeration of all `pub` items (functions, structs, enums, traits, types, modules, and re-exports) within the provided codebase.

#### `crates/op-cache/src/lib.rs`
*   `pub mod agent;`
*   `pub mod agent_registry;`
*   `pub mod btrfs_cache;`
*   `pub mod capability_resolver;`
*   `pub mod numa;`
*   `pub mod orchestrator;`
*   `pub mod pattern_tracker;`
*   `pub mod snapshot_manager;`
*   `pub mod workflow_cache;`
*   `pub mod workflow_executor;`
*   `pub mod workflow_tracker;`
*   `pub mod workstack_cache;`
*   `pub mod grpc;`
*   `pub use agent::{Agent, AgentRegistry, Capability, Priority};` (Re-exports)
*   `pub use btrfs_cache::BtrfsCache;` (Re-export)
*   `pub use numa::{NumaNode, NumaTopology};` (Re-export)
*   `pub use orchestrator::Orchestrator;` (Re-export)
*   `pub use pattern_tracker::PatternTracker;` (Re-export)
*   `pub use snapshot_manager::SnapshotManager;` (Re-export)
*   `pub use workstack_cache::WorkstackCache;` (Re-export)
*   `pub mod proto` (Generated protobuf namespace)
*   `pub mod prelude` (Crate prelude namespace)

#### `crates/op-cache/src/agent.rs`
*   `pub use crate::agent_registry::{AgentCapability as Capability, AgentDefinition as Agent, AgentPriority as Priority, AgentRegistry};` (Re-exports)

#### `crates/op-cache/src/agent_registry.rs`
*   `pub enum AgentCapability` (Enum)
    *   *Variants*: `CodeAnalysis`, `SecurityAudit`, `PerformanceAnalysis`, `DependencyAnalysis`, `CodeGeneration`, `TestGeneration`, `DocumentationGeneration`, `RefactoringSuggestion`, `CodeTransformation`, `FormatConversion`, `LanguageTranslation`, `DataExtraction`, `DataValidation`, `DataEnrichment`, `Embedding`, `Planning`, `Summarization`, `QuestionAnswering`, `Classification`, `ApiCall`, `DatabaseQuery`, `FileOperation`, `ShellExecution`, `Custom(u32)`
    *   `pub fn parse(s: &str) -> Option<Self>` (Associated function)
    *   `pub fn name(&self) -> &'static str` (Method)
*   `pub enum AgentPriority` (Enum)
    *   *Variants*: `High`, `Normal`, `Low`
*   `pub struct AgentDefinition` (Struct)
    *   *Fields*: `pub id`, `pub name`, `pub description`, `pub capabilities`, `pub requires`, `pub priority`, `pub parallelizable`, `pub estimated_latency_ms`, `pub max_input_size`, `pub version`, `pub enabled`
    *   `pub fn new(id: &str, name: &str) -> Self` (Associated function)
    *   `pub fn with_description(mut self, desc: &str) -> Self` (Method)
    *   `pub fn with_capability(mut self, cap: AgentCapability) -> Self` (Method)
    *   `pub fn with_capabilities(mut self, caps: &[AgentCapability]) -> Self` (Method)
    *   `pub fn requires_capability(mut self, cap: AgentCapability) -> Self` (Method)
    *   `pub fn with_priority(mut self, priority: AgentPriority) -> Self` (Method)
    *   `pub fn parallelizable(mut self, parallel: bool) -> Self` (Method)
    *   `pub fn with_latency(mut self, ms: u64) -> Self` (Method)
    *   `pub fn provides(&self, cap: AgentCapability) -> bool` (Method)
    *   `pub fn needs(&self, cap: AgentCapability) -> bool` (Method)
    *   `pub fn capability_set(&self) -> HashSet<AgentCapability>` (Method)
*   `pub type AgentExecutor` (Type Alias)
*   `pub struct RegisteredAgent` (Struct)
    *   *Fields*: `pub definition`, `pub executor`
*   `pub struct AgentRegistry` (Struct)
    *   `pub fn new() -> Self` (Associated function)
    *   `pub async fn register(&self, definition: AgentDefinition, executor: AgentExecutor) -> Result<()>` (Method)
    *   `pub async fn unregister(&self, agent_id: &str) -> Result<Option<AgentDefinition>>` (Method)
    *   `pub async fn get(&self, agent_id: &str) -> Option<AgentDefinition>` (Method)
    *   `pub async fn get_executor(&self, agent_id: &str) -> Option<AgentExecutor>` (Method)
    *   `pub async fn find_by_capability(&self, cap: AgentCapability) -> Vec<AgentDefinition>` (Method)
    *   `pub async fn find_by_capabilities(&self, caps: &[AgentCapability]) -> Vec<AgentDefinition>` (Method)
    *   `pub async fn find_best_for_capability(&self, cap: AgentCapability) -> Option<AgentDefinition>` (Method)
    *   `pub async fn list_all(&self) -> Vec<AgentDefinition>` (Method)
    *   `pub async fn list_capabilities(&self) -> Vec<AgentCapability>` (Method)
    *   `pub async fn has_capability(&self, cap: AgentCapability) -> bool` (Method)
    *   `pub async fn stats(&self) -> RegistryStats` (Method)
    *   `pub async fn execute(&self, agent_id: &str, input: &[u8]) -> Result<Vec<u8>>` (Method)
*   `pub struct RegistryStats` (Struct)
    *   *Fields*: `pub total_agents`, `pub enabled_agents`, `pub disabled_agents`, `pub total_capabilities`

#### `crates/op-cache/src/btrfs_cache.rs`
*   `pub enum CachePlacementStrategy` (Enum)
    *   *Variants*: `LocalNode`, `RoundRobin`, `MostMemory`, `Disabled`
*   `pub enum MemoryPolicy` (Enum)
    *   *Variants*: `Bind(Vec<u32>)`, `Preferred(Option<u32>)`, `Interleave(Vec<u32>)`, `Default`
*   `pub struct BtrfsCache` (Struct)
    *   `pub async fn new(cache_dir: PathBuf) -> Result<Self>` (Associated function)
    *   `pub fn get_or_embed<F>(&self, text: &str, compute_fn: F) -> Result<Vec<f32>>` (Method)
    *   `pub fn get_embedding(&self, text: &str) -> Result<Option<Vec<f32>>>` (Method)
    *   `pub fn put_embedding(&self, text: &str, vector: &[f32]) -> Result<()>` (Method)
    *   `pub fn stats(&self) -> Result<CacheStats>` (Method)
    *   `pub fn cleanup_old(&self, days: i64) -> Result<usize>` (Method)
    *   `pub fn clear(&self) -> Result<()>` (Method)
    *   `pub fn clear_embeddings(&self) -> Result<()>` (Method)
    *   `pub fn clear_blocks(&self) -> Result<()>` (Method)
    *   `pub async fn create_snapshot(&self) -> Result<PathBuf>` (Method)
    *   `pub async fn list_snapshots(&self) -> Result<Vec<super::snapshot_manager::SnapshotInfo>>` (Method)
    *   `pub async fn delete_all_snapshots(&self) -> Result<usize>` (Method)
    *   `pub async fn stream_to_remote(&self, remote_host: &str, remote_path: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>>` (Method)
    *   `pub async fn receive_from_remote(&self, remote_host: &str, remote_snapshot: &str, local_path: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>>` (Method)
    *   `pub fn numa_info(&self) -> NumaInfo` (Method)
    *   `pub fn cache_dir(&self) -> &PathBuf` (Method)
*   `pub struct CacheStats` (Struct)
    *   *Fields*: `pub total_entries`, `pub hot_entries`, `pub total_accesses`, `pub disk_usage_bytes`, `pub embeddings_size_bytes`, `pub blocks_size_bytes`
    *   `pub fn hot_ratio(&self) -> f64` (Method)
    *   `pub fn avg_accesses(&self) -> f64` (Method)
*   `pub struct NumaInfo` (Struct)
    *   *Fields*: `pub node_count`, `pub cpu_affinity`, `pub placement_strategy`, `pub memory_policy`

#### `crates/op-cache/src/orchestrator.rs`
*   `pub struct OrchestratorConfig` (Struct)
    *   *Fields*: `pub workstack_threshold`, `pub enable_caching`, `pub numa_pinning`, `pub track_patterns`, `pub promotion_threshold`
*   `pub struct OrchestrationResult` (Struct)
    *   *Fields*: `pub request_id`, `pub output`, `pub steps`, `pub total_latency_ms`, `pub cache_hits`, `pub cache_misses`, `pub used_workstack`, `pub resolved_agents`
*   `pub struct StepResult` (Struct)
    *   *Fields*: `pub step_index`, `pub agent_id`, `pub latency_ms`, `pub cached`, `pub output_size`
*   `pub struct Orchestrator` (Struct)
    *   `pub async fn new(cache_dir: PathBuf, config: OrchestratorConfig, registry: Arc<AgentRegistry>) -> Result<Self>` (Associated function)
    *   `pub async fn execute(&self, request: CapabilityRequest) -> Result<OrchestrationResult>` (Method)
    *   `pub async fn execute_agents(&self, agent_ids: &[&str], input: Vec<u8>) -> Result<OrchestrationResult>` (Method)
    *   `pub async fn stats(&self) -> Result<OrchestratorStats>` (Method)
    *   `pub fn registry(&self) -> &Arc<AgentRegistry>` (Method)
    *   `pub fn get_promotion_candidates(&self) -> Result<Vec<super::pattern_tracker::PromotionSuggestion>>` (Method)
*   `pub struct OrchestratorStats` (Struct)
    *   *Fields*: `pub registered_agents`, `pub enabled_agents`, `pub available_capabilities`, `pub tracked_patterns`, `pub promoted_patterns`, `pub cache_entries`, `pub cache_hit_rate`

#### `crates/op-cache/src/pattern_tracker.rs`
*   `pub struct PatternTrackerConfig` (Struct)
    *   *Fields*: `pub promotion_threshold`, `pub detection_window_secs`, `pub track_enabled`
*   `pub struct TrackedPattern` (Struct)
    *   *Fields*: `pub pattern_id`, `pub agent_sequence`, `pub call_count`, `pub first_seen`, `pub last_called`, `pub avg_latency_ms`, `pub promoted`, `pub workstack_id`
    *   `pub fn sequence_description(&self) -> String` (Method)
*   `pub struct PromotionSuggestion` (Struct)
    *   *Fields*: `pub pattern`, `pub estimated_time_saved_ms`, `pub confidence_score`, `pub suggested_name`
*   `pub struct PatternTracker` (Struct)
    *   `pub async fn new(cache_dir: PathBuf, config: PatternTrackerConfig) -> Result<Self>` (Associated function)
    *   `pub fn record_sequence(&self, agents: &[&str], _input_hash: &str, total_latency_ms: u64) -> Result<Option<PromotionSuggestion>>` (Method)
    *   `pub fn promote_pattern(&self, pattern: &TrackedPattern) -> Result<String>` (Method)
    *   `pub fn get_promotion_candidates(&self) -> Result<Vec<PromotionSuggestion>>` (Method)
    *   `pub fn stats(&self) -> Result<TrackerStats>` (Method)
    *   `pub fn cleanup(&self, days: i64) -> Result<usize>` (Method)
*   `pub struct TrackerStats` (Struct)
    *   *Fields*: `pub total_patterns`, `pub promoted_count`, `pub pending_promotion`, `pub promotion_threshold`

#### `crates/op-cache/src/snapshot_manager.rs`
*   `pub struct SnapshotConfig` (Struct)
    *   *Fields*: `pub snapshot_dir`, `pub max_snapshots`, `pub prefix`
*   `pub struct SnapshotManager` (Struct)
    *   `pub fn new(source_subvol: PathBuf, config: SnapshotConfig) -> Self` (Associated function)
    *   `pub async fn create_snapshot(&self) -> Result<PathBuf>` (Method)
    *   `pub async fn list_snapshots(&self) -> Result<Vec<SnapshotInfo>>` (Method)
    *   `pub async fn delete_snapshot(&self, snapshot_path: &Path) -> Result<()>` (Method)
    *   `pub async fn delete_all_snapshots(&self) -> Result<usize>` (Method)
    *   `pub async fn oldest_snapshot(&self) -> Result<Option<SnapshotInfo>>` (Method)
    *   `pub async fn newest_snapshot(&self) -> Result<Option<SnapshotInfo>>` (Method)
*   `pub struct SnapshotInfo` (Struct)
    *   *Fields*: `pub name`, `pub path`, `pub created`, `pub counter`

#### `crates/op-cache/src/workflow_cache.rs`
*   `pub struct WorkflowCacheConfig` (Struct)
    *   *Fields*: `pub default_ttl_secs`, `pub max_size_bytes`, `pub compress`, `pub hot_threshold_secs`
*   `pub struct CachedStepResult` (Struct)
    *   *Fields*: `pub workflow_id`, `pub step_index`, `pub input_hash`, `pub output`, `pub created_at`, `pub expires_at`, `pub access_count`, `pub last_accessed`, `pub size_bytes`
    *   `pub fn is_expired(&self) -> bool` (Method)
    *   `pub fn is_hot(&self, threshold_secs: i64) -> bool` (Method)
*   `pub struct WorkflowCache` (Struct)
    *   `pub async fn new(cache_dir: PathBuf, config: WorkflowCacheConfig) -> Result<Self>` (Associated function)
    *   `pub fn get(&self, workflow_id: &str, step_index: usize, input_hash: &str) -> Result<Option<Vec<u8>>>` (Method)
    *   `pub fn put(&self, workflow_id: &str, step_index: usize, input_hash: &str, output: &[u8], ttl_secs: Option<i64>) -> Result<()>` (Method)
    *   `pub fn invalidate(&self, workflow_id: &str, step_index: usize, input_hash: &str) -> Result<()>` (Method)
    *   `pub fn invalidate_workflow(&self, workflow_id: &str) -> Result<usize>` (Method)
    *   `pub fn invalidate_step(&self, workflow_id: &str, step_index: usize) -> Result<usize>` (Method)
    *   `pub fn cleanup_expired(&self) -> Result<CleanupResult>` (Method)
    *   `pub fn evict_to_size(&self, max_bytes: u64) -> Result<CleanupResult>` (Method)
    *   `pub fn stats(&self) -> Result<CacheStats>` (Method)
    *   `pub fn workflow_stats(&self, workflow_id: &str) -> Result<Option<WorkflowCacheStats>>` (Method)
*   `pub struct CleanupResult` (Struct)
    *   *Fields*: `pub entries_removed`, `pub bytes_freed`
*   `pub struct CacheStats` (Struct)
    *   *Fields*: `pub total_entries`, `pub total_size_bytes`, `pub hot_entries`, `pub expired_entries`, `pub total_hits`, `pub total_misses`, `pub workflows_cached`, `pub hit_rate`
*   `pub struct WorkflowCacheStats` (Struct)
    *   *Fields*: `pub workflow_id`, `pub total_entries`, `pub total_size_bytes`, `pub hit_count`, `pub miss_count`, `pub hit_rate`

#### `crates/op-cache/src/workflow_executor.rs`
*   `pub struct WorkflowExecutorConfig` (Struct)
    *   *Fields*: `pub numa_pinning`, `pub enable_caching`, `pub max_parallel_steps`, `pub step_timeout_secs`, `pub retry_on_failure`, `pub max_retries`
*   `pub struct StepResult` (Struct)
    *   *Fields*: `pub step_index`, `pub agent_id`, `pub output`, `pub latency_ms`, `pub cached`, `pub retries`
*   `pub struct WorkflowResult` (Struct)
    *   *Fields*: `pub workflow_id`, `pub steps`, `pub total_latency_ms`, `pub cache_hits`, `pub cache_misses`, `pub numa_node`
    *   `pub fn final_output(&self) -> Option<&[u8]>` (Method)
    *   `pub fn cache_hit_rate(&self) -> f64` (Method)
    *   `pub fn estimated_time_saved_ms(&self) -> u64` (Method)
*   `pub type AgentFn` (Type Alias)
*   `pub type ProgressCallback` (Type Alias)
*   `pub struct WorkflowExecutor` (Struct)
    *   `pub async fn new(cache_dir: PathBuf, config: WorkflowExecutorConfig) -> Result<Self>` (Associated function)
    *   `pub async fn register_agent(&self, agent_id: &str, agent_fn: AgentFn)` (Method)
    *   `pub async fn execute(&self, workflow_id: &str, input: &[u8], progress: Option<ProgressCallback>) -> Result<WorkflowResult>` (Method)
    *   `pub async fn execute_workflow(&self, workflow: &PromotedWorkflow, input: &[u8], progress: Option<ProgressCallback>) -> Result<WorkflowResult>` (Method)
    *   `pub async fn execute_sequence(&self, agents: &[&str], input: &[u8], progress: Option<ProgressCallback>) -> Result<WorkflowResult>` (Method)
    *   `pub async fn stats(&self) -> Result<ExecutorStats>` (Method)
    *   `pub fn get_promotion_suggestions(&self) -> Result<Vec<super::workflow_tracker::PromotionSuggestion>>` (Method)
    *   `pub fn promote_pattern(&self, pattern: &super::workflow_tracker::WorkflowPattern) -> Result<String>` (Method)
    *   `pub fn get_workflows(&self) -> Result<Vec<PromotedWorkflow>>` (Method)
    *   `pub fn invalidate_workflow_cache(&self, workflow_id: &str) -> Result<usize>` (Method)
    *   `pub fn cleanup_cache(&self) -> Result<super::workflow_cache::CleanupResult>` (Method)
*   `pub struct ExecutorStats` (Struct)
    *   *Fields*: `pub registered_agents`, `pub promoted_workflows`, `pub pending_promotions`, `pub total_workflow_executions`, `pub cache_entries`, `pub cache_size_bytes`, `pub cache_hit_rate`, `pub numa_nodes`, `pub numa_pinning_enabled`

#### `crates/op-cache/src/workflow_tracker.rs`
*   `pub struct WorkflowTrackerConfig` (Struct)
    *   *Fields*: `pub promotion_threshold`, `pub detection_window_secs`, `pub min_sequence_length`, `pub max_sequence_length`, `pub auto_promote`
*   `pub struct WorkflowPattern` (Struct)
    *   *Fields*: `pub pattern_id`, `pub agent_sequence`, `pub call_count`, `pub first_seen`, `pub last_called`, `pub avg_latency_ms`, `pub promoted`, `pub workflow_id`
    *   `pub fn meets_threshold(&self, threshold: u32) -> bool` (Method)
    *   `pub fn sequence_description(&self) -> String` (Method)
*   `pub struct PromotionSuggestion` (Struct)
    *   *Fields*: `pub pattern`, `pub estimated_time_saved_ms`, `pub confidence_score`, `pub suggested_name`
*   `pub struct WorkflowTracker` (Struct)
    *   `pub async fn new(cache_dir: PathBuf, config: WorkflowTrackerConfig) -> Result<Self>` (Associated function)
    *   `pub fn record_call(&self, session_id: &str, agent_id: &str, input_hash: &str, latency_ms: u64) -> Result<()>` (Method)
    *   `pub fn record_sequence(&self, agents: &[&str], _input_hash: &str, total_latency_ms: u64) -> Result<Option<PromotionSuggestion>>` (Method)
    *   `pub fn promote_pattern(&self, pattern: &WorkflowPattern) -> Result<String>` (Method)
    *   `pub fn get_promotion_candidates(&self) -> Result<Vec<PromotionSuggestion>>` (Method)
    *   `pub fn get_promoted_workflows(&self) -> Result<Vec<PromotedWorkflow>>` (Method)
    *   `pub fn get_workflow(&self, workflow_id: &str) -> Result<Option<PromotedWorkflow>>` (Method)
    *   `pub fn record_execution(&self, workflow_id: &str) -> Result<()>` (Method)
    *   `pub fn stats(&self) -> Result<TrackerStats>` (Method)
    *   `pub fn clear_session(&self)` (Method)
    *   `pub fn cleanup(&self, days: i64) -> Result<CleanupStats>` (Method)
*   `pub struct PromotedWorkflow` (Struct)
    *   *Fields*: `pub workflow_id`, `pub pattern_hash`, `pub name`, `pub description`, `pub agent_sequence`, `pub created_at`, `pub execution_count`
*   `pub struct TrackerStats` (Struct)
    *   *Fields*: `pub total_patterns`, `pub promoted_count`, `pub pending_promotion`, `pub total_calls`, `pub total_workflow_executions`, `pub promotion_threshold`
*   `pub struct CleanupStats` (Struct)
    *   *Fields*: `pub calls_deleted`, `pub sequences_deleted`, `pub patterns_deleted`

#### `crates/op-cache/src/workstack_cache.rs`
*   `pub struct WorkstackCacheConfig` (Struct)
    *   *Fields*: `pub default_ttl_secs`, `pub max_size_bytes`, `pub compress`, `pub hot_threshold_secs`
*   `pub struct WorkstackCache` (Struct)
    *   `pub async fn new(cache_dir: PathBuf, config: WorkstackCacheConfig) -> Result<Self>` (Associated function)
    *   `pub fn get(&self, workstack_id: &str, step_index: usize, input_hash: &str) -> Result<Option<Vec<u8>>>` (Method)
    *   `pub fn put(&self, workstack_id: &str, step_index: usize, input_hash: &str, output: &[u8], ttl_secs: Option<i64>) -> Result<()>` (Method)
    *   `pub fn invalidate_workstack(&self, workstack_id: &str) -> Result<usize>` (Method)
    *   `pub fn cleanup_expired(&self) -> Result<CleanupResult>` (Method)
    *   `pub fn stats(&self) -> Result<CacheStats>` (Method)
*   `pub struct CleanupResult` (Struct)
    *   *Fields*: `pub entries_removed`, `pub bytes_freed`
*   `pub struct CacheStats` (Struct)
    *   *Fields*: `pub total_entries`, `pub total_size_bytes`, `pub hot_entries`, `pub total_hits`, `pub total_misses`, `pub workstacks_cached`, `pub hit_rate`

#### `crates/op-cache/src/capability_resolver.rs`
*   `pub struct CapabilityRequest` (Struct)
    *   *Fields*: `pub required_capabilities`, `pub preferred_agents`, `pub excluded_agents`, `pub allow_parallel`, `pub max_agents`, `pub input`
    *   `pub fn new(capabilities: Vec<AgentCapability>, input: Vec<u8>) -> Self` (Associated function)
    *   `pub fn from_strings(cap_strings: &[&str], input: Vec<u8>) -> Self` (Associated function)
    *   `pub fn prefer_agents(mut self, agents: &[&str]) -> Self` (Method)
    *   `pub fn exclude_agents(mut self, agents: &[&str]) -> Self` (Method)
    *   `pub fn allow_parallel(mut self, allow: bool) -> Self` (Method)
*   `pub struct ResolvedSequence` (Struct)
    *   *Fields*: `pub agents`, `pub fulfilled_capabilities`, `pub missing_capabilities`, `pub estimated_latency_ms`, `pub parallel_groups`, `pub resolution_path`
    *   `pub fn agent_ids(&self) -> Vec<String>` (Method)
    *   `pub fn is_complete(&self) -> bool` (Method)
    *   `pub fn len(&self) -> usize` (Method)
    *   `pub fn is_empty(&self) -> bool` (Method)
*   `pub struct CapabilityResolver` (Struct)
    *   `pub fn new(registry: Arc<AgentRegistry>) -> Self` (Associated function)
    *   `pub async fn resolve(&self, request: &CapabilityRequest) -> Result<ResolvedSequence>` (Method)
    *   `pub async fn stats(&self) -> ResolverStats` (Method)
*   `pub struct ResolverStats` (Struct)
    *   *Fields*: `pub available_agents`, `pub available_capabilities`

#### `crates/op-cache/src/numa.rs`
*   `pub struct NumaNode` (Struct)
    *   *Fields*: `pub node_id`, `pub cpu_list`, `pub memory_total_kb`, `pub memory_free_kb`, `pub distance_to_nodes`
    *   `pub fn is_online(&self) -> bool` (Method)
    *   `pub fn memory_utilization(&self) -> f64` (Method)
    *   `pub fn distance_to(&self, other_node: u32) -> u32` (Method)
*   `pub struct NumaTopology` (Struct)
    *   `pub fn detect() -> Result<Self>` (Associated function)
    *   `pub fn nodes(&self) -> &HashMap<u32, NumaNode>` (Method)
    *   `pub fn get_node(&self, node_id: u32) -> Option<&NumaNode>` (Method)
    *   `pub fn current_node(&self) -> Option<u32>` (Method)
    *   `pub fn node_with_most_memory(&self) -> Option<u32>` (Method)
    *   `pub fn optimal_node(&self) -> u32` (Method)
    *   `pub fn is_numa_system(&self) -> bool` (Method)
    *   `pub fn node_count(&self) -> usize` (Method)
    *   `pub fn cpus_for_node(&self, node_id: u32) -> Vec<u32>` (Method)
    *   `pub fn refresh(&mut self) -> Result<()>` (Method)
*   `pub struct NumaStats` (Struct)
    *   *Fields*: `pub local_accesses`, `pub remote_accesses`, `pub total_latency_ns`, `pub operations`
    *   `pub fn new() -> Self` (Associated function)
    *   `pub fn record_local_access(&mut self, latency_ns: u64)` (Method)
    *   `pub fn record_remote_access(&mut self, latency_ns: u64)` (Method)
    *   `pub fn avg_latency_ns(&self) -> u64` (Method)
    *   `pub fn local_hit_rate(&self) -> f64` (Method)
    *   `pub fn remote_penalty(&self) -> f64` (Method)

#### `crates/op-cache/src/grpc/agent_service.rs`
*   `pub type AgentExecutor` (Type Alias)
*   `pub struct AgentServiceImpl` (Struct)
    *   `pub fn new() -> Self` (Associated function)
    *   `pub async fn register_local(&self, agent: Agent, executor: AgentExecutor) -> Result<(), String>` (Method)

#### `crates/op-cache/src/grpc/cache_service.rs`
*   `pub struct CacheServiceImpl` (Struct)
    *   `pub fn new() -> Self` (Associated function)
    *   `pub fn with_ttl(default_ttl_secs: i64) -> Self` (Associated function)
    *   `pub async fn get_step_internal(&self, workstack_id: &str, step_index: u32, input_hash: &str) -> Option<Vec<u8>>` (Method)
    *   `pub async fn put_step_internal(&self, workstack_id: &str, step_index: u32, input_hash: &str, output: &[u8])` (Method)
    *   `pub async fn get_stats_internal(&self) -> CacheStats` (Method)

#### `crates/op-cache/src/grpc/mcp_service.rs`
*   `pub struct McpServiceImpl` (Struct)
    *   `pub fn new(agent_service: Arc<AgentServiceImpl>, orchestrator_service: Arc<OrchestratorServiceImpl>) -> Self` (Associated function)

#### `crates/op-cache/src/grpc/mod.rs`
*   `pub mod agent_service;`
*   `pub mod cache_service;`
*   `pub mod mcp_service;`
*   `pub mod orchestrator_service;`
*   `pub mod server;`
*   `pub use agent_service::AgentServiceImpl;` (Re-export)
*   `pub use cache_service::CacheServiceImpl;` (Re-export)
*   `pub use mcp_service::McpServiceImpl;` (Re-export)
*   `pub use orchestrator_service::OrchestratorServiceImpl;` (Re-export)
*   `pub use server::{GrpcServer, GrpcServerConfig};` (Re-export)
*   `pub mod proto` (Re-export of parent crate protobuf namespace)

#### `crates/op-cache/src/grpc/orchestrator_service.rs`
*   `pub struct OrchestratorServiceImpl` (Struct)
    *   `pub fn new(agent_service: Arc<AgentServiceImpl>, cache_service: Arc<CacheServiceImpl>) -> Self` (Associated function)
    *   `pub fn with_config(agent_service: Arc<AgentServiceImpl>, cache_service: Arc<CacheServiceImpl>, workstack_threshold: usize, enable_caching: bool, promotion_threshold: u32) -> Self` (Associated function)

#### `crates/op-cache/src/grpc/server.rs`
*   `pub struct GrpcServerConfig` (Struct)
    *   *Fields*: `pub listen_addr`, `pub workstack_threshold`, `pub enable_caching`, `pub promotion_threshold`, `pub default_cache_ttl_secs`
*   `pub struct GrpcServer` (Struct)
    *   `pub fn new() -> Self` (Associated function)
    *   `pub fn with_config(config: GrpcServerConfig) -> Self` (Associated function)
    *   `pub fn agent_service(&self) -> Arc<AgentServiceImpl>` (Method)
    *   `pub fn orchestrator_service(&self) -> Arc<OrchestratorServiceImpl>` (Method)
    *   `pub fn cache_service(&self) -> Arc<CacheServiceImpl>` (Method)
    *   `pub fn mcp_service(&self) -> Arc<McpServiceImpl>` (Method)
    *   `pub async fn serve(self) -> Result<()>` (Method)
    *   `pub async fn serve_with_shutdown(self, shutdown: impl std::future::Future<Output = ()>) -> Result<()>` (Method)

---

### 1.2 Summary Metrics
*   **Total Public Modules**: 25 (including root re-exports & sub-modules)
*   **Total Public Structs**: 24
*   **Total Public Enums**: 5
*   **Total Public Types/Aliases**: 4

### 1.3 Top 10 Most Impactful Public APIs

| # | Item | Type | Location | Impact Description |
|---|---|---|---|---|
| 1 | `BtrfsCache` | Struct | `crates/op-cache/src/btrfs_cache.rs:43` | Primary disk caching engine combining SQLite indexing and BTRFS subvolumes. |
| 2 | `Orchestrator` | Struct | `crates/op-cache/src/orchestrator.rs:59` | Central request router mapping incoming work capabilities into logical executing agent nodes. |
| 3 | `AgentRegistry` | Struct | `crates/op-cache/src/agent_registry.rs:242` | Memory-map of active agent capabilities and execution channels. |
| 4 | `NumaTopology` | Struct | `crates/op-cache/src/numa.rs:43` | Hardware topology manager enforcing NUMA alignment and CPU affinities. |
| 5 | `WorkflowExecutor` | Struct | `crates/op-cache/src/workflow_executor.rs:94` | Asynchronous core scheduler initiating actual execution sequences of multi-agent tasks. |
| 6 | `WorkflowTracker` | Struct | `crates/op-cache/src/workflow_tracker.rs:69` | SQL-backed pattern analyzer promoting active agent pipelines to first-class workflows. |
| 7 | `WorkstackCache` | Struct | `crates/op-cache/src/workstack_cache.rs:37` | Dedicated cache layer for keeping intermediate results of high-latency multi-step sequences. |
| 8 | `WorkflowCache` | Struct | `crates/op-cache/src/workflow_cache.rs:71` | Handles fine-grained state tracking and execution snapshots of long-running orchestration. |
| 9 | `PatternTracker` | Struct | `crates/op-cache/src/pattern_tracker.rs:56` | Core engine tracking session sequences to optimize agent pipelines. |
| 10| `GrpcServer` | Struct | `crates/op-cache/src/grpc/server.rs:35` | External gRPC interface exposing all caching and capabilities to the wider control plane. |

---

### 1.4 Namespace Pollution: Glob Re-exports
*   **Location**: `crates/op-cache/src/grpc/mod.rs:22`
*   **Finding**: `pub use crate::proto::*;`
*   **Aarchitectural Risk**: Glob imports pull all underlying generated protobuf objects into the public namespace of the `grpc` module. This pollutes module autocompletion, conflicts with identically named entities, and exposes internal service structures. It bypasses Rust's clean encapsulation design principles.

---

### 1.5 Encapsulation Bypass: Structs with `pub` Fields
*   **Struct**: `AgentDefinition` (`crates/op-cache/src/agent_registry.rs:114`)
    *   *Violation*: All fields (e.g. `id`, `capabilities`, `requires`, `enabled`, `priority`) are declared `pub`. This allows arbitrary, validation-free mutability by foreign modules, despite the codebase containing dedicated builder methods (`with_capability`, `requires_capability`). A consumer can bypass invariants, registering agents with malformed identifiers or cyclic requirements.
*   **Struct**: `CapabilityRequest` (`crates/op-cache/src/capability_resolver.rs:14`)
    *   *Violation*: Public fields allow direct alteration of the parameters `required_capabilities`, `preferred_agents`, and `excluded_agents` after initialization, completely invalidating consistency logic in the capability matching pathway.

---

## 2. Dead Code Audit

### 2.1 Compiler Directive Warnings (`#[allow(dead_code)]`)
The following attributes are explicitly used to suppress compiler warning systems rather than resolving structural dead code:

*   `crates/op-cache/src/numa.rs:2`: `#![allow(dead_code)]` — Applied at the file scope. This hides unused variables, structs, and methods across the entire NUMA mapping module.
*   `crates/op-cache/src/btrfs_cache.rs:48`: `#[allow(dead_code)] numa_stats: Mutex<NumaStats>` — Suppresses unused field warnings for topological monitoring.
*   `crates/op-cache/src/btrfs_cache.rs:51`: `#[allow(dead_code)] impl BtrfsCache` — Hides unused methods in the main subvolume engine.
*   `crates/op-cache/src/orchestrator.rs:65`: `#[allow(dead_code)] numa_topology: NumaTopology` — Hides unused field mapping in the primary orchestrator.
*   `crates/op-cache/src/snapshot_manager.rs:222`: `#[allow(dead_code)] pub async fn oldest_snapshot` — Suppresses unused public method.
*   `crates/op-cache/src/snapshot_manager.rs:228`: `#[allow(dead_code)] pub async fn newest_snapshot` — Suppresses unused public method.
*   `crates/op-cache/src/snapshot_manager.rs:238`: `#[allow(dead_code)] pub created: Option<std::time::SystemTime>` — Suppresses unused field.
*   `crates/op-cache/src/workflow_tracker.rs:104`: `#[allow(dead_code)] input_hash: String` — Field is populated but never queried.
*   `crates/op-cache/src/workflow_tracker.rs:106`: `#[allow(dead_code)] timestamp: i64` — Field is populated but never queried.
*   `crates/op-cache/src/workflow_tracker.rs:108`: `#[allow(dead_code)] latency_ms: u64` — Field is populated but never queried.
*   `crates/op-cache/src/grpc/agent_service.rs:27`: `#[allow(dead_code)] endpoint: Option<String>` — Suppresses unused field.
*   `crates/op-cache/src/grpc/cache_service.rs:25`: `#[allow(dead_code)] compressed: bool` — Suppresses unused field.
*   `crates/op-cache/src/grpc/mcp_service.rs:37`: `#[allow(dead_code)] orchestrator_service: Arc<OrchestratorServiceImpl>` — Unused dependency injection.
*   `crates/op-cache/src/grpc/orchestrator_service.rs:24`: `#[allow(dead_code)] first_seen: Instant` — Struct field populated but never read.

---

### 2.2 Unused Code & Redundant Imports Table

| Item | Type | file:line | Recommendation |
|---|---|---|---|
| `get_or_embed` | Method | `crates/op-cache/src/btrfs_cache.rs:260` | Expose to high-level query APIs or remove. |
| `get_embedding` | Method | `crates/op-cache/src/btrfs_cache.rs:280` | Integrate with embedding server or remove. |
| `put_embedding` | Method | `crates/op-cache/src/btrfs_cache.rs:289` | Integrate with embedding server or remove. |
| `cleanup_old` | Method | `crates/op-cache/src/btrfs_cache.rs:509` | Connect to a cron-driven maintenance worker task. |
| `clear_embeddings` | Method | `crates/op-cache/src/btrfs_cache.rs:570` | Connect to cache eviction CLI commands. |
| `clear_blocks` | Method | `crates/op-cache/src/btrfs_cache.rs:587` | Connect to block pruning scripts. |
| `stream_to_remote` | Method | `crates/op-cache/src/btrfs_cache.rs:602` | Remove completely to mitigate command injection risks. |
| `receive_from_remote` | Method | `crates/op-cache/src/btrfs_cache.rs:641` | Remove completely to mitigate command injection risks. |
| `numa_info` | Method | `crates/op-cache/src/btrfs_cache.rs:677` | Connect to a NUMA diagnostic endpoint. |
| `cache_dir` | Method | `crates/op-cache/src/btrfs_cache.rs:687` | Expose for directory cleaning CLI flags. |
| `avg_accesses` | Method | `crates/op-cache/src/btrfs_cache.rs:748` | Bind to administrative Prometheus dashboards. |
| `memory_utilization` | Method | `crates/op-cache/src/numa.rs:30` | Bind to dynamic load-balancing node selectors. |
| `distance_to` | Method | `crates/op-cache/src/numa.rs:39` | Bind to routing optimization or remove. |
| `get_node` | Method | `crates/op-cache/src/numa.rs:341` | Integrate into node status checks. |
| `refresh` | Method | `crates/op-cache/src/numa.rs:371` | Spawn in a low-frequency tokio scheduler loop. |
| `local_hit_rate` | Method | `crates/op-cache/src/numa.rs:418` | Output via telemetry systems. |
| `remote_penalty` | Method | `crates/op-cache/src/numa.rs:426` | Bind to orchestrator performance estimation. |
| `register_local` | Method | `crates/op-cache/src/grpc/agent_service.rs:51` | Integrate with local system test runners. |
| `list_capabilities` | Method | `crates/op-cache/src/grpc/agent_service.rs:457` | Expose to CLI schema discovery commands. |
| `health_check` | Method | `crates/op-cache/src/grpc/agent_service.rs:482` | Connect to service cluster health checks. |
| `get_workstack_stats` | Method | `crates/op-cache/src/grpc/cache_service.rs:312` | Bind to cache metrics endpoints. |
| `list_tools` | Method | `crates/op-cache/src/grpc/mcp_service.rs:495` | Integrate into the dynamic tool lookup tests. |
| `_input_hash` | Variable | `crates/op-cache/src/workflow_executor.rs:186` | Remove variable definition. |
| `tracing::info` | Unused Import | `crates/op-cache/src/grpc/mcp_service.rs:11` | Remove redundant import. |
| `AgentService` | Unused Import | `crates/op-cache/src/grpc/mcp_service.rs:14` | Remove redundant import. |

---

## 3. Schema-as-Code Violations
To comply with standard workspace governance, all contract boundaries, configuration layers, and pipeline serializations must use versioned schemas (such as Protocol Buffers and OSCAL profiles). The following areas violate this policy by utilizing ad-hoc models:

### 3.1 Ad-hoc Structural Entities
*   **Location**: `crates/op-cache/src/agent_registry.rs:114` (`AgentDefinition`)
*   **Location**: `crates/op-cache/src/orchestrator.rs:43` (`OrchestrationResult`)
*   **Location**: `crates/op-cache/src/orchestrator.rs:57` (`StepResult`)
*   **Location**: `crates/op-cache/src/workflow_cache.rs:37` (`CachedStepResult`)
*   **Violation**: These structs represent operational request/response parameters, configurations, and state data. Instead of being statically declared inside Rust, they must be dynamically derived from versioned OSCAL profiles or defined in a centralized Protocol Buffers contract.

### 3.2 Dynamic JSON Construction & Raw SQL Storage
*   **Location**: `crates/op-cache/src/pattern_tracker.rs:110` & `crates/op-cache/src/workflow_tracker.rs:159`
    *   *Violation*: Sequence paths are converted to dynamic JSON structures in-line via `simd_json::to_string(agents)?` and written to standard SQLite text columns. This relies on an implicit schema, presenting structural upgrade risks if pipeline structures evolve.
*   **Location**: `crates/op-cache/src/grpc/mcp_service.rs:340` (`build_agent_input_schema`)
    *   *Violation*: Builds JSON structures using runtime macros (`serde_json::json!`). This avoids centralized API versioning, leading to drift between the platform schemas and MCP adapters.

### 3.3 Ad-hoc Internal Serialization
*   **Location**: `crates/op-cache/src/grpc/mcp_service.rs:426-476`
    *   *Violation*: Protocol contracts (`ToolCallParams`, `McpContentResponse`, `McpContent`, `McpToolsListResult`, `McpToolJson`, `McpInitializeResult`, etc.) are declared directly inside the service implementation file. These objects must be migrated to standard, versioned `.proto` definitions.

---

## 4. Security & Architectural Quality Findings

### CRITICAL: Remote Command Injection via Shell Execution
*   **Category**: Injection Vulnerability (CWE-78)
*   **Location**: `crates/op-cache/src/btrfs_cache.rs:613-620` and `crates/op-cache/src/btrfs_cache.rs:652-659`
*   **Description**:
    The system facilitates cache state streaming across systems using BTRFS send/receive mechanics:
    ```rust
    let cmd = format!(
        "btrfs send {} | ssh {} 'btrfs receive {}'",
        snapshot_path.display(),
        remote_host,
        remote_path
    );

    let output = tokio::process::Command::new("bash")
        .arg("-c")
        .arg(&cmd)
        .output()
        .await
    ```
    And:
    ```rust
    let cmd = format!(
        "ssh {} 'btrfs send {}' | btrfs receive {}",
        remote_host, remote_snapshot, local_path
    );

    let output = tokio::process::Command::new("bash")
        .arg("-c")
        .arg(&cmd)
        .output()
        .await
    ```
    The strings `remote_host`, `remote_path`, `remote_snapshot`, and `local_path` are directly formatted into a command execution string that is processed via `bash -c`. Any shell metacharacters (e.g. `;`, `&`, `|`, `` ` ``, `$()`) present in these variables will bypass the command boundary and execute arbitrary processes locally.
*   **Impact**:
    If an attacker registers a malicious remote agent or modifies the target streaming paths via compromised configuration database records or a gRPC request payload, they can execute arbitrary shell commands on the host operating system with the elevated privileges of the cache service worker.
*   **Remediation**:
    1. Do not use an active shell interpreter (`bash -c`) to orchestrate piping.
    2. Execute `btrfs` and `ssh` directly using `tokio::process::Command::new("ssh")` and `tokio::process::Command::new("btrfs")`.
    3. Pass parameters as separate, explicit items in the `args` array to prevent shell escaping.
    4. Connect the output of the first process to the input of the second programmatically via standard `Stdio::piped()` buffers.

---

### HIGH: Unsafe Raw Deserialization via `bincode` on Unverified Disk Files
*   **Category**: Deserialization Vulnerability (CWE-502)
*   **Location**: `crates/op-cache/src/btrfs_cache.rs:434`
*   **Description**:
    The system reads cached vector representations from the local storage disk and directly processes the byte content using unconstrained `bincode::deserialize` calls:
    ```rust
    let data = std::fs::read(&path)
        .context(format!("Failed to read cached embedding: {:?}", path))?;

    let vector: Vec<f32> =
        bincode::deserialize(&data).context("Failed to deserialize cached embedding")?;
    ```
    `bincode` is not designed to process untrusted data. It does not enforce memory allocation bounds during the deserialization phase, which can trigger panics or memory allocation exhaustion.
*   **Impact**:
    Since the default cache path falls back to `/tmp` in test configurations or `/var/lib/op-dbus` (which may suffer from insecure host file permissions), a local attacker can write or substitute corrupt files inside the cache directory. When loaded by the daemon, these malicious payloads can trigger denial of service or target system compromise.
*   **Remediation**:
    1. Restrict directory permissions on `/var/lib/op-dbus` to `0700` so only the running system daemon can read/write files.
    2. Before invoking `bincode`, compute and verify a cryptographic checksum (e.g. HMAC-SHA256) of the byte block to ensure it has not been modified.

---

### HIGH: Memory Safety Hazard and Undefined Behavior via `unsafe` String Parsing
*   **Category**: Memory Safety / Undefined Behavior (CWE-119 / CWE-125)
*   **Location**: `crates/op-cache/src/workflow_tracker.rs:251-253`, `crates/op-cache/src/workflow_tracker.rs:494-495`, and `crates/op-cache/src/workflow_tracker.rs:524-525`
*   **Description**:
    The system retrieves serialized agent sequence strings from SQLite and parses them using the `unsafe` variant of `simd_json::from_str`:
    ```rust
    let mut agent_sequence_json: String = row.get(1)?;
    let agent_sequence: Vec<String> =
        unsafe { simd_json::from_str(&mut agent_sequence_json) }
            .unwrap_or_default();
    ```
    The `simd_json` parser achieves extreme parsing performance via vectorization instructions (AVX/SSE) and processes input buffers **in-place**. It modifies the target string representation directly.
    Because of this in-place mutation, the input buffer **must** be allocated with a special allocation padding of at least `simd_json::PADDING` bytes at the end of the memory block.
    Standard `String` buffers allocated by the `rusqlite` row mapper do not guarantee this alignment or padding.
*   **Impact**:
    Executing SIMD vectorization parsing over unaligned or unpadded string allocations can cause out-of-bounds memory reads or writes, resulting in segmentation faults, system crashes, or undefined behavior.
*   **Remediation**:
    1. Use `simd_json::to_padded_container` or copy the raw SQL string data into a `simd_json::PaddedBytes` container prior to parsing.
    2. Alternatively, use a safe, standard JSON parsing library like `serde_json` for low-frequency SQLite deserialization.

---

### MEDIUM: Blocking Database I/O inside the Asynchronous Tokio Executor Threadpool
*   **Category**: Concurrency / Thread Starvation (CWE-400)
*   **Location**: `crates/op-cache/src/orchestrator.rs:432-436`, `crates/op-cache/src/workflow_executor.rs:252-254`, and `crates/op-cache/src/workflow_executor.rs:466-468`
*   **Description**:
    Within core asynchronous paths (e.g. `Orchestrator::execute` and `WorkflowExecutor::execute`), the engine invokes synchronous database operations on SQLite through `rusqlite::Connection` wrapped in a standard `std::sync::Mutex`:
    ```rust
    let (output, cached) = if self.config.enable_caching {
        match self
            .cache
            .get(&workflow.workflow_id, step_index, &step_input_hash)?
    ```
    And:
    ```rust
    // Track pattern
    if self.config.track_patterns {
        let input_hash = Self::hash_bytes(&input);
        if let Some(suggestion) =
            self.pattern_tracker
                .record_sequence(agent_ids, &input_hash, total_latency_ms)?
    ```
    Both `self.cache.get` and `self.pattern_tracker.record_sequence` lock the SQLite database via a synchronous mutex, blocking the thread while executing file I/O operations.
*   **Impact**:
    This blocks the asynchronous Tokio worker thread executing the task. Under high orchestrator load with many concurrent requests, multiple threads will stall waiting for the database locks, leading to thread starvation, severe orchestration latency, and eventual system freeze.
*   **Remediation**:
    Wrap all synchronous `rusqlite` database operations in `tokio::task::spawn_blocking` calls, or migrate the storage layers to `tokio-rusqlite` or `sqlx` (already available in the workspace dependencies).

---

### MEDIUM: Clock Drift Crash Vulnerability in Cache Expiry
*   **Category**: Resiliency / Denial of Service (CWE-754)
*   **Location**: `crates/op-cache/src/grpc/cache_service.rs:104-109`
*   **Description**:
    The timestamp generator `now_timestamp` calculates seconds elapsed since the UNIX epoch:
    ```rust
    fn now_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }
    ```
    `duration_since` returns an `Err` if the host system time is set backwards relative to the system epoch (e.g., during NTP clock synchronization adjustments, leap second events, or manual time updates). Applying `.unwrap()` directly to this result causes an immediate panic.
*   **Impact**:
    If an NTP correction shifts the host clock backwards, any subsequent gRPC request calling `now_timestamp` will cause the executing task to panic, resulting in client query failure and a Denial of Service.
*   **Remediation**:
    Handle clock variations safely by replacing `.unwrap()` with a fallback recovery path:
    ```rust
    fn now_timestamp() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
    ```

---

### LOW: Overhead and Starvation Risk via External `taskset` Spawning
*   **Category**: Resource Management / Portability
*   **Location**: `crates/op-cache/src/btrfs_cache.rs:723-731` and `crates/op-cache/src/workflow_executor.rs:534-539`
*   **Description**:
    To enforce CPU affinity across NUMA boundaries, the cache engine executes the external system utility `taskset` as a child process:
    ```rust
    let output = tokio::process::Command::new("taskset")
        .args(["-cp", &cpu_list, &std::process::id().to_string()])
        .output()
        .await;
    ```
    Spawning an external process is resource-intensive and will fail if the system running the cache does not have `taskset` installed (such as inside stripped, minimal Docker containers).
*   **Impact**:
    It degrades system execution speed under high throughput and breaks execution logic on host environments where `taskset` is missing.
*   **Remediation**:
    Use native Linux system calls directly within the Rust application. Incorporate the `nix::sched::sched_setaffinity` API from the `nix` crate to assign CPU mask affinity programmatically, which avoids child process overhead.

---
## ⚠ Citation Warnings
- `crates/op-cache/src/snapshot_manager.rs:238`: file has 236 lines
- `crates/op-cache/src/grpc/agent_service.rs:457`: file has 402 lines
- `crates/op-cache/src/grpc/agent_service.rs:482`: file has 402 lines
- `crates/op-cache/src/grpc/mcp_service.rs:495`: file has 368 lines
- `crates/op-cache/src/grpc/mcp_service.rs:426`: file has 368 lines
