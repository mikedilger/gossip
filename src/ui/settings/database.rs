use crate::comms::ToOverlordMessage;
use crate::ui::GossipUi;
use crate::GLOBALS;
use eframe::egui;
use egui::widgets::Slider;
use egui::{Context, Ui};

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.heading("Database Settings");

    ui.add_space(20.0);
    ui.label("Prune period");
    ui.add(Slider::new(&mut app.settings.prune_period_days, 7..=360).text("days"));

    // Only let them prune after they have saved
    if let Ok(Some(stored_settings)) = GLOBALS.storage.read_settings() {
        if stored_settings == app.settings {
            ui.add_space(20.0);
            if ui.button("Prune Database Now")
                .on_hover_text("This will delete events older than the prune period. but the LMDB files will continue consuming disk space. To compact them, copy withem with `mdb_copy -c` when gossip is not running.")
                .clicked() {
                    GLOBALS.status_queue.write().write(
                        "Pruning database, please wait...".to_owned()
                    );
                    let _ = GLOBALS.to_overlord.send(ToOverlordMessage::PruneDatabase);
                }
        }
    }
}
