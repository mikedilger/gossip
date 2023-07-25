use super::GossipUi;
use crate::comms::ToOverlordMessage;
use crate::globals::GLOBALS;
use crate::relay::Relay;
use eframe::egui;
use egui::{Align, Context, Layout, Ui};
use egui_extras::{Column, TableBuilder};
use nostr_types::{RelayUrl, Unixtime};

const READ_HOVER_TEXT: &str = "Where you actually read events from (including those tagging you, but also for other purposes).";
const INBOX_HOVER_TEXT: &str = "Where you tell others you read from. You should also check Read. These relays shouldn't require payment. It is recommended to have a few.";
const DISCOVER_HOVER_TEXT: &str = "Where you discover other people's relays lists.";
const WRITE_HOVER_TEXT: &str =
    "Where you actually write your events to. It is recommended to have a few.";
const OUTBOX_HOVER_TEXT: &str = "Where you tell others you write to. You should also check Write. It is recommended to have a few.";
const ADVERTISE_HOVER_TEXT: &str = "Where you advertise your relay list (inbox/outbox) to. It is recommended to advertise to lots of relays so that you can be found.";

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.add_space(16.0);
    ui.heading("Relays List");

    ui.horizontal(|ui| {
        ui.label("Enter a new relay URL:");
        ui.add(text_edit_line!(app, app.new_relay_url));
        if ui.button("Add").clicked() {
            if let Ok(url) = RelayUrl::try_from_str(&app.new_relay_url) {
                let _ = GLOBALS.to_overlord.send(ToOverlordMessage::AddRelay(url));
                GLOBALS.status_queue.write().write(format!(
                    "I asked the overlord to add relay {}. Check for it below.",
                    &app.new_relay_url
                ));
                app.new_relay_url = "".to_owned();
            } else {
                GLOBALS
                    .status_queue
                    .write()
                    .write("That's not a valid relay URL.".to_owned());
            }
        }
        ui.separator();
        if ui.button("↑ Advertise Relay List ↑").clicked() {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::AdvertiseRelayList);
        }
        ui.checkbox(&mut app.show_hidden_relays, "Show hidden relays");
    });

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    // TBD time how long this takes. We don't want expensive code in the UI
    // FIXME keep more relay info and display it
    let mut relays: Vec<Relay> = GLOBALS
        .storage
        .filter_relays(|relay| app.show_hidden_relays || !relay.hidden)
        .unwrap_or(vec![]);

    relays.sort_by(|a, b| {
        b.has_usage_bits(Relay::WRITE)
            .cmp(&a.has_usage_bits(Relay::WRITE))
            .then(a.url.cmp(&b.url))
    });

    ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
        ui.add_space(18.0);

        ui.with_layout(Layout::top_down(Align::Min), |ui| {
            ui.heading("All Known Relays:");
            relay_table(ui, &mut relays, "allrelays");
        });
    });
}

fn relay_table(ui: &mut Ui, relays: &mut [Relay], id: &'static str) {
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
                    ui.heading("Read").on_hover_text(READ_HOVER_TEXT);
                });
                header.col(|ui| {
                    ui.heading("Inbox").on_hover_text(INBOX_HOVER_TEXT);
                });
                header.col(|ui| {
                    ui.heading("Discover").on_hover_text(DISCOVER_HOVER_TEXT);
                });
                header.col(|ui| {
                    ui.heading("Write").on_hover_text(WRITE_HOVER_TEXT);
                });
                header.col(|ui| {
                    ui.heading("Outbox").on_hover_text(OUTBOX_HOVER_TEXT);
                });
                header.col(|ui| {
                    ui.heading("Advertise").on_hover_text(ADVERTISE_HOVER_TEXT);
                });
                header.col(|ui| {
                    ui.heading("Read rank")
                        .on_hover_text("How likely we will connect to relays to read other people's posts, from 0 (never) to 9 (highly). Default is 3.".to_string());
                });
                header.col(|ui| {
                    ui.heading("Hide")
                        .on_hover_text("Hide this relay.".to_string());
                });
            }).body(|body| {
                body.rows(24.0, relays.len(), |row_index, mut row| {
                    let relay = relays.get_mut(row_index).unwrap();
                    row.col(|ui| {
                        crate::ui::widgets::break_anywhere_label(ui,&relay.url.0);
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
                        let mut read = relay.has_usage_bits(Relay::READ); // checkbox needs a mutable state variable.
                        if ui.checkbox(&mut read, "")
                            .on_hover_text(READ_HOVER_TEXT)
                            .clicked()
                        {
                            let _ = GLOBALS
                                .to_overlord
                                .send(ToOverlordMessage::AdjustRelayUsageBit(relay.url.clone(), Relay::READ, read));
                        }
                    });
                    row.col(|ui| {
                        let mut inbox = relay.has_usage_bits(Relay::INBOX); // checkbox needs a mutable state variable.
                        if ui.checkbox(&mut inbox, "")
                            .on_hover_text(INBOX_HOVER_TEXT)
                            .clicked()
                        {
                            let _ = GLOBALS
                                .to_overlord
                                .send(ToOverlordMessage::AdjustRelayUsageBit(relay.url.clone(), Relay::INBOX, inbox));
                        }
                    });
                    row.col(|ui| {
                        let mut discover = relay.has_usage_bits(Relay::DISCOVER); // checkbox needs a mutable state variable.
                        if ui.checkbox(&mut discover, "")
                            .on_hover_text(DISCOVER_HOVER_TEXT)
                            .clicked()
                        {
                            let _ = GLOBALS
                                .to_overlord
                                .send(ToOverlordMessage::AdjustRelayUsageBit(relay.url.clone(), Relay::DISCOVER, discover));
                        }
                    });
                    row.col(|ui| {
                        let mut write = relay.has_usage_bits(Relay::WRITE); // checkbox needs a mutable state variable.
                        if ui.checkbox(&mut write, "")
                            .on_hover_text(WRITE_HOVER_TEXT)
                            .clicked()
                        {
                            let _ = GLOBALS
                                .to_overlord
                                .send(ToOverlordMessage::AdjustRelayUsageBit(relay.url.clone(), Relay::WRITE, write));
                        }
                    });
                    row.col(|ui| {
                        let mut outbox = relay.has_usage_bits(Relay::OUTBOX); // checkbox needs a mutable state variable.
                        if ui.checkbox(&mut outbox, "")
                            .on_hover_text(OUTBOX_HOVER_TEXT)
                            .clicked()
                        {
                            let _ = GLOBALS
                                .to_overlord
                                .send(ToOverlordMessage::AdjustRelayUsageBit(relay.url.clone(), Relay::OUTBOX, outbox));
                        }
                    });
                    row.col(|ui| {
                        let mut advertise = relay.has_usage_bits(Relay::ADVERTISE); // checkbox needs a mutable state variable.
                        if ui.checkbox(&mut advertise, "")
                            .on_hover_text(ADVERTISE_HOVER_TEXT)
                            .clicked()
                        {
                            let _ = GLOBALS
                                .to_overlord
                                .send(ToOverlordMessage::AdjustRelayUsageBit(relay.url.clone(), Relay::ADVERTISE, advertise));
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
                    row.col(|ui| {
                        let icon = if relay.hidden { "♻️" } else { "🗑️" };
                        if ui.button(icon).clicked() {
                            let _ = GLOBALS
                                .to_overlord
                                .send(ToOverlordMessage::HideOrShowRelay(relay.url.clone(), !relay.hidden));
                        }
                    });
                })
            });
    });
}
