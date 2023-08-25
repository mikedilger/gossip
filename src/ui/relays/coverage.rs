use egui_winit::egui::{Context, Ui};

use crate::{globals::GLOBALS, ui::GossipUi, comms::ToOverlordMessage};

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    let is_editing = app.relays.edit.is_some();
    ui.add_space(10.0);
    ui.horizontal_wrapped(|ui| {
        ui.heading("Coverage");
        ui.set_enabled(!is_editing);
        ui.add_space(10.0);
        if ui.button("Pick Again").clicked() {
            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::PickRelays);
        }
    });
    ui.add_space(10.0);

    if GLOBALS.relay_picker.pubkey_counts_iter().count() > 0 {
        for elem in GLOBALS.relay_picker.pubkey_counts_iter() {
            let pk = elem.key();
            let count = elem.value();
            let name = GossipUi::display_name_from_pubkey_lookup(pk);
            ui.label(format!("{}: coverage short by {} relay(s)", name, count));
        }
        ui.add_space(12.0);
        if ui.button("Pick Again").clicked() {
            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::PickRelays);
        }
    } else {
        ui.label("All followed people are fully covered.".to_owned());
    }
}
