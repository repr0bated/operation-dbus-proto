//! MirrorSession module for per-peer session management

use crate::event::MirrorEvent;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::broadcast;

/// Per-peer session tracking state
#[derive(Debug)]
pub struct MirrorSession {
    /// Peer's UniqueName on D-Bus
    pub peer_name: String,
    /// Set of subscribed object paths
    pub subscribed_paths: HashSet<String>,
    /// Last acknowledged sequence number per path
    pub last_acked_sequence: HashMap<String, u64>,
    /// Pending events queue (max 500 events)
    pub pending_events: Vec<MirrorEvent>,
    /// Session creation time
    pub created_at: SystemTime,
    /// Total event count for this session
    pub event_count: usize,
}

impl MirrorSession {
    /// Create a new session for a peer
    pub fn new(peer_name: String) -> Self {
        Self {
            peer_name,
            subscribed_paths: HashSet::new(),
            last_acked_sequence: HashMap::new(),
            pending_events: Vec::new(),
            created_at: SystemTime::now(),
            event_count: 0,
        }
    }

    /// Subscribe to an object path
    pub fn subscribe_path(&mut self, path: String) {
        self.subscribed_paths.insert(path);
    }

    /// Unsubscribe from an object path
    pub fn unsubscribe_path(&mut self, path: &str) {
        self.subscribed_paths.remove(path);
    }

    /// Check if session has exceeded event queue limit
    pub fn is_queue_full(&self) -> bool {
        self.pending_events.len() >= 500
    }

    /// Add an event to the pending queue
    pub fn add_event(&mut self, event: MirrorEvent) {
        if !self.is_queue_full() {
            self.pending_events.push(event);
            self.event_count += 1;
        }
    }

    /// Get and remove all pending events
    pub fn take_events(&mut self) -> Vec<MirrorEvent> {
        std::mem::take(&mut self.pending_events)
    }

    /// Update last acknowledged sequence number for a path
    pub fn update_ack_sequence(&mut self, path: &str, sequence: u64) {
        self.last_acked_sequence.insert(path.to_string(), sequence);
    }
}
