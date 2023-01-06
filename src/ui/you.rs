use super::GossipUi;
use crate::comms::ToOverlordMessage;
use crate::globals::GLOBALS;
use crate::ui::widgets::CopyButton;
use eframe::egui;
use egui::{Context, TextEdit, Ui};
use nostr_types::{KeySecurity, PublicKeyHex};
use zeroize::Zeroize;

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.add_space(30.0);

    ui.label("NOTICE: use CTRL-V to paste (middle/right click wont work)");

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    if GLOBALS.signer.blocking_read().is_ready() {
        ui.heading("Ready to sign events");

        ui.add_space(10.0);

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
        ui.horizontal(|ui| {
            ui.label(&format!("Public Key (Hex): {}", pkhex.0));
            if ui.add(CopyButton {}).clicked() {
                ui.output().copied_text = pkhex.0;
            }
        });

        if let Ok(bech32) = public_key.try_as_bech32_string() {
            ui.horizontal(|ui| {
                ui.label(&format!("Public Key (bech32): {}", bech32));
                if ui.add(CopyButton {}).clicked() {
                    ui.output().copied_text = bech32;
                }
            });
        }

        ui.add_space(10.0);

        if let Some(epk) = GLOBALS.signer.blocking_read().encrypted_private_key() {
            ui.horizontal(|ui| {
                ui.label(&format!("Encrypted Private Key: {}", epk));
                if ui.add(CopyButton {}).clicked() {
                    ui.output().copied_text = epk.to_string();
                }
            });
        }

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);
        ui.heading("Raw Export");
        if key_security == KeySecurity::Medium {
            ui.label("WARNING: This will downgrade your key security to WEAK");
        }

        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.label("Enter Password To Export: ");
            ui.add(TextEdit::singleline(&mut app.password).password(true));
        });

        if ui.button("Export Private Key as bech32").clicked() {
            match GLOBALS
                .signer
                .blocking_write()
                .export_private_key_bech32(&app.password)
            {
                Ok(mut bech32) => {
                    println!("Exported private key (bech32): {}", bech32);
                    bech32.zeroize();
                    *GLOBALS.status_message.blocking_write() =
                        "Exported key has been printed to the console standard output.".to_owned();
                }
                Err(e) => *GLOBALS.status_message.blocking_write() = format!("{}", e),
            }
            app.password.zeroize();
            app.password = "".to_owned();
        }
        if ui.button("Export Private Key as hex").clicked() {
            match GLOBALS
                .signer
                .blocking_write()
                .export_private_key_hex(&app.password)
            {
                Ok(mut hex) => {
                    println!("Exported private key (hex): {}", hex);
                    hex.zeroize();
                    *GLOBALS.status_message.blocking_write() =
                        "Exported key has been printed to the console standard output.".to_owned();
                }
                Err(e) => *GLOBALS.status_message.blocking_write() = format!("{}", e),
            }
            app.password.zeroize();
            app.password = "".to_owned();
        }

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);
        ui.heading("DELETE This Identity");

        ui.horizontal(|ui| {
            ui.add_space(10.0);
            ui.label("Enter Password To Delete: ");
            ui.add(TextEdit::singleline(&mut app.password).password(true));
        });

        if ui.button("DELETE (Cannot be undone!)").clicked() {
            match GLOBALS
                .signer
                .blocking_write()
                .delete_identity(&app.password)
            {
                Ok(_) => *GLOBALS.status_message.blocking_write() = "Identity deleted.".to_string(),
                Err(e) => *GLOBALS.status_message.blocking_write() = format!("{}", e),
            }
            app.password.zeroize();
            app.password = "".to_owned();
        }
    } else if GLOBALS.signer.blocking_read().is_loaded() {
        ui.heading("Password Needed");

        ui.horizontal(|ui| {
            ui.label("Password: ");
            ui.add(TextEdit::singleline(&mut app.password).password(true));
        });

        if ui.button("Unlock Private Key").clicked() {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::UnlockKey(app.password.clone()));
            app.password.zeroize();
            app.password = "".to_owned();
        }
    } else {
        ui.heading("Generate a Keypair");

        ui.horizontal(|ui| {
            ui.label("Enter a password to keep it encrypted under");
            ui.add(TextEdit::singleline(&mut app.password).password(true));
        });
        if ui.button("Generate Now").clicked() {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::GeneratePrivateKey(app.password.clone()));
            app.password.zeroize();
            app.password = "".to_owned();
        }

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        ui.heading("Import a Private Key");

        ui.horizontal(|ui| {
            ui.label("Enter private key");
            ui.add(
                TextEdit::singleline(&mut app.import_priv)
                    .hint_text("nsec1 or hex")
                    .password(true),
            );
        });
        ui.horizontal(|ui| {
            ui.label("Enter a password to keep it encrypted under");
            ui.add(TextEdit::singleline(&mut app.password).password(true));
        });
        if ui.button("import").clicked() {
            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::ImportPriv(
                app.import_priv.clone(),
                app.password.clone(),
            ));
            app.import_priv.zeroize();
            app.import_priv = "".to_owned();
            app.password.zeroize();
            app.password = "".to_owned();
        }

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        ui.heading("Import a Public Key");
        ui.add_space(10.0);

        ui.label("This won't let you post or react to posts, but you can view other people's posts (and fetch your following list) with just a public key.");

        if let Some(pk) = GLOBALS.signer.blocking_read().public_key() {
            let pkhex: PublicKeyHex = pk.into();
            ui.horizontal(|ui| {
                ui.label(&format!("Public Key (Hex): {}", pkhex.0));
                if ui.add(CopyButton {}).clicked() {
                    ui.output().copied_text = pkhex.0;
                }
            });

            if let Ok(bech32) = pk.try_as_bech32_string() {
                ui.horizontal(|ui| {
                    ui.label(&format!("Public Key (bech32): {}", bech32));
                    if ui.add(CopyButton {}).clicked() {
                        ui.output().copied_text = bech32;
                    }
                });
            }

            if ui.button("Delete this public key").clicked() {
                let _ = GLOBALS.to_overlord.send(ToOverlordMessage::DeletePub);
            }
        } else {
            ui.horizontal_wrapped(|ui| {
                ui.label("Enter your public key");
                ui.add(TextEdit::singleline(&mut app.import_pub).hint_text("npub1 or hex"));
                if ui.button("Import a Public Key").clicked() {
                    let _ = GLOBALS
                        .to_overlord
                        .send(ToOverlordMessage::ImportPub(app.import_pub.clone()));
                    app.import_pub = "".to_owned();
                }
            });
        }
    }
}
