use crate::ui::wizard::WizardPage;
use crate::ui::{GossipUi, Page};
use eframe::egui;
use egui::{Context, Ui};

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    if app.wizard_state.pubkey.is_some() {
        app.page = Page::Wizard(WizardPage::ReadNostrConfig);
        return;
    };

    ui.add_space(20.0);
    if ui.button("  >  Import an Encrypted Private Key").clicked() {
        app.page = Page::Wizard(WizardPage::ImportEncryptedPrivateKey);
    }

    ui.add_space(20.0);
    if ui.button("  >  Import a Naked Private Key").clicked() {
        app.page = Page::Wizard(WizardPage::ImportPrivateKey);
    }

    ui.add_space(20.0);
    if ui.button("  >  Import a Public Key only").clicked() {
        app.page = Page::Wizard(WizardPage::ImportPublicKey);
    }

    ui.add_space(20.0);
    if ui.button("  <  Go Back").clicked() {
        app.page = Page::Wizard(WizardPage::WelcomeGossip);
    }
}
