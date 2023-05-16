use super::GossipUi;
use crate::db::DbRelay;
use crate::globals::GLOBALS;
use crate::{comms::ToOverlordMessage, ui::components};
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
        ui.checkbox(&mut app.show_hidden_relays, "Show hidden relays");
    });

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    // TBD time how long this takes. We don't want expensive code in the UI
    // FIXME keep more relay info and display it
    let mut relays: Vec<DbRelay> = GLOBALS
        .all_relays
        .iter()
        .map(|ri| ri.value().clone())
        .filter(|ri| app.show_hidden_relays || !ri.hidden)
        .collect();
    relays.sort_by(|a, b| {
        b.has_usage_bits(DbRelay::WRITE)
            .cmp(&a.has_usage_bits(DbRelay::WRITE))
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

fn relay_table(ui: &mut Ui, relays: &mut [DbRelay], id: &'static str) {
    egui::ScrollArea::vertical()
        .id_source(id)
        // .override_scroll_delta(Vec2 {
        //     x: 0.0,
        //     y: app.current_scroll_offset * 2.0, // double speed
        // })
        .show(ui, |ui| {
            for relay in relays {
                ui.add(components::RelayEntry::new(relay));
            }
        });
}
