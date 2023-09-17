use crate::comms::ToOverlordMessage;
use crate::globals::GLOBALS;
use crate::ui::wizard::WizardPage;
use crate::ui::{GossipUi, Page};
use eframe::egui;
use egui::{Context, Ui};
use zeroize::Zeroize;

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    // If already generated, advance
    if app.wizard_state.has_private_key {
        app.page = Page::Wizard(WizardPage::ReadNostrConfig);
    }

    ui.add_space(10.0);
    ui.label("Nostr is a fully distributed social media network protocol.");

    ui.add_space(10.0);
    ui.label("Gossip is one of many clients. The account you create here will work on all the other clients too. You will not be locked into this client in any way.");

    ui.add_space(10.0);
    ui.label("Your keypair is your account. Accounts do not need to be registered anywhere. Every new keypair is a valid nostr account. The public key is used to identify you, and the private key is used to prove you are the one who created the public key.");

    ui.add_space(20.0);
    ui.heading("Generate a Keypair");

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.label("Enter a passphrase to keep it encrypted under");
        ui.add(text_edit_line!(app, app.password).password(true));
    });

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.label("Repeat that passphrase");
        ui.add(text_edit_line!(app, app.password2).password(true));
    });

    ui.add_space(10.0);
    if ui.button("  >  Generate Now").clicked() {
        if app.password != app.password2 {
            GLOBALS
                .status_queue
                .write()
                .write("Passwords do not match".to_owned());
        } else {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::GeneratePrivateKey(app.password.clone()));
        }
        app.password.zeroize();
        app.password = "".to_owned();
        app.password2.zeroize();
        app.password2 = "".to_owned();
    }

    ui.add_space(20.0);
    if ui.button("  <  Go Back").clicked() {
        app.page = Page::Wizard(WizardPage::WelcomeGossip);
    }
}
