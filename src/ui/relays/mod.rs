use super::{GossipUi, Page, components};
use crate::{comms::ToOverlordMessage, db::DbRelay};
use crate::globals::GLOBALS;
use eframe::egui;
use eframe::epaint::ahash::HashSet;
use egui::{Context, ScrollArea, Ui, Vec2};
use egui_extras::{Column, TableBuilder};
use nostr_types::RelayUrl;

mod all;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, frame: &mut eframe::Frame, ui: &mut Ui) {
    #[cfg(not(feature = "side-menu"))]
    {
        ui.horizontal(|ui| {
            if ui
                .add(egui::SelectableLabel::new(
                    app.page == Page::RelaysLive,
                    "Live",
                ))
                .clicked()
            {
                app.set_page(Page::RelaysLive);
            }
            ui.separator();
            if ui
                .add(egui::SelectableLabel::new(
                    app.page == Page::RelaysAll,
                    "Configure",
                ))
                .clicked()
            {
                app.set_page(Page::RelaysAll);
            }
            ui.separator();
        });
        ui.separator();
    }

    if app.page == Page::RelaysLive {
        ui.add_space(10.0);

        ui.heading("Connected Relays");
        ui.add_space(18.0);

        let connected_relays: HashSet<RelayUrl> = GLOBALS
            .connected_relays
            .iter()
            .map(|r| {
                    r.key().clone()
            })
            .collect();

        let mut relays: Vec<DbRelay> = GLOBALS
                .all_relays
                .iter()
                .map(|ri| ri.value().clone())
                .filter(|ri| connected_relays.contains(&ri.url))
                .collect();

        relays.sort_by(|a, b| {
            b.has_usage_bits(DbRelay::WRITE)
                .cmp(&a.has_usage_bits(DbRelay::WRITE))
                .then(a.url.cmp(&b.url))
        });

        ScrollArea::vertical()
            .id_source("relay_coverage")
            .override_scroll_delta(Vec2 {
                x: 0.0,
                y: app.current_scroll_offset,
            })
            .show(ui, |ui| {
                for relay in relays {
                    let mut widget = components::RelayEntry::new(&relay);
                    if let Some(ref assignment) =
                        GLOBALS.relay_picker.get_relay_assignment(&relay.url)
                    {
                        widget = widget.user_count(assignment.pubkeys.len());
                    }
                    ui.add( widget );
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
    } else if app.page == Page::RelaysAll {
        all::update(app, ctx, frame, ui);
    }
}
