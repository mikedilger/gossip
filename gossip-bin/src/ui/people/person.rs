use super::{GossipUi, Page};
use crate::ui::widgets;
use crate::ui::widgets::list_entry;
use crate::ui::widgets::CopyButton;
use crate::AVATAR_SIZE_F32;
use eframe::egui;
use egui::{Context, RichText, TextEdit, Ui, Vec2};
use egui_winit::egui::vec2;
use egui_winit::egui::InnerResponse;
use egui_winit::egui::Response;
use egui_winit::egui::Widget;
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::DmChannel;
use gossip_lib::FeedKind;
use gossip_lib::Person;
use gossip_lib::PersonList;
use gossip_lib::GLOBALS;
use nostr_types::{PublicKey, RelayUrl};
use serde_json::Value;

const ITEM_V_SPACE: f32 = 2.0;
const AVATAR_COL_WIDTH: f32 = AVATAR_SIZE_F32 * 3.0;
const AVATAR_COL_SPACE: f32 = 20.0;
const AVATAR_COL_WIDTH_SPACE: f32 = AVATAR_COL_WIDTH + AVATAR_COL_SPACE * 2.0;
const MIN_ITEM_WIDTH: f32 = 200.0;

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

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        let name = GossipUi::person_name(&person);
        ui.label(
            RichText::new(name)
                .size(22.0)
                .color(app.theme.accent_color()),
        );
    });

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

    let is_self = if let Some(pubkey) = GLOBALS.signer.public_key() {
        pubkey == person.pubkey
    } else {
        false
    };

    let width = ui.available_width() - AVATAR_COL_WIDTH_SPACE;
    let width = width.max(MIN_ITEM_WIDTH);
    let half_width = width / 2.0;

    ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
        // Left column, main part of page
        ui.with_layout(egui::Layout::top_down(egui::Align::TOP), |ui| {
            ui.set_min_width(width);
            ui.set_max_width(width);
            let person = person.clone();

            // "responsive" layout
            let (layout, lwidth) = if width > (MIN_ITEM_WIDTH * 2.0) {
                (egui::Layout::left_to_right(egui::Align::TOP), half_width)
            } else {
                (egui::Layout::top_down(egui::Align::TOP), width)
            };

            ui.with_layout(layout, |ui| {
                profile_item_qr(
                    ui,
                    app,
                    lwidth,
                    "public key",
                    gossip_lib::names::pubkey_short(&pubkey),
                    "npub",
                );
                profile_item(ui, app, lwidth, "NIP-05", person.nip05().unwrap_or(""));
            });

            ui.with_layout(layout, |ui| {
                profile_item(ui, app, lwidth, "name", person.name().unwrap_or(""));
                profile_item(
                    ui,
                    app,
                    lwidth,
                    "display name",
                    person.display_name().unwrap_or(""),
                );
            });

            if !is_self {
                // Petname and petname editing
                make_frame().show(ui, |ui| {
                    ui.vertical(|ui| {
                        item_label(ui, "Pet Name");
                        ui.add_space(ITEM_V_SPACE);
                        ui.horizontal(|ui| {
                            if app.editing_petname {
                                let edit_color = app.theme.input_text_color();
                                ui.add(
                                    TextEdit::singleline(&mut app.petname).text_color(edit_color),
                                );
                                if ui.link("Save").clicked() {
                                    let mut person = person.clone();
                                    if app.petname.trim().is_empty() {
                                        person.petname = None;
                                    } else {
                                        person.petname = Some(app.petname.clone());
                                    }
                                    if let Err(e) = GLOBALS.storage.write_person(&person, None) {
                                        GLOBALS.status_queue.write().write(format!("{}", e));
                                    }
                                    app.editing_petname = false;
                                    app.notes.cache_invalidate_person(&person.pubkey);
                                }
                                if ui.link("Cancel").clicked() {
                                    app.editing_petname = false;
                                }
                                if ui.link("Remove").clicked() {
                                    let mut person = person.clone();
                                    person.petname = None;
                                    if let Err(e) = GLOBALS.storage.write_person(&person, None) {
                                        GLOBALS.status_queue.write().write(format!("{}", e));
                                    }
                                    app.editing_petname = false;
                                    app.notes.cache_invalidate_person(&person.pubkey);
                                }
                            } else {
                                if let Some(petname) = person.petname.clone() {
                                    ui.label(&petname);
                                    ui.add_space(3.0);
                                    if ui.link("Edit").clicked() {
                                        app.editing_petname = true;
                                        app.petname = petname.to_owned();
                                    }
                                    if ui.link("Remove").clicked() {
                                        let mut person = person.clone();
                                        person.petname = None;
                                        if let Err(e) = GLOBALS.storage.write_person(&person, None)
                                        {
                                            GLOBALS.status_queue.write().write(format!("{}", e));
                                        }
                                        app.notes.cache_invalidate_person(&person.pubkey);
                                    }
                                } else {
                                    if ui
                                        .link("Add")
                                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                                        .clicked()
                                    {
                                        app.editing_petname = true;
                                        app.petname = "".to_owned();
                                    }
                                }
                            }
                        });
                    });
                });
            }

            if let Some(about) = person.about() {
                if !about.trim().is_empty() {
                    profile_item(ui, app, width, "about", about);
                }
            }

            ui.add_space(10.0);
            ui.heading("User lists");
            ui.separator();
            ui.add_space(10.0);

            let all_lists = PersonList::all_lists();
            if let Ok(membership_map) = GLOBALS.storage.read_person_lists(&pubkey) {
                for (list, listname) in all_lists {
                    ui.horizontal(|ui| {
                        let membership = membership_map.get(&list);
                        let mut inlist = membership.is_some();
                        if crate::ui::components::switch_simple(ui, inlist).clicked() {
                            if inlist {
                                let _ =
                                    GLOBALS.storage.remove_person_from_list(&pubkey, list, None);
                                inlist = false;
                            } else {
                                let _ = GLOBALS
                                    .storage
                                    .add_person_to_list(&pubkey, list, true, None);
                                inlist = true;
                            }
                            inlist = !inlist;
                        }
                        ui.add_space(10.0);
                        if inlist {
                            ui.label(listname);

                            ui.add_space(10.0);
                            let public = membership.unwrap();
                            // button to toggle public
                            let label = if *public { "Public" } else { "Private" };
                            if ui.button(label).clicked() {
                                let _ = GLOBALS
                                    .storage
                                    .add_person_to_list(&pubkey, list, !*public, None);
                            }
                        } else {
                            ui.label(RichText::new(listname).weak());
                        }
                    });
                }
            }

            ui.add_space(10.0);
            ui.heading("More details");
            ui.separator();
            ui.add_space(10.0);

            if let Some(md) = &person.metadata {
                // render some important fields first
                {
                    const LUD06: &str = "lud06";
                    if md.other.contains_key(LUD06) {
                        if let Some(serde_json::Value::String(svalue)) = md.other.get(LUD06) {
                            if !svalue.trim().is_empty() {
                                lud06 = svalue.to_owned();
                                profile_item_qr(ui, app, width, LUD06, svalue, LUD06);
                            }
                        }
                    }
                }

                {
                    const LUD16: &str = "lud16";
                    if md.other.contains_key(LUD16) {
                        if let Some(serde_json::Value::String(svalue)) = md.other.get(LUD16) {
                            if !svalue.trim().is_empty() {
                                lud16 = svalue.to_owned();
                                profile_item_qr(ui, app, width, LUD16, svalue, LUD16);
                            }
                        }
                    }
                }

                {
                    const WEBSITE: &str = "website";
                    if md.other.contains_key(WEBSITE) {
                        if let Some(serde_json::Value::String(svalue)) = md.other.get(WEBSITE) {
                            if !svalue.trim().is_empty() {
                                profile_item(ui, app, width, WEBSITE, svalue);
                            }
                        }
                    }
                }

                const SKIP: &[&str] = &["display_name", "lud06", "lud16", "website"];

                for (key, value) in &md.other {
                    // skip the "important" fields that are already rendered
                    if SKIP.contains(&key.as_str()) {
                        continue;
                    }

                    let svalue = if let Value::String(s) = value {
                        s.to_owned()
                    } else {
                        serde_json::to_string(&value).unwrap_or_default()
                    };

                    // skip empty fields, unless it's the main account profile
                    if !is_self && svalue.trim().is_empty() {
                        continue;
                    }

                    profile_item(ui, app, width, key, &svalue);
                }
            }

            let mut need_to_set_active_person = true;
            if let Some(ap) = GLOBALS.people.get_active_person() {
                if ap == pubkey {
                    need_to_set_active_person = false;
                    app.setting_active_person = false;

                    let mut relays = GLOBALS.people.get_active_person_write_relays();
                    relays.sort_by(|a, b| b.1.cmp(&a.1)); // list in score order
                    let relays_str: String = relays
                        .iter()
                        .filter(|f| f.1 > 5) // do not list low-score relays
                        .map(|f| f.0.host())
                        .collect::<Vec<String>>()
                        .join(", ");

                    profile_item(ui, app, width, "Relays", relays_str);

                    // Option to manually add a relay for them
                    make_frame().show(ui, |ui| {
                        ui.vertical(|ui| {
                            item_label(ui, "Manual Relay");
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

        // Right column, starting with avatar
        ui.allocate_ui_with_layout(
            vec2(AVATAR_COL_WIDTH, f32::INFINITY),
            egui::Layout::right_to_left(egui::Align::TOP).with_main_justify(true),
            |ui| {
                ui.vertical(|ui| {
                    let avatar = if let Some(avatar) = app.try_get_avatar(ctx, &pubkey) {
                        avatar
                    } else {
                        app.placeholder_avatar.clone()
                    };

                    ui.vertical_centered_justified(|ui| {
                        widgets::paint_avatar(ui, &person, &avatar, widgets::AvatarSize::Profile);

                        const MIN_SIZE: Vec2 = vec2(40.0, 22.0);
                        const BTN_SPACING: f32 = 15.0;
                        const BTN_ROUNDING: f32 = 4.0;
                        ui.add_space(40.0);

                        ui.vertical_centered_justified(|ui| {
                            app.theme.accent_button_1_style(ui.style_mut());

                            if ui
                                .add(
                                    egui::Button::new("View notes")
                                        .min_size(MIN_SIZE)
                                        .rounding(BTN_ROUNDING),
                                )
                                .clicked()
                            {
                                app.set_page(Page::Feed(FeedKind::Person(person.pubkey)));
                            }

                            ui.add_space(BTN_SPACING);

                            if !is_self {
                                if ui
                                    .add(
                                        egui::Button::new("Send a DM")
                                            .min_size(MIN_SIZE)
                                            .rounding(BTN_ROUNDING),
                                    )
                                    .clicked()
                                {
                                    let channel = DmChannel::new(&[person.pubkey]);
                                    app.set_page(Page::Feed(FeedKind::DmChat(channel)));
                                };
                            } else {
                                if ui
                                    .add(
                                        egui::Button::new("Edit Profile")
                                            .min_size(MIN_SIZE)
                                            .rounding(BTN_ROUNDING),
                                    )
                                    .clicked()
                                {
                                    app.set_page(Page::YourMetadata);
                                }
                            }
                        });
                    });
                });
                ui.add_space(AVATAR_COL_SPACE);
            },
        );
    }); // horizontal

    // Render a modal with QR based on selections made above
    const DLG_SIZE: Vec2 = vec2(300.0, 200.0);
    match app.person_qr {
        Some("npub") => {
            let ret = widgets::modal_popup(ui, DLG_SIZE, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);
                    ui.heading("Public Key (npub)");
                    ui.add_space(10.0);
                    app.render_qr(ui, ctx, "person_qr", &npub);
                    ui.add_space(10.0);
                    ui.label(&npub);
                    ui.add_space(10.0);
                    if ui.link("Copy npub").clicked() {
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
                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);
                    ui.heading("Lightning Network Address (lud06)");
                    ui.add_space(10.0);
                    app.render_qr(ui, ctx, "person_qr", &lud06);
                    ui.add_space(10.0);
                    ui.label(&lud06);
                    ui.add_space(10.0);
                    if ui.link("Copy lud06").clicked() {
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
                    if ui.link("Copy lud16").clicked() {
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
fn profile_item(
    ui: &mut Ui,
    app: &mut GossipUi,
    width: f32,
    label: impl Into<String>,
    content: impl Into<String>,
) {
    let content: String = content.into();
    let symbol = CopyButton::new().stroke(egui::Stroke::new(1.4, app.theme.accent_color()));
    let response = profile_item_frame(ui, width, label, &content, symbol).response;

    if response.clicked() {
        ui.output_mut(|o| o.copied_text = content.to_owned());
    }
}

/// A profile item with qr copy option
fn profile_item_qr(
    ui: &mut Ui,
    app: &mut GossipUi,
    width: f32,
    label: impl Into<String>,
    display_content: impl Into<String>,
    qr_content: &'static str,
) {
    let symbol = egui::Label::new(
        egui::RichText::new("⚃")
            .size(16.5)
            .color(app.theme.accent_color()),
    );
    let response = profile_item_frame(ui, width, label, display_content, symbol).response;

    if response.clicked() {
        app.qr_codes.remove("person_qr");
        app.person_qr = Some(qr_content);
    }
}

fn make_frame() -> egui::Frame {
    egui::Frame::none()
        .inner_margin(egui::Margin {
            left: 10.0,
            right: 10.0,
            top: 8.0,
            bottom: 8.0,
        })
        .outer_margin(egui::Margin {
            left: 0.0,
            right: 0.0,
            top: 0.0,
            bottom: 0.0,
        })
        .fill(egui::Color32::TRANSPARENT)
        .rounding(egui::Rounding::same(5.0))
}

fn item_label(ui: &mut Ui, label: impl Into<String>) {
    let label: String = label.into();
    ui.label(RichText::new(label.to_uppercase()).weak().small());
}

fn profile_item_frame(
    ui: &mut Ui,
    width: f32,
    label: impl Into<String>,
    content: impl Into<String>,
    symbol: impl Widget,
) -> InnerResponse<Response> {
    let content: String = content.into();
    let label: String = label.into();

    let width =
        width - list_entry::TEXT_LEFT - list_entry::TEXT_RIGHT - ui.spacing().item_spacing.x;

    let frame = make_frame();
    let mut prepared = frame.begin(ui);

    let inner = {
        let ui = &mut prepared.content_ui;
        ui.horizontal(|ui| {
            ui.set_min_width(width);
            ui.set_max_width(width);
            ui.vertical(|ui| {
                item_label(ui, &label);
                ui.add_space(ITEM_V_SPACE);
                ui.horizontal_wrapped(|ui| {
                    ui.label(content);
                });
            })
            .response
        })
        .response
    };

    let frame_rect = (prepared.frame.inner_margin + prepared.frame.outer_margin)
        .expand_rect(prepared.content_ui.min_rect());

    let response = ui
        .interact(frame_rect, ui.auto_id_with(&label), egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);

    if response.hovered() {
        let sym_rect = egui::Rect::from_min_size(
            prepared.content_ui.min_rect().right_top() + vec2(-10.0, 0.0),
            vec2(10.0, 10.0),
        );
        // prepared.content_ui.allocate_ui_at_rect(sym_rect, |ui| {
        //     ui.add_sized(sym_rect.size(), symbol)
        // });
        egui::Area::new(ui.auto_id_with(label + "_sym"))
            .interactable(false)
            .movable(false)
            .order(egui::Order::Foreground)
            .fixed_pos(sym_rect.left_top())
            .show(prepared.content_ui.ctx(), |ui| {
                ui.add_sized(sym_rect.size(), symbol)
            });
        if ui.visuals().dark_mode {
            prepared.frame.fill = ui.visuals().window_fill;
        } else {
            prepared.frame.fill = ui.visuals().extreme_bg_color;
        }
    }

    prepared.end(ui);

    InnerResponse { inner, response }
}
