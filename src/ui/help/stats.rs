use super::GossipUi;
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{Context, Ui};
use humansize::{format_size, DECIMAL};
use std::sync::atomic::Ordering;

pub(super) fn update(_app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.label("STATS PAGE - Coming Soon".to_string());

    ui.label(format!(
        "BYTES READ: {}",
        format_size(GLOBALS.bytes_read.load(Ordering::Relaxed), DECIMAL)
    ));
}
