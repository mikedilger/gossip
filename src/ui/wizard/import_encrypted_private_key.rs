use crate::comms::ToOverlordMessage;
use crate::globals::GLOBALS;
use crate::ui::wizard::WizardPage;
use crate::ui::{GossipUi, Page};
use eframe::egui;
use egui::{Context, Ui};
use zeroize::Zeroize;

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    // If already imported, advance
    if GLOBALS.signer.is_ready() {
        app.page = Page::Wizard(WizardPage::ReadNostrConfig);
    }

    ui.add_space(20.0);

    ui.horizontal_wrapped(|ui| {
        ui.label("Enter your encrypted private key");
        ui.add(
            text_edit_line!(app, app.import_priv)
                .hint_text("ncryptsec1")
                .desired_width(f32::INFINITY)
                .password(true),
        );
    });

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.label("Enter the passphrase it is encrypted under");
        ui.add(text_edit_line!(app, app.password).password(true));
    });

    ui.add_space(20.0);
    if ui.button("  >  Import").clicked() {
        let _ = GLOBALS.to_overlord.send(ToOverlordMessage::ImportPriv(
            app.import_priv.clone(),
            app.password.clone(),
        ));
        app.password.zeroize();
        app.password = "".to_owned();
    }

    ui.add_space(20.0);
    if ui.button("  <  Go Back").clicked() {
        app.page = Page::Wizard(WizardPage::ImportKeys);
    }
}
