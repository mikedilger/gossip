use super::{GossipUi, Page};
use crate::comms::BusMessage;
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{Context, RichText, ScrollArea, TextStyle, TopBottomPanel, Ui, Vec2};

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    TopBottomPanel::top("people_menu").show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut app.page, Page::PeopleList, "Followed");
            ui.separator();
            ui.selectable_value(&mut app.page, Page::PeopleFollow, "Follow Someone New");
            ui.separator();
        });
    });

    if app.page == Page::PeopleFollow {
        ui.add_space(24.0);

        ui.add_space(8.0);
        ui.heading("Follow someone");
        ui.add_space(18.0);

        ui.separator();

        ui.horizontal(|ui| {
            ui.label("Enter user@domain");
            ui.text_edit_singleline(&mut app.nip35follow);
            if ui.button("follow").clicked() {
                let tx = GLOBALS.to_overlord.clone();
                let _ = tx.send(BusMessage {
                    target: "overlord".to_string(),
                    kind: "follow_nip35".to_string(),
                    json_payload: serde_json::to_string(&app.nip35follow).unwrap(),
                });
                app.nip35follow = "".to_owned();
            }
        });
    } else if app.page == Page::PeopleList {
        ui.add_space(24.0);

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
}
