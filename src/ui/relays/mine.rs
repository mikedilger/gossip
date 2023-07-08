use super::{
    filter_relay, relay_filter_combo, relay_sort_combo, GossipUi,
};
use crate::db::DbRelay;
use crate::globals::GLOBALS;
use crate::ui::widgets;
use eframe::egui;
use egui::{Context, Ui};
use egui_winit::egui::Id;

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    let is_editing = app.relays.edit.is_some();
    ui.add_space(10.0);
    ui.horizontal_wrapped(|ui| {
        ui.heading("My Relays");
        ui.add_space(50.0);
        ui.set_enabled(!is_editing);
        widgets::search_filter_field(ui, &mut app.relays.search, 200.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            ui.add_space(20.0);
            relay_filter_combo(app, ui);
            ui.add_space(20.0);
            relay_sort_combo(app, ui);
        });
    });
    ui.add_space(10.0);

    let mut relays: Vec<DbRelay> = GLOBALS
        .all_relays
        .iter()
        .map(|ri| ri.value().clone())
        .filter(|ri| ri.usage_bits != 0 && filter_relay(&app.relays, ri))
        .collect();

    if !is_editing {
        relays.sort_by(|a, b| super::sort_relay(&app.relays, a, b));
    } else {
        // when editing, use constant sorting by url so the sorting doesn't change on edit
        relays.sort_by(|a, b| a.url.cmp(&b.url));
    }

    let id_source: Id = "MyRelaysScroll".into();

    super::relay_scroll_list(app, ui, relays, id_source);
}
