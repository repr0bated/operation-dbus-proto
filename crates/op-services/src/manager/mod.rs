//! Service manager core

mod process;
mod service_manager;

pub use process::ProcessManager;
pub use service_manager::{ServiceEvent, ServiceManager};
