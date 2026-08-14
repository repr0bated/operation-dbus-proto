//! `grpc.call` — dynamic unary invocation through the action bus.
//!
//! Resolves the method against blob-backed descriptor pools (so a method is
//! callable without a live reflection handshake), dials through the shared
//! connection pool, and encodes/decodes via `prost_reflect::DynamicMessage`.

use super::{ActionFuture, ActionHandler, ActionRequest, ActionResult};
use crate::conn::{ConnectionPool, EndpointSpec};
use crate::grpc::{invoke_unary_on, ReflectionRegistry};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

/// Wall-clock ceiling for one unary call, so a wedged backend cannot leave a
/// spinner up forever.
const CALL_TIMEOUT: Duration = Duration::from_secs(5);

pub struct GrpcCallHandler {
    // `ReflectionRegistry` is already `Clone` over interior `Arc<RwLock<_>>`
    // fields, so it is held directly rather than wrapped in another
    // `Arc<RwLock<_>>` — a second layer would add a lock without adding safety.
    registry: ReflectionRegistry,
    pool: ConnectionPool,
    timeout: Duration,
}

impl GrpcCallHandler {
    pub fn new(registry: ReflectionRegistry, pool: ConnectionPool) -> Self {
        Self {
            registry,
            pool,
            timeout: CALL_TIMEOUT,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Check the payload against the `required` list in [`Self::input_schema`].
    ///
    /// Driven by the schema itself so the two cannot drift apart. This is a
    /// presence check, not full JSON Schema validation — the authoritative
    /// shape check is `DynamicMessage::deserialize` against the input
    /// descriptor, which happens during dispatch.
    pub fn validate(&self, payload: &Value) -> Result<(), String> {
        let schema = self.input_schema();
        let required = schema
            .get("required")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for field in required {
            let Some(name) = field.as_str() else { continue };
            match payload.get(name) {
                None | Some(Value::Null) => {
                    return Err(format!("missing required field '{name}'"))
                }
                Some(_) => {}
            }
        }
        Ok(())
    }

    async fn call(&self, req: &ActionRequest, started: Instant) -> ActionResult {
        let elapsed = |from: Instant| from.elapsed().as_millis() as u64;

        if let Err(e) = self.validate(&req.payload) {
            return ActionResult::error(req, "invalid_request", e, elapsed(started));
        }
        // `validate` proved both are present and non-null; a non-string here is
        // still a client error rather than a panic.
        let (Some(service), Some(method)) = (req.str_field("service"), req.str_field("method"))
        else {
            return ActionResult::error(
                req,
                "invalid_request",
                "'service' and 'method' must be strings",
                elapsed(started),
            );
        };

        let endpoint = match endpoint_from_payload(&req.payload) {
            Ok(e) => e,
            Err(e) => return ActionResult::error(req, "invalid_request", e, elapsed(started)),
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
                format!(
                    "no descriptor for {path} (plugin may be inactive — blob not in SHM catalog)"
                ),
                elapsed(started),
            );
        };

        let channel = match self.pool.get_channel(&endpoint).await {
            Ok(c) => c,
            Err(e) => {
                return ActionResult::error(
                    req,
                    "connection_failed",
                    format!("{e:#}"),
                    elapsed(started),
                )
            }
        };

        // Time only the RPC, so latency reported to the UI excludes descriptor
        // resolution and a possible first-use dial.
        let rpc_started = Instant::now();
        match tokio::time::timeout(
            self.timeout,
            invoke_unary_on(channel, &descriptor, &body),
        )
        .await
        {
            Ok(Ok(value)) => ActionResult::success(req, value, elapsed(rpc_started)),
            Ok(Err(e)) => {
                // The channel may be half-dead; force a re-dial next time.
                self.pool.mark_dead(&endpoint, format!("{e:#}"));
                ActionResult::error(req, "grpc_error", format!("{e:#}"), elapsed(rpc_started))
            }
            Err(_) => {
                self.pool.mark_dead(&endpoint, "call timed out");
                ActionResult::error(
                    req,
                    "timeout",
                    format!("{path} exceeded {:?}", self.timeout),
                    elapsed(rpc_started),
                )
            }
        }
    }
}

impl ActionHandler for GrpcCallHandler {
    fn action_prefix(&self) -> &str {
        "grpc.call"
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["service", "method", "payload"],
            "properties": {
                "service": {
                    "type": "string",
                    "description": "Fully-qualified service name, e.g. operation.method.adc.get.GetService"
                },
                "method": { "type": "string", "description": "RPC name within the service" },
                "payload": { "type": "object", "description": "Request body, matching the method's input descriptor" },
                "endpoint": {
                    "description": "Where to dial. Omit for 'auto' (Unix socket, then TCP).",
                    "oneOf": [
                        { "type": "string", "enum": ["auto"] },
                        { "type": "string", "description": "unix:/path or http://host:port" },
                        { "type": "object" }
                    ]
                }
            }
        })
    }

    fn dispatch<'a>(&'a self, req: &'a ActionRequest) -> ActionFuture<'a> {
        let started = Instant::now();
        Box::pin(async move { self.call(req, started).await })
    }
}

/// Accept an endpoint as the tagged object form, a bare connection string, or
/// nothing at all (meaning `Auto`).
pub(crate) fn endpoint_from_payload(payload: &Value) -> Result<EndpointSpec, String> {
    match payload.get("endpoint") {
        None | Some(Value::Null) => Ok(EndpointSpec::Auto),
        Some(Value::String(s)) => endpoint_from_str(s),
        Some(other) => serde_json::from_value(other.clone())
            .map_err(|e| format!("invalid endpoint {other}: {e}")),
    }
}

fn endpoint_from_str(s: &str) -> Result<EndpointSpec, String> {
    let s = s.trim();
    if s.eq_ignore_ascii_case("auto") || s.is_empty() {
        return Ok(EndpointSpec::Auto);
    }
    if let Some(path) = s.strip_prefix("unix:") {
        let path = path.trim_start_matches('/');
        return Ok(EndpointSpec::Unix {
            path: format!("/{path}"),
        });
    }
    if s.starts_with("http://") || s.starts_with("https://") {
        return Ok(EndpointSpec::GrpcWeb { url: s.to_string() });
    }
    // Bare `host:port`.
    match s.rsplit_once(':') {
        Some((host, port)) => port
            .parse::<u16>()
            .map(|port| EndpointSpec::Tcp {
                host: host.to_string(),
                port,
            })
            .map_err(|_| format!("invalid endpoint '{s}': port is not a number")),
        None => Err(format!("invalid endpoint '{s}'")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::{ActionBus, ActionStatus};
    use std::sync::Arc;

    fn handler() -> GrpcCallHandler {
        GrpcCallHandler::new(ReflectionRegistry::new(), ConnectionPool::new())
    }

    #[test]
    fn input_schema_requires_service_method_and_payload() {
        let schema = handler().input_schema();
        let required: Vec<&str> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(required, vec!["service", "method", "payload"]);
    }

    #[test]
    fn validate_rejects_missing_method() {
        let err = handler()
            .validate(&json!({ "service": "svc.S", "payload": {} }))
            .unwrap_err();
        assert!(err.contains("method"), "unexpected error: {err}");
    }

    #[test]
    fn validate_rejects_null_field() {
        assert!(handler()
            .validate(&json!({ "service": "svc.S", "method": null, "payload": {} }))
            .is_err());
    }

    #[test]
    fn validate_accepts_a_complete_payload() {
        assert!(handler()
            .validate(&json!({ "service": "svc.S", "method": "M", "payload": {} }))
            .is_ok());
    }

    #[test]
    fn endpoint_defaults_to_auto() {
        assert_eq!(
            endpoint_from_payload(&json!({})).unwrap(),
            EndpointSpec::Auto
        );
        assert_eq!(
            endpoint_from_payload(&json!({ "endpoint": "auto" })).unwrap(),
            EndpointSpec::Auto
        );
    }

    #[test]
    fn endpoint_parses_string_and_object_forms() {
        assert_eq!(
            endpoint_from_payload(&json!({ "endpoint": "unix:/run/x.sock" })).unwrap(),
            EndpointSpec::Unix {
                path: "/run/x.sock".into()
            }
        );
        assert_eq!(
            endpoint_from_payload(&json!({ "endpoint": "127.0.0.1:8090" })).unwrap(),
            EndpointSpec::Tcp {
                host: "127.0.0.1".into(),
                port: 8090
            }
        );
        assert_eq!(
            endpoint_from_payload(&json!({ "endpoint": {"type":"unix","path":"/a.sock"} })).unwrap(),
            EndpointSpec::Unix {
                path: "/a.sock".into()
            }
        );
    }

    #[test]
    fn endpoint_rejects_garbage() {
        assert!(endpoint_from_payload(&json!({ "endpoint": "127.0.0.1:notaport" })).is_err());
    }

    #[tokio::test]
    async fn dispatch_reports_invalid_request_before_touching_the_network() {
        let bus = ActionBus::new();
        bus.register(Arc::new(handler()));

        let result = bus
            .dispatch(ActionRequest::new(
                "grpc.call",
                json!({ "service": "svc.S", "payload": {} }),
            ))
            .await;

        match result.status {
            ActionStatus::Error { code, message } => {
                assert_eq!(code, "invalid_request");
                assert!(message.contains("method"), "unexpected message: {message}");
            }
            other => panic!("expected invalid_request, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn dispatch_reports_method_not_found_for_an_unknown_service() {
        let bus = ActionBus::new();
        bus.register(Arc::new(handler()));

        let result = bus
            .dispatch(ActionRequest::new(
                "grpc.call",
                json!({ "service": "no.Such", "method": "M", "payload": {} }),
            ))
            .await;

        match result.status {
            ActionStatus::Error { code, .. } => assert_eq!(code, "method_not_found"),
            other => panic!("expected method_not_found, got {other:?}"),
        }
        // Resolution failure must be caught before any dial is attempted.
        assert!(bus.audit_log()[0].target.contains("no.Such/M"));
    }
}
