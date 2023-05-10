use super::{GossipUi, Page};
use crate::comms::ToOverlordMessage;
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{Context, ScrollArea, Ui, Vec2};
use egui_extras::{Column, TableBuilder};
use nostr_types::RelayUrl;

mod all;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, frame: &mut eframe::Frame, ui: &mut Ui) {
    #[cfg(not(feature = "side-menu"))]
    {
        ui.horizontal(|ui| {
            if ui
                .add(egui::SelectableLabel::new(app.page == Page::RelaysLive, "Live"))
                .clicked()
            {
                app.set_page(Page::RelaysLive);
            }
            ui.separator();
            if ui
                .add(egui::SelectableLabel::new(
                    app.page == Page::RelaysAll,
                    "Configure",
                ))
                .clicked()
            {
                app.set_page(Page::RelaysAll);
            }
            ui.separator();
        });
        ui.separator();
    }

    if app.page == Page::RelaysLive {
        ui.add_space(10.0);

        ui.heading("Connected Relays");
        ui.add_space(18.0);

        let connected_relays: Vec<RelayUrl> = GLOBALS
            .connected_relays
            .iter()
            .map(|r| r.key().clone())
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
                        .header(20.0, |mut header| {
                            header.col(|ui| {
                                ui.heading("Relay URL");
                            });
                            header.col(|ui| {
                                ui.heading("Num Keys");
                            });
                            header.col(|_| {});
                        })
                        .body(|body| {
                            body.rows(24.0, connected_relays.len(), |row_index, mut row| {
                                let relay_url = &connected_relays[row_index];
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
                        let name = GossipUi::display_name_from_pubkeyhex_lookup(pk);
                        ui.label(format!("{}: coverage short by {} relay(s)", name, count));
                    }
                } else {
                    ui.label("All followed people are fully covered.".to_owned());
                }
            });
    } else if app.page == Page::RelaysAll {
        all::update(app, ctx, frame, ui);
    }
}
