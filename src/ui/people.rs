use super::{GossipUi, Page};
use crate::comms::BusMessage;
use crate::db::DbPerson;
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{Context, Image, RichText, ScrollArea, Sense, TextEdit, TopBottomPanel, Ui, Vec2};
use nostr_types::PublicKey;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    TopBottomPanel::top("people_menu").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut app.page, Page::PeopleList, "Followed");
            ui.separator();
            ui.selectable_value(&mut app.page, Page::PeopleFollow, "Follow Someone New");
            ui.separator();
            if let Some(name) = &app.person_view_name {
                ui.selectable_value(&mut app.page, Page::Person, name);
                ui.separator();
            }
        });
    });

    if app.page == Page::PeopleFollow {
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
            ui.add(
                TextEdit::singleline(&mut app.follow_hex_pubkey).hint_text("0123456789abcdef..."),
            );
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
    } else if app.page == Page::PeopleList {
        ui.add_space(24.0);

        ui.heading("NOTICE: Gossip is not synchronizing with data on the nostr relays. This is a separate list and it won't overwrite anything.");

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        ui.heading("People Followed");
        ui.add_space(18.0);

        let people = GLOBALS.people.blocking_read().clone();

        ScrollArea::vertical().show(ui, |ui| {
            for (_, person) in people.iter() {
                if person.followed != 1 {
                    continue;
                }

                ui.horizontal(|ui| {
                    // Avatar first
                    if ui
                        .add(
                            Image::new(&app.placeholder_avatar, Vec2 { x: 36.0, y: 36.0 })
                                .sense(Sense::click()),
                        )
                        .clicked()
                    {
                        set_person_view(app, person);
                    };

                    ui.vertical(|ui| {
                        ui.label(RichText::new(GossipUi::hex_pubkey_short(&person.pubkey)).weak());

                        ui.horizontal(|ui| {
                            if let Some(name) = &person.name {
                                ui.label(RichText::new(name).strong());
                            } else {
                                ui.label(
                                    RichText::new(GossipUi::hex_pubkey_short(&person.pubkey))
                                        .weak(),
                                );
                            }

                            ui.add_space(24.0);

                            if let Some(dns_id) = &person.dns_id {
                                if person.dns_id_valid > 0 {
                                    ui.label(RichText::new(dns_id).monospace().small());
                                } else {
                                    ui.label(
                                        RichText::new(dns_id).monospace().small().strikethrough(),
                                    );
                                }
                            }
                        });
                    });
                });

                ui.add_space(4.0);

                ui.separator();
            }
        });
    } else if app.page == Page::Person {
        if app.person_view_pubkey.is_none()
            || app.person_view_person.is_none()
            || app.person_view_name.is_none()
        {
            ui.label("ERROR");
        } else {
            //let pubkey = app.person_view_pubkey.as_ref().unwrap();
            let person = app.person_view_person.as_ref().unwrap();
            let name = app.person_view_name.as_ref().unwrap();

            ui.add_space(24.0);

            ui.heading(name);

            ui.horizontal(|ui| {
                // Avatar first
                ui.image(&app.placeholder_avatar, Vec2 { x: 36.0, y: 36.0 });

                ui.vertical(|ui| {
                    ui.label(RichText::new(GossipUi::hex_pubkey_short(&person.pubkey)).weak());

                    ui.horizontal(|ui| {
                        if let Some(name) = &person.name {
                            ui.label(RichText::new(name).strong());
                        } else {
                            ui.label(
                                RichText::new(GossipUi::hex_pubkey_short(&person.pubkey)).weak(),
                            );
                        }

                        ui.add_space(24.0);

                        if let Some(dns_id) = &person.dns_id {
                            if person.dns_id_valid > 0 {
                                ui.label(RichText::new(dns_id).monospace().small());
                            } else {
                                ui.label(RichText::new(dns_id).monospace().small().strikethrough());
                            }
                        }

                        if person.followed > 0 {
                            ui.label("FOLLOWED");
                        } else {
                            ui.label("not followed");
                        }
                    });
                });
            });

            ui.add_space(12.0);

            if let Some(about) = person.about.as_deref() {
                ui.label(about);
            }

            ui.add_space(12.0);
        }
    }
}

fn set_person_view(app: &mut GossipUi, person: &DbPerson) {
    if let Ok(pk) = PublicKey::try_from_hex_string(&person.pubkey) {
        app.person_view_pubkey = Some(pk);
        app.person_view_person = Some(person.clone());
        app.person_view_name = if let Some(name) = &person.name {
            Some(name.to_string())
        } else {
            Some(GossipUi::pubkey_short(&pk))
        };
        app.page = Page::Person;
    }
}
