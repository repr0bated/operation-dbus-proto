//! Mail Server Status Handlers

use axum::{extract::Extension, response::Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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

/// Check whether Postfix/Dovecot process is running or mail sockets exist.
/// Container processes are visible from host procfs and /run/mail-3tched/ sockets.
async fn is_mail_server_running() -> bool {
    let sockets = [
        "/run/mail-3tched/submission.sock",
        "/run/mail-3tched/smtp.sock",
        "/run/mail-3tched/imap.sock",
    ];
    if sockets.iter().any(|p| std::path::Path::new(p).exists()) {
        return true;
    }

    let mut entries = match tokio::fs::read_dir("/proc").await {
        Ok(e) => e,
        Err(_) => return false,
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name();
        let Some(pid_str) = name.to_str() else {
            continue;
        };
        if pid_str.parse::<u32>().is_err() {
            continue;
        }

        let comm_path = entry.path().join("comm");
        if let Ok(comm) = tokio::fs::read_to_string(&comm_path).await {
            let proc_name = comm.trim();
            if proc_name == "postfix" || proc_name == "master" || proc_name == "dovecot" {
                return true;
            }
        }
    }

    false
}

/// GET /api/mail/status - Get mail server status
pub async fn mail_status_handler(Extension(_state): Extension<Arc<AppState>>) -> Json<MailStatus> {
    let running = is_mail_server_running().await;

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
