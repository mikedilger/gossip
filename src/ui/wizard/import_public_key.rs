use crate::comms::ToOverlordMessage;
use crate::globals::GLOBALS;
use crate::ui::wizard::WizardPage;
use crate::ui::{GossipUi, Page};
use eframe::egui;
use egui::{Context, RichText, Ui};

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    // If already imported, advance
    if app.wizard_state.pubkey.is_some() {
        app.page = Page::Wizard(WizardPage::ReadNostrConfig);
    }

    ui.add_space(20.0);
    ui.label("By importing only a public key, you will not be able to post, like, zap, send or receive DMs.");

    ui.add_space(20.0);
    ui.label("You will be able to change who you follow, and your relays, but you won't be able to save that information to nostr, those changes will remain local to this client.");

    ui.add_space(20.0);

    ui.horizontal_wrapped(|ui| {
        ui.label("Enter your public key");
        ui.add(text_edit_line!(app, app.import_pub).desired_width(f32::INFINITY));
    });

    ui.add_space(10.0);
    if ui
        .button(RichText::new("  >  Import").color(app.settings.theme.accent_color()))
        .clicked()
    {
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::ImportPub(app.import_pub.clone()));
        app.import_pub = "".to_owned();
    }

    ui.add_space(20.0);
    if ui.button("  <  Go Back").clicked() {
        app.page = Page::Wizard(WizardPage::ImportKeys);
    }
}
