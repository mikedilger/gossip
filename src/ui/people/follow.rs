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

    ui.label(
        "NOTICE: Gossip doesn't update the filters when you follow someone yet, so you have to restart the client to fetch their events. Will fix soon.
",
    );

    ui.label("NOTICE: use CTRL-V to paste (middle/right click won't work)");

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    ui.heading("Follow an nprofile");

    ui.horizontal(|ui| {
        ui.label("Enter");
        ui.add(text_edit_line!(app, app.nprofile_follow).hint_text("nprofile1..."));
    });
    if ui.button("follow").clicked() {
        let _ = GLOBALS.to_overlord.send(ToOverlordMessage::FollowNprofile(
            app.nprofile_follow.clone(),
        ));
        app.nprofile_follow = "".to_owned();
    }

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    ui.heading("NIP-05: Follow a DNS ID");

    ui.horizontal(|ui| {
        ui.label("Enter user@domain");
        ui.add(text_edit_line!(app, app.nip05follow).hint_text("user@domain"));
    });
    if ui.button("follow").clicked() {
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::FollowNip05(app.nip05follow.clone()));
        app.nip05follow = "".to_owned();
    }

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    ui.heading("Follow a public key at a relay");

    ui.horizontal(|ui| {
        ui.label("Enter public key");
        ui.add(text_edit_line!(app, app.follow_pubkey).hint_text("npub1 or hex"));
    });
    ui.horizontal(|ui| {
        ui.label("Enter a relay URL where we can find them");
        ui.add(text_edit_line!(app, app.follow_pubkey_at_relay).hint_text("wss://..."));
    });
    if ui.button("follow").clicked() {
        if let Ok(url) = RelayUrl::try_from_str(&app.follow_pubkey_at_relay) {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::FollowPubkeyAndRelay(
                    app.follow_pubkey.clone(),
                    url,
                ));
            app.follow_pubkey = "".to_owned();
            app.follow_pubkey_at_relay = "".to_owned();
        } else {
            GLOBALS
                .status_queue
                .write()
                .write("Invalid Relay Url".to_string());
        }
    }

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);
}
