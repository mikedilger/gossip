use std::{cell::RefCell, rc::Rc};

use eframe::egui::{self, Align, Color32, FontSelection, Layout, RichText, Ui};
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

        StripBuilder::new(ui)
            .size(Size::remainder())
            .size(Size::initial(TRUNC))
            .cell_layout(Layout::left_to_right(Align::Center))
            .horizontal(|mut strip| {
                strip.strip(|builder| {
                    builder
                        .size(Size::initial(super::HEADER_HEIGHT))
                        .size(Size::initial(14.0))
                        .cell_layout(Layout::left_to_right(Align::TOP).with_main_wrap(true))
                        .vertical(|mut strip| {
                            strip.cell(|ui| {
                                ui.label(
                                    egui::RichText::new(super::unixtime_to_string(
                                        self.timestamp().try_into().unwrap_or_default(),
                                    ))
                                    .weak()
                                    .small(),
                                );
                                ui.add_space(10.0);
                                ui.label(self.title().small());
                            });
                            strip.cell(|ui| {
                                ui.label("Connect to");
                                if ui
                                    .link(
                                        self.relay.as_url_crate_url().domain().unwrap_or_default(),
                                    )
                                    .on_hover_text("Edit this Relay in your Relay settings")
                                    .clicked()
                                {
                                    new_page =
                                        Some(Page::RelaysKnownNetwork(Some(self.relay.clone())));
                                }

                                let mut job = egui::text::LayoutJob::default();
                                let label = if self.jobs.len() > 1 {
                                    RichText::new("Reasons: ")
                                } else {
                                    RichText::new("Reason: ")
                                };
                                label.append_to(
                                    &mut job,
                                    ui.style(),
                                    FontSelection::Default,
                                    Align::Min,
                                );
                                RichText::new(jobstrs.join(", "))
                                    .color(theme.accent_complementary_color())
                                    .append_to(
                                        &mut job,
                                        ui.style(),
                                        FontSelection::Default,
                                        Align::Min,
                                    );
                                let galley = ui.fonts(|f| f.layout_job(job));

                                if galley.rect.width() > (ui.available_width()) {
                                    ui.end_row();
                                }

                                ui.label(galley);
                            });
                        });
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
                        widgets::switch_with_size(ui, &mut self.remember, super::SWITCH_SIZE)
                            .on_hover_text("store permission permanently");
                    });
                });
            });

        new_page
    }
}
