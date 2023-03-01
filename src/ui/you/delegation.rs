use super::GossipUi;
use crate::globals::GLOBALS;
use crate::delegation::{parse_delegation_tag, serialize_delegation_tag};
use eframe::egui;
use egui::{Context, TextEdit, Ui};

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.heading("Delegatee");
    ui.add_space(24.0);

    ui.label("Enter NIP-26 delegation tag, to post on the behalf of another indentity (I will be the delegatee)");
    // TODO validate&set automatically upon entry
    ui.add(
        TextEdit::multiline(&mut app.delegation_tag_str)
            .hint_text("full delegation tag, JSON")
            .desired_width(f32::INFINITY),
    );
    ui.horizontal(|ui| {
        ui.label("Delegator pubkey:");
        let mut delegator_npub = "(not set)".to_string();
        if let Some(pk) = app.delegation_delegator {
            delegator_npub = pk.try_as_bech32_string().unwrap_or_default();
        }
        // TODO: read-only edit box so it can be copied?
        ui.label(&delegator_npub);
    });
    ui.horizontal(|ui| {
        if ui.button("Set").clicked() {
            match parse_delegation_tag(&app.delegation_tag_str) {
                Err(e) => {
                    *GLOBALS.status_message.blocking_write() = format!("Could not parse tag {e}")
                }
                Ok((tag, delegator_pubkey)) => {
                    app.delegation_tag = Some(tag.clone());
                    app.delegation_delegator = Some(delegator_pubkey);
                    // normalize string
                    app.delegation_tag_str = serialize_delegation_tag(&tag);
                    *GLOBALS.status_message.blocking_write() = format!(
                        "Delegation tag set, delegator: {}",
                        delegator_pubkey.try_as_bech32_string().unwrap_or_default()
                    );
                }
            }
        }
        if ui.button("Remove").clicked() {
            if app.delegation_tag != None
                || !app.delegation_tag_str.is_empty()
                || app.delegation_delegator != None
            {
                app.delegation_tag = None;
                app.delegation_tag_str = String::new();
                app.delegation_delegator = None;
                *GLOBALS.status_message.blocking_write() = format!("Delegation tag removed");
            }
        }
    });
    ui.separator();
}

