use super::{GossipUi, Page};
use crate::comms::ToOverlordMessage;
use crate::globals::{Globals, GLOBALS};
use crate::ui::widgets::CopyButton;
use eframe::egui;
use egui::style::Margin;
use egui::{Color32, Context, Frame, ScrollArea, SelectableLabel, Stroke, TextEdit, Ui, Vec2};
use nostr_types::{KeySecurity, PublicKeyHex};
use zeroize::Zeroize;

mod metadata;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.horizontal(|ui| {
        if ui
            .add(SelectableLabel::new(app.page == Page::YourKeys, "Keys"))
            .clicked()
        {
            app.set_page(Page::YourKeys);
        }
        ui.separator();
        if ui
            .add(SelectableLabel::new(
                app.page == Page::YourMetadata,
                "Metadata",
            ))
            .clicked()
        {
            app.set_page(Page::YourMetadata);
        }
        ui.separator();
    });
    ui.separator();

    if app.page == Page::YourKeys {
        ui.add_space(10.0);
        ui.heading("Your Keys");

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        ScrollArea::vertical()
            .id_source("your_keys")
            .override_scroll_delta(Vec2 {
                x: 0.0,
                y: app.current_scroll_offset,
            })
            .show(ui, |ui| {
                if GLOBALS.signer.is_ready() {
                    ui.heading("Ready to sign events");

                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(10.0);

                    show_pub_key_detail(app, ctx, ui);

                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(10.0);

                    show_priv_key_detail(app, ui);

                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(10.0);

                    offer_change_password(app, ui);

                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(10.0);

                    offer_export_priv_key(app, ui);

                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(10.0);

                    offer_delete(app, ui);
                } else if GLOBALS.signer.is_loaded() {
                    Frame::none()
                        .stroke(Stroke {
                            width: 2.0,
                            color: Color32::RED,
                        })
                        .inner_margin(Margin {
                            left: 10.0,
                            right: 10.0,
                            top: 10.0,
                            bottom: 10.0,
                        })
                        .show(ui, |ui| {
                            offer_unlock_priv_key(app, ui);
                        });

                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(10.0);

                    show_pub_key_detail(app, ctx, ui);

                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(10.0);

                    offer_delete(app, ui);
                } else {
                    offer_generate(app, ui);

                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(10.0);

                    offer_import_priv_key(app, ui);

                    ui.add_space(10.0);
                    ui.separator();
                    ui.add_space(10.0);

                    offer_import_pub_key(app, ui);
                }
            });
    } else if app.page == Page::YourMetadata {
        metadata::update(app, ctx, _frame, ui);
    }
}

fn show_pub_key_detail(app: &mut GossipUi, ctx: &Context, ui: &mut Ui) {
    // Render public key if available
    if let Some(public_key) = GLOBALS.signer.public_key() {
        ui.heading("Public Key");
        ui.add_space(10.0);

        let pkhex: PublicKeyHex = public_key.into();
        ui.horizontal_wrapped(|ui| {
            ui.label(&format!("Public Key (Hex): {}", pkhex.as_str()));
            if ui.add(CopyButton {}).clicked() {
                ui.output_mut(|o| o.copied_text = pkhex.into_string());
            }
        });

        if let Ok(bech32) = public_key.try_as_bech32_string() {
            ui.horizontal_wrapped(|ui| {
                ui.label(&format!("Public Key (bech32): {}", bech32));
                if ui.add(CopyButton {}).clicked() {
                    ui.output_mut(|o| o.copied_text = bech32.clone());
                }
            });
            ui.add_space(10.0);
            app.render_qr(ui, ctx, "you_npub_qr", &bech32);
        }

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        if let Some(profile) = Globals::get_your_nprofile() {
            ui.heading("N-Profile");
            ui.add_space(10.0);

            let nprofile = profile.try_as_bech32_string().unwrap();
            ui.horizontal_wrapped(|ui| {
                ui.label(&format!("Your Profile: {}", &nprofile));
                if ui.add(CopyButton {}).clicked() {
                    ui.output_mut(|o| o.copied_text = nprofile.clone());
                }
            });
            ui.add_space(10.0);
            app.render_qr(ui, ctx, "you_nprofile_qr", &nprofile);
        }
    }
}

fn offer_unlock_priv_key(app: &mut GossipUi, ui: &mut Ui) {
    ui.heading("Passphrase Needed");

    ui.horizontal(|ui| {
        ui.label("Passphrase: ");
        let response = ui.add(TextEdit::singleline(&mut app.password).password(true));
        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::UnlockKey(app.password.clone()));
            app.password.zeroize();
            app.password = "".to_owned();
        }
    });

    if ui.button("Unlock Private Key").clicked() {
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::UnlockKey(app.password.clone()));
        app.password.zeroize();
        app.password = "".to_owned();
    }
}

fn show_priv_key_detail(_app: &mut GossipUi, ui: &mut Ui) {
    let key_security = GLOBALS.signer.key_security().unwrap();

    if let Some(epk) = GLOBALS.signer.encrypted_private_key() {
        ui.heading("Encrypted Private Key");
        ui.horizontal_wrapped(|ui| {
            ui.label(&epk.0);
            if ui.add(CopyButton {}).clicked() {
                ui.output_mut(|o| o.copied_text = epk.to_string());
            }
        });

        ui.add_space(10.0);

        ui.label(&*format!(
            "Private Key security is {}",
            match key_security {
                KeySecurity::Weak => "weak",
                KeySecurity::Medium => "medium",
            }
        ));
    }
}

fn offer_change_password(app: &mut GossipUi, ui: &mut Ui) {
    ui.heading("Change Passphrase");

    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label("Enter Existing Passphrase: ");
        ui.add(TextEdit::singleline(&mut app.password).password(true));
    });

    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label("Enter New Passphrase: ");
        ui.add(TextEdit::singleline(&mut app.password2).password(true));
    });

    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label("Repeat New Passphrase: ");
        ui.add(TextEdit::singleline(&mut app.password3).password(true));
    });

    if ui.button("Change Passphrase").clicked() {
        if app.password2 != app.password3 {
            *GLOBALS.status_message.blocking_write() = "Passphrases do not match.".to_owned();
            app.password2.zeroize();
            app.password2 = "".to_owned();
            app.password3.zeroize();
            app.password3 = "".to_owned();
        } else {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::ChangePassphrase(
                    app.password.clone(),
                    app.password2.clone(),
                ));
            app.password.zeroize();
            app.password = "".to_owned();
            app.password2.zeroize();
            app.password2 = "".to_owned();
            app.password3.zeroize();
            app.password3 = "".to_owned();
        }
    }
}

fn offer_export_priv_key(app: &mut GossipUi, ui: &mut Ui) {
    let key_security = GLOBALS.signer.key_security().unwrap();

    ui.heading("Raw Export");
    if key_security == KeySecurity::Medium {
        ui.label("WARNING: This will downgrade your key security to WEAK");
    }

    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label("Enter Passphrase To Export: ");
        ui.add(TextEdit::singleline(&mut app.password).password(true));
    });

    if ui.button("Export Private Key as bech32").clicked() {
        match GLOBALS.signer.export_private_key_bech32(&app.password) {
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
        match GLOBALS.signer.export_private_key_hex(&app.password) {
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
}

fn offer_import_priv_key(app: &mut GossipUi, ui: &mut Ui) {
    ui.heading("Import a Private Key");

    ui.horizontal(|ui| {
        ui.label("Enter private key");
        ui.add(
            TextEdit::singleline(&mut app.import_priv)
                .hint_text("ncryptsec1, nsec1, or hex")
                .desired_width(f32::INFINITY)
                .password(true),
        );
    });
    ui.horizontal(|ui| {
        ui.label("Enter a passphrase to keep it encrypted under");
        ui.add(TextEdit::singleline(&mut app.password).password(true));
    });
    ui.horizontal(|ui| {
        ui.label("Repeat passphrase to be sure");
        ui.add(TextEdit::singleline(&mut app.password2).password(true));
    });
    if ui.button("import").clicked() {
        if app.password != app.password2 {
            *GLOBALS.status_message.blocking_write() = "Passwords do not match".to_owned();
        } else {
            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::ImportPriv(
                app.import_priv.clone(),
                app.password.clone(),
            ));
            app.import_priv.zeroize();
            app.import_priv = "".to_owned();
        }
        app.password.zeroize();
        app.password = "".to_owned();
        app.password2.zeroize();
        app.password2 = "".to_owned();
    }
}

fn offer_import_pub_key(app: &mut GossipUi, ui: &mut Ui) {
    ui.heading("Import a Public Key");
    ui.add_space(10.0);

    ui.label("This won't let you post or react to posts, but you can view other people's posts (and fetch your following list) with just a public key.");

    if let Some(pk) = GLOBALS.signer.public_key() {
        let pkhex: PublicKeyHex = pk.into();
        ui.horizontal(|ui| {
            ui.label(&format!("Public Key (Hex): {}", pkhex.as_str()));
            if ui.add(CopyButton {}).clicked() {
                ui.output_mut(|o| o.copied_text = pkhex.into_string());
            }
        });

        if let Ok(bech32) = pk.try_as_bech32_string() {
            ui.horizontal(|ui| {
                ui.label(&format!("Public Key (bech32): {}", bech32));
                if ui.add(CopyButton {}).clicked() {
                    ui.output_mut(|o| o.copied_text = bech32);
                }
            });
        }

        if ui.button("Delete this public key").clicked() {
            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::DeletePub);
        }
    } else {
        ui.horizontal_wrapped(|ui| {
            ui.label("Enter your public key");
            ui.add(
                TextEdit::singleline(&mut app.import_pub)
                    .hint_text("npub1 or hex")
                    .desired_width(f32::INFINITY),
            );
            if ui.button("Import a Public Key").clicked() {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::ImportPub(app.import_pub.clone()));
                app.import_pub = "".to_owned();
            }
        });
    }
}

fn offer_delete(app: &mut GossipUi, ui: &mut Ui) {
    ui.heading("DELETE This Identity");

    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label("Enter Passphrase To Delete: ");
        ui.add(TextEdit::singleline(&mut app.del_password).password(true));
    });

    if ui.button("DELETE (Cannot be undone!)").clicked() {
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::DeletePriv(app.del_password.clone()));
        app.del_password.zeroize();
        app.del_password = "".to_owned();
    }
}

fn offer_generate(app: &mut GossipUi, ui: &mut Ui) {
    ui.heading("Generate a Keypair");

    ui.horizontal(|ui| {
        ui.label("Enter a passphrase to keep it encrypted under");
        ui.add(TextEdit::singleline(&mut app.password).password(true));
    });
    ui.horizontal(|ui| {
        ui.label("Repeat passphrase to be sure");
        ui.add(TextEdit::singleline(&mut app.password2).password(true));
    });
    if ui.button("Generate Now").clicked() {
        if app.password != app.password2 {
            *GLOBALS.status_message.blocking_write() = "Passwords do not match".to_owned();
        } else {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::GeneratePrivateKey(app.password.clone()));
        }
        app.password.zeroize();
        app.password = "".to_owned();
        app.password2.zeroize();
        app.password2 = "".to_owned();
    }
}
