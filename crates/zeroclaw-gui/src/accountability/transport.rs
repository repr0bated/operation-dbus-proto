//! gRPC transport for the audit trail.
//!
//! Calls `EventChainService.GetEvents` on the op-grpc-bridge and pushes the
//! decoded page into a channel the egui frame loop drains. One request per user
//! action; no polling loop, no background timer (NFR-3).
//!
//! The tonic `Channel` is created once and reused. A failed connection leaves
//! the cell empty so the next user action retries rather than latching an error
//! for the process lifetime.
//!
//! OSCAL subid: `obs.service.event-chain.query@v1`

use anyhow::Result;
use tokio::sync::{mpsc, OnceCell};
use tonic::transport::Channel;

use super::store::{AuditEvent, AuditFilter};

// Generated client stubs for `operation.v1` (produced by build.rs from
// op-grpc-bridge's canonical operation.proto), included from `src/proto/` the
// same way this crate's other generated clients are.
//
// Nested in its own module with lints relaxed: the file defines every message
// in the control-plane proto, and this crate only consumes the EventChain
// subset. Warning on the rest would be noise about generated code.
#[allow(dead_code, clippy::all, unused_qualifications)]
mod generated {
    include!("../proto/operation.v1.rs");
}

use generated::{deny_reason, event_chain_service_client, ChainEvent, Decision, GetEventsRequest};

/// Shared channel to the gRPC bridge, reused across fetches.
static CHANNEL: OnceCell<Channel> = OnceCell::const_new();

/// One result frame from the transport.
#[derive(Debug)]
pub enum AccountabilityFrame {
    /// A completed page of events.
    Page {
        events: Vec<AuditEvent>,
        has_more: bool,
        total_in_chain: u64,
    },
    /// The fetch failed; the message is shown in the view.
    Error(String),
}

/// gRPC client for `EventChainService`.
///
/// Deliberately holds no reference to chat state or the chat channel — the two
/// paths talk to different services and share nothing (NFR-2).
pub struct AccountabilityTransport;

impl AccountabilityTransport {
    /// gRPC endpoint, matching the one `main.rs` uses for reflection and chat.
    pub fn endpoint() -> String {
        std::env::var("ZEROCLAW_GRPC").unwrap_or_else(|_| "http://10.200.0.1:50051".into())
    }

    /// Connect once and reuse. Retries on the next call if connecting failed.
    async fn channel() -> Result<Channel> {
        let channel = CHANNEL
            .get_or_try_init(|| async { crate::conn::connect_channel(&Self::endpoint()).await })
            .await?;
        Ok(channel.clone())
    }

    /// Fetch one page. Returns the receiver the store drains.
    ///
    /// Must be called from within the tokio runtime context (the egui update
    /// loop runs inside it, the same way the chat send path does).
    pub fn spawn_fetch(filter: AuditFilter) -> mpsc::Receiver<AccountabilityFrame> {
        let (tx, rx) = mpsc::channel::<AccountabilityFrame>(4);
        tokio::spawn(async move {
            let frame = match Self::fetch_page(&filter).await {
                Ok(frame) => frame,
                Err(error) => AccountabilityFrame::Error(format!("{error:#}")),
            };
            let _ = tx.send(frame).await;
        });
        rx
    }

    /// Issue `GetEvents` and decode the response.
    pub async fn fetch_page(filter: &AuditFilter) -> Result<AccountabilityFrame> {
        let channel = Self::channel().await?;
        let mut client = event_chain_service_client::EventChainServiceClient::new(channel);

        // Ask for one extra row so "has_more" is known even when the server's
        // own has_more is a page-full heuristic.
        let limit = filter.limit.clamp(1, 100);
        let request = GetEventsRequest {
            from_event_id: filter.from_event_id,
            to_event_id: filter.to_event_id,
            limit,
            plugin_id: filter.plugin_id.clone(),
            tags: Vec::new(),
            decision_filter: filter.decision.as_proto(),
        };

        let mut tonic_request = tonic::Request::new(request);
        crate::grpc::attach_ghostbridge_identity(&mut tonic_request);
        let response = client.get_events(tonic_request).await?;
        let response = response.into_inner();
        let has_more = response.has_more;
        let events: Vec<AuditEvent> = response
            .events
            .into_iter()
            .map(proto_to_audit_event)
            .collect();

        Ok(AccountabilityFrame::Page {
            events,
            has_more,
            // GetEventsResponse carries no chain total; report the page size so
            // the view can still show a count. The D-Bus `query_events` method
            // is the surface that reports the true chain total.
            total_in_chain: 0,
        })
    }
}

/// Decode a proto `ChainEvent` into the store's row type, field by field.
pub fn proto_to_audit_event(event: ChainEvent) -> AuditEvent {
    AuditEvent {
        event_id: event.event_id,
        prev_hash: event.prev_hash,
        event_hash: event.event_hash,
        timestamp: event.timestamp.map(|ts| ts.to_string()).unwrap_or_default(),
        actor_id: event.actor_id,
        capability_id: event.capability_id,
        plugin_id: event.plugin_id,
        schema_version: event.schema_version,
        operation_type: event.operation_type,
        target: event.target,
        tags_touched: event.tags_touched,
        decision: decision_label(event.decision).to_string(),
        deny_reason: event
            .deny_reason
            .and_then(|r| r.reason)
            .map(describe_deny_reason)
            .unwrap_or_default(),
        input_patch_hash: event.input_patch_hash,
        result_effective_hash: event.result_effective_hash,
    }
}

/// Proto `Decision` discriminant to display text.
fn decision_label(decision: i32) -> &'static str {
    match Decision::try_from(decision) {
        Ok(Decision::Allow) => "Allow",
        Ok(Decision::Deny) => "Deny",
        _ => "Unspecified",
    }
}

/// Render a `DenyReason` oneof as an operator-readable line.
fn describe_deny_reason(reason: deny_reason::Reason) -> String {
    match reason {
        deny_reason::Reason::TagLock(v) => {
            format!("tag_lock: tag={} wrapper_id={}", v.tag, v.wrapper_id)
        }
        deny_reason::Reason::ConstraintFail(v) => {
            format!("constraint_fail: {} — {}", v.constraint, v.message)
        }
        deny_reason::Reason::CapabilityMissing(v) => {
            format!("capability_missing: {}", v.capability)
        }
        deny_reason::Reason::ReadOnlyViolation(v) => {
            format!("read_only_violation: field={}", v.field)
        }
    }
}
