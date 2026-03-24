//! Security middleware for op-web
//!
//! Provides IP-based security zones with API key bypass support.

use axum::{
    extract::{ConnectInfo, Request},
    http::HeaderMap,
    middleware::Next,
    response::Response,
};
use op_core::security::AccessZone;
use std::net::SocketAddr;
use tracing::{debug, info};

/// API keys that bypass IP restrictions and grant TrustedMesh access
const BYPASS_API_KEYS: &[&str] = &[
    "4f8c2b5d-9a1e-4b7c-8d2f-3a6b5c9e4d1f", // Primary MCP access key
    "test-key-huggingface-2024",            // Hugging Face test key
];

/// Check for API key in headers that bypasses IP restrictions
fn check_bypass_api_key(headers: &HeaderMap) -> Option<&'static str> {
    // Check X-API-Key header
    if let Some(key) = headers.get("x-api-key").and_then(|v| v.to_str().ok()) {
        let key = key.trim();
        for &bypass_key in BYPASS_API_KEYS {
            if key == bypass_key {
                return Some(bypass_key);
            }
        }
    }

    // Check Authorization: Bearer <key> header
    if let Some(auth) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
        if let Some(key) = auth.trim().strip_prefix("Bearer ") {
            let key = key.trim();
            for &bypass_key in BYPASS_API_KEYS {
                if key == bypass_key {
                    return Some(bypass_key);
                }
            }
        }
    }

    // Check x-op-mcp-token header
    if let Some(key) = headers.get("x-op-mcp-token").and_then(|v| v.to_str().ok()) {
        let key = key.trim();
        for &bypass_key in BYPASS_API_KEYS {
            if key == bypass_key {
                return Some(bypass_key);
            }
        }
    }

    None
}

fn extract_auth_token(headers: &HeaderMap) -> Option<String> {
    if let Some(raw) = headers.get("x-op-mcp-token").and_then(|v| v.to_str().ok()) {
        let token = raw.trim();
        if !token.is_empty() {
            return Some(token.to_string());
        }
    }

    if let Some(raw) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
        let trimmed = raw.trim();
        if let Some(bearer) = trimmed.strip_prefix("Bearer ") {
            let token = bearer.trim();
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }

    None
}

/// Extract IP from headers or connection info
pub fn extract_ip(headers: &HeaderMap, addr: Option<&SocketAddr>) -> String {
    // 1. Check X-Forwarded-For (standard proxy header)
    if let Some(forwarded) = headers.get("x-forwarded-for") {
        if let Ok(s) = forwarded.to_str() {
            if let Some(client_ip) = s.split(',').next() {
                return client_ip.trim().to_string();
            }
        }
    }

    // 2. Check X-Real-IP (nginx convention)
    if let Some(real_ip) = headers.get("x-real-ip") {
        if let Ok(s) = real_ip.to_str() {
            return s.trim().to_string();
        }
    }

    // 3. Fallback to socket address
    if let Some(addr) = addr {
        return addr.ip().to_string();
    }

    "0.0.0.0".to_string()
}

/// Middleware to identify Client IP and attach AccessZone to the request
///
/// Security Logic:
/// 1. If request has a valid bypass API key -> TrustedMesh (full access)
/// 2. Otherwise, determine zone from IP address
pub async fn ip_security_middleware(
    connect_info: Option<ConnectInfo<SocketAddr>>,
    mut request: Request,
    next: Next,
) -> Response {
    let headers = request.headers();
    let addr = connect_info.map(|ci| ci.0);
    let client_ip = extract_ip(headers, addr.as_ref());

    // Check for bypass API key first
    let zone = if let Some(key) = check_bypass_api_key(headers) {
        info!(
            "API key bypass granted: IP={} key={}...{}",
            client_ip,
            &key[..8],
            &key[key.len() - 4..]
        );
        AccessZone::TrustedMesh
    } else {
        // Determine zone from IP
        let zone = AccessZone::from_ip(&client_ip);
        debug!("Request from IP: {} [Zone: {:?}]", client_ip, zone);
        zone
    };

    // Attach AccessZone to the request extensions
    request.extensions_mut().insert(zone);

    next.run(request).await
}
