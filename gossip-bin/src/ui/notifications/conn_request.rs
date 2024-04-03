use std::{cell::RefCell, rc::Rc};

use eframe::egui::{self, Color32, Layout, RichText, Ui};
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
const TRUNC: f32 = 340.0;
/// min-height of each section
const HEIGHT: f32 = 23.0;

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

        let description = |myself: &mut ConnRequest,
                           theme: &Theme,
                           ui: &mut Ui,
                           new_page: &mut Option<Page>,
                           trunc_width: f32| {
            ui.set_height(HEIGHT);
            ui.label("Connect to");
            if ui
                .link(myself.relay.as_url_crate_url().domain().unwrap_or_default())
                .on_hover_text("Edit this Relay in your Relay settings")
                .clicked()
            {
                *new_page = Some(Page::RelaysKnownNetwork(Some(myself.relay.clone())));
            }

            if myself.jobs.len() > 1 {
                ui.label("Reasons:");
            } else {
                ui.label("Reason:");
            }

            widgets::truncated_label(
                ui,
                RichText::new(jobstrs.join(", ")).color(theme.accent_complementary_color()),
                trunc_width,
            );
        };

        let action =
            |myself: &mut ConnRequest, theme: &Theme, ui: &mut Ui, _new_page: &mut Option<Page>| {
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.set_height(HEIGHT);
                    ui.scope(|ui| {
                        super::decline_style(theme, ui.style_mut());
                        if ui.button("Decline").clicked() {
                            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::ConnectDeclined(
                                myself.relay.to_owned(),
                                myself.remember,
                            ));
                        }
                    });
                    ui.add_space(10.0);
                    ui.scope(|ui| {
                        super::approve_style(theme, ui.style_mut());
                        if ui.button("Approve").clicked() {
                            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::ConnectApproved(
                                myself.relay.to_owned(),
                                myself.remember,
                            ));
                        }
                    });
                    ui.add_space(10.0);
                    ui.label("Remember");
                    widgets::switch_with_size(ui, &mut myself.remember, super::SWITCH_SIZE)
                        .on_hover_text("store permission permanently");
                });
            };

        // "responsive" layout
        let width = ui.available_width();
        if width > (TRUNC * 2.2) {
            ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                description(self, theme, ui, &mut new_page, width - TRUNC);
                action(self, theme, ui, &mut new_page);
            });
        } else {
            ui.with_layout(Layout::top_down(egui::Align::LEFT), |ui| {
                ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                    description(self, theme, ui, &mut new_page, width);
                });
                action(self, theme, ui, &mut new_page);
            });
        };

        new_page
    }
}
