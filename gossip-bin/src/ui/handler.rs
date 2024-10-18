use super::{widgets, GossipUi};
use eframe::egui::{self};
use egui::{Context, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::{HandlerKey, HandlersTable, Table, GLOBALS};
use nostr_types::{EventKind, NAddr};

pub(super) fn update_all_kinds(app: &mut GossipUi, ctx: &Context, ui: &mut Ui) {
    widgets::page_header(ui, "External Event Handlers", |_ui| ());

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
            let response = widgets::list_entry::clickable_frame(
                ui,
                app,
                Some(app.theme.main_content_bgcolor()),
                |ui, app| {
                    ui.set_min_width(ui.available_width());

                    let kind_name = format!("Kind {:?}", u32::from(*kind));

                    let handlers: Vec<(HandlerKey, bool, bool)> = GLOBALS
                        .db()
                        .read_configured_handlers(*kind)
                        .unwrap_or(vec![]);

                    let all_count = handlers.len();
                    let enabled_count = handlers.iter().filter_map(|f| f.1.then(|| {})).count();
                    let recommended_count = handlers.iter().filter_map(|f| f.2.then(|| {})).count();

                    ui.horizontal(|ui| {
                        let kwidth = ui.label(egui::RichText::new(&kind_name)).rect.width();
                        ui.add_space(200.0 - kwidth);
                        ui.label(egui::RichText::new(format!("{:?}", kind)));

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::default()), |ui| {
                            if all_count == 0 {
                                ui.label(egui::RichText::new("No recommendations").weak());
                            } else {
                                if recommended_count != all_count {
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "{} of {} apps",
                                            enabled_count, all_count
                                        ))
                                        .color(app.theme.accent_color()),
                                    );
                                } else {
                                    ui.label(
                                        egui::RichText::new(format!("{} apps", all_count))
                                            .color(app.theme.accent_color()),
                                    );
                                }
                            }
                        });
                    })
                },
            );

            if response
                .response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .interact(egui::Sense::click())
                .clicked()
            {
                app.set_page(ctx, super::Page::Handlers(*kind));
            }
        }
    });
}

pub(super) fn update_kind(app: &mut GossipUi, _ctx: &Context, ui: &mut Ui, kind: EventKind) {
    widgets::page_header(
        ui,
        format!("Handler: {:?} ({})", kind, u32::from(kind)),
        |ui| {
            if widgets::Button::secondary(&app.theme, "Share recommendations")
                .show(ui)
                .clicked()
            {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::ShareHandlerRecommendations(kind));
            }
        },
    );

    let handlers: Vec<(HandlerKey, bool, bool)> = GLOBALS
        .db()
        .read_configured_handlers(kind)
        .unwrap_or(vec![]);

    app.vert_scroll_area().show(ui, |ui| {
        for (key, mut enabled, mut recommended) in handlers.iter() {
            if let Ok(Some(handler)) = HandlersTable::read_record(key.clone(), None) {
                if !kind.is_parameterized_replaceable() && handler.nevent_url.is_none() {
                    continue;
                }
                if kind.is_parameterized_replaceable() && handler.naddr_url.is_none() {
                    continue;
                }

                let name = match handler.bestname(kind) {
                    Some(n) => n,
                    None => continue,
                };

                widgets::list_entry::clickable_frame(
                    ui,
                    app,
                    Some(app.theme.main_content_bgcolor()),
                    |ui, app| {
                        ui.set_min_width(ui.available_width());

                        ui.horizontal(|ui| {
                            if widgets::Switch::small(&app.theme, &mut enabled)
                                .show(ui)
                                .changed()
                            {
                                let _ = GLOBALS.db().write_configured_handler(
                                    kind,
                                    key.clone(),
                                    enabled,
                                    recommended,
                                    None,
                                );
                            }

                            ui.add_space(10.0);
                            let lwidth = ui.label(&name).rect.width();

                            ui.add_space(200.0 - lwidth);

                            if widgets::Switch::small(&app.theme, &mut recommended)
                                .with_label("recommend")
                                .show(ui)
                                .changed()
                            {
                                let _ = GLOBALS.db().write_configured_handler(
                                    kind,
                                    key.clone(),
                                    enabled,
                                    recommended,
                                    None,
                                );
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label("About: ");
                            if let Some(metadata) = handler.metadata() {
                                ui.label(metadata.about.as_deref().unwrap_or("".into()));
                            }
                        });

                        ui.horizontal(|ui| {
                            ui.label("Recommended by: <TODO>");
                        });
                    },
                );
            }
        }
    });
}
