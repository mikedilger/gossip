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

    app.vert_scroll_area()
        .id_source("dm_chat_list")
        .max_width(f32::INFINITY)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let color = app.theme.accent_color();
            for channeldata in channels.drain(..) {
                widgets::list_entry::make_frame(ui)
                    .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal_wrapped(|ui| {
                        let rect = egui::Rect::from_min_size(
                            ui.next_widget_position(),
                            ui.available_size()
                        );
                        widgets::list_entry::paint_frame(ui, &rect, None);

                        let channel_name = channeldata.dm_channel.name();
                        ui.label(format!(
                            "({}/{})",
                            channeldata.unread_message_count, channeldata.message_count
                        ));

                        ui.label(
                            RichText::new(crate::date_ago::date_ago(channeldata.latest_message))
                                .italics()
                                .weak(),
                        )
                        .on_hover_ui(|ui| {
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

                        if ui
                            .add(
                                Label::new(RichText::new(channel_name).color(color))
                                    .sense(Sense::click()),
                            )
                            .clicked()
                        {
                            app.set_page(Page::Feed(FeedKind::DmChat(channeldata.dm_channel.clone())));
                            app.dm_draft_data.clear();
                            app.draft_needs_focus = true;
                        }
                    });
                });
            }
        });
}
