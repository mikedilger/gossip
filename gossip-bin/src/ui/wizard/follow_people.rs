use crate::ui::wizard::WizardPage;
use crate::ui::{GossipUi, Page};
use eframe::egui;
use egui::{Context, RichText, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::{Person, PersonList, GLOBALS};
use gossip_relay_picker::Direction;
use nostr_types::{Profile, PublicKey};

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    if app.wizard_state.pubkey.is_none() && !app.wizard_state.follow_only {
        app.set_page(ctx, Page::Wizard(WizardPage::WelcomeGossip));
        return;
    }

    // Merge in their contacts data
    if app.wizard_state.contacts_sought {
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::UpdatePersonList {
                person_list: PersonList::Followed,
                merge: false,
            });
        app.wizard_state.contacts_sought = false;
    }

    ui.add_space(10.0);
    ui.heading("Followed:");
    let mut limit = 10;
    for pk in &app.wizard_state.followed {
        let person = match GLOBALS.storage.read_person(pk) {
            Ok(Some(p)) => p,
            Ok(None) => Person::new(*pk),
            Err(_) => Person::new(*pk),
        };

        if let Some(metadata) = person.metadata {
            // We have metadata, render their name
            if let Some(name) = &metadata.name {
                ui.label(name);
            } else {
                ui.label(pk.as_hex_string());
            }
        } else {
            // We don't have metadata
            if let Ok(outboxes) = GLOBALS.storage.get_best_relays(*pk, Direction::Write) {
                if !outboxes.is_empty() {
                    // But we have their outboxes
                    if !app.wizard_state.followed_getting_metadata.contains(pk) {
                        // And we haven't asked for metadata yet,
                        // trigger fetch of their metadata
                        let _ = GLOBALS
                            .to_overlord
                            .send(ToOverlordMessage::UpdateMetadata(*pk));
                        // then remember we did so we don't keep doing it over and over again
                        app.wizard_state
                            .followed_getting_metadata
                            .insert(pk.to_owned());
                    }
                    ui.label(format!("{} [seeking metadata]", pk.as_hex_string()));
                } else {
                    // We don't have outboxes... this will come. Following them triggered this.
                    ui.label(format!("{} [seeking their relay list]", pk.as_hex_string()));
                }
            } else {
                // We don't have outboxes... this will come. Following them triggered this.
                ui.label(format!("{} [seeking their relay list]", pk.as_hex_string()));
            }
        }

        limit -= 1;
        if limit == 0 && app.wizard_state.followed.len() > 10 {
            ui.add_space(10.0);
            ui.label(format!(
                "...and {} more",
                app.wizard_state.followed.len() - 10
            ));
            break;
        }
    }

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(20.0);

    ui.horizontal(|ui| {
        ui.label("Follow Someone:");
        let response = text_edit_line!(app, app.add_contact)
            .with_paste()
            .hint_text(
                "Enter a key (bech32 npub1 or hex), or an nprofile, or a DNS id (user@domain)",
            )
            .show(ui)
            .0
            .response;
        if response.changed() {
            app.wizard_state.error = None;
        }
        if ui.button("follow").clicked() {
            if let Ok(pubkey) = PublicKey::try_from_bech32_string(app.add_contact.trim(), true) {
                let _ = GLOBALS.to_overlord.send(ToOverlordMessage::FollowPubkey(
                    pubkey,
                    PersonList::Followed,
                    true,
                ));
            } else if let Ok(pubkey) = PublicKey::try_from_hex_string(app.add_contact.trim(), true)
            {
                let _ = GLOBALS.to_overlord.send(ToOverlordMessage::FollowPubkey(
                    pubkey,
                    PersonList::Followed,
                    true,
                ));
            } else if let Ok(profile) =
                Profile::try_from_bech32_string(app.add_contact.trim(), true)
            {
                let _ = GLOBALS.to_overlord.send(ToOverlordMessage::FollowNprofile(
                    profile,
                    PersonList::Followed,
                    true,
                ));
            } else if gossip_lib::nip05::parse_nip05(app.add_contact.trim()).is_ok() {
                let _ = GLOBALS.to_overlord.send(ToOverlordMessage::FollowNip05(
                    app.add_contact.trim().to_owned(),
                    PersonList::Followed,
                    true,
                ));
            } else {
                app.wizard_state.error = Some("ERROR: Invalid pubkey".to_owned());
            }
            app.add_contact = "".to_owned();
        }
    });

    // error block
    if let Some(err) = &app.wizard_state.error {
        // Ignore this one:
        if err.starts_with("Could not find a person-list") {
            app.wizard_state.error = None;
        } else {
            ui.add_space(10.0);
            ui.label(RichText::new(err).color(app.theme.warning_marker_text_color()));
        }
    }

    ui.add_space(10.0);
    ui.label("We accept:");
    ui.label("  • Public key (npub1..)");
    ui.label("  • Public key (hex)");
    ui.label("  • Profile (nprofile1..)");
    ui.label("  • DNS ID (user@domain)");

    if app.wizard_state.has_private_key {
        ui.add_space(20.0);
        let mut label = RichText::new("  >  Publish and Finish");
        if app.wizard_state.new_user {
            label = label.color(app.theme.accent_color());
        }
        if ui.button(label).clicked() {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::PushPersonList(PersonList::Followed));

            super::complete_wizard(app, ctx);
        }

        ui.add_space(20.0);
        let mut label = RichText::new("  >  Finish without publishing");
        if !app.wizard_state.new_user {
            label = label.color(app.theme.accent_color());
        }
        if ui.button(label).clicked() {
            super::complete_wizard(app, ctx);
        }
    } else {
        ui.add_space(20.0);
        let mut label = RichText::new("  >  Finish");
        label = label.color(app.theme.accent_color());
        if ui.button(label).clicked() {
            super::complete_wizard(app, ctx);
        }
    }
}
