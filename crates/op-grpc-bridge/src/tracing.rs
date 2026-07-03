// 🟢 🛡️ Ghostbridge Trace Middleware
// Stamps X-Ghostbridge-Footprint and X-Ghostbridge-Trace-ID on every HTTP/gRPC
// response. Operates inside the zeroclaw Axum host (the HTTP/gRPC-Web side of
// The Shuttle). Mirrors the accountability loop enforced by the tonic
// interceptor on the native-gRPC side.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use axum::extract::Request;
use axum::response::Response;
use tracing::{warn, Span};
use uuid::Uuid;

const FOOTPRINT_HEADER: &str = "X-Ghostbridge-Footprint";
const TRACE_ID_HEADER: &str = "X-Ghostbridge-Trace-ID";

const SENTINEL_FOOTPRINT: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// Accountability trace context propagated from the HTTP layer into the
/// tonic gRPC service via request extensions. Both the HTTP headers and the
/// gRPC response message carry the same values.
#[derive(Clone, Debug)]
pub struct TraceContext {
    pub footprint: String,
    pub trace_id: String,
}

impl TraceContext {
    /// Extract or generate trace context from HTTP headers.
    pub fn from_headers(headers: &axum::http::HeaderMap) -> Self {
        let footprint = headers
            .get(FOOTPRINT_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                warn!("missing X-Ghostbridge-Footprint; stamping sentinel footprint");
                SENTINEL_FOOTPRINT.to_string()
            });
        let trace_id = headers
            .get(TRACE_ID_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                let id = Uuid::now_v7().to_string();
                warn!(trace_id = %id, "missing X-Ghostbridge-Trace-ID; generated UUIDv7");
                id
            });
        Self {
            footprint,
            trace_id,
        }
    }

    /// Extract trace context from tonic request metadata/headers.
    pub fn from_tonic_metadata(metadata: &tonic::metadata::MetadataMap) -> Self {
        let footprint = metadata
            .get(FOOTPRINT_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(|| SENTINEL_FOOTPRINT.to_string());
        let trace_id = metadata
            .get(TRACE_ID_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                let id = Uuid::now_v7().to_string();
                warn!(trace_id = %id, "missing X-Ghostbridge-Trace-ID in gRPC metadata; generated UUIDv7");
                id
            });
        Self {
            footprint,
            trace_id,
        }
    }
}

/// Tower layer that ensures every response carries the accountability headers.
#[derive(Clone, Default)]
pub struct GhostbridgeTraceLayer;

impl GhostbridgeTraceLayer {
    pub fn new() -> Self {
        Self
    }
}

impl<S> tower::Layer<S> for GhostbridgeTraceLayer {
    type Service = GhostbridgeTraceService<S>;

    fn layer(&self, inner: S) -> GhostbridgeTraceService<S> {
        GhostbridgeTraceService {
            inner,
            span: Span::current(),
        }
    }
}

/// Tower service wrapping an inner service to inject trace headers.
#[derive(Clone)]
pub struct GhostbridgeTraceService<S> {
    inner: S,
    span: Span,
}

impl<S> tower::Service<Request> for GhostbridgeTraceService<S>
where
    S: tower::Service<Request, Response = Response> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Response, S::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), S::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request) -> Self::Future {
        let mut inner = self.inner.clone();
        let span = self.span.clone();

        let context = TraceContext::from_headers(req.headers());
        req.extensions_mut().insert(context.clone());

        Box::pin(async move {
            let _enter = span.enter();
            let mut response = inner.call(req).await?;

            response
                .headers_mut()
                .insert(FOOTPRINT_HEADER, context.footprint.parse().unwrap());
            response
                .headers_mut()
                .insert(TRACE_ID_HEADER, context.trace_id.parse().unwrap());

            Ok(response)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use tower::Layer;
    use tower::ServiceExt;

    #[tokio::test]
    async fn should_stamp_trace_headers_on_response() {
        let service = GhostbridgeTraceLayer::new()
            .layer(axum::Router::new().route("/", axum::routing::get(|| async { "ok" })));

        let req = Request::builder()
            .uri("/")
            .header(FOOTPRINT_HEADER, "abc123")
            .header(TRACE_ID_HEADER, "trace-123")
            .body(Body::empty())
            .unwrap();

        let response = service.oneshot(req).await.unwrap();
        assert_eq!(response.headers().get(FOOTPRINT_HEADER).unwrap(), "abc123");
        assert_eq!(
            response.headers().get(TRACE_ID_HEADER).unwrap(),
            "trace-123"
        );
    }

    #[tokio::test]
    async fn should_generate_trace_id_when_absent() {
        let service = GhostbridgeTraceLayer::new()
            .layer(axum::Router::new().route("/", axum::routing::get(|| async { "ok" })));

        let req = Request::builder().uri("/").body(Body::empty()).unwrap();

        let response = service.oneshot(req).await.unwrap();
        assert!(response.headers().get(FOOTPRINT_HEADER).is_some());
        let trace_id = response.headers().get(TRACE_ID_HEADER).unwrap();
        let trace_str = trace_id.to_str().unwrap();
        assert!(
            Uuid::parse_str(trace_str).is_ok(),
            "trace id should be a valid UUID"
        );
    }
}
