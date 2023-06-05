use super::{GossipUi, Page};
use crate::comms::ToOverlordMessage;
use crate::globals::GLOBALS;
use crate::people::DbPerson;
use crate::AVATAR_SIZE_F32;
use eframe::egui;
use egui::{Context, Image, RichText, ScrollArea, Sense, Ui, Vec2};
use std::sync::atomic::Ordering;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    let people: Vec<DbPerson> = GLOBALS
        .people
        .get_all()
        .drain(..)
        .filter(|p| p.followed == 1)
        .collect();

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

        if ui.button("↓ Overwrite ↓").clicked() {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::UpdateFollowing(false));
        }
        if ui.button("↓ Merge ↓").clicked() {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::UpdateFollowing(true));
        }

        if GLOBALS.signer.is_ready() {
            if ui.button("↑ Publish ↑").clicked() {
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

        if ui.button("Refresh Metadata").clicked() {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::RefreshFollowedMetadata);
        }
    });

    ui.add_space(10.0);

    let last_contact_list_edit = GLOBALS
        .people
        .last_contact_list_edit
        .load(Ordering::Relaxed);
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

    ScrollArea::vertical()
        .override_scroll_delta(Vec2 {
            x: 0.0,
            y: app.current_scroll_offset,
        })
        .show(ui, |ui| {
            for person in people.iter() {
                if person.followed != 1 {
                    continue;
                }

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
                            Image::new(&avatar, Vec2 { x: size, y: size })
                                .sense(Sense::click()),
                        )
                        .clicked()
                    {
                        app.set_page(Page::Person(person.pubkey.clone()));
                    };

                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(GossipUi::pubkeyhex_convert_short(&person.pubkey))
                                .weak(),
                        );
                        GossipUi::render_person_name_line(app, ui, person);
                    });
                });

                ui.add_space(4.0);

                ui.separator();
            }
        });
}
