use super::{GossipUi, Page};
use crate::comms::ToOverlordMessage;
use crate::globals::GLOBALS;
use crate::people::DbPerson;
use crate::AVATAR_SIZE_F32;
use eframe::egui;
use egui::{Context, Image, RichText, ScrollArea, SelectableLabel, Sense, Ui, Vec2};
use std::sync::atomic::Ordering;

mod follow;
mod muted;
mod person;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    let maybe_person = match &app.page {
        Page::Person(pubkeyhex) => GLOBALS.people.get(pubkeyhex),
        _ => None,
    };

    ui.horizontal(|ui| {
        if ui
            .add(SelectableLabel::new(
                app.page == Page::PeopleList,
                "Followed",
            ))
            .clicked()
        {
            app.set_page(Page::PeopleList);
        }
        ui.separator();
        if ui
            .add(SelectableLabel::new(
                app.page == Page::PeopleFollow,
                "Follow Someone New",
            ))
            .clicked()
        {
            app.set_page(Page::PeopleFollow);
        }
        ui.separator();
        if ui
            .add(SelectableLabel::new(app.page == Page::PeopleMuted, "Muted"))
            .clicked()
        {
            app.set_page(Page::PeopleMuted);
        }
        ui.separator();
        if let Some(person) = &maybe_person {
            if ui
                .add(SelectableLabel::new(
                    app.page == Page::Person(person.pubkey.clone()),
                    get_name(person),
                ))
                .clicked()
            {
                app.set_page(Page::Person(person.pubkey.clone()));
            }
            ui.separator();
        }
    });
    ui.separator();

    if app.page == Page::PeopleList {
        ui.add_space(24.0);

        ui.horizontal(|ui| {
            if ui.button("↓ PULL ↓\nOverwrite").clicked() {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::PullFollowOverwrite);
            }
            if ui.button("↓ PULL ↓\nMerge (Add)").clicked() {
                let _ = GLOBALS.to_overlord.send(ToOverlordMessage::PullFollowMerge);
            }

            #[allow(clippy::collapsible_if)]
            if GLOBALS.signer.is_ready() {
                if ui.button("↑ PUSH ↑\n").clicked() {
                    let _ = GLOBALS.to_overlord.send(ToOverlordMessage::PushFollow);
                }
            }

            if ui.button("Refresh\nMetadata").clicked() {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::RefreshFollowedMetadata);
            }
        });

        if !GLOBALS.signer.is_ready() {
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

        let people: Vec<DbPerson> = GLOBALS
            .people
            .get_all()
            .drain(..)
            .filter(|p| p.followed == 1)
            .collect();

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
                            ui.label(RichText::new(GossipUi::pubkey_short(&person.pubkey)).weak());
                            GossipUi::render_person_name_line(app, ui, person);
                        });
                    });

                    ui.add_space(4.0);

                    ui.separator();
                }
            });
    } else if app.page == Page::PeopleFollow {
        follow::update(app, ctx, _frame, ui);
    } else if app.page == Page::PeopleMuted {
        muted::update(app, ctx, _frame, ui);
    } else if matches!(app.page, Page::Person(_)) {
        person::update(app, ctx, _frame, ui);
    }
}

fn get_name(person: &DbPerson) -> String {
    if let Some(name) = person.name() {
        name.to_owned()
    } else {
        GossipUi::pubkey_short(&person.pubkey)
    }
}
