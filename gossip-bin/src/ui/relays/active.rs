use std::collections::HashSet;

use super::GossipUi;
use crate::ui::widgets;
use crate::ui::Page;
use eframe::egui;
use egui::{Context, Ui};
use egui_winit::egui::Id;
use gossip_lib::Relay;
use gossip_lib::GLOBALS;
use nostr_types::RelayUrl;

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    let is_editing = app.relays.edit.is_some();
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

    widgets::page_header(
        ui,
        &format!(
            "{} ({} relays)",
            Page::RelaysActivityMonitor.name(),
            relays.len()
        ),
        |ui| {
            if is_editing {
                ui.disable();
            }
            super::configure_list_btn(app, ui);
            btn_h_space!(ui);
            super::relay_filter_combo(app, ui);
            btn_h_space!(ui);
            super::relay_sort_combo(app, ui);
            btn_h_space!(ui);
            widgets::TextEdit::search(&app.theme, &app.assets, &mut app.relays.search)
                .desired_width(super::SEARCH_WIDTH)
                .show(ui);
            if widgets::Button::primary(&app.theme, "Add Relay")
                .show(ui)
                .clicked()
            {
                super::start_entry_dialog(app);
            }
        },
    );

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
        .db()
        .filter_relays(|relay| {
            (connected_relays.contains(&relay.url) || timeout_relays.contains(&relay.url))
                && super::filter_relay(&app.relays, relay)
        })
        .unwrap_or_default();

    relays.sort_by(|a, b| super::sort_relay(&app.relays, a, b));
    relays
}
