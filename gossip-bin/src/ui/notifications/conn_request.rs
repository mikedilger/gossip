use std::{cell::RefCell, rc::Rc};

use eframe::egui::{Color32, Layout, RichText, Ui};
use gossip_lib::{
    comms::{RelayJob, ToOverlordMessage},
    GLOBALS,
};
use nostr_types::RelayUrl;

use crate::ui::{widgets, Page, Theme};

pub use super::Notification;
pub struct ConnRequest {
    relay_url: RelayUrl,
    jobs: Vec<RelayJob>,
    timestamp: u64,
    make_permanent: bool,
}

impl ConnRequest {
    pub fn new(relay_url: RelayUrl, jobs: Vec<RelayJob>, timestamp: u64) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            relay_url,
            jobs,
            timestamp,
            make_permanent: false,
        }))
    }
}

impl Notification for ConnRequest {
    fn timestamp(&self) -> u64 {
        self.timestamp
    }

    fn title(&self) -> RichText {
        RichText::new("Relay Connection Request".to_uppercase())
            .color(Color32::from_rgb(0xEF, 0x44, 0x44))
    }

    fn summary(&self) -> String {
        todo!()
    }

    fn show(&mut self, theme: &Theme, ui: &mut Ui) -> Option<Page> {
        let jobstrs: Vec<String> = self
            .jobs
            .iter()
            .map(|j| format!("{:?}", j.reason))
            .collect();

        ui.with_layout(Layout::left_to_right(super::ALIGN), |ui| {
            ui.set_height(super::HEIGHT);
            let text = format!("Connect to {} for {}", self.relay_url, jobstrs.join(", "));
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
                        let _ = GLOBALS.to_overlord.send(ToOverlordMessage::ConnectDeclined(
                            self.relay_url.to_owned(),
                            self.make_permanent,
                        ));
                    }
                });
                ui.add_space(10.0);
                ui.scope(|ui| {
                    super::approve_style(theme, ui.style_mut());
                    if ui.button("Approve").clicked() {
                        let _ = GLOBALS.to_overlord.send(ToOverlordMessage::ConnectApproved(
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
