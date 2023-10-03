use crate::ui::wizard::WizardPage;
use crate::ui::{GossipUi, Page};
use eframe::egui;
use egui::{Context, RichText, Ui};

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    if app.wizard_state.pubkey.is_some() {
        app.page = Page::Wizard(WizardPage::ReadNostrConfig);
        return;
    };

    ui.add_space(20.0);
    if ui
        .button(RichText::new("  >  Import a Private Key").color(app.settings.theme.accent_color()))
        .clicked()
    {
        app.page = Page::Wizard(WizardPage::ImportPrivateKey);
    }

    ui.add_space(20.0);
    if ui
        .button(
            RichText::new("  >  Import a Public Key only").color(app.settings.theme.accent_color()),
        )
        .clicked()
    {
        app.page = Page::Wizard(WizardPage::ImportPublicKey);
    }

    ui.add_space(20.0);
    if ui.button("  <  Go Back").clicked() {
        app.page = Page::Wizard(WizardPage::WelcomeGossip);
    }
}
