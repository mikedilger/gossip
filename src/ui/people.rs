use super::{GossipUi, Page};
use crate::comms::BusMessage;
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{Context, RichText, ScrollArea, TextEdit, TextStyle, TopBottomPanel, Ui, Vec2};

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    TopBottomPanel::top("people_menu").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut app.page, Page::PeopleList, "Followed");
            ui.separator();
            ui.selectable_value(&mut app.page, Page::PeopleFollow, "Follow Someone New");
            ui.separator();
        });
    });

    if app.page == Page::PeopleFollow {
        ui.add_space(30.0);

        ui.heading("NOTICE: Gossip doesn't update the filters when you follow someone yet, so you have to restart the client to fetch their events. Will fix soon.");

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

        ui.add_space(8.0);
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
                    ui.image(&app.placeholder_avatar, Vec2 { x: 36.0, y: 36.0 });

                    ui.vertical(|ui| {
                        ui.label(RichText::new(GossipUi::hex_pubkey_short(&person.pubkey)).weak());

                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(person.name.as_deref().unwrap_or(""))
                                    .text_style(TextStyle::Name("Bold".into())),
                            );

                            ui.add_space(24.0);

                            if let Some(dns_id) = person.dns_id.as_deref() {
                                ui.label(dns_id);
                            }
                        });
                    });
                });

                ui.add_space(12.0);

                if let Some(about) = person.about.as_deref() {
                    ui.label(about);
                }

                ui.add_space(12.0);

                ui.separator();
            }
        });
    }
}
