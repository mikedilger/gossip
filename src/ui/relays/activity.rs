use std::collections::HashSet;

use super::{
    filter_relay, relay_filter_combo, relay_sort_combo, GossipUi, RelayFilter, RelaySorting,
};
use crate::db::DbRelay;
use crate::globals::GLOBALS;
use crate::ui::widgets;
use crate::{comms::ToOverlordMessage, ui::widgets::NavItem};
use eframe::egui;
use egui::{Context, Ui};
use egui_winit::egui::{vec2, Rect, Sense};
use nostr_types::RelayUrl;

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.add_space(10.0);
    ui.horizontal_wrapped(|ui| {
        ui.heading("Activity Monitor");
        ui.add_space(50.0);
        widgets::search_filter_field(ui, &mut app.relay_ui.search, 200.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            ui.add_space(20.0);
            relay_filter_combo(app, ui, "RelayActivityMonitorFilterCombo".into());
            ui.add_space(20.0);
            relay_sort_combo(app, ui, "RelayActivityMonitorSortCombo".into());
        });
    });
    ui.add_space(10.0);

    let connected_relays: HashSet<RelayUrl> = GLOBALS
        .connected_relays
        .iter()
        .map(|r| r.key().clone())
        .collect();

    let mut relays: Vec<DbRelay> = GLOBALS
        .all_relays
        .iter()
        .map(|ri| ri.value().clone())
        .filter(|ri| connected_relays.contains(&ri.url) && filter_relay(&app.relay_ui, ri))
        .collect();

    relays.sort_by(|a, b| {
        super::sort_relay(&app.relay_ui, a, b)
    });

    egui::ScrollArea::vertical()
        .id_source("relay_coverage")
        .override_scroll_delta(egui::Vec2 {
            x: 0.0,
            y: app.current_scroll_offset,
        })
        .show(ui, |ui| {
            for relay in relays {
                let mut widget =
                    widgets::RelayEntry::new(&relay).accent(app.settings.theme.accent_color());
                if let Some(ref assignment) = GLOBALS.relay_picker.get_relay_assignment(&relay.url)
                {
                    widget = widget.user_count(assignment.pubkeys.len());
                }
                ui.add(widget);
            }

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            if ui.button("Pick Again").clicked() {
                let _ = GLOBALS.to_overlord.send(ToOverlordMessage::PickRelays);
            }

            ui.add_space(12.0);
            ui.heading("Coverage");

            if GLOBALS.relay_picker.pubkey_counts_iter().count() > 0 {
                for elem in GLOBALS.relay_picker.pubkey_counts_iter() {
                    let pk = elem.key();
                    let count = elem.value();
                    let name = GossipUi::display_name_from_pubkeyhex_lookup(pk);
                    ui.label(format!("{}: coverage short by {} relay(s)", name, count));
                }
            } else {
                ui.label("All followed people are fully covered.".to_owned());
            }
        });
}
