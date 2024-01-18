use crate::ui::{GossipUi, Page};
use eframe::egui;
use egui::widgets::{Button, Slider};
use egui::{Align, Context, Layout};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::{FeedKind, PersonList, Relay, GLOBALS};
use nostr_types::RelayUrl;

mod follow_people;
mod import_keys;
mod import_private_key;
mod import_public_key;
mod read_nostr_config;
mod setup_metadata;
mod setup_relays;
mod welcome_gossip;
mod welcome_nostr;

mod wizard_state;
pub use wizard_state::WizardState;

static DEFAULT_RELAYS: [&str; 20] = [
    "wss://nostr.einundzwanzig.space/",
    "wss://nostr.mutinywallet.com/",
    "wss://relay.nostrplebs.com/",
    "wss://nostr-pub.wellorder.net/",
    "wss://relay.damus.io/",
    "wss://public.relaying.io/",
    "wss://nostrue.com/",
    "wss://relay.snort.social/",
    "wss://relay.noswhere.com/",
    "wss://relay.primal.net/",
    "wss://relay.nostr.jabber.ch/",
    "wss://relay.wellorder.net/",
    "wss://nostr.coinfundit.com/",
    "wss://relay.nostrich.de/",
    "wss://nostr21.com/",
    "wss://relayable.org/",
    "wss://nostr.bitcoiner.social/",
    "wss://no.str.cr/",
    "wss://offchain.pub/",
    "wss://nostr.azte.co/",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WizardPage {
    WelcomeGossip,
    WelcomeNostr,
    ImportKeys,
    ImportPrivateKey,
    ImportPublicKey,
    ReadNostrConfig,
    SetupRelays,
    SetupMetadata,
    FollowPeople,
}

impl WizardPage {
    pub fn as_str(&self) -> &'static str {
        match self {
            WizardPage::WelcomeGossip => "Welcome to Gossip",
            WizardPage::WelcomeNostr => "Welcome to Nostr",
            WizardPage::ImportKeys => "Import Keys",
            WizardPage::ImportPrivateKey => "Import a Private Key",
            WizardPage::ImportPublicKey => "Import only a Public Key",
            WizardPage::ReadNostrConfig => "Read your Nostr Configuration Data",
            WizardPage::SetupRelays => "Setup Relays",
            WizardPage::SetupMetadata => "Setup your Metadata",
            WizardPage::FollowPeople => "Follow People",
        }
    }
}

pub(super) fn start_wizard_page(wizard_state: &mut WizardState) -> Option<WizardPage> {
    // Update wizard state
    wizard_state.update();

    if wizard_state.follow_only {
        if !wizard_state.followed.is_empty() {
            return None;
        }
        return Some(WizardPage::FollowPeople);
    }

    if wizard_state.new_user {
        // No relays (new user) -> SetupRelays
        if wizard_state.relay_list_events.is_empty() {
            return Some(WizardPage::SetupRelays);
        }
    } else {
        // No relay lists (existing user) -> ReadNostrConfig
        if wizard_state.relay_list_events.is_empty() {
            return Some(WizardPage::ReadNostrConfig);
        }

        // No metadata events (existing user) -> ReadNostrConfig
        if wizard_state.metadata_events.is_empty() {
            return Some(WizardPage::ReadNostrConfig);
        }

        // No contact list (existing user) --> ReadNostrConfig
        if wizard_state.contact_list_events.is_empty() {
            return Some(WizardPage::ReadNostrConfig);
        }
    }

    // if no outbox relays --> SetupRelays
    let outbox_relays: Vec<Relay> = wizard_state
        .relays
        .iter()
        .filter(|relay| relay.has_usage_bits(Relay::OUTBOX))
        .cloned()
        .collect();
    if outbox_relays.len() < 2 {
        return Some(WizardPage::SetupRelays);
    }

    // if no inbox relays --> SetupRelays
    let inbox_relays: Vec<Relay> = wizard_state
        .relays
        .iter()
        .filter(|relay| relay.has_usage_bits(Relay::INBOX))
        .cloned()
        .collect();
    if inbox_relays.len() < 3 {
        return Some(WizardPage::SetupRelays);
    }

    // if no disc relays --> SetupRelays
    let disc_relays: Vec<Relay> = wizard_state
        .relays
        .iter()
        .filter(|relay| relay.has_usage_bits(Relay::DISCOVER))
        .cloned()
        .collect();
    if disc_relays.is_empty() {
        return Some(WizardPage::SetupRelays);
    }

    if !wizard_state.follow_only {
        if let Some(pk) = &wizard_state.pubkey {
            match GLOBALS.storage.read_person(pk) {
                Ok(Some(person)) => {
                    if person.metadata.is_none() {
                        return Some(WizardPage::SetupMetadata);
                    }
                }
                _ => return Some(WizardPage::SetupMetadata),
            }
        } else {
            return Some(WizardPage::SetupMetadata);
        }
    };

    if wizard_state.followed.is_empty() {
        return Some(WizardPage::FollowPeople);
    }

    None
}

pub(super) fn update(app: &mut GossipUi, ctx: &Context, frame: &mut eframe::Frame, wp: WizardPage) {
    // Update the wizard state
    app.wizard_state.update();

    egui::CentralPanel::default()
        .frame({
            let frame = egui::Frame::central_panel(&app.theme.get_style());
            frame.inner_margin(egui::Margin {
                left: 20.0,
                right: 10.0,
                top: 10.0,
                bottom: 0.0,
            })
        })
        .show(ctx, |ui| {
            ui.add_space(24.0);
            ui.heading(wp.as_str());
            ui.add_space(12.0);
            /*
            if let Some(err) = app.wizard_state.error {
            ui.label(RichText::new(err).color(app.theme.warning_marker_text_color()));
            ui.add_space(12.0);
            }
            */
            ui.separator();

            match wp {
                WizardPage::WelcomeGossip => welcome_gossip::update(app, ctx, frame, ui),
                WizardPage::WelcomeNostr => welcome_nostr::update(app, ctx, frame, ui),
                WizardPage::ImportKeys => import_keys::update(app, ctx, frame, ui),
                WizardPage::ImportPrivateKey => import_private_key::update(app, ctx, frame, ui),
                WizardPage::ImportPublicKey => import_public_key::update(app, ctx, frame, ui),
                WizardPage::ReadNostrConfig => read_nostr_config::update(app, ctx, frame, ui),
                WizardPage::SetupRelays => setup_relays::update(app, ctx, frame, ui),
                WizardPage::SetupMetadata => setup_metadata::update(app, ctx, frame, ui),
                WizardPage::FollowPeople => follow_people::update(app, ctx, frame, ui),
            }

            ui.add_space(20.0);
            if wp != WizardPage::FollowPeople {
                if ui.button("  X  Exit this Wizard").clicked() {
                    complete_wizard(app, ctx);
                }
            }

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            ui.with_layout(Layout::bottom_up(Align::Min), |ui| {

                ui.add_space(20.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label("Switch to");
                    if app.theme.dark_mode {
                        if ui
                            .add(Button::new("☀ Light"))
                            .on_hover_text("Switch to light mode")
                            .clicked()
                        {
                            write_setting!(dark_mode, false);
                            app.theme.dark_mode = false;
                            crate::ui::theme::apply_theme(&app.theme, ctx);
                        }
                    } else {
                        if ui
                            .add(Button::new("🌙 Dark"))
                            .on_hover_text("Switch to dark mode")
                            .clicked()
                        {
                            write_setting!(dark_mode, true);
                            app.theme.dark_mode = true;
                            crate::ui::theme::apply_theme(&app.theme, ctx);
                        }
                    }
                    ui.label("mode");
                });

                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label("Adjust DPI");
                    ui.add(Slider::new(&mut app.override_dpi_value, 72..=200));
                    if ui.button("Apply").clicked() {
                        // Make it happen
                        let ppt: f32 = app.override_dpi_value as f32 / 72.0;
                        ctx.set_pixels_per_point(ppt);

                        // Store in settings
                        write_setting!(override_dpi, Some(app.override_dpi_value));
                    }
                });

                ui.add_space(10.0);
                match wp {
                    WizardPage::WelcomeGossip | WizardPage::WelcomeNostr | WizardPage::ImportKeys => {
                        // No input fields on those pages
                    }
                    _ => {
                        ui.label("NOTE: Use CTRL-V to paste (other unix-style pastes probably won't work)");
                        ui.add_space(10.0);
                    },
                }
            });
        });
}

fn complete_wizard(app: &mut GossipUi, ctx: &Context) {
    let _ = GLOBALS.storage.set_flag_wizard_complete(true, None);
    app.set_page(ctx, Page::Feed(FeedKind::List(PersonList::Followed, false)));

    // Once the wizard is complete, we need to tell the overlord to re-run
    // its startup stuff, because we now have configuration that matters, and
    // this way people don't have to restart gossip
    let _ = GLOBALS
        .to_overlord
        .send(ToOverlordMessage::StartLongLivedSubscriptions);
}

fn modify_relay<M>(relay_url: &RelayUrl, mut modify: M)
where
    M: FnMut(&mut Relay),
{
    // Load relay record
    let mut relay = GLOBALS
        .storage
        .read_or_create_relay(relay_url, None)
        .unwrap();
    let old = relay.clone();

    // Run modification
    modify(&mut relay);

    // Save relay via the Overlord, so minions can be updated
    let _ = GLOBALS
        .to_overlord
        .send(ToOverlordMessage::UpdateRelay(old, relay));
}
