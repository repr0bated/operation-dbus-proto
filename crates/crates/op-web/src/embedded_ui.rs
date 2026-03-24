//! Embedded UI handler using rust-embed
//!
//! Serves the React SPA from compiled-in assets.
//! No external static files - single binary deployment.

use axum::{
    body::Body,
    http::{header, StatusCode, Uri},
    response::{IntoResponse, Response},
};
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
    let path = uri.path().trim_start_matches('/');

    // Try exact path first
    if let Some(content) = UiAssets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();

        // Set cache headers based on file type
        let cache_control = if path.starts_with("assets/") {
            // Hashed assets can be cached forever
            "public, max-age=31536000, immutable"
        } else if path == "index.html" {
            // HTML should be revalidated
            "no-cache"
        } else {
            // Other assets cached for 1 day
            "public, max-age=86400"
        };

        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime.as_ref())
            .header(header::CACHE_CONTROL, cache_control)
            .header("X-Content-Type-Options", "nosniff")
            .header("Referrer-Policy", "strict-origin-when-cross-origin")
            .body(Body::from(content.data.into_owned()))
            .unwrap();
    }

    // SPA fallback: serve index.html for client-side routing
    if let Some(content) = UiAssets::get("index.html") {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
            .header(header::CACHE_CONTROL, "no-cache")
            .header("X-Content-Type-Options", "nosniff")
            .header("Referrer-Policy", "strict-origin-when-cross-origin")
            .body(Body::from(content.data.into_owned()))
            .unwrap();
    }

    // No UI built yet - return helpful message
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(
            r#"
<!DOCTYPE html>
<html>
<head><title>op-web UI</title></head>
<body style="font-family: system-ui; padding: 2rem; background: #1a1a1a; color: #fff;">
    <h1>UI Not Built</h1>
    <p>The embedded UI has not been built yet.</p>
    <p>To build the UI:</p>
    <pre style="background: #333; padding: 1rem; border-radius: 4px;">
cd crates/op-web/ui
npm ci
npm run build
    </pre>
    <p>Then rebuild op-web:</p>
    <pre style="background: #333; padding: 1rem; border-radius: 4px;">
cargo build -p op-web
    </pre>
</body>
</html>
"#,
        ))
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
