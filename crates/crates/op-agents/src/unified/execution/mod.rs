//! Execution Agents
//!
//! Agents that can execute code and commands with sandboxing.
//! These have SecurityProfiles and process management.

mod base;
mod python;
mod rust;
mod javascript;
mod golang;
mod shell;

pub use base::ExecutionAgent;
pub use python::PythonExecutor;
pub use rust::RustExecutor;
pub use javascript::JavaScriptExecutor;
pub use golang::GoExecutor;
pub use shell::ShellExecutor;

use std::collections::HashMap;
use once_cell::sync::Lazy;

/// All available execution agents
pub static EXECUTION_AGENTS: Lazy<HashMap<&'static str, fn() -> Box<dyn super::UnifiedAgent>>> = Lazy::new(|| {
    let mut m: HashMap<&'static str, fn() -> Box<dyn super::UnifiedAgent>> = HashMap::new();
    m.insert("python-executor", || Box::new(PythonExecutor::new()));
    m.insert("rust-executor", || Box::new(RustExecutor::new()));
    m.insert("javascript-executor", || Box::new(JavaScriptExecutor::new()));
    m.insert("go-executor", || Box::new(GoExecutor::new()));
    m.insert("shell-executor", || Box::new(ShellExecutor::new()));
    m
});
