use eframe::egui;
use egui::{Context, RichText, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::GLOBALS;
use nostr_types::{Profile, PublicKey};

use super::wizard_controls;
use crate::ui::wizard::WizardPage;
use crate::ui::{GossipUi, Page};

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
        text_edit_line!(app, app.import_pub)
            .with_paste()
            .desired_width(f32::INFINITY)
            .show(ui);
    });

    let ready = try_parse_pubkey(&app.import_pub).is_ok();

    // error block
    if let Some(err) = &app.wizard_state.error {
        ui.add_space(10.0);
        ui.label(RichText::new(err).color(app.theme.warning_marker_text_color()));
    }

    ui.add_space(20.0); // vertical space
    wizard_controls(
        ui,
        app,
        ready,
        |app| {
            app.set_page(ctx, Page::Wizard(WizardPage::ImportKeys));
        },
        |app| {
            app.wizard_state.error = None;
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::ImportPub(app.import_pub.clone()));
            app.import_pub = "".to_owned();
        },
    );
}

fn try_parse_pubkey(keystr: &str) -> Result<(), ()> {
    if PublicKey::try_from_bech32_string(keystr.trim(), true).is_ok()
        || PublicKey::try_from_hex_string(keystr.trim(), true).is_ok()
        || Profile::try_from_bech32_string(keystr.trim(), true).is_ok()
    {
        Ok(())
    } else {
        Err(())
    }
}
