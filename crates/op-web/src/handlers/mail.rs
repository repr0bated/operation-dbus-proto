//! Mail Server Status Handlers

use axum::{extract::Extension, response::Json};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::Arc;
use tracing::{error, warn};

use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct MailStatus {
    pub running: bool,
    pub smtp_port: u16,
    pub imap_port: u16,
    pub sent_today: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MailQueueItem {
    pub id: String,
    pub from: String,
    pub to: String,
    pub subject: String,
    pub status: String,
    pub retry_count: usize,
    pub created_at: String,
}

/// GET /api/mail/status - Get mail server status
pub async fn mail_status_handler(Extension(_state): Extension<Arc<AppState>>) -> Json<MailStatus> {
    // Check if maddy is running in container
    let running = Command::new("incus")
        .args(&[
            "exec",
            "crd-astral",
            "--",
            "systemctl",
            "is-active",
            "maddy",
        ])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);

    Json(MailStatus {
        running,
        smtp_port: 25,
        imap_port: 143,
        sent_today: 0, // TODO: Parse from maddy logs
    })
}

/// GET /api/mail/queue - Get mail queue
pub async fn mail_queue_handler(
    Extension(_state): Extension<Arc<AppState>>,
) -> Json<Vec<MailQueueItem>> {
    // TODO: Query maddy's queue directory or database
    Json(vec![])
}

/// GET /api/mail/accounts - Get email accounts
pub async fn mail_accounts_handler(
    Extension(state): Extension<Arc<AppState>>,
) -> Json<Vec<String>> {
    // Return list of user emails
    let users = state.user_store.list_users().await;
    Json(users.into_iter().map(|u| u.email).collect())
}
