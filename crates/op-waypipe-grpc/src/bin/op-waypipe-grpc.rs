//! CLI for the waypipe ↔ gRPC tunnel (Identity Sled auth).

use std::path::PathBuf;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use tracing::info;

use op_waypipe_grpc::{launch, serve, LaunchOpts, ServeOpts, TunnelConfig};

#[derive(Parser, Debug)]
#[command(
    name = "op-waypipe-grpc",
    about = "Waypipe over gRPC (SHM identity sled auth)"
)]
struct Cli {
    /// Optional JSON config. If omitted: `$OP_WAYPIPE_GRPC_CONFIG`,
    /// `~/.config/op-waypipe-grpc/config.json`, then embedded defaults.
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Listen for Tunnel RPCs (run on the remote host).
    Serve,
    /// Launch a remote command through waypipe (run on the laptop).
    Launch {
        /// gRPC host:port (e.g. 100.69.0.254:50052).
        #[arg(long)]
        endpoint: String,
        /// Override --compress (default from config).
        #[arg(long)]
        compress: Option<String>,
        /// Remote argv after `--`.
        #[arg(required = true, last = true)]
        command: Vec<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let (config, source) = TunnelConfig::load_resolved(cli.config.as_deref())?;
    info!(config = %source, "loaded config");

    match cli.cmd {
        Cmd::Serve => serve(ServeOpts { config }).await,
        Cmd::Launch {
            endpoint,
            compress,
            command,
        } => {
            if command.is_empty() {
                bail!("command required after --");
            }
            launch(LaunchOpts {
                config,
                grpc_endpoint: endpoint,
                command,
                compress,
                client_socket: None,
            })
            .await
        }
    }
}
