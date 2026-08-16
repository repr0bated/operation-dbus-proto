//! gRPC-Web enablement with the Ghostbridge identity headers in the CORS
//! preflight allow-list.
//!
//! `tonic_web::enable` hard-codes its preflight `access-control-allow-headers`
//! to `x-grpc-web, content-type, x-user-agent, grpc-timeout`, and its internal
//! CORS layer is the one that actually answers browser preflights (the outer
//! transport-level `CorsLayer` never sees them). The dashboard sends
//! `x-ghostbridge-footprint` / `x-ghostbridge-trace-id` on every call, so the
//! stock preflight makes browsers refuse to send them and every request dies
//! at the identity interceptor. This mirrors `tonic_web::enable` with the
//! identity headers added.

use std::task::{Context, Poll};
use std::time::Duration;

use tonic::body::Body;
use tonic::codegen::http::{HeaderName, Request, Response};
use tonic::codegen::{Body as HttpBody, Bytes, Service, StdError};
use tonic::server::NamedService;
use tonic_web::{GrpcWebLayer, GrpcWebService};
use tower::Layer;
use tower_http::cors::{AllowOrigin, Cors, CorsLayer};

const MAX_AGE: Duration = Duration::from_secs(86400);

const ALLOW_HEADERS: [HeaderName; 8] = [
    HeaderName::from_static("x-grpc-web"),
    HeaderName::from_static("content-type"),
    HeaderName::from_static("x-user-agent"),
    HeaderName::from_static("grpc-timeout"),
    HeaderName::from_static("x-ghostbridge-footprint"),
    HeaderName::from_static("x-ghostbridge-genesis"),
    HeaderName::from_static("x-ghostbridge-trace-id"),
    HeaderName::from_static("x-wireguard-pubkey"),
];

const EXPOSE_HEADERS: [HeaderName; 3] = [
    HeaderName::from_static("grpc-status"),
    HeaderName::from_static("grpc-message"),
    HeaderName::from_static("grpc-status-details-bin"),
];

/// Drop-in replacement for `tonic_web::enable` with the Ghostbridge identity
/// headers added to the CORS preflight allow-list.
pub fn enable<S, ResBody>(service: S) -> GhostCorsGrpcWeb<S>
where
    S: Service<Request<Body>, Response = Response<ResBody>>,
    S: Clone + Send + 'static,
    S::Future: Send + 'static,
    ResBody: HttpBody<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<StdError> + std::fmt::Display,
{
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::mirror_request())
        .allow_credentials(true)
        .max_age(MAX_AGE)
        .expose_headers(EXPOSE_HEADERS.to_vec())
        .allow_headers(ALLOW_HEADERS.to_vec());

    GhostCorsGrpcWeb(cors.layer(GrpcWebLayer::new().layer(service)))
}

/// Newtype so the CORS-wrapped service still forwards [`NamedService`], same
/// role as tonic-web's own `CorsGrpcWeb`.
#[derive(Debug, Clone)]
pub struct GhostCorsGrpcWeb<S>(Cors<GrpcWebService<S>>);

impl<S, ResBody> Service<Request<Body>> for GhostCorsGrpcWeb<S>
where
    S: Service<Request<Body>, Response = Response<ResBody>>,
    S: Clone + Send + 'static,
    S::Future: Send + 'static,
    ResBody: HttpBody<Data = Bytes> + Send + 'static,
    ResBody::Error: Into<StdError> + std::fmt::Display,
{
    type Response = Response<Body>;
    type Error = S::Error;
    type Future = <Cors<GrpcWebService<S>> as Service<Request<Body>>>::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        <Cors<GrpcWebService<S>> as Service<Request<Body>>>::poll_ready(&mut self.0, cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        self.0.call(req)
    }
}

impl<S> NamedService for GhostCorsGrpcWeb<S>
where
    S: NamedService,
{
    const NAME: &'static str = S::NAME;
}
