use super::GossipUi;
use crate::ui::widgets::CopyButton;
use eframe::egui;
use egui::{Context, Ui};
use gossip_lib::GLOBALS;
use tokio::task;

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.add_space(10.0);
    ui.horizontal_wrapped(|ui| {
        // ui.add_space(2.0);
        ui.heading("Delegatee");
    });
    ui.add_space(10.0);
    ui.label("If NIP-26 Delegation is set, I will post on behalf of the delegator");
    ui.add_space(24.0);

    match GLOBALS.delegation.get_delegatee_tag() {
        None => {
            ui.label("No delegation is set");
        }
        Some(_dtag) => {
            ui.label("Delegation is set!");
            ui.horizontal(|ui| {
                ui.label("Delegator pubkey:");
                let delegator_npub = GLOBALS
                    .delegation
                    .get_delegator_pubkey_as_bech32_str()
                    .unwrap_or("(not set)".to_string());
                ui.label(&delegator_npub);
                if ui
                    .add(CopyButton {})
                    .on_hover_text("Copy Public Key")
                    .clicked()
                {
                    ui.output_mut(|o| o.copied_text = delegator_npub);
                }
            });
            ui.label("Delegation tag:");
            let mut dtag_str = GLOBALS.delegation.get_delegatee_tag_as_str();
            ui.add_enabled(
                false,
                text_edit_multiline!(app, dtag_str)
                    .interactive(false)
                    .desired_width(f32::INFINITY),
            );
            ui.horizontal(|ui| {
                if ui.button("Remove").clicked() {
                    app.delegatee_tag_str = String::new();
                    let _ = GLOBALS
                        .to_overlord
                        .send(gossip_lib::comms::ToOverlordMessage::DelegationReset);
                }
            });
        }
    };
    ui.separator();
    ui.add_space(12.0);

    ui.label("Enter new delegation tag");
    ui.add(
        text_edit_multiline!(app, app.delegatee_tag_str)
            .hint_text("full delegation tag, JSON")
            .desired_width(f32::INFINITY),
    );
    ui.horizontal(|ui| {
        if ui.button("Set").clicked() {
            if !app.delegatee_tag_str.is_empty() {
                match GLOBALS.delegation.set(&app.delegatee_tag_str) {
                    Err(e) => {
                        GLOBALS
                            .status_queue
                            .write()
                            .write(format!("Could not parse tag {e}"));
                    }
                    Ok(_) => {
                        // reset entry field
                        app.delegatee_tag_str = "".to_owned();
                        // save and statusmsg
                        task::spawn(async move {
                            if let Err(e) = GLOBALS.delegation.save().await {
                                tracing::error!("{}", e);
                            }
                            GLOBALS.status_queue.write().write(format!(
                                "Delegation tag set, delegator: {}",
                                GLOBALS
                                    .delegation
                                    .get_delegator_pubkey_as_bech32_str()
                                    .unwrap_or("?".to_string())
                            ));
                        });
                    }
                };
            }
        }
    });
    ui.separator();
}
