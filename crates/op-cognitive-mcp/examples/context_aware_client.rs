//! Context-Aware MCP Client Example
//!
//! Demonstrates how to use the proactive knowledge push feature
//! of the cognitive-mcp server with SSE streaming.
//!
//! ## Features Demonstrated
//!
//! 1. **SSE Stream Connection** - Subscribe to `/context/stream/:session_id`
//! 2. **Activity Recording** - Report user activity for context tracking
//! 3. **Proactive Receives** - Handle knowledge pushes triggered by:
//!    - New topic detection
//!    - Stuck session patterns
//!    - Error assistance
//!    - Idle recovery
//! 4. **On-Demand Push** - Request knowledge when needed
//!
//! ## Running
//!
//! ```bash
//! cargo run --example context_aware_client -- --session-id my-session-1
//! ```

use clap::Parser;
use futures::StreamExt;
use reqwest::Client;
use serde_json::json;
use std::time::Duration;
use tokio::time::{interval, sleep};
use tracing::{error, info, warn};

const COGNITIVE_MCP_URL: &str = "http://100.90.37.254:3003";

#[derive(Parser)]
#[command(name = "context-aware-client")]
#[command(about = "Example client for context-aware cognitive-mcp with proactive knowledge pushes")]
struct Cli {
    /// Session identifier for context tracking
    #[arg(short, long, default_value = "demo-session")]
    session_id: String,

    /// Duration to run (seconds)
    #[arg(short, long, default_value = "60")]
    duration: u64,

    /// Simulate stuck session pattern
    #[arg(long)]
    simulate_stuck: bool,

    /// Simulate error pattern
    #[arg(long)]
    simulate_errors: bool,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Initialize logging
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(if cli.verbose {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    info!("Context-Aware MCP Client Starting");
    info!(session_id = %cli.session_id, duration = cli.duration, "Configuration");

    let client = Client::new();
    let session_id = cli.session_id.clone();

    // Start SSE stream in background
    let stream_handle = tokio::spawn(async move {
        if let Err(e) = run_sse_stream(&session_id).await {
            error!(error = %e, "SSE stream error");
        }
    });

    // Give stream time to connect
    sleep(Duration::from_millis(500)).await;

    // Simulate user activities
    simulate_activities(&client, &cli).await?;

    // Wait for completion
    let _ = tokio::time::timeout(Duration::from_secs(cli.duration), stream_handle).await;

    info!("Context-aware client demonstration complete");
    Ok(())
}

async fn run_sse_stream(session_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    use reqwest_eventsource::{Event as SseEvent, EventSource};

    let url = format!("{}/context/stream/{}", COGNITIVE_MCP_URL, session_id);
    info!(url = %url, "Connecting to SSE stream");

    let mut es = EventSource::get(&url);

    while let Some(event) = es.next().await {
        match event {
            Ok(SseEvent::Open) => {
                info!("SSE connection opened");
            }
            Ok(SseEvent::Message(message)) => {
                handle_sse_event(&message.event, &message.data).await;
            }
            Err(e) => {
                warn!(error = %e, "SSE error");
                break;
            }
        }
    }

    Ok(())
}

async fn handle_sse_event(event_type: &str, data: &str) {
    match event_type {
        "connected" => {
            info!("✅ Connected to context push stream");
        }
        "knowledge_push" => {
            match serde_json::from_str::<serde_json::Value>(data) {
                Ok(push) => {
                    let trigger = push
                        .get("trigger")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let relevance = push
                        .get("relevance_score")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    let reason = push
                        .get("trigger_reason")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    info!(
                        trigger = trigger,
                        relevance = relevance,
                        "🧠 PROACTIVE KNOWLEDGE PUSH RECEIVED"
                    );
                    info!(reason = reason, "   Reason");

                    // Display content based on type
                    if let Some(content) = push.get("content") {
                        match content.get("type").and_then(|v| v.as_str()) {
                            Some("rag_results") => {
                                if let Some(results) = content.get("results") {
                                    info!(results = %results, "   RAG Results");
                                }
                            }
                            Some("memory_entries") => {
                                if let Some(namespace) = content.get("namespace") {
                                    info!(namespace = %namespace, "   Memory Namespace");
                                }
                            }
                            Some("error_assistance") => {
                                if let Some(steps) = content.get("resolution_steps") {
                                    info!(steps = %steps, "   Resolution Steps");
                                }
                            }
                            _ => {
                                info!(content = %content, "   Content");
                            }
                        }
                    }

                    if let Some(action) = push.get("suggested_action").and_then(|v| v.as_str()) {
                        info!(action = action, "   Suggested Action");
                    }
                }
                Err(e) => {
                    warn!(error = %e, data = data, "Failed to parse knowledge push");
                }
            }
        }
        "disconnected" => {
            info!("⚠️  SSE stream disconnected");
        }
        _ => {
            debug!(event_type = event_type, data = data, "SSE event");
        }
    }
}

async fn simulate_activities(client: &Client, cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let base_url = COGNITIVE_MCP_URL;

    info!("Starting activity simulation...");

    // Record initial session start
    record_activity(
        client,
        base_url,
        &cli.session_id,
        "tool_call",
        "Session started - exploring cognitive-mcp features",
        json!({ "demo": true }),
    )
    .await?;

    info!("Activity 1: Tool call recorded");
    sleep(Duration::from_secs(2)).await;

    // Simulate queries with different topics (to trigger new_topic detection)
    let topics = vec![
        "How does database connection pooling work?",
        "Configure PostgreSQL with connection limits",
        "What are the best practices for async database access?",
        "React component state management patterns",
        "Implementing React hooks with TypeScript",
        "Frontend performance optimization techniques",
    ];

    for (i, query) in topics.iter().enumerate() {
        record_activity(
            client,
            base_url,
            &cli.session_id,
            "query",
            query,
            json!({ "sequence": i + 1 }),
        )
        .await?;

        info!(query = query, "Activity: Query recorded");

        // Wait between queries
        sleep(Duration::from_secs(3)).await;
    }

    // Simulate stuck session if requested
    if cli.simulate_stuck {
        info!("Simulating stuck session pattern (repeated similar queries)...");

        for i in 0..4 {
            record_activity(
                client,
                base_url,
                &cli.session_id,
                "query",
                "How to fix the database connection error", // Repeated query
                json!({ "stuck_simulation": true, "attempt": i }),
            )
            .await?;

            sleep(Duration::from_secs(2)).await;
        }
    }

    // Simulate errors if requested
    if cli.simulate_errors {
        info!("Simulating error pattern...");

        for i in 0..3 {
            record_activity(
                client,
                base_url,
                &cli.session_id,
                "error",
                "Database connection timeout",
                json!({
                    "error_type": "timeout",
                    "error_simulation": true,
                    "attempt": i
                }),
            )
            .await?;

            sleep(Duration::from_secs(1)).await;
        }
    }

    // Request on-demand knowledge push
    info!("Requesting on-demand knowledge push...");
    let response = client
        .post(format!("{}/context/request_push", base_url))
        .json(&json!({
            "session_id": cli.session_id,
            "query": "Best practices for async Rust patterns",
            "context": { "demo": true, "manual_request": true }
        }))
        .send()
        .await?;

    let result: serde_json::Value = response.json().await?;
    info!(result = %result, "On-demand push response");

    // Simulate idle and return
    info!("Simulating idle period...");
    sleep(Duration::from_secs(10)).await;

    record_activity(
        client,
        base_url,
        &cli.session_id,
        "return_from_idle",
        "User returned after idle period",
        json!({ "idle_duration_secs": 10 }),
    )
    .await?;

    // Check session status
    info!("Checking session status...");
    let status = client
        .get(format!("{}/context/status/{}", base_url, cli.session_id))
        .send()
        .await?;

    let status_json: serde_json::Value = status.json().await?;
    info!(status = %status_json, "Session status");

    // Periodic keepalive with varied activities
    let mut interval = interval(Duration::from_secs(5));
    let start = tokio::time::Instant::now();
    let duration = Duration::from_secs(cli.duration.saturating_sub(20)); // Leave buffer

    info!("Continuing periodic activity recording...");

    let mut counter = 0;
    loop {
        interval.tick().await;

        if start.elapsed() > duration {
            break;
        }

        counter += 1;

        // Record varied activities
        let (activity_type, content) = match counter % 4 {
            0 => (
                "query",
                format!("Exploring feature set - query {}", counter),
            ),
            1 => ("tool_call", format!("Using tool - call {}", counter)),
            2 => (
                "context_switch",
                format!("Switched to new task {}", counter),
            ),
            _ => ("query", format!("Follow-up question {}", counter)),
        };

        record_activity(
            client,
            base_url,
            &cli.session_id,
            activity_type,
            &content,
            json!({ "periodic": true, "counter": counter }),
        )
        .await?;

        if counter % 5 == 0 {
            info!(
                counter = counter,
                "Recorded {} periodic activities", counter
            );
        }
    }

    info!("Activity simulation complete");
    Ok(())
}

async fn record_activity(
    client: &Client,
    base_url: &str,
    session_id: &str,
    activity_type: &str,
    content: &str,
    metadata: serde_json::Value,
) -> Result<(), Box<dyn std::error::Error>> {
    let payload = json!({
        "session_id": session_id,
        "activity_type": activity_type,
        "content": content,
        "metadata": metadata
    });

    let response = client
        .post(format!("{}/context/record", base_url))
        .json(&payload)
        .send()
        .await?;

    if !response.status().is_success() {
        warn!(
            status = response.status().as_u16(),
            "Failed to record activity"
        );
    }

    Ok(())
}
