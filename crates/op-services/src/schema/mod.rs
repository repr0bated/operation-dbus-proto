//! Service Manager Schemas
//!
//! Re-exports service definition types from op-plugins

pub use op_plugins::service_def::{
    ActiveState, DesiredState, ExecCommand, LogType, ManagerState, ReadyNotification,
    ResourceLimits, RestartCondition, RestartPolicy, ServiceDef, ServiceName, ServiceState,
    ServiceStatus, ServiceType, SystemdPlugin, ValidationError,
};
