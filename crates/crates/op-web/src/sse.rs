//! Server-Sent Events (SSE) Handler

use axum::{
    extract::Extension,
    response::sse::{Event, Sse},
};
use futures::stream::{self, Stream};
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::state::AppState;

/// SSE Event Broadcaster
pub struct SseEventBroadcaster {
    tx: broadcast::Sender<SseEvent>,
}

#[derive(Clone, Debug)]
pub struct SseEvent {
    pub event_type: String,
    pub data: String,
}

impl SseEventBroadcaster {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(100);
        Self { tx }
    }

    #[allow(dead_code)]
    pub fn broadcast(&self, event_type: &str, data: &str) {
        let _ = self.tx.send(SseEvent {
            event_type: event_type.to_string(),
            data: data.to_string(),
        });
    }

    pub fn subscribe(&self) -> broadcast::Receiver<SseEvent> {
        self.tx.subscribe()
    }
}

/// GET /api/events - SSE event stream
pub async fn sse_handler(
    Extension(state): Extension<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.sse_broadcaster.subscribe();

    let stream = BroadcastStream::new(rx).filter_map(
        |result: Result<SseEvent, tokio_stream::wrappers::errors::BroadcastStreamRecvError>| {
            result
                .ok()
                .map(|event| Ok(Event::default().event(event.event_type).data(event.data)))
        },
    );

    // Add keepalive
    let keepalive = stream::repeat_with(|| Ok(Event::default().comment("keepalive")))
        .throttle(Duration::from_secs(30));

    let combined = stream::select(stream, keepalive);

    Sse::new(combined).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    )
}
