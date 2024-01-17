use crate::ui::wizard::WizardPage;
use crate::ui::{GossipUi, Page};
use eframe::egui;
use egui::{Context, RichText, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::GLOBALS;
use zeroize::Zeroize;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    // If already imported, advance
    if app.wizard_state.has_private_key {
        app.set_page(ctx, Page::Wizard(WizardPage::ReadNostrConfig));
    }

    ui.add_space(20.0);

    ui.horizontal_wrapped(|ui| {
        ui.label("Enter your private key");
        let response = text_edit_line!(app, app.import_priv)
            .hint_text("nsec1, hex, or ncryptsec1")
            .desired_width(f32::INFINITY)
            .password(true)
            .with_paste()
            .show_extended(ui, &mut app.clipboard)
            .response;
        if response.changed() {
            app.wizard_state.error = None;
        };
    });

    if app.import_priv.is_empty() {
        ui.add_space(10.0);
        ui.label(
            RichText::new("Please enter your key.").color(app.theme.warning_marker_text_color()),
        );
    }

    let ncryptsec = app.import_priv.starts_with("ncryptsec1");

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        if ncryptsec {
            ui.label("Enter passphrase to decrypt the encrypted private key");
        } else {
            ui.label("Enter a passphrase to keep it encrypted under");
        }
        let response = text_edit_line!(app, app.password)
            .password(true)
            .with_paste()
            .show_extended(ui, &mut app.clipboard)
            .response;
        if response.changed() {
            app.wizard_state.error = None;
        }
    });

    if !ncryptsec {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label("Repeat passphrase to be sure");
            let response = text_edit_line!(app, app.password2)
                .password(true)
                .with_paste()
                .show_extended(ui, &mut app.clipboard)
                .response;
            if response.changed() {
                app.wizard_state.error = None;
            }
        });
    }

    let password_mismatch = !ncryptsec && (app.password != app.password2);

    if password_mismatch {
        ui.add_space(10.0);
        ui.label(
            RichText::new("Passwords do not match.").color(app.theme.warning_marker_text_color()),
        );
    }

    // error block
    if let Some(err) = &app.wizard_state.error {
        ui.add_space(10.0);
        ui.label(RichText::new(err).color(app.theme.warning_marker_text_color()));
    }

    let ready = !app.import_priv.is_empty() && !password_mismatch;

    if ready {
        if app.password.is_empty() {
            ui.add_space(10.0);

            ui.label(
                RichText::new("Your password is empty!")
                    .color(app.theme.warning_marker_text_color()),
            );
        }

        ui.add_space(20.0);

        if ui
            .button(RichText::new("  >  Import").color(app.theme.accent_color()))
            .clicked()
        {
            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::ImportPriv {
                privkey: app.import_priv.clone(),
                password: app.password.clone(),
            });
            app.import_priv.zeroize();
            app.import_priv = "".to_owned();
            app.password.zeroize();
            app.password = "".to_owned();
            app.password2.zeroize();
            app.password2 = "".to_owned();
        }
    }

    ui.add_space(20.0);
    if ui.button("  <  Go Back").clicked() {
        app.set_page(ctx, Page::Wizard(WizardPage::ImportKeys));
    }
}
