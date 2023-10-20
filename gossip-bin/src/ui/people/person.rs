use super::{GossipUi, Page};
use crate::ui::widgets;
use crate::ui::widgets::CopyButton;
use crate::AVATAR_SIZE_F32;
use eframe::egui;
use egui::{Context, Image, RichText, TextEdit, Ui, Vec2};
use egui_winit::egui::InnerResponse;
use egui_winit::egui::Response;
use egui_winit::egui::Widget;
use egui_winit::egui::vec2;
use gossip_lib::DmChannel;
use gossip_lib::FeedKind;
use gossip_lib::PersonList;
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

    ui.add_space(20.0);
    ui.horizontal(|ui|{
        ui.add_space(15.0);
        let display_name = gossip_lib::names::display_name_from_person(&person);
        ui.label(RichText::new(display_name)
            .size(22.0)
            .color(app.theme.accent_color()));
    });
    ui.add_space(20.0);

    app.vert_scroll_area()
        .id_source("person page")
        .max_width(f32::INFINITY)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            content(app, ctx, ui, pubkey, person);
        });
}

const ITEM_V_SPACE: f32 = 2.0;
const AVATAR_COL_WIDTH: f32 = AVATAR_SIZE_F32 * 3.0 + 60.0;

fn content(app: &mut GossipUi, ctx: &Context, ui: &mut Ui, pubkey: PublicKey, person: Person) {
    let npub = pubkey.as_bech32_string();
    let mut lud06 = "unable to get lud06".to_owned();
    let mut lud16 = "unable to get lud16".to_owned();
    // let name = person.display_name()
    //     .unwrap_or(person.nip05()
    //         .unwrap_or(npub.as_str()));

    ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui|{
        ui.allocate_ui_with_layout(
            vec2(ui.available_width() - AVATAR_COL_WIDTH, f32::INFINITY),
            egui::Layout::top_down(egui::Align::TOP).with_cross_justify(true),
            |ui|{ // left column
            let person = person.clone();
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
                        ui.add_space(ITEM_V_SPACE);
                        ui.horizontal(|ui|{
                            if let Some(petname) = person.petname.clone() {
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

            if let Some(about) = person.about() {
                profile_item(ui, "about", about);
            }

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

                    let relays = GLOBALS.people.get_active_person_write_relays();
                    let relays_str: String = relays.iter()
                        .map(|f| f.0.host())
                        .collect::<Vec<String>>()
                        .join(", ");

                    profile_item(ui, "Relays", relays_str);

                    // Option to manually add a relay for them
                    widgets::list_entry::make_frame(ui)
                        .fill(egui::Color32::TRANSPARENT)
                        .show(ui, |ui| {
                            ui.vertical(|ui| {
                                ui.label(RichText::new("MANUAL RELAY").weak());
                                ui.add_space(ITEM_V_SPACE);
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
                            });
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

        // avatar column
        ui.allocate_ui_with_layout(
            vec2( AVATAR_COL_WIDTH, f32::INFINITY),
            egui::Layout::top_down_justified(egui::Align::TOP).with_cross_justify(true),
            |ui|{ // right column
            ui.add_space(10.0);

            let avatar = if let Some(avatar) = app.try_get_avatar(ctx, &pubkey) {
                avatar
            } else {
                app.placeholder_avatar.clone()
            };
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                ui.vertical_centered_justified(|ui|{
                    let followed = person.is_in_list(PersonList::Followed);
                    let muted = person.is_in_list(PersonList::Muted);
                    let is_self = if let Some(pubkey) = GLOBALS.signer.public_key() {
                        pubkey == person.pubkey
                    } else {
                        false
                    };

                    ui.add(
                        Image::new(&avatar)
                            .max_size(Vec2 {
                                x: AVATAR_SIZE_F32 * 3.0,
                                y: AVATAR_SIZE_F32 * 3.0,
                            })
                            .maintain_aspect_ratio(true),
                    );

                    const MIN_SIZE: Vec2 = vec2(40.0, 25.0);
                    const BTN_SPACING: f32 = 15.0;
                    ui.add_space(20.0);

                    ui.vertical_centered_justified(|ui|{
                        // *ui.style_mut() = app.theme.get_on_accent_style();

                        if ui.add(egui::Button::new("View posts").min_size(MIN_SIZE)).clicked() {
                            app.set_page(Page::Feed(FeedKind::Person(person.pubkey)));
                        }
                        ui.add_space(BTN_SPACING);
                        if ui.add(egui::Button::new("Send message").min_size(MIN_SIZE)).clicked() {
                            let channel = DmChannel::new(&[person.pubkey]);
                            app.set_page(Page::Feed(FeedKind::DmChat(channel)));
                        };
                    });


                    ui.add_space(BTN_SPACING*2.0);

                    if !followed && ui.add(egui::Button::new("Follow").min_size(MIN_SIZE)).clicked() {
                        let _ = GLOBALS.people.follow(&person.pubkey, true, true);
                    } else if followed && ui.add(egui::Button::new("Unfollow").min_size(MIN_SIZE)).clicked() {
                        let _ = GLOBALS.people.follow(&person.pubkey, false, true);
                    }
                    ui.add_space(BTN_SPACING);
                    ui.add(egui::Button::new("Add to Priority").min_size(MIN_SIZE));
                    ui.add_space(BTN_SPACING);
                    // Do not show 'Mute' if this is yourself
                    if muted || !is_self {
                        let mute_label = if muted { "Unmute" } else { "Mute" };
                        if ui.add(egui::Button::new(mute_label).min_size(MIN_SIZE)).clicked() {
                            let _ = GLOBALS.people.mute(&person.pubkey, !muted, true);
                            app.notes.cache_invalidate_person(&person.pubkey);
                        }
                    }
                    ui.add_space(BTN_SPACING);
                });
                ui.add_space(20.0);
            });
        });

        // space column
        ui.allocate_ui_with_layout(
            vec2(20.0, f32::INFINITY),
            egui::Layout::left_to_right(egui::Align::TOP),
            |ui|{
            ui.add_space(20.0);
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
    }); // horizontal

    // Render a modal with QR based on selections made above
    const DLG_SIZE: Vec2 = vec2(300.0, 200.0);
    match app.person_qr {
        Some("npub") => {
            let ret = widgets::modal_popup(ui, DLG_SIZE, |ui| {
                    ui.vertical_centered(|ui|{
                        ui.add_space(10.0);
                        ui.heading("Public Key (npub)");
                        ui.add_space(10.0);
                        app.render_qr(ui, ctx, "person_qr", &npub);
                        ui.add_space(10.0);
                        ui.label(&npub);
                        ui.add_space(10.0);
                        if ui.link("copy npub").clicked() {
                            ui.output_mut(|o| o.copied_text = npub.to_owned());
                        }
                    });
                });
            if ret.inner.clicked() {
                app.person_qr = None;
            }
        }
        Some("lud06") => {
            let ret = widgets::modal_popup(ui, DLG_SIZE, |ui| {
                ui.vertical_centered(|ui|{
                        ui.add_space(10.0);
                        ui.heading("Lightning Network Address (lud06)");
                        ui.add_space(10.0);
                        app.render_qr(ui, ctx, "person_qr", &lud06);
                        ui.add_space(10.0);
                        ui.label(&lud06);
                        ui.add_space(10.0);
                        if ui.link("copy lud06").clicked() {
                            ui.output_mut(|o| o.copied_text = lud06.to_owned());
                        }
                    });
                });
            if ret.inner.clicked() {
                app.person_qr = None;
            }
        }
        Some("lud16") => {
            let ret = widgets::modal_popup(ui, DLG_SIZE, |ui| {
                ui.vertical_centered(|ui| {
                        ui.add_space(10.0);
                        ui.heading("Lightning Network Address (lud16)");
                        ui.add_space(10.0);
                        app.render_qr(ui, ctx, "person_qr", &lud16);
                        ui.add_space(10.0);
                        ui.label(&lud16);
                        ui.add_space(10.0);
                        if ui.link("copy lud16").clicked() {
                            ui.output_mut(|o| o.copied_text = lud16.to_owned());
                        }
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
        .clicked() {
        ui.output_mut(|o| o.copied_text = content.to_owned());
    }
}

/// A profile item with qr copy option
fn profile_item_qr(ui: &mut Ui, app: &mut GossipUi, label: impl Into<String>, display_content: impl Into<String>, qr_content: &'static str) {
    let response = profile_item_frame(ui, label, display_content, egui::Label::new("⚃")).response;

    if response
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
                ui.add_space(ITEM_V_SPACE);
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
            prepared.content_ui.min_rect().right_top() + vec2(-20.0, 0.0),
            vec2(10.0, 10.0)
        );
        prepared.content_ui.put(sym_rect, symbol);
        prepared.frame.fill = ui.visuals().extreme_bg_color;
    } else {
        prepared.frame.fill = egui::Color32::TRANSPARENT;
    }

    prepared.end(ui);

    InnerResponse { inner, response }
}
