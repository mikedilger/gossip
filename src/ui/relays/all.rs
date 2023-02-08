use super::GossipUi;
use crate::comms::ToOverlordMessage;
use crate::db::DbRelay;
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{Align, Context, Layout, TextEdit, Ui};
use egui_extras::{Column, TableBuilder};
use nostr_types::{RelayUrl, Unixtime};

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.add_space(16.0);
    ui.heading("Relays List");

    ui.horizontal(|ui| {
        ui.label("Enter a new relay URL:");
        ui.add(TextEdit::singleline(&mut app.new_relay_url));
        if ui.button("Add").clicked() {
            if let Ok(url) = RelayUrl::try_from_str(&app.new_relay_url) {
                let _ = GLOBALS.to_overlord.send(ToOverlordMessage::AddRelay(url));
                *GLOBALS.status_message.blocking_write() = format!(
                    "I asked the overlord to add relay {}. Check for it below.",
                    &app.new_relay_url
                );
                app.new_relay_url = "".to_owned();
            } else {
                *GLOBALS.status_message.blocking_write() =
                    "That's not a valid relay URL.".to_owned();
            }
        }
        ui.separator();
        if ui.button("↑ Advertise Relay List ↑").clicked() {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::AdvertiseRelayList);
        }
    });

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    // TBD time how long this takes. We don't want expensive code in the UI
    // FIXME keep more relay info and display it
    let mut relays: Vec<DbRelay> = GLOBALS
        .relay_picker2
        .all_relays
        .iter()
        .map(|ri| ri.value().clone())
        .collect();
    relays.sort_by(|a, b| b.write.cmp(&a.write).then(a.url.cmp(&b.url)));

    ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
        ui.add_space(18.0);

        ui.with_layout(Layout::top_down(Align::Min), |ui| {
            ui.heading("All Known Relays:");
            relay_table(ui, &mut relays, "allrelays");
        });
    });
}

fn relay_table(ui: &mut Ui, relays: &mut [DbRelay], id: &'static str) {
    ui.push_id(id, |ui| {
        TableBuilder::new(ui)
            .striped(true)
            .column(Column::auto_with_initial_suggestion(250.0).resizable(true))
            .column(Column::auto().resizable(true))
            .column(Column::auto().resizable(true))
            .column(Column::auto().resizable(true))
            .column(Column::auto().resizable(true))
            .column(Column::auto().resizable(true))
            .column(Column::auto().resizable(true))
            .column(Column::auto().resizable(true))
            .column(Column::remainder())
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.heading("Relay URL");
                });
                header.col(|ui| {
                    ui.heading("Attempts");
                });
                header.col(|ui| {
                    ui.heading("Success Rate (%)");
                });
                header.col(|ui| {
                    ui.heading("Last Connected");
                });
                header.col(|ui| {
                    ui.heading("Last Event")
                        .on_hover_text("This only counts events served after EOSE, as they mark where we can pick up from next time.");
                });
                header.col(|ui| {
                    ui.heading("Read")
                        .on_hover_text("Read for events with mentions of you on these relays. It is recommended to have a few." );
                });
                header.col(|ui| {
                    ui.heading("Write")
                        .on_hover_text("Write your events to these relays. It is recommended to have a few." );
                });
                header.col(|ui| {
                    ui.heading("Advertise")
                        .on_hover_text("Advertise your read/write settings to this relay. It is recommended to advertise to many relays so that you can be found.");
                });
                header.col(|ui| {
                    ui.heading("Read rank")
                        .on_hover_text("How likely we will connect to relays to read other people's posts, from 0 (never) to 9 (highly). Default is 3.".to_string());
                });
            }).body(|body| {
                body.rows(24.0, relays.len(), |row_index, mut row| {
                    let relay = relays.get_mut(row_index).unwrap();
                    row.col(|ui| {
                        ui.label(&relay.url.0);
                    });
                    row.col(|ui| {
                        ui.label(&format!("{}", relay.attempts()));
                    });
                    row.col(|ui| {
                        ui.label(&format!("{}", (relay.success_rate() * 100.0) as u32));
                    });
                    row.col(|ui| {
                        if let Some(at) = relay.last_connected_at {
                            let ago = crate::date_ago::date_ago(Unixtime(at as i64));
                            ui.label(&ago);
                        }
                    });
                    row.col(|ui| {
                        if let Some(at) = relay.last_general_eose_at {
                            let ago = crate::date_ago::date_ago(Unixtime(at as i64));
                            ui.label(&ago);
                        }
                    });
                    row.col(|ui| {
                        let mut read = relay.read; // checkbox needs a mutable state variable.
                        if ui.checkbox(&mut read, "")
                            .on_hover_text("If selected, we will search for posts mentioning you on this relay.")
                            .clicked()
                        {
                            let _ = GLOBALS
                                .to_overlord
                                .send(ToOverlordMessage::SetRelayReadWrite(relay.url.clone(), read, relay.write));
                        }
                    });
                    row.col(|ui| {
                        let mut write = relay.write; // checkbox needs a mutable state variable.
                        if ui.checkbox(&mut write, "")
                            .on_hover_text("If selected, posts you create will be sent to this relay.")
                            .clicked()
                        {
                            let _ = GLOBALS
                                .to_overlord
                                .send(ToOverlordMessage::SetRelayReadWrite(relay.url.clone(), relay.read, write));
                        }
                    });
                    row.col(|ui| {
                        let mut advertise = relay.advertise; // checkbox needs a mutable state variable.
                        if ui.checkbox(&mut advertise, "")
                            .on_hover_text("If selected, when you send out your relay list advertisements, one of them will go to this relay.")
                            .clicked()
                        {
                            let _ = GLOBALS
                                .to_overlord
                                .send(ToOverlordMessage::SetRelayAdvertise(relay.url.clone(), advertise));
                        }
                    });
                    row.col(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(format!("{}",relay.rank));
                            if ui.button("↓").clicked() && relay.rank>0 {
                                let _ = GLOBALS
                                    .to_overlord
                                    .send(ToOverlordMessage::RankRelay(relay.url.clone(), relay.rank as u8 - 1));
                            }
                            if ui.button("↑").clicked() && relay.rank<9 {
                                let _ = GLOBALS
                                    .to_overlord
                                    .send(ToOverlordMessage::RankRelay(relay.url.clone(), relay.rank as u8 + 1));
                            }
                        });
                    });
                })
            });
    });
}
