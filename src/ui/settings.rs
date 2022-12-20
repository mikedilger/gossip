use super::GossipUi;
use eframe::egui;
use egui::widgets::Button;
use egui::{Context, Ui};

pub(super) fn update(
    _app: &mut GossipUi,
    _ctx: &Context,
    _frame: &mut eframe::Frame,
    ui: &mut Ui,
    darkmode: bool,
) {
    ui.heading("Settings");

    #[allow(clippy::collapsible_else_if)]
    if darkmode {
        if ui
            .add(Button::new("☀ Light"))
            .on_hover_text("Switch to light mode")
            .clicked()
        {
            ui.ctx().set_visuals(super::style::light_mode_visuals());
        }
    } else {
        if ui
            .add(Button::new("🌙 Dark"))
            .on_hover_text("Switch to dark mode")
            .clicked()
        {
            ui.ctx().set_visuals(super::style::dark_mode_visuals());
        }
    }

    ui.label("SETTINGS PAGE - Coming Soon".to_string());
}
