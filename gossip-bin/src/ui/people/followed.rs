use super::{GossipUi, Page};
use crate::AVATAR_SIZE_F32;
use eframe::egui;
use egui::{Context, Image, RichText, Sense, Ui, Vec2};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::{GLOBALS, Person, PersonList};
use std::sync::atomic::Ordering;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    let followed_pubkeys = GLOBALS.storage.get_people_in_list(PersonList::Followed, None)
        .unwrap_or(vec![]);
    let mut people: Vec<Person> = Vec::new();
    for pk in &followed_pubkeys {
        if let Ok(Some(person)) = GLOBALS.storage.read_person(pk) {
            people.push(person);
        } else {
            let person = Person::new(pk.to_owned());
            let _ = GLOBALS.storage.write_person(&person, None);
            people.push(person);
        }
    }
    people.sort();

    ui.add_space(12.0);

    let last_contact_list_size = GLOBALS
        .people
        .last_contact_list_size
        .load(Ordering::Relaxed);
    let last_contact_list_asof = GLOBALS
        .people
        .last_contact_list_asof
        .load(Ordering::Relaxed);
    let mut asof = "unknown".to_owned();
    if let Ok(stamp) = time::OffsetDateTime::from_unix_timestamp(last_contact_list_asof) {
        if let Ok(formatted) = stamp.format(time::macros::format_description!(
            "[year]-[month repr:short]-[day] ([weekday repr:short]) [hour]:[minute]"
        )) {
            asof = formatted;
        }
    }

    ui.label(
        RichText::new(format!(
            "REMOTE: {} (size={})",
            asof, last_contact_list_size
        ))
        .size(15.0),
    )
    .on_hover_text("This is the data in the latest ContactList event fetched from relays");

    ui.add_space(10.0);

    ui.horizontal(|ui| {
        ui.add_space(30.0);

        if ui
            .button("↓ Overwrite ↓")
            .on_hover_text(
                "This pulls down your Contact List, erasing anything that is already here",
            )
            .clicked()
        {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::UpdateFollowing { merge: false });
        }
        if ui
            .button("↓ Merge ↓")
            .on_hover_text(
                "This pulls down your Contact List, merging it into what is already here",
            )
            .clicked()
        {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::UpdateFollowing { merge: true });
        }

        if GLOBALS.signer.is_ready() {
            if ui
                .button("↑ Publish ↑")
                .on_hover_text("This publishes your Contact List")
                .clicked()
            {
                let _ = GLOBALS.to_overlord.send(ToOverlordMessage::PushFollow);
            }
        }

        if GLOBALS.signer.is_ready() {
            if app.follow_clear_needs_confirm {
                if ui.button("CANCEL").clicked() {
                    app.follow_clear_needs_confirm = false;
                }
                if ui.button("YES, CLEAR ALL").clicked() {
                    let _ = GLOBALS.to_overlord.send(ToOverlordMessage::ClearFollowing);
                    app.follow_clear_needs_confirm = false;
                }
            } else {
                if ui.button("Clear All").clicked() {
                    app.follow_clear_needs_confirm = true;
                }
            }
        }

        if ui
            .button("Refresh Metadata")
            .on_hover_text(
                "This will seek out metadata (name, avatar, etc) on each person in the list below",
            )
            .clicked()
        {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::RefreshSubscribedMetadata);
        }
    });

    ui.add_space(10.0);

    let last_contact_list_edit = match GLOBALS.storage.read_last_contact_list_edit() {
        Ok(date) => date,
        Err(e) => {
            tracing::error!("{}", e);
            0
        }
    };

    let mut ledit = "unknown".to_owned();
    if let Ok(stamp) = time::OffsetDateTime::from_unix_timestamp(last_contact_list_edit) {
        if let Ok(formatted) = stamp.format(time::macros::format_description!(
            "[year]-[month repr:short]-[day] ([weekday repr:short]) [hour]:[minute]"
        )) {
            ledit = formatted;
        }
    }
    ui.label(RichText::new(format!("LOCAL: {} (size={})", ledit, people.len())).size(15.0))
        .on_hover_text("This is the local (and effective) following list");

    if !GLOBALS.signer.is_ready() {
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            ui.label("You need to ");
            if ui.link("setup your identity").clicked() {
                app.set_page(Page::YourKeys);
            }
            ui.label(" to push.");
        });
    }

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    ui.heading(format!("People Followed ({})", people.len()));
    ui.add_space(18.0);

    app.vert_scroll_area().show(ui, |ui| {
        for person in people.iter() {
            ui.horizontal(|ui| {
                // Avatar first
                let avatar = if let Some(avatar) = app.try_get_avatar(ctx, &person.pubkey) {
                    avatar
                } else {
                    app.placeholder_avatar.clone()
                };
                let size = AVATAR_SIZE_F32
                    * GLOBALS.pixels_per_point_times_100.load(Ordering::Relaxed) as f32
                    / 100.0;
                if ui
                    .add(
                        Image::new(&avatar)
                            .max_size(Vec2 { x: size, y: size })
                            .maintain_aspect_ratio(true)
                            .sense(Sense::click()),
                    )
                    .clicked()
                {
                    app.set_page(Page::Person(person.pubkey));
                };

                ui.vertical(|ui| {
                    ui.label(RichText::new(gossip_lib::names::pubkey_short(&person.pubkey)).weak());
                    GossipUi::render_person_name_line(app, ui, person, false);
                    if !GLOBALS
                        .storage
                        .have_persons_relays(person.pubkey)
                        .unwrap_or(false)
                    {
                        ui.label(
                            RichText::new("Relay list not found")
                                .color(app.theme.warning_marker_text_color()),
                        );
                    }
                });
            });

            ui.add_space(4.0);

            ui.separator();
        }
    });
}
