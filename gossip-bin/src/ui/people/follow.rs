use super::GossipUi;
use eframe::egui;
use egui::{Context, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::GLOBALS;
use nostr_types::{Profile, PublicKey};

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.add_space(10.0);
    ui.horizontal_wrapped(|ui| {
        ui.add_space(2.0);
        ui.heading("Follow Someone");
    });

    ui.add_space(10.0);

    ui.label(
        "NOTICE: Gossip doesn't update the filters when you follow someone yet, so you have to restart the client to fetch their events. Will fix soon.
",
    );

    ui.label("NOTICE: use CTRL-V to paste (middle/right click won't work)");

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    ui.heading("Follow someone");

    ui.horizontal(|ui| {
        ui.label("Enter");
        ui.add(
            text_edit_line!(app, app.follow_someone)
                .hint_text("npub1, hex key, nprofile1, or user@domain"),
        );
    });
    if ui.button("follow").clicked() {
        if let Ok(pubkey) = PublicKey::try_from_bech32_string(app.follow_someone.trim(), true) {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::FollowPubkey(pubkey, true));
        } else if let Ok(pubkey) = PublicKey::try_from_hex_string(app.follow_someone.trim(), true) {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::FollowPubkey(pubkey, true));
        } else if let Ok(profile) = Profile::try_from_bech32_string(app.follow_someone.trim(), true)
        {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::FollowNprofile(profile, true));
        } else if gossip_lib::nip05::parse_nip05(app.follow_someone.trim()).is_ok() {
            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::FollowNip05(
                app.follow_someone.trim().to_owned(),
                true,
            ));
        } else {
            GLOBALS
                .status_queue
                .write()
                .write("Invalid pubkey.".to_string());
        }
        app.follow_someone = "".to_owned();
    }

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);
}
