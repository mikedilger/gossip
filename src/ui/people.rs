use super::GossipUi;
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{Context, RichText, ScrollArea, TextStyle, Ui, Vec2};

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.add_space(8.0);
    ui.heading("People Followed");
    ui.add_space(18.0);

    let people = GLOBALS.people.blocking_lock().clone();

    ScrollArea::vertical().show(ui, |ui| {
        for (_, person) in people.iter() {
            if person.followed != 1 {
                continue;
            }

            ui.horizontal(|ui| {
                // Avatar first
                ui.image(&app.placeholder_avatar, Vec2 { x: 36.0, y: 36.0 });

                ui.vertical(|ui| {
                    ui.label(RichText::new(GossipUi::hex_pubkey_short(&person.pubkey)).weak());

                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(person.name.as_deref().unwrap_or(""))
                                .text_style(TextStyle::Name("Bold".into())),
                        );

                        ui.add_space(24.0);

                        if let Some(dns_id) = person.dns_id.as_deref() {
                            ui.label(dns_id);
                        }
                    });
                });
            });

            ui.add_space(12.0);

            if let Some(about) = person.about.as_deref() {
                ui.label(about);
            }

            ui.add_space(12.0);

            ui.separator();
        }
    });
}
