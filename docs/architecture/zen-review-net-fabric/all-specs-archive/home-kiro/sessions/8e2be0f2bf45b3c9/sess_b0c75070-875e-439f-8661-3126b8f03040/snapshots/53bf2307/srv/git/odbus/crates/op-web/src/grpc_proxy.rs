//! Reverse-proxies gRPC-Web traffic from op-web's public :8080 to
//! op-grpc-bridge's loopback TLS door on :8090, so every browser client
//! resolves the gRPC target as same-origin.

use axum::{
    body::Body,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use std::sync::LazyLock;
use tracing::warn;

const GRPC_UPSTREAM: &str = "https://127.0.0.1:8090";

pub fn is_grpc_request(req: &axum::extract::Request<Body>) -> bool {
    req.headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.starts_with("application/grpc"))
}

static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .http1_only()
        .no_proxy()
        .build()
        .expect("grpc-web https client")
});

pub async fn proxy(req: axum::extract::Request<Body>) -> Response {
    let path_and_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");
    let url = format!("{GRPC_UPSTREAM}{path_and_query}");
    let method = req.method().clone();
    let headers = req.headers().clone();
    let body = req.into_body();

    let mut builder = CLIENT.request(
        reqwest::Method::from_bytes(method.as_str().as_bytes()).unwrap_or(reqwest::Method::POST),
        &url,
    );
    for (name, value) in headers.iter() {
        if name == header::HOST || name == header::CONTENT_LENGTH {
            continue;
        }
        builder = builder.header(name.as_str(), value.as_bytes());
    }
    builder = builder.body(reqwest::Body::wrap_stream(body.into_data_stream()));

    match builder.send().await {
        Ok(upstream) => {
            let status =
                StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            let mut response = Response::builder().status(status);
            for (name, value) in upstream.headers() {
                if let (Ok(n), Ok(v)) = (
                    axum::http::HeaderName::from_bytes(name.as_str().as_bytes()),
                    axum::http::HeaderValue::from_bytes(value.as_bytes()),
                ) {
                    response = response.header(n, v);
                }
            }
            match response.body(Body::from_stream(upstream.bytes_stream())) {
                Ok(resp) => resp,
                Err(err) => {
                    warn!(%err, "grpc-web proxy: failed to build response");
                    StatusCode::BAD_GATEWAY.into_response()
                }
            }
        }
        Err(err) => {
            warn!(%err, upstream = GRPC_UPSTREAM, "grpc-web proxy: upstream request failed");
            StatusCode::BAD_GATEWAY.into_response()
        }
    }
}
