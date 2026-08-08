//! Reverse-proxies gRPC-Web traffic from op-web's public :8080 to
//! op-grpc-bridge's loopback-only :8090, so every browser client resolves
//! the gRPC target as same-origin — no client-side hardcoded host/IP,
//! whether reached via localhost, the svc0/NetMaker mesh, or the public
//! domain. op-grpc-bridge itself stays loopback-only; this proxy is the
//! sole multi-interface path to it (mirrors the intended "gRPC socket is
//! the bridge that makes the sealed schema hot" pipeline — the bridge is
//! never exposed raw).
//!
//! Routing is by Content-Type, not a hand-maintained path-prefix allowlist.
//! An earlier version mirrored the dev-only prefix list in
//! `operation-dashboard-ui-07/vite.config.ts`'s `server.proxy`
//! (`/operation.v1`, `/operation.registry.v1`, ...) — that list turned out
//! to already be stale even there: every per-plugin generated method client
//! (`src/grpc/gen/plugin_methods/*.ts`, the dominant RPC pattern — one
//! `operation.method.<plugin>.<method>.<Method>Service` package per method)
//! was never covered by it, on neither the Vite dev proxy nor here. Since
//! new plugins/methods are added continuously and each mints a new package
//! name, any prefix list drifts stale again immediately. gRPC-Web requests
//! are unambiguously identifiable without knowing package names in advance:
//! they are always POST with a `Content-Type: application/grpc-web*`.

use axum::{
    body::Body,
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use hyper_util::client::legacy::{connect::HttpConnector, Client};
use hyper_util::rt::TokioExecutor;
use std::sync::LazyLock;
use tracing::warn;

const GRPC_UPSTREAM_AUTHORITY: &str = "127.0.0.1:8090";

/// True if `req` is a gRPC-Web call that should be forwarded to
/// op-grpc-bridge rather than served as a static SPA asset. Static assets
/// and index.html are always GET with non-`application/grpc*` content
/// types, so this never mis-routes the dashboard shell itself.
pub fn is_grpc_request(req: &axum::extract::Request<Body>) -> bool {
    req.headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.starts_with("application/grpc"))
}

static CLIENT: LazyLock<Client<HttpConnector, Body>> =
    LazyLock::new(|| Client::builder(TokioExecutor::new()).build_http());

/// Forward `req` to op-grpc-bridge unchanged apart from the URI authority,
/// streaming both request and response bodies (required for gRPC-Web
/// server-streaming RPCs).
pub async fn proxy(req: axum::extract::Request<Body>) -> Response {
    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");

    let uri: Uri = match format!("http://{GRPC_UPSTREAM_AUTHORITY}{path_and_query}").parse() {
        Ok(uri) => uri,
        Err(err) => {
            warn!(%err, "grpc-web proxy: failed to build upstream URI");
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };

    let (mut parts, body) = req.into_parts();
    parts.uri = uri;
    parts.headers.remove(axum::http::header::HOST);
    let upstream_req = axum::http::Request::from_parts(parts, body);

    match CLIENT.request(upstream_req).await {
        Ok(resp) => resp.into_response(),
        Err(err) => {
            warn!(%err, upstream = GRPC_UPSTREAM_AUTHORITY, "grpc-web proxy: upstream request failed");
            StatusCode::BAD_GATEWAY.into_response()
        }
    }
}
