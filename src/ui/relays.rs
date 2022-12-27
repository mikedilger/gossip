use super::GossipUi;
use crate::comms::BusMessage;
use crate::db::DbRelay;
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{Align, Context, Layout, RichText, ScrollArea, TextStyle, Ui};
use nostr_types::Url;

pub(super) fn update(_app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.add_space(8.0);
    ui.heading("Relays known");
    ui.add_space(18.0);

    ui.label(
        RichText::new(
            "Relays on this list have been automatically discovered in various kinds of events.",
        )
        .text_style(TextStyle::Body),
    );

    // TBD time how long this takes. We don't want expensive code in the UI
    let mut relays = GLOBALS.relays.blocking_read().clone();
    let mut relays: Vec<DbRelay> = relays.drain().map(|(_, relay)| relay).collect();
    relays.sort_by(|a, b| a.url.cmp(&b.url));

    ui.add_space(32.0);

    ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
        if ui.button("SAVE CHANGES").clicked() {
            let tx = GLOBALS.to_overlord.clone();
            let _ = tx.send(BusMessage {
                target: "overlord".to_string(),
                kind: "save_relays".to_string(),
                json_payload: serde_json::to_string("").unwrap(),
            });
        }

        ui.with_layout(Layout::top_down(Align::Center), |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                ui.label("This is the scroll area top");
                for relay in relays.iter_mut() {
                    ui.horizontal(|ui| {
                        ui.label(&relay.url);

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {

                            let mut post = relay.post; // checkbox needs a mutable state variable.

                            if ui.checkbox(&mut post, "Post Here")
                                .on_hover_text("If selected, posts you create will be sent to this relay. But you have to press [SAVE CHANGES] at the bottom of this page.")
                                .clicked()
                            {
                                if let Some(relay) = GLOBALS.relays.blocking_write().get_mut(&Url(relay.url.clone())) {
                                    relay.post = post;
                                    relay.dirty = true;
                                }
                            }

                            //if ui.button("CONNECT").clicked() {
                            //    ui.label("TBD");
                            //}
                        });
                    });

                    ui.add_space(12.0);
                    ui.separator();
                }
                ui.label("This is the scroll area bottom");
            });
        });
    });
}
