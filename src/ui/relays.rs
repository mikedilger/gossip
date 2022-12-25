use super::GossipUi;
use crate::db::DbRelay;
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{Align, Context, Layout, ScrollArea, Ui};

pub(super) fn update(_app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.add_space(8.0);
    ui.heading("Relays known");
    ui.add_space(18.0);

    let mut relays = GLOBALS.relays.blocking_lock().clone();
    let mut relays: Vec<DbRelay> = relays.drain().map(|(_, relay)| relay).collect();
    relays.sort_by(|a, b| a.url.cmp(&b.url));

    ScrollArea::vertical().show(ui, |ui| {
        for relay in relays.iter() {
            ui.horizontal(|ui| {
                ui.label(&relay.url);

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("CONNECT").clicked() {
                        ui.label("TBD");
                    }
                });
            });

            ui.add_space(12.0);
            ui.separator();
        }
    });
}
