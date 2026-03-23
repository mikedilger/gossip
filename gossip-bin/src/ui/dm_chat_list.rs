use crate::AVATAR_SIZE_F32;

use super::{widgets, GossipUi, Page};
use eframe::egui;
use eframe::egui::vec2;
use eframe::egui::Rect;
use egui::{Context, Label, RichText, Ui};
use gossip_lib::FeedKind;
use gossip_lib::Person;
use gossip_lib::GLOBALS;
use gossip_lib::{PersonTable, Table};
use nostr_types::PublicKey;
use std::time::{Duration, Instant};

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    // Possibly refresh DM channels (every 5 seconds)
    if app.dm_channel_next_refresh < Instant::now() {
        let result = GLOBALS
            .runtime
            .block_on(async { GLOBALS.db().dm_channels().await });
        app.dm_channel_cache = match result {
            Ok(channels) => {
                app.dm_channel_error = None;
                channels
            }
            Err(e) => {
                app.dm_channel_error = Some(format!("{}", e));
                vec![]
            }
        };

        app.dm_channel_next_refresh = Instant::now() + Duration::new(5, 0);
    }

    if let Some(err) = &app.dm_channel_error {
        ui.label(err);
        return;
    }

    let mut channels = app.dm_channel_cache.clone();

    let is_signer_ready = GLOBALS.identity.is_unlocked();

    widgets::page_header(ui, "Direct Messages", |ui| {
        ui.add_space(16.0);
        if is_signer_ready {
            if widgets::Button::bordered(&app.theme, "New message")
                .small(true)
                .show(ui)
                .clicked()
            {
                app.dm_new_message = true;
                app.dm_new_message_error = None;
            }
        }
        ui.add_space(8.0);
        if widgets::Button::bordered(&app.theme, "Mark all read")
            .small(true)
            .show(ui)
            .clicked()
        {
            let _ = GLOBALS.db().mark_all_dms_read();
        }
    });

    if app.dm_new_message {
        render_new_message_popup(app, ctx);
    }

    app.vert_scroll_area()
        .id_salt("dm_chat_list")
        .show(ui, |ui| {
            let color = app.theme.accent_color();
            for channeldata in channels.drain(..) {
                let row_response =
                    widgets::list_entry::clickable_frame(
                        ui,
                        app,
                        Some(app.theme.main_content_bgcolor()),
                        Some(app.theme.hovered_content_bgcolor()),
                        |ui, app| {
                            ui.set_min_width(ui.available_width());
                            ui.set_min_height(AVATAR_SIZE_F32);
                            ui.set_max_height(AVATAR_SIZE_F32);
                            ui.horizontal(|ui| {

                                // avatar(s)
                                if let Some(local) = GLOBALS.identity.public_key() {
                                    for key in channeldata.dm_channel.keys() {
                                        if key != &local {
                                            let person = if let Ok(Some(person)) = PersonTable::read_record(*key, None) {
                                                person
                                            } else {
                                                let mut person = Person::new(*key);
                                                let _ = PersonTable::write_record(&mut person, None);
                                                person
                                            };

                                            let avatar = if let Some(avatar) = app.try_get_avatar(ctx, &person.pubkey) {
                                                avatar
                                            } else {
                                                app.placeholder_avatar.clone()
                                            };

                                            widgets::paint_avatar(ui, &person, &avatar, widgets::AvatarSize::Feed);
                                        }
                                    }
                                }

                                ui.add_space(10.0);

                                ui.vertical(|ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        let channel_name = channeldata.dm_channel.name();
                                        ui.add(Label::new(
                                            RichText::new(channel_name).heading().color(color),
                                        ));

                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::TOP),
                                            |ui| {
                                                ui.label(crate::date_ago::date_ago(
                                                    channeldata.latest_message_created_at,
                                                ))
                                                .on_hover_ui(|ui| {
                                                    if let Ok(stamp) =
                                                        time::OffsetDateTime::from_unix_timestamp(
                                                            channeldata.latest_message_created_at.0,
                                                        )
                                                    {
                                                        if let Ok(formatted) = stamp
                                                .format(&time::format_description::well_known::Rfc2822)
                                            {
                                                ui.label(formatted);
                                            }
                                                    }
                                                });
                                                ui.label(" - ");
                                                ui.label(
                                                    RichText::new(format!(
                                                        "{} unread",
                                                        channeldata.unread_message_count
                                                    ))
                                                    .color(app.theme.accent_color()),
                                                );
                                            },
                                        );
                                    });

                                    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                                        ui.horizontal(|ui| {
                                            if is_signer_ready {
                                                if let Some(message) = &channeldata.latest_message_content {
                                                    widgets::truncated_label(
                                                        ui,
                                                        message,
                                                        ui.available_width() - 100.0,
                                                    );
                                                }
                                            }

                                            ui.with_layout(
                                                egui::Layout::right_to_left(egui::Align::TOP),
                                                |ui| {
                                                    ui.label(
                                                        RichText::new(format!(
                                                            "{} messages",
                                                            channeldata.message_count
                                                        ))
                                                        .weak(),
                                                    );
                                                },
                                            );
                                        });
                                    });
                                });
                            });
                        },
                    );
                let rect = Rect::from_min_size(
                    row_response.response.rect.min,
                    vec2(
                        row_response.response.rect.width() - 100.0,
                        row_response.response.rect.height(),
                    ),
                );
                if ui
                    .interact(rect, ui.next_auto_id(), egui::Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    open_dm_channel(app, ctx, channeldata.dm_channel.clone());
                }
            }
        });
}

fn open_dm_channel(app: &mut GossipUi, ctx: &Context, channel: gossip_lib::DmChannel) {
    app.set_page(ctx, Page::Feed(FeedKind::DmChat(channel.clone())));
    app.draft_needs_focus = true;

    // Maybe clear the draft, if we are going into a different channel than last time.
    if let Some(oldtarget) = &app.dm_draft_data_target {
        if *oldtarget != channel {
            app.save_dm_draft_state();
            app.dm_draft_data.clear();
            app.load_dm_draft_state(&channel);
        }
    } else {
        app.dm_draft_data.clear();
        app.load_dm_draft_state(&channel);
    }
    app.dm_draft_data_target = Some(channel);
}

fn render_new_message_popup(app: &mut GossipUi, ctx: &Context) {
    const DLG_SIZE: eframe::egui::Vec2 = vec2(420.0, 320.0);

    let ret = widgets::modal_popup(ctx, DLG_SIZE, DLG_SIZE, true, |ui| {
        ui.vertical(|ui| {
            ui.heading("New direct message");
            ui.add_space(8.0);

            if let Some(err) = &app.dm_new_message_error {
                ui.label(RichText::new(err).color(app.theme.warning_marker_text_color()));
                ui.add_space(8.0);
            }

            ui.label("Search for a known contact");
            let mut output = widgets::TextEdit::search(
                &app.theme,
                &app.assets,
                &mut app.dm_new_message_search,
            )
            .desired_width(f32::INFINITY)
            .show(ui);

            let mut selected = app.dm_new_message_search_selected;
            let mut enter_key = false;
            if app.dm_new_message_search_results.is_empty() {
                selected = None;
            } else {
                (selected, enter_key) = widgets::capture_keyboard_for_search(
                    ui,
                    app.dm_new_message_search_results.len(),
                    selected,
                );
            }

            if app.dm_new_message_search.len() > 2 {
                if Some(&app.dm_new_message_search) != app.dm_new_message_searched.as_ref()
                    && output.cursor_range.is_some()
                {
                    let mut pairs = GLOBALS
                        .people
                        .search_people_to_tag(app.dm_new_message_search.as_str())
                        .unwrap_or_default();
                    pairs.sort_by(|(_, ak), (_, bk)| {
                        let af = GLOBALS
                            .db()
                            .is_person_in_list(ak, gossip_lib::PersonList::Followed)
                            .unwrap_or(false);
                        let bf = GLOBALS
                            .db()
                            .is_person_in_list(bk, gossip_lib::PersonList::Followed)
                            .unwrap_or(false);
                        bf.cmp(&af).then(std::cmp::Ordering::Greater)
                    });
                    app.dm_new_message_searched = Some(app.dm_new_message_search.clone());
                    app.dm_new_message_search_results = pairs.to_owned();
                }
            } else {
                app.dm_new_message_searched = None;
                app.dm_new_message_search_results.clear();
            }

            widgets::show_contact_search(
                ui,
                app,
                egui::AboveOrBelow::Below,
                &mut output,
                &mut selected,
                app.dm_new_message_search_results.clone(),
                enter_key,
                |_, app, _, pair| {
                    app.dm_new_message_search = pair.0.clone();
                    app.dm_new_message_search_results.clear();
                    app.dm_new_message_search_selected = None;
                    app.dm_new_message_address = pair.1.as_bech32_string();
                },
            );
            app.dm_new_message_search_selected = selected;

            ui.add_space(10.0);
            ui.label("Or enter an npub, hex key, or nprofile address");
            ui.add(
                text_edit_line!(app, app.dm_new_message_address)
                    .desired_width(f32::INFINITY)
                    .hint_text("npub1, hex key, or nprofile1"),
            );

            ui.add_space(12.0);
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.horizontal(|ui| {
                    if widgets::Button::secondary(&app.theme, "Cancel")
                        .show(ui)
                        .clicked()
                    {
                        app.clear_new_message_dialog();
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        if widgets::Button::primary(&app.theme, "Start chat")
                            .show(ui)
                            .clicked()
                        {
                            if let Some(pubkey) = parse_new_message_target(
                                app.dm_new_message_address.trim(),
                            ) {
                                open_dm_channel(
                                    app,
                                    ctx,
                                    gossip_lib::DmChannel::new(&[pubkey]),
                                );
                                app.clear_new_message_dialog();
                            } else {
                                app.dm_new_message_error =
                                    Some("Enter a valid recipient to start a chat.".to_owned());
                            }
                        }
                    });
                });
            });
        });
    });

    if ret.inner.clicked() {
        app.clear_new_message_dialog();
    }
}

fn parse_new_message_target(value: &str) -> Option<PublicKey> {
    if let Ok(pubkey) = PublicKey::try_from_bech32_string(value, true) {
        Some(pubkey)
    } else if let Ok(pubkey) = PublicKey::try_from_hex_string(value, true) {
        Some(pubkey)
    } else if let Ok(profile) = nostr_types::Profile::try_from_bech32_string(value, true) {
        Some(profile.pubkey)
    } else {
        None
    }
}
