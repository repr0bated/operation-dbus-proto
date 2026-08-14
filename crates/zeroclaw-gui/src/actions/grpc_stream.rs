//! `grpc.stream_subscribe` — server-streaming RPCs through the action bus.
//!
//! Dispatch returns immediately with [`ActionStatus::Streaming`] carrying a
//! `stream_id`; a background task drains the RPC into a bounded channel. The
//! viewer component polls [`GrpcStreamHandler::read_frame`] once per egui
//! frame, so the render path never awaits.
//!
//! [`ActionStatus::Streaming`]: super::ActionStatus::Streaming

use super::grpc_call::endpoint_from_payload;
use super::{ActionFuture, ActionHandler, ActionRequest, ActionResult};
use crate::conn::{ConnectionPool, EndpointSpec};
use crate::grpc::{open_server_stream, ReflectionRegistry};
use parking_lot::Mutex;
use prost_reflect::DynamicMessage;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio::task::AbortHandle;
use uuid::Uuid;

/// Frames buffered per stream before the reader task applies backpressure.
/// Bounded on purpose: a stream the UI has stopped reading must not grow
/// without limit.
const FRAME_BUFFER: usize = 256;

/// One live subscription.
pub struct StreamHandle {
    pub stream_id: Uuid,
    /// Fully-qualified `service/method` this stream was opened on.
    pub path: String,
    receiver: Mutex<mpsc::Receiver<StreamFrame>>,
    abort: AbortHandle,
}

impl StreamHandle {
    pub fn stream_id(&self) -> Uuid {
        self.stream_id
    }
}

/// A decoded frame, or the stream's terminal state.
#[derive(Debug, Clone)]
pub enum StreamFrame {
    Message(Value),
    /// Server closed the stream normally.
    Complete,
    Error(String),
}

pub struct GrpcStreamHandler {
    registry: ReflectionRegistry,
    pool: ConnectionPool,
    active: Mutex<HashMap<Uuid, Arc<StreamHandle>>>,
}

impl GrpcStreamHandler {
    pub fn new(registry: ReflectionRegistry, pool: ConnectionPool) -> Self {
        Self {
            registry,
            pool,
            active: Mutex::new(HashMap::new()),
        }
    }

    /// Non-blocking read of the next buffered frame, or `None` when the buffer
    /// is empty or the id is unknown.
    pub fn read_frame(&self, stream_id: Uuid) -> Option<StreamFrame> {
        let handle = self.active.lock().get(&stream_id).cloned()?;
        let mut rx = handle.receiver.lock();
        rx.try_recv().ok()
    }

    /// Drain everything buffered for one stream. Cheaper than repeated
    /// [`Self::read_frame`] when the viewer is catching up after a pause.
    pub fn drain_frames(&self, stream_id: Uuid, max: usize) -> Vec<StreamFrame> {
        let Some(handle) = self.active.lock().get(&stream_id).cloned() else {
            return Vec::new();
        };
        let mut rx = handle.receiver.lock();
        let mut out = Vec::new();
        while out.len() < max {
            match rx.try_recv() {
                Ok(frame) => out.push(frame),
                Err(_) => break,
            }
        }
        out
    }

    /// Abort the reader task and forget the stream. Idempotent.
    pub fn cancel_stream(&self, stream_id: Uuid) -> bool {
        match self.active.lock().remove(&stream_id) {
            Some(handle) => {
                handle.abort.abort();
                true
            }
            None => false,
        }
    }

    pub fn active_stream_ids(&self) -> Vec<Uuid> {
        self.active.lock().keys().copied().collect()
    }

    pub fn is_active(&self, stream_id: Uuid) -> bool {
        self.active.lock().contains_key(&stream_id)
    }

    pub fn active_count(&self) -> usize {
        self.active.lock().len()
    }

    async fn subscribe(&self, req: &ActionRequest, started: Instant) -> ActionResult {
        let elapsed = || started.elapsed().as_millis() as u64;

        let (Some(service), Some(method)) = (req.str_field("service"), req.str_field("method"))
        else {
            return ActionResult::error(
                req,
                "invalid_request",
                "'service' and 'method' are required strings",
                elapsed(),
            );
        };

        let endpoint = match endpoint_from_payload(&req.payload) {
            Ok(e) => e,
            Err(e) => return ActionResult::error(req, "invalid_request", e, elapsed()),
        };
        let body = req
            .payload
            .get("payload")
            .cloned()
            .unwrap_or_else(|| json!({}));

        let path = format!("{service}/{method}");
        let Some(descriptor) = self.registry.resolve_method(&path) else {
            return ActionResult::error(
                req,
                "method_not_found",
                format!("no descriptor for {path}"),
                elapsed(),
            );
        };
        if !descriptor.is_server_streaming() || descriptor.is_client_streaming() {
            return ActionResult::error(
                req,
                "not_streaming",
                format!("{path} is not a server-streaming method"),
                elapsed(),
            );
        }

        let channel = match self.pool.get_channel(&endpoint).await {
            Ok(c) => c,
            Err(e) => {
                return ActionResult::error(req, "connection_failed", format!("{e:#}"), elapsed())
            }
        };

        let mut stream = match open_server_stream(channel, &descriptor, &body).await {
            Ok(s) => s,
            Err(e) => {
                self.pool.mark_dead(&endpoint, format!("{e:#}"));
                return ActionResult::error(req, "grpc_error", format!("{e:#}"), elapsed());
            }
        };

        let stream_id = Uuid::new_v4();
        let (tx, rx) = mpsc::channel(FRAME_BUFFER);
        let output_desc = descriptor.output();

        let task = tokio::spawn(async move {
            loop {
                let frame = match stream.message().await {
                    Ok(Some(bytes)) => {
                        match DynamicMessage::decode(output_desc.clone(), bytes.as_slice())
                            .map_err(|e| e.to_string())
                            .and_then(|msg| {
                                serde_json::to_value(&msg).map_err(|e| e.to_string())
                            }) {
                            Ok(value) => StreamFrame::Message(value),
                            Err(e) => StreamFrame::Error(format!("decode failed: {e}")),
                        }
                    }
                    Ok(None) => StreamFrame::Complete,
                    Err(s) => StreamFrame::Error(format!("gRPC {}: {}", s.code(), s.message())),
                };
                let terminal = !matches!(frame, StreamFrame::Message(_));
                // A closed receiver means the viewer is gone; stop reading.
                if tx.send(frame).await.is_err() || terminal {
                    break;
                }
            }
        });

        self.active.lock().insert(
            stream_id,
            Arc::new(StreamHandle {
                stream_id,
                path,
                receiver: Mutex::new(rx),
                abort: task.abort_handle(),
            }),
        );

        ActionResult::streaming(req, stream_id, elapsed())
    }
}

impl ActionHandler for GrpcStreamHandler {
    fn action_prefix(&self) -> &str {
        "grpc.stream_subscribe"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["service", "method"],
            "properties": {
                "service": { "type": "string" },
                "method": { "type": "string", "description": "Must be a server-streaming RPC" },
                "payload": { "type": "object" },
                "endpoint": { "description": "Omit for 'auto'." }
            }
        })
    }

    fn dispatch<'a>(&'a self, req: &'a ActionRequest) -> ActionFuture<'a> {
        let started = Instant::now();
        Box::pin(async move { self.subscribe(req, started).await })
    }
}

/// Endpoint an action targets, for callers that need it before dispatch.
pub fn target_endpoint(payload: &Value) -> EndpointSpec {
    endpoint_from_payload(payload).unwrap_or(EndpointSpec::Auto)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::{ActionBus, ActionStatus};

    fn handler() -> GrpcStreamHandler {
        GrpcStreamHandler::new(ReflectionRegistry::new(), ConnectionPool::new())
    }

    #[test]
    fn schema_requires_service_and_method() {
        let schema = handler().input_schema();
        let required: Vec<&str> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(required, vec!["service", "method"]);
    }

    #[test]
    fn unknown_stream_reads_nothing_and_cancels_to_false() {
        let h = handler();
        let id = Uuid::new_v4();
        assert!(h.read_frame(id).is_none());
        assert!(h.drain_frames(id, 10).is_empty());
        assert!(!h.cancel_stream(id));
        assert_eq!(h.active_count(), 0);
    }

    #[tokio::test]
    async fn missing_fields_are_rejected_before_dialling() {
        let bus = ActionBus::new();
        bus.register(Arc::new(handler()));

        let result = bus
            .dispatch(ActionRequest::new(
                "grpc.stream_subscribe",
                json!({ "service": "svc.S" }),
            ))
            .await;

        match result.status {
            ActionStatus::Error { code, .. } => assert_eq!(code, "invalid_request"),
            other => panic!("expected invalid_request, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn unknown_method_reports_method_not_found() {
        let bus = ActionBus::new();
        bus.register(Arc::new(handler()));

        let result = bus
            .dispatch(ActionRequest::new(
                "grpc.stream_subscribe",
                json!({ "service": "no.Such", "method": "Watch" }),
            ))
            .await;

        match result.status {
            ActionStatus::Error { code, .. } => assert_eq!(code, "method_not_found"),
            other => panic!("expected method_not_found, got {other:?}"),
        }
    }

    /// The registry/pool are only consulted after the id is minted, so the
    /// bookkeeping half of subscribe → cancel is exercised directly.
    #[tokio::test]
    async fn registered_stream_can_be_read_then_cancelled() {
        let h = handler();
        let stream_id = Uuid::new_v4();
        let (tx, rx) = mpsc::channel(4);
        let task = tokio::spawn(async move {
            // Park until aborted, standing in for a live RPC read loop.
            std::future::pending::<()>().await;
        });
        h.active.lock().insert(
            stream_id,
            Arc::new(StreamHandle {
                stream_id,
                path: "svc.S/Watch".into(),
                receiver: Mutex::new(rx),
                abort: task.abort_handle(),
            }),
        );

        tx.send(StreamFrame::Message(json!({ "n": 1 }))).await.unwrap();
        tx.send(StreamFrame::Complete).await.unwrap();

        assert!(h.is_active(stream_id));
        match h.read_frame(stream_id) {
            Some(StreamFrame::Message(v)) => assert_eq!(v["n"], 1),
            other => panic!("expected a message frame, got {other:?}"),
        }
        assert!(matches!(
            h.read_frame(stream_id),
            Some(StreamFrame::Complete)
        ));
        assert!(h.read_frame(stream_id).is_none());

        assert!(h.cancel_stream(stream_id));
        assert!(!h.is_active(stream_id));
        assert_eq!(h.active_count(), 0);
        // Cancelling twice is not an error.
        assert!(!h.cancel_stream(stream_id));
    }

    #[tokio::test]
    async fn drain_frames_respects_its_cap() {
        let h = handler();
        let stream_id = Uuid::new_v4();
        let (tx, rx) = mpsc::channel(8);
        let task = tokio::spawn(async move { std::future::pending::<()>().await });
        h.active.lock().insert(
            stream_id,
            Arc::new(StreamHandle {
                stream_id,
                path: "svc.S/Watch".into(),
                receiver: Mutex::new(rx),
                abort: task.abort_handle(),
            }),
        );
        for n in 0..5 {
            tx.send(StreamFrame::Message(json!({ "n": n }))).await.unwrap();
        }

        assert_eq!(h.drain_frames(stream_id, 3).len(), 3);
        assert_eq!(h.drain_frames(stream_id, 10).len(), 2);
        assert!(h.drain_frames(stream_id, 10).is_empty());
    }
}
