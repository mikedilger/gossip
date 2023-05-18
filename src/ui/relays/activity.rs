use std::collections::HashSet;
use std::hash::Hash;

use super::{
    filter_relay, relay_filter_combo, relay_sort_combo, GossipUi, RelayFilter, RelaySorting,
};
use crate::db::DbRelay;
use crate::globals::GLOBALS;
use crate::ui::widgets;
use crate::{comms::ToOverlordMessage, ui::widgets::NavItem};
use eframe::egui;
use egui::{Context, Ui};
use egui_winit::egui::{vec2, Rect, Sense, Id, ScrollArea, Pos2};
use nostr_types::RelayUrl;

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    let is_editing = app.relay_ui.edit.is_some();
    ui.add_space(10.0);
    ui.horizontal_wrapped(|ui| {
        ui.heading("Activity Monitor");
        ui.add_space(50.0);
        ui.set_enabled(!is_editing);
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

    let scroll_size = ui.available_size_before_wrap();
    let id_source: Id = "RelayActivityMonitorScroll".into();
    let enable_scroll = app.relay_ui.edit.is_none() && !ScrollArea::is_scrolling(ui, id_source);

    egui::ScrollArea::vertical()
        .id_source(id_source)
        .show(ui, |ui| {
            let mut pos_last_entry = ui.cursor().left_top();

            for relay in relays {
                let edit = if let Some(edit_url) = &app.relay_ui.edit {
                    edit_url == &relay.url
                } else {
                    false
                };
                let enabled = edit || !is_editing;
                let mut widget =
                    widgets::RelayEntry::new(&relay)
                        .edit(edit)
                        .set_active(enabled)
                        .accent(app.settings.theme.accent_color())
                        .option_symbol(&app.options_symbol);
                if let Some(ref assignment) = GLOBALS.relay_picker.get_relay_assignment(&relay.url)
                {
                    widget = widget.user_count(assignment.pubkeys.len());
                }
                let response = ui.add_enabled(enabled, widget);
                if response.clicked() {
                    if !edit {
                        app.relay_ui.edit = Some(relay.url);
                        response.scroll_to_me(Some(egui::Align::Center));
                    } else {
                        app.relay_ui.edit = None;
                    }
                }
                pos_last_entry = response.rect.left_top();
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
            if app.relay_ui.edit.is_some() {
                let desired_size = scroll_size - vec2( 0.0 , ui.cursor().top() - pos_last_entry.y);
                ui.allocate_exact_size(desired_size, Sense::hover());
            }
        });
}
