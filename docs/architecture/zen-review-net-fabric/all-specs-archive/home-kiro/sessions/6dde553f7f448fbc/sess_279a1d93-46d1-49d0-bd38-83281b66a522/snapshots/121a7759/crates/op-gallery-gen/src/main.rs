//! Model-agnostic generative UI gallery system.
//!
//! Replaces op-gemma generators with an inference loop that reads sealed plugin blobs,
//! accepts operator guidance via Antigravity chat UI, and generates json-render.dev specs.

use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

use op_gallery_gen::{
    GalleryGenConfig,
    context::GenerationContext,
    inference::InferenceLoop,
    validator::SpecValidator,
    admission::GalleryAdmission,
};

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
        
        /// ZeroClaw HTTP endpoint
        #[arg(long, default_value = "http://localhost:8082")]
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
        Commands::Generate { target, endpoint, mcp, qdrant, guidance } => {
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
    
    let mut ctx = GenerationContext::new(schemas);
    ctx.mcp_enabled = mcp;
    ctx.qdrant_enabled = qdrant;
    ctx.operator_guidance = guidance;
    
    let loop_ = InferenceLoop::new(config.zeroclaw_endpoint, config.max_turns);
    let validator = SpecValidator::new();
    let admission = GalleryAdmission::new(config.target_count, config.stable_core_max);
    
    let mut generated = 0;
    let mut attempts = 0;
    let max_attempts = target * 3; // Allow 3x attempts
    
    while generated < target && attempts < max_attempts {
        attempts += 1;
        tracing::info!("Generation attempt {} of {}", attempts, max_attempts);
        
        match loop_.generate(&ctx).await {
            Ok(gen_spec) => {
                let validation = validator.validate(&gen_spec.spec);
                
                if validation.valid {
                    let admit_result = admission.admit(&validation);
                    
                    if admit_result.admitted {
                        generated += 1;
                        tracing::info!("Successfully admitted spec {} of {}", generated, target);
                    } else if let Some(reason) = admit_result.rejection_reason {
                        tracing::warn!("Spec rejected: {}", reason);
                    }
                } else {
                    tracing::warn!("Spec validation failed: {:?}", validation.errors);
                }
            }
            Err(e) => {
                tracing::error!("Generation failed: {}", e);
            }
        }
    }
    
    tracing::info!("Generation complete: {} specs generated in {} attempts", generated, attempts);
    Ok(())
}

fn run_validation(file: &str) -> Result<()> {
    use std::fs;
    
    let content = fs::read_to_string(file)?;
    let spec: serde_json::Value = serde_json::from_str(&content)?;
    
    let validator = SpecValidator::new();
    let result = validator.validate(&spec);
    
    if result.valid {
        println!("✓ Spec is valid");
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
    let admission = GalleryAdmission::default();
    let stats = admission.stats();
    
    println!("Gallery Statistics:");
    println!("  Max size: {}", stats.max_size);
    println!("  Stable core max: {}", stats.stable_core_max);
    println!("  Current size: {}", stats.current_size);
    println!("  Available slots: {}", stats.available_slots);
}
