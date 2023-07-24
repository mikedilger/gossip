use super::{GossipUi, Page};
use crate::comms::ToOverlordMessage;
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{Context, ScrollArea, Ui, Vec2};
use egui_extras::{Column, TableBuilder};
use nostr_types::{RelayUrl, Unixtime};

mod all;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, frame: &mut eframe::Frame, ui: &mut Ui) {
    if app.page == Page::RelaysLive {
        ui.add_space(10.0);

        ui.heading("Connected Relays");
        ui.add_space(18.0);

        let connected_relays: Vec<(RelayUrl, String)> = GLOBALS
            .connected_relays
            .iter()
            .map(|r| {
                (
                    r.key().clone(),
                    r.value()
                        .iter()
                        .map(|rj| {
                            if rj.persistent {
                                format!("[{}]", rj.reason)
                            } else {
                                rj.reason.to_string()
                            }
                        })
                        .collect::<Vec<String>>()
                        .join(", "),
                )
            })
            .collect();

        ScrollArea::vertical()
            .id_source("relay_coverage")
            .override_scroll_delta(Vec2 {
                x: 0.0,
                y: app.current_scroll_offset,
            })
            .show(ui, |ui| {
                ui.push_id("general_feed_relays", |ui| {
                    TableBuilder::new(ui)
                        .striped(true)
                        .column(Column::auto_with_initial_suggestion(250.0).resizable(true))
                        .column(Column::auto().resizable(true))
                        .column(Column::auto().resizable(true))
                        .column(Column::auto().resizable(true))
                        .header(20.0, |mut header| {
                            header.col(|ui| {
                                ui.heading("Relay URL");
                            });
                            header.col(|ui| {
                                ui.heading("# Keys");
                            });
                            header.col(|ui| {
                                ui.heading("Reasons")
                                    .on_hover_text("Reasons in [brackets] are persistent based on your relay usage configurations; if the connection drops, it will be restarted and resubscribed after a delay.");
                            });
                            header.col(|_| {});
                        })
                        .body(|body| {
                            body.rows(24.0, connected_relays.len(), |row_index, mut row| {
                                let relay_url = &connected_relays[row_index].0;
                                let reasons = &connected_relays[row_index].1;
                                row.col(|ui| {
                                    crate::ui::widgets::break_anywhere_label(ui, &relay_url.0);
                                });
                                row.col(|ui| {
                                    if let Some(ref assignment) =
                                        GLOBALS.relay_picker.get_relay_assignment(relay_url)
                                    {
                                        ui.label(format!("{}", assignment.pubkeys.len()));
                                    }
                                });
                                row.col(|ui| {
                                    ui.label(reasons);
                                });
                                row.col(|ui| {
                                    if ui.button("Disconnect").clicked() {
                                        let _ = GLOBALS.to_overlord.send(
                                            ToOverlordMessage::DropRelay(relay_url.to_owned()),
                                        );
                                    }
                                });
                            });
                        });
                });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                if ui.button("Pick Again").clicked() {
                    let _ = GLOBALS.to_overlord.send(ToOverlordMessage::PickRelays);
                }

                ui.add_space(12.0);
                ui.heading("Coverage");

                if GLOBALS.relay_picker.pubkey_counts_iter().count() > 0 {
                    for elem in GLOBALS.relay_picker.pubkey_counts_iter() {
                        let pk = elem.key();
                        let count = elem.value();
                        let name = GossipUi::display_name_from_pubkey_lookup(pk);
                        ui.label(format!("{}: coverage short by {} relay(s)", name, count));
                    }
                } else {
                    ui.label("All followed people are fully covered.".to_owned());
                }

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                ui.heading("Penalty Box");
                ui.add_space(10.0);

                let now = Unixtime::now().unwrap().0;

                let excluded: Vec<(String, i64)> = GLOBALS.relay_picker.excluded_relays_iter().map(|refmulti| {
                    (refmulti.key().as_str().to_owned(),
                     *refmulti.value() - now)
                }).collect();

                TableBuilder::new(ui)
                    .striped(true)
                    .column(Column::auto().resizable(true))
                    .column(Column::auto().resizable(true))
                    .header(20.0, |mut header| {
                        header.col(|ui| {
                            ui.heading("Relay URL");
                        });
                        header.col(|ui| {
                            ui.heading("Time Remaining");
                        });
                    })
                    .body(|body| {
                        body.rows(24.0, excluded.len(), |row_index, mut row| {
                            let data = &excluded[row_index];
                            row.col(|ui| {
                                ui.label(&data.0);
                            });
                            row.col(|ui| {
                                ui.label(format!("{}", data.1));
                            });
                        });
                    });
            });
    } else if app.page == Page::RelaysAll {
        all::update(app, ctx, frame, ui);
    }
}
