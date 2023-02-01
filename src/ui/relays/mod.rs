use super::{GossipUi, Page};
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{Align, Context, Layout, SelectableLabel, Ui};
use egui_extras::{Column, TableBuilder};

mod all;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.horizontal(|ui| {
        if ui
            .add(SelectableLabel::new(
                app.page == Page::RelaysLive,
                "Live",
            ))
            .clicked()
        {
            app.set_page(Page::RelaysLive);
        }
        ui.separator();
        if ui
            .add(SelectableLabel::new(
                app.page == Page::RelaysAll,
                "Configure"))
            .clicked()
        {
            app.set_page(Page::RelaysAll);
        }
        ui.separator();
    });
    ui.separator();

    if app.page == Page::RelaysLive {

        ui.add_space(10.0);

        ui.with_layout(Layout::top_down(Align::Min), |ui| {
            ui.heading("Connected To");
            for url in GLOBALS.relays_watching.blocking_read().iter() {
                ui.label(&url.0);
            }
        });

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        ui.heading("Serving Long-term General Feed");

        let relay_assignments = GLOBALS.relay_assignments.blocking_read().clone();

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
                        ui.heading("Num Pubkeys");
                    });
                }).body(|body| {
                    body.rows(24.0, relay_assignments.len(), |row_index, mut row| {
                        row.col(|ui| {
                            ui.label(&relay_assignments[row_index].relay.url.0);
                        });
                        row.col(|ui| {
                            ui.label(format!("{}", relay_assignments[row_index].pubkeys.len()));
                        });
                    });
                });
        });


    } else if app.page == Page::RelaysAll {
        all::update(app, ctx, frame, ui);
    }
}
