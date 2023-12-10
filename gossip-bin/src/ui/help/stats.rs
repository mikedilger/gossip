use super::GossipUi;
use eframe::egui;
use egui::{Context, Ui};
use gossip_lib::GLOBALS;
use humansize::{format_size, DECIMAL};
use std::sync::atomic::Ordering;

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.add_space(10.0);
    ui.heading("Statistics".to_string());
    ui.add_space(12.0);
    ui.separator();

    ui.add_space(10.0);

    app.vert_scroll_area().show(ui, |ui| {
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

        ui.label(format!(
            "General: {} records",
            GLOBALS.storage.get_general_len().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "Events: {} records",
            GLOBALS.storage.get_event_len().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "Event Index (EK-PK): {} records",
            GLOBALS.storage.get_event_ek_pk_index_len().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "Event Index (EK-C): {} records",
            GLOBALS.storage.get_event_ek_c_index_len().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "Event Index (Tags): {} records",
            GLOBALS.storage.get_event_tag_index_len().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "Event Seen on Relay: {} records",
            GLOBALS.storage.get_event_seen_on_relay_len().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "Event Viewed: {} records",
            GLOBALS.storage.get_event_viewed_len().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "Hashtags: {} records",
            GLOBALS.storage.get_hashtags_len().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "Relays: {} records",
            GLOBALS.storage.get_relays_len().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "People: {} records",
            GLOBALS.storage.get_people_len().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "Person-Relays: {} records",
            GLOBALS.storage.get_person_relays_len().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "Person-Lists: {} records",
            GLOBALS.storage.get_person_lists_len().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "Event Relationships By Id: {} records",
            GLOBALS.storage.get_relationships_by_id_len().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "Event Relationships By Addr: {} records",
            GLOBALS.storage.get_relationships_by_addr_len().unwrap_or(0)
        ));
        ui.add_space(6.0);
    });
}
