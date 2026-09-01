//! Accountability module — reasoning audit trail and PII review surface.
//!
//! Architecturally decoupled from the conversation path: separate store,
//! separate transport, no shared state. Queries the `EventChainService` gRPC
//! surface directly.
//!
//! The D-Bus `snowball.query_events` method exposes the same audit data to
//! MCP clients, AI agents, and `zcall` operators. Both paths read one
//! `EventChain` on the server, so the two views cannot disagree.
//!
//! Submodules:
//! - [`store`]     — page of events, filters, pagination cursor.
//! - [`transport`] — tonic `EventChainService` client.
//! - [`view`]      — egui rendering: filter bar, event table, detail rows.
//!
//! OSCAL subid: `exp.software.zeroclaw.accountability.render@v1`

pub mod store;
pub mod transport;
pub mod view;

pub use store::AccountabilityStore;
