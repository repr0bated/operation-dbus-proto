//! op-services: System-wide service manager (systemd replacement)

pub mod dbus;
pub mod grpc;
pub mod manager;
pub mod schema;
pub mod store;

pub use manager::{ProcessManager, ServiceEvent, ServiceManager};
pub use schema::{
    ActiveState, DesiredState, ExecCommand, LogType, ManagerState, ReadyNotification,
    ResourceLimits, RestartCondition, RestartPolicy, ServiceDef, ServiceName, ServiceState,
    ServiceStatus, ServiceType, SystemdPlugin, ValidationError,
};
pub use store::Store;
