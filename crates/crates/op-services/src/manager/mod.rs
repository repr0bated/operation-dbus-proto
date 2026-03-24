//! Service manager core

mod dinit_proxy;
mod process;
mod service_manager;

pub use dinit_proxy::*;
pub use process::*;
pub use service_manager::*;
