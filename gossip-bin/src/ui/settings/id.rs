use crate::ui::{GossipUi, Page};
use eframe::egui;
use egui::widgets::Slider;
use egui::{Context, Ui};
use gossip_lib::GLOBALS;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.heading("Identity Settings");
    ui.add_space(20.0);

    // public_key
    ui.horizontal(|ui| {
        ui.label("Public Key:");
        if let Some(pk) = GLOBALS.identity.public_key() {
            ui.label(pk.as_bech32_string());
        } else {
            ui.label("NOT SET");
        }
    });
    ui.horizontal(|ui| {
        ui.label("Manage your public key identity on the");
        if ui.link("Account > Keys").clicked() {
            app.set_page(ctx, Page::YourKeys);
        }
        ui.label("page.");
    });

    // log_n
    ui.add_space(20.0);
    ui.label("Encrypted Private Key scrypt N parameter");
    ui.label("(NOTE: changing this will not re-encrypt any existing encrypted private key)");
    ui.horizontal(|ui| {
        ui.add(Slider::new(&mut app.unsaved_settings.log_n, 18..=22).text("logN iterations"));
        reset_button!(app, ui, log_n);
    });

    // Login at startup
    ui.add_space(20.0);

    ui.horizontal(|ui| {
        ui.checkbox(
            &mut app.unsaved_settings.login_at_startup,
            "Login at startup",
        )
        .on_hover_text("If set, you will be prompted for your password before gossip starts up.");
        reset_button!(app, ui, login_at_startup);
    });

    ui.add_space(20.0);
}
