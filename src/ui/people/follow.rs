use super::GossipUi;
use crate::comms::ToOverlordMessage;
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{Context, Ui};
use nostr_types::PublicKey;

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

    ui.heading("Follow a public key");

    ui.horizontal(|ui| {
        ui.label("Enter public key");
        ui.add(text_edit_line!(app, app.follow_pubkey).hint_text("npub1 or hex"));
    });
    if ui.button("follow").clicked() {
        if let Ok(pubkey) = PublicKey::try_from_bech32_string(app.follow_pubkey.trim(), true) {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::FollowPubkey(pubkey));
        } else if let Ok(pubkey) = PublicKey::try_from_hex_string(app.follow_pubkey.trim(), true) {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::FollowPubkey(pubkey));
        } else {
            GLOBALS
                .status_queue
                .write()
                .write("Invalid pubkey.".to_string());
        }
        app.follow_pubkey = "".to_owned();
    }

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);
}
