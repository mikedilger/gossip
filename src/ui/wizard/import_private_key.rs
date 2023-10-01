use crate::comms::ToOverlordMessage;
use crate::globals::GLOBALS;
use crate::ui::wizard::WizardPage;
use crate::ui::{GossipUi, Page};
use eframe::egui;
use egui::{Context, RichText, Ui};
use zeroize::Zeroize;

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    // If already imported, advance
    if app.wizard_state.has_private_key {
        app.page = Page::Wizard(WizardPage::ReadNostrConfig);
    }

    ui.add_space(20.0);

    ui.horizontal_wrapped(|ui| {
        ui.label("Enter your private key");
        if ui
            .add(
                text_edit_line!(app, app.import_priv)
                    .hint_text("nsec1 or hex")
                    .desired_width(f32::INFINITY)
                    .password(true),
            )
            .changed()
        {
            app.wizard_state.error = None;
        };
    });

    let ncryptsec = app.import_priv.starts_with("ncryptsec1");

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        if ncryptsec {
            ui.label("Enter passphrase to decrypt the encrypted private key");
        } else {
            ui.label("Enter a passphrase to keep it encrypted under");
        }
        if ui
            .add(text_edit_line!(app, app.password).password(true))
            .changed()
        {
            app.wizard_state.error = None;
        }
    });

    if !ncryptsec {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label("Repeat passphrase to be sure");
            if ui
                .add(text_edit_line!(app, app.password2).password(true))
                .changed()
            {
                app.wizard_state.error = None;
            }
        });
    }

    // error block
    if let Some(err) = &app.wizard_state.error {
        ui.add_space(10.0);
        ui.label(RichText::new(err).color(app.settings.theme.warning_marker_text_color()));
    }

    let ready = !app.import_priv.is_empty()
        && !app.password.is_empty()
        && (ncryptsec || !app.password2.is_empty());

    if ready {
        ui.add_space(10.0);
        if ui
            .button(RichText::new("  >  Import").color(app.settings.theme.accent_color()))
            .clicked()
        {
            if !ncryptsec && app.password != app.password2 {
                app.wizard_state.error = Some("ERROR: Passwords do not match".to_owned());
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
    }

    ui.add_space(20.0);
    if ui.button("  <  Go Back").clicked() {
        app.page = Page::Wizard(WizardPage::ImportKeys);
    }
}
