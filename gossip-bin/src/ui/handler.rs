use core::f32;

use super::{widgets, GossipUi};
use eframe::egui::{self, Widget, WidgetText};
use egui::{Context, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::{HandlerKey, HandlersTable, Table, GLOBALS};
use nostr_types::{EventKind, NAddr};

pub(super) fn update_all_kinds(app: &mut GossipUi, _ctx: &Context, ui: &mut Ui) {
    widgets::page_header(ui, "External Event Handlers", |_ui| ());

    let panel_height = ui.available_height();

    ui.label("Import a handler via nevent");
    let response = ui.add(text_edit_line!(app, app.handler_naddr).hint_text("naddr1..."));
    let mut go: bool = false;
    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
        go = true;
    }
    if ui.button("Import").clicked() {
        go = true;
    }
    if go {
        if app.handler_naddr.starts_with("nostr:") {
            app.handler_naddr = app.handler_naddr[6..].to_owned();
        }

        match NAddr::try_from_bech32_string(&app.handler_naddr) {
            Ok(naddr) => {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::FetchNAddr(naddr));
            }
            Err(_) => {
                GLOBALS
                    .status_queue
                    .write()
                    .write("Invalid naddr".to_string());
            }
        }
        app.handler_naddr = "".to_string();
    }

    app.vert_scroll_area().show(ui, |ui| {
        let data = GLOBALS
            .db()
            .read_all_configured_handlers()
            .unwrap_or(vec![]);
        let mut kinds: Vec<EventKind> = data.iter().map(|(k, _, _, _)| *k).collect();
        kinds.dedup();

        for kind in kinds.iter() {
            widgets::list_entry::clickable_frame(
                ui,
                app,
                Some(app.theme.main_content_bgcolor()),
                |ui, app| {
                    ui.set_min_width(ui.available_width());
                    ui.set_height(37.0);

                    let kind_name = format!("{}", kind);
                    let id = egui::Id::new(u32::from(*kind).to_string());

                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(&kind_name).heading());
                        ui.add_space(10.0);
                        ui.label(egui::RichText::new(u32::from(*kind).to_string()).heading());
                    });

                    egui::CollapsingHeader::new("Configure Handlers")
                        .id_source(id)
                        .show_unindented(ui, |ui| {
                            let handlers: Vec<(HandlerKey, bool, bool)> = GLOBALS
                                .db()
                                .read_configured_handlers(*kind)
                                .unwrap_or(vec![]);

                            if ui.button("Share recommendations").clicked() {
                                let _ = GLOBALS
                                    .to_overlord
                                    .send(ToOverlordMessage::ShareHandlerRecommendations(*kind));
                            }

                            app.vert_scroll_area()
                                .id_source(id.with("s"))
                                .max_width(f32::INFINITY)
                                .max_height(panel_height * 0.66)
                                .show(ui, |ui| {
                                    for (key, mut enabled, mut recommended) in handlers.iter() {
                                        if let Ok(Some(handler)) =
                                            HandlersTable::read_record(key.clone(), None)
                                        {
                                            if !kind.is_parameterized_replaceable()
                                                && handler.nevent_url.is_none()
                                            {
                                                continue;
                                            }
                                            if kind.is_parameterized_replaceable()
                                                && handler.naddr_url.is_none()
                                            {
                                                continue;
                                            }
                                            let name = match handler.bestname(*kind) {
                                                Some(n) => n,
                                                None => continue,
                                            };

                                            ui.with_layout(
                                                egui::Layout::left_to_right(egui::Align::TOP),
                                                |ui| {
                                                    ui.label(name);
                                                    if widgets::Switch::small(
                                                        &app.theme,
                                                        &mut enabled,
                                                    )
                                                    .with_label("enable")
                                                    .show(ui)
                                                    .changed()
                                                    {
                                                        let _ =
                                                            GLOBALS.db().write_configured_handler(
                                                                *kind,
                                                                key.clone(),
                                                                enabled,
                                                                recommended,
                                                                None,
                                                            );
                                                    }
                                                    if widgets::Switch::small(
                                                        &app.theme,
                                                        &mut recommended,
                                                    )
                                                    .with_label("recommend")
                                                    .show(ui)
                                                    .changed()
                                                    {
                                                        let _ =
                                                            GLOBALS.db().write_configured_handler(
                                                                *kind,
                                                                key.clone(),
                                                                enabled,
                                                                recommended,
                                                                None,
                                                            );
                                                    }
                                                },
                                            );

                                            ui.add_space(7.0);
                                        }
                                    }
                                });
                        });
                },
            );
        }
    });
}

pub(super) fn update_kind(_app: &mut GossipUi, _ctx: &Context, ui: &mut Ui, kind: EventKind) {}
