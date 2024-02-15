use super::modify_relay;
use crate::ui::wizard::{WizardPage, DEFAULT_RELAYS};
use crate::ui::{GossipUi, Page};
use eframe::egui;
use egui::{Button, Color32, Context, RichText, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::Relay;
use gossip_lib::GLOBALS;
use nostr_types::RelayUrl;
use std::collections::BTreeMap;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.add_space(20.0);
    ui.label("Please choose which relays you will use.");

    let read_relay = |url: &RelayUrl| GLOBALS.storage.read_or_create_relay(url, None).unwrap();

    // Convert our default relay strings into Relays
    // fetching from storage so we don't overwrite any critical values when saving them later.
    let mut relay_options: BTreeMap<RelayUrl, Relay> = DEFAULT_RELAYS
        .iter()
        .map(|s| {
            let url = RelayUrl::try_from_str(s).unwrap();
            (url.clone(), read_relay(&url))
        })
        .collect();

    // Get their relays
    let relays: Vec<Relay> = GLOBALS
        .storage
        .filter_relays(|relay| relay.has_any_usage_bit())
        .unwrap_or_default();

    // Add their relays to the relay_options
    for relay in &relays {
        relay_options.insert(relay.url.clone(), relay.clone());
    }

    let outbox_relays: Vec<Relay> = relays
        .iter()
        .filter(|relay| relay.has_usage_bits(Relay::OUTBOX))
        .cloned()
        .collect();

    let inbox_relays: Vec<Relay> = relays
        .iter()
        .filter(|relay| relay.has_usage_bits(Relay::INBOX))
        .cloned()
        .collect();

    let discovery_relays: Vec<Relay> = relays
        .iter()
        .filter(|relay| relay.has_usage_bits(Relay::DISCOVER))
        .cloned()
        .collect();

    ui.add_space(20.0);
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.heading("OUTBOX")
                    .on_hover_text("Relays where you post notes to");
                if outbox_relays.len() >= 3 {
                    ui.label(RichText::new(" - OK").color(Color32::GREEN));
                } else {
                    ui.label(
                        RichText::new(" - We suggest 3")
                            .color(app.theme.warning_marker_text_color()),
                    );
                }
            });
            ui.add_space(10.0);
            for relay in outbox_relays.iter() {
                ui.horizontal(|ui| {
                    if ui.button("🗑").clicked() {
                        modify_relay(&relay.url, |relay| {
                            relay.clear_usage_bits(Relay::OUTBOX | Relay::WRITE);
                        });
                    }
                    ui.label(relay.url.as_str());
                });
            }
        });

        ui.add_space(10.0);

        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.heading("INBOX").on_hover_text(
                    "Relays where people can send you events tagging you, including DMs",
                );
                if inbox_relays.len() >= 2 {
                    ui.label(RichText::new(" - OK").color(Color32::GREEN));
                } else {
                    ui.label(
                        RichText::new(" - We suggest 2")
                            .color(app.theme.warning_marker_text_color()),
                    );
                }
            });
            ui.add_space(10.0);
            for relay in inbox_relays.iter() {
                ui.horizontal(|ui| {
                    if ui.button("🗑").clicked() {
                        modify_relay(&relay.url, |relay| {
                            relay.clear_usage_bits(Relay::INBOX | Relay::READ);
                        });
                    }
                    ui.label(relay.url.as_str());
                });
            }
        });

        ui.add_space(10.0);

        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.heading("DISCOVERY")
                    .on_hover_text("Relays where you find out what relays other people are using");
                if !discovery_relays.is_empty() {
                    ui.label(RichText::new(" - OK").color(Color32::GREEN));
                } else {
                    ui.label(
                        RichText::new(" - You should have one")
                            .color(app.theme.warning_marker_text_color()),
                    );
                }
            });
            ui.add_space(10.0);
            for relay in discovery_relays.iter() {
                ui.horizontal(|ui| {
                    if ui.button("🗑").clicked() {
                        modify_relay(&relay.url, |relay| {
                            relay.clear_usage_bits(Relay::DISCOVER);
                        });
                    }
                    ui.label(relay.url.as_str());
                });
            }
        });
    });

    ui.add_space(10.0);
    ui.separator();

    ui.label("Add relays to the above lists:");
    ui.add_space(15.0);
    ui.horizontal_wrapped(|ui| {
        ui.label("Enter Relay URL");
        let response = text_edit_line!(app, app.wizard_state.relay_url)
            .with_paste()
            .show(ui)
            .0
            .response;
        if response.changed() {
            app.wizard_state.error = None;
        }
        ui.label("or");
        ui.menu_button("▼ Pick from Top Relays", |ui| {
            for (url, _relay) in relay_options.iter() {
                if ui.add(Button::new(url.as_str()).wrap(false)).clicked() {
                    app.wizard_state.relay_url = url.as_str().to_owned();
                }
            }
        });
    });

    // error block
    if let Some(err) = &app.wizard_state.error {
        ui.add_space(10.0);
        ui.label(RichText::new(err).color(app.theme.warning_marker_text_color()));
    }

    let ready = !app.wizard_state.relay_url.is_empty();

    if ready {
        ui.add_space(15.0);

        ui.horizontal(|ui| {
            if ui.button("  ^  Add to Outbox").clicked() {
                if let Ok(rurl) = RelayUrl::try_from_str(&app.wizard_state.relay_url) {
                    if !relay_options.contains_key(&rurl) {
                        relay_options.insert(rurl.clone(), read_relay(&rurl));
                    }
                    modify_relay(&rurl, |relay| {
                        relay.set_usage_bits(Relay::OUTBOX | Relay::WRITE);
                    });
                } else {
                    app.wizard_state.error = Some("ERROR: Invalid Relay URL".to_owned());
                }
            }

            if ui.button("  ^  Add to Inbox").clicked() {
                if let Ok(rurl) = RelayUrl::try_from_str(&app.wizard_state.relay_url) {
                    if !relay_options.contains_key(&rurl) {
                        relay_options.insert(rurl.clone(), read_relay(&rurl));
                    }
                    modify_relay(&rurl, |relay| {
                        relay.set_usage_bits(Relay::INBOX | Relay::READ);
                    });
                } else {
                    app.wizard_state.error = Some("ERROR: Invalid Relay URL".to_owned());
                }
            }

            if ui.button("  ^  Add to Discovery").clicked() {
                if let Ok(rurl) = RelayUrl::try_from_str(&app.wizard_state.relay_url) {
                    if !relay_options.contains_key(&rurl) {
                        relay_options.insert(rurl.clone(), read_relay(&rurl));
                    }
                    modify_relay(&rurl, |relay| {
                        relay.set_usage_bits(Relay::DISCOVER);
                    });
                } else {
                    app.wizard_state.error = Some("ERROR: Invalid Relay URL".to_owned());
                }
            }
        });
    }

    if app.wizard_state.has_private_key {
        ui.add_space(20.0);
        let mut label = RichText::new("  >  Publish and Continue");
        if app.wizard_state.new_user {
            label = label.color(app.theme.accent_color());
        }
        if ui.button(label).clicked() {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::AdvertiseRelayList);
            app.set_page(ctx, Page::Wizard(WizardPage::SetupMetadata));
        }

        ui.add_space(20.0);
        let mut label = RichText::new("  >  Continue without publishing");
        if !app.wizard_state.new_user {
            label = label.color(app.theme.accent_color());
        }
        if ui.button(label).clicked() {
            app.set_page(ctx, Page::Wizard(WizardPage::SetupMetadata));
        };
    } else {
        ui.add_space(20.0);
        let mut label = RichText::new("  >  Continue");
        label = label.color(app.theme.accent_color());
        if ui.button(label).clicked() {
            app.set_page(ctx, Page::Wizard(WizardPage::SetupMetadata));
        };
    }
}
