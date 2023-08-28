use super::{GossipUi, Page};
use crate::feed::FeedKind;
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{Context, Label, RichText, ScrollArea, Sense, Ui};

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    let mut channels = match GLOBALS.storage.dm_channels() {
        Ok(channels) => channels,
        Err(_) => {
            ui.label("ERROR");
            return;
        }
    };

    channels.sort_by_key(|a| a.name());

    ScrollArea::vertical()
        .id_source("dm_chat_list")
        .show(ui, |ui| {
            let color = app.settings.theme.accent_color();
            for channel in channels.drain(..) {
                ui.horizontal_wrapped(|ui| {
                    let channel_name = channel.name();
                    if ui
                        .add(
                            Label::new(RichText::new(channel_name).color(color))
                                .sense(Sense::click()),
                        )
                        .clicked()
                    {
                        app.set_page(Page::Feed(FeedKind::DmChat(channel)));
                    }
                });
                ui.add_space(20.0);
            }
        });
}
