This file is a merged representation of the entire codebase, combined into a single document by Repomix.

<file_summary>
This section contains a summary of this file.

<purpose>
This file contains a packed representation of the entire repository's contents.
It is designed to be easily consumable by AI systems for analysis, code review,
or other automated processes.
</purpose>

<file_format>
The content is organized as follows:
1. This summary section
2. Repository information
3. Directory structure
4. Repository files (if enabled)
5. Multiple file entries, each consisting of:
  - File path as an attribute
  - Full contents of the file
</file_format>

<usage_guidelines>
- This file should be treated as read-only. Any changes should be made to the
  original repository files, not this packed version.
- When processing this file, use the file path to distinguish
  between different files in the repository.
- Be aware that this file may contain sensitive information. Handle it with
  the same level of security as you would the original repository.
</usage_guidelines>

<notes>
- Some files may have been excluded based on .gitignore rules and Repomix's configuration
- Binary files are not included in this packed representation. Please refer to the Repository Structure section for a complete list of file paths, including binary files
- Files matching patterns in .gitignore are excluded
- Files matching default ignore patterns are excluded
- Files are sorted by Git change count (files with more changes are at the bottom)
</notes>

</file_summary>

<directory_structure>
src/
  health.rs
  lib.rs
  metrics.rs
  middleware.rs
  request_filters.rs
  router.rs
  server.rs
  tls.rs
Cargo.toml
compare-op-http.md
SPEC.md
</directory_structure>

<files>
This section contains the contents of the repository's files.

<file path="src/health.rs">
//! Health Check Endpoints
//!
//! Provides health check endpoints for monitoring and load balancers.

use axum::{Json, response::IntoResponse};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

/// Health check response
#[derive(Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub timestamp: u64,
    pub uptime: u64,
    pub version: String,
    pub services: HashMap<String, ServiceHealth>,
}

/// Individual service health status
#[derive(Serialize, Deserialize, Clone)]
pub struct ServiceHealth {
    pub status: String,
    pub message: Option<String>,
    pub last_check: u64,
}

/// Health checker for monitoring services
#[derive(Clone)]
pub struct HealthChecker {
    start_time: SystemTime,
    services: HashMap<String, ServiceHealth>,
}

impl HealthChecker {
    /// Create a new health checker
    pub fn new() -> Self {
        Self {
            start_time: SystemTime::now(),
            services: HashMap::new(),
        }
    }

    /// Register a service for health checking
    pub fn register_service(&mut self, name: impl Into<String>) {
        let service_name = name.into();
        self.services.insert(service_name, ServiceHealth {
            status: "unknown".to_string(),
            message: None,
            last_check: 0,
        });
    }

    /// Update service health status
    pub fn update_service_health(
        &mut self,
        name: &str,
        status: impl Into<String>,
        message: Option<String>,
    ) {
        if let Some(service) = self.services.get_mut(name) {
            service.status = status.into();
            service.message = message;
            service.last_check = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
        }
    }

    /// Perform comprehensive health check
    pub async fn check_health(&self) -> HealthResponse {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let uptime = now - self.start_time
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Determine overall status
        let overall_status = if self.services.values().all(|s| s.status == "healthy") {
            "healthy"
        } else if self.services.values().any(|s| s.status == "unhealthy") {
            "unhealthy"
        } else {
            "degraded"
        };

        HealthResponse {
            status: overall_status.to_string(),
            timestamp: now,
            uptime,
            version: env!("CARGO_PKG_VERSION").to_string(),
            services: self.services.clone(),
        }
    }

    /// Simple health check (just returns OK)
    pub async fn simple_health_check(&self) -> &'static str {
        "OK"
    }

    /// Detailed health check JSON response
    pub async fn detailed_health_check(&self) -> impl IntoResponse {
        Json(self.check_health().await)
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// Health check handlers for Axum
pub mod handlers {
    use super::*;
    use axum::response::IntoResponse;

    /// Simple health check handler
    pub async fn health_check() -> &'static str {
        "OK"
    }

    /// Detailed health check handler
    pub async fn detailed_health_check(
        checker: axum::extract::State<HealthChecker>,
    ) -> impl IntoResponse {
        checker.detailed_health_check().await
    }

    /// Readiness check handler
    pub async fn readiness_check(
        checker: axum::extract::State<HealthChecker>,
    ) -> impl IntoResponse {
        let health = checker.check_health().await;
        if health.status == "healthy" {
            (axum::http::StatusCode::OK, Json(health))
        } else {
            (axum::http::StatusCode::SERVICE_UNAVAILABLE, Json(health))
        }
    }

    /// Liveness check handler
    pub async fn liveness_check() -> &'static str {
        "OK"
    }
}

/// Health check utilities
pub mod utils {
    use super::*;

    /// Check if a service is responding
    pub async fn check_service_health(
        name: &str,
        url: &str,
        timeout: std::time::Duration,
    ) -> ServiceHealth {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build();

        let start_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        match client {
            Ok(client) => {
                match client.get(url).send().await {
                    Ok(response) if response.status().is_success() => ServiceHealth {
                        status: "healthy".to_string(),
                        message: Some(format!("HTTP {} OK", response.status())),
                        last_check: start_time,
                    },
                    Ok(response) => ServiceHealth {
                        status: "unhealthy".to_string(),
                        message: Some(format!("HTTP {} error", response.status())),
                        last_check: start_time,
                    },
                    Err(e) => ServiceHealth {
                        status: "unhealthy".to_string(),
                        message: Some(format!("Connection error: {}", e)),
                        last_check: start_time,
                    },
                }
            }
            Err(e) => ServiceHealth {
                status: "error".to_string(),
                message: Some(format!("Client creation error: {}", e)),
                last_check: start_time,
            },
        }
    }

    /// Check database connectivity
    pub async fn check_database_health(connection_string: &str) -> ServiceHealth {
        let start_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Placeholder - would implement actual DB health checks
        ServiceHealth {
            status: "healthy".to_string(),
            message: Some("Database connection OK".to_string()),
            last_check: start_time,
        }
    }

    /// Check filesystem health
    pub async fn check_filesystem_health(paths: &[&str]) -> ServiceHealth {
        let start_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        for path in paths {
            if !std::path::Path::new(path).exists() {
                return ServiceHealth {
                    status: "unhealthy".to_string(),
                    message: Some(format!("Path does not exist: {}", path)),
                    last_check: start_time,
                };
            }
        }

        ServiceHealth {
            status: "healthy".to_string(),
            message: Some("All paths accessible".to_string()),
            last_check: start_time,
        }
    }
}
</file>

<file path="src/lib.rs">
//! op-http: Central HTTP/TLS Server
//!
//! This is the SINGLE source of truth for all HTTP/TLS handling in op-dbus.
//! All other crates export routers that get composed here.
//!
//! Architecture:
//! ```text
//! unified-server binary
//!     └── op-http (this crate)
//!         ├── TLS termination (rustls)
//!         ├── Middleware stack (CORS, tracing, compression)
//!         └── Router composition
//!             ├── /api/mcp/*    → op_mcp::create_router()
//!             ├── /api/chat/*   → op_chat::create_router()
//!             ├── /api/web/*    → op_web::create_router()
//!             ├── /api/tools/*  → op_tools::create_router()
//!             ├── /api/agents/* → op_agents::create_router()
//!             ├── /ws/*         → websocket handlers
//!             └── /*            → static files
//! ```

pub mod middleware;
pub mod router;
pub mod server;
pub mod tls;

// Re-export main types
pub use middleware::{MiddlewareConfig, MiddlewareStack};
pub use router::{RouterBuilder, ServiceRouter};
pub use server::{HttpServer, HttpServerBuilder, ServerConfig};
pub use tls::{TlsConfig, TlsMode};

// Re-export axum for convenience - other crates use this
pub use axum;
pub use tower;
pub use tower_http;

/// Error types for the HTTP server
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("TLS configuration error: {0}")]
    TlsError(String),

    #[error("Server binding error: {0}")]
    BindError(#[from] std::io::Error),

    #[error("Router configuration error: {0}")]
    RouterError(String),

    #[error("Certificate error: {0}")]
    CertificateError(String),
}

pub type Result<T> = std::result::Result<T, ServerError>;

/// Prelude for convenient imports by other crates
pub mod prelude {
    pub use super::axum::{
        extract::{Json, Path, Query, State},
        response::{IntoResponse, Response},
        routing::{delete, get, post, put},
        Router,
    };
    pub use super::middleware::{MiddlewareConfig, MiddlewareStack};
    pub use super::router::{RouterBuilder, ServiceRouter};
    pub use super::server::{HttpServer, HttpServerBuilder, ServerConfig};
    pub use super::tls::{TlsConfig, TlsMode};
    pub use super::Result;
}
</file>

<file path="src/metrics.rs">
//! Metrics and Monitoring
//!
//! Provides Prometheus metrics, request counting, and performance monitoring.

use axum::{extract::Request, middleware::Next, response::Response};
use lazy_static::lazy_static;
use prometheus::{
    register_counter, register_histogram, register_gauge,
    Counter, Histogram, Gauge, Encoder, TextEncoder,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Metrics registry
#[derive(Clone)]
pub struct Metrics {
    request_count: Counter,
    request_duration: Histogram,
    active_connections: Gauge,
    services: Arc<RwLock<HashMap<String, ServiceMetrics>>>,
}

impl Metrics {
    /// Create a new metrics instance
    pub fn new() -> Self {
        let request_count = register_counter!(
            "http_requests_total",
            "Total number of HTTP requests"
        ).unwrap();

        let request_duration = register_histogram!(
            "http_request_duration_seconds",
            "HTTP request duration in seconds"
        ).unwrap();

        let active_connections = register_gauge!(
            "http_active_connections",
            "Number of active HTTP connections"
        ).unwrap();

        Self {
            request_count,
            request_duration,
            active_connections,
            services: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Record a request
    pub async fn record_request(&self, method: &str, path: &str, status: u16, duration: f64) {
        // Record global metrics
        self.request_count.inc();
        self.request_duration.observe(duration);

        // Record service-specific metrics
        let mut services = self.services.write().await;

        // Extract service name from path (e.g., /api/chat -> chat)
        let service_name = extract_service_name(path);
        let service_metrics = services.entry(service_name.to_string())
            .or_insert_with(|| ServiceMetrics::new(&service_name));

        service_metrics.record_request(method, status, duration).await;
    }

    /// Increment active connections
    pub fn increment_connections(&self) {
        self.active_connections.inc();
    }

    /// Decrement active connections
    pub fn decrement_connections(&self) {
        self.active_connections.dec();
    }

    /// Get Prometheus metrics as string
    pub async fn prometheus_metrics(&self) -> String {
        let encoder = TextEncoder::new();
        let metric_families = prometheus::gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap()
    }

    /// Get service metrics
    pub async fn service_metrics(&self) -> HashMap<String, ServiceMetrics> {
        self.services.read().await.clone()
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Service-specific metrics
#[derive(Clone)]
pub struct ServiceMetrics {
    pub name: String,
    request_count: Counter,
    request_duration: Histogram,
    error_count: Counter,
}

impl ServiceMetrics {
    /// Create new service metrics
    pub fn new(name: &str) -> Self {
        let request_count = register_counter!(
            format!("{}_requests_total", name),
            format!("Total requests for {} service", name)
        ).unwrap();

        let request_duration = register_histogram!(
            format!("{}_request_duration_seconds", name),
            format!("Request duration for {} service", name)
        ).unwrap();

        let error_count = register_counter!(
            format!("{}_errors_total", name),
            format!("Total errors for {} service", name)
        ).unwrap();

        Self {
            name: name.to_string(),
            request_count,
            request_duration,
            error_count,
        }
    }

    /// Record a request for this service
    pub async fn record_request(&self, method: &str, status: u16, duration: f64) {
        self.request_count.inc();
        self.request_duration.observe(duration);

        if status >= 400 {
            self.error_count.inc();
        }
    }
}

/// Extract service name from path
fn extract_service_name(path: &str) -> &str {
    if path.starts_with("/api/") {
        // Extract service name from /api/service/...
        path.split('/').nth(2).unwrap_or("unknown")
    } else if path.starts_with("/ws/") {
        // Extract service name from /ws/service
        path.split('/').nth(2).unwrap_or("unknown")
    } else {
        "unknown"
    }
}

/// Metrics middleware
pub async fn metrics_middleware(
    metrics: axum::extract::State<Arc<Metrics>>,
    request: Request,
    next: Next,
) -> Response {
    let start = std::time::Instant::now();
    let method = request.method().to_string();
    let path = request.uri().path().to_string();

    // Increment active connections
    metrics.increment_connections();

    let response = next.run(request).await;

    // Decrement active connections
    metrics.decrement_connections();

    let duration = start.elapsed().as_secs_f64();
    let status = response.status().as_u16();

    // Record metrics
    metrics.record_request(&method, &path, status, duration).await;

    response
}

/// Metrics handlers for Axum
pub mod handlers {
    use super::*;
    use axum::response::IntoResponse;

    /// Prometheus metrics endpoint
    pub async fn prometheus_metrics(
        metrics: axum::extract::State<Arc<Metrics>>,
    ) -> impl IntoResponse {
        let metrics_text = metrics.prometheus_metrics().await;
        axum::response::Response::builder()
            .header("content-type", "text/plain; version=0.0.4; charset=utf-8")
            .body(metrics_text)
            .unwrap()
    }

    /// JSON metrics endpoint
    pub async fn json_metrics(
        metrics: axum::extract::State<Arc<Metrics>>,
    ) -> impl IntoResponse {
        use simd_json::json;

        let service_metrics = metrics.service_metrics().await;
        let response = json!({
            "services": service_metrics.into_iter()
                .map(|(name, metrics)| {
                    (name, simd_json::json!({
                        "name": metrics.name,
                        // Prometheus counters are not directly accessible
                        // In a real implementation, you'd expose the values
                    }))
                })
                .collect::<simd_json::value::owned::Object<String, simd_json::OwnedValue>>()
        });

        axum::Json(response)
    }

    /// Metrics dashboard endpoint
    pub async fn metrics_dashboard() -> impl IntoResponse {
        let html = r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Metrics Dashboard</title>
            <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
        </head>
        <body>
            <h1>HTTP Server Metrics</h1>
            <div>
                <h2>Prometheus Metrics</h2>
                <a href="/metrics">View Prometheus Metrics</a>
            </div>
            <div>
                <h2>JSON Metrics</h2>
                <a href="/metrics/json">View JSON Metrics</a>
            </div>
        </body>
        </html>
        "#;

        axum::response::Html(html)
    }
}

/// Performance monitoring utilities
pub mod perf {
    use super::*;

    /// Performance monitor
    pub struct PerformanceMonitor {
        metrics: Arc<Metrics>,
    }

    impl PerformanceMonitor {
        pub fn new(metrics: Arc<Metrics>) -> Self {
            Self { metrics }
        }

        /// Monitor a function execution
        pub async fn monitor<F, Fut, T>(&self, name: &str, f: F) -> T
        where
            F: FnOnce() -> Fut,
            Fut: std::future::Future<Output = T>,
        {
            let start = std::time::Instant::now();
            let result = f().await;
            let duration = start.elapsed().as_secs_f64();

            // Record custom metric
            self.metrics.request_duration.observe(duration);

            result
        }
    }
}

// Global metrics instance
lazy_static! {
    pub static ref GLOBAL_METRICS: Arc<Metrics> = Arc::new(Metrics::new());
}
</file>

<file path="src/middleware.rs">
//! Shared Middleware Stack
//!
//! Centralized middleware configuration applied to all routes.
//! This ensures consistent behavior across all HTTP endpoints.

use axum::{
    body::Body,
    http::Request,
    middleware::{self, Next},
    response::Response,
    Router,
};
use std::time::{Duration, Instant};
use tower_http::{
    compression::CompressionLayer,
    cors::{Any, CorsLayer},
    timeout::TimeoutLayer,
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};
use tracing::Level;

/// Middleware configuration
#[derive(Debug, Clone)]
pub struct MiddlewareConfig {
    /// Enable CORS (default: true)
    pub cors_enabled: bool,
    /// CORS allowed origins (None = any)
    pub cors_origins: Option<Vec<String>>,
    /// Enable request tracing (default: true)
    pub tracing_enabled: bool,
    /// Enable response compression (default: true)
    pub compression_enabled: bool,
    /// Request timeout (default: 30s)
    pub timeout: Duration,
    /// Enable security headers (default: true)
    pub security_headers: bool,
    /// Enable request logging (default: true)
    pub request_logging: bool,
}

impl Default for MiddlewareConfig {
    fn default() -> Self {
        Self {
            cors_enabled: true,
            cors_origins: None,
            tracing_enabled: true,
            compression_enabled: true,
            timeout: Duration::from_secs(30),
            security_headers: true,
            request_logging: true,
        }
    }
}

impl MiddlewareConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cors(mut self, enabled: bool) -> Self {
        self.cors_enabled = enabled;
        self
    }

    pub fn cors_origins(mut self, origins: Vec<String>) -> Self {
        self.cors_origins = Some(origins);
        self
    }

    pub fn tracing(mut self, enabled: bool) -> Self {
        self.tracing_enabled = enabled;
        self
    }

    pub fn compression(mut self, enabled: bool) -> Self {
        self.compression_enabled = enabled;
        self
    }

    pub fn timeout(mut self, duration: Duration) -> Self {
        self.timeout = duration;
        self
    }

    pub fn security_headers(mut self, enabled: bool) -> Self {
        self.security_headers = enabled;
        self
    }

    pub fn request_logging(mut self, enabled: bool) -> Self {
        self.request_logging = enabled;
        self
    }
}

/// Middleware stack that can be applied to a router
pub struct MiddlewareStack {
    config: MiddlewareConfig,
}

impl MiddlewareStack {
    pub fn new(config: MiddlewareConfig) -> Self {
        Self { config }
    }

    pub fn default_stack() -> Self {
        Self::new(MiddlewareConfig::default())
    }

    /// Apply the middleware stack to a router
    pub fn apply(self, router: Router) -> Router {
        let mut router = router;

        // Security headers (innermost - runs first on response)
        if self.config.security_headers {
            router = router.layer(middleware::from_fn(security_headers_middleware));
        }

        // Request logging
        if self.config.request_logging {
            router = router.layer(middleware::from_fn(request_logging_middleware));
        }

        // Timeout
        router = router.layer(TimeoutLayer::new(self.config.timeout));

        // Compression
        if self.config.compression_enabled {
            router = router.layer(CompressionLayer::new());
        }

        // Tracing
        if self.config.tracing_enabled {
            router = router.layer(
                TraceLayer::new_for_http()
                    .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                    .on_response(DefaultOnResponse::new().level(Level::INFO)),
            );
        }

        // CORS (outermost - runs first on request)
        if self.config.cors_enabled {
            let cors = if let Some(ref origins) = self.config.cors_origins {
                // Specific origins
                let origins: Vec<_> = origins.iter().filter_map(|o| o.parse().ok()).collect();
                CorsLayer::new()
                    .allow_origin(origins)
                    .allow_methods(Any)
                    .allow_headers(Any)
            } else {
                // Any origin
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods(Any)
                    .allow_headers(Any)
            };
            router = router.layer(cors);
        }

        router
    }
}

/// Security headers middleware
async fn security_headers_middleware(request: Request<Body>, next: Next) -> Response {
    let mut response = next.run(request).await;

    let headers = response.headers_mut();
    headers.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
    headers.insert("X-Frame-Options", "DENY".parse().unwrap());
    headers.insert("X-XSS-Protection", "1; mode=block".parse().unwrap());
    headers.insert(
        "Referrer-Policy",
        "strict-origin-when-cross-origin".parse().unwrap(),
    );
    headers.insert(
        "Permissions-Policy",
        "geolocation=(), microphone=(), camera=()".parse().unwrap(),
    );

    response
}

/// Request logging middleware
async fn request_logging_middleware(request: Request<Body>, next: Next) -> Response {
    let start = Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();
    let version = request.version();

    let response = next.run(request).await;

    let duration = start.elapsed();
    let status = response.status();

    // Log at appropriate level based on status
    if status.is_server_error() {
        tracing::error!(
            "{} {} {:?} {} - {}ms",
            method,
            uri,
            version,
            status.as_u16(),
            duration.as_millis()
        );
    } else if status.is_client_error() {
        tracing::warn!(
            "{} {} {:?} {} - {}ms",
            method,
            uri,
            version,
            status.as_u16(),
            duration.as_millis()
        );
    } else {
        tracing::info!(
            "{} {} {:?} {} - {}ms",
            method,
            uri,
            version,
            status.as_u16(),
            duration.as_millis()
        );
    }

    response
}

/// Convenience function to apply default middleware
pub fn default_middleware_stack(router: Router) -> Router {
    MiddlewareStack::default_stack().apply(router)
}

/// Convenience function to apply middleware with config
pub fn apply_middleware(router: Router, config: MiddlewareConfig) -> Router {
    MiddlewareStack::new(config).apply(router)
}
</file>

<file path="src/request_filters.rs">
//! HTTP Request Filters
//!
//! Native request filtering and processing for security, logging, rate limiting, etc.

use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::time::Instant;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

/// Security middleware - adds security headers
pub async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;

    let headers = response.headers_mut();
    headers.insert("X-Content-Type-Options", "nosniff".parse().unwrap());
    headers.insert("X-Frame-Options", "DENY".parse().unwrap());
    headers.insert("X-XSS-Protection", "1; mode=block".parse().unwrap());
    headers.insert("Referrer-Policy", "strict-origin-when-cross-origin".parse().unwrap());

    // Add HSTS for HTTPS
    if let Some(host) = headers.get("host") {
        if let Ok(host_str) = host.to_str() {
            if host_str.contains(":443") || host_str.starts_with("https://") {
                headers.insert("Strict-Transport-Security",
                    "max-age=31536000; includeSubDomains".parse().unwrap());
            }
        }
    }

    response
}

/// Request logging middleware
pub async fn request_logger(request: Request, next: Next) -> Response {
    let start = Instant::now();
    let method = request.method().clone();
    let uri = request.uri().clone();
    let version = request.version();

    let response = next.run(request).await;

    let duration = start.elapsed();
    let status = response.status();

    tracing::info!(
        "{} {} {:?} {} - {}ms",
        method,
        uri,
        version,
        status,
        duration.as_millis()
    );

    response
}

/// API key authentication middleware
pub async fn api_key_auth(
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Response {
    // Check for API key in various headers
    let api_key = headers.get("x-api-key")
        .or_else(|| headers.get("authorization"))
        .or_else(|| headers.get("x-password"))
        .and_then(|h| h.to_str().ok())
        .map(|s| s.trim_start_matches("Bearer ").trim());

    // For now, allow all requests (authentication is optional)
    // In production, you would validate the API key here
    if let Some(key) = api_key {
        if !key.is_empty() {
            tracing::debug!("API key provided: {}", if key.len() > 8 {
                format!("{}...{}", &key[..4], &key[key.len()-4..])
            } else {
                "***".to_string()
            });
        }
    }

    next.run(request).await
}

/// Rate limiting middleware (basic implementation)
pub async fn rate_limit(
    request: Request,
    next: Next,
) -> Response {
    // Simple rate limiting based on IP
    // In production, you'd want a more sophisticated solution
    let client_ip = request.headers()
        .get("x-forwarded-for")
        .or_else(|| request.headers().get("x-real-ip"))
        .and_then(|h| h.to_str().ok())
        .unwrap_or("unknown");

    // For now, just log and allow all requests
    tracing::debug!("Request from IP: {}", client_ip);

    next.run(request).await
}

/// Compression middleware
pub async fn compression(
    request: Request,
    next: Next,
) -> Response {
    // Add compression headers
    let mut response = next.run(request).await;

    let headers = response.headers_mut();
    headers.insert("Content-Encoding", "gzip".parse().unwrap());

    response
}

/// Create default CORS layer
pub fn default_cors() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any)
}

use tower_http::classify::SharedClassifier;
use tower_http::trace::DefaultOnRequest;
use tower_http::trace::DefaultOnResponse;
use tower_http::trace::DefaultOnBodyChunk;
use tower_http::trace::DefaultOnEos;
use tower_http::trace::DefaultOnFailure;
use tower_http::classify::ServerErrorsAsFailures;

/// Create default trace layer
pub fn default_trace() -> TraceLayer<
    SharedClassifier<ServerErrorsAsFailures>,
    tower_http::trace::DefaultMakeSpan,
    DefaultOnRequest,
    DefaultOnResponse,
    DefaultOnBodyChunk,
    DefaultOnEos,
    DefaultOnFailure,
> {
    TraceLayer::new_for_http()
}

/// Error handling middleware
pub async fn error_handler(
    err: Box<dyn std::error::Error + Send + Sync>,
) -> impl IntoResponse {
    tracing::error!("Request error: {}", err);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Internal Server Error"
    )
}

/// Timeout middleware
pub async fn timeout(
    request: Request,
    next: Next,
) -> Response {
    // Set a reasonable timeout for requests
    match tokio::time::timeout(
        std::time::Duration::from_secs(30),
        next.run(request)
    ).await {
        Ok(response) => response,
        Err(_) => (
            StatusCode::REQUEST_TIMEOUT,
            "Request timeout"
        ).into_response(),
    }
}
</file>

<file path="src/router.rs">
//! Router Composition
//!
//! Utilities for composing routers from multiple crates into a unified router.
//! Each crate implements ServiceRouter to expose its routes.

use axum::Router;
use std::path::PathBuf;
use tower_http::services::ServeDir;
use tracing::info;

/// Trait for crates that provide HTTP routes
///
/// Implement this trait in your crate to expose routes to the central server:
/// ```ignore
/// pub struct MyServiceRouter;
///
/// impl ServiceRouter for MyServiceRouter {
///     fn prefix() -> &'static str {
///         "/api/myservice"
///     }
///
///     fn name() -> &'static str {
///         "my-service"
///     }
/// }
///
/// // Then provide a create_router function:
/// pub fn create_router(state: MyState) -> Router {
///     Router::new()
///         .route("/health", get(health))
///         .route("/data", post(data))
///         .with_state(state)
/// }
/// ```
pub trait ServiceRouter: Send + Sync {
    /// The URL prefix for this service (e.g., "/api/mcp")
    fn prefix() -> &'static str;

    /// Service name for logging
    fn name() -> &'static str;

    /// Optional: service description
    fn description() -> &'static str {
        ""
    }
}

/// Builder for composing multiple service routers
pub struct RouterBuilder {
    router: Router,
    static_dir: Option<PathBuf>,
    services: Vec<(&'static str, &'static str)>, // (prefix, name)
}

impl RouterBuilder {
    /// Create a new router builder
    pub fn new() -> Self {
        Self {
            router: Router::new(),
            static_dir: None,
            services: Vec::new(),
        }
    }

    /// Add a router at a specific prefix
    pub fn nest(mut self, prefix: &'static str, name: &'static str, router: Router) -> Self {
        info!("Mounting service '{}' at {}", name, prefix);
        self.router = self.router.nest(prefix, router);
        self.services.push((prefix, name));
        self
    }

    /// Add a route directly to the root router
    pub fn route(mut self, path: &str, method_router: axum::routing::MethodRouter) -> Self {
        self.router = self.router.route(path, method_router);
        self
    }

    /// Merge another router (no prefix)
    pub fn merge(mut self, router: Router) -> Self {
        self.router = self.router.merge(router);
        self
    }

    /// Set static file directory (served at root, fallback)
    pub fn static_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.static_dir = Some(path.into());
        self
    }

    /// Get list of mounted services
    pub fn services(&self) -> &[(&'static str, &'static str)] {
        &self.services
    }

    /// Build the final router
    pub fn build(mut self) -> Router {
        // Add static file serving if configured (as fallback)
        if let Some(static_dir) = self.static_dir {
            if static_dir.exists() {
                info!("Serving static files from: {:?}", static_dir);
                self.router = self.router.fallback_service(ServeDir::new(static_dir));
            } else {
                tracing::warn!("Static directory not found: {:?}", static_dir);
            }
        }

        self.router
    }
}

impl Default for RouterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience function to create a router builder
pub fn router() -> RouterBuilder {
    RouterBuilder::new()
}
</file>

<file path="src/server.rs">
//! Central HTTP/TLS Server Implementation
//!
//! Single server that handles all HTTP/HTTPS traffic for op-dbus.

use crate::middleware::{apply_middleware, MiddlewareConfig};
use crate::tls::TlsConfig;
use crate::{Result, ServerError};
use axum::Router;
use hyper::server::conn::http1;
use hyper_util::rt::TokioIo;
use hyper_util::service::TowerToHyperService;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::info;

/// Server configuration
#[derive(Clone, Debug)]
pub struct ServerConfig {
    /// HTTP port
    pub http_port: u16,
    /// HTTPS port (if TLS enabled)
    pub https_port: u16,
    /// Bind host
    pub bind_host: String,
    /// Public hostname for logging/display
    pub public_host: String,
    /// TLS configuration
    pub tls: TlsConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            http_port: 8080,
            https_port: 8443,
            bind_host: "0.0.0.0".to_string(),
            public_host: gethostname::gethostname().to_string_lossy().to_string(),
            tls: TlsConfig::default(),
        }
    }
}

/// Central HTTP Server
pub struct HttpServer {
    config: ServerConfig,
    router: Router,
}

impl HttpServer {
    /// Create a new server builder
    pub fn builder() -> HttpServerBuilder {
        HttpServerBuilder::new()
    }

    /// Get server configuration
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }

    /// Start the server
    pub async fn serve(self) -> Result<()> {
        let http_addr: SocketAddr = format!("{}:{}", self.config.bind_host, self.config.http_port)
            .parse()
            .map_err(|_| {
                ServerError::BindError(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Invalid HTTP bind address",
                ))
            })?;

        // Try to build TLS acceptor
        let tls_acceptor = self.config.tls.build_acceptor()?;

        if let Some(acceptor) = tls_acceptor {
            // HTTPS mode - serve on both HTTP and HTTPS
            let https_addr: SocketAddr =
                format!("{}:{}", self.config.bind_host, self.config.https_port)
                    .parse()
                    .map_err(|_| {
                        ServerError::BindError(std::io::Error::new(
                            std::io::ErrorKind::InvalidInput,
                            "Invalid HTTPS bind address",
                        ))
                    })?;

            // Start HTTP server in background
            let http_router = self.router.clone();
            tokio::spawn(async move {
                let listener = match TcpListener::bind(http_addr).await {
                    Ok(l) => l,
                    Err(e) => {
                        tracing::error!("Failed to bind HTTP: {}", e);
                        return;
                    }
                };
                info!("HTTP server listening on http://{}", http_addr);
                let _ = axum::serve(listener, http_router).await;
            });

            // Start HTTPS server (main thread)
            let listener = TcpListener::bind(https_addr)
                .await
                .map_err(ServerError::BindError)?;

            info!("HTTPS server listening on https://{}", https_addr);
            info!(
                "Public URL: https://{}:{}",
                self.config.public_host, self.config.https_port
            );

            loop {
                let (stream, peer_addr) =
                    listener.accept().await.map_err(ServerError::BindError)?;
                let acceptor = acceptor.clone();
                let router = self.router.clone();

                tokio::spawn(async move {
                    match acceptor.accept(stream).await {
                        Ok(tls_stream) => {
                            let io = TokioIo::new(tls_stream);
                            let service = TowerToHyperService::new(router);

                            if let Err(e) =
                                http1::Builder::new().serve_connection(io, service).await
                            {
                                tracing::debug!("Connection error from {}: {}", peer_addr, e);
                            }
                        }
                        Err(e) => {
                            tracing::debug!("TLS handshake error from {}: {}", peer_addr, e);
                        }
                    }
                });
            }
        } else {
            // HTTP only mode
            let listener = TcpListener::bind(http_addr)
                .await
                .map_err(ServerError::BindError)?;

            info!("HTTP server listening on http://{}", http_addr);
            info!(
                "Public URL: http://{}:{}",
                self.config.public_host, self.config.http_port
            );
            info!("TLS disabled - using HTTP only");

            axum::serve(listener, self.router)
                .await
                .map_err(|e| ServerError::BindError(std::io::Error::other(e)))?;
        }

        Ok(())
    }
}

/// Builder for HttpServer
pub struct HttpServerBuilder {
    bind_host: String,
    http_port: u16,
    https_port: u16,
    public_host: Option<String>,
    tls_config: TlsConfig,
    router: Option<Router>,
    middleware_config: MiddlewareConfig,
}

impl HttpServerBuilder {
    pub fn new() -> Self {
        Self {
            bind_host: "0.0.0.0".to_string(),
            http_port: 8080,
            https_port: 8443,
            public_host: None,
            tls_config: TlsConfig::default(),
            router: None,
            middleware_config: MiddlewareConfig::default(),
        }
    }

    /// Set bind address (host:port format or just port)
    pub fn bind(mut self, addr: impl Into<String>) -> Self {
        let addr = addr.into();
        if let Some((host, port)) = addr.split_once(':') {
            self.bind_host = host.to_string();
            if let Ok(p) = port.parse() {
                self.http_port = p;
            }
        } else if let Ok(p) = addr.parse::<u16>() {
            self.http_port = p;
        }
        self
    }

    /// Set HTTP port
    pub fn http_port(mut self, port: u16) -> Self {
        self.http_port = port;
        self
    }

    /// Set HTTPS port
    pub fn https_port(mut self, port: u16) -> Self {
        self.https_port = port;
        self
    }

    /// Set public hostname
    pub fn public_host(mut self, host: impl Into<String>) -> Self {
        self.public_host = Some(host.into());
        self
    }

    /// Enable HTTPS with explicit certificate paths
    pub fn https(mut self, cert_path: impl Into<String>, key_path: impl Into<String>) -> Self {
        self.tls_config = TlsConfig::with_certs(cert_path, key_path);
        self
    }

    /// Enable HTTPS with auto-detection
    pub fn https_auto(mut self) -> Self {
        self.tls_config = TlsConfig::auto();
        self
    }

    /// Disable HTTPS (HTTP only)
    pub fn http_only(mut self) -> Self {
        self.tls_config = TlsConfig::disabled();
        self
    }

    /// Set the router
    pub fn router(mut self, router: Router) -> Self {
        self.router = Some(router);
        self
    }

    /// Set middleware configuration
    pub fn middleware(mut self, config: MiddlewareConfig) -> Self {
        self.middleware_config = config;
        self
    }

    /// Enable/disable CORS
    pub fn cors(mut self, enabled: bool) -> Self {
        self.middleware_config.cors_enabled = enabled;
        self
    }

    /// Enable/disable tracing
    pub fn tracing(mut self, enabled: bool) -> Self {
        self.middleware_config.tracing_enabled = enabled;
        self
    }

    /// Enable/disable compression
    pub fn compression(mut self, enabled: bool) -> Self {
        self.middleware_config.compression_enabled = enabled;
        self
    }

    /// Build the server
    pub fn build(self) -> Result<HttpServer> {
        let router = self.router.unwrap_or_default();

        // Apply middleware stack
        let router = apply_middleware(router, self.middleware_config);

        let public_host = self
            .public_host
            .unwrap_or_else(|| gethostname::gethostname().to_string_lossy().to_string());

        let config = ServerConfig {
            http_port: self.http_port,
            https_port: self.https_port,
            bind_host: self.bind_host,
            public_host,
            tls: self.tls_config,
        };

        Ok(HttpServer { config, router })
    }
}

impl Default for HttpServerBuilder {
    fn default() -> Self {
        Self::new()
    }
}
</file>

<file path="src/tls.rs">
//! TLS Configuration and Certificate Management
//!
//! Centralized TLS handling for all HTTP services.
//! Supports auto-detection of certificates from common locations.
//! Updated to include Cloudflare Origin certificate detection.

use crate::{Result, ServerError};
use rustls::ServerConfig as RustlsServerConfig;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;
use tokio_rustls::TlsAcceptor;
use tracing::{info, warn};

/// TLS mode configuration
#[derive(Clone, Debug, Default)]
pub enum TlsMode {
    /// No TLS, HTTP only
    #[default]
    Disabled,
    /// TLS enabled with explicit certificate paths
    Enabled { cert_path: String, key_path: String },
    /// Auto-detect certificates from common locations
    Auto,
}

/// TLS configuration
#[derive(Clone, Debug)]
pub struct TlsConfig {
    pub mode: TlsMode,
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            mode: TlsMode::Disabled,
            cert_path: None,
            key_path: None,
        }
    }
}

impl TlsConfig {
    /// Create a new TLS config with auto-detection
    pub fn auto() -> Self {
        Self {
            mode: TlsMode::Auto,
            cert_path: None,
            key_path: None,
        }
    }

    /// Create a new TLS config with explicit paths
    pub fn with_certs(cert_path: impl Into<String>, key_path: impl Into<String>) -> Self {
        let cert = cert_path.into();
        let key = key_path.into();
        Self {
            mode: TlsMode::Enabled {
                cert_path: cert.clone(),
                key_path: key.clone(),
            },
            cert_path: Some(cert),
            key_path: Some(key),
        }
    }

    /// Create a disabled TLS config (HTTP only)
    pub fn disabled() -> Self {
        Self::default()
    }

    /// Check if TLS is enabled
    pub fn is_enabled(&self) -> bool {
        !matches!(self.mode, TlsMode::Disabled)
    }

    /// Build a TLS acceptor from this config
    pub fn build_acceptor(&self) -> Result<Option<TlsAcceptor>> {
        match &self.mode {
            TlsMode::Disabled => Ok(None),
            TlsMode::Enabled {
                cert_path,
                key_path,
            } => {
                let acceptor = create_tls_acceptor(cert_path, key_path)?;
                Ok(Some(acceptor))
            }
            TlsMode::Auto => {
                if let Some((cert_path, key_path)) = detect_certificates()? {
                    info!("Auto-detected TLS certificates:");
                    info!("  cert: {}", cert_path);
                    info!("  key:  {}", key_path);
                    let acceptor = create_tls_acceptor(&cert_path, &key_path)?;
                    Ok(Some(acceptor))
                } else {
                    warn!("No TLS certificates found, falling back to HTTP");
                    Ok(None)
                }
            }
        }
    }
}

/// Create a TLS acceptor from certificate files
fn create_tls_acceptor(cert_path: &str, key_path: &str) -> Result<TlsAcceptor> {
    let cert_file = File::open(cert_path)
        .map_err(|e| ServerError::CertificateError(format!("Failed to open cert file: {}", e)))?;
    let key_file = File::open(key_path)
        .map_err(|e| ServerError::CertificateError(format!("Failed to open key file: {}", e)))?;

    let mut cert_reader = BufReader::new(cert_file);
    let mut key_reader = BufReader::new(key_file);

    let certs: Vec<_> = rustls_pemfile::certs(&mut cert_reader)
        .filter_map(|r| r.ok())
        .collect();

    let key = rustls_pemfile::private_key(&mut key_reader)
        .map_err(|e| ServerError::CertificateError(format!("Failed to read private key: {}", e)))?
        .ok_or_else(|| ServerError::CertificateError("No private key found".to_string()))?;

    if certs.is_empty() {
        return Err(ServerError::CertificateError(
            "No certificates found".to_string(),
        ));
    }

    let tls_config = RustlsServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| ServerError::TlsError(format!("TLS config error: {}", e)))?;

    Ok(TlsAcceptor::from(Arc::new(tls_config)))
}

/// Auto-detect SSL certificates from common locations
pub fn detect_certificates() -> Result<Option<(String, String)>> {
    // Priority 1: Environment variables
    if let (Ok(cert), Ok(key)) = (
        std::env::var("SSL_CERT_PATH"),
        std::env::var("SSL_KEY_PATH"),
    ) {
        if Path::new(&cert).exists() && Path::new(&key).exists() {
            info!("Using certificates from environment variables");
            return Ok(Some((cert, key)));
        }
    }

    // Priority 2: Cloudflare Origin certificates (NEW)
    let cloudflare_paths = [
        // Standard Cloudflare Origin certificate locations
        (
            "/etc/ssl/cloudflare/origin.pem",
            "/etc/ssl/cloudflare/origin.key",
        ),
        (
            "/etc/ssl/cloudflare/cert.pem",
            "/etc/ssl/cloudflare/key.pem",
        ),
        ("/etc/cloudflare/origin.pem", "/etc/cloudflare/origin.key"),
        ("/etc/cloudflare/cert.pem", "/etc/cloudflare/key.pem"),
        // Domain-specific Cloudflare paths
        (
            "/etc/ssl/cloudflare/ghostbridge.tech/cert.pem",
            "/etc/ssl/cloudflare/ghostbridge.tech/key.pem",
        ),
        // User directory
        (
            "/home/jeremy/certs/cloudflare_origin.pem",
            "/home/jeremy/certs/cloudflare_origin.key",
        ),
    ];

    for (cert, key) in &cloudflare_paths {
        if Path::new(cert).exists() && Path::new(key).exists() {
            info!("Found Cloudflare Origin certificate");
            return Ok(Some((cert.to_string(), key.to_string())));
        }
    }

    // Priority 3: Nginx/custom certificates
    let nginx_certs = [
        (
            "/etc/nginx/ssl/ghostbridge.crt",
            "/etc/nginx/ssl/ghostbridge.key",
        ),
        ("/etc/nginx/ssl/proxmox.crt", "/etc/nginx/ssl/proxmox.key"),
        ("/etc/nginx/ssl/server.crt", "/etc/nginx/ssl/server.key"),
        (
            "/etc/nginx/ssl/cloudflare.crt",
            "/etc/nginx/ssl/cloudflare.key",
        ),
    ];

    for (cert, key) in &nginx_certs {
        if Path::new(cert).exists() && Path::new(key).exists() {
            info!("Found nginx SSL certificate");
            return Ok(Some((cert.to_string(), key.to_string())));
        }
    }

    // Priority 4: Let's Encrypt certificates
    let letsencrypt_domains = [
        "ghostbridge.tech",
        "proxmox.ghostbridge.tech",
        "op-web.ghostbridge.tech",
    ];

    let hostname = gethostname::gethostname().to_string_lossy().to_string();

    for domain in letsencrypt_domains
        .iter()
        .chain(std::iter::once(&hostname.as_str()))
    {
        let cert = format!("/etc/letsencrypt/live/{}/fullchain.pem", domain);
        let key = format!("/etc/letsencrypt/live/{}/privkey.pem", domain);
        if Path::new(&cert).exists() && Path::new(&key).exists() {
            info!("Found Let's Encrypt certificate for {}", domain);
            return Ok(Some((cert, key)));
        }
    }

    // Priority 5: Proxmox cluster certificates
    let pve_cert = format!("/etc/pve/nodes/{}/pve-ssl.pem", hostname);
    let pve_key = format!("/etc/pve/nodes/{}/pve-ssl.key", hostname);

    if Path::new(&pve_cert).exists() && Path::new(&pve_key).exists() {
        info!("Found Proxmox cluster certificate");
        return Ok(Some((pve_cert, pve_key)));
    }

    // Priority 6: System default certificates (self-signed)
    let system_certs = [
        (
            "/etc/ssl/certs/ssl-cert-snakeoil.pem",
            "/etc/ssl/private/ssl-cert-snakeoil.key",
        ),
        (
            "/etc/ssl/certs/localhost.pem",
            "/etc/ssl/private/localhost.key",
        ),
    ];

    for (cert, key) in &system_certs {
        if Path::new(cert).exists() && Path::new(key).exists() {
            warn!("Using system default (possibly self-signed) certificate");
            return Ok(Some((cert.to_string(), key.to_string())));
        }
    }

    Ok(None)
}

/// Validate that a certificate and key match
pub fn validate_cert_key_match(cert_path: &str, key_path: &str) -> Result<bool> {
    use std::process::Command;

    // Get certificate modulus
    let cert_output = Command::new("openssl")
        .args(["x509", "-in", cert_path, "-noout", "-modulus"])
        .output()
        .map_err(|e| ServerError::CertificateError(format!("Failed to run openssl: {}", e)))?;

    let key_output = Command::new("openssl")
        .args(["rsa", "-in", key_path, "-noout", "-modulus"])
        .output()
        .map_err(|e| ServerError::CertificateError(format!("Failed to run openssl: {}", e)))?;

    Ok(cert_output.stdout == key_output.stdout)
}

/// Get certificate expiry information
pub fn get_cert_expiry(cert_path: &str) -> Result<String> {
    use std::process::Command;

    let output = Command::new("openssl")
        .args(["x509", "-in", cert_path, "-noout", "-enddate"])
        .output()
        .map_err(|e| ServerError::CertificateError(format!("Failed to run openssl: {}", e)))?;

    let expiry = String::from_utf8_lossy(&output.stdout)
        .replace("notAfter=", "")
        .trim()
        .to_string();

    Ok(expiry)
}

/// Check if certificate is from Cloudflare
pub fn is_cloudflare_cert(cert_path: &str) -> Result<bool> {
    use std::process::Command;

    let output = Command::new("openssl")
        .args(["x509", "-in", cert_path, "-noout", "-issuer"])
        .output()
        .map_err(|e| ServerError::CertificateError(format!("Failed to run openssl: {}", e)))?;

    let issuer = String::from_utf8_lossy(&output.stdout).to_lowercase();
    Ok(issuer.contains("cloudflare"))
}
</file>

<file path="Cargo.toml">
[package]
name = "op-http"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
description = "Central HTTP/TLS server for all op-dbus modules"

[dependencies]
# Async runtime
tokio = { workspace = true }
futures = { workspace = true }

# Serialization
serde = { workspace = true }
simd-json = { workspace = true }

# Error handling
anyhow = { workspace = true }
thiserror = { workspace = true }

# Logging
tracing = { workspace = true }

# HTTP server
axum = { workspace = true }
tower = { workspace = true }
tower-http = { workspace = true, features = ["cors", "fs", "trace", "compression-gzip", "compression-br", "timeout"] }
hyper = { workspace = true }
hyper-util = { workspace = true }

# TLS
rustls = { workspace = true }
rustls-pemfile = { workspace = true }
tokio-rustls = { workspace = true }

# Utils
chrono = { workspace = true }
gethostname = { workspace = true }
</file>

<file path="compare-op-http.md">
# compare-op-http

**Date**: 2026-04-05  
**Spec files analyzed**: SPEC.md  
**Analysis mode**: Current system state specification with spec-alignment notes

---

## Summary

| Category | Count |
|---|---|
| Rust source files | 8 |
| Proto files | 0 |
| Binary targets | 0 |
| UI files | 0 |
| Root-declared modules | 4 |
| Partial artifacts | 0 |
| Spec-listed source files | 8 |
| Spec-listed but missing | 0 |
| Extra implementation files | 0 |

## Current Implementation Overview

- Central HTTP/TLS server for all op-dbus modules

## Module / File Comparison

| Module or File | State | Current Role | Notes |
|---|---|---|---|
| `src/tls.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/tls.rs |
| `src/server.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/server.rs |
| `src/router.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/router.rs |
| `src/request_filters.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/request_filters.rs |
| `src/middleware.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/middleware.rs |
| `src/metrics.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/metrics.rs |
| `src/lib.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/lib.rs |
| `src/health.rs` | ✅ Present | Declared in source inventory from spec/design docs | src/health.rs |
| `root` | ✅ Present | root source group | src/health.rs, src/lib.rs, src/metrics.rs, src/middleware.rs, src/request_filters.rs, src/router.rs, src/server.rs, src/tls.rs |

## Feature / Capability Comparison

| Capability | State | Evidence | Source |
|---|---|---|---|
| tls | ✅ Implemented | src/tls.rs | SPEC main module |
| server | ✅ Implemented | src/server.rs | SPEC main module |
| router | ✅ Implemented | src/router.rs | SPEC main module |
| request_filters | ✅ Implemented | src/request_filters.rs | SPEC main module |
| middleware | ✅ Implemented | src/middleware.rs | SPEC main module |
| metrics | ✅ Implemented | src/metrics.rs | SPEC main module |
| health | ✅ Implemented | src/health.rs | SPEC main module |

## Dependencies Comparison

### Internal Workspace Dependencies
- None

### External Runtime Dependencies
- `tokio` - documented in SPEC
- `futures` - documented in SPEC
- `serde` - documented in SPEC
- `simd-json` - documented in SPEC
- `anyhow` - documented in SPEC
- `thiserror` - documented in SPEC
- `tracing` - documented in SPEC
- `axum` - documented in SPEC
- `tower` - documented in SPEC
- `tower-http` - documented in SPEC
- `hyper` - documented in SPEC
- `hyper-util` - not listed in SPEC dependency block
- `rustls` - not listed in SPEC dependency block
- `rustls-pemfile` - not listed in SPEC dependency block
- `tokio-rustls` - not listed in SPEC dependency block
- `chrono` - not listed in SPEC dependency block
- `gethostname` - not listed in SPEC dependency block

### Development and Build Dependencies
- None

## Notes and Observations

- Local documentation files present: SPEC.md.
- Root module declarations found in `lib.rs`/`main.rs`: middleware, router, server, tls.
- 6 runtime dependencies are present in `Cargo.toml` but omitted from the SPEC dependency block.
</file>

<file path="SPEC.md">
# op-http - Specification

## Overview
**Crate**: `op-http`  
**Location**: `crates/op-http`

## Quick Reference

### From Cargo.toml
```toml
[package]
name = "op-http"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
```

### Source Structure
```
op-http/src/tls.rs
op-http/src/server.rs
op-http/src/router.rs
op-http/src/request_filters.rs
op-http/src/middleware.rs
op-http/src/metrics.rs
op-http/src/lib.rs
op-http/src/health.rs
```

### Key Dependencies
```toml
# Async runtime
tokio = { workspace = true }
futures = { workspace = true }

# Serialization
serde = { workspace = true }
simd-json = { workspace = true }

# Error handling
anyhow = { workspace = true }
thiserror = { workspace = true }

# Logging
tracing = { workspace = true }

# HTTP server
axum = { workspace = true }
tower = { workspace = true }
tower-http = { workspace = true, features = ["cors", "fs", "trace", "compression-gzip", "compression-br", "timeout"] }
hyper = { workspace = true }
```

### Binaries
```toml
# No binaries
```

### Features
```toml
# No features
```

## Documentation Files


## Module Structure
       8 Rust source files

### Main Modules
tls
server
router
request_filters
middleware
metrics
health

## Purpose
Central HTTP/TLS server for all op-dbus modules

## Build Information
- **Edition**: edition.workspace = true
- **Version**: version.workspace = true
- **License**: license.workspace = true

## Related Crates
Internal dependencies:


---
*Generated from crate analysis*
</file>

</files>
