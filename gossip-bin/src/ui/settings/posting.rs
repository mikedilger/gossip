use crate::ui::GossipUi;
use eframe::egui;
use egui::widgets::Slider;
use egui::{Context, Ui};

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.heading("Posting Settings");
    ui.add_space(10.0);

    ui.horizontal(|ui| {
        ui.label("Proof of Work: ")
            .on_hover_text("The larger the number, the longer it takes.");
        ui.add(Slider::new(&mut app.unsaved_settings.pow, 0..=40).text("leading zero bits"));
    });

    ui.checkbox(
        &mut app.unsaved_settings.set_client_tag,
        "Add tag [\"client\",\"gossip\"] to posts",
    )
    .on_hover_text("Takes effect immediately.");

    ui.checkbox(
        &mut app.unsaved_settings.set_user_agent,
        &format!(
            "Send User-Agent Header to Relays: gossip/{}",
            app.about.version
        ),
    )
    .on_hover_text("Takes effect on next relay connection.");

    ui.add_space(20.0);
}
