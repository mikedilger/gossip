use crate::comms::ToOverlordMessage;
use crate::globals::GLOBALS;
use crate::relay::Relay;
use crate::ui::wizard::WizardPage;
use crate::ui::{GossipUi, Page};
use eframe::egui;
use egui::{Color32, Context, RichText, Ui};
use gossip_relay_picker::Direction;
use nostr_types::RelayUrl;

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    // New users dont have existing config
    if app.wizard_state.new_user {
        app.page = Page::Wizard(WizardPage::SetupRelays);
        return;
    }

    let pubkey = match app.wizard_state.pubkey {
        None => {
            app.page = Page::Wizard(WizardPage::WelcomeGossip);
            return;
        }
        Some(pk) => pk,
    };

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.label("Relay List:");
        if app.wizard_state.need_relay_list() {
            ui.label(RichText::new("Missing").color(Color32::RED));
        } else {
            ui.label(RichText::new("Found").color(Color32::GREEN));
        }
    });

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.label("Metadata:");
        if app.wizard_state.metadata_events.is_empty() {
            ui.label(RichText::new("Missing").color(Color32::RED));
        } else {
            ui.label(RichText::new("Found").color(Color32::GREEN));
        }
    });

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.label("Contact List:");
        if app.wizard_state.contact_list_events.is_empty() {
            ui.label(RichText::new("Missing").color(Color32::RED));
        } else {
            ui.label(RichText::new("Found").color(Color32::GREEN));
        }
    });

    if app.wizard_state.need_relay_list() && !app.wizard_state.relay_list_sought {
        app.wizard_state.relay_list_sought = true;

        let discovery_relays: Vec<RelayUrl> = app
            .wizard_state
            .relays
            .iter()
            .filter(|relay| relay.has_usage_bits(Relay::DISCOVER))
            .map(|r| r.url.clone())
            .collect();

        // Fetch relay list from there
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::SubscribeDiscover(
                vec![pubkey],
                Some(discovery_relays),
            ));
    }

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    if app.wizard_state.need_relay_list() || app.wizard_state.need_user_data() {
        ui.label("Please enter a relay where your existing configuration can be loaded from:");

        // If we have write relays, show those
        if let Ok(pairs) = GLOBALS.storage.get_best_relays(pubkey, Direction::Write) {
            for (url, _score) in pairs {
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.label("Load from:");
                    if ui.button(url.as_str()).clicked() {
                        let _ = GLOBALS
                            .to_overlord
                            .send(ToOverlordMessage::SubscribeConfig(url.to_owned()));
                    }
                });
            }
        }

        ui.add_space(20.0);
        ui.horizontal_wrapped(|ui| {
            ui.label("Enter Relay URL");
            ui.add(text_edit_line!(app, app.wizard_state.relay_url));
        });

        ui.add_space(20.0);
        if ui.button("  >  Fetch From This Relay").clicked() {
            if let Ok(rurl) = RelayUrl::try_from_str(&app.wizard_state.relay_url) {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::SubscribeConfig(rurl.to_owned()));
                app.wizard_state.relay_url = String::new();
            } else {
                GLOBALS
                    .status_queue
                    .write()
                    .write("Invalid Relay URL".to_string());
            }
        }

        if app.wizard_state.need_relay_list() {
            ui.add_space(20.0);
            if ui
                .button("  >  Look up my relay list from this Relay")
                .clicked()
            {
                if let Ok(rurl) = RelayUrl::try_from_str(&app.wizard_state.relay_url) {
                    let _ = GLOBALS
                        .to_overlord
                        .send(ToOverlordMessage::SubscribeDiscover(
                            vec![pubkey],
                            Some(vec![rurl.to_owned()]),
                        ));
                    app.wizard_state.relay_url = String::new();
                } else {
                    GLOBALS
                        .status_queue
                        .write()
                        .write("Invalid Relay URL".to_string());
                }
            }
        }
    }

    ui.add_space(20.0);
    let label = if app.wizard_state.need_relay_list() || app.wizard_state.need_user_data() {
        "  >  Skip this step"
    } else {
        "  >  Next"
    };
    if ui.button(label).clicked() {
        app.page = Page::Wizard(WizardPage::SetupRelays);
    }
}
