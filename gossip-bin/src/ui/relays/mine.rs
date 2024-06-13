use super::GossipUi;
use crate::ui::{widgets, Page};
use eframe::egui;
use egui::{Context, Ui};
use egui_winit::egui::Id;
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::Relay;
use gossip_lib::GLOBALS;
use std::sync::atomic::Ordering;

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    let is_editing = app.relays.edit.is_some();
    widgets::page_header(ui, Page::RelaysMine.name(), |ui| {
        ui.set_enabled(!is_editing);
        super::configure_list_btn(app, ui);
        btn_h_space!(ui);
        super::relay_filter_combo(app, ui);
        btn_h_space!(ui);
        super::relay_sort_combo(app, ui);
        btn_h_space!(ui);
        widgets::TextEdit::search(&app.theme, &app.assets, &mut app.relays.search)
            .desired_width(200.0)
            .show(ui);
        widgets::set_important_button_visuals(ui, app);

        let advertise_remaining = GLOBALS.advertise_jobs_remaining.load(Ordering::Relaxed);
        if advertise_remaining == 0 {
            if ui.button("Advertise Relay List")
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Advertise my relays. Will send your relay usage information to every relay that seems to be working well so that other people know how to follow and contact you.")
                .clicked()
            {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::AdvertiseRelayList);
            }
        } else {
            ui.add_enabled(false, egui::Button::new(
                format!("Advertising, {} to go", advertise_remaining)
            ));
        }
    });

    let relays = if !is_editing {
        // clear edit cache if present
        if !app.relays.edit_relays.is_empty() {
            app.relays.edit_relays.clear()
        }
        get_relays(app)
    } else {
        // when editing, use cached list
        // build list if still empty
        if app.relays.edit_relays.is_empty() {
            app.relays.edit_relays = get_relays(app);
        }
        app.relays.edit_relays.clone()
    };

    let id_source: Id = "MyRelaysScroll".into();

    super::relay_scroll_list(app, ui, relays, id_source);
}

fn get_relays(app: &mut GossipUi) -> Vec<Relay> {
    let mut relays: Vec<Relay> = GLOBALS
        .storage
        .filter_relays(|relay| relay.has_any_usage_bit() && super::filter_relay(&app.relays, relay))
        .unwrap_or_default();

    relays.sort_by(|a, b| super::sort_relay(&app.relays, a, b));
    relays
}
