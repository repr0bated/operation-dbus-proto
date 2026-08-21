//! Quota Awareness Layer (R11)
//!
//! Tracks query usage against configurable tier limits.
//! Default free tier: ~50 queries/day per the NotebookLM MCP spec.
//! The quota resets daily at midnight UTC.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Quota tier configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaTier {
    pub name: String,
    pub daily_limit: u32,
}

impl Default for QuotaTier {
    fn default() -> Self {
        Self {
            name: "free".to_string(),
            daily_limit: 50,
        }
    }
}

/// Thread-safe quota tracker.
pub struct QuotaManager {
    tier: Arc<RwLock<QuotaTier>>,
    queries_today: AtomicU32,
    last_reset: Arc<RwLock<DateTime<Utc>>>,
}

impl QuotaManager {
    pub fn new(tier: QuotaTier) -> Self {
        Self {
            tier: Arc::new(RwLock::new(tier)),
            queries_today: AtomicU32::new(0),
            last_reset: Arc::new(RwLock::new(Utc::now())),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(QuotaTier::default())
    }

    /// Check if a query is allowed under the current quota.
    /// Returns (allowed, remaining, limit).
    pub async fn check_and_increment(&self) -> (bool, u32, u32) {
        self.maybe_reset().await;

        let limit = self.tier.read().await.daily_limit;
        loop {
            let current = self.queries_today.load(Ordering::Acquire);
            if current >= limit {
                return (false, 0, limit);
            }

            // `current < limit <= u32::MAX`, so this increment cannot wrap.
            let next = current + 1;
            if self
                .queries_today
                .compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return (true, limit.saturating_sub(next), limit);
            }
        }
    }

    /// Get current quota status without incrementing.
    pub async fn status(&self) -> (u32, u32) {
        self.maybe_reset().await;
        let tier = self.tier.read().await;
        let used = self.queries_today.load(Ordering::Relaxed);
        let remaining = tier.daily_limit.saturating_sub(used);
        (remaining, tier.daily_limit)
    }

    /// Update the quota tier at runtime (R11: set_quota_tier).
    pub async fn set_tier(&self, tier: QuotaTier) {
        *self.tier.write().await = tier;
    }

    /// Get current tier info.
    pub async fn tier(&self) -> QuotaTier {
        self.tier.read().await.clone()
    }

    /// Reset counter if a new UTC day has started.
    async fn maybe_reset(&self) {
        let now = Utc::now();
        // One writer protects the date comparison and counter reset as a
        // single operation. With a read-then-write pair, two first requests
        // after midnight could each reset the counter and erase an admission
        // made by the other request.
        let mut last = self.last_reset.write().await;
        if now.date_naive() != last.date_naive() {
            self.queries_today.store(0, Ordering::Relaxed);
            *last = now;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn should_allow_queries_within_limit() {
        let mgr = QuotaManager::new(QuotaTier {
            name: "test".into(),
            daily_limit: 3,
        });

        let (ok, remaining, limit) = mgr.check_and_increment().await;
        assert!(ok);
        assert_eq!(remaining, 2);
        assert_eq!(limit, 3);
    }

    #[tokio::test]
    async fn should_deny_queries_over_limit() {
        let mgr = QuotaManager::new(QuotaTier {
            name: "test".into(),
            daily_limit: 2,
        });

        mgr.check_and_increment().await;
        mgr.check_and_increment().await;

        let (ok, remaining, _) = mgr.check_and_increment().await;
        assert!(!ok);
        assert_eq!(remaining, 0);
    }

    #[tokio::test]
    async fn should_report_status() {
        let mgr = QuotaManager::with_defaults();
        let (remaining, limit) = mgr.status().await;
        assert_eq!(remaining, 50);
        assert_eq!(limit, 50);
    }

    #[tokio::test]
    async fn quota_counter_never_wraps_at_the_u32_boundary() {
        let mgr = QuotaManager::new(QuotaTier {
            name: "maximum".into(),
            daily_limit: u32::MAX,
        });
        mgr.queries_today.store(u32::MAX - 1, Ordering::Relaxed);

        assert_eq!(mgr.check_and_increment().await, (true, 0, u32::MAX));
        assert_eq!(mgr.check_and_increment().await, (false, 0, u32::MAX));
        assert_eq!(mgr.status().await, (0, u32::MAX));
    }

    #[tokio::test]
    async fn stale_quota_is_reset_before_the_next_admission() {
        let mgr = QuotaManager::new(QuotaTier {
            name: "test".into(),
            daily_limit: 2,
        });
        mgr.queries_today.store(2, Ordering::Relaxed);
        *mgr.last_reset.write().await = Utc::now() - chrono::Duration::days(1);

        assert_eq!(mgr.check_and_increment().await, (true, 1, 2));
        assert_eq!(mgr.status().await, (1, 2));
    }
}
