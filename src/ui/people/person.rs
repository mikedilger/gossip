use super::{GossipUi, Page};
use crate::comms::ToOverlordMessage;
use crate::globals::GLOBALS;
use crate::people::Person;
use crate::ui::widgets::CopyButton;
use crate::AVATAR_SIZE_F32;
use eframe::egui;
use egui::{Context, Frame, RichText, ScrollArea, Ui, Vec2};
use nostr_types::PublicKey;
use serde_json::Value;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    let (pubkey, person) = match &app.page {
        Page::Person(pubkey) => {
            let person = match GLOBALS.storage.read_person(pubkey) {
                Ok(Some(p)) => p,
                _ => Person::new(pubkey.to_owned()),
            };
            (pubkey.to_owned(), person)
        }
        _ => {
            ui.label("ERROR");
            return;
        }
    };

    ScrollArea::vertical()
        .id_source("person page")
        .override_scroll_delta(Vec2 {
            x: 0.0,
            y: app.current_scroll_offset,
        })
        .max_width(f32::INFINITY)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            content(app, ctx, ui, pubkey, person);
        });
}

fn content(app: &mut GossipUi, ctx: &Context, ui: &mut Ui, pubkey: PublicKey, person: Person) {
    ui.add_space(24.0);

    ui.horizontal(|ui| {
        // Avatar first
        let avatar = if let Some(avatar) = app.try_get_avatar(ctx, &pubkey) {
            avatar
        } else {
            app.placeholder_avatar.clone()
        };
        ui.image(
            &avatar,
            Vec2 {
                x: AVATAR_SIZE_F32 * 3.0,
                y: AVATAR_SIZE_F32 * 3.0,
            },
        );
        ui.vertical(|ui| {
            let name = GossipUi::display_name_from_dbperson(&person);
            ui.heading(name);
            ui.label(RichText::new(GossipUi::pubkey_short(&pubkey)).weak());
            GossipUi::render_person_name_line(app, ui, &person);
        });
    });

    ui.add_space(12.0);

    let npub = pubkey.as_bech32_string();
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Public Key: ").strong());
        ui.label(&npub);
        if ui
            .add(CopyButton {})
            .on_hover_text("Copy Public Key")
            .clicked()
        {
            ui.output_mut(|o| o.copied_text = npub.to_owned());
        }
        if ui.button("⚃").on_hover_text("Show as QR code").clicked() {
            app.qr_codes.remove("person_qr");
            app.person_qr = Some("npub");
        }
    });

    if let Some(name) = person.name() {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Name: ").strong());
            ui.label(name);
            if ui.add(CopyButton {}).on_hover_text("Copy Name").clicked() {
                ui.output_mut(|o| o.copied_text = name.to_owned());
            }
        });
    }

    if let Some(about) = person.about() {
        ui.label(RichText::new("About: ").strong());
        Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(about);
                if ui.add(CopyButton {}).on_hover_text("Copy About").clicked() {
                    ui.output_mut(|o| o.copied_text = about.to_owned());
                }
            });
        });
    }

    if let Some(picture) = person.picture() {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Picture: ").strong());
            ui.label(picture);
            if ui
                .add(CopyButton {})
                .on_hover_text("Copy Picture")
                .clicked()
            {
                ui.output_mut(|o| o.copied_text = picture.to_owned());
            }
        });
    }

    if let Some(nip05) = person.nip05() {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("nip05: ").strong());
            ui.label(nip05);
            if ui.add(CopyButton {}).on_hover_text("Copy nip05").clicked() {
                ui.output_mut(|o| o.copied_text = nip05.to_owned());
            }
        });
    }

    let mut lud06 = "unable to get lud06".to_owned();
    let mut lud16 = "unable to get lud16".to_owned();
    if let Some(md) = &person.metadata {
        for (key, value) in &md.other {
            let svalue = if let Value::String(s) = value {
                s.to_owned()
            } else {
                serde_json::to_string(&value).unwrap()
            };

            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(format!("{}: ", key)).strong());
                ui.label(&svalue);
                if ui
                    .add(CopyButton {})
                    .on_hover_text(format!("Copy {}", key))
                    .clicked()
                {
                    ui.output_mut(|o| o.copied_text = svalue.clone());
                }
                if key == "lud06" {
                    lud06 = svalue.to_owned();
                    if ui.button("⚃").on_hover_text("Show as QR code").clicked() {
                        app.qr_codes.remove("person_qr");
                        app.person_qr = Some("lud06");
                    }
                }
                if key == "lud16" {
                    lud16 = svalue.to_owned();
                    if ui.button("⚃").on_hover_text("Show as QR code").clicked() {
                        app.qr_codes.remove("person_qr");
                        app.person_qr = Some("lud16");
                    }
                }
            });
        }
    }

    // Render at most one QR based on selections made above
    match app.person_qr {
        Some("npub") => {
            ui.separator();
            ui.heading("Public Key (npub)");
            app.render_qr(ui, ctx, "person_qr", &npub);
            ui.label(&npub);
        }
        Some("lud06") => {
            ui.separator();
            ui.heading("Lightning Network Address (lud06)");
            app.render_qr(ui, ctx, "person_qr", &lud06);
            ui.label(&lud06);
        }
        Some("lud16") => {
            ui.separator();
            ui.heading("Lightning Network Address (lud16)");
            app.render_qr(ui, ctx, "person_qr", &lud16);
            ui.label(&lud16);
        }
        _ => {}
    }

    let mut need_to_set_active_person = true;

    if let Some(ap) = GLOBALS.people.get_active_person() {
        if ap == pubkey {
            need_to_set_active_person = false;
            app.setting_active_person = false;

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);
            ui.heading("Relays");
            let relays = GLOBALS.people.get_active_person_write_relays();
            for (relay_url, score) in relays.iter() {
                ui.label(format!("{} (score={})", relay_url, score));
            }
        }
    }
    if need_to_set_active_person && !app.setting_active_person {
        app.setting_active_person = true;
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::SetActivePerson(pubkey));
    }
}
