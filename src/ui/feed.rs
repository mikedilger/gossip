use super::GossipUi;
use eframe::egui;
use egui::{Context, ScrollArea, TextStyle, Ui};
use tracing::info;

pub(super) fn update(_app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {

    let feed = crate::globals::blocking_get_feed();

    let screen_rect = ctx.input().screen_rect; // Rect
    let scroll_delta = ctx.input().scroll_delta; // Vec2


    ScrollArea::vertical().show(ui, |ui| {
        for id in feed.iter().rev() {
            // Stop rendering at the bottom of the window:
            let pos2 = ui.next_widget_position();
            if pos2.y > screen_rect.max.y { break; }

            if let Some(event) = crate::globals::GLOBALS.events.blocking_lock().get(id) {

                ui.label(crate::date_ago::date_ago(event.created_at));

                if let Some(person) = crate::globals::GLOBALS.people.blocking_lock().get(&event.pubkey) {
                    if let Some(name) = &person.name {
                        ui.label(&format!("{}", name));
                    } else {
                        ui.label(&format!("{}", event.pubkey.as_hex_string()));
                    }
                } else {
                    ui.label(&format!("{}", event.pubkey.as_hex_string()));
                }
                ui.label(&format!("{}", event.content));
                ui.separator();
            } else {
                ui.label(&format!("-- missing event --"));
                ui.separator();
            }
        }
    });
}
