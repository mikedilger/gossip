use super::GossipUi;
use crate::comms::ToOverlordMessage;
use crate::db::DbRelay;
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{Align, Context, Layout, TextEdit, Ui};
use egui_extras::{Column, TableBuilder};
use nostr_types::Url;

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.add_space(8.0);
    ui.heading("Relays");
    ui.add_space(18.0);

    ui.horizontal(|ui| {
        ui.label("Enter a new relay URL:");
        ui.add(TextEdit::singleline(&mut app.new_relay_url));
        if ui.button("Add").clicked() {
            let test_url = Url::new(&app.new_relay_url);
            if test_url.is_valid_relay_url() {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::AddRelay(app.new_relay_url.clone()));
                if let Ok(db_relay) = DbRelay::new(app.new_relay_url.clone()) {
                    GLOBALS.relays.blocking_write().insert(test_url, db_relay);
                }
                app.new_relay_url = "".to_owned();
                *GLOBALS.status_message.blocking_write() = format!(
                    "I asked the overlord to add relay {}. Check for it below.",
                    &app.new_relay_url
                );
            } else {
                *GLOBALS.status_message.blocking_write() =
                    "That's not a valid relay URL.".to_owned();
            }
        }
    });

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    // TBD time how long this takes. We don't want expensive code in the UI
    let mut relays = GLOBALS.relays.blocking_read().clone();
    let mut relays: Vec<DbRelay> = relays.drain().map(|(_, relay)| relay).collect();
    relays.sort_by(|a, b| a.url.cmp(&b.url));

    let mut postrelays: Vec<DbRelay> = relays
        .iter()
        .filter(|r| r.post)
        .map(|r| r.to_owned())
        .collect();

    ui.horizontal(|ui| {
        ui.set_max_height(0.0); // tries to be as short as possible

        ui.with_layout(Layout::top_down(Align::Min), |ui| {
            ui.heading("Connected to:");
            for url in GLOBALS.relays_watching.blocking_read().iter() {
                ui.label(url.inner());
            }
        });

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(12.0);

        ui.with_layout(Layout::top_down(Align::Min), |ui| {
            ui.heading("Writing to:");
            relay_table(ui, &mut postrelays, "postrelays");
        });
    });

    ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
        if ui.button("SAVE CHANGES").clicked() {
            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::SaveRelays);
        }

        ui.with_layout(Layout::top_down(Align::Min), |ui| {
            ui.heading("Other Known Relays:");
            relay_table(ui, &mut relays, "otherrelays");
        });
    });

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);
}

fn relay_table(ui: &mut Ui, relays: &mut [DbRelay], id: &'static str) {
    ui.push_id(id, |ui| {
        TableBuilder::new(ui)
            .column(Column::auto_with_initial_suggestion(100.0).resizable(true))
            .column(Column::auto().resizable(true))
            .column(Column::auto().resizable(true))
            .column(Column::remainder())
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.heading("Relay URL");
                });
                header.col(|ui| {
                    ui.heading("Success Rate (%)");
                });
                header.col(|ui| {
                    ui.heading("Attempts");
                });
                header.col(|ui| {
                    ui.heading("Post Here");
                });
            }).body(|mut body| {
                for relay in relays.iter_mut() {
                    body.row(30.0, |mut row| {
                        row.col(|ui| {
                            ui.label(&relay.url);
                        });
                        row.col(|ui| {
                            ui.label(&format!("{}", (relay.success_rate() * 100.0) as u32));
                        });
                        row.col(|ui| {
                            ui.label(&format!("{}", relay.attempts()));
                        });
                        row.col(|ui| {
                            let mut post = relay.post; // checkbox needs a mutable state variable.
                            let url = Url::new(&relay.url);
                            if url.is_valid_relay_url() && ui.checkbox(&mut post, "Post Here")
                                .on_hover_text("If selected, posts you create will be sent to this relay. But you have to press [SAVE CHANGES] at the bottom of this page.")
                                .clicked()
                            {
                                if let Some(relay) = GLOBALS.relays.blocking_write().get_mut(&url) {
                                    relay.post = post;
                                    relay.dirty = true;
                                }
                            }
                        });
                    });
                }
            });
    });
}
