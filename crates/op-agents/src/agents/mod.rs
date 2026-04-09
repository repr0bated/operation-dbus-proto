#![allow(dead_code)]
//! Agent implementations organized by category
//!
//! Categories:
//! - orchestration: Meta-agents that coordinate others
//! - operations: SRE and operations agents (incident response, troubleshooting)
//! - aiml: AI/ML specialized agents (ai-engineer, ml-engineer, etc.)
//! - persona: Dynamic configuration-driven agents

pub mod aiml;
pub mod base;
pub mod operations;
pub mod orchestration;
pub mod persona;

// Re-export common types
pub use base::{AgentContext, AgentTask, AgentTrait, TaskResult};
