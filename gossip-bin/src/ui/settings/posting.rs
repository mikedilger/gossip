use crate::ui::GossipUi;
use eframe::egui;
use egui::widgets::Slider;
use egui::{Context, TextEdit, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::GLOBALS;

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

    ui.horizontal(|ui| {
        ui.label("Blossom servers: ")
            .on_hover_text("Specify your blossom servers (just the host and port if it is not 443). Separate then by spaces or newlines");
        ui.add(
            TextEdit::multiline(
                &mut app.unsaved_settings.blossom_servers)
                .desired_width(f32::INFINITY)
        );
    });

    if ui.button("Publish Blossom Servers").clicked() {
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::PushBlossomServers);
    };

    ui.add_space(20.0);
}
