//! Audit-trail page store.
//!
//! Holds one page of events plus the filter and pagination cursor that produced
//! it. This is plain owned state, not reactive state: there are no subscribers
//! and no notifications. The egui frame loop drains the channel and reads the
//! `Vec<AuditEvent>` directly.
//!
//! Fetches are demand-driven — first render of the tab, an explicit Refresh, a
//! filter change, or a pagination click. Nothing polls on a timer.

use std::collections::HashSet;
use std::time::Instant;
use tokio::sync::mpsc;

use super::transport::AccountabilityFrame;

/// Events requested per page. The server clamps at 100; never request
/// unbounded (`limit = 0` means "everything" on the wire).
pub const PAGE_SIZE: u32 = 50;

/// Decision filter, mapping onto the proto `Decision` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DecisionFilter {
    #[default]
    All,
    Allow,
    Deny,
}

impl DecisionFilter {
    /// Proto `Decision` enum discriminant:
    /// `DECISION_UNSPECIFIED = 0`, `ALLOW = 1`, `DENY = 2`.
    pub fn as_proto(self) -> i32 {
        match self {
            DecisionFilter::All => 0,
            DecisionFilter::Allow => 1,
            DecisionFilter::Deny => 2,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            DecisionFilter::All => "All",
            DecisionFilter::Allow => "Allow",
            DecisionFilter::Deny => "Deny",
        }
    }
}

/// Server-side query parameters for one page.
#[derive(Debug, Clone)]
pub struct AuditFilter {
    /// `0` means "from the beginning".
    pub from_event_id: u64,
    /// `0` means "through the latest event".
    pub to_event_id: u64,
    pub limit: u32,
    /// Empty means all plugins.
    pub plugin_id: String,
    pub decision: DecisionFilter,
}

impl Default for AuditFilter {
    fn default() -> Self {
        Self {
            from_event_id: 0,
            to_event_id: 0,
            limit: PAGE_SIZE,
            plugin_id: String::new(),
            decision: DecisionFilter::All,
        }
    }
}

/// One audit event, decoded from the proto `ChainEvent`.
///
/// Every field the proto message carries is preserved so the detail row can
/// show the operator the complete record (FR-5).
#[derive(Debug, Clone, Default)]
pub struct AuditEvent {
    pub event_id: u64,
    pub prev_hash: String,
    pub event_hash: String,
    /// RFC 3339 where the proto timestamp decodes, otherwise empty.
    pub timestamp: String,
    pub actor_id: String,
    pub capability_id: String,
    pub plugin_id: String,
    pub schema_version: String,
    pub operation_type: String,
    pub target: String,
    pub tags_touched: Vec<String>,
    pub decision: String,
    pub deny_reason: String,
    pub input_patch_hash: String,
    pub result_effective_hash: String,
}

/// Audit-trail page state. Owned by `ExplorerState`.
#[derive(Debug)]
pub struct AccountabilityStore {
    /// The current page, in server order (ascending `event_id`).
    pub events: Vec<AuditEvent>,
    /// Filter that produced `events`, and the one the next fetch will use.
    pub filter: AuditFilter,
    /// Client-side-only actor filter. The server has no actor predicate, so
    /// this narrows the fetched page rather than the query (FR-4).
    pub actor_query: String,
    /// True when the server reported more events beyond this page.
    pub has_more: bool,
    /// A fetch is in flight.
    pub loading: bool,
    /// Last transport error, shown in the view until the next successful fetch.
    pub error: Option<String>,
    /// Set once the tab has triggered its initial fetch, so navigating to the
    /// tab loads data exactly once instead of on every frame.
    pub initialized: bool,
    /// Set by the view; the transport picks it up and clears it.
    pub pending_fetch: Option<AuditFilter>,
    /// Result channel for the in-flight fetch.
    pub frame_rx: Option<mpsc::Receiver<AccountabilityFrame>>,
    /// `event_id`s whose detail row is expanded.
    pub expanded: HashSet<u64>,
    /// Total events in the server's chain, from the last response.
    pub total_in_chain: u64,
    last_repaint: Instant,
}

impl Default for AccountabilityStore {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            filter: AuditFilter::default(),
            actor_query: String::new(),
            has_more: false,
            loading: false,
            error: None,
            initialized: false,
            pending_fetch: None,
            frame_rx: None,
            expanded: HashSet::new(),
            total_in_chain: 0,
            last_repaint: Instant::now(),
        }
    }
}

impl AccountabilityStore {
    /// Throttle repaints to ~60fps while a fetch is in flight.
    pub fn should_repaint(&mut self) -> bool {
        if self.last_repaint.elapsed().as_millis() >= 16 {
            self.last_repaint = Instant::now();
            true
        } else {
            false
        }
    }

    /// Queue a fetch with the current filter.
    pub fn request_fetch(&mut self) {
        // Never send an unbounded request.
        self.filter.limit = self.filter.limit.clamp(1, 100);
        self.pending_fetch = Some(self.filter.clone());
        self.loading = true;
        self.initialized = true;
    }

    /// Page towards older events: window ends just below the current first id.
    pub fn page_back(&mut self) {
        let first = self.events.first().map(|e| e.event_id).unwrap_or(0);
        if first <= 1 {
            return;
        }
        self.filter.from_event_id = first.saturating_sub(u64::from(self.filter.limit)).max(1);
        self.filter.to_event_id = first - 1;
        self.request_fetch();
    }

    /// Page towards newer events: window starts just above the current last id.
    pub fn page_forward(&mut self) {
        let last = self.events.last().map(|e| e.event_id).unwrap_or(0);
        if last == 0 {
            return;
        }
        self.filter.from_event_id = last + 1;
        self.filter.to_event_id = 0;
        self.request_fetch();
    }

    /// Return to the newest window (no id bounds).
    pub fn reset_range(&mut self) {
        self.filter.from_event_id = 0;
        self.filter.to_event_id = 0;
    }

    /// Drain the transport result into the store. Returns true when anything
    /// changed, which tells the view to request a repaint.
    ///
    /// A fetch yields exactly one frame — a page or an error — so this reads a
    /// single message and drops the receiver rather than looping.
    pub fn drain_frames(&mut self) -> bool {
        let Some(rx) = self.frame_rx.as_mut() else {
            return false;
        };
        let Ok(frame) = rx.try_recv() else {
            // Still in flight; keep the receiver for the next frame.
            return false;
        };

        match frame {
            AccountabilityFrame::Page {
                events,
                has_more,
                total_in_chain,
            } => {
                self.events = events;
                self.has_more = has_more;
                self.total_in_chain = total_in_chain;
                self.error = None;
                // Drop details for rows no longer on the page.
                let visible: HashSet<u64> = self.events.iter().map(|e| e.event_id).collect();
                self.expanded.retain(|id| visible.contains(id));
            }
            AccountabilityFrame::Error(message) => {
                self.error = Some(message);
            }
        }

        self.loading = false;
        self.frame_rx = None;
        true
    }

    /// The page narrowed by the client-side actor filter.
    pub fn visible_events(&self) -> Vec<&AuditEvent> {
        let needle = self.actor_query.trim();
        self.events
            .iter()
            .filter(|e| needle.is_empty() || e.actor_id == needle)
            .collect()
    }

    pub fn toggle_expanded(&mut self, event_id: u64) {
        if !self.expanded.remove(&event_id) {
            self.expanded.insert(event_id);
        }
    }
}
