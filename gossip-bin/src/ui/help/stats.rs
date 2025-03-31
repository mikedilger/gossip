use super::GossipUi;
use eframe::egui;
use egui::{Context, Ui};
use gossip_lib::{FollowingsTable, HandlersTable, PersonTable, Table, GLOBALS};
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

        let num_stalled = GLOBALS.fetcher.num_requests_stalled();
        let num_in_flight = GLOBALS.fetcher.num_requests_in_flight();

        ui.label(format!("HTTP Requests in flight: {}", num_in_flight));
        ui.label(format!("HTTP Requests queued: {}", num_stalled));

        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);

        ui.label(format!(
            "Number of known relays: {}",
            match GLOBALS.db().filter_relays(|_| true) {
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
            "General: {} bytes",
            GLOBALS.db().get_general_size().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "Events: {} bytes, {} events",
            GLOBALS.db().get_event_size().unwrap_or(0),
            GLOBALS.db().get_event_len().unwrap_or(0),
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "Event Index (Author + Kind): {} bytes",
            GLOBALS.db().get_event_akci_index_size().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "Event Index (Kind): {} bytes",
            GLOBALS.db().get_event_kci_index_size().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "Event Index (Tags): {} bytes",
            GLOBALS.db().get_event_tci_index_size().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "Event Seen on Relay: {} bytes",
            GLOBALS.db().get_event_seen_on_relay_size().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "Event Viewed: {} bytes",
            GLOBALS.db().get_event_viewed_size().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "Hashtags: {} bytes",
            GLOBALS.db().get_hashtags_size().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "Relays: {} bytes",
            GLOBALS.db().get_relays_size().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "People: {} bytes",
            PersonTable::bytes_used().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "Person-Relays: {} bytes",
            GLOBALS.db().get_person_relays_size().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "Person-Lists: {} bytes",
            GLOBALS.db().get_person_lists_size().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "Event Relationships By Id: {} bytes",
            GLOBALS.db().get_relationships_by_id_size().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "Event Relationships By Addr: {} bytes",
            GLOBALS.db().get_relationships_by_addr_size().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "Nip46 Servers: {} bytes",
            GLOBALS.db().get_nip46servers_size().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "Followings: {} bytes",
            FollowingsTable::bytes_used().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "FoF: {} bytes",
            GLOBALS.db().get_fof_size().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "Handlers: {} bytes",
            HandlersTable::bytes_used().unwrap_or(0)
        ));
        ui.add_space(6.0);

        ui.label(format!(
            "Configured Handlers: {} bytes",
            GLOBALS.db().get_configured_handlers_size().unwrap_or(0)
        ));
        ui.add_space(6.0);
    });
}
