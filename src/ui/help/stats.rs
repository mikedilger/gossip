use super::GossipUi;
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{Context, ScrollArea, Ui};
use humansize::{format_size, DECIMAL};
use std::sync::atomic::Ordering;

pub(super) fn update(_app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.add_space(24.0);
    ui.heading("Statistics".to_string());
    ui.add_space(12.0);
    ui.separator();

    ui.add_space(10.0);

    ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(10.0);

        ui.label(format!(
            "Total Bytes Read: {}",
            format_size(GLOBALS.bytes_read.load(Ordering::Relaxed), DECIMAL)
        ));

        ui.add_space(6.0);

        ui.label(format!(
            "HTTP Requests in flight: {}",
            GLOBALS.fetcher.requests_in_flight()
        ));

        ui.label(format!(
            "HTTP Requests queued: {}",
            GLOBALS.fetcher.requests_queued()
        ));

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);

        ui.label(format!(
            "Number of known relays: {}",
            match GLOBALS.storage.filter_relays(|_| true) {
                Err(e) => {
                    tracing::error!("{}", e);
                    0
                }
                Ok(vec) => vec.len(),
            }
        ));

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);

        let general_stats = GLOBALS
            .storage
            .get_general_stats()
            .map(|s| format!("General: {} records, {} pages", s.entries(), s.leaf_pages()))
            .unwrap_or("".to_owned());
        ui.label(general_stats);
        ui.add_space(6.0);

        let event_stats = GLOBALS
            .storage
            .get_event_stats()
            .map(|s| format!("Events: {} records, {} pages", s.entries(), s.leaf_pages()))
            .unwrap_or("".to_owned());
        ui.label(event_stats);
        ui.add_space(6.0);

        let event_ek_pk_index_stats = GLOBALS
            .storage
            .get_event_ek_pk_index_stats()
            .map(|s| {
                format!(
                    "Event Index (EK-PK): {} records, {} pages",
                    s.entries(),
                    s.leaf_pages()
                )
            })
            .unwrap_or("".to_owned());
        ui.label(event_ek_pk_index_stats);
        ui.add_space(6.0);

        let event_ek_c_index_stats = GLOBALS
            .storage
            .get_event_ek_c_index_stats()
            .map(|s| {
                format!(
                    "Event Index (EK-C): {} records, {} pages",
                    s.entries(),
                    s.leaf_pages()
                )
            })
            .unwrap_or("".to_owned());
        ui.label(event_ek_c_index_stats);
        ui.add_space(6.0);

        let event_references_person_stats = GLOBALS
            .storage
            .get_event_references_person_stats()
            .map(|s| {
                format!(
                    "Event Index (References Person): {} records, {} pages",
                    s.entries(),
                    s.leaf_pages()
                )
            })
            .unwrap_or("".to_owned());
        ui.label(event_references_person_stats);
        ui.add_space(6.0);

        let event_tags_stats = GLOBALS
            .storage
            .get_event_tags_stats()
            .map(|s| {
                format!(
                    "Event Index (Tags): {} records, {} pages",
                    s.entries(),
                    s.leaf_pages()
                )
            })
            .unwrap_or("".to_owned());
        ui.label(event_tags_stats);
        ui.add_space(6.0);

        let relationships_stats = GLOBALS
            .storage
            .get_relationships_stats()
            .map(|s| {
                format!(
                    "Event Relationships: {} records, {} pages",
                    s.entries(),
                    s.leaf_pages()
                )
            })
            .unwrap_or("".to_owned());
        ui.label(relationships_stats);
        ui.add_space(6.0);

        let event_seen_on_relay_stats = GLOBALS
            .storage
            .get_event_seen_on_relay_stats()
            .map(|s| {
                format!(
                    "Event Seen-on Relay: {} records, {} pages",
                    s.entries(),
                    s.leaf_pages()
                )
            })
            .unwrap_or("".to_owned());
        ui.label(event_seen_on_relay_stats);
        ui.add_space(6.0);

        let event_viewed_stats = GLOBALS
            .storage
            .get_event_viewed_stats()
            .map(|s| {
                format!(
                    "Event Viewed: {} records, {} pages",
                    s.entries(),
                    s.leaf_pages()
                )
            })
            .unwrap_or("".to_owned());
        ui.label(event_viewed_stats);
        ui.add_space(6.0);

        let hashtags_stats = GLOBALS
            .storage
            .get_hashtags_stats()
            .map(|s| {
                format!(
                    "Hashtags: {} records, {} pages",
                    s.entries(),
                    s.leaf_pages()
                )
            })
            .unwrap_or("".to_owned());
        ui.label(hashtags_stats);
        ui.add_space(6.0);

        let relays_stats = GLOBALS
            .storage
            .get_relays_stats()
            .map(|s| format!("Relays: {} records, {} pages", s.entries(), s.leaf_pages()))
            .unwrap_or("".to_owned());
        ui.label(relays_stats);
        ui.add_space(6.0);

        let people_stats = GLOBALS
            .storage
            .get_people_stats()
            .map(|s| format!("People: {} records, {} pages", s.entries(), s.leaf_pages()))
            .unwrap_or("".to_owned());
        ui.label(people_stats);
        ui.add_space(6.0);

        let person_relays_stats = GLOBALS
            .storage
            .get_person_relays_stats()
            .map(|s| {
                format!(
                    "Person-Relays: {} records, {} pages",
                    s.entries(),
                    s.leaf_pages()
                )
            })
            .unwrap_or("".to_owned());
        ui.label(person_relays_stats);
        ui.add_space(6.0);
    });
}
