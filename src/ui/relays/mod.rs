use super::{GossipUi, Page};
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{Context, ScrollArea, SelectableLabel, Ui};
use egui_extras::{Column, TableBuilder};
use nostr_types::RelayUrl;

mod all;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.horizontal(|ui| {
        if ui
            .add(SelectableLabel::new(app.page == Page::RelaysLive, "Live"))
            .clicked()
        {
            app.set_page(Page::RelaysLive);
        }
        ui.separator();
        if ui
            .add(SelectableLabel::new(
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

    if app.page == Page::RelaysLive {
        ui.add_space(10.0);

        ui.heading("Connected Relays");
        ui.add_space(18.0);

        let connected_relays: Vec<RelayUrl> = GLOBALS
            .relay_tracker
            .connected_relays
            .iter()
            .map(|r| r.key().clone())
            .collect();

        ScrollArea::vertical()
            .id_source("relay_coverage")
            .show(ui, |ui| {
                ui.push_id("general_feed_relays", |ui| {
                    TableBuilder::new(ui)
                        .striped(true)
                        .column(Column::auto_with_initial_suggestion(250.0).resizable(true))
                        .column(Column::auto().resizable(true))
                        .header(20.0, |mut header| {
                            header.col(|ui| {
                                ui.heading("Relay URL");
                            });
                            header.col(|ui| {
                                ui.heading("Num Keys");
                            });
                        })
                        .body(|body| {
                            body.rows(24.0, connected_relays.len(), |row_index, mut row| {
                                let relay_url = &connected_relays[row_index];
                                row.col(|ui| {
                                    ui.label(&relay_url.0);
                                });
                                row.col(|ui| {
                                    if let Some(ref assignment) = GLOBALS.relay_tracker.relay_assignments.get(relay_url) {
                                        ui.label(format!("{}", assignment.pubkeys.len()));
                                    }
                                });
                            });
                        });
                });

                ui.add_space(10.0);
                ui.separator();
                ui.add_space(10.0);

                ui.add_space(12.0);
                ui.heading("Coverage");

                if !GLOBALS
                    .relay_tracker
                    .pubkey_counts
                    .is_empty()
                {
                    for elem in GLOBALS.relay_tracker.pubkey_counts.iter() {
                        let pk = elem.key();
                        let count = elem.value();
                        let maybe_person = GLOBALS.people.get(pk);
                        let name = match maybe_person {
                            None => GossipUi::hex_pubkey_short(pk),
                            Some(p) => match p.name() {
                                None => GossipUi::hex_pubkey_short(pk),
                                Some(n) => n.to_owned(),
                            },
                        };
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
