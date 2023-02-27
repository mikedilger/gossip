use super::GossipUi;
// use crate::globals::GLOBALS;
use eframe::egui;
use egui::{Context, TextEdit, Ui};
use nostr_types::{PublicKey, PublicKeyHex, Signature, SignatureHex, Tag};

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.heading("Delegatee");
    ui.add_space(24.0);

    ui.label("Enter NIP-26 delegation tag, to post on the behalf of another indentity (delegatee)");
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
            app.delegation_tag = None;
            match parse_delegation_tag(&app.delegation_tag_str) {
                Err(_e) => {}, // TODO *GLOBALS.status_message.write().await = format!("Could not parse tag {e}"),
                Ok((tag, delegator_pubkey)) => {
                    app.delegation_tag = Some(tag);
                    app.delegation_delegator = Some(delegator_pubkey);
                    // TODO *GLOBALS.status_message.write().await = format!("Delegation tag set, delegator {pubkeybech}");
                },
            }
        }
        if ui.button("Reset").clicked() {
            app.delegation_tag = None;
            app.delegation_tag_str = String::new();
            app.delegation_delegator = None;
        }
    });
    ui.separator();
}

// Returns parsed tag & delegator pub; error is string (for simplicity)
fn parse_delegation_tag(tag: &str) -> Result<(Tag, PublicKey), String> {
    // TODO parsing should be done using nostr crate v0.19 DelegationTag
    match json::parse(tag) {
        Err(e) => return Err(format!("Could not parse tag, {}", e.to_string())),
        Ok(jv) => {
            if !jv.is_array() || jv.len() < 4 {
                return Err(format!("Expected array with 4 elements"));
            }
            if !jv[0].is_string() || !jv[1].is_string() || !jv[2].is_string() || !jv[3].is_string() {
                return Err(format!("Expected array with 4 strings"));
            }
            if jv[0].as_str().unwrap() != "delegation" {
                return Err(format!("First string should be 'delegation'"));
            }
            match PublicKey::try_from_hex_string(jv[1].as_str().unwrap()) {
                Err(e) => return Err(format!("Could not parse public key, {}", e.to_string())),
                Ok(public_key) => {
                    let pubkey = PublicKeyHex::from(public_key);
                    let conditions = jv[2].as_str().unwrap().to_string();
                    let sig_str = jv[3].as_str().unwrap();
                    match Signature::try_from_hex_string(sig_str) {
                        Err(e) => return Err(format!("Could not parse signature, {}", e.to_string())),
                        Ok(signature) => {
                            let sig = SignatureHex::from(signature);
                            Ok((Tag::Delegation { pubkey, conditions, sig }, public_key))
                        }
                    }
                }
            }
        }
    }
}
