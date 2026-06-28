//! s6-systemctl D-Bus Daemon
//!
//! Provides a D-Bus interface that maps systemctl commands to s6/s6-rc
//! operations for Artix Linux systems using s6 as the init system.
//!
//! D-Bus object path: /org/opdbus/v1/s6/systemctl
//! Interface: org.opdbus.v1.S6.Systemctl
//!
//! ## Mapping Reference
//!
//! | systemctl command | s6 equivalent |
//! |-------------------|---------------|
//! | start <svc>       | s6-rc -u change <svc> |
//! | stop <svc>        | s6-rc -d change <svc> |
//! | restart <svc>     | stop + start |
//! | reload <svc>      | s6-svc -h <svc> |
//! | enable <svc>      | s6-rc-bundle add <svc> |
//! | disable <svc>     | s6-rc-bundle delete <svc> |
//! | status <svc>      | s6-svstat <svc> |
//! | list-units        | s6-rc -a list |
//!
//! ## Architecture
//!
//! Per AGENTS.md D-Bus-first rules:
//! - All service operations go through D-Bus methods
//! - No direct subprocess spawning from clients
//! - Schema-driven interface with helpful error messages

use anyhow::{Context, Result};
use tracing::{info, warn};
use zbus::connection::Connection;

mod dbus;
use dbus::S6SystemctlService;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting op-s6-systemctl daemon");

    // Create the D-Bus service
    let service = S6SystemctlService::new();

    // Build D-Bus connection
    let conn = Connection::system()
        .await
        .context("Failed to connect to system D-Bus")?;

    info!("Connected to system D-Bus");

    // Register the service on the connection
    conn.object_server()
        .at("/org/opdbus/v1/s6/systemctl", service)
        .await
        .context("Failed to register D-Bus object")?;

    // Request the bus name
    conn.request_name("org.opdbus.v1.S6.Systemctl")
        .await
        .context("Failed to request bus name")?;

    info!("D-Bus service registered at /org/opdbus/v1/s6/systemctl");
    info!("Interface: org.opdbus.v1.S6.Systemctl");
    info!("Daemon ready - press Ctrl+C to stop");

    // Check if s6 tools are available
    match tokio::process::Command::new("s6-svscan")
        .arg("--help")
        .output()
        .await
    {
        Ok(_) => info!("s6 tools detected - service operational"),
        Err(_) => warn!(
            "s6 tools not detected. Service will return helpful error messages. \
             Ensure s6 is installed and in PATH."
        ),
    }

    // Keep the connection alive
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}
