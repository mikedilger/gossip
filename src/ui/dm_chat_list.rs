use super::{GossipUi, Page};
use crate::dm_channel::DmChannelData;
use crate::feed::FeedKind;
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{Context, Label, RichText, ScrollArea, Sense, Ui};

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    let mut channels: Vec<DmChannelData> = match GLOBALS.storage.dm_channels() {
        Ok(channels) => channels,
        Err(_) => {
            ui.label("ERROR");
            return;
        }
    };

    ui.heading("Direct Private Message Channels");
    ui.add_space(12.0);

    ScrollArea::vertical()
        .id_source("dm_chat_list")
        .max_width(f32::INFINITY)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let color = app.settings.theme.accent_color();
            for channeldata in channels.drain(..) {
                ui.horizontal_wrapped(|ui| {
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
                        app.show_post_area = true;
                        app.draft_needs_focus = false;

                        app.draft_data.replying_to = None;
                        app.draft_data.repost = None;
                        app.draft_data.dm_channel = Some(channeldata.dm_channel);
                    }
                });
                ui.add_space(20.0);
            }
        });
}
