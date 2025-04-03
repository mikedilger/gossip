use crate::ui::GossipUi;
use egui_winit::egui::{self, Ui};
use gossip_lib::GLOBALS;
use nostr_types::Url;

fn draw_open_and_copy(ui: &mut Ui, url_string: String) {
    if ui.button("Open in browser").clicked() {
        let modifiers = ui.ctx().input(|i| i.modifiers);
        ui.ctx().output_mut(|o| {
            o.open_url = Some(egui::output::OpenUrl {
                url: url_string.clone(),
                new_tab: modifiers.any(),
            });
        });
        ui.close_menu();
        GLOBALS
            .status_queue
            .write()
            .write("Opening in browser...".to_owned());
    }

    if ui.button("Copy URL").clicked() {
        ui.output_mut(|o| o.copied_text = url_string.clone());
        ui.close_menu();
        GLOBALS
            .status_queue
            .write()
            .write("Link copied to clipboard!".to_owned());
    }
}

fn draw_reload_media(ui: &mut Ui, app: &mut GossipUi, url: Url) {
    let url_string = url.to_string();
    if let Some(error) = app.has_media_loading_failed(url_string.as_str()) {
        if ui
            .button("Retry loading ...")
            .on_hover_text(error)
            .clicked()
        {
            app.retry_media(&url);
        }
    }
}

pub(in crate::ui) fn show_link_context(ui: &mut Ui, app: &mut GossipUi, url_string: String) {
    draw_open_and_copy(ui, url_string);

    if app.is_scrolling() {
        ui.close_menu();
    }
}

pub(in crate::ui) fn show_media_link_context(ui: &mut Ui, app: &mut GossipUi, url: Url) {
    draw_open_and_copy(ui, url.to_string());
    draw_reload_media(ui, app, url);

    // This is at the end because if the menu is closed while empty,
    // the next time it opens, it will have a width that is too small.
    if app.is_scrolling() {
        ui.close_menu();
    }
}
