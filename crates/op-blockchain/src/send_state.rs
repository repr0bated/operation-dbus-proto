//! BTRFS incremental send state tracking per remote replica.
//!
//! Tracks the last successfully sent snapshot per remote so that:
//! 1. `btrfs send -p <parent> <child>` can be used for incremental sends
//! 2. Pruning never deletes a snapshot still needed as a send parent
//!
//! State is persisted to `.send-state.json` in the snapshots directory
//! and survives restarts.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

/// Per-remote replication state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteState {
    pub remote_id: String,
    pub ssh_host: String,
    pub btrfs_receive_path: String,
    /// The last snapshot successfully received by this remote.
    /// This is the `-p` parent for the next incremental send.
    pub last_sent_snapshot: Option<String>,
    /// Unix timestamp of last successful send.
    pub last_sent_at: Option<u64>,
    /// True if remote has never received a snapshot — must do a full send first.
    pub needs_full_send: bool,
}

/// Tracks send state for all configured remotes.
///
/// Key invariant: a snapshot is pinned (cannot be pruned) as long as any
/// remote depends on it as its incremental send parent.
#[derive(Debug, Serialize, Deserialize)]
pub struct SendState {
    pub remotes: HashMap<String, RemoteState>,
    #[serde(skip)]
    state_file: PathBuf,
}

impl SendState {
    /// Load send state from disk, or create empty state if file doesn't exist.
    pub async fn load(snapshots_dir: &Path) -> Result<Self> {
        let state_file = snapshots_dir.join(".send-state.json");

        if state_file.exists() {
            let mut content = tokio::fs::read_to_string(&state_file)
                .await
                .context("Failed to read send state")?;
            let mut state: SendState = unsafe { simd_json::from_str(&mut content) }
                .context("Failed to parse send state")?;
            state.state_file = state_file;
            debug!("Loaded send state with {} remotes", state.remotes.len());
            Ok(state)
        } else {
            debug!("No existing send state — starting fresh");
            Ok(Self {
                remotes: HashMap::new(),
                state_file,
            })
        }
    }

    /// Bootstrap remotes from environment variable.
    /// Format: `OPDBUS_REPLICAS=id1:host1:/path1,id2:host2:/path2`
    pub fn bootstrap_from_env(&mut self) {
        if let Ok(replicas) = std::env::var("OPDBUS_REPLICAS") {
            for entry in replicas.split(',') {
                let parts: Vec<&str> = entry.trim().splitn(3, ':').collect();
                if parts.len() == 3 {
                    let remote_id = parts[0].to_string();
                    if !self.remotes.contains_key(&remote_id) {
                        info!(
                            "Bootstrapping remote '{}' from env: {}:{}",
                            remote_id, parts[1], parts[2]
                        );
                        self.remotes.insert(
                            remote_id.clone(),
                            RemoteState {
                                remote_id,
                                ssh_host: parts[1].to_string(),
                                btrfs_receive_path: parts[2].to_string(),
                                last_sent_snapshot: None,
                                last_sent_at: None,
                                needs_full_send: true,
                            },
                        );
                    }
                }
            }
        }
    }

    /// Persist send state to disk.
    pub async fn save(&self) -> Result<()> {
        let content =
            simd_json::to_string_pretty(self).context("Failed to serialize send state")?;
        // Atomic write: temp file then rename
        let tmp = self.state_file.with_extension("json.tmp");
        tokio::fs::write(&tmp, &content)
            .await
            .context("Failed to write send state temp file")?;
        tokio::fs::rename(&tmp, &self.state_file)
            .await
            .context("Failed to rename send state file")?;
        debug!("Saved send state ({} remotes)", self.remotes.len());
        Ok(())
    }

    /// Add a new remote. No-op if remote_id already exists.
    pub fn add_remote(&mut self, remote_id: String, ssh_host: String, btrfs_receive_path: String) {
        self.remotes.entry(remote_id.clone()).or_insert_with(|| {
            info!(
                "Added remote '{}': {}:{}",
                remote_id, ssh_host, btrfs_receive_path
            );
            RemoteState {
                remote_id,
                ssh_host,
                btrfs_receive_path,
                last_sent_snapshot: None,
                last_sent_at: None,
                needs_full_send: true,
            }
        });
    }

    /// Remove a remote. Warns if it still has a pinned snapshot.
    pub fn remove_remote(&mut self, remote_id: &str) -> Option<RemoteState> {
        if let Some(state) = self.remotes.get(remote_id) {
            if state.last_sent_snapshot.is_some() {
                warn!(
                    "Removing remote '{}' which still pins snapshot '{}'",
                    remote_id,
                    state.last_sent_snapshot.as_deref().unwrap_or("?")
                );
            }
        }
        self.remotes.remove(remote_id)
    }

    /// Get the parent snapshot for incremental send to a remote.
    /// Returns `None` if the remote needs a full send.
    pub fn parent_for(&self, remote_id: &str) -> Option<&str> {
        self.remotes.get(remote_id).and_then(|r| {
            if r.needs_full_send {
                None
            } else {
                r.last_sent_snapshot.as_deref()
            }
        })
    }

    /// Record that a snapshot was successfully sent to a remote.
    /// The old parent becomes unpinned for this remote.
    pub fn record_successful_send(&mut self, remote_id: &str, snapshot_name: &str) {
        if let Some(remote) = self.remotes.get_mut(remote_id) {
            let old_parent = remote.last_sent_snapshot.replace(snapshot_name.to_string());
            remote.needs_full_send = false;
            remote.last_sent_at = Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            );

            info!(
                "Remote '{}': sent '{}' (previous parent: {})",
                remote_id,
                snapshot_name,
                old_parent.as_deref().unwrap_or("none/full-send")
            );
        } else {
            warn!(
                "record_successful_send called for unknown remote '{}'",
                remote_id
            );
        }
    }

    /// Check if a snapshot is pinned by any remote as its send parent.
    /// Pinned snapshots MUST NOT be pruned.
    pub fn is_snapshot_pinned(&self, snapshot_name: &str) -> bool {
        self.remotes.values().any(|r| {
            r.last_sent_snapshot
                .as_deref()
                .map(|s| s == snapshot_name)
                .unwrap_or(false)
        })
    }

    /// Get all currently pinned snapshots (for pruning coordination).
    pub fn all_pinned_snapshots(&self) -> HashSet<String> {
        self.remotes
            .values()
            .filter_map(|r| r.last_sent_snapshot.clone())
            .collect()
    }

    /// List all configured remotes.
    pub fn list_remotes(&self) -> Vec<&RemoteState> {
        self.remotes.values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_load_save_roundtrip() {
        let tmp = std::env::temp_dir().join(format!("send-state-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap();
        let mut state = SendState::load(&tmp).await.unwrap();

        state.add_remote(
            "replica1".into(),
            "10.0.0.2".into(),
            "/var/lib/blockchain/vectors/".into(),
        );
        state.record_successful_send("replica1", "vectors-abc123");
        state.save().await.unwrap();

        let loaded = SendState::load(&tmp).await.unwrap();
        assert_eq!(loaded.remotes.len(), 1);
        assert_eq!(loaded.parent_for("replica1"), Some("vectors-abc123"));
    }

    #[test]
    fn test_pinning() {
        let mut state = SendState {
            remotes: HashMap::new(),
            state_file: PathBuf::from("/tmp/test"),
        };

        state.add_remote("r1".into(), "host1".into(), "/path1".into());
        state.add_remote("r2".into(), "host2".into(), "/path2".into());

        // Initially nothing pinned
        assert!(!state.is_snapshot_pinned("snap-1"));
        assert!(state.all_pinned_snapshots().is_empty());

        // Send snap-1 to r1
        state.record_successful_send("r1", "snap-1");
        assert!(state.is_snapshot_pinned("snap-1"));

        // Send snap-2 to r1 — snap-1 no longer pinned by r1
        state.record_successful_send("r1", "snap-2");
        assert!(!state.is_snapshot_pinned("snap-1"));
        assert!(state.is_snapshot_pinned("snap-2"));

        // Send snap-1 to r2 — snap-1 pinned again (by r2)
        state.record_successful_send("r2", "snap-1");
        assert!(state.is_snapshot_pinned("snap-1"));
        assert!(state.is_snapshot_pinned("snap-2"));

        let pinned = state.all_pinned_snapshots();
        assert_eq!(pinned.len(), 2);
        assert!(pinned.contains("snap-1"));
        assert!(pinned.contains("snap-2"));
    }

    #[test]
    fn test_parent_for_full_send() {
        let mut state = SendState {
            remotes: HashMap::new(),
            state_file: PathBuf::from("/tmp/test"),
        };

        state.add_remote("r1".into(), "host1".into(), "/path1".into());

        // New remote needs full send
        assert!(state.parent_for("r1").is_none());

        // After first send, parent is available
        state.record_successful_send("r1", "snap-1");
        assert_eq!(state.parent_for("r1"), Some("snap-1"));
    }
}
