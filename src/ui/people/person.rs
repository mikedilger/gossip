use super::{GossipUi, Page};
use crate::comms::ToOverlordMessage;
use crate::feed::FeedKind;
use crate::globals::GLOBALS;
use crate::people::DbPerson;
use crate::ui::widgets::CopyButton;
use crate::AVATAR_SIZE_F32;
use eframe::egui;
use egui::{Context, Frame, RichText, ScrollArea, Ui, Vec2};
use nostr_types::PublicKeyHex;
use serde_json::Value;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    let (pubkeyhex, person) = match &app.page {
        Page::Person(pubkeyhex) => {
            let person = match GLOBALS.people.get(pubkeyhex) {
                Some(p) => p,
                None => DbPerson::new(pubkeyhex.to_owned()),
            };
            (pubkeyhex.to_owned(), person)
        }
        _ => {
            ui.label("ERROR");
            return;
        }
    };

    ScrollArea::vertical()
        .id_source("person page")
        .max_width(f32::INFINITY)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            content(app, ctx, ui, pubkeyhex, person);
        });
}

fn content(
    app: &mut GossipUi,
    ctx: &Context,
    ui: &mut Ui,
    pubkeyhex: PublicKeyHex,
    person: DbPerson,
) {
    ui.add_space(24.0);

    ui.horizontal(|ui| {
        // Avatar first
        let avatar = if let Some(avatar) = app.try_get_avatar(ctx, &pubkeyhex) {
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
            ui.heading(get_name(&person));
            ui.label(RichText::new(GossipUi::hex_pubkey_short(&pubkeyhex)).weak());
            GossipUi::render_person_name_line(ui, &person);

            if person.followed == 0 && ui.button("FOLLOW").clicked() {
                GLOBALS.people.follow(&pubkeyhex, true);
            } else if person.followed == 1 && ui.button("UNFOLLOW").clicked() {
                GLOBALS.people.follow(&pubkeyhex, false);
            }

            if person.muted == 0 && ui.button("MUTE").clicked() {
                GLOBALS.people.mute(&pubkeyhex, true);
            } else if person.muted == 1 && ui.button("UNMUTE").clicked() {
                GLOBALS.people.mute(&pubkeyhex, false);
            }

            if ui.button("UPDATE METADATA").clicked() {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::UpdateMetadata(pubkeyhex.clone()));
            }

            if ui.button("VIEW THEIR FEED").clicked() {
                app.set_page(Page::Feed(FeedKind::Person(pubkeyhex.clone())));
            }
        });
    });

    ui.add_space(12.0);

    if let Some(name) = person.name() {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Name: ").strong());
            ui.label(name);
            if ui.add(CopyButton {}).on_hover_text("Copy Name").clicked() {
                ui.output().copied_text = name.to_owned();
            }
        });
    }

    if let Some(about) = person.about() {
        ui.label(RichText::new("About: ").strong());
        Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(about);
                if ui.add(CopyButton {}).on_hover_text("Copy About").clicked() {
                    ui.output().copied_text = about.to_owned();
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
                ui.output().copied_text = picture.to_owned();
            }
        });
    }

    if let Some(nip05) = person.nip05() {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("nip05: ").strong());
            ui.label(nip05);
            if ui.add(CopyButton {}).on_hover_text("Copy nip05").clicked() {
                ui.output().copied_text = nip05.to_owned();
            }
        });
    }

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
                    ui.output().copied_text = svalue;
                }
            });
        }
    }
}

fn get_name(person: &DbPerson) -> String {
    if let Some(name) = person.name() {
        name.to_owned()
    } else {
        GossipUi::hex_pubkey_short(&person.pubkey)
    }
}
