use super::GossipUi;
use crate::globals::GLOBALS;
use crate::ui::widgets::CopyButton;
use eframe::egui;
use egui::{Context, Ui};
use tokio::task;

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.heading("Delegatee");
    ui.add_space(24.0);

    ui.label("Enter NIP-26 delegation tag, to post on the behalf of another indentity (I will be the delegatee)");
    ui.add(
        text_edit_multiline!(app, app.delegatee_tag_str)
            .hint_text("full delegation tag, JSON")
            .desired_width(f32::INFINITY),
    );
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
    ui.horizontal(|ui| {
        if ui.button("Set").clicked() {
            match GLOBALS.delegation.set(&app.delegatee_tag_str) {
                Err(e) => {
                    *GLOBALS.status_message.blocking_write() = format!("Could not parse tag {e}")
                }
                Ok(_) => {
                    // normalize string
                    app.delegatee_tag_str = GLOBALS.delegation.get_delegatee_tag_as_str();
                    // save and statusmsg
                    task::spawn(async move {
                        if let Err(e) = GLOBALS.delegation.save_through_settings().await {
                            tracing::error!("{}", e);
                        }
                        *GLOBALS.status_message.write().await = format!(
                            "Delegation tag set, delegator: {}",
                            GLOBALS
                                .delegation
                                .get_delegator_pubkey_as_bech32_str()
                                .unwrap_or("?".to_string())
                        );
                    });
                }
            };
        }
        if ui.button("Remove").clicked() {
            app.delegatee_tag_str = String::new();
            if GLOBALS.delegation.reset() {
                // save and statusmsg
                task::spawn(async move {
                    if let Err(e) = GLOBALS.delegation.save_through_settings().await {
                        tracing::error!("{}", e);
                    }
                    *GLOBALS.status_message.write().await = "Delegation tag removed".to_string();
                });
            }
        }
    });
    ui.separator();
}
