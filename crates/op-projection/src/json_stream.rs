//! JSON-stream Server: Real-time UI delivery.
//!
//! This module implements the `JsonStreamServer` trait, providing SSE/WebSocket
//! server functionality for streaming projections to the UI using Axum.

use crate::data_models::*;
use crate::interfaces::{JsonStreamServer, JsonStreamStatus};
use anyhow::Result;
use axum::{
    extract::State,
    response::sse::{Event, Sse},
    routing::get,
    Router,
};
use futures::stream::{self, Stream};
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use tracing::{info, warn};

/// Internal state for the Axum server
struct ServerState {
    tx: broadcast::Sender<ProjectionUpdate>,
    client_count: Arc<std::sync::atomic::AtomicUsize>,
    total_clients: Arc<std::sync::atomic::AtomicU64>,
}

/// Server that streams projection updates to connected clients.
#[derive(Debug)]
pub struct ProjectionStreamServer {
    /// Port to listen on
    port: u16,
    /// Whether the server is running
    running: bool,
    /// Channel for broadcasting updates
    tx: broadcast::Sender<ProjectionUpdate>,
    /// Number of connected clients
    client_count: Arc<std::sync::atomic::AtomicUsize>,
    /// Total clients served
    total_clients: Arc<std::sync::atomic::AtomicU64>,
    /// Total messages sent
    messages_sent: Arc<std::sync::atomic::AtomicU64>,
}

impl ProjectionStreamServer {
    /// Creates a new ProjectionStreamServer
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self {
            port: 0,
            running: false,
            tx,
            client_count: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            total_clients: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            messages_sent: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }
}

impl Default for ProjectionStreamServer {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonStreamServer for ProjectionStreamServer {
    fn start(&mut self, port: u16) -> Result<()> {
        self.port = port;
        self.running = true;
        
        let state = Arc::new(ServerState {
            tx: self.tx.clone(),
            client_count: self.client_count.clone(),
            total_clients: self.total_clients.clone(),
        });

        let app = Router::new()
            .route("/events", get(sse_handler))
            .with_state(state);

        let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
        
        info!(port = port, "Starting JSON-stream SSE server");
        
        tokio::spawn(async move {
            let listener = match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => l,
                Err(e) => {
                    warn!(error = %e, "Failed to bind JSON-stream server");
                    return;
                }
            };
            
            if let Err(e) = axum::serve(listener, app).await {
                warn!(error = %e, "JSON-stream server error");
            }
        });

        info!(port = port, "JSON-stream server started in background");
        Ok(())
    }

    fn broadcast(&self, update: &ProjectionUpdate) {
        if !self.running {
            return;
        }

        if let Err(_e) = self.tx.send(update.clone()) {
            // This is expected if no clients are connected
        } else {
            self.messages_sent.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }

    fn handle_client(&self, client_id: &str) -> Result<()> {
        info!(client_id = client_id, "New client connected to JSON-stream");
        self.client_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.total_clients.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    fn disconnect_client(&self, client_id: &str) {
        info!(client_id = client_id, "Client disconnected from JSON-stream");
        self.client_count.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }

    fn client_count(&self) -> usize {
        self.client_count.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn status(&self) -> JsonStreamStatus {
        JsonStreamStatus {
            running: self.running,
            port: self.port,
            client_count: self.client_count(),
            total_clients: self.total_clients.load(std::sync::atomic::Ordering::SeqCst),
            messages_sent: self.messages_sent.load(std::sync::atomic::Ordering::SeqCst),
        }
    }
}

/// SSE handler for Axum
async fn sse_handler(
    State(state): State<Arc<ServerState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    state.client_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    state.total_clients.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    
    let rx = state.tx.subscribe();
    
    let stream = BroadcastStream::new(rx).filter_map(|result| {
        match result {
            Ok(update) => {
                let data = serde_json::to_string(&update).unwrap_or_default();
                Some(Ok(Event::default().event("projection_update").data(data)))
            }
            Err(_) => None,
        }
    });

    // Add keepalive
    let keepalive = stream::repeat_with(|| Ok(Event::default().comment("keepalive")))
        .throttle(std::time::Duration::from_secs(30));

    let combined = stream::select(stream, keepalive);

    Sse::new(combined).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("ping"),
    )
}
