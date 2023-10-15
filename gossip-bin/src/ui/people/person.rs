use super::{GossipUi, Page};
use crate::ui::widgets::CopyButton;
use crate::ui::PersonTab;
use crate::AVATAR_SIZE_F32;
use eframe::egui;
use egui::{Context, Image, RichText, TextEdit, Ui, Vec2};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::Person;
use gossip_lib::GLOBALS;
use nostr_types::{PublicKey, RelayUrl};
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

    app.vert_scroll_area()
        .id_source("person page")
        .max_width(f32::INFINITY)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            content(app, ctx, ui, pubkey, person);
        });
}

fn content(app: &mut GossipUi, ctx: &Context, ui: &mut Ui, pubkey: PublicKey, person: Person) {
    ui.vertical(|ui| {
        ui.add_space(10.0);
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), ui.spacing().interact_size.y),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let avatar = if let Some(avatar) = app.try_get_avatar(ctx, &pubkey) {
                        avatar
                    } else {
                        app.placeholder_avatar.clone()
                    };
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        ui.add(
                            Image::new(&avatar)
                                .max_size(Vec2 {
                                    x: AVATAR_SIZE_F32 * 3.0,
                                    y: AVATAR_SIZE_F32 * 3.0,
                                })
                                .maintain_aspect_ratio(true),
                        );
                    });
                });
                ui.vertical(|ui| {
                    let display_name = gossip_lib::names::display_name_from_person(&person);
                    ui.heading(display_name);
                    ui.label(RichText::new(gossip_lib::names::pubkey_short(&pubkey)));
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.label("Pet name:");
                        if app.editing_petname {
                            let edit_color = app.theme.input_text_color();
                            ui.add(TextEdit::singleline(&mut app.petname).text_color(edit_color));
                            if ui.button("save").clicked() {
                                let mut person = person.clone();
                                person.petname = Some(app.petname.clone());
                                if let Err(e) = GLOBALS.storage.write_person(&person, None) {
                                    GLOBALS.status_queue.write().write(format!("{}", e));
                                }
                                app.editing_petname = false;
                                app.notes.cache_invalidate_person(&person.pubkey);
                            }
                            if ui.button("cancel").clicked() {
                                app.editing_petname = false;
                            }
                            if ui.button("remove").clicked() {
                                let mut person = person.clone();
                                person.petname = None;
                                if let Err(e) = GLOBALS.storage.write_person(&person, None) {
                                    GLOBALS.status_queue.write().write(format!("{}", e));
                                }
                                app.editing_petname = false;
                                app.notes.cache_invalidate_person(&person.pubkey);
                            }
                        } else {
                            match &person.petname {
                                Some(pn) => {
                                    ui.label(pn);
                                    if ui.button("edit").clicked() {
                                        app.editing_petname = true;
                                        app.petname = pn.to_owned();
                                    }
                                    if ui.button("remove").clicked() {
                                        let mut person = person.clone();
                                        person.petname = None;
                                        if let Err(e) = GLOBALS.storage.write_person(&person, None)
                                        {
                                            GLOBALS.status_queue.write().write(format!("{}", e));
                                        }
                                        app.notes.cache_invalidate_person(&person.pubkey);
                                    }
                                }
                                None => {
                                    ui.label(RichText::new("none").italics());
                                    if ui.button("add").clicked() {
                                        app.editing_petname = true;
                                        app.petname = "".to_owned();
                                    }
                                }
                            }
                        }
                    });

                    ui.add_space(10.0);
                    {
                        let visuals = ui.visuals_mut();
                        visuals.widgets.inactive.weak_bg_fill = app.theme.accent_color();
                        visuals.widgets.inactive.fg_stroke.width = 1.0;
                        visuals.widgets.inactive.fg_stroke.color =
                            app.theme.get_style().visuals.extreme_bg_color;
                        visuals.widgets.hovered.weak_bg_fill = app.theme.navigation_text_color();
                        visuals.widgets.hovered.fg_stroke.color = app.theme.accent_color();
                        visuals.widgets.inactive.fg_stroke.color =
                            app.theme.get_style().visuals.extreme_bg_color;
                        GossipUi::render_person_name_line(app, ui, &person, true);
                    }

                    if let Some(about) = person.about() {
                        ui.add_space(10.0);
                        ui.separator();
                        ui.add_space(10.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.label(about);
                            if ui.add(CopyButton {}).on_hover_text("Copy About").clicked() {
                                ui.output_mut(|o| o.copied_text = about.to_owned());
                            }
                        });
                    }
                });
            },
        );
    });

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

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

            ui.horizontal(|ui| {
                let tab_followed =
                    ui.selectable_value(&mut app.person_tab, PersonTab::Followed, "Followed");
                if tab_followed.clicked() {
                    let _ = GLOBALS
                        .to_overlord
                        .send(ToOverlordMessage::FetchPersonContactList(person.pubkey));
                }

                ui.label("|");

                let tab_followers =
                    ui.selectable_value(&mut app.person_tab, PersonTab::Followers, "Followers");
                if tab_followers.clicked() {
                    // TODO
                }

                ui.label("|");

                let tab_relays =
                    ui.selectable_value(&mut app.person_tab, PersonTab::Relays, "Relays");
                if tab_relays.clicked() {
                    // TODO
                }
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            match app.person_tab {
                PersonTab::Followed => {
                    for followed in GLOBALS.people.get_followed(person.pubkey).iter() {
                        // tracing::debug!("Followed pubkey: {:?}", followed);
                    }
                }

                PersonTab::Followers => {}

                PersonTab::Relays => {
                    for (relay_url, score) in GLOBALS.people.get_active_person_write_relays().iter()
                    {
                        ui.label(format!("{} (score={})", relay_url, score));
                    }

                    // Add a relay for them
                    ui.add_space(10.0);
                    ui.label("Manually specify a relay they use (read and write):");
                    ui.horizontal(|ui| {
                        ui.add(text_edit_line!(app, app.add_relay).hint_text("wss://..."));
                        if ui.button("Add").clicked() {
                            if let Ok(url) = RelayUrl::try_from_str(&app.add_relay) {
                                let _ = GLOBALS
                                    .to_overlord
                                    .send(ToOverlordMessage::AddPubkeyRelay(pubkey, url));
                                app.add_relay = "".to_owned();
                            } else {
                                GLOBALS
                                    .status_queue
                                    .write()
                                    .write("Invalid Relay Url".to_string());
                            }
                        }
                    });
                }
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
