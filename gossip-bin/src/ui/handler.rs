use super::{GossipUi, Page};
use eframe::egui;
use egui::{Context, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::{HandlerKey, HandlersTable, Table, GLOBALS};
use nostr_types::EventKind;

pub(super) fn update_all_kinds(app: &mut GossipUi, _ctx: &Context, ui: &mut Ui) {
    ui.heading("External Event Handlers");
    ui.add_space(10.0);

    let data = GLOBALS
        .db()
        .read_all_configured_handlers()
        .unwrap_or(vec![]);
    let mut kinds: Vec<EventKind> = data.iter().map(|(k, _, _)| *k).collect();
    kinds.dedup();

    for kind in kinds.iter() {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
            if ui.button("Manage").clicked() {
                app.set_page(ui.ctx(), Page::Handlers(*kind));
            }
            ui.label(format!("{} {:?}", u32::from(*kind), kind));
        });
    }
}

pub(super) fn update_kind(_app: &mut GossipUi, _ctx: &Context, ui: &mut Ui, kind: EventKind) {
    ui.heading(format!(
        "External Handlers for kind={} {:?}",
        u32::from(kind),
        kind
    ));

    let handlers: Vec<(HandlerKey, bool)> = GLOBALS
        .db()
        .read_configured_handlers(kind)
        .unwrap_or(vec![]);
    for (key, mut enabled) in handlers.iter() {
        if let Ok(Some(handler)) = HandlersTable::read_record(key.clone(), None) {
            if !kind.is_parameterized_replaceable() && handler.nevent_url.is_none() {
                continue;
            }
            if kind.is_parameterized_replaceable() && handler.naddr_url.is_none() {
                continue;
            }

            ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                if ui.checkbox(&mut enabled, handler.name()).changed() {
                    let _ = GLOBALS
                        .db()
                        .write_configured_handler(kind, key.clone(), enabled, None);
                }
                if ui.button("Share").clicked() {
                    let _ = GLOBALS
                        .to_overlord
                        .send(ToOverlordMessage::ShareHandler(kind, key.clone()));
                }
            });
        }
    }
}
