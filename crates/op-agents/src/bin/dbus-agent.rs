//! D-Bus Agent Launcher
//!
//! Universal binary to run any agent type as a D-Bus service.
//!
//! # Usage
//!
//! ```bash
//! # Run an agent on the session bus
//! dbus-agent python-pro
//!
//! # Run with a custom agent ID
//! dbus-agent python-pro my-python-agent
//!
//! # Run on the system bus (requires privileges)
//! dbus-agent --system rust-pro
//!
//! # List available agent types
//! dbus-agent --list
//! ```
//!
//! # D-Bus Registration
//!
//! The agent will be registered as:
//! - Service name: `org.dbusmcp.Agent.{AgentType}` (e.g., `org.dbusmcp.Agent.PythonPro`)
//! - Object path: `/org/dbusmcp/Agent/{AgentType}`
//! - Interface: `org.dbusmcp.Agent`
//!
//! # Discovery
//!
//! Once running, the agent can be discovered by the ChatActor's tool_loader
//! and registered as a tool that the LLM can call.

use std::env;

use op_core::BusType;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

use op_agents::agent_catalog::builtin_agent_descriptors;
use op_agents::dbus_service::start_agent;
use op_agents::{create_agent, list_agent_types};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("op_agents=info,dbus_agent=info"));

    tracing_subscriber::fmt().with_env_filter(filter).init();

    // Parse arguments
    let mut args: Vec<String> = env::args().collect();
    let program = args.remove(0);

    let mut bus_type = BusType::Session;
    let mut list_only = false;
    let mut agent_type = None;
    let mut agent_id = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--system" => bus_type = BusType::System,
            "--session" => bus_type = BusType::Session,
            "--list" | "-l" => list_only = true,
            "--help" | "-h" => {
                print_usage(&program);
                return Ok(());
            }
            arg if arg.starts_with('-') => {
                warn!("Unknown option: {}", arg);
            }
            arg if agent_type.is_none() => {
                agent_type = Some(arg.to_string());
            }
            arg if agent_id.is_none() => {
                agent_id = Some(arg.to_string());
            }
            _ => warn!("Ignoring extra argument: {}", args[i]),
        }
        i += 1;
    }

    // Handle --list
    if list_only {
        println!("Available Agent Types:");
        println!("======================");

        // Print structural/legacy agents and dynamic personas
        let types = list_agent_types();
        for t in types {
            println!("  - {}", t);
        }

        println!("\nUse '{} <agent_type>' to start an agent.", program);
        return Ok(());
    }

    // Require agent type
    let agent_type = match agent_type {
        Some(t) => t,
        None => {
            error!("Agent type required.");
            print_usage(&program);
            std::process::exit(1);
        }
    };

    // Default agent ID to agent type if not provided
    let agent_id = agent_id.unwrap_or_else(|| agent_type.clone());

    info!(
        "Starting agent type '{}' with ID '{}' on {:?} bus",
        agent_type, agent_id, bus_type
    );

    // Create the agent instance
    let agent = match create_agent(&agent_type, agent_id.clone()) {
        Ok(a) => a,
        Err(e) => {
            error!("Failed to create agent: {}", e);
            std::process::exit(1);
        }
    };

    // Start D-Bus service
    let _connection = start_agent(agent, &agent_id, bus_type)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to start D-Bus service: {}", e))?;

    info!("Agent running. Waiting for D-Bus method calls...");

    // Wait forever
    std::future::pending::<()>().await;

    Ok(())
}

fn print_usage(program: &str) {
    println!("Usage: {} [OPTIONS] <agent_type> [agent_id]", program);
    println!();
    println!("Options:");
    println!("  --system     Connect to the system D-Bus");
    println!("  --session    Connect to the session D-Bus (default)");
    println!("  --list, -l   List available agent types");
    println!("  --help, -h   Show this help message");
    println!();
    println!("Examples:");
    println!(
        "  {} python-pro                 # Run python-pro agent on session bus",
        program
    );
    println!(
        "  {} --system network-engineer  # Run network agent on system bus",
        program
    );
}
