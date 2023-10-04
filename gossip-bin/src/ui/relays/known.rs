use super::GossipUi;
use crate::ui::{widgets, Page};
use eframe::egui;
use egui::{Context, Ui};
use egui_winit::egui::Id;
use gossip_lib::globals::GLOBALS;
use gossip_lib::relay::Relay;

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    let is_editing = app.relays.edit.is_some();
    ui.add_space(10.0);
    ui.horizontal_wrapped(|ui| {
        ui.heading(Page::RelaysKnownNetwork.name());
        ui.set_enabled(!is_editing);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            ui.add_space(20.0);
            super::configure_list_btn(app, ui);
            ui.add_space(20.0);
            super::relay_filter_combo(app, ui);
            ui.add_space(20.0);
            super::relay_sort_combo(app, ui);
            ui.add_space(20.0);
            widgets::search_filter_field(ui, &mut app.relays.search, 200.0);
        });
    });
    ui.add_space(10.0);

    // TBD time how long this takes. We don't want expensive code in the UI
    // FIXME keep more relay info and display it
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

    let id_source: Id = "KnowRelaysScroll".into();

    super::relay_scroll_list(app, ui, relays, id_source);
}

fn get_relays(app: &mut GossipUi) -> Vec<Relay> {
    let mut relays: Vec<Relay> = GLOBALS
        .storage
        .filter_relays(|relay| {
            app.relays.show_hidden || !relay.hidden && super::filter_relay(&app.relays, relay)
        })
        .unwrap_or(Vec::new());

    relays.sort_by(|a, b| super::sort_relay(&app.relays, a, b));
    relays
}
