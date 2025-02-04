use std::{cell::RefCell, rc::Rc};

use eframe::egui::{self, Align, Color32, Layout, RichText, Ui};
use egui_extras::{Size, StripBuilder};
use gossip_lib::{
    comms::ToOverlordMessage,
    nostr_connect_server::{Approval, ParsedCommand},
    PendingItem, GLOBALS,
};
use nostr_types::PublicKey;
use serde::Serialize;

use crate::ui::{widgets, Page, Theme};

pub use super::Notification;
use super::NotificationFilter;

pub struct Nip46Request {
    client_name: String,
    account: PublicKey,
    command: ParsedCommand,
    item: PendingItem,
    timestamp: u64,
    remember: bool,
}

impl Nip46Request {
    pub fn new(item: PendingItem, timestamp: u64) -> Rc<RefCell<Self>> {
        match &item {
            PendingItem::Nip46Request {
                client_name,
                account,
                command,
            } => Rc::new(RefCell::new(Self {
                client_name: client_name.clone(),
                account: *account,
                command: command.clone(),
                item,
                timestamp,
                remember: false,
            })),
            _ => panic!("Only accepts PendingItem::Nip46Request"),
        }
    }
}

const ALIGN: egui::Align = egui::Align::Center;
const HEIGHT: f32 = 23.0;

impl<'a> Notification<'a> for Nip46Request {
    fn timestamp(&self) -> u64 {
        self.timestamp
    }

    fn title(&self) -> RichText {
        RichText::new("NIP46 Signing request".to_uppercase())
            .color(Color32::from_rgb(0xEF, 0x44, 0x44))
    }

    fn matches_filter(&self, filter: &NotificationFilter) -> bool {
        matches!(
            filter,
            NotificationFilter::All | NotificationFilter::Nip46Request
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
        const TRUNC: f32 = 285.0 + 40.0;
        StripBuilder::new(ui)
            .size(Size::remainder().at_least(37.0)) // space for summary and actions
            .size(Size::remainder().at_least(14.0)) // space for "Detail" section
            .vertical(|mut strip| {
                strip.strip(|strip| {
                    strip
                        .size(Size::remainder())
                        .size(Size::initial(TRUNC))
                        .cell_layout(Layout::left_to_right(Align::Center))
                        .horizontal(|mut strip| {
                            strip.cell(|ui| {
                                let text = format!(
                                    "NIP-46 Request from '{}'. Allow {}?",
                                    self.client_name, self.command.method
                                );
                                widgets::truncated_label(
                                    ui,
                                    text,
                                    ui.available_width(),
                                )
                                .on_hover_text(self.command.params.join(", "));
                            });

                            strip.cell(|ui| {
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
                                        if ui.button("Approve").clicked() {
                                            let _ = GLOBALS.to_overlord.send(
                                                ToOverlordMessage::Nip46ServerOpApprovalResponse(
                                                    self.account,
                                                    self.command.clone(),
                                                    if self.remember { Approval::Always } else { Approval::Once },
                                                ),
                                            );
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
                });

                strip.cell(|ui| {
                    egui::CollapsingHeader::new("Details")
                        .id_salt(&self.command.id)
                        .show_unindented(ui, |ui| {
                            for param in &self.command.params {
                                if let Ok(obj) = serde_json::from_str::<serde_json::Value>(param) {
                                    let mut writer = Vec::new();
                                    let formatter =
                                        serde_json::ser::PrettyFormatter::with_indent(b"  ");
                                    let mut ser = serde_json::Serializer::with_formatter(
                                        &mut writer,
                                        formatter,
                                    );

                                    if obj.serialize(&mut ser).is_ok() {
                                        if let Ok(str) = String::from_utf8(writer) {
                                            egui_extras::syntax_highlighting::code_view_ui(
                                                ui,
                                                &egui_extras::syntax_highlighting::CodeTheme::from_style(
                                                    ui.style(),
                                                ),
                                                &str,
                                                "json",
                                            );
                                        }
                                    }
                                } else {
                                    ui.label(format!("Not valid JSON: {}", param));
                                }
                            }
                        });
                });
            });

        None
    }
}
