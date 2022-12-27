use super::GossipUi;
use crate::comms::BusMessage;
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{Context, TextEdit, Ui};
use nostr_types::{KeySecurity, PublicKeyHex};
use tracing::info;
use zeroize::Zeroize;

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.add_space(30.0);

    ui.label("NOTICE: use CTRL-V to paste (middle/right click wont work)");

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    if GLOBALS.signer.blocking_read().is_ready() {
        ui.heading("Ready to sign events");

        let key_security = GLOBALS.signer.blocking_read().key_security().unwrap();
        let public_key = GLOBALS.signer.blocking_read().public_key().unwrap();

        ui.label(&*format!(
            "Private Key security is {}",
            match key_security {
                KeySecurity::Weak => "weak",
                KeySecurity::Medium => "medium",
            }
        ));

        let pkhex: PublicKeyHex = public_key.into();
        ui.label(&format!("Public Key: {}", pkhex.0));
    } else if GLOBALS.signer.blocking_read().is_loaded() {
        ui.heading("Password Needed");

        ui.horizontal(|ui| {
            ui.label("Password: ");
            ui.add(TextEdit::singleline(&mut app.password).password(true));
        });

        if ui.button("Unlock Private Key").clicked() {
            let tx = GLOBALS.to_overlord.clone();
            let _ = tx.send(BusMessage {
                target: "overlord".to_string(),
                kind: "unlock_key".to_string(),
                json_payload: serde_json::to_string(&app.password).unwrap(),
            });
            app.password.zeroize();
            app.password = "".to_owned();
        }
    } else {
        ui.heading("Generate a Keypair");

        if ui.button("Generate Now").clicked() {
            info!("TBD GENERATE");
        }

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        ui.heading("Import a bech32 private key");

        ui.horizontal(|ui| {
            ui.label("Enter bech32 private key");
            ui.add(
                TextEdit::singleline(&mut app.import_bech32)
                    .hint_text("nsec1...")
                    .password(true),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Enter a password to keep it encrypted under");
            ui.add(TextEdit::singleline(&mut app.password).password(true));
        });
        if ui.button("import").clicked() {
            let tx = GLOBALS.to_overlord.clone();
            let _ = tx.send(BusMessage {
                target: "overlord".to_string(),
                kind: "import_bech32".to_string(),
                json_payload: serde_json::to_string(&(&app.import_bech32, &app.password)).unwrap(),
            });
            app.import_bech32.zeroize();
            app.import_bech32 = "".to_owned();
            app.password.zeroize();
            app.password = "".to_owned();
        }

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        ui.heading("Import a hex private key");

        ui.horizontal(|ui| {
            ui.label("Enter hex-encoded private key");
            ui.add(
                TextEdit::singleline(&mut app.import_hex)
                    .hint_text("0123456789abcdef...")
                    .password(true),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Enter a password to keep it encrypted under");
            ui.add(TextEdit::singleline(&mut app.password).password(true));
        });
        if ui.button("import").clicked() {
            let tx = GLOBALS.to_overlord.clone();
            let _ = tx.send(BusMessage {
                target: "overlord".to_string(),
                kind: "import_hex".to_string(),
                json_payload: serde_json::to_string(&(&app.import_hex, &app.password)).unwrap(),
            });
            app.import_hex.zeroize();
            app.import_hex = "".to_owned();
            app.password.zeroize();
            app.password = "".to_owned();
        }
    }
}
