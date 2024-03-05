use crate::ui::wizard::WizardPage;
use crate::ui::{widgets, GossipUi, Page};
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
    ui.heading(format!("Followed ({}):", app.wizard_state.followed.len()));

    egui::ScrollArea::new([false, true])
        .max_width(f32::INFINITY)
        .max_height(0.4 * ctx.screen_rect().height())
        .show(ui, |ui| {
            for iter in app.wizard_state.followed.iter_mut() {
                // use cached person dataset
                let person = if let Some(person) = &iter.1 {
                    person
                } else {
                    let pk = iter.0.unwrap();
                    let person = match GLOBALS.storage.read_person(&pk) {
                        Ok(Some(p)) => p,
                        Ok(None) => Person::new(pk),
                        Err(_) => Person::new(pk),
                    };
                    iter.0 = None;
                    iter.1 = Some(person.to_owned());
                    iter.1.as_ref().unwrap()
                };

                widgets::list_entry::make_frame(ui, Some(app.theme.main_content_bgcolor())).show(
                    ui,
                    |ui| {
                        ui.horizontal(|ui| {
                            if let Some(metadata) = &person.metadata {
                                // We have metadata, render their name
                                if let Some(name) = &metadata.name {
                                    ui.label(name);
                                } else {
                                    ui.label(person.pubkey.as_hex_string());
                                }
                            } else {
                                // We don't have metadata
                                if let Ok(outboxes) = GLOBALS
                                    .storage
                                    .get_best_relays(person.pubkey, Direction::Write)
                                {
                                    if !outboxes.is_empty() {
                                        // But we have their outboxes
                                        if !app
                                            .wizard_state
                                            .followed_getting_metadata
                                            .contains(&person.pubkey)
                                        {
                                            tracing::warn!(
                                                "seek metadata for {}",
                                                person.pubkey.as_hex_string()
                                            );
                                            // // And we haven't asked for metadata yet,
                                            // // trigger fetch of their metadata
                                            // let _ = GLOBALS.to_overlord.send(
                                            //     ToOverlordMessage::UpdateMetadata(person.pubkey),
                                            // );
                                            // then remember we did so we don't keep doing it over and over again
                                            app.wizard_state
                                                .followed_getting_metadata
                                                .insert(person.pubkey.to_owned());
                                        }
                                        ui.label(format!(
                                            "{} [seeking metadata]",
                                            person.pubkey.as_hex_string()
                                        ));
                                    } else {
                                        // We don't have outboxes... this will come. Following them triggered this.
                                        ui.label(format!(
                                            "{} [seeking their relay list]",
                                            person.pubkey.as_hex_string()
                                        ));
                                    }
                                } else {
                                    // We don't have outboxes... this will come. Following them triggered this.
                                    ui.label(format!(
                                        "{} [seeking their relay list]",
                                        person.pubkey.as_hex_string()
                                    ));
                                }
                            }
                        });
                    },
                );

                // refresh pending metadata
                let uitime = ctx.input(|i| i.time);
                if (app.wizard_state.followed_last_try + 5.0) < uitime {
                    let list = app.wizard_state.followed_getting_metadata.drain();
                    let _ = GLOBALS
                        .to_overlord
                        .send(ToOverlordMessage::UpdateMetadataInBulk(list.collect()));
                    app.wizard_state.followed_getting_metadata.clear();
                    app.wizard_state.followed_last_try = uitime;
                }
            }
        });

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
