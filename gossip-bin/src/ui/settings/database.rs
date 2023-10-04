use crate::ui::GossipUi;
use eframe::egui;
use egui::widgets::Slider;
use egui::{Context, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::Settings;
use gossip_lib::GLOBALS;

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.heading("Storage Settings");

    ui.add_space(20.0);

    ui.horizontal(|ui| {
        ui.label("How long to keep events")
            .on_hover_text("Events older than this will be deleted");
        ui.add(Slider::new(&mut app.settings.prune_period_days, 7..=720).text("days"));
    });

    ui.horizontal(|ui| {
        ui.label("How long to keep downloaded files")
            .on_hover_text("Cached files older than this will be deleted");
        ui.add(Slider::new(&mut app.settings.cache_prune_period_days, 7..=720).text("days"));
    });

    // Only let them prune after they have saved
    let stored_settings = Settings::load();
    if stored_settings == app.settings {
        ui.add_space(20.0);
        if ui.button("Delete Old Events Now").on_hover_text("This will delete events older than the period specified above. but the LMDB files will continue consuming disk space. To compact them, copy withem with `mdb_copy -c` when gossip is not running.").clicked() {
            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::PruneDatabase);
        }

        ui.add_space(20.0);
        if ui.button("Delete Old Downloaded Files").on_hover_text("This will delete cache files with modification times older than the period specified above (unfortunately access times are often unavailable and/or unreliable). Note that this will eventually delete everybody's avatar, even if those are in heavy use.").clicked() {
            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::PruneCache);
        }
    }
}
