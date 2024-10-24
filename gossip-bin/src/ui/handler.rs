use core::f32;

use super::{widgets, GossipUi};
use eframe::egui::{self, vec2, RichText};
use egui::{Context, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::{Handler, HandlerKey, HandlersTable, Table, GLOBALS};
use nostr_types::{EventKind, NAddr, PublicKey};

pub struct Handlers {
    /// Handler that is open for detailed view, if any
    detail: Option<PublicKey>,
    /// Is the add-handler dialog open?
    add_dialog: bool,
    /// Entry text for add-dialog
    add_naddr: String,
    /// Any errors while entering naddr
    add_err: Option<String>,
}

impl Default for Handlers {
    fn default() -> Self {
        Self {
            detail: None,
            add_dialog: false,
            add_naddr: "".to_owned(),
            add_err: None,
        }
    }
}

fn add_dialog(ui: &mut Ui, app: &mut GossipUi) {
    const DLG_SIZE: egui::Vec2 = vec2(400.0, 260.0);
    let dlg_response = widgets::modal_popup(ui.ctx(), vec2(400.0, 0.0), DLG_SIZE, true, |ui| {
        ui.heading("Import a handler via nevent");
        ui.add_space(8.0);

        ui.label("To add a new handler, paste its corresponding naddr here:");
        ui.add_space(12.0);

        let response = widgets::TextEdit::singleline(&app.theme, &mut app.handlers.add_naddr)
            .desired_width(f32::INFINITY)
            .hint_text("naddr1...")
            .with_paste()
            .show(ui);
        let mut go: bool = false;

        ui.add_space(12.0);

        ui.horizontal(|ui| {
            if let Some(err) = &app.handlers.add_err {
                ui.label(RichText::new(err).color(app.theme.warning_marker_text_color()));
            } else {
                ui.label("");
            }

            ui.with_layout(egui::Layout::right_to_left(Default::default()), |ui| {
                if widgets::Button::primary(&app.theme, "Import")
                    .show(ui)
                    .clicked()
                {
                    go = true;
                }
            });
        });
        if response.response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            go = true;
        }
        if go {
            if app.handlers.add_naddr.starts_with("nostr:") {
                app.handlers.add_naddr = app.handlers.add_naddr[6..].to_owned();
            }

            match NAddr::try_from_bech32_string(&app.handlers.add_naddr) {
                Ok(naddr) => {
                    let _ = GLOBALS
                        .to_overlord
                        .send(ToOverlordMessage::FetchNAddr(naddr));
                    app.handlers.add_naddr = "".to_string();
                    app.handlers.add_dialog = false;
                    app.handlers.add_err = None;
                }
                Err(_) => {
                    app.handlers.add_err = Some("Invalid naddr".to_owned());
                }
            }
        }
    });

    if dlg_response.inner.clicked() {
        app.handlers.add_dialog = false;
        app.handlers.add_err = None;
    }
}

pub(super) fn update_all_kinds(app: &mut GossipUi, ctx: &Context, ui: &mut Ui) {
    // If we end up in this overview, we clear any detail view
    app.handlers.detail.take();

    if app.handlers.add_dialog {
        add_dialog(ui, app);
    }

    // render page
    widgets::page_header(ui, "External Event Handlers", |ui| {
        if widgets::Button::primary(&app.theme, "Import Handler")
            .show(ui)
            .clicked()
        {
            app.handlers.add_dialog = true;
        }
    });

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

                    let kind_name = format!("Kind {}", u32::from(*kind));

                    let handlers: Vec<(HandlerKey, bool, bool)> = GLOBALS
                        .db()
                        .read_configured_handlers(*kind)
                        .unwrap_or(vec![]);

                    let all_count = handlers.len();
                    let enabled_count = handlers.iter().filter_map(|f| f.1.then(|| {})).count();

                    ui.horizontal(|ui| {
                        let kwidth = ui.label(egui::RichText::new(&kind_name)).rect.width();
                        ui.add_space(200.0 - kwidth);
                        ui.label(egui::RichText::new(format!("{}", kind)));

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::default()), |ui| {
                            if all_count == 0 {
                                ui.label(egui::RichText::new("No recommendations").weak());
                            } else {
                                if enabled_count == 0 {
                                    ui.label(
                                        egui::RichText::new(format!("zero of {} apps", all_count))
                                            .weak(),
                                    );
                                } else if enabled_count != all_count {
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
        format!("Handler: {} ({})", kind, u32::from(kind)),
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

                let response = widgets::list_entry::clickable_frame(
                    ui,
                    app,
                    Some(app.theme.main_content_bgcolor()),
                    |ui, app| {
                        ui.set_min_width(ui.available_width());

                        ui.push_id(&name, |ui| {
                            handler_header(
                                ui,
                                app,
                                &handler,
                                &name,
                                kind,
                                &mut enabled,
                                &mut recommended,
                            );

                            if app.handlers.detail == Some(handler.key.pubkey) {
                                handler_detail(ui, app, &handler, kind);
                            }

                            ui.interact_bg(egui::Sense::click())
                        })
                        .inner
                    },
                );

                if response
                    .inner
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    if app.handlers.detail == Some(handler.key.pubkey) {
                        app.handlers.detail.take();
                    } else {
                        app.handlers.detail = Some(handler.key.pubkey);
                    }
                }
            }
        }
    });
}

fn handler_header(
    ui: &mut Ui,
    app: &mut GossipUi,
    handler: &Handler,
    name: &String,
    kind: EventKind,
    enabled: &mut bool,
    recommended: &mut bool,
) {
    ui.horizontal(|ui| {
        if widgets::Switch::small(&app.theme, enabled)
            .show(ui)
            .changed()
        {
            let _ = GLOBALS.db().write_configured_handler(
                kind,
                handler.key.clone(),
                *enabled,
                *recommended,
                None,
            );
        }

        ui.add_space(10.0);
        let lresp = ui.link(name).on_hover_text("go to profile");
        if lresp.clicked() {
            app.set_page(ui.ctx(), super::Page::Person(handler.key.pubkey));
        }
        let lwidth = lresp.rect.width();

        ui.add_space(200.0 - lwidth);
        if let Some(metadata) = handler.metadata() {
            if let Some(value) = metadata.other.get("website") {
                match value {
                    serde_json::Value::String(url) => {
                        if ui
                            .link(url.to_string())
                            .on_hover_text("open website in browser")
                            .clicked()
                        {
                            ui.output_mut(|o| {
                                o.open_url = Some(egui::OpenUrl {
                                    url: url.to_string(),
                                    new_tab: true,
                                });
                            });
                        }
                    }
                    _ => {}
                }
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::default()), |ui| {
            if widgets::Switch::small(&app.theme, recommended)
                .with_label("recommend")
                .show(ui)
                .changed()
            {
                let _ = GLOBALS.db().write_configured_handler(
                    kind,
                    handler.key.clone(),
                    *enabled,
                    *recommended,
                    None,
                );
            }
        });
    });
}

fn handler_detail(ui: &mut Ui, app: &mut GossipUi, handler: &Handler, kind: EventKind) {
    if let Some(metadata) = handler.metadata() {
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "About: {}",
                metadata.about.as_deref().unwrap_or("".into())
            ));
        });
    }

    let recommended_by: Vec<PublicKey> = GLOBALS
        .db()
        .who_recommended_handler(&handler.key, kind)
        .unwrap_or(vec![]);
    ui.horizontal(|ui| {
        let count = recommended_by.len();
        if count > 0 {
            ui.label("Recommended by: ");
            for (i, pubkey) in recommended_by.iter().enumerate() {
                let name = gossip_lib::names::best_name_from_pubkey_lookup(pubkey);
                if ui.link(name).clicked() {
                    app.set_page(ui.ctx(), super::Page::Person(pubkey.to_owned()));
                }
                if (i + 1) < count {
                    ui.label("|");
                }
            }
        }
    });
}
