use super::{GossipUi, Page};
use eframe::egui;
use egui::{Context, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::{HandlerKey, HandlersTable, Table, GLOBALS};
use nostr_types::{EventKind, NAddr};

pub(super) fn update_all_kinds(app: &mut GossipUi, _ctx: &Context, ui: &mut Ui) {
    ui.heading("External Event Handlers");
    ui.add_space(10.0);

    let data = GLOBALS
        .db()
        .read_all_configured_handlers()
        .unwrap_or(vec![]);
    let mut kinds: Vec<EventKind> = data.iter().map(|(k, _, _, _)| *k).collect();
    kinds.dedup();

    for kind in kinds.iter() {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
            if ui.button("Manage").clicked() {
                app.set_page(ui.ctx(), Page::Handlers(*kind));
            }
            ui.label(format!("{} {:?}", u32::from(*kind), kind));
        });
    }

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

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
}

pub(super) fn update_kind(_app: &mut GossipUi, _ctx: &Context, ui: &mut Ui, kind: EventKind) {
    ui.heading(format!(
        "External Handlers for kind={} {:?}",
        u32::from(kind),
        kind
    ));

    let handlers: Vec<(HandlerKey, bool, bool)> = GLOBALS
        .db()
        .read_configured_handlers(kind)
        .unwrap_or(vec![]);
    for (key, mut enabled, mut recommended) in handlers.iter() {
        if let Ok(Some(handler)) = HandlersTable::read_record(key.clone(), None) {
            if !kind.is_parameterized_replaceable() && handler.nevent_url.is_none() {
                continue;
            }
            if kind.is_parameterized_replaceable() && handler.naddr_url.is_none() {
                continue;
            }

            ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                ui.label(handler.name());
                if ui.checkbox(&mut enabled, "enable").changed() {
                    let _ = GLOBALS.db().write_configured_handler(
                        kind,
                        key.clone(),
                        enabled,
                        recommended,
                        None,
                    );
                }
                if ui.checkbox(&mut recommended, "recommend").changed() {
                    let _ = GLOBALS.db().write_configured_handler(
                        kind,
                        key.clone(),
                        enabled,
                        recommended,
                        None,
                    );
                }
            });
        }
    }

    if ui.button("Share recommendations").clicked() {
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::ShareHandlerRecommendations(kind));
    }
}
