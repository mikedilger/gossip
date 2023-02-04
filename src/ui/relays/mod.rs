use super::{GossipUi, Page};
use crate::db::DbRelay;
use crate::globals::GLOBALS;
use crate::relay_assignment::RelayAssignment;
use eframe::egui;
use egui::{Context, ScrollArea, SelectableLabel, Ui};
use egui_extras::{Column, TableBuilder};

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

        let relays_watching = GLOBALS.relays_watching.blocking_read().clone();
        let mut relay_assignments = GLOBALS.relay_assignments.blocking_read().clone();
        let relays: Vec<RelayAssignment> = relays_watching
            .iter()
            .map(|url| {
                if let Some(pos) = relay_assignments.iter().position(|r| r.relay.url == *url) {
                    relay_assignments.swap_remove(pos)
                } else {
                    RelayAssignment {
                        relay: DbRelay::new(url.to_owned()),
                        pubkeys: vec![],
                    }
                }
            })
            .collect();

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
                    body.rows(24.0, relays.len(), |row_index, mut row| {
                        row.col(|ui| {
                            ui.label(&relays[row_index].relay.url.0);
                        });
                        row.col(|ui| {
                            ui.label(format!("{}", &relays[row_index].pubkeys.len()));
                        });
                    });
                });
        });

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        ui.heading("Coverage");
        ui.add_space(12.0);

        ScrollArea::vertical()
            .id_source("relay_coverage")
            .show(ui, |ui| {
                if !GLOBALS
                    .relay_picker
                    .blocking_read()
                    .pubkey_counts
                    .is_empty()
                {
                    for (pk, count) in GLOBALS.relay_picker.blocking_read().pubkey_counts.iter() {
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
