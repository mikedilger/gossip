use crate::ui::wizard::WizardPage;
use crate::ui::{GossipUi, Page};
use eframe::egui;
use egui::{Context, RichText, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::GLOBALS;

use super::wizard_controls;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    // If identity is already configured, advance
    if GLOBALS.identity.public_key().is_some() {
        app.set_page(ctx, Page::Wizard(WizardPage::ReadNostrConfig));
        return;
    }

    ui.add_space(20.0);

    egui::Grid::new("signerurl")
        .num_columns(2)
        .striped(false)
        .spacing([10.0, 10.0])
        .show(ui, |ui| {
            ui.label("Enter the remote signer URL");
            // NOTE: there is a bug in our TextEdit widget that causes right-aligned
            //       elements on the page to slide off the right side of the window
            //       when the input content gets long.  So instead of using
            //       our text_edit_line!, I am working around it with the upstream's.
            let response = egui::TextEdit::singleline(&mut app.wizard_state.remote_signer_url)
                .show(ui)
                .response;
            if response.changed() {
                app.wizard_state.error = None;
            }
            ui.end_row();
            ui.label(""); // empty cell
        });

    let ready = !app.wizard_state.remote_signer_url.is_empty() && (app.password == app.password2);

    ui.add_space(20.0);
    egui::Grid::new("inputs")
        .num_columns(2)
        .striped(false)
        .spacing([10.0, 10.0])
        .show(ui, |ui| {
            ui.label("Enter new passphrase");
            let response = text_edit_line!(app, app.password)
                .password(true)
                .with_paste()
                .show(ui)
                .response;
            if response.changed() {
                app.wizard_state.error = None;
            }
            ui.end_row();
            ui.label("Repeat passphrase to be sure");

            let response = text_edit_line!(app, app.password2)
                .password(true)
                .with_paste()
                .show(ui)
                .response;
            if response.changed() {
                app.wizard_state.error = None;
            }
            ui.end_row();
            ui.label(""); // empty cell

            let text = if ready {
                if app.password.is_empty() {
                    "Your password is empty!"
                } else {
                    ""
                }
            } else {
                if app.wizard_state.remote_signer_url.is_empty() {
                    "Enter a URL"
                } else {
                    "Passwords do not match."
                }
            };
            ui.label(RichText::new(text).color(app.theme.warning_marker_text_color()));
            ui.label(""); // empty cell
            ui.end_row();
        });

    // error block
    if let Some(err) = &app.wizard_state.error {
        ui.add_space(10.0);
        ui.label(RichText::new(err).color(app.theme.warning_marker_text_color()));
    }

    wizard_controls(
        ui,
        app,
        true, // FIXME
        |app| {
            app.set_page(ctx, Page::Wizard(WizardPage::WelcomeGossip));
        },
        |app| {
            app.wizard_state.error = None;
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::SetupRemoteSigner(
                    app.wizard_state.remote_signer_url.clone(),
                    app.password.clone(),
                ));
            // Leave this incase they need to try again, and to avoid the "password is empty"
            // message.
            // FIXME: where to clear/zeroize?
            //app.password.zeroize();
            //app.password = "".to_owned();
            //app.password2.zeroize();
            //app.password2 = "".to_owned();
        },
    )
}
