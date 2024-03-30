use std::{cell::RefCell, rc::Rc};

use eframe::egui::{self, Color32, Layout, RichText, Ui};
use gossip_lib::{comms::ToOverlordMessage, PendingItem, GLOBALS};
use nostr_types::{PublicKey, RelayUrl};

use crate::ui::{widgets, Page, Theme};

pub use super::Notification;
use super::NotificationFilter;

pub struct AuthRequest {
    #[allow(unused)]
    account: PublicKey,
    relay: RelayUrl,
    item: PendingItem,
    timestamp: u64,
    remember: bool,
}

impl AuthRequest {
    pub fn new(item: PendingItem, timestamp: u64) -> Rc<RefCell<Self>> {
        match &item {
            PendingItem::RelayAuthenticationRequest { account, relay } => {
                Rc::new(RefCell::new(Self {
                    account: account.clone(),
                    relay: relay.clone(),
                    timestamp,
                    item,
                    remember: false,
                }))
            }
            _ => panic!("Only accepts PendingItem::RelayAuthenticationRequest"),
        }
    }
}

const HEIGHT: f32 = 23.0;
const TRUNC: f32 = 340.0;

impl<'a> Notification<'a> for AuthRequest {
    fn timestamp(&self) -> u64 {
        self.timestamp
    }

    fn title(&self) -> RichText {
        RichText::new("Relay Request".to_uppercase()).color(Color32::from_rgb(0xEF, 0x44, 0x44))
    }

    fn matches_filter(&self, filter: &NotificationFilter) -> bool {
        match filter {
            NotificationFilter::All => true,
            NotificationFilter::RelayAuthenticationRequest => true,
            _ => false,
        }
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

        let description =
            |myself: &mut AuthRequest, _theme: &Theme, ui: &mut Ui, new_page: &mut Option<Page>| {
                ui.set_height(HEIGHT);
                // FIXME pull account name with self.account once multiple keys are supported
                ui.label("Authenticate to");
                if ui
                    .link(myself.relay.as_url_crate_url().domain().unwrap_or_default())
                    .on_hover_text("Edit this Relay in your Relay settings")
                    .clicked()
                {
                    *new_page = Some(Page::RelaysKnownNetwork(Some(myself.relay.clone())));
                }
            };

        let action =
            |myself: &mut AuthRequest, theme: &Theme, ui: &mut Ui, _new_page: &mut Option<Page>| {
                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.set_height(HEIGHT);
                    ui.scope(|ui| {
                        super::decline_style(theme, ui.style_mut());
                        if ui.button("Decline").clicked() {
                            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::AuthDeclined(
                                myself.relay.to_owned(),
                                myself.remember,
                            ));
                        }
                    });
                    ui.add_space(10.0);
                    ui.scope(|ui| {
                        super::approve_style(theme, ui.style_mut());
                        if ui.button("Approve").clicked() {
                            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::AuthApproved(
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
        if width > (TRUNC * 2.0) {
            ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                description(self, theme, ui, &mut new_page);
                action(self, theme, ui, &mut new_page);
            });
        } else {
            ui.with_layout(Layout::top_down(egui::Align::LEFT), |ui| {
                ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                    description(self, theme, ui, &mut new_page);
                });
                action(self, theme, ui, &mut new_page);
            });
        };

        new_page
    }
}
