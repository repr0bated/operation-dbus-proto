//! Cognitive Memory Store
//!
//! Manages persistent and ephemeral memory for cognitive operations.
//! Supports different memory types:
//! - Ephemeral: Session-based memory
//! - Persistent: Database-backed memory
//! - Shared: Cross-session memory

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub key: String,
    pub value: simd_json::OwnedValue,
    pub memory_type: MemoryType,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub access_count: u64,
    pub last_accessed: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MemoryType {
    Ephemeral,
    Persistent,
    Shared,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryQuery {
    pub key_pattern: Option<String>,
    pub memory_type: Option<MemoryType>,
    pub tags: Option<Vec<String>>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

pub struct CognitiveMemoryStore {
    memory: Arc<RwLock<HashMap<String, MemoryEntry>>>,
    max_entries: usize,
}

impl CognitiveMemoryStore {
    pub fn new(max_entries: usize) -> Self {
        Self {
            memory: Arc::new(RwLock::new(HashMap::new())),
            max_entries,
        }
    }

    pub async fn store(&self, entry: MemoryEntry) -> Result<(), String> {
        let mut memory = self.memory.write().await;

        // Check size limits
        if memory.len() >= self.max_entries && !memory.contains_key(&entry.id) {
            return Err("Memory store at capacity".to_string());
        }

        memory.insert(entry.id.clone(), entry);
        Ok(())
    }

    pub async fn retrieve(&self, id: &str) -> Option<MemoryEntry> {
        let mut memory = self.memory.write().await;
        if let Some(entry) = memory.get_mut(id) {
            entry.access_count += 1;
            entry.last_accessed = Utc::now();
            Some(entry.clone())
        } else {
            None
        }
    }

    pub async fn query(&self, query: MemoryQuery) -> Vec<MemoryEntry> {
        let memory = self.memory.read().await;
        let mut results: Vec<MemoryEntry> = memory.values().cloned().collect();

        // Apply filters
        if let Some(pattern) = &query.key_pattern {
            results.retain(|entry| entry.key.contains(pattern));
        }

        if let Some(memory_type) = &query.memory_type {
            results.retain(|entry| entry.memory_type == *memory_type);
        }

        if let Some(tags) = &query.tags {
            results.retain(|entry| {
                tags.iter().all(|tag| entry.tags.contains(tag))
            });
        }

        // Apply pagination
        let offset = query.offset.unwrap_or(0);
        let limit = query.limit.unwrap_or(results.len());
        results.into_iter()
            .skip(offset)
            .take(limit)
            .collect()
    }

    pub async fn delete(&self, id: &str) -> bool {
        let mut memory = self.memory.write().await;
        memory.remove(id).is_some()
    }

    pub async fn cleanup_expired(&self) -> usize {
        let mut memory = self.memory.write().await;
        let now = Utc::now();
        let initial_len = memory.len();

        memory.retain(|_, entry| {
            entry.expires_at.is_none_or(|expires| expires > now)
        });

        initial_len - memory.len()
    }

    pub async fn get_stats(&self) -> MemoryStats {
        let memory = self.memory.read().await;
        let mut type_counts = HashMap::new();

        for entry in memory.values() {
            *type_counts.entry(entry.memory_type.clone()).or_insert(0) += 1;
        }

        MemoryStats {
            total_entries: memory.len(),
            type_counts,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct MemoryStats {
    pub total_entries: usize,
    pub type_counts: HashMap<MemoryType, usize>,
}