use crate::globals::GLOBALS;
use crate::ui::wizard::WizardPage;
use crate::ui::{GossipUi, Page};
use eframe::egui;
use egui::{Context, RichText, Ui};

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    if app.wizard_state.pubkey.is_some() {
        app.page = Page::Wizard(WizardPage::ReadNostrConfig);
        return;
    };

    ui.add_space(10.0);
    ui.label("Gossip is a Nostr client for desktops");

    ui.add_space(10.0);
    ui.label("Please select from the following choices:");

    ui.add_space(20.0);
    if ui
        .button(
            RichText::new("  >  Create a New Nostr Account")
                .color(app.settings.theme.accent_color()),
        )
        .clicked()
    {
        app.wizard_state.new_user = true;
        app.page = Page::Wizard(WizardPage::WelcomeNostr);
    }

    ui.add_space(20.0);
    if ui
        .button(
            RichText::new("  >  I Already have a Nostr Account")
                .color(app.settings.theme.accent_color()),
        )
        .clicked()
    {
        app.wizard_state.new_user = false;
        app.page = Page::Wizard(WizardPage::ImportKeys);
    }

    ui.add_space(20.0);
    if ui.button("  >  Just follow people (no account)").clicked() {
        app.wizard_state.new_user = false;
        app.wizard_state.follow_only = true;
        let _ = GLOBALS.storage.write_following_only(true, None);
        app.page = Page::Wizard(WizardPage::FollowPeople);
    }
}
