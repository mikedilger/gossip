use std::{cell::RefCell, rc::Rc};

use eframe::egui::{self, Align, Color32, Layout, RichText, Ui};
use egui_extras::{Size, StripBuilder};
use gossip_lib::{
    comms::{RelayJob, ToOverlordMessage},
    PendingItem, GLOBALS,
};
use nostr_types::RelayUrl;

use crate::ui::{widgets, Page, Theme};

pub use super::Notification;
use super::NotificationFilter;
pub struct ConnRequest {
    relay: RelayUrl,
    jobs: Vec<RelayJob>,
    item: PendingItem,
    timestamp: u64,
    remember: bool,
}

impl ConnRequest {
    pub fn new(item: PendingItem, timestamp: u64) -> Rc<RefCell<Self>> {
        match &item {
            PendingItem::RelayConnectionRequest { relay, jobs } => Rc::new(RefCell::new(Self {
                relay: relay.clone(),
                jobs: jobs.clone(),
                item,
                timestamp,
                remember: false,
            })),
            _ => panic!("Only accepts PendingItem::RelayConnectionRequest"),
        }
    }
}

/// width needed for action section
const TRUNC: f32 = 280.0;

fn reasons_color(theme: &Theme) -> Color32 {
    if theme.dark_mode {
        theme.amber_400()
    } else {
        theme.amber_500()
    }
}

impl<'a> Notification<'a> for ConnRequest {
    fn timestamp(&self) -> u64 {
        self.timestamp
    }

    fn title(&self) -> RichText {
        RichText::new("Relay Request".to_uppercase()).color(Color32::from_rgb(0xEF, 0x44, 0x44))
    }

    fn matches_filter(&self, filter: &NotificationFilter) -> bool {
        matches!(
            filter,
            NotificationFilter::All | NotificationFilter::RelayConnectionRequest
        )
    }

    fn item(&'a self) -> &'a PendingItem {
        &self.item
    }

    fn get_remember(&self) -> bool {
        self.remember
    }

    fn set_remember(&mut self, value: bool) {
        self.remember = value;
    }

    fn show(&mut self, theme: &Theme, ui: &mut Ui) -> Option<Page> {
        let mut new_page = None;
        let jobstrs: Vec<String> = self
            .jobs
            .iter()
            .map(|j| format!("{:?}", j.reason))
            .collect();

        let panel_width = ui.available_width();
        StripBuilder::new(ui)
            .size(Size::remainder())
            .size(Size::initial(TRUNC))
            .cell_layout(Layout::left_to_right(Align::Center).with_main_wrap(true))
            .horizontal(|mut strip| {
                strip.cell(|ui| {
                    ui.label("Connect to:");
                    if widgets::relay_url(ui, theme, &self.relay)
                        .on_hover_text("Edit this Relay in your Relay settings")
                        .clicked()
                    {
                        new_page = Some(Page::RelaysKnownNetwork(Some(self.relay.clone())));
                    }

                    if panel_width < 720.0 {
                        ui.end_row();
                    } else {
                        ui.add_space(20.0);
                    }
                    if self.jobs.len() > 1 {
                        ui.label(RichText::new("Reasons:"));
                    } else {
                        ui.label(RichText::new("Reason:"));
                    };

                    for (i, job) in jobstrs.iter().enumerate() {
                        if i + 1 < jobstrs.len() {
                            ui.label(
                                RichText::new(format!("{},", job)).color(reasons_color(theme)),
                            );
                        } else {
                            ui.label(RichText::new(job).color(reasons_color(theme)));
                        }
                    }
                });

                strip.cell(|ui| {
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.scope(|ui| {
                            super::decline_style(theme, ui.style_mut());
                            if ui.button("Decline").clicked() {
                                let _ =
                                    GLOBALS.to_overlord.send(ToOverlordMessage::ConnectDeclined(
                                        self.relay.to_owned(),
                                        self.remember,
                                    ));
                            }
                        });
                        ui.add_space(10.0);
                        ui.scope(|ui| {
                            super::approve_style(theme, ui.style_mut());
                            if ui.button("Approve").clicked() {
                                let _ =
                                    GLOBALS.to_overlord.send(ToOverlordMessage::ConnectApproved(
                                        self.relay.to_owned(),
                                        self.remember,
                                    ));
                            }
                        });
                        ui.add_space(10.0);
                        ui.label("Remember");
                        widgets::Switch::large(theme, &mut self.remember)
                            .show(ui)
                            .on_hover_text("store permission permanently");
                    });
                });
            });

        new_page
    }
}
