use super::GossipUi;
use crate::comms::ToOverlordMessage;
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{Context, Ui};
use nostr_types::RelayUrl;

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.add_space(30.0);

    ui.heading("Follow Someone");
    ui.add_space(10.0);

    ui.label("NOTICE: Gossip doesn't update the filters when you follow someone yet, so you have to restart the client to fetch their events. Will fix soon.
");

    ui.label("NOTICE: use CTRL-V to paste (middle/right click won't work)");

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    ui.heading("Follow");
    ui.label("Specify a nprofile1, npub1, hex or nip05 to follow. Optionally, specify a relay URL where we can find them.");

    ui.horizontal(|ui| {
        ui.label("Enter");
        ui.add(text_edit_line!(app, app.follow_query).hint_text("nprofile, npub, hex, or nip05..."));
    });
    ui.horizontal(|ui| {
        ui.label("Enter a relay URL where we can find them (optional)");
        ui.add(text_edit_line!(app, app.follow_relay).hint_text("wss://..."));
    });

    if ui.button("follow").clicked() {
        let _ = GLOBALS.to_overlord.send(ToOverlordMessage::FollowAuto(
            app.follow_query.clone(),
            if app.follow_relay.is_empty() { None } else { RelayUrl::try_from_str(app.follow_relay.as_str()).ok() },
        ));
        app.follow_query = "".to_owned();
        app.follow_relay = "".to_owned();
    }

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);
}
