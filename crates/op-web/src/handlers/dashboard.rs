//! Dashboard Metrics Handlers — reads schema-validated projections from D-Bus tree.

use axum::{extract::Extension, response::Json};
use serde::{Deserialize, Serialize};
use simd_json::prelude::ValueAsScalar;
use std::sync::Arc;

use crate::projection_client;
use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardMetrics {
    pub active_connections: usize,
    pub total_users: usize,
    pub mail_queue: usize,
    pub mcp_services: usize,
    pub cpu: f32,
    pub memory: f32,
    pub network: f32,
    /// Source of the data: "projection" if from D-Bus tree, "fallback" if raw /proc
    pub source: String,
    /// Schema version of the projection data
    pub schema_version: String,
}

/// GET /api/dashboard/metrics — Read from D-Bus projection tree (schema-validated).
/// Falls back to raw /proc only if projections are not yet available.
pub async fn dashboard_metrics_handler(
    Extension(state): Extension<Arc<AppState>>,
) -> Json<DashboardMetrics> {
    // Get user count
    let total_users = state.user_store.list_users().await.len();

    // Get VPN connections
    let active_connections = get_vpn_peer_count();

    // Try to read system stats from the D-Bus projection cache first.
    // These are live procfs projections from org.opdbus.v1 /org/opdbus/v1/plugins/procfs/*.
    let mem_proj =
        projection_client::get_projection(&state.projection_cache, "system.memory").await;
    let load_proj = projection_client::get_projection(&state.projection_cache, "system.load").await;

    let (cpu, memory, source, schema_version) = if mem_proj.is_some() || load_proj.is_some() {
        let mut cpu = 0.0f32;
        let mut memory = 0.0f32;

        // memory projection from /proc/meminfo: keys are lowercase, values are strings like "24019080 kB"
        if let Some(simd_json::OwnedValue::Object(ref obj)) = mem_proj {
            let total_kb = obj
                .get("memtotal")
                .and_then(|v| {
                    v.as_str().map(|s| {
                        s.split_whitespace()
                            .next()
                            .unwrap_or("0")
                            .parse::<u64>()
                            .unwrap_or(0)
                    })
                })
                .unwrap_or(0);
            let free_kb = obj
                .get("memfree")
                .and_then(|v| {
                    v.as_str().map(|s| {
                        s.split_whitespace()
                            .next()
                            .unwrap_or("0")
                            .parse::<u64>()
                            .unwrap_or(0)
                    })
                })
                .unwrap_or(0);
            if total_kb > 0 {
                memory = ((total_kb - free_kb) as f32 / total_kb as f32) * 100.0;
            }
        }

        // CPU load from /proc/loadavg projection: keys are "load1", "load5", "load15"
        // Values may be floats or strings depending on parser.
        if let Some(simd_json::OwnedValue::Object(ref obj)) = load_proj {
            let load_1min = obj
                .get("load1")
                .and_then(|v| {
                    as_f64(v).or_else(|| v.as_str().map(|s| s.parse::<f64>().unwrap_or(0.0)))
                })
                .unwrap_or(0.0) as f32;
            // Approximate CPU % from load (assuming 8 cores)
            cpu = (load_1min * 100.0 / 8.0).min(100.0);
        }

        (cpu, memory, "projection".to_string(), "live".to_string())
    } else {
        // Fallback: read raw /proc directly
        let (cpu, memory) = get_system_stats();
        (cpu, memory, "fallback".to_string(), "none".to_string())
    };

    Json(DashboardMetrics {
        active_connections,
        total_users,
        mail_queue: 0,
        mcp_services: 0,
        cpu,
        memory,
        network: 0.0,
        source,
        schema_version,
    })
}

/// GET /api/dashboard/projections — Return all cached schema-validated projections.
pub async fn dashboard_projections_handler(
    Extension(state): Extension<Arc<AppState>>,
) -> Json<std::collections::HashMap<String, simd_json::OwnedValue>> {
    let projections = projection_client::get_all_projections(&state.projection_cache).await;
    Json(projections)
}

fn get_vpn_peer_count() -> usize {
    let peers_dir = "/sys/class/net/wg0/wireguard/peers";
    std::fs::read_dir(peers_dir)
        .ok()
        .map(|entries| entries.filter_map(|e| e.ok()).count())
        .unwrap_or(0)
}

fn get_system_stats() -> (f32, f32) {
    // Get CPU usage from /proc/loadavg
    let cpu = std::fs::read_to_string("/proc/loadavg")
        .ok()
        .and_then(|s| {
            s.split_whitespace()
                .next()
                .and_then(|n| n.parse::<f32>().ok())
        })
        .map(|load| (load * 100.0).min(100.0))
        .unwrap_or(0.0);

    // Get memory usage from /proc/meminfo
    let memory = std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|s| {
            let mut total = 0u64;
            let mut available = 0u64;
            for line in s.lines() {
                if line.starts_with("MemTotal:") {
                    total = line.split_whitespace().nth(1)?.parse().ok()?;
                } else if line.starts_with("MemAvailable:") {
                    available = line.split_whitespace().nth(1)?.parse().ok()?;
                }
            }
            if total > 0 {
                Some(((total - available) as f32 / total as f32) * 100.0)
            } else {
                None
            }
        })
        .unwrap_or(0.0);

    (cpu, memory)
}

/// Extract f64 from simd_json OwnedValue.
fn as_f64(v: &simd_json::OwnedValue) -> Option<f64> {
    match v {
        simd_json::OwnedValue::Static(simd_json::StaticNode::F64(n)) => Some(*n),
        simd_json::OwnedValue::Static(simd_json::StaticNode::U64(n)) => Some(*n as f64),
        simd_json::OwnedValue::Static(simd_json::StaticNode::I64(n)) => Some(*n as f64),
        _ => None,
    }
}
