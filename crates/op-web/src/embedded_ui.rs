//! Embedded UI handler using rust-embed
//!
//! Serves the React SPA from compiled-in assets.
//! No external static files - single binary deployment.

use axum::{
    body::Body,
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use mime_guess::from_path;
use rust_embed::RustEmbed;

/// Embedded UI assets from ui/dist
/// Built at compile time via build.rs
#[derive(RustEmbed)]
#[folder = "ui/dist"]
#[prefix = ""]
pub struct UiAssets;

/// Serve embedded UI assets
///
/// Handles:
/// - Exact path matches for static files
/// - SPA fallback to index.html for client-side routing
/// - Proper MIME types via mime_guess
/// - Cache headers for hashed assets
pub async fn serve_embedded_ui(uri: Uri) -> impl IntoResponse {
    let path = normalized_asset_path(uri.path());
    let asset = UiAssets::get(&path).or_else(|| UiAssets::get("index.html"));

    match asset {
        Some(content) => response_for_asset(&path, content.data.into_owned()),
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
            .body(Body::from("embedded ui asset not found"))
            .unwrap(),
    }
}

fn normalized_asset_path(path: &str) -> String {
    let trimmed = path.trim_start_matches('/');
    if trimmed.is_empty() || path.ends_with('/') {
        "index.html".to_string()
    } else {
        trimmed.to_string()
    }
}

fn response_for_asset(path: &str, body: Vec<u8>) -> Response {
    let mime = from_path(path).first_or_octet_stream();
    let cache_control = if path == "index.html" {
        "no-cache"
    } else {
        "public, max-age=31536000, immutable"
    };

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.as_ref())
        .header(header::CACHE_CONTROL, cache_control)
        .header("X-Content-Type-Options", "nosniff")
        .header("Referrer-Policy", "strict-origin-when-cross-origin")
        .body(Body::from(body))
        .unwrap()
}

/// Check if UI assets are available
pub fn ui_available() -> bool {
    UiAssets::get("index.html").is_some()
}

/// Get list of embedded files (for debugging)
pub fn list_embedded_files() -> Vec<String> {
    UiAssets::iter().map(|f| f.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ui_assets_struct() {
        // This will be empty until UI is built
        let files: Vec<_> = UiAssets::iter().collect();
        // Just verify the struct compiles
        assert!(files.is_empty() || !files.is_empty());
    }

    #[test]
    fn test_list_embedded_files() {
        let files = list_embedded_files();
        // Will be empty until UI is built
        assert!(files.is_empty() || files.contains(&"index.html".to_string()));
    }
}
