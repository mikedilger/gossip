use eframe::egui;
use egui::{Context, RichText, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::GLOBALS;
use zeroize::Zeroize;

use super::wizard_controls;
use crate::ui::wizard::WizardPage;
use crate::ui::{GossipUi, Page};

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    // If already generated, advance
    if app.wizard_state.has_private_key {
        app.wizard_state.generating = false;
        app.set_page(ctx, Page::Wizard(WizardPage::SetupRelays));
    }

    ui.add_space(10.0);
    ui.label("Nostr is a fully distributed social media network protocol.");

    ui.add_space(10.0);
    ui.label("Gossip is one of many clients. The account you create here will work on all the other clients too. You will not be locked into this client in any way.");

    ui.add_space(10.0);
    ui.label("Your keypair is your account. Accounts do not need to be registered anywhere. Every new keypair is a valid nostr account. The public key is used to identify you, and the private key is used to prove you are the one who created the public key.");

    ui.add_space(20.0);
    ui.heading("Generate a Keypair");

    // compute results from previous ui update
    let password_mismatch = app.password != app.password2;
    let ready = !password_mismatch;

    ui.add_space(10.0);
    egui::Grid::new("inputs")
        .num_columns(2)
        .striped(false)
        .spacing([10.0, 10.0])
        .show(ui, |ui| {
            ui.label("Enter a passphrase to keep it encrypted under");
            if ui
                .add(text_edit_line!(app, app.password).password(true))
                .changed()
            {
                app.wizard_state.error = None;
            }
            ui.end_row();

            ui.label("Repeat that passphrase");
            if ui
                .add(text_edit_line!(app, app.password2).password(true))
                .changed()
            {
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
                "Passwords do not match."
            };
            ui.label(RichText::new(text).color(app.theme.warning_marker_text_color()));
            ui.end_row();
        });

    // error block
    if !app.wizard_state.generating {
        if let Some(err) = &app.wizard_state.error {
            ui.add_space(10.0);
            ui.label(RichText::new(err).color(app.theme.warning_marker_text_color()));
        }
    }

    if app.wizard_state.generating {
        ui.add_space(10.0);
        ui.label("Generating keypair ...");
    }

    ui.add_space(20.0);
    wizard_controls(
        ui,
        app,
        ready,
        |app| {
            app.set_page(ctx, Page::Wizard(WizardPage::WelcomeGossip));
        },
        |app| {
            app.wizard_state.generating = true;
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::GeneratePrivateKey(app.password.clone()));
            app.password.zeroize();
            app.password = "".to_owned();
            app.password2.zeroize();
            app.password2 = "".to_owned();
        },
    );
}
