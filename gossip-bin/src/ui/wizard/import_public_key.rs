use crate::ui::wizard::WizardPage;
use crate::ui::{GossipUi, Page};
use eframe::egui;
use egui::{Context, RichText, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::GLOBALS;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    // If already imported, advance
    if app.wizard_state.pubkey.is_some() {
        app.set_page(ctx, Page::Wizard(WizardPage::ReadNostrConfig));
    }

    ui.add_space(20.0);
    ui.label("By importing only a public key, you will not be able to post, like, zap, send or receive DMs.");

    ui.add_space(20.0);
    ui.label("You will be able to change who you follow, and your relays, but you won't be able to save that information to nostr, those changes will remain local to this client.");

    ui.add_space(20.0);

    ui.horizontal_wrapped(|ui| {
        ui.label("Enter your public key");
        let response = text_edit_line!(app, app.import_pub)
            .with_paste()
            .desired_width(f32::INFINITY)
            .show(ui)
            .response;
        if response.changed() {
            app.wizard_state.error = None;
        }
    });

    // error block
    if let Some(err) = &app.wizard_state.error {
        ui.add_space(10.0);
        ui.label(RichText::new(err).color(app.theme.warning_marker_text_color()));
    }

    let ready = !app.import_pub.is_empty();

    if ready {
        ui.add_space(10.0);
        if ui
            .button(RichText::new("  >  Import").color(app.theme.accent_color()))
            .clicked()
        {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::ImportPub(app.import_pub.clone()));
            app.import_pub = "".to_owned();
        }
    }

    ui.add_space(20.0);
    if ui.button("  <  Go Back").clicked() {
        app.set_page(ctx, Page::Wizard(WizardPage::ImportKeys));
    }
}
