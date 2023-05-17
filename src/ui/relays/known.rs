use super::{filter_relay, relay_filter_combo, relay_sort_combo, GossipUi};
use crate::db::DbRelay;
use crate::globals::GLOBALS;
use crate::ui::widgets;
use eframe::egui;
use egui::{Align, Context, Layout, Ui};

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.add_space(10.0);
    ui.horizontal_wrapped(|ui| {
        ui.heading("Known Relays");
        ui.add_space(50.0);
        widgets::search_filter_field(ui, &mut app.relay_ui.search, 200.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            ui.add_space(20.0);
            relay_filter_combo(app, ui, "KnownRelaysFilterCombo".into());
            ui.add_space(20.0);
            relay_sort_combo(app, ui, "KnownRelaysSortCombo".into());
        });
    });
    ui.add_space(10.0);

    // ui.horizontal(|ui| {
    //     ui.label("Enter a new relay URL:");
    //     ui.add(text_edit_line!(app, app.new_relay_url));
    //     if ui.button("Add").clicked() {
    //         if let Ok(url) = RelayUrl::try_from_str(&app.new_relay_url) {
    //             let _ = GLOBALS.to_overlord.send(ToOverlordMessage::AddRelay(url));
    //             *GLOBALS.status_message.blocking_write() = format!(
    //                 "I asked the overlord to add relay {}. Check for it below.",
    //                 &app.new_relay_url
    //             );
    //             app.new_relay_url = "".to_owned();
    //         } else {
    //             *GLOBALS.status_message.blocking_write() =
    //                 "That's not a valid relay URL.".to_owned();
    //         }
    //     }
    //     ui.separator();
    //     if ui.button("↑ Advertise Relay List ↑").clicked() {
    //         let _ = GLOBALS
    //             .to_overlord
    //             .send(ToOverlordMessage::AdvertiseRelayList);
    //     }
    //     ui.checkbox(&mut app.show_hidden_relays, "Show hidden relays");
    // });

    // TBD time how long this takes. We don't want expensive code in the UI
    // FIXME keep more relay info and display it
    let mut relays: Vec<DbRelay> = GLOBALS
        .all_relays
        .iter()
        .map(|ri| ri.value().clone())
        .filter(|ri| app.show_hidden_relays || !ri.hidden && filter_relay(&app.relay_ui, ri))
        .collect();

    relays.sort_by(|a, b| {
        super::sort_relay(&app.relay_ui, a, b)
    });

    ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
        ui.with_layout(Layout::top_down(Align::Min), |ui| {
            egui::ScrollArea::vertical()
                .id_source("knownrelays")
                .override_scroll_delta(egui::Vec2 {
                    x: 0.0,
                    y: app.current_scroll_offset * 2.0, // double speed
                })
                .show(ui, |ui| {
                    for relay in relays {
                        let mut widget =
                            widgets::RelayEntry::new(&relay)
                                .accent(app.settings.theme.accent_color())
                                .option_symbol(&app.options_symbol);
                        if let Some(ref assignment) =
                            GLOBALS.relay_picker.get_relay_assignment(&relay.url)
                        {
                            widget = widget.user_count(assignment.pubkeys.len());
                        }
                        ui.add(widget);
                    }
                });
        });
    });
}
