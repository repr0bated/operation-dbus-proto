// The consumers of this module are the UI components in tasks 7–12 of the
// netmaker json-render spec; until they land, the bus is exercised only by its
// own tests. Scoped to this module so it never hides dead code elsewhere, and
// deletable as soon as a component dispatches its first action.
#![allow(dead_code)]

//! Action bus — the one dispatch surface for side-effecting operations.
//!
//! Components never perform I/O while rendering. They build an
//! [`ActionRequest`] and hand it to the [`ActionBus`], which routes by
//! `action_type` prefix to a registered [`ActionHandler`] and records every
//! dispatch in an audit log. That keeps the egui render path allocation-cheap
//! and non-blocking, and gives one place to see what the console actually did.

pub mod grpc_call;
pub mod grpc_stream;

use parking_lot::Mutex;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Instant, SystemTime};
use uuid::Uuid;

/// How many audit entries to keep before dropping the oldest.
const AUDIT_CAPACITY: usize = 512;

/// Boxed future returned by [`ActionHandler::dispatch`].
///
/// The bus stores handlers as `dyn ActionHandler`, and `async fn` in a trait is
/// not object-safe, so the future is boxed explicitly.
pub type ActionFuture<'a> = Pin<Box<dyn Future<Output = ActionResult> + Send + 'a>>;

/// One request through the bus. `correlation_id` ties the request, its result,
/// and its audit entry together.
#[derive(Debug, Clone)]
pub struct ActionRequest {
    pub correlation_id: Uuid,
    pub action_type: String,
    pub payload: Value,
    pub timestamp: SystemTime,
}

impl ActionRequest {
    pub fn new(action_type: impl Into<String>, payload: Value) -> Self {
        Self {
            correlation_id: Uuid::new_v4(),
            action_type: action_type.into(),
            payload,
            timestamp: SystemTime::now(),
        }
    }

    /// Read a required string field out of the payload.
    pub fn str_field(&self, key: &str) -> Option<&str> {
        self.payload.get(key).and_then(Value::as_str)
    }
}

/// Terminal state of a dispatch. `Streaming` is not a failure — it means the
/// handler opened a stream and the caller should read frames by `stream_id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ActionStatus {
    Success,
    Error { code: String, message: String },
    Streaming { stream_id: Uuid },
}

impl ActionStatus {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        ActionStatus::Error {
            code: code.into(),
            message: message.into(),
        }
    }

    pub fn is_success(&self) -> bool {
        matches!(self, ActionStatus::Success | ActionStatus::Streaming { .. })
    }
}

/// Outcome of a dispatch, always carrying the originating `correlation_id`.
#[derive(Debug, Clone)]
pub struct ActionResult {
    pub correlation_id: Uuid,
    pub status: ActionStatus,
    pub payload: Value,
    pub latency_ms: u64,
}

impl ActionResult {
    pub fn success(req: &ActionRequest, payload: Value, latency_ms: u64) -> Self {
        Self {
            correlation_id: req.correlation_id,
            status: ActionStatus::Success,
            payload,
            latency_ms,
        }
    }

    pub fn error(
        req: &ActionRequest,
        code: impl Into<String>,
        message: impl Into<String>,
        latency_ms: u64,
    ) -> Self {
        Self {
            correlation_id: req.correlation_id,
            status: ActionStatus::error(code, message),
            payload: Value::Null,
            latency_ms,
        }
    }

    pub fn streaming(req: &ActionRequest, stream_id: Uuid, latency_ms: u64) -> Self {
        Self {
            correlation_id: req.correlation_id,
            status: ActionStatus::Streaming { stream_id },
            payload: Value::Null,
            latency_ms,
        }
    }

    /// `Some(message)` when the dispatch failed, for UI error surfaces.
    pub fn error_message(&self) -> Option<&str> {
        match &self.status {
            ActionStatus::Error { message, .. } => Some(message.as_str()),
            _ => None,
        }
    }
}

/// A registered capability. `action_prefix` claims a dotted namespace: a
/// handler with prefix `grpc.call` receives `grpc.call` and `grpc.call.*`.
pub trait ActionHandler: Send + Sync {
    fn action_prefix(&self) -> &str;

    /// JSON Schema for the accepted payload. Surfaced in the UI and used by
    /// [`ActionBus::validate`] to reject malformed requests before dispatch.
    fn input_schema(&self) -> Value;

    fn dispatch<'a>(&'a self, req: &'a ActionRequest) -> ActionFuture<'a>;
}

/// One line of the dispatch log.
#[derive(Debug, Clone, Serialize)]
pub struct AuditEntry {
    pub correlation_id: Uuid,
    #[serde(skip)]
    pub timestamp: SystemTime,
    pub action_type: String,
    /// Best-effort human label for what was acted on (`service/method`, a
    /// socket path, …). Empty when the payload carries nothing identifying.
    pub target: String,
    pub status: ActionStatus,
    pub latency_ms: u64,
}

/// Routes actions to handlers and keeps the audit trail.
#[derive(Default)]
pub struct ActionBus {
    handlers: Mutex<HashMap<String, Arc<dyn ActionHandler>>>,
    audit: Mutex<Vec<AuditEntry>>,
}

impl ActionBus {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a handler under its own `action_prefix`. A second handler for
    /// the same prefix replaces the first.
    pub fn register(&self, handler: Arc<dyn ActionHandler>) {
        let prefix = handler.action_prefix().to_string();
        self.handlers.lock().insert(prefix, handler);
    }

    pub fn registered_prefixes(&self) -> Vec<String> {
        let mut prefixes: Vec<String> = self.handlers.lock().keys().cloned().collect();
        prefixes.sort();
        prefixes
    }

    /// Longest-prefix match, so `grpc.call.batch` prefers a `grpc.call`
    /// handler over a broader `grpc` one.
    fn resolve(&self, action_type: &str) -> Option<Arc<dyn ActionHandler>> {
        self.handlers
            .lock()
            .iter()
            .filter(|(prefix, _)| {
                action_type == prefix.as_str()
                    || action_type.starts_with(&format!("{}.", prefix.as_str()))
            })
            .max_by_key(|(prefix, _)| prefix.len())
            .map(|(_, handler)| handler.clone())
    }

    /// Dispatch and record. Never panics and never returns `Err`: a missing
    /// handler or a handler failure both come back as `ActionStatus::Error`,
    /// so the caller has exactly one shape to render.
    pub async fn dispatch(&self, req: ActionRequest) -> ActionResult {
        let started = Instant::now();
        let result = match self.resolve(&req.action_type) {
            Some(handler) => handler.dispatch(&req).await,
            None => ActionResult::error(
                &req,
                "unregistered_action",
                format!("no handler registered for action type '{}'", req.action_type),
                started.elapsed().as_millis() as u64,
            ),
        };
        self.record(&req, &result);
        result
    }

    fn record(&self, req: &ActionRequest, result: &ActionResult) {
        let mut audit = self.audit.lock();
        if audit.len() >= AUDIT_CAPACITY {
            let overflow = audit.len() + 1 - AUDIT_CAPACITY;
            audit.drain(0..overflow);
        }
        audit.push(AuditEntry {
            correlation_id: req.correlation_id,
            timestamp: req.timestamp,
            action_type: req.action_type.clone(),
            target: audit_target(&req.payload),
            status: result.status.clone(),
            latency_ms: result.latency_ms,
        });
    }

    pub fn audit_log(&self) -> Vec<AuditEntry> {
        self.audit.lock().clone()
    }

    pub fn audit_len(&self) -> usize {
        self.audit.lock().len()
    }
}

/// Pull a display label out of an action payload. Purely for the audit log —
/// never used for routing.
fn audit_target(payload: &Value) -> String {
    if let (Some(service), Some(method)) = (
        payload.get("service").and_then(Value::as_str),
        payload.get("method").and_then(Value::as_str),
    ) {
        return format!("{service}/{method}");
    }
    for key in ["target", "endpoint", "path", "plugin"] {
        match payload.get(key) {
            Some(Value::String(s)) => return s.clone(),
            Some(other) => return other.to_string(),
            None => {}
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct MockHandler {
        prefix: &'static str,
    }

    impl ActionHandler for MockHandler {
        fn action_prefix(&self) -> &str {
            self.prefix
        }

        fn input_schema(&self) -> Value {
            json!({ "type": "object" })
        }

        fn dispatch<'a>(&'a self, req: &'a ActionRequest) -> ActionFuture<'a> {
            Box::pin(async move { ActionResult::success(req, json!({ "echo": req.payload }), 1) })
        }
    }

    fn bus_with_mock(prefix: &'static str) -> ActionBus {
        let bus = ActionBus::new();
        bus.register(Arc::new(MockHandler { prefix }));
        bus
    }

    #[tokio::test]
    async fn dispatch_routes_to_registered_handler() {
        let bus = bus_with_mock("mock.action");
        let req = ActionRequest::new("mock.action", json!({ "v": 1 }));
        let correlation_id = req.correlation_id;

        let result = bus.dispatch(req).await;

        assert_eq!(result.status, ActionStatus::Success);
        assert_eq!(result.correlation_id, correlation_id);
        assert_eq!(result.payload["echo"]["v"], 1);
    }

    #[tokio::test]
    async fn dispatch_to_unregistered_prefix_is_an_error() {
        let bus = bus_with_mock("mock.action");

        let result = bus.dispatch(ActionRequest::new("nope.missing", json!({}))).await;

        match result.status {
            ActionStatus::Error { code, .. } => assert_eq!(code, "unregistered_action"),
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn audit_log_records_each_dispatch() {
        let bus = bus_with_mock("mock.action");
        let req = ActionRequest::new("mock.action", json!({ "service": "svc.S", "method": "M" }));
        let correlation_id = req.correlation_id;

        bus.dispatch(req).await;

        let log = bus.audit_log();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].correlation_id, correlation_id);
        assert_eq!(log[0].action_type, "mock.action");
        assert_eq!(log[0].target, "svc.S/M");
        assert_eq!(log[0].status, ActionStatus::Success);
    }

    #[tokio::test]
    async fn failed_dispatch_is_audited_too() {
        let bus = bus_with_mock("mock.action");

        bus.dispatch(ActionRequest::new("unknown", json!({}))).await;

        let log = bus.audit_log();
        assert_eq!(log.len(), 1);
        assert!(matches!(log[0].status, ActionStatus::Error { .. }));
    }

    #[tokio::test]
    async fn longest_prefix_wins() {
        let bus = ActionBus::new();
        bus.register(Arc::new(MockHandler { prefix: "grpc" }));
        bus.register(Arc::new(MockHandler { prefix: "grpc.call" }));

        assert_eq!(bus.resolve("grpc.call").unwrap().action_prefix(), "grpc.call");
        assert_eq!(bus.resolve("grpc.stream").unwrap().action_prefix(), "grpc");
    }

    #[test]
    fn prefix_match_respects_dot_boundaries() {
        let bus = bus_with_mock("grpc.call");
        // `grpc.callback` must not match the `grpc.call` handler.
        assert!(bus.resolve("grpc.callback").is_none());
        assert!(bus.resolve("grpc.call.batch").is_some());
    }

    #[test]
    fn audit_target_falls_back_through_keys() {
        assert_eq!(audit_target(&json!({ "endpoint": "unix:/x.sock" })), "unix:/x.sock");
        assert_eq!(audit_target(&json!({ "other": 1 })), "");
    }
}
