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

impl Default for SseEventBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

impl SseEventBroadcaster {
    pub fn new() -> Self {
        // Sized for a hydration burst, not for steady state: on connect the
        // upstream sends one contract and one state frame per plugin back to
        // back. At 100 a browser that reads slowly would silently lose part of
        // its own hydration, and dropped state frames — unlike dropped schema
        // frames — leave no trace a consumer can detect.
        let (tx, _) = broadcast::channel(1024);
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

/// Replayable hydration for browsers that connect after op-web did.
///
/// Hydration happens once, when *op-web* opens its upstream subscription. Every
/// browser after that attaches to a live broadcast and would otherwise see
/// nothing until something mutates — which is why a freshly opened dashboard
/// looked empty even though the system was fully populated.
///
/// So the bridge records the last frame it saw per contract and per state key,
/// and each new SSE subscriber is replayed that set before the live stream.
/// This is a replay buffer, not a second source of truth: every entry is a
/// verbatim frame the upstream produced, and the contract hashes on them let a
/// consumer detect if a replayed frame is behind the catalog.
#[derive(Default)]
pub struct HydrationCache {
    /// Latest contract frame per plugin id.
    schemas: std::sync::RwLock<std::collections::BTreeMap<String, String>>,
    /// Latest state frame per `plugin|path|property` key — the same key the UI
    /// accumulates under, so a replay reconstructs exactly what a subscriber
    /// present from the start would be holding.
    states: std::sync::RwLock<std::collections::BTreeMap<String, String>>,
}

impl HydrationCache {
    pub fn record_schema(&self, plugin_id: &str, json: &str) {
        if let Ok(mut schemas) = self.schemas.write() {
            schemas.insert(plugin_id.to_string(), json.to_string());
        }
    }

    pub fn record_state(&self, key: String, json: &str) {
        if let Ok(mut states) = self.states.write() {
            states.insert(key, json.to_string());
        }
    }

    /// Contracts first, then state — a renderer needs the shape before values.
    pub fn replay(&self) -> Vec<SseEvent> {
        let mut out = Vec::new();
        if let Ok(schemas) = self.schemas.read() {
            out.extend(schemas.values().map(|data| SseEvent {
                event_type: "schema_update".to_string(),
                data: data.clone(),
            }));
        }
        if let Ok(states) = self.states.read() {
            out.extend(states.values().map(|data| SseEvent {
                event_type: "state_update".to_string(),
                data: data.clone(),
            }));
        }
        out
    }

    pub fn len(&self) -> (usize, usize) {
        (
            self.schemas.read().map(|s| s.len()).unwrap_or(0),
            self.states.read().map(|s| s.len()).unwrap_or(0),
        )
    }
}

/// GET /api/events - SSE event stream
pub async fn sse_handler(
    Extension(state): Extension<Arc<AppState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // Subscribe before taking the replay so a frame arriving mid-hydration is
    // duplicated rather than lost. Same ordering rule the gRPC subscribe uses.
    let rx = state.sse_broadcaster.subscribe();
    let replay = state.hydration.replay();

    let stream = BroadcastStream::new(rx).filter_map(
        |result: Result<SseEvent, tokio_stream::wrappers::errors::BroadcastStreamRecvError>| {
            result
                .ok()
                .map(|event| Ok(Event::default().event(event.event_type).data(event.data)))
        },
    );

    // Replay is ordered and must arrive before anything live, so it is chained
    // ahead of the broadcast rather than selected against it.
    let hydration = stream::iter(
        replay
            .into_iter()
            .map(|event| Ok(Event::default().event(event.event_type).data(event.data))),
    );
    let stream = hydration.chain(stream);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replay_puts_every_contract_before_any_state() {
        let cache = HydrationCache::default();
        cache.record_state("network|/p/network|bridges".into(), r#"{"v":1}"#);
        cache.record_schema("network", r#"{"schema_hash":"aaaa"}"#);
        cache.record_state("adc|/p/adc|status".into(), r#"{"v":2}"#);
        cache.record_schema("adc", r#"{"schema_hash":"bbbb"}"#);

        let replay = cache.replay();
        let kinds: Vec<&str> = replay.iter().map(|e| e.event_type.as_str()).collect();

        // A renderer needs the shape before the values, regardless of the order
        // the frames were originally recorded in.
        assert_eq!(
            kinds,
            vec![
                "schema_update",
                "schema_update",
                "state_update",
                "state_update"
            ]
        );
        assert_eq!(cache.len(), (2, 2));
    }

    #[test]
    fn a_replaced_frame_is_not_replayed_twice() {
        let cache = HydrationCache::default();
        cache.record_schema("network", r#"{"schema_hash":"old"}"#);
        cache.record_schema("network", r#"{"schema_hash":"new"}"#);
        cache.record_state("network|/p/network|bridges".into(), r#"{"v":1}"#);
        cache.record_state("network|/p/network|bridges".into(), r#"{"v":2}"#);

        let replay = cache.replay();

        // Only the current frame per key survives — a late subscriber hydrates
        // to present state, not to a history of it.
        assert_eq!(replay.len(), 2);
        assert!(replay[0].data.contains("new"));
        assert!(replay[1].data.contains(r#""v":2"#));
    }
}
