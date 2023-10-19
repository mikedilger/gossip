use super::{GossipUi, Page};
use crate::ui::widgets;
use crate::ui::widgets::CopyButton;
use crate::AVATAR_SIZE_F32;
use eframe::egui;
use egui::{Context, Image, RichText, TextEdit, Ui, Vec2};
use egui_winit::egui::InnerResponse;
use egui_winit::egui::Response;
use egui_winit::egui::Widget;
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
    let npub = pubkey.as_bech32_string();
    let mut lud06 = "unable to get lud06".to_owned();
    let mut lud16 = "unable to get lud16".to_owned();
    // let name = person.display_name()
    //     .unwrap_or(person.nip05()
    //         .unwrap_or(npub.as_str()));
    let display_name = gossip_lib::names::display_name_from_person(&person);

    widgets::page_header(ui, display_name.clone(), |_|{});

    ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP).with_main_justify(true), |ui|{
        ui.with_layout(egui::Layout::top_down_justified(egui::Align::TOP).with_cross_justify(true), |ui|{ // left column
            ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP).with_main_justify(true), |ui|{
                profile_item_qr(ui, app, "public key", gossip_lib::names::pubkey_short(&pubkey), "npub");
                profile_item(ui, "NIP-05", person.nip05().unwrap_or(""));
            });

            ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP).with_main_justify(true), |ui|{
                profile_item(ui, "name", person.name().unwrap_or(""));
                profile_item(ui, "display name", person.display_name().unwrap_or(""));
            });

            widgets::list_entry::make_frame(ui)
                .fill(egui::Color32::TRANSPARENT)
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new("PET NAME").weak());
                        ui.horizontal(|ui|{
                            if let Some(petname) = person.petname {
                                ui.label(petname);
                                ui.add_space(3.0);
                                if ui.link("change")
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked() {

                                }
                            } else {
                                ui.label(RichText::new("[not set]").italics().weak());
                                ui.add_space(3.0);
                                if ui.link("add")
                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                    .clicked() {

                                }
                            }
                        });
                    });
                });


            if let Some(md) = &person.metadata {
                for (key, value) in &md.other {
                    let svalue = if let Value::String(s) = value {
                        s.to_owned()
                    } else {
                        serde_json::to_string(&value).unwrap_or_default()
                    };

                    if key == "lud06" {
                        lud06 = svalue.to_owned();
                        profile_item_qr(ui, app, key, &svalue, "lud06");
                    } else if key == "lud16" {
                        lud16 = svalue.to_owned();
                        profile_item_qr(ui, app, key,&svalue, "lud16");
                    } else {
                        profile_item(ui, key, &svalue);
                    }
                }
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

                    ui.add_space(10.0);
                }
            }
            if need_to_set_active_person && !app.setting_active_person {
                app.setting_active_person = true;
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::SetActivePerson(pubkey));
            }
        }); // vertical
        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui|{
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
                        // ui.vertical(|ui| {

                        //     ui.heading(display_name);
                        //     ui.label(RichText::new(gossip_lib::names::pubkey_short(&pubkey)));
                        //     ui.add_space(10.0);
                        //     ui.horizontal(|ui| {
                        //         ui.label("Pet name:");
                        //         if app.editing_petname {
                        //             let edit_color = app.theme.input_text_color();
                        //             ui.add(TextEdit::singleline(&mut app.petname).text_color(edit_color));
                        //             if ui.button("save").clicked() {
                        //                 let mut person = person.clone();
                        //                 person.petname = Some(app.petname.clone());
                        //                 if let Err(e) = GLOBALS.storage.write_person(&person, None) {
                        //                     GLOBALS.status_queue.write().write(format!("{}", e));
                        //                 }
                        //                 app.editing_petname = false;
                        //                 app.notes.cache_invalidate_person(&person.pubkey);
                        //             }
                        //             if ui.button("cancel").clicked() {
                        //                 app.editing_petname = false;
                        //             }
                        //             if ui.button("remove").clicked() {
                        //                 let mut person = person.clone();
                        //                 person.petname = None;
                        //                 if let Err(e) = GLOBALS.storage.write_person(&person, None) {
                        //                     GLOBALS.status_queue.write().write(format!("{}", e));
                        //                 }
                        //                 app.editing_petname = false;
                        //                 app.notes.cache_invalidate_person(&person.pubkey);
                        //             }
                        //         } else {
                        //             match &person.petname {
                        //                 Some(pn) => {
                        //                     ui.label(pn);
                        //                     if ui.button("edit").clicked() {
                        //                         app.editing_petname = true;
                        //                         app.petname = pn.to_owned();
                        //                     }
                        //                     if ui.button("remove").clicked() {
                        //                         let mut person = person.clone();
                        //                         person.petname = None;
                        //                         if let Err(e) = GLOBALS.storage.write_person(&person, None)
                        //                         {
                        //                             GLOBALS.status_queue.write().write(format!("{}", e));
                        //                         }
                        //                         app.notes.cache_invalidate_person(&person.pubkey);
                        //                     }
                        //                 }
                        //                 None => {
                        //                     ui.label(RichText::new("none").italics());
                        //                     if ui.button("add").clicked() {
                        //                         app.editing_petname = true;
                        //                         app.petname = "".to_owned();
                        //                     }
                        //                 }
                        //             }
                        //         }
                        //     });

                        //     ui.add_space(10.0);
                        //     {
                        //         let visuals = ui.visuals_mut();
                        //         visuals.widgets.inactive.weak_bg_fill = app.theme.accent_color();
                        //         visuals.widgets.inactive.fg_stroke.width = 1.0;
                        //         visuals.widgets.inactive.fg_stroke.color =
                        //             app.theme.get_style().visuals.extreme_bg_color;
                        //         visuals.widgets.hovered.weak_bg_fill = app.theme.navigation_text_color();
                        //         visuals.widgets.hovered.fg_stroke.color = app.theme.accent_color();
                        //         visuals.widgets.inactive.fg_stroke.color =
                        //             app.theme.get_style().visuals.extreme_bg_color;
                        //         GossipUi::render_person_name_line(app, ui, &person, true);
                        //     }

                        //     if let Some(about) = person.about() {
                        //         ui.add_space(10.0);
                        //         ui.separator();
                        //         ui.add_space(10.0);
                        //         ui.horizontal_wrapped(|ui| {
                        //             ui.label(about);
                        //             if ui.add(CopyButton {}).on_hover_text("Copy About").clicked() {
                        //                 ui.output_mut(|o| o.copied_text = about.to_owned());
                        //             }
                        //         });
                        //     }
                        // });
                    },
                );
            }); // vertical
        }); // right_to_left
    }); // horizontal

    // Render a modal with QR based on selections made above
    match app.person_qr {
        Some("npub") => {
            let ret = widgets::modal_popup(ui, "Public Key (npub)", |ui| {
                    ui.vertical_centered(|ui|{
                        ui.add_space(10.0);
                        app.render_qr(ui, ctx, "person_qr", &npub);
                        ui.add_space(10.0);
                        ui.label(&npub);
                        ui.add_space(10.0);
                    });
                });
            if ret.inner.clicked() {
                app.person_qr = None;
            }
        }
        Some("lud06") => {
            let ret = widgets::modal_popup(ui, "Lightning Network Address (lud06)", |ui| {
                ui.vertical_centered(|ui|{
                        ui.add_space(10.0);
                        app.render_qr(ui, ctx, "person_qr", &lud06);
                        ui.add_space(10.0);
                        ui.label(&lud06);
                        ui.add_space(10.0);
                    });
                });
            if ret.inner.clicked() {
                app.person_qr = None;
            }
        }
        Some("lud16") => {
            let ret = widgets::modal_popup(ui, "Lightning Network Address (lud16)", |ui| {
                ui.vertical_centered(|ui|{
                        ui.add_space(10.0);
                        app.render_qr(ui, ctx, "person_qr", &lud16);
                        ui.add_space(10.0);
                        ui.label(&lud16);
                        ui.add_space(10.0);
                    });
                });
            if ret.inner.clicked() {
                app.person_qr = None;
            }
        }
        _ => {}
    }
}

/// A profile item
fn profile_item(ui: &mut Ui, label: impl Into<String>, content: impl Into<String>) {
    let content: String = content.into();
    let response = profile_item_frame(ui, label, &content, CopyButton{}).response;

    if response
        .on_hover_text("copy to clipboard")
        .clicked() {
        ui.output_mut(|o| o.copied_text = content.to_owned());
    }
}

/// A profile item with qr copy option
fn profile_item_qr(ui: &mut Ui, app: &mut GossipUi, label: impl Into<String>, display_content: impl Into<String>, qr_content: &'static str) {
    let response = profile_item_frame(ui, label, display_content, egui::Label::new("⚃")).response;

    if response
        .on_hover_text("show QR or copy to clipboard")
        .clicked() {
        app.qr_codes.remove("person_qr");
        app.person_qr = Some(qr_content);
    }
}

fn profile_item_frame(ui: &mut Ui, label: impl Into<String>, content: impl Into<String>, symbol: impl Widget) -> InnerResponse<Response> {
    let content: String = content.into();
    let label: String = label.into();

    let mut prepared = widgets::list_entry::make_frame(ui).begin(ui);
    let inner = {
        let ui =&mut prepared.content_ui;
        ui.horizontal(|ui|{
            let response = ui.vertical(|ui|{
                ui.label(RichText::new(label.to_uppercase()).weak());
                ui.add_space(2.0);
                ui.label(content);
            }).response;
            ui.add_space(20.0);
            response
        }).response
    };

    let frame_rect = (prepared.frame.inner_margin + prepared.frame.outer_margin).expand_rect(prepared.content_ui.min_rect());

    let response = ui.interact(frame_rect, ui.auto_id_with(label), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);

    if response.hovered() {
        let sym_rect = egui::Rect::from_min_size(
            prepared.content_ui.min_rect().right_top() + egui::vec2(-20.0, 0.0),
            egui::vec2(10.0, 10.0)
        );
        prepared.content_ui.put(sym_rect, symbol);
        prepared.frame.fill = ui.visuals().extreme_bg_color;
    } else {
        prepared.frame.fill = egui::Color32::TRANSPARENT;
    }

    prepared.end(ui);

    InnerResponse { inner, response }
}
