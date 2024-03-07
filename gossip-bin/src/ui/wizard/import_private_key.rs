use crate::ui::wizard::WizardPage;
use crate::ui::{GossipUi, Page};
use eframe::egui;
use egui::{Context, RichText, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::GLOBALS;
use zeroize::Zeroize;

use super::wizard_controls;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    // If already imported, advance
    if app.wizard_state.has_private_key {
        app.set_page(ctx, Page::Wizard(WizardPage::ReadNostrConfig));
    }

    ui.add_space(20.0);

    egui::Grid::new("keys")
        .num_columns(2)
        .striped(false)
        .spacing([10.0, 10.0])
        .show(ui, |ui| {
            ui.label("Enter your private key");
            let response = text_edit_line!(app, app.import_priv)
                .hint_text("nsec1, hex, or ncryptsec1")
                .desired_width(f32::INFINITY)
                .password(true)
                .with_paste()
                .show(ui)
                .response;
            if response.changed() {
                app.wizard_state.error = None;
            };
            ui.end_row();

            ui.label(""); // empty cell
            if app.import_priv.is_empty() {
                ui.label(
                    RichText::new("Please enter your key.")
                        .color(app.theme.warning_marker_text_color()),
                );
            }
            ui.end_row();
        });

    let ncryptsec = app.import_priv.starts_with("ncryptsec1");
    let password_mismatch = !ncryptsec && (app.password != app.password2);
    let ready = !app.import_priv.is_empty() && !password_mismatch;

    ui.add_space(20.0);
    egui::Grid::new("inputs")
        .num_columns(2)
        .striped(false)
        .spacing([10.0, 10.0])
        .show(ui, |ui| {
            if ncryptsec {
                ui.label("Enter passphrase to decrypt the encrypted private key");
            } else {
                ui.label("Enter a passphrase to keep it encrypted under");
            }
            let response = text_edit_line!(app, app.password)
                .password(true)
                .with_paste()
                .show(ui)
                .response;
            if response.changed() {
                app.wizard_state.error = None;
            }
            ui.end_row();

            if !ncryptsec {
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
            }

            ui.label(""); // empty cell
            let text = if ready {
                if app.password.is_empty() {
                    "Your password is empty!"
                } else {
                    ""
                }
            } else {
                "Passwords do not match."
            };
            ui.label(RichText::new(text).color(app.theme.warning_marker_text_color()));
            ui.end_row();
        });

    // error block
    if let Some(err) = &app.wizard_state.error {
        ui.add_space(10.0);
        ui.label(RichText::new(err).color(app.theme.warning_marker_text_color()));
        if ncryptsec {
            ui.label("Please check your ncryptsec and password again");
        }
    }

    wizard_controls(
        ui,
        app,
        ready,
        |app| {
            app.set_page(ctx, Page::Wizard(WizardPage::ImportKeys));
        },
        |app| {
            app.wizard_state.error = None;
            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::ImportPriv {
                privkey: app.import_priv.clone(),
                password: app.password.clone(),
            });
            app.password.zeroize();
            app.password = "".to_owned();
            app.password2.zeroize();
            app.password2 = "".to_owned();
        },
    )
}
