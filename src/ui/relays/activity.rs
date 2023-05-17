use std::collections::HashSet;

use super::GossipUi;
use crate::db::DbRelay;
use crate::globals::GLOBALS;
use crate::ui::widgets;
use crate::{comms::ToOverlordMessage, ui::widgets::NavItem};
use eframe::egui;
use egui::{Context, Ui};
use egui_winit::egui::{vec2, Rect, Sense};
use nostr_types::RelayUrl;

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.add_space(10.0);

    ui.horizontal_wrapped(|ui| {
        ui.heading("Activity Monitor");
        ui.add_space(50.0);

        widgets::search_filter_field(ui, &mut app.relay_ui.search);

        // let sort_combo = egui::ComboBox::from_id_source("relayactivitymonitorsortcombo");
        // sort_combo
        //     .selected_text(  )
        //     .show_ui(ui, |ui| {
        //         // for theme_variant in ThemeVariant::all() {
        //         //     if ui.add(egui::widgets::SelectableLabel::new(*theme_variant == app.settings.theme.variant, theme_variant.name())).clicked() {

        //         //     };
        //         // }
        //     });

        // let filter_combo = egui::ComboBox::from_id_source("relayactivitymonitorfiltercombo");
        // filter_combo
        //     .selected_text( app.settings.theme.name() )
        //     .show_ui(ui, |ui| {
        //         // for theme_variant in ThemeVariant::all() {
        //         //     if ui.add(egui::widgets::SelectableLabel::new(*theme_variant == app.settings.theme.variant, theme_variant.name())).clicked() {

        //         //     };
        //         // }
        //     });
    });

    ui.add_space(10.0);

    let connected_relays: HashSet<RelayUrl> = GLOBALS
        .connected_relays
        .iter()
        .map(|r| r.key().clone())
        .collect();

    let mut relays: Vec<DbRelay> = GLOBALS
        .all_relays
        .iter()
        .map(|ri| ri.value().clone())
        .filter(|ri| {
            connected_relays.contains(&ri.url) && {
                if app.relay_ui.search.len() > 1 {
                    ri.url
                        .as_str()
                        .to_lowercase()
                        .contains(&app.relay_ui.search.to_lowercase())
                } else {
                    true
                }
            }
        })
        .collect();

    relays.sort_by(|a, b| {
        b.has_usage_bits(DbRelay::WRITE)
            .cmp(&a.has_usage_bits(DbRelay::WRITE))
            .then(a.url.cmp(&b.url))
    });

    egui::ScrollArea::vertical()
        .id_source("relay_coverage")
        .override_scroll_delta(egui::Vec2 {
            x: 0.0,
            y: app.current_scroll_offset,
        })
        .show(ui, |ui| {
            for relay in relays {
                let mut widget = widgets::RelayEntry::new(&relay);
                if let Some(ref assignment) = GLOBALS.relay_picker.get_relay_assignment(&relay.url)
                {
                    widget = widget.user_count(assignment.pubkeys.len());
                }
                ui.add(widget);
            }

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            if ui.button("Pick Again").clicked() {
                let _ = GLOBALS.to_overlord.send(ToOverlordMessage::PickRelays);
            }

            ui.add_space(12.0);
            ui.heading("Coverage");

            if GLOBALS.relay_picker.pubkey_counts_iter().count() > 0 {
                for elem in GLOBALS.relay_picker.pubkey_counts_iter() {
                    let pk = elem.key();
                    let count = elem.value();
                    let name = GossipUi::display_name_from_pubkeyhex_lookup(pk);
                    ui.label(format!("{}: coverage short by {} relay(s)", name, count));
                }
            } else {
                ui.label("All followed people are fully covered.".to_owned());
            }
        });
}
