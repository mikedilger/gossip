use crate::ui::GossipUi;
use crate::unsaved_settings::UnsavedSettings;
use eframe::egui;
use egui::widgets::Slider;
use egui::{Context, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::GLOBALS;

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.heading("Storage Settings");

    ui.add_space(20.0);

    ui.horizontal(|ui| {
        ui.label("When pruning events (below), How long to keep events")
            .on_hover_text("Events older than this will be deleted");
        ui.add(Slider::new(&mut app.unsaved_settings.prune_period_days, 7..=720).text("days"));
    });

    ui.horizontal(|ui| {
        ui.label("When pruning cache (below), How long to keep downloaded files")
            .on_hover_text("Cached files older than this will be deleted");
        ui.add(
            Slider::new(&mut app.unsaved_settings.cache_prune_period_days, 7..=720).text("days"),
        );
    });

    // Only let them prune after they have saved
    let stored_settings = UnsavedSettings::load();
    if stored_settings == app.unsaved_settings {
        ui.add_space(20.0);
        if ui.button("Delete Old Events Now").on_hover_text("This will delete events older than the period specified above. but the LMDB files will continue consuming disk space. To compact them, copy withem with `mdb_copy -c` when gossip is not running (see doc/DATABASE_MAINTENANCE.md).").clicked() {
            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::PruneOldEvents);
        }

        ui.add_space(20.0);
        if ui.button("Delete Unused People Now").on_hover_text("This will delete people without useful events and otherwise not referenced, but the LMDB files will continue consuming disk space. To compact them, copy withem with `mdb_copy -c` when gossip is not running (see doc/DATABASE_MAINTENANCE.md).").clicked() {
            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::PruneUnusedPeople);
        }

        ui.add_space(20.0);
        if ui.button("Delete Old Downloaded Files").on_hover_text("This will delete cache files with modification times older than the period specified above (unfortunately access times are often unavailable and/or unreliable). Note that this will eventually delete everybody's avatar, even if those are in heavy use.").clicked() {
            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::PruneCache);
        }
    }

    ui.add_space(20.0);
}
