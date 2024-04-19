use super::{widgets, GossipUi, Page};
use eframe::egui;
use egui::{Context, Label, RichText, Ui};
use gossip_lib::FeedKind;
use gossip_lib::GLOBALS;
use gossip_lib::{Error, ErrorKind};
use std::time::{Duration, Instant};

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    // Possibly refresh DM channels (every 5 seconds)
    if app.dm_channel_next_refresh < Instant::now() {
        app.dm_channel_cache = match GLOBALS.storage.dm_channels() {
            Ok(channels) => {
                app.dm_channel_error = None;
                channels
            }
            Err(Error {
                kind: ErrorKind::NoPrivateKey,
                ..
            }) => {
                app.dm_channel_error = Some("Private Key Not Available".to_owned());
                vec![]
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

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.heading("Direct Private Message Channels");
    });
    ui.add_space(10.0);

    let is_signer_ready = GLOBALS.identity.is_unlocked();

    app.vert_scroll_area()
        .id_source("dm_chat_list")
        .show(ui, |ui| {
            let color = app.theme.accent_color();
            for channeldata in channels.drain(..) {
                let row_response =
                    widgets::list_entry::clickable_frame(
                        ui,
                        app,
                        Some(app.theme.main_content_bgcolor()),
                        |ui, app| {
                            ui.set_min_width(ui.available_width());
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
                        },
                    );
                if row_response
                    .response
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    app.set_page(
                        ctx,
                        Page::Feed(FeedKind::DmChat(channeldata.dm_channel.clone())),
                    );
                    app.dm_draft_data.clear();
                    app.draft_needs_focus = true;
                }
            }
        });
}
