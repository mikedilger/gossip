use std::cell::RefCell;
use std::rc::Rc;

use crate::ui::wizard::WizardPage;
use crate::ui::{widgets, GossipUi, Page};
use eframe::egui;
use egui::{Context, RichText, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::{PersonList, GLOBALS};
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

    // Retrieve `Person` records
    // this will take the (Some(Pubkey), None) tuple
    // and turn it into a (None, Some(Person)) tuple
    for iter in app.wizard_state.followed.iter_mut() {
        if iter.1.is_none() {
            let pk = iter.0.unwrap();
            if let Ok(Some(p)) = GLOBALS.storage.read_person(&pk) {
                iter.0 = None;
                iter.1 = Some(Rc::new(RefCell::new(p)));
            }
        }
    }

    let mut followed = app.wizard_state.followed.clone();

    ui.heading(format!(
        "People Followed ({}):",
        app.wizard_state.followed.len()
    ));
    ui.add_space(10.0);

    egui::ScrollArea::new([false, true])
        .max_width(f32::INFINITY)
        .max_height(ctx.screen_rect().height() - 400.0)
        .show(ui, |ui| {
            for iter in followed.iter_mut() {
                if iter.1.is_none() {
                    continue;
                }

                let person = iter.1.as_mut().unwrap();
                widgets::list_entry::make_frame(ui, Some(app.theme.main_content_bgcolor())).show(
                    ui,
                    |ui| {
                        let pubkey = person.borrow().pubkey;
                        ui.horizontal(|ui| {
                            // Avatar first
                            let avatar = if let Some(avatar) = app.try_get_avatar(ctx, &pubkey) {
                                avatar
                            } else {
                                app.placeholder_avatar.clone()
                            };

                            widgets::paint_avatar(
                                ui,
                                &person.borrow(),
                                &avatar,
                                widgets::AvatarSize::Feed,
                            );

                            ui.add_space(20.0);

                            ui.vertical(|ui| {
                                ui.add_space(5.0);
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(person.borrow().best_name()).size(15.5));

                                    ui.add_space(10.0);

                                    if person.borrow().metadata.is_none() {
                                        // We don't have metadata
                                        if let Ok(outboxes) = GLOBALS
                                            .storage
                                            .get_best_relays(pubkey, Direction::Write)
                                        {
                                            if !outboxes.is_empty() {
                                                // But we have their outboxes
                                                if !app
                                                    .wizard_state
                                                    .followed_getting_metadata
                                                    .contains(&pubkey)
                                                {
                                                    // Add this key to the list of metadata to be updated
                                                    app.wizard_state
                                                        .followed_getting_metadata
                                                        .insert(pubkey.to_owned());
                                                }
                                                ui.label(
                                                    RichText::new("seeking metadata...").color(
                                                        app.theme.warning_marker_text_color(),
                                                    ),
                                                );
                                            } else {
                                                // We don't have outboxes... this will come. Following them triggered this.
                                                ui.label(
                                                    RichText::new("seeking relay list...").color(
                                                        app.theme.warning_marker_text_color(),
                                                    ),
                                                );
                                            }
                                        } else {
                                            // We don't have outboxes... this will come. Following them triggered this.
                                            ui.label(
                                                RichText::new("seeking relay list...")
                                                    .color(app.theme.warning_marker_text_color()),
                                            );
                                        }
                                    }
                                });
                                ui.add_space(3.0);
                                ui.label(
                                    GossipUi::richtext_from_person_nip05(&person.borrow()).weak(),
                                );
                            });

                            ui.vertical(|ui| {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Min)
                                        .with_cross_align(egui::Align::Center),
                                    |ui| {
                                        widgets::MoreMenu::simple(ui, app).show(
                                            ui,
                                            |ui, is_open| {
                                                // actions
                                                if ui.button("Remove").clicked() {
                                                    let _ =
                                                        GLOBALS.storage.remove_person_from_list(
                                                            &pubkey,
                                                            PersonList::Followed,
                                                            None,
                                                        );
                                                    *is_open = false;
                                                }
                                            },
                                        );
                                    },
                                );
                            });
                        });
                    },
                );
            }
        });

    // refresh pending metadata
    // TODO this is a workaround because the overlord would stop processing
    // if it hit a relay that wanted authentication
    let uitime = ctx.input(|i| i.time);
    if (app.wizard_state.followed_last_try + 5.0) < uitime {
        let list = app.wizard_state.followed_getting_metadata.drain();
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::UpdateMetadataInBulk(list.collect()));
        app.wizard_state.followed_getting_metadata.clear();
        app.wizard_state.followed_last_try = uitime;
    }

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(20.0);

    ui.horizontal(|ui| {
        ui.label("Follow Someone:");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::default()), |ui| {
            app.theme.accent_button_2_style(ui.style_mut());
            if ui.button("follow").clicked() {
                if let Ok(pubkey) = PublicKey::try_from_bech32_string(app.add_contact.trim(), true)
                {
                    let _ = GLOBALS.to_overlord.send(ToOverlordMessage::FollowPubkey(
                        pubkey,
                        PersonList::Followed,
                        true,
                    ));
                } else if let Ok(pubkey) =
                    PublicKey::try_from_hex_string(app.add_contact.trim(), true)
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
            ui.add_space(10.0);
            let response = text_edit_line!(app, app.add_contact)
                .desired_width(ui.available_width())
                .with_paste()
                .hint_text(
                    "Enter a key (bech32 npub1 or hex), or an nprofile, or a DNS id (user@domain)",
                )
                .show(ui)
                .response;
            if response.changed() {
                app.wizard_state.error = None;
            }
        });
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

    ui.with_layout(egui::Layout::right_to_left(egui::Align::default()), |ui| {
        if app.wizard_state.has_private_key {
            ui.scope(|ui| {
                if app.wizard_state.new_user {
                    app.theme.accent_button_1_style(ui.style_mut());
                } else {
                    app.theme.accent_button_2_style(ui.style_mut());
                }
                if ui.button("Publish and Finish").clicked() {
                    let _ = GLOBALS
                        .to_overlord
                        .send(ToOverlordMessage::PushPersonList(PersonList::Followed));

                    super::complete_wizard(app, ctx);
                }
            });

            ui.add_space(20.0);
            ui.scope(|ui| {
                if !app.wizard_state.new_user {
                    app.theme.accent_button_1_style(ui.style_mut());
                } else {
                    app.theme.accent_button_2_style(ui.style_mut());
                }
                if ui.button("Finish without publishing").clicked() {
                    super::complete_wizard(app, ctx);
                }
            });
        } else {
            app.theme.accent_button_1_style(ui.style_mut());
            if ui.button("Finish").clicked() {
                super::complete_wizard(app, ctx);
            }
        }
    });
}
