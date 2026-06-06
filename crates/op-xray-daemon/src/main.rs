//! Xray D-Bus Daemon
//!
//! Provides D-Bus interfaces for managing the Xray proxy daemon lifecycle.
//! Replaces direct subprocess spawning with a schema-driven D-Bus service.
//!
//! D-Bus object path: /opdbus/v1/xray
//! Interface: org.opdbus.v1.Xray
//!
//! ## Architecture
//!
//! Per AGENTS.md D-Bus-first rules:
//! - All xray control operations go through D-Bus methods
//! - Config path is /dev/shm/xray_config.json per AGENTS.md §4a (Zero-Btrfs)
//! - No direct Command::new("xray") spawning - use D-Bus service instead
//!
//! ## Methods
//! - start(config_path) -> (bool, String)
//! - stop() -> (bool, String)
//! - restart(config_path) -> (bool, String)
//! - status() -> JSON string
//! - reload() -> (bool, String)
//! - get_config() -> String

use anyhow::{Context, Result};
use tokio::signal;
use tracing::info;
use zbus::connection::Connection;

mod dbus;
use dbus::XrayService;

/// CLI arguments
#[derive(Debug, Clone)]
struct Args {
    /// Initial config path to use on startup
    config_path: Option<String>,
}

impl Args {
    fn parse() -> Self {
        let args: Vec<String> = std::env::args().collect();
        let mut config_path = None;

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--config" => {
                    if i + 1 < args.len() {
                        config_path = Some(args[i + 1].clone());
                        i += 2;
                    } else {
                        eprintln!("Error: --config requires an argument");
                        std::process::exit(1);
                    }
                }
                "-h" | "--help" => {
                    println!("Xray D-Bus Daemon - org.opdbus.v1.Xray");
                    println!();
                    println!("Usage: op-xray-daemon [OPTIONS]");
                    println!();
                    println!("Options:");
                    println!("  --config <path>    Initial config path (default: /dev/shm/xray_config.json)");
                    println!("  -h, --help         Show this help message");
                    std::process::exit(0);
                }
                _ => {
                    eprintln!("Warning: Unknown argument: {}", args[i]);
                    i += 1;
                }
            }
        }

        Self { config_path }
    }
}

/// Default config path per AGENTS.md §4a (Zero-Btrfs)
const DEFAULT_CONFIG_PATH: &str = "/dev/shm/xray_config.json";

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging with env filter
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting op-xray-daemon");

    // Parse CLI arguments
    let args = Args::parse();

    // Determine config path
    let config_path = args
        .config_path
        .unwrap_or_else(|| DEFAULT_CONFIG_PATH.to_string());
    info!("Using config path: {}", config_path);

    // Create D-Bus service
    let service = XrayService::new();

    // Build D-Bus connection
    let conn = Connection::system()
        .await
        .context("Failed to connect to system D-Bus")?;

    info!("Connected to system D-Bus");

    // Serve the XrayService on the connection
    conn.object_server()
        .at("/opdbus/v1/xray", service)
        .await
        .context("Failed to register D-Bus object")?;

    // Request the bus name
    conn.request_name("opdbus.v1")
        .await
        .context("Failed to request bus name")?;

    info!("D-Bus service registered at /opdbus/v1/xray");
    info!("Interface: org.opdbus.v1.Xray");

    // Set up signal handlers for graceful shutdown
    let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
        .context("Failed to register SIGTERM handler")?;
    let mut sigint = signal::unix::signal(signal::unix::SignalKind::interrupt())
        .context("Failed to register SIGINT handler")?;

    info!("Daemon ready - waiting for requests");

    // If config was provided via CLI, auto-start xray
    if std::path::Path::new(&config_path).exists() {
        info!("Auto-starting xray with config: {}", config_path);
        // This would require access to the service instance, which is managed by zbus
        // For now, clients must call Start() explicitly via D-Bus
        info!("Note: Auto-start not implemented - use D-Bus Start() method");
    }

    // Wait for shutdown signal
    tokio::select! {
        _ = sigterm.recv() => {
            info!("Received SIGTERM, shutting down gracefully...");
        }
        _ = sigint.recv() => {
            info!("Received SIGINT, shutting down gracefully...");
        }
    }

    info!("op-xray-daemon shutting down");
    Ok(())
}
