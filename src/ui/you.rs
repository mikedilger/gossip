use super::GossipUi;
use eframe::egui;
use egui::{Context, Ui};

pub(super) fn update(_app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.label("YOU PAGE - Coming Soon".to_string());
}
