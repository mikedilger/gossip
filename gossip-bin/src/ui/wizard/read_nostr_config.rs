use crate::ui::wizard::WizardPage;
use crate::ui::{GossipUi, Page};
use eframe::egui;
use egui::{Color32, Context, RichText, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::Relay;
use gossip_lib::GLOBALS;
use nostr_types::RelayUrl;

use super::continue_control;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    // New users dont have existing config
    if app.wizard_state.new_user {
        app.set_page(ctx, Page::Wizard(WizardPage::SetupRelays));
        return;
    }

    let pubkey = match app.wizard_state.pubkey {
        None => {
            app.set_page(ctx, Page::Wizard(WizardPage::WelcomeGossip));
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

    if app.wizard_state.need_relay_list() && app.wizard_state.relay_list_sought {
        app.wizard_state.relay_list_sought = false;

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
        let mut found = false;

        // If we have write relays, show those
        if let Ok(urls) = Relay::choose_relay_urls(Relay::WRITE, |_| true) {
            if !urls.is_empty() {
                app.vert_scroll_area()
                .max_width(f32::INFINITY)
                .max_height(ctx.screen_rect().height() - 340.0)
                .show(ui, |ui| {
                    found = true;
                    ui.label("Good news: we found your profile!");
                    ui.label("Please choose one relay to load your profile (kind: 03) from, or specify a manual relay below:");
                    for url in urls {
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            ui.label(url.as_str());
                            app.theme.primary_button_style(ui.style_mut());
                            if ui.button("Load").clicked() {
                                let _ =
                                    GLOBALS
                                        .to_overlord
                                        .send(ToOverlordMessage::SubscribeConfig(Some(vec![
                                            url.to_owned()
                                        ])));
                            }
                        });
                    }
                });
            }
        }

        if !found {
            ui.label("We could not yet find your profile...");
            ui.label("You can manually enter a relay where your existing profile (kind: 03) can be loaded from:");
        }

        ui.add_space(20.0);
        ui.horizontal_wrapped(|ui| {
            ui.label("Enter Relay URL");
            let response = text_edit_line!(app, app.wizard_state.relay_url)
                .with_paste()
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

        let ready = !app.wizard_state.relay_url.is_empty();

        if ready {
            ui.add_space(20.0);
            ui.scope(|ui| {
                app.theme.secondary_button_style(ui.style_mut());
                if ui.button("Fetch From This Relay").clicked() {
                    if let Ok(rurl) = RelayUrl::try_from_str(&app.wizard_state.relay_url) {
                        let _ = GLOBALS
                            .to_overlord
                            .send(ToOverlordMessage::SubscribeConfig(Some(vec![
                                rurl.to_owned()
                            ])));
                        app.wizard_state.relay_url = String::new();
                    } else {
                        app.wizard_state.error = Some("ERROR: Invalid Relay URL".to_owned());
                    }
                }
            });
        }
    }

    ui.add_space(20.0);
    continue_control(ui, app, true, |app| {
        app.set_page(ctx, Page::Wizard(WizardPage::SetupRelays));
    });
}
