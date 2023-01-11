use super::GossipUi;
use crate::comms::ToOverlordMessage;
use crate::db::DbRelay;
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{Align, Context, Layout, RichText, ScrollArea, TextEdit, Ui};
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

    let postrelays: Vec<DbRelay> = relays
        .iter()
        .filter(|r| r.post)
        .map(|r| r.to_owned())
        .collect();

    ui.add_space(32.0);

    ui.heading("Connected to:");

    for url in GLOBALS.relays_watching.blocking_read().iter() {
        ui.label(url.inner());
    }

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
        if ui.button("SAVE CHANGES").clicked() {
            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::SaveRelays);
        }

        ui.with_layout(Layout::top_down(Align::Center), |ui| {
            ui.heading("Your Relays (write):");

            for relay in postrelays.iter() {
                render_relay(ui, relay, true);
                ui.add_space(3.0);
                ui.separator();
                ui.add_space(3.0);
            }

            ui.heading("Other Known Relays:");

            ScrollArea::vertical().show(ui, |ui| {
                for relay in relays.iter_mut() {
                    if ! postrelays.contains(relay){
                        render_relay(ui, relay, false);
                        ui.add_space(3.0);
                        ui.separator();
                        ui.add_space(3.0);
                    }
                }
            });
        });
    });
}

fn render_relay(ui: &mut Ui, relay: &DbRelay, bold: bool) {
    ui.horizontal(|ui| {
        let mut rt = RichText::new(&relay.url);
        if bold { rt = rt.strong(); }
        ui.label(rt);

        ui.label(&format!("Success={} Failure={}", relay.success_count, relay.failure_count));

        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {

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

            //if ui.button("CONNECT").clicked() {
            //    ui.label("TBD");
            //}
        });
    });
}
