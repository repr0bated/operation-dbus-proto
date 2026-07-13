//! gRPC-web reverse proxy with server-side Ghostbridge identity injection
//! and JSON-to-binary protobuf translation.
//!
//! Browser dashboards speak gRPC-web+json to the same origin (`:8080`). They
//! cannot present the `x-ghostbridge-footprint` / `x-ghostbridge-trace-id`
//! headers the bridge's `GhostbridgeInterceptor` requires (derived from the
//! live `IdentitySled` in shared memory). The bridge only accepts
//! `application/grpc-web+proto` (binary). This middleware:
//!
//! 1. Intercepts gRPC-web traffic, authorizes by `AccessZone`.
//! 2. Reads the live sled footprint/trace and injects identity headers.
//! 3. For `grpc-web+json`: translates JSON ↔ binary protobuf using
//!    `prost-reflect` `DynamicMessage` and the compiled `FILE_DESCRIPTOR_SET`.
//! 4. For `grpc-web+proto`: passes through directly.
//! 5. Forwards to the local bridge at `127.0.0.1:8090`, streaming the response.
//!
//! Streaming responses are closed after `STREAM_IDLE_TIMEOUT` of inactivity
//! to avoid exhausting the browser's HTTP/1.1 connection limit (6 per origin).
//! The UI auto-reconnects via its exponential-backoff scheduler.

use std::sync::OnceLock;
use std::time::Duration;

use axum::{
    body::Body,
    extract::Request,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};
use op_core::security::AccessZone;
use prost_reflect::{prost::Message, DescriptorPool, DynamicMessage, MessageDescriptor};
use serde::de::DeserializeSeed;

/// Local full-data-plane bridge (zeroclaw listener, enforces the interceptor).
const UPSTREAM: &str = "http://127.0.0.1:8090";

/// Max time to wait for the next frame from the upstream before closing the
/// streaming response. Prevents the browser's HTTP/1.1 connection pool (6 per
/// origin) from being exhausted by idle server-streaming subscriptions.
const STREAM_IDLE_TIMEOUT: Duration = Duration::from_millis(500);

/// Headers to strip from the upstream response (let the CORS layer handle CORS,
/// don't leak identity headers to the browser).
const STRIP_RESPONSE_HEADERS: &[&str] = &[
    "access-control-allow-origin",
    "access-control-allow-credentials",
    "access-control-allow-methods",
    "access-control-allow-headers",
    "access-control-expose-headers",
    "access-control-max-age",
    "x-ghostbridge-footprint",
    "x-ghostbridge-trace-id",
    "x-wireguard-pubkey",
    "vary",
];

fn shared_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}

fn descriptor_pool() -> &'static DescriptorPool {
    static POOL: OnceLock<DescriptorPool> = OnceLock::new();
    POOL.get_or_init(|| {
        DescriptorPool::decode(op_grpc_bridge::proto::FILE_DESCRIPTOR_SET)
            .expect("FILE_DESCRIPTOR_SET must decode")
    })
}

/// Look up the input and output `MessageDescriptor` for a gRPC service/method.
fn lookup_method(
    service_full_name: &str,
    method_name: &str,
) -> Option<(MessageDescriptor, MessageDescriptor)> {
    let pool = descriptor_pool();
    let service = pool.get_service_by_name(service_full_name)?;
    let method = service.methods().find(|m| m.name() == method_name)?;
    Some((method.input(), method.output()))
}

fn is_grpc_web(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.starts_with("application/grpc-web"))
        .unwrap_or(false)
}

fn is_grpc_web_json(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.contains("json"))
        .unwrap_or(false)
}

/// Read the live identity sled and return `(footprint_hex, trace_id_hex)`.
fn read_identity() -> Result<(String, String), String> {
    let (ptr, _mmap) = op_identity::read_sled().map_err(|e| format!("sled unreadable: {e}"))?;
    // Safety: `_mmap` is held for the duration of this borrow; no concurrent
    // writer mutates the sled mid-read (writes use atomic rename).
    let sled = unsafe { &*ptr };
    if !sled.is_sled_valid() {
        return Err("identity sled is not valid (zero footprint/trace)".to_string());
    }
    Ok((hex::encode(sled.hashed_footprint), sled.trace_id_hex()))
}

/// Build a gRPC-web error response (HTTP 200 + grpc-status trailer frame).
fn grpc_error(content_type: &str, grpc_status: u8, message: &str) -> Response {
    let trailer = format!(
        "grpc-status: {}\r\ngrpc-message: {}\r\n",
        grpc_status,
        message.replace('\n', " ")
    );
    let trailer_bytes = trailer.as_bytes();
    let mut frame = Vec::with_capacity(5 + trailer_bytes.len());
    frame.push(0x80); // trailer frame flag
    let len = trailer_bytes.len() as u32;
    frame.extend_from_slice(&len.to_be_bytes());
    frame.extend_from_slice(trailer_bytes);

    let mut response = Response::new(Body::from(frame));
    *response.status_mut() = StatusCode::OK;
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(content_type)
            .unwrap_or(HeaderValue::from_static("application/grpc-web+proto")),
    );
    response
}

/// Translate a gRPC-web+json request body into gRPC-web+proto binary.
fn translate_request(path: &str, body_bytes: &[u8]) -> Result<Vec<u8>, (u8, String)> {
    let (service, method) = match path.strip_prefix('/').and_then(|p| p.split_once('/')) {
        Some((svc, mth)) => (svc, mth),
        None => return Err((3, format!("invalid gRPC path: {path}"))),
    };

    let (input_desc, _output_desc) = lookup_method(service, method)
        .ok_or_else(|| (5, format!("service/method not found: {service}/{method}")))?;

    if body_bytes.len() < 5 {
        return Err((3, "request body too short for gRPC-web frame".to_string()));
    }
    let payload_len =
        u32::from_be_bytes([body_bytes[1], body_bytes[2], body_bytes[3], body_bytes[4]]) as usize;
    if body_bytes.len() < 5 + payload_len {
        return Err((3, "truncated gRPC-web frame".to_string()));
    }
    let json_payload = &body_bytes[5..5 + payload_len];

    if json_payload.is_empty() {
        return Ok(make_grpc_web_frame(&[]));
    }

    let json_str = std::str::from_utf8(json_payload)
        .map_err(|_| (3, "request JSON is not valid UTF-8".to_string()))?;

    let mut deserializer = serde_json::de::Deserializer::from_str(json_str);
    let msg: DynamicMessage = input_desc
        .deserialize(&mut deserializer)
        .map_err(|e| (3, format!("JSON deserialization failed: {e}")))?;
    deserializer
        .end()
        .map_err(|e| (3, format!("trailing data in JSON: {e}")))?;

    let binary = msg.encode_to_vec();
    Ok(make_grpc_web_frame(&binary))
}

/// Build a gRPC-web data frame (flag 0x00 + 4-byte length + payload).
fn make_grpc_web_frame(payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(5 + payload.len());
    frame.push(0x00);
    let len = payload.len() as u32;
    frame.extend_from_slice(&len.to_be_bytes());
    frame.extend_from_slice(payload);
    frame
}

/// Translate a single binary protobuf data frame into a JSON gRPC-web data frame.
fn translate_data_frame(output_desc: &MessageDescriptor, data: &[u8]) -> Result<Vec<u8>, String> {
    let msg = DynamicMessage::decode(output_desc.clone(), data)
        .map_err(|e| format!("protobuf decode failed: {e}"))?;
    let json = serde_json::to_vec(&msg).map_err(|e| format!("JSON encode failed: {e}"))?;
    Ok(make_grpc_web_frame(&json))
}

/// Parse a gRPC-web frame from a byte buffer.
/// Returns `(flag, payload, consumed)` if a complete frame is available.
fn parse_frame(buf: &[u8]) -> Option<(u8, &[u8], usize)> {
    if buf.len() < 5 {
        return None;
    }
    let flag = buf[0];
    let len = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]) as usize;
    if buf.len() < 5 + len {
        return None;
    }
    Some((flag, &buf[5..5 + len], 5 + len))
}

/// Build a trailers-only gRPC-web frame (flag 0x80 + trailer text).
fn make_trailer_frame(status: u8, message: &str) -> Vec<u8> {
    let trailer = format!("grpc-status: {status}\r\ngrpc-message: {}\r\n", message);
    let tb = trailer.as_bytes();
    let mut frame = Vec::with_capacity(5 + tb.len());
    frame.push(0x80);
    frame.extend_from_slice(&(tb.len() as u32).to_be_bytes());
    frame.extend_from_slice(tb);
    frame
}

pub async fn grpc_web_proxy_middleware(request: Request, next: Next) -> Response {
    if !is_grpc_web(request.headers()) {
        return next.run(request).await;
    }

    let is_json = is_grpc_web_json(request.headers());
    let response_ct = if is_json {
        "application/grpc-web+json"
    } else {
        "application/grpc-web+proto"
    };

    // Authorization: local/mesh callers may borrow the host identity directly.
    // Public callers (e.g. Lovable cloud) must present an x-ghostbridge-token
    // header matching the sled's hashed footprint (self-proving: the token IS
    // the footprint, which only the operator and the host know).
    let zone = request
        .extensions()
        .get::<AccessZone>()
        .copied()
        .unwrap_or(AccessZone::Public);

    let (footprint, trace) = match read_identity() {
        Ok(v) => v,
        Err(msg) => return grpc_error(response_ct, 9, &msg),
    };

    match zone {
        AccessZone::Localhost | AccessZone::TrustedMesh => {
            // Full access for local/mesh callers.
        }
        AccessZone::Public | AccessZone::PrivateNetwork => {
            // Public callers need the footprint token.
            let token = request
                .headers()
                .get("x-ghostbridge-token")
                .and_then(|v| v.to_str().ok());
            if token != Some(&footprint) {
                return grpc_error(
                    response_ct,
                    7,
                    "Ghostbridge: token required for external access",
                );
            }
        }
    }

    let (parts, body) = request.into_parts();
    let path = parts.uri.path();
    let path_and_query = parts
        .uri
        .path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or(path);
    let url = format!("{UPSTREAM}{path_and_query}");

    let body_bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(_) => return grpc_error(response_ct, 3, "failed to read request body"),
    };

    let (upstream_body, _upstream_ct) = if is_json {
        match translate_request(path, &body_bytes) {
            Ok(translated) => (translated, "application/grpc-web+proto"),
            Err((code, msg)) => return grpc_error(response_ct, code, &msg),
        }
    } else {
        (body_bytes.to_vec(), "application/grpc-web+proto")
    };

    let output_desc = if is_json {
        if let Some((service, method)) = path.strip_prefix('/').and_then(|p| p.split_once('/')) {
            lookup_method(service, method).map(|(_, out)| out)
        } else {
            None
        }
    } else {
        None
    };

    let mut upstream_headers = reqwest::header::HeaderMap::new();
    for (name, value) in parts.headers.iter() {
        if matches!(
            name.as_str(),
            "host"
                | "content-length"
                | "connection"
                | "accept-encoding"
                | "content-type"
                | "accept"
        ) {
            continue;
        }
        if let (Ok(hn), Ok(hv)) = (
            reqwest::header::HeaderName::from_bytes(name.as_str().as_bytes()),
            reqwest::header::HeaderValue::from_bytes(value.as_bytes()),
        ) {
            upstream_headers.insert(hn, hv);
        }
    }
    upstream_headers.insert(
        reqwest::header::CONTENT_TYPE,
        reqwest::header::HeaderValue::from_static("application/grpc-web+proto"),
    );
    upstream_headers.insert(
        reqwest::header::ACCEPT,
        reqwest::header::HeaderValue::from_static("application/grpc-web+proto"),
    );
    if let Ok(v) = reqwest::header::HeaderValue::from_str(&footprint) {
        upstream_headers.insert("x-ghostbridge-footprint", v);
    }
    if let Ok(v) = reqwest::header::HeaderValue::from_str(&trace) {
        upstream_headers.insert("x-ghostbridge-trace-id", v);
    }

    let upstream = match shared_client()
        .post(&url)
        .headers(upstream_headers)
        .body(upstream_body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return grpc_error(
                response_ct,
                14,
                &format!("upstream bridge unreachable: {e}"),
            )
        }
    };

    let status = StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::OK);

    // Build filtered response headers (strip CORS + identity headers).
    let resp_headers = filter_upstream_headers(upstream.headers(), response_ct);

    if !is_json || output_desc.is_none() {
        // No translation needed: stream the raw response back with idle timeout.
        let stream = upstream.bytes_stream();
        let idle_capped = idle_timeout_stream(stream, response_ct);
        let mut response = Response::new(Body::from_stream(idle_capped));
        *response.status_mut() = status;
        copy_headers(&mut response, &resp_headers);
        return response;
    }

    // JSON translation path: transform the binary protobuf response stream
    // into JSON gRPC-web frames frame-by-frame, with idle timeout.
    let output_desc = output_desc.unwrap();
    let upstream_stream = upstream.bytes_stream();

    let translated = async_stream::stream! {
        let mut buf: Vec<u8> = Vec::new();
        let mut stream = std::pin::pin!(upstream_stream);

        use futures::StreamExt;
        loop {
            // Try to parse and translate a complete frame from the buffer.
            if let Some((flag, payload, consumed)) = parse_frame(&buf) {
                if flag & 0x80 != 0 {
                    // Trailer frame — pass through as-is.
                    yield Ok::<Vec<u8>, std::convert::Infallible>(
                        buf[..consumed].to_vec(),
                    );
                } else {
                    // Data frame — translate binary protobuf to JSON.
                    match translate_data_frame(&output_desc, payload) {
                        Ok(json_frame) => yield Ok(json_frame),
                        Err(e) => {
                            yield Ok(make_trailer_frame(2, &e));
                            return;
                        }
                    }
                }
                buf.drain(..consumed);
                continue;
            }

            // Need more bytes from the upstream, with idle timeout.
            match tokio::time::timeout(STREAM_IDLE_TIMEOUT, stream.next()).await {
                Ok(Some(Ok(chunk))) => buf.extend_from_slice(&chunk),
                Ok(Some(Err(e))) => {
                    yield Ok(make_trailer_frame(14, &format!("upstream read error: {e}")));
                    return;
                }
                Ok(None) => break, // Stream ended
                Err(_) => {
                    // Idle timeout — close the stream gracefully so the
                    // browser frees the connection. The UI auto-reconnects.
                    yield Ok(make_trailer_frame(0, "stream idle timeout"));
                    return;
                }
            }
        }

        if !buf.is_empty() {
            yield Ok(buf);
        }
    };

    let body = Body::from_stream(translated);
    let mut response = Response::new(body);
    *response.status_mut() = status;
    copy_headers(&mut response, &resp_headers);
    response
}

/// Wrap a raw byte stream with an idle timeout. If no data arrives within
/// `STREAM_IDLE_TIMEOUT`, yields a trailers-only gRPC-web frame with
/// `grpc-status: 0` and ends the stream.
fn idle_timeout_stream(
    stream: impl futures::Stream<Item = Result<bytes::Bytes, reqwest::Error>>,
    content_type: &str,
) -> impl futures::Stream<Item = Result<Vec<u8>, std::convert::Infallible>> {
    let ct = content_type.to_string();
    async_stream::stream! {
        let mut stream = std::pin::pin!(stream);
        use futures::StreamExt;
        loop {
            match tokio::time::timeout(STREAM_IDLE_TIMEOUT, stream.next()).await {
                Ok(Some(Ok(chunk))) => yield Ok(chunk.to_vec()),
                Ok(Some(Err(e))) => {
                    yield Ok(make_trailer_frame(14, &format!("upstream error: {e}")));
                    return;
                }
                Ok(None) => return,
                Err(_) => {
                    // Idle timeout — send a trailer to close gracefully.
                    // For grpc-web+proto we need a proper trailer frame.
                    let _ = &ct; // content_type not needed in trailer
                    yield Ok(make_trailer_frame(0, "stream idle timeout"));
                    return;
                }
            }
        }
    }
}

/// Filter upstream response headers: remove CORS and identity headers (the
/// CORS layer handles CORS; identity headers must not leak to the browser).
fn filter_upstream_headers(
    src: &reqwest::header::HeaderMap,
    response_ct: &str,
) -> reqwest::header::HeaderMap {
    let mut filtered = reqwest::header::HeaderMap::new();
    for (name, value) in src.iter() {
        let name_str = name.as_str();
        if STRIP_RESPONSE_HEADERS.contains(&name_str) {
            continue;
        }
        if name_str == "content-length" || name_str == "transfer-encoding" {
            continue;
        }
        if let (Ok(hn), Ok(hv)) = (
            reqwest::header::HeaderName::from_bytes(name_str.as_bytes()),
            reqwest::header::HeaderValue::from_bytes(value.as_bytes()),
        ) {
            filtered.insert(hn, hv);
        }
    }
    // Set the correct content-type for the client.
    filtered.insert(
        reqwest::header::CONTENT_TYPE,
        reqwest::header::HeaderValue::from_str(response_ct).unwrap(),
    );
    filtered
}

fn copy_headers(response: &mut Response, src: &reqwest::header::HeaderMap) {
    let out = response.headers_mut();
    for (name, value) in src.iter() {
        if let (Ok(hn), Ok(hv)) = (
            axum::http::HeaderName::from_bytes(name.as_str().as_bytes()),
            axum::http::HeaderValue::from_bytes(value.as_bytes()),
        ) {
            out.append(hn, hv);
        }
    }
}
