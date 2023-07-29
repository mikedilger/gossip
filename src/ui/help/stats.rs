use super::GossipUi;
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{Context, ScrollArea, Ui};
use humansize::{format_size, DECIMAL};
use std::sync::atomic::Ordering;

pub(super) fn update(_app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.add_space(24.0);
    ui.heading("Statistics".to_string());
    ui.add_space(12.0);
    ui.separator();

    ui.add_space(10.0);

    ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(10.0);

        ui.label(format!(
            "Total Bytes Read: {}",
            format_size(GLOBALS.bytes_read.load(Ordering::Relaxed), DECIMAL)
        ));

        ui.add_space(6.0);

        ui.label(format!(
            "HTTP Requests in flight: {}",
            GLOBALS.fetcher.requests_in_flight()
        ));

        ui.label(format!(
            "HTTP Requests queued: {}",
            GLOBALS.fetcher.requests_queued()
        ));

        ui.add_space(6.0);

        ui.label(format!("Events in memory: {}", GLOBALS.events.len()));

        ui.add_space(6.0);

        ui.label(format!("People in memory: {}", GLOBALS.people.len()));

        ui.add_space(6.0);

        ui.label(format!(
            "Number of known relays: {}",
            GLOBALS.all_relays.len()
        ));
    });
}
