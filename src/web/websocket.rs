//! WebSocket Handler

use axum::extract::ws::{Message, WebSocket};
use futures::{SinkExt, StreamExt};
use std::sync::Arc;

use crate::mcp::{McpCompactDispatcher, McpRequest};

/// WebSocket message handler
pub struct WebSocketHandler {
    dispatcher: Arc<McpCompactDispatcher>,
}

impl WebSocketHandler {
    pub fn new(dispatcher: Arc<McpCompactDispatcher>) -> Self {
        Self { dispatcher }
    }

    /// Handle a WebSocket connection
    pub async fn handle(&self, socket: WebSocket) {
        let (mut sender, mut receiver) = socket.split();
        let dispatcher = Arc::clone(&self.dispatcher);

        while let Some(msg) = receiver.next().await {
            let msg = match msg {
                Ok(m) => m,
                Err(e) => {
                    tracing::error!("WebSocket error: {}", e);
                    break;
                }
            };

            if let Message::Text(text) = msg {
                // Parse as MCP request
                let request: McpRequest = match serde_json::from_str(&text) {
                    Ok(r) => r,
                    Err(e) => {
                        let error = serde_json::json!({
                            "jsonrpc": "2.0",
                            "error": {
                                "code": -32700,
                                "message": format!("Parse error: {}", e)
                            },
                            "id": null
                        });
                        let _ = sender.send(Message::Text(error.to_string().into())).await;
                        continue;
                    }
                };

                // Handle the request
                let response = dispatcher.handle_request(request).await;
                let response_json = serde_json::to_string(&response).unwrap_or_default();
                
                if sender.send(Message::Text(response_json.into())).await.is_err() {
                    break;
                }
            }
        }
    }
}
