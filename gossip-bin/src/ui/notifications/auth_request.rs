use std::{cell::RefCell, rc::Rc};

use eframe::egui::{self, Color32, Layout, RichText, Ui};
use gossip_lib::{comms::ToOverlordMessage, GLOBALS};
use nostr_types::{PublicKey, RelayUrl};

use crate::ui::{widgets, Page, Theme};

pub use super::Notification;

pub struct AuthRequest {
    #[allow(unused)]
    account: PublicKey,
    relay_url: RelayUrl,
    timestamp: u64,
    make_permanent: bool,
}

impl AuthRequest {
    pub fn new(account: PublicKey, relay_url: RelayUrl, timestamp: u64) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            account,
            relay_url,
            timestamp,
            make_permanent: false,
        }))
    }
}

const HEIGHT: f32 = 23.0;
const TRUNC: f32 = 340.0;

impl Notification for AuthRequest {
    fn timestamp(&self) -> u64 {
        self.timestamp
    }

    fn title(&self) -> RichText {
        RichText::new("Relay Request".to_uppercase()).color(Color32::from_rgb(0xEF, 0x44, 0x44))
    }

    fn summary(&self) -> String {
        todo!()
    }

    fn show(&mut self, theme: &Theme, ui: &mut Ui) -> Option<Page> {
        let mut new_page = None;

        let description =
            |myself: &mut AuthRequest, _theme: &Theme, ui: &mut Ui, new_page: &mut Option<Page>| {
                ui.set_height(HEIGHT);
                // FIXME pull account name with self.account once multiple keys are supported
                ui.label("Authenticate to");
                if ui
                    .link(
                        myself
                            .relay_url
                            .as_url_crate_url()
                            .domain()
                            .unwrap_or_default(),
                    )
                    .on_hover_text("Edit this Relay in your Relay settings")
                    .clicked()
                {
                    *new_page = Some(Page::RelaysKnownNetwork(Some(myself.relay_url.clone())));
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
                                myself.relay_url.to_owned(),
                                myself.make_permanent,
                            ));
                        }
                    });
                    ui.add_space(10.0);
                    ui.scope(|ui| {
                        super::approve_style(theme, ui.style_mut());
                        if ui.button("Approve").clicked() {
                            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::AuthApproved(
                                myself.relay_url.to_owned(),
                                myself.make_permanent,
                            ));
                        }
                    });
                    ui.add_space(10.0);
                    ui.label("Remember");
                    widgets::switch_with_size(ui, &mut myself.make_permanent, super::SWITCH_SIZE)
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
