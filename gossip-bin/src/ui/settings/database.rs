use crate::ui::GossipUi;
use eframe::egui;
use egui::widgets::Slider;
use egui::{Context, Ui};

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.heading("Storage Settings");

    ui.add_space(20.0);

    ui.horizontal(|ui| {
        ui.label("When pruning events (below), How long to keep events")
            .on_hover_text("Events older than this will be deleted");
        ui.add(Slider::new(&mut app.unsaved_settings.prune_period_days, 7..=720).text("days"));
        reset_button!(app, ui, prune_period_days);
    });

    ui.horizontal(|ui| {
        ui.label("When pruning cache (below), How long to keep downloaded files")
            .on_hover_text("Cached files older than this will be deleted");
        ui.add(
            Slider::new(&mut app.unsaved_settings.cache_prune_period_days, 7..=720).text("days"),
        );
        reset_button!(app, ui, cache_prune_period_days);
    });

    ui.add_space(20.0);
    ui.label("Pruning must be done from the command line when gossip is not running. See https://github.com/mikedilger/gossip/tree/master/docs/PRUNING.md");

    ui.add_space(20.0);
}
