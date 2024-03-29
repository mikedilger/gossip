use std::{cell::RefCell, rc::Rc};

use eframe::egui::{self, Color32, Layout, RichText, Ui};
use gossip_lib::{
    comms::ToOverlordMessage,
    nip46::{Approval, ParsedCommand},
    GLOBALS,
};
use nostr_types::PublicKey;
use serde::Serialize;

use crate::ui::{widgets, Page, Theme};

pub use super::Notification;

pub struct Nip46Request {
    name: String,
    account: PublicKey,
    command: ParsedCommand,
    timestamp: u64,
}

impl Nip46Request {
    pub fn new(
        name: String,
        account: PublicKey,
        command: ParsedCommand,
        timestamp: u64,
    ) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            name,
            account,
            command,
            timestamp,
        }))
    }
}

const ALIGN: egui::Align = egui::Align::Center;
const HEIGHT: f32 = 23.0;

impl Notification for Nip46Request {
    fn timestamp(&self) -> u64 {
        self.timestamp
    }

    fn title(&self) -> RichText {
        RichText::new("NIP46 Signing request".to_uppercase())
            .color(Color32::from_rgb(0xEF, 0x44, 0x44))
    }

    fn summary(&self) -> String {
        todo!()
    }

    fn show(&mut self, theme: &Theme, ui: &mut Ui) -> Option<Page> {
        ui.with_layout(Layout::left_to_right(ALIGN).with_main_wrap(true), |ui| {
            ui.set_height(HEIGHT);
            let text = format!(
                "NIP-46 Request from '{}'. Allow {}?",
                self.name, self.command.method
            );
            widgets::truncated_label(ui, text, ui.available_width() - 300.0)
                .on_hover_text(self.command.params.join(", "));
            ui.with_layout(Layout::right_to_left(ALIGN), |ui| {
                ui.set_height(HEIGHT);
                ui.scope(|ui| {
                    super::decline_style(theme, ui.style_mut());
                    if ui.button("Decline").clicked() {
                        let _ = GLOBALS.to_overlord.send(
                            ToOverlordMessage::Nip46ServerOpApprovalResponse(
                                self.account,
                                self.command.clone(),
                                Approval::None,
                            ),
                        );
                    }
                });
                ui.add_space(10.0);
                ui.scope(|ui| {
                    super::approve_style(theme, ui.style_mut());
                    if ui.button("Approve Once").clicked() {
                        let _ = GLOBALS.to_overlord.send(
                            ToOverlordMessage::Nip46ServerOpApprovalResponse(
                                self.account,
                                self.command.clone(),
                                Approval::Once,
                            ),
                        );
                    }
                });
                ui.add_space(10.0);
                ui.scope(|ui| {
                    super::approve_style(theme, ui.style_mut());
                    if ui.button("Approve Always").clicked() {
                        let _ = GLOBALS.to_overlord.send(
                            ToOverlordMessage::Nip46ServerOpApprovalResponse(
                                self.account,
                                self.command.clone(),
                                Approval::Always,
                            ),
                        );
                    }
                });
            });
        });
        for param in &self.command.params {
            if let Ok(obj) = serde_json::from_str::<serde_json::Value>(param) {
                let mut writer = Vec::new();
                let formatter = serde_json::ser::PrettyFormatter::with_indent(b"  ");
                let mut ser = serde_json::Serializer::with_formatter(&mut writer, formatter);

                if obj.serialize(&mut ser).is_ok() {
                    if let Ok(str) = String::from_utf8(writer) {
                        egui_extras::syntax_highlighting::code_view_ui(
                            ui,
                            &egui_extras::syntax_highlighting::CodeTheme::from_style(ui.style()),
                            &str,
                            "json",
                        );
                    }
                }
            } else {
                ui.label(format!("Not valid JSON: {}", param));
            }
        }
        None
    }
}
