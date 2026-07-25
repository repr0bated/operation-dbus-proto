//! Orchestration Agents
//!
//! Meta-agents that coordinate other agents for complex workflows.

mod base;
mod tdd_orchestrator;
mod code_review_orchestrator;

pub use base::OrchestrationAgent;
pub use tdd_orchestrator::TddOrchestrator;
pub use code_review_orchestrator::CodeReviewOrchestrator;

use std::collections::HashMap;
use std::sync::LazyLock;

/// All available orchestration agents
pub static ORCHESTRATION_AGENTS: LazyLock<
    HashMap<&'static str, fn() -> Box<dyn super::UnifiedAgent>>,
> = LazyLock::new(|| {
    let mut m: HashMap<&'static str, fn() -> Box<dyn super::UnifiedAgent>> = HashMap::new();
    m.insert("tdd-orchestrator", || Box::new(TddOrchestrator::new()));
    m.insert("code-review-orchestrator", || Box::new(CodeReviewOrchestrator::new()));
    m
});
