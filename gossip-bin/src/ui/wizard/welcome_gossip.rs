use crate::ui::wizard::WizardPage;
use crate::ui::{GossipUi, Page};
use eframe::egui;
use egui::{Context, RichText, Ui};
use gossip_lib::GLOBALS;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    if app.wizard_state.pubkey.is_some() {
        app.set_page(ctx, Page::Wizard(WizardPage::ReadNostrConfig));
        return;
    };

    ui.add_space(10.0);
    ui.label("Gossip is a Nostr client for desktops");

    ui.add_space(10.0);
    ui.label("Please select from the following choices:");

    ui.add_space(20.0);
    if ui
        .button(RichText::new("  >  Create a New Nostr Account").color(app.theme.accent_color()))
        .clicked()
    {
        app.wizard_state.new_user = true;
        app.set_page(ctx, Page::Wizard(WizardPage::WelcomeNostr));
    }

    ui.add_space(20.0);
    if ui
        .button(
            RichText::new("  >  I Already have a Nostr Account").color(app.theme.accent_color()),
        )
        .clicked()
    {
        app.wizard_state.new_user = false;
        app.set_page(ctx, Page::Wizard(WizardPage::ImportKeys))
    }

    ui.add_space(20.0);
    if ui.button("  >  Just follow people (no account)").clicked() {
        app.wizard_state.new_user = false;
        app.wizard_state.follow_only = true;
        let _ = GLOBALS.storage.set_flag_following_only(true, None);
        app.set_page(ctx, Page::Wizard(WizardPage::FollowPeople));
    }

    ui.add_space(20.0);
    if ui.button("  X  Exit this Wizard").clicked() {
        super::complete_wizard(app, ctx);
    }
}
