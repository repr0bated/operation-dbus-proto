//! Model-agnostic generative UI gallery system.
//!
//! Replaces op-gemma generators with an inference loop that reads sealed plugin blobs,
//! accepts operator guidance via Antigravity chat UI, and generates json-render.dev specs.

use std::sync::Arc;

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use op_gallery_gen::{context::GenerationContext, validator::SpecValidator, GalleryGenConfig};

#[derive(Parser)]
#[command(name = "op-gallery-gen")]
#[command(about = "Model-agnostic generative UI gallery system")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate UI specs for the gallery
    Generate {
        /// Target number of specs to generate
        #[arg(short, long, default_value = "200")]
        target: usize,

        /// OpenAI-compatible chat endpoint (op-web :8080)
        #[arg(long, default_value = "http://127.0.0.1:8080")]
        endpoint: String,

        /// Enable MCP tool layer
        #[arg(long)]
        mcp: bool,

        /// Enable Qdrant semantic search
        #[arg(long)]
        qdrant: bool,

        /// Operator guidance text
        #[arg(short, long)]
        guidance: Option<String>,
    },

    /// Validate a spec file
    Validate {
        /// Path to spec JSON file
        #[arg(short, long)]
        file: String,
    },

    /// Show gallery statistics
    Stats,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Generate {
            target,
            endpoint,
            mcp,
            qdrant,
            guidance,
        } => {
            run_generation(target, endpoint, mcp, qdrant, guidance).await?;
        }
        Commands::Validate { file } => {
            run_validation(&file)?;
        }
        Commands::Stats => {
            run_stats();
        }
    }

    Ok(())
}

async fn run_generation(
    target: usize,
    endpoint: String,
    mcp: bool,
    qdrant: bool,
    guidance: Option<String>,
) -> Result<()> {
    tracing::info!("Starting gallery generation with target: {}", target);

    let config = GalleryGenConfig {
        target_count: target,
        zeroclaw_endpoint: endpoint.clone(),
        enable_mcp: mcp,
        enable_qdrant: qdrant,
        ..Default::default()
    };

    // TODO: Load plugin schemas from blobs
    let schemas = vec![];

    let mut ctx = GenerationContext::new(
        schemas,
        String::new(),
        config.load_catalog()?.prompt().to_string(),
    );
    ctx.mcp_enabled = mcp;
    ctx.qdrant_enabled = qdrant;
    ctx.operator_guidance = guidance;

    let store: Arc<dyn op_gallery_gen::GalleryStore> =
        Arc::new(op_gallery_gen::InMemoryGalleryStore::new());
    let progress = Arc::new(op_gallery_gen::RunProgress::new());
    progress.reset(target);

    let generated = op_gallery_gen::run_gallery_fill(&config, ctx, store, progress).await?;

    tracing::info!("Generation complete: {} specs generated", generated);
    Ok(())
}

fn run_validation(file: &str) -> Result<()> {
    use std::fs;

    let content = fs::read_to_string(file)?;
    let spec: serde_json::Value = serde_json::from_str(&content)?;

    // Fail rather than fall back to grammar-only: "valid" without a vocabulary
    // says nothing about whether the renderer would accept this spec.
    let catalog = GalleryGenConfig::default().load_catalog()?;
    let validator = SpecValidator::with_catalog(catalog);
    let result = validator.validate(&spec);

    if result.valid {
        println!("✓ Spec is valid");
        println!("Catalog:   {}", validator.catalog_hash().unwrap_or("none"));
        println!("Signature: {}", result.signature);
    } else {
        println!("✗ Spec validation failed:");
        for error in &result.errors {
            println!("  [{}] {}", error.code, error.message);
        }
    }

    Ok(())
}

fn run_stats() {
    let store = op_gallery_gen::InMemoryGalleryStore::new();
    let stats = op_gallery_gen::GalleryStore::stats(&store);

    println!("Gallery Statistics:");
    println!("  Max size: {}", stats.max_size);
    println!("  Stable core max: {}", stats.stable_core_max);
    println!("  Current size: {}", stats.current_size);
    println!("  Stable core: {}", stats.stable_core_count);
    println!("  Novelty: {}", stats.novelty_count);
    println!("  Available slots: {}", stats.available_slots);
}
