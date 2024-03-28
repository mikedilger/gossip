use std::{cell::RefCell, rc::Rc};

use eframe::egui::{Color32, Layout, RichText, Ui};
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

impl Notification for AuthRequest {
    fn timestamp(&self) -> u64 {
        self.timestamp
    }

    fn title(&self) -> RichText {
        RichText::new("Relay Authentication Request".to_uppercase())
            .color(Color32::from_rgb(0xEF, 0x44, 0x44))
    }

    fn summary(&self) -> String {
        todo!()
    }

    fn show(&mut self, theme: &Theme, ui: &mut Ui) -> Option<Page> {
        ui.with_layout(Layout::left_to_right(super::ALIGN), |ui| {
            ui.set_height(super::HEIGHT);
            // FIXME pull account name with self.account once multiple keys are supported
            let text = format!("Authenticate to {}", self.relay_url);
            widgets::truncated_label(
                ui,
                self.relay_url.to_string().trim_end_matches('/'),
                ui.available_width() - super::TRUNC,
            )
            .on_hover_text(text);
            ui.with_layout(Layout::right_to_left(super::ALIGN), |ui| {
                ui.scope(|ui| {
                    super::decline_style(theme, ui.style_mut());
                    if ui.button("Decline").clicked() {
                        let _ = GLOBALS.to_overlord.send(ToOverlordMessage::AuthDeclined(
                            self.relay_url.to_owned(),
                            self.make_permanent,
                        ));
                    }
                });
                ui.add_space(10.0);
                ui.scope(|ui| {
                    super::approve_style(theme, ui.style_mut());
                    if ui.button("Approve").clicked() {
                        let _ = GLOBALS.to_overlord.send(ToOverlordMessage::AuthApproved(
                            self.relay_url.to_owned(),
                            self.make_permanent,
                        ));
                    }
                });
                ui.add_space(10.0);
                ui.label("Remember");
                widgets::switch_with_size(ui, &mut self.make_permanent, super::SWITCH_SIZE)
                    .on_hover_text("store permission permanently");
            });
        });
        None
    }
}
