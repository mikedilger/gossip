use std::collections::HashSet;

use super::{
    filter_relay, relay_filter_combo, relay_sort_combo, GossipUi,
};
use crate::db::DbRelay;
use crate::globals::GLOBALS;
use crate::ui::widgets::{self, RelayEntry};
use crate::{comms::ToOverlordMessage};
use eframe::egui;
use egui::{Context, Ui};
use egui_winit::egui::{vec2, Id, Sense};
use nostr_types::RelayUrl;

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    let is_editing = app.relays.edit.is_some();
    ui.add_space(10.0);
    ui.horizontal_wrapped(|ui| {
        ui.heading("Activity Monitor");
        ui.add_space(50.0);
        ui.set_enabled(!is_editing);
        widgets::search_filter_field(ui, &mut app.relays.search, 200.0);
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
        .filter(|ri| connected_relays.contains(&ri.url) && filter_relay(&app.relays, ri))
        .collect();

    relays.sort_by(|a, b| super::sort_relay(&app.relays, a, b));

    let scroll_size = ui.available_size_before_wrap();
    let id_source: Id = "RelayActivityMonitorScroll".into();

    egui::ScrollArea::vertical()
        .id_source(id_source)
        .show(ui, |ui| {
            let mut pos_last_entry = ui.cursor().left_top();
            let mut has_edit_target = false;

            for db_relay in relays {
                let db_url = db_relay.url.clone();
                let edit = if let Some(edit_url) = &app.relays.edit {
                    if edit_url == &db_url {
                        has_edit_target = true;
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };
                let enabled = edit || !is_editing;
                let mut widget = RelayEntry::new(db_relay)
                    .accent(app.settings.theme.accent_color())
                    .option_symbol(app.options_symbol.clone());
                widget.set_edit(edit);
                widget.set_active(enabled);
                if let Some(ref assignment) = GLOBALS.relay_picker.get_relay_assignment(&db_url) {
                    widget.set_user_count(assignment.pubkeys.len());
                }
                let response = ui.add_enabled(enabled, widget);
                if response.clicked() {
                    if !edit {
                        app.relays.edit = Some(db_url);
                        response.scroll_to_me(Some(egui::Align::Center));
                        has_edit_target = true;
                    } else {
                        app.relays.edit = None;
                    }
                }
                pos_last_entry = response.rect.left_top();
            }

            if !has_edit_target {
                // the relay we wanted to edit was not in the list anymore
                // -> release edit modal
                app.relays.edit = None;
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

            // add enough space to show the last relay entry at the top when editing
            if app.relays.edit.is_some() {
                let desired_size = scroll_size - vec2(0.0, ui.cursor().top() - pos_last_entry.y);
                ui.allocate_exact_size(desired_size, Sense::hover());
            }
        });
}
