use crate::comms::ToOverlordMessage;
use crate::globals::GLOBALS;
use crate::ui::wizard::WizardPage;
use crate::ui::{GossipUi, Page};
use eframe::egui;
use egui::{Context, Ui};
use zeroize::Zeroize;

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    // If already imported, advance
    if app.wizard_state.has_private_key {
        app.page = Page::Wizard(WizardPage::ReadNostrConfig);
    }

    ui.add_space(20.0);

    ui.horizontal_wrapped(|ui| {
        ui.label("Enter your private key");
        ui.add(
            text_edit_line!(app, app.import_priv)
                .hint_text("nsec1 or hex")
                .desired_width(f32::INFINITY)
                .password(true),
        );
    });

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.label("Enter a passphrase to keep it encrypted under");
        ui.add(text_edit_line!(app, app.password).password(true));
    });

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.label("Repeat passphrase to be sure");
        ui.add(text_edit_line!(app, app.password2).password(true));
    });

    ui.add_space(10.0);
    if ui.button("  >  Import").clicked() {
        if app.password != app.password2 {
            GLOBALS
                .status_queue
                .write()
                .write("Passwords do not match".to_owned());
        } else {
            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::ImportPriv(
                app.import_priv.clone(),
                app.password.clone(),
            ));
            app.import_priv.zeroize();
            app.import_priv = "".to_owned();
        }
        app.password.zeroize();
        app.password = "".to_owned();
        app.password2.zeroize();
        app.password2 = "".to_owned();
    }

    ui.add_space(20.0);
    if ui.button("  <  Go Back").clicked() {
        app.page = Page::Wizard(WizardPage::ImportKeys);
    }
}
