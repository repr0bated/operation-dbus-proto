use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use json_benchmark::JsonProcessor;
use serde::{Deserialize, Serialize};
use simd_json::json;
use std::sync::Arc;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
struct AppState {
    json_processor: Arc<JsonProcessor>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiRequest {
    action: String,
    data: simd_json::OwnedValue,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiResponse {
    success: bool,
    message: String,
    data: Option<simd_json::OwnedValue>,
    timestamp: i64,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("debug,tower_http=debug")
        .init();

    let state = AppState {
        json_processor: Arc::new(JsonProcessor::new(16384)),
    };

    let app = Router::new()
        .route("/", get(root))
        .route("/process", post(process_json))
        .route("/batch", post(batch_process))
        .route("/health", get(health_check))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("🚀 SIMD-JSON API server running on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}

async fn root() -> impl IntoResponse {
    Json(json!({
        "message": "simd-json API server",
        "version": "1.0.0",
        "features": ["high-performance", "zero-copy", "simd-optimized"]
    }))
}

async fn process_json(
    State(state): State<AppState>,
    Json(mut payload): Json<ApiRequest>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    
    // Process the JSON data using simd-json
    let result = match payload.data {
        simd_json::OwnedValue::Object(ref map) => {
            // Example processing: count keys and calculate size
            let key_count = map.len();
            let estimated_size = simd_json::to_vec(&payload.data).unwrap_or_default().len();
            
            Ok(json!({
                "processed_keys": key_count,
                "estimated_bytes": estimated_size,
                "action": &payload.action
            }))
        }
        _ => Err("Expected JSON object"),
    };

    let processing_time = start.elapsed();

    match result {
        Ok(data) => {
            let response = ApiResponse {
                success: true,
                message: "JSON processed successfully".to_string(),
                data: Some(data),
                timestamp: chrono::Utc::now().timestamp(),
            };
            
            println!("✓ Processed JSON in {:?}", processing_time);
            (StatusCode::OK, Json(response))
        }
        Err(error) => {
            let response = ApiResponse {
                success: false,
                message: format!("Processing error: {}", error),
                data: None,
                timestamp: chrono::Utc::now().timestamp(),
            };
            
            (StatusCode::BAD_REQUEST, Json(response))
        }
    }
}

async fn batch_process(
    State(state): State<AppState>,
    Json(payloads): Json<Vec<ApiRequest>>,
) -> impl IntoResponse {
    let start = std::time::Instant::now();
    let processor = &state.json_processor;

    let results: Vec<ApiResponse> = payloads.into_iter().map(|payload| {
        match &payload.data {
            simd_json::OwnedValue::Object(map) => {
                let key_count = map.len();
                ApiResponse {
                    success: true,
                    message: format!("Processed {} keys", key_count),
                    data: Some(json!({ "keys_processed": key_count })),
                    timestamp: chrono::Utc::now().timestamp(),
                }
            }
            _ => ApiResponse {
                success: false,
                message: "Expected JSON object".to_string(),
                data: None,
                timestamp: chrono::Utc::now().timestamp(),
            }
        }
    }).collect();

    let processing_time = start.elapsed();
    println!("✓ Batch processed {} items in {:?}", results.len(), processing_time);

    (StatusCode::OK, Json(json!({ "results": results })))
}

async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "healthy",
        "library": "simd-json",
        "version": env!("CARGO_PKG_VERSION"),
        "timestamp": chrono::Utc::now().timestamp()
    }))
}