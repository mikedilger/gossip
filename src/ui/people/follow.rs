use super::GossipUi;
use crate::comms::BusMessage;
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{Context, TextEdit, Ui};

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.add_space(30.0);

    ui.heading("NOTICE: Gossip doesn't update the filters when you follow someone yet, so you have to restart the client to fetch their events. Will fix soon.");

    ui.heading("NOTICE: Gossip is not synchronizing with data on the nostr relays. This is a separate list and it won't overwrite anything.");

    ui.label("NOTICE: use CTRL-V to paste (middle/right click wont work)");

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    ui.heading("NIP-35: Follow a DNS ID");

    ui.horizontal(|ui| {
        ui.label("Enter user@domain");
        ui.add(TextEdit::singleline(&mut app.nip35follow).hint_text("user@domain"));
    });
    if ui.button("follow").clicked() {
        let tx = GLOBALS.to_overlord.clone();
        let _ = tx.send(BusMessage {
            target: "overlord".to_string(),
            kind: "follow_nip35".to_string(),
            json_payload: serde_json::to_string(&app.nip35follow).unwrap(),
        });
        app.nip35follow = "".to_owned();
    }

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    ui.heading("Follow a bech32 public key");

    ui.horizontal(|ui| {
        ui.label("Enter bech32 public key");
        ui.add(TextEdit::singleline(&mut app.follow_bech32_pubkey).hint_text("npub1..."));
    });
    ui.horizontal(|ui| {
        ui.label("Enter a relay URL where we can find them");
        ui.add(TextEdit::singleline(&mut app.follow_pubkey_at_relay).hint_text("wss://..."));
    });
    if ui.button("follow").clicked() {
        let tx = GLOBALS.to_overlord.clone();
        let _ = tx.send(BusMessage {
            target: "overlord".to_string(),
            kind: "follow_bech32".to_string(),
            json_payload: serde_json::to_string(&(
                &app.follow_bech32_pubkey,
                &app.follow_pubkey_at_relay,
            ))
            .unwrap(),
        });
        app.follow_bech32_pubkey = "".to_owned();
        app.follow_pubkey_at_relay = "".to_owned();
    }

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    ui.heading("Follow a hex public key");

    ui.horizontal(|ui| {
        ui.label("Enter hex-encoded public key");
        ui.add(TextEdit::singleline(&mut app.follow_hex_pubkey).hint_text("0123456789abcdef..."));
    });
    ui.horizontal(|ui| {
        ui.label("Enter a relay URL where we can find them");
        ui.add(TextEdit::singleline(&mut app.follow_pubkey_at_relay).hint_text("wss://..."));
    });
    if ui.button("follow").clicked() {
        let tx = GLOBALS.to_overlord.clone();
        let _ = tx.send(BusMessage {
            target: "overlord".to_string(),
            kind: "follow_hexkey".to_string(),
            json_payload: serde_json::to_string(&(
                &app.follow_hex_pubkey,
                &app.follow_pubkey_at_relay,
            ))
            .unwrap(),
        });
        app.follow_hex_pubkey = "".to_owned();
        app.follow_pubkey_at_relay = "".to_owned();
    }
}
