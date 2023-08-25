use std::collections::HashSet;

use super::GossipUi;
use crate::comms::ToOverlordMessage;
use crate::globals::GLOBALS;
use crate::relay::Relay;
use crate::ui::widgets;
use eframe::egui;
use egui::{Context, Ui};
use egui_winit::egui::Id;
use nostr_types::RelayUrl;

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    let is_editing = app.relays.edit.is_some();
    ui.add_space(10.0);
    ui.horizontal_wrapped(|ui| {
        ui.heading("Active Relays");
        ui.set_enabled(!is_editing);
        ui.add_space(10.0);
        if ui.button("Pick Again").clicked() {
            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::PickRelays);
        }
        ui.add_space(50.0);
        widgets::search_filter_field(ui, &mut app.relays.search, 200.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            ui.add_space(20.0);
            super::configure_list_btn(app, ui);
            ui.add_space(20.0);
            super::relay_filter_combo(app, ui);
            ui.add_space(20.0);
            super::relay_sort_combo(app, ui);
        });
    });
    ui.add_space(10.0);

    let relays = if !is_editing {
        // clear edit cache if present
        if !app.relays.edit_relays.is_empty() {
            app.relays.edit_relays.clear()
        }
        get_relays(app)
    } else {
        // when editing, use cached list
        // build list if still empty
        if app.relays.edit_relays.is_empty() {
            app.relays.edit_relays = get_relays(app);
        }
        app.relays.edit_relays.clone()
    };

    let id_source: Id = "RelayActivityMonitorScroll".into();

    super::relay_scroll_list(app, ui, relays, id_source);
}

fn get_relays(app: &mut GossipUi) -> Vec<Relay> {
    let connected_relays: HashSet<RelayUrl> = GLOBALS
        .connected_relays
        .iter()
        .map(|r| r.key().clone())
        .collect();

    let timeout_relays: HashSet<RelayUrl> = GLOBALS
        .relay_picker
        .excluded_relays_iter()
        .map(|r| r.key().clone())
        .collect();

    let mut relays: Vec<Relay> = GLOBALS
        .storage
        .filter_relays(|relay| {
            (connected_relays.contains(&relay.url) || timeout_relays.contains(&relay.url))
                && super::filter_relay(&app.relays, relay)
        })
        .unwrap_or(Vec::new());

    relays.sort_by(|a, b| super::sort_relay(&app.relays, a, b));
    relays
}
