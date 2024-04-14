use std::{cell::RefCell, rc::Rc};

use eframe::egui::{self, Align, Color32, Layout, RichText, Ui};
use egui_extras::{Size, StripBuilder};
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
                    account: *account,
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

const TRUNC: f32 = 320.0;

impl<'a> Notification<'a> for AuthRequest {
    fn timestamp(&self) -> u64 {
        self.timestamp
    }

    fn title(&self) -> RichText {
        RichText::new("Relay Request".to_uppercase()).color(Color32::from_rgb(0xEF, 0x44, 0x44))
    }

    fn matches_filter(&self, filter: &NotificationFilter) -> bool {
        matches!(
            filter,
            NotificationFilter::All | NotificationFilter::RelayAuthenticationRequest
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

        StripBuilder::new(ui)
            .size(Size::remainder())
            .size(Size::initial(TRUNC))
            .cell_layout(Layout::left_to_right(Align::Center).with_main_wrap(true))
            .horizontal(|mut strip| {
                strip.cell(|ui| {
                    // FIXME pull account name with self.account once multiple keys are supported
                    ui.label("Authenticate to:");
                    if widgets::relay_url(ui, theme, &self.relay)
                        .on_hover_text("Edit this Relay in your Relay settings")
                        .clicked()
                    {
                        new_page = Some(Page::RelaysKnownNetwork(Some(self.relay.clone())));
                    }
                });

                strip.cell(|ui| {
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.scope(|ui| {
                            super::decline_style(theme, ui.style_mut());
                            if ui.button("Decline").clicked() {
                                let _ = GLOBALS.to_overlord.send(ToOverlordMessage::AuthDeclined(
                                    self.relay.to_owned(),
                                    self.remember,
                                ));
                            }
                        });
                        ui.add_space(10.0);
                        ui.scope(|ui| {
                            super::approve_style(theme, ui.style_mut());
                            if ui.button("Approve").clicked() {
                                let _ = GLOBALS.to_overlord.send(ToOverlordMessage::AuthApproved(
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
