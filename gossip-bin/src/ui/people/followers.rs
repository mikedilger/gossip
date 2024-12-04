use super::GossipUi;
use eframe::egui;
use egui::{Context, RichText, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::{Globals, Person, PersonTable, Table, GLOBALS};
use nostr_types::PublicKey;

pub(super) fn update(
    app: &mut GossipUi,
    _ctx: &Context,
    _frame: &mut eframe::Frame,
    ui: &mut Ui,
    pubkey: PublicKey,
) {
    // Make sure we are tracking them
    let _ = GLOBALS
        .to_overlord
        .send(ToOverlordMessage::TrackFollowers(pubkey));

    let person = match PersonTable::read_record(pubkey, None) {
        Ok(Some(p)) => p,
        _ => Person::new(pubkey.to_owned()),
    };

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(
            RichText::new(person.best_name())
                .size(22.0)
                .color(app.theme.accent_color()),
        );
    });

    ui.add_space(5.0);

    ui.vertical(|ui| {
        if let Some(handle) = GLOBALS.followers.try_read() {
            if let Some(set) = handle.get(&pubkey) {
                ui.label(format!("FOUND {} PEOPLE", set.len()));
            } else {
                ui.label("Not tracked");
            }
        } else {
            ui.label("Busy counting...");
        }
    });
}
