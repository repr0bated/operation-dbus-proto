//! Dashboard Metrics Handlers

use axum::{extract::Extension, response::Json};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::Arc;
use tracing::error;

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
}

/// GET /api/dashboard/metrics - Get dashboard overview metrics
pub async fn dashboard_metrics_handler(
    Extension(state): Extension<Arc<AppState>>,
) -> Json<DashboardMetrics> {
    // Get user count
    let total_users = state.user_store.list_users().await.len();

    // Get VPN connections
    let active_connections = get_vpn_peer_count();

    // Get system stats
    let (cpu, memory) = get_system_stats();

    Json(DashboardMetrics {
        active_connections,
        total_users,
        mail_queue: 0,   // TODO
        mcp_services: 0, // TODO
        cpu,
        memory,
        network: 0.0, // TODO
    })
}

fn get_vpn_peer_count() -> usize {
    Command::new("wg")
        .args(&["show", "wg0", "peers"])
        .output()
        .ok()
        .map(|output| {
            if output.status.success() {
                String::from_utf8_lossy(&output.stdout).lines().count()
            } else {
                0
            }
        })
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
