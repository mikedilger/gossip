use super::{filter_relay, relay_filter_combo, relay_sort_combo, GossipUi};
use crate::{db::DbRelay, ui::widgets::RelayEntry};
use crate::globals::GLOBALS;
use crate::ui::widgets;
use eframe::egui;
use egui::{Context, Ui};
use egui_winit::egui::{vec2, Id, ScrollArea, Sense};

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    let is_editing = app.relays.edit.is_some();
    ui.add_space(10.0);
    ui.horizontal_wrapped(|ui| {
        ui.heading("Known Relays");
        ui.add_space(50.0);
        ui.set_enabled(!is_editing);
        widgets::search_filter_field(ui, &mut app.relays.search, 200.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            ui.add_space(20.0);
            relay_filter_combo(app, ui);
            ui.add_space(20.0);
            relay_sort_combo(app, ui);
            ui.add_space(20.0);
            ui.checkbox(&mut app.show_hidden_relays, "Show hidden");
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
        .filter(|ri| app.show_hidden_relays || !ri.hidden && filter_relay(&app.relays, ri))
        .collect();

    relays.sort_by(|a, b| super::sort_relay(&app.relays, a, b));

    let scroll_size = ui.available_size_before_wrap();
    let id_source: Id = "KnowRelaysScroll".into();

    ScrollArea::vertical().id_source(id_source).show(ui, |ui| {
        let mut pos_last_entry = ui.cursor().left_top();
        let mut has_edit_target = false;

        for db_relay in relays {
            let db_url = db_relay.url.clone();
            let edit = if let Some(edit_url) = &app.relays.edit {
                if edit_url == &db_url {
                    has_edit_target = true;
                    true
                } else {
                    false
                }
            } else {
                false
            };
            let enabled = edit || !is_editing;
            let mut widget = RelayEntry::new(db_relay, app);
            widget.set_edit(edit);
            widget.set_enabled(enabled);
            if let Some(ref assignment) = GLOBALS.relay_picker.get_relay_assignment(&db_url) {
                widget.set_user_count(assignment.pubkeys.len());
            }
            let response = ui.add_enabled(enabled, widget.clone());
            if response.clicked() {
                if !edit {
                    app.relays.edit = Some(db_url);
                    response.scroll_to_me(Some(egui::Align::Center));
                    has_edit_target = true;
                } else {
                    app.relays.edit = None;
                }
            }
            pos_last_entry = response.rect.left_top();
        }

        if !has_edit_target {
            // the relay we wanted to edit was not in the list anymore
            // -> release edit modal
            app.relays.edit = None;
        }

        // add enough space to show the last relay entry at the top when editing
        if app.relays.edit.is_some() {
            let desired_size = scroll_size - vec2(0.0, ui.cursor().top() - pos_last_entry.y);
            ui.allocate_exact_size(desired_size, Sense::hover());
        }
    });
}
