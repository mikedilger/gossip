use crate::ui::{GossipUi, Page};
use eframe::egui;
use egui::widgets::Slider;
use egui::{Context, Ui};

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.heading("Identity Settings");
    ui.add_space(20.0);

    // public_key
    ui.horizontal(|ui| {
        ui.label("Public Key:");
        if let Some(pk) = app.settings.public_key {
            ui.label(pk.as_bech32_string());
        } else {
            ui.label("NOT SET");
        }
    });
    ui.horizontal(|ui| {
        ui.label("Manage your public key identity on the");
        if ui.link("Account > Keys").clicked() {
            app.set_page(Page::YourKeys);
        }
        ui.label("page.");
    });

    // log_n
    ui.add_space(20.0);
    ui.label("Encrypted Private Key scrypt N parameter");
    ui.label("(NOTE: changing this will not re-encrypt any existing encrypted private key)");
    ui.add(Slider::new(&mut app.settings.log_n, 18..=22).text("logN iteratons"));

    ui.add_space(20.0);
}
