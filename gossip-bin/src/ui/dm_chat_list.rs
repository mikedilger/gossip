use super::{GossipUi, Page, widgets};
use eframe::egui;
use egui::{Context, Label, RichText, Sense, Ui};
use gossip_lib::DmChannelData;
use gossip_lib::FeedKind;
use gossip_lib::GLOBALS;

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    let mut channels: Vec<DmChannelData> = match GLOBALS.storage.dm_channels() {
        Ok(channels) => channels,
        Err(_) => {
            ui.label("ERROR");
            return;
        }
    };

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.heading("Direct Private Message Channels");
    });
    ui.add_space(10.0);

    let is_signer_ready = GLOBALS.signer.is_ready();

    app.vert_scroll_area()
        .id_source("dm_chat_list")
        .max_width(f32::INFINITY)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let color = app.theme.accent_color();
            for channeldata in channels.drain(..) {
                let row_response = widgets::list_entry::make_frame(ui)
                    .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.vertical(|ui| {
                        ui.horizontal_wrapped(|ui| {
                            let channel_name = channeldata.dm_channel.name();
                            ui.add(
                                Label::new(RichText::new(channel_name)
                                        .heading()
                                        .color(color)),
                                );

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                                ui.label(
                                    crate::date_ago::date_ago(channeldata.latest_message),
                                    ).on_hover_ui(|ui| {
                                    if let Ok(stamp) =
                                        time::OffsetDateTime::from_unix_timestamp(channeldata.latest_message.0)
                                    {
                                        if let Ok(formatted) =
                                            stamp.format(&time::format_description::well_known::Rfc2822)
                                        {
                                            ui.label(formatted);
                                        }
                                    }
                                });
                                ui.label(" - ");
                                ui.label(
                                    RichText::new(
                                        format!("{} unread", channeldata.unread_message_count))
                                    .color(app.theme.accent_color())
                                );
                            });
                        });

                        ui.horizontal_wrapped(|ui| {
                            if is_signer_ready {
                            // TODO
                            // if let Some(message) = channeldata.last_message {
                            //     ui.label(safe_truncate(message, 200));
                            // }
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                                ui.label(
                                    RichText::new(format!("{} messages", channeldata.message_count))
                                        .weak()
                                );
                            });
                        });
                    });
                });
                if row_response.response.interact(Sense::click()).clicked() {
                    app.set_page(Page::Feed(FeedKind::DmChat(channeldata.dm_channel.clone())));
                    app.dm_draft_data.clear();
                    app.draft_needs_focus = true;
                }
            }
        });
}
