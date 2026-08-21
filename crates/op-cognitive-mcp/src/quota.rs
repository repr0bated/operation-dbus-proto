//! Quota Awareness Layer (R11)
//!
//! Tracks query usage against configurable tier limits.
//! Default free tier: ~50 queries/day per the NotebookLM MCP spec.
//! The quota resets daily at midnight UTC.
//!
//! Runtime counters are optionally persisted in the same Cozo-backed store as
//! Cognitive memory.  The bridge uses that constructor, so a process restart
//! cannot turn an exhausted daily allowance into a fresh quota.

use crate::memory_store::{CognitiveMemoryStore, NamespaceKind};
use anyhow::{Context, Result};
use chrono::{NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

const NOTEBOOKLM_FREE_DAILY_CHAT_LIMIT: u32 = 50;
const NOTEBOOKLM_PLUS_DAILY_CHAT_LIMIT: u32 = 200;
const NOTEBOOKLM_PRO_DAILY_CHAT_LIMIT: u32 = 500;
const NOTEBOOKLM_ULTRA_DAILY_CHAT_LIMIT: u32 = 2_500;
const QUOTA_NAMESPACE: &str = "project:3tched-cognative";
const QUOTA_ENTRY_KEY: &str = "_runtime_quota";

/// The persisted state deliberately contains no credential, request content,
/// identity, or provider claim.  It is only the UTC accounting checkpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct PersistedQuotaState {
    version: u8,
    reset_date: NaiveDate,
    queries_today: u32,
}

impl PersistedQuotaState {
    fn fresh() -> Self {
        Self {
            version: 1,
            reset_date: Utc::now().date_naive(),
            queries_today: 0,
        }
    }

    fn reset_if_stale(&mut self) -> bool {
        let today = Utc::now().date_naive();
        if self.reset_date != today {
            self.reset_date = today;
            self.queries_today = 0;
            return true;
        }
        false
    }
}

/// Quota tier configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaTier {
    pub name: String,
    pub daily_limit: u32,
}

impl Default for QuotaTier {
    fn default() -> Self {
        Self::from_config_values(
            std::env::var("COGNITIVE_MCP_QUOTA_PROFILE").ok().as_deref(),
            std::env::var("COGNITIVE_MCP_DAILY_QUERY_LIMIT")
                .ok()
                .as_deref(),
        )
    }
}

impl QuotaTier {
    /// Resolve the provider account allowance without making an account claim
    /// from a cookie. Operators can override the daily query limit explicitly;
    /// this host defaults to the configured NotebookLM Pro account.
    fn from_config_values(profile: Option<&str>, configured_limit: Option<&str>) -> Self {
        let (name, profile_limit) = match profile
            .unwrap_or("notebooklm_pro")
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "free" | "standard" | "notebooklm_free" => {
                ("notebooklm_free", NOTEBOOKLM_FREE_DAILY_CHAT_LIMIT)
            }
            "plus" | "notebooklm_plus" => ("notebooklm_plus", NOTEBOOKLM_PLUS_DAILY_CHAT_LIMIT),
            "ultra" | "notebooklm_ultra" => ("notebooklm_ultra", NOTEBOOKLM_ULTRA_DAILY_CHAT_LIMIT),
            // Unknown profile values must not silently reduce the available
            // Pro allowance. A numeric operator override remains authoritative.
            "pro" | "notebooklm_pro" | _ => ("notebooklm_pro", NOTEBOOKLM_PRO_DAILY_CHAT_LIMIT),
        };
        let daily_limit = configured_limit
            .and_then(|value| value.trim().parse::<u32>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(profile_limit);
        Self {
            name: name.to_string(),
            daily_limit,
        }
    }
}

/// Thread-safe quota tracker.
pub struct QuotaManager {
    tier: Arc<RwLock<QuotaTier>>,
    state: Arc<Mutex<PersistedQuotaState>>,
    persistence: Option<Arc<CognitiveMemoryStore>>,
}

impl QuotaManager {
    pub fn new(tier: QuotaTier) -> Self {
        Self {
            tier: Arc::new(RwLock::new(tier)),
            state: Arc::new(Mutex::new(PersistedQuotaState::fresh())),
            persistence: None,
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(QuotaTier::default())
    }

    /// Construct a quota manager that resumes the canonical daily counter.
    ///
    /// The namespace is private operational state: model-facing memory tools
    /// are disabled, and only the bridge-owned ingress owns quota admission.
    pub async fn with_persistent_store(
        tier: QuotaTier,
        store: Arc<CognitiveMemoryStore>,
    ) -> Result<Self> {
        store
            .upsert_namespace(
                QUOTA_NAMESPACE,
                NamespaceKind::Project,
                Some("Cognitive operational state; not model-controlled memory"),
                None,
                None,
                serde_json::json!({
                    "owner": "canonical_orchestrator",
                    "purpose": "quota_accounting",
                }),
            )
            .await
            .context("ensure durable Cognitive quota namespace")?;

        let manager = Self {
            tier: Arc::new(RwLock::new(tier)),
            state: Arc::new(Mutex::new(PersistedQuotaState::fresh())),
            persistence: Some(store),
        };
        manager.restore().await?;
        Ok(manager)
    }

    pub async fn with_persistent_defaults(store: Arc<CognitiveMemoryStore>) -> Result<Self> {
        Self::with_persistent_store(QuotaTier::default(), store).await
    }

    /// Check if a query is allowed under the current quota.
    /// Returns (allowed, remaining, limit).
    pub async fn check_and_increment(&self) -> Result<(bool, u32, u32)> {
        let limit = self.tier.read().await.daily_limit;
        let (result, changed, snapshot) = {
            // One mutex makes reset, admission, and the snapshot persisted for
            // that admission a single critical section.  The counter cannot
            // wrap because admission is checked before incrementing.
            let mut state = self.state.lock().await;
            let mut changed = state.reset_if_stale();
            let result = if state.queries_today >= limit {
                (false, 0, limit)
            } else {
                state.queries_today += 1;
                changed = true;
                (true, limit.saturating_sub(state.queries_today), limit)
            };
            (result, changed, state.clone())
        };
        if changed {
            self.persist(&snapshot).await?;
        }
        Ok(result)
    }

    /// Get current quota status without incrementing.
    pub async fn status(&self) -> Result<(u32, u32)> {
        let tier = self.tier.read().await;
        let (changed, state) = {
            let mut state = self.state.lock().await;
            let changed = state.reset_if_stale();
            (changed, state.clone())
        };
        if changed {
            self.persist(&state).await?;
        }
        Ok((
            tier.daily_limit.saturating_sub(state.queries_today),
            tier.daily_limit,
        ))
    }

    /// Update the quota tier at runtime (R11: set_quota_tier).
    pub async fn set_tier(&self, tier: QuotaTier) {
        *self.tier.write().await = tier;
    }

    /// Get current tier info.
    pub async fn tier(&self) -> QuotaTier {
        self.tier.read().await.clone()
    }

    async fn restore(&self) -> Result<()> {
        let Some(store) = &self.persistence else {
            return Ok(());
        };
        let Some(entry) = store
            .retrieve_entry(QUOTA_NAMESPACE, QUOTA_ENTRY_KEY)
            .await
            .context("read durable Cognitive quota checkpoint")?
        else {
            return Ok(());
        };
        let mut restored: PersistedQuotaState = serde_json::from_value(entry.value)
            .context("decode durable Cognitive quota checkpoint")?;
        if restored.version != 1 {
            anyhow::bail!(
                "unsupported durable Cognitive quota checkpoint version {}",
                restored.version
            );
        }
        let changed = restored.reset_if_stale();
        *self.state.lock().await = restored.clone();
        if changed {
            self.persist(&restored).await?;
        }
        Ok(())
    }

    async fn persist(&self, state: &PersistedQuotaState) -> Result<()> {
        let Some(store) = &self.persistence else {
            return Ok(());
        };
        store
            .store_entry(
                QUOTA_NAMESPACE,
                QUOTA_ENTRY_KEY,
                serde_json::to_value(state).context("serialize Cognitive quota checkpoint")?,
                vec!["internal".to_string(), "quota".to_string()],
                None,
            )
            .await
            .context("persist Cognitive quota checkpoint")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cozo_shuttle::CozoGraphShuttle;
    use std::path::Path;

    async fn persistent_store(path: &Path) -> Arc<CognitiveMemoryStore> {
        let shuttle = Arc::new(
            CozoGraphShuttle::new_persistent(path.to_path_buf())
                .expect("create persistent quota test store"),
        );
        Arc::new(
            CognitiveMemoryStore::new(shuttle)
                .await
                .expect("create Cognitive quota test memory store"),
        )
    }

    #[tokio::test]
    async fn should_allow_queries_within_limit() {
        let mgr = QuotaManager::new(QuotaTier {
            name: "test".into(),
            daily_limit: 3,
        });

        let (ok, remaining, limit) = mgr.check_and_increment().await.unwrap();
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

        mgr.check_and_increment().await.unwrap();
        mgr.check_and_increment().await.unwrap();

        let (ok, remaining, _) = mgr.check_and_increment().await.unwrap();
        assert!(!ok);
        assert_eq!(remaining, 0);
    }

    #[tokio::test]
    async fn should_report_status() {
        let mgr = QuotaManager::with_defaults();
        let (remaining, limit) = mgr.status().await.unwrap();
        assert_eq!(remaining, NOTEBOOKLM_PRO_DAILY_CHAT_LIMIT);
        assert_eq!(limit, NOTEBOOKLM_PRO_DAILY_CHAT_LIMIT);
    }

    #[test]
    fn profile_configuration_uses_pro_chat_quota_and_keeps_an_operator_override() {
        let pro = QuotaTier::from_config_values(Some("pro"), None);
        assert_eq!(pro.name, "notebooklm_pro");
        assert_eq!(pro.daily_limit, NOTEBOOKLM_PRO_DAILY_CHAT_LIMIT);

        let free = QuotaTier::from_config_values(Some("free"), None);
        assert_eq!(free.daily_limit, NOTEBOOKLM_FREE_DAILY_CHAT_LIMIT);

        let overridden = QuotaTier::from_config_values(Some("pro"), Some("731"));
        assert_eq!(overridden.daily_limit, 731);
    }

    #[tokio::test]
    async fn quota_counter_never_wraps_at_the_u32_boundary() {
        let mgr = QuotaManager::new(QuotaTier {
            name: "maximum".into(),
            daily_limit: u32::MAX,
        });
        mgr.state.lock().await.queries_today = u32::MAX - 1;

        assert_eq!(
            mgr.check_and_increment().await.unwrap(),
            (true, 0, u32::MAX)
        );
        assert_eq!(
            mgr.check_and_increment().await.unwrap(),
            (false, 0, u32::MAX)
        );
        assert_eq!(mgr.status().await.unwrap(), (0, u32::MAX));
    }

    #[tokio::test]
    async fn stale_quota_is_reset_before_the_next_admission() {
        let mgr = QuotaManager::new(QuotaTier {
            name: "test".into(),
            daily_limit: 2,
        });
        {
            let mut state = mgr.state.lock().await;
            state.queries_today = 2;
            state.reset_date = Utc::now().date_naive() - chrono::Duration::days(1);
        }

        assert_eq!(mgr.check_and_increment().await.unwrap(), (true, 1, 2));
        assert_eq!(mgr.status().await.unwrap(), (1, 2));
    }

    #[tokio::test]
    async fn persistent_quota_survives_a_store_reopen() {
        let dir = tempfile::tempdir().expect("temporary quota directory");
        let db_path = dir.path().join("cognitive-quota.db");
        let tier = QuotaTier {
            name: "test".into(),
            daily_limit: 2,
        };

        {
            let store = persistent_store(&db_path).await;
            let quota = QuotaManager::with_persistent_store(tier.clone(), store)
                .await
                .expect("create durable quota manager");
            assert_eq!(quota.check_and_increment().await.unwrap(), (true, 1, 2));
        }

        let reopened_store = persistent_store(&db_path).await;
        let reopened = QuotaManager::with_persistent_store(tier, reopened_store)
            .await
            .expect("restore durable quota manager");
        assert_eq!(reopened.status().await.unwrap(), (1, 2));
        assert_eq!(reopened.check_and_increment().await.unwrap(), (true, 0, 2));
        assert_eq!(reopened.check_and_increment().await.unwrap(), (false, 0, 2));
    }
}
