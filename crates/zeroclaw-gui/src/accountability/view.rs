//! egui rendering for the Accountability pane.
//!
//! Layout:
//! ```text
//! Accountability
//! Reasoning audit trail and PII review.
//! ┌───────────────────────────────────────────────────────────┐
//! │ plugin [____] actor [____] decision [All ▾]  [Refresh]    │
//! └───────────────────────────────────────────────────────────┘
//! ┌───────────────────────────────────────────────────────────┐
//! │ #  │ timestamp │ actor │ plugin │ method │ decision │ hash │
//! │ ▸ 42 │ …       │ …     │ …      │ …      │ Allow    │ ab12 │
//! │   └ detail: every ChainEvent field, for human PII review   │
//! └───────────────────────────────────────────────────────────┘
//! [◀ Older]  [Newer ▶]  showing N events
//! ```
//!
//! v1 "PII review" is the detail row: it surfaces the complete event record —
//! including `input_patch_hash` and `result_effective_hash` — so an operator can
//! judge whether sensitive data flowed through a call and correlate it with
//! external inventories. There is no automated detection or redaction; the proto
//! `ChainEvent` carries hashes, never raw payloads.
//!
//! OSCAL subid: `exp.software.zeroclaw.accountability.view@v1`

use super::store::{AccountabilityStore, AuditEvent, DecisionFilter};
use super::transport::AccountabilityTransport;
use crate::theme::*;
use egui::{Frame, Margin, RichText, Rounding, Stroke};

/// Render the Accountability pane.
pub fn render_accountability(
    ui: &mut egui::Ui,
    store: &mut AccountabilityStore,
    ctx: &egui::Context,
) {
    // Move any completed fetch into the store.
    if store.drain_frames() {
        ctx.request_repaint();
    }

    // First visit to the tab triggers exactly one fetch — this is navigation,
    // not polling.
    if !store.initialized {
        store.request_fetch();
    }

    // Hand a queued fetch to the transport.
    if let Some(filter) = store.pending_fetch.take() {
        store.frame_rx = Some(AccountabilityTransport::spawn_fetch(filter));
    }

    if store.loading && store.should_repaint() {
        ctx.request_repaint();
    }

    ui.label(
        RichText::new("Accountability")
            .size(22.0)
            .strong()
            .color(FG),
    );
    ui.add_space(4.0);
    ui.label(
        RichText::new("Reasoning audit trail and PII review.")
            .color(MUTED)
            .size(13.0),
    );
    ui.add_space(12.0);

    filter_bar(ui, store);
    ui.add_space(12.0);

    if let Some(error) = store.error.clone() {
        card(ui, |ui| {
            ui.label(
                RichText::new("Query failed")
                    .color(DANGER)
                    .size(13.0)
                    .strong(),
            );
            ui.add_space(4.0);
            ui.label(RichText::new(error).color(MUTED).size(12.0));
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!("Endpoint: {}", AccountabilityTransport::endpoint()))
                    .color(MUTED)
                    .size(11.0),
            );
        });
        ui.add_space(12.0);
    }

    let visible: Vec<AuditEvent> = store.visible_events().into_iter().cloned().collect();

    if visible.is_empty() {
        card(ui, |ui| {
            if store.loading {
                ui.label(
                    RichText::new("Loading audit trail…")
                        .color(MUTED)
                        .size(13.0),
                );
            } else if store.error.is_some() {
                ui.label(RichText::new("No events to show.").color(MUTED).size(13.0));
            } else {
                ui.label(RichText::new("No events yet.").color(FG).size(13.0));
                ui.add_space(4.0);
                ui.label(
                    RichText::new(
                        "Every PluginV1.Call is recorded here. Trigger one \
                         (e.g. `zcall cognitive_mcp get_health`) and refresh.",
                    )
                    .color(MUTED)
                    .size(12.0),
                );
            }
        });
    } else {
        event_table(ui, store, &visible);
    }

    ui.add_space(12.0);
    pagination_bar(ui, store, visible.len());
}

fn card<R>(ui: &mut egui::Ui, body: impl FnOnce(&mut egui::Ui) -> R) -> R {
    Frame::none()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0_f32, BORDER))
        .rounding(Rounding::same(8.0))
        .inner_margin(Margin::same(16.0))
        .show(ui, body)
        .inner
}

fn filter_bar(ui: &mut egui::Ui, store: &mut AccountabilityStore) {
    card(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("plugin").color(MUTED).size(12.0));
            let plugin_changed = ui
                .add(
                    egui::TextEdit::singleline(&mut store.filter.plugin_id)
                        .hint_text("all")
                        .desired_width(140.0),
                )
                .lost_focus();

            ui.add_space(12.0);
            ui.label(RichText::new("actor").color(MUTED).size(12.0));
            ui.add(
                egui::TextEdit::singleline(&mut store.actor_query)
                    .hint_text("any (filters this page)")
                    .desired_width(160.0),
            );

            ui.add_space(12.0);
            ui.label(RichText::new("decision").color(MUTED).size(12.0));
            let mut decision_changed = false;
            egui::ComboBox::from_id_source("accountability_decision")
                .selected_text(store.filter.decision.label())
                .show_ui(ui, |ui| {
                    for option in [
                        DecisionFilter::All,
                        DecisionFilter::Allow,
                        DecisionFilter::Deny,
                    ] {
                        if ui
                            .selectable_value(&mut store.filter.decision, option, option.label())
                            .clicked()
                        {
                            decision_changed = true;
                        }
                    }
                });

            ui.add_space(16.0);
            let refresh = ui.button("Refresh").clicked();

            // A changed server-side predicate invalidates the id window.
            if plugin_changed || decision_changed || refresh {
                store.reset_range();
                store.request_fetch();
            }
        });
    });
}

fn event_table(ui: &mut egui::Ui, store: &mut AccountabilityStore, visible: &[AuditEvent]) {
    card(ui, |ui| {
        // Header
        ui.horizontal(|ui| {
            header_cell(ui, "#", 64.0);
            header_cell(ui, "timestamp", 190.0);
            header_cell(ui, "actor", 120.0);
            header_cell(ui, "plugin", 120.0);
            header_cell(ui, "method", 150.0);
            header_cell(ui, "decision", 80.0);
            header_cell(ui, "event hash", 120.0);
        });
        ui.add_space(6.0);
        ui.separator();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for event in visible {
                    let expanded = store.expanded.contains(&event.event_id);
                    ui.horizontal(|ui| {
                        let marker = if expanded { "▾" } else { "▸" };
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(format!("{marker} {}", event.event_id))
                                        .color(FG)
                                        .size(12.0),
                                )
                                .frame(false)
                                .min_size(egui::vec2(64.0, 18.0)),
                            )
                            .clicked()
                        {
                            store.toggle_expanded(event.event_id);
                        }
                        body_cell(ui, &event.timestamp, 190.0, MUTED);
                        body_cell(ui, &event.actor_id, 120.0, FG);
                        body_cell(ui, &event.plugin_id, 120.0, FG);
                        // The proto carries no method_name; `target` is the
                        // object path and `operation_type` the D-Bus verb.
                        body_cell(ui, &event.operation_type, 150.0, FG);
                        let decision_color = match event.decision.as_str() {
                            "Allow" => OK,
                            "Deny" => DANGER,
                            _ => MUTED,
                        };
                        body_cell(ui, &event.decision, 80.0, decision_color);
                        body_cell(ui, &short_hash(&event.event_hash), 120.0, MUTED);
                    });

                    if expanded {
                        detail_rows(ui, event);
                    }
                    ui.add_space(2.0);
                }
            });
    });
}

/// Every field the proto `ChainEvent` exposes, for human review (FR-5).
fn detail_rows(ui: &mut egui::Ui, event: &AuditEvent) {
    Frame::none()
        .fill(SURFACE_2)
        .rounding(Rounding::same(6.0))
        .inner_margin(Margin::symmetric(12.0, 8.0))
        .show(ui, |ui| {
            let fields: [(&str, String); 14] = [
                ("event_id", event.event_id.to_string()),
                ("timestamp", event.timestamp.clone()),
                ("actor_id", event.actor_id.clone()),
                ("capability_id", event.capability_id.clone()),
                ("plugin_id", event.plugin_id.clone()),
                ("schema_version", event.schema_version.clone()),
                ("operation_type", event.operation_type.clone()),
                ("target", event.target.clone()),
                ("tags_touched", event.tags_touched.join(", ")),
                ("decision", event.decision.clone()),
                ("deny_reason", event.deny_reason.clone()),
                ("input_patch_hash", event.input_patch_hash.clone()),
                ("result_effective_hash", event.result_effective_hash.clone()),
                ("prev_hash", event.prev_hash.clone()),
            ];
            for (name, value) in fields {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(name).color(MUTED).size(11.0).monospace());
                    ui.add_space(8.0);
                    let shown = if value.is_empty() {
                        "—"
                    } else {
                        value.as_str()
                    };
                    ui.label(RichText::new(shown).color(FG).size(11.0).monospace());
                });
            }
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("event_hash")
                        .color(MUTED)
                        .size(11.0)
                        .monospace(),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new(&event.event_hash)
                        .color(FG)
                        .size(11.0)
                        .monospace(),
                );
            });
        });
}

fn pagination_bar(ui: &mut egui::Ui, store: &mut AccountabilityStore, shown: usize) {
    ui.horizontal(|ui| {
        let can_page_back = store.events.first().map(|e| e.event_id).unwrap_or(0) > 1;
        if ui
            .add_enabled(
                can_page_back && !store.loading,
                egui::Button::new("◀ Older"),
            )
            .clicked()
        {
            store.page_back();
        }
        if ui
            .add_enabled(
                store.has_more && !store.loading,
                egui::Button::new("Newer ▶"),
            )
            .clicked()
        {
            store.page_forward();
        }
        if ui
            .add_enabled(!store.loading, egui::Button::new("Latest"))
            .clicked()
        {
            store.reset_range();
            store.request_fetch();
        }

        ui.add_space(12.0);
        let mut status = format!("showing {shown}");
        if store.actor_query.trim().is_empty() {
            // Without a client-side filter, page length is the fetched length.
        } else {
            status.push_str(&format!(" of {} fetched", store.events.len()));
        }
        if store.has_more {
            status.push_str(" — more available");
        }
        ui.label(RichText::new(status).color(MUTED).size(12.0));
    });
}

fn header_cell(ui: &mut egui::Ui, text: &str, width: f32) {
    ui.allocate_ui(egui::vec2(width, 18.0), |ui| {
        ui.label(RichText::new(text).color(MUTED).size(11.0).strong());
    });
}

fn body_cell(ui: &mut egui::Ui, text: &str, width: f32, color: egui::Color32) {
    ui.allocate_ui(egui::vec2(width, 18.0), |ui| {
        let shown = if text.is_empty() { "—" } else { text };
        ui.label(RichText::new(shown).color(color).size(12.0));
    });
}

/// First 12 chars of a hash — enough to correlate, short enough to fit.
fn short_hash(hash: &str) -> String {
    if hash.len() <= 12 {
        hash.to_string()
    } else {
        format!("{}…", &hash[..12])
    }
}
