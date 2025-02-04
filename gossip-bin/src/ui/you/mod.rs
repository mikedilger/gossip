use super::{GossipUi, Page};
use crate::ui::widgets::CopyButton;
use eframe::egui::{self, Margin};
use egui::{Color32, Context, Frame, Stroke, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::{Globals, GLOBALS};
use nostr_types::{KeySecurity, PublicKeyHex};
use zeroize::Zeroize;

mod delegation;
mod metadata;
mod nostr_connect;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    if app.page == Page::YourKeys {
        ui.add_space(10.0);
        ui.heading("My Keys");

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        app.vert_scroll_area().id_salt("your_keys").show(ui, |ui| {
            if GLOBALS.identity.is_unlocked() {
                ui.heading("Ready to sign events");

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                show_pub_key_detail(app, ui);

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
            } else if GLOBALS.identity.has_private_key() {
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
                        ui.heading("Passphrase Needed");
                        offer_unlock_priv_key(app, ui);
                    });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                show_pub_key_detail(app, ui);

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                offer_delete(app, ui);
            } else if GLOBALS.identity.public_key().is_some() {
                show_pub_key_detail(app, ui);

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                offer_import_priv_key(app, ui);

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                offer_delete_or_import_pub_key(app, ui);
            } else {
                offer_generate(app, ui);

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                offer_import_priv_key(app, ui);

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                offer_delete_or_import_pub_key(app, ui);
            }
        });
    } else if app.page == Page::YourMetadata {
        metadata::update(app, ctx, _frame, ui);
    } else if app.page == Page::YourDelegation {
        delegation::update(app, ctx, _frame, ui);
    } else if app.page == Page::YourNostrConnect {
        nostr_connect::update(app, ctx, _frame, ui);
    }
}

fn show_pub_key_detail(app: &mut GossipUi, ui: &mut Ui) {
    // Render public key if available
    if let Some(public_key) = GLOBALS.identity.public_key() {
        ui.heading("Public Key");
        ui.add_space(10.0);

        let pkhex: PublicKeyHex = public_key.into();
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Public Key (Hex): {}", pkhex.as_str()));
            if ui.add(CopyButton::new()).clicked() {
                ui.output_mut(|o| o.copied_text = pkhex.into_string());
            }
        });

        let bech32 = public_key.as_bech32_string();
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("Public Key (bech32): {}", bech32));
            if ui.add(CopyButton::new()).clicked() {
                ui.output_mut(|o| o.copied_text = bech32.clone());
            }
        });
        ui.add_space(10.0);
        app.render_qr(ui, "you_npub_qr", &bech32);

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        if let Some(profile) = Globals::get_your_nprofile() {
            ui.heading("N-Profile");
            ui.add_space(10.0);

            let nprofile = profile.as_bech32_string();
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("Your Profile: {}", &nprofile));
                if ui.add(CopyButton::new()).clicked() {
                    ui.output_mut(|o| o.copied_text = nprofile.clone());
                }
            });
            ui.add_space(10.0);
            app.render_qr(ui, "you_nprofile_qr", &nprofile);
        }
    }
}

pub(super) fn offer_unlock_priv_key(app: &mut GossipUi, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.label("Passphrase: ");
        let response = ui.add(text_edit_line!(app, app.password).password(true));
        if app.unlock_needs_focus {
            response.request_focus();
            app.unlock_needs_focus = false;
        }
        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let _ = gossip_lib::Overlord::unlock_key(app.password.clone());
            app.password.zeroize();
            app.password = "".to_owned();
            app.draft_needs_focus = true;
        }
        if ui.button("Unlock Private Key").clicked() {
            let _ = gossip_lib::Overlord::unlock_key(app.password.clone());
            app.password.zeroize();
            app.password = "".to_owned();
            app.draft_needs_focus = true;
        }
    });
}

fn show_priv_key_detail(_app: &mut GossipUi, ui: &mut Ui) {
    let key_security = GLOBALS.identity.key_security().unwrap();

    if let Some(epk) = GLOBALS.identity.encrypted_private_key() {
        ui.heading("Encrypted Private Key");
        ui.horizontal_wrapped(|ui| {
            ui.label(&epk.0);
            if ui.add(CopyButton::new()).clicked() {
                ui.output_mut(|o| o.copied_text = epk.to_string());
            }
        });

        ui.add_space(10.0);

        ui.label(&*format!(
            "Private Key security is {}",
            match key_security {
                KeySecurity::Weak => "weak",
                KeySecurity::Medium => "medium",
                KeySecurity::NotTracked => "not tracked",
            }
        ));
    }
}

fn offer_change_password(app: &mut GossipUi, ui: &mut Ui) {
    ui.heading("Change Passphrase");

    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label("Enter Existing Passphrase: ");
        ui.add(text_edit_line!(app, app.password).password(true));
    });

    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label("Enter New Passphrase: ");
        ui.add(text_edit_line!(app, app.password2).password(true));
    });

    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label("Repeat New Passphrase: ");
        ui.add(text_edit_line!(app, app.password3).password(true));
    });

    if ui.button("Change Passphrase").clicked() {
        if app.password2 != app.password3 {
            GLOBALS
                .status_queue
                .write()
                .write("Passphrases do not match.".to_owned());
            app.password2.zeroize();
            app.password2 = "".to_owned();
            app.password3.zeroize();
            app.password3 = "".to_owned();
        } else {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::ChangePassphrase {
                    old: app.password.clone(),
                    new: app.password2.clone(),
                });
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
    let key_security = GLOBALS.identity.key_security().unwrap();

    ui.heading("Raw Export");
    if key_security == KeySecurity::Medium {
        ui.label("WARNING: This will downgrade your key security to WEAK");
    }

    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label("Enter Passphrase To Export: ");
        ui.add(text_edit_line!(app, app.password).password(true));
    });

    if ui.button("Export Private Key as bech32").clicked() {
        match GLOBALS.identity.export_private_key_bech32(&app.password) {
            Ok((mut bech32, _)) => {
                println!("Exported private key (bech32): {}", bech32);
                bech32.zeroize();
                GLOBALS.status_queue.write().write(
                    "Exported key has been printed to the console standard output.".to_owned(),
                );
            }
            Err(e) => GLOBALS.status_queue.write().write(format!("{}", e)),
        }
        app.password.zeroize();
        app.password = "".to_owned();
    }
    if ui.button("Export Private Key as hex").clicked() {
        match GLOBALS.identity.export_private_key_hex(&app.password) {
            Ok((mut hex, _)) => {
                println!("Exported private key (hex): {}", hex);
                hex.zeroize();
                GLOBALS.status_queue.write().write(
                    "Exported key has been printed to the console standard output.".to_owned(),
                );
            }
            Err(e) => GLOBALS.status_queue.write().write(format!("{}", e)),
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
            text_edit_line!(app, app.import_priv)
                .hint_text("nsec1, or hex")
                .desired_width(f32::INFINITY)
                .password(true),
        );
    });
    ui.horizontal(|ui| {
        ui.label("Enter a passphrase to keep it encrypted under");
        ui.add(text_edit_line!(app, app.password).password(true));
    });
    ui.horizontal(|ui| {
        ui.label("Repeat passphrase to be sure");
        ui.add(text_edit_line!(app, app.password2).password(true));
    });
    if ui.button("import").clicked() {
        if app.password != app.password2 {
            GLOBALS
                .status_queue
                .write()
                .write("Passwords do not match".to_owned());
        } else {
            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::ImportPriv {
                privkey: app.import_priv.clone(),
                password: app.password.clone(),
            });
        }
        app.password.zeroize();
        app.password = "".to_owned();
        app.password2.zeroize();
        app.password2 = "".to_owned();
    }

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    ui.heading("Import an Encrypted Private Key");

    ui.horizontal(|ui| {
        ui.label("Enter encrypted private key");
        ui.add(
            text_edit_line!(app, app.import_priv)
                .hint_text("ncryptsec1")
                .desired_width(f32::INFINITY)
                .password(true),
        );
    });
    ui.horizontal(|ui| {
        ui.label("Enter the passphrase it is encrypted under");
        ui.add(text_edit_line!(app, app.password).password(true));
    });
    if ui.button("import").clicked() {
        let _ = GLOBALS.to_overlord.send(ToOverlordMessage::ImportPriv {
            privkey: app.import_priv.clone(),
            password: app.password.clone(),
        });
        app.import_priv = "".to_owned();
        app.password.zeroize();
        app.password = "".to_owned();
    }
}

fn offer_delete_or_import_pub_key(app: &mut GossipUi, ui: &mut Ui) {
    if let Some(pk) = GLOBALS.identity.public_key() {
        ui.heading("Public Key");
        ui.add_space(10.0);

        let pkhex: PublicKeyHex = pk.into();
        ui.horizontal(|ui| {
            ui.label(format!("Public Key (Hex): {}", pkhex.as_str()));
            if ui.add(CopyButton::new()).clicked() {
                ui.output_mut(|o| o.copied_text = pkhex.into_string());
            }
        });

        let bech32 = pk.as_bech32_string();
        ui.horizontal(|ui| {
            ui.label(format!("Public Key (bech32): {}", bech32));
            if ui.add(CopyButton::new()).clicked() {
                ui.output_mut(|o| o.copied_text = bech32);
            }
        });

        if ui.button("Delete this public key").clicked() {
            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::DeletePub);
        }
    } else {
        ui.heading("Import a Public Key");
        ui.add_space(10.0);

        ui.label("This won't let you post or react to posts, but you can view other people's posts (and fetch your following list) with just a public key.");

        ui.horizontal_wrapped(|ui| {
            ui.label("Enter your public key");
            ui.add(
                text_edit_line!(app, app.import_pub)
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

pub(super) fn offer_delete(app: &mut GossipUi, ui: &mut Ui) {
    ui.heading("DELETE This Identity");

    ui.horizontal_wrapped(|ui| {
        if app.delete_confirm {
            ui.label("Please confirm that you really mean to do this: ");
            if ui.button("DELETE (Yes I'm Sure)").clicked() {
                let _ = GLOBALS.to_overlord.send(ToOverlordMessage::DeletePriv);
                app.delete_confirm = false;
            }
        } else {
            if ui.button("DELETE (Cannot be undone!)").clicked() {
                app.delete_confirm = true;
            }
        }
    });
}

fn offer_generate(app: &mut GossipUi, ui: &mut Ui) {
    ui.heading("Generate a Keypair");

    ui.horizontal(|ui| {
        ui.label("Enter a passphrase to keep it encrypted under");
        ui.add(text_edit_line!(app, app.password).password(true));
    });
    ui.horizontal(|ui| {
        ui.label("Repeat passphrase to be sure");
        ui.add(text_edit_line!(app, app.password2).password(true));
    });
    if ui.button("Generate Now").clicked() {
        if app.password != app.password2 {
            GLOBALS
                .status_queue
                .write()
                .write("Passwords do not match".to_owned());
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
