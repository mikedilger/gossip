use super::{GossipUi, Page};
use crate::comms::ToOverlordMessage;
use crate::db::DbPerson;
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{Context, Image, RichText, ScrollArea, Sense, Ui, Vec2};

mod follow;
mod person;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    let maybe_person = match &app.page {
        Page::Person(pubkeyhex) => GLOBALS.people.get(pubkeyhex),
        _ => None,
    };

    ui.horizontal(|ui| {
        ui.selectable_value(&mut app.page, Page::PeopleList, "Followed");
        ui.separator();
        ui.selectable_value(&mut app.page, Page::PeopleFollow, "Follow Someone New");
        ui.separator();
        if let Some(person) = &maybe_person {
            ui.selectable_value(
                &mut app.page,
                Page::Person(person.pubkey.clone()),
                get_name(person),
            );
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
            /* not yet implemented
            if ui.button("↑ PUSH ↑\n").clicked() {
                let _ = GLOBALS.to_overlord.send(ToOverlordMessage::PushFollow);
            }
            */
        });

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        ui.heading("People Followed");
        ui.add_space(18.0);

        let people = GLOBALS.people.get_all();

        ScrollArea::vertical().show(ui, |ui| {
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
                    if ui
                        .add(
                            Image::new(
                                &avatar,
                                Vec2 {
                                    x: crate::AVATAR_SIZE_F32,
                                    y: crate::AVATAR_SIZE_F32,
                                },
                            )
                            .sense(Sense::click()),
                        )
                        .clicked()
                    {
                        app.set_page(Page::Person(person.pubkey.clone()));
                    };

                    ui.vertical(|ui| {
                        ui.label(RichText::new(GossipUi::hex_pubkey_short(&person.pubkey)).weak());
                        GossipUi::render_person_name_line(ui, Some(person));
                    });
                });

                ui.add_space(4.0);

                ui.separator();
            }
        });
    } else if app.page == Page::PeopleFollow {
        follow::update(app, ctx, _frame, ui);
    } else if matches!(app.page, Page::Person(_)) {
        person::update(app, ctx, _frame, ui);
    }
}

fn get_name(person: &DbPerson) -> String {
    if let Some(name) = &person.name {
        name.to_owned()
    } else {
        GossipUi::hex_pubkey_short(&person.pubkey)
    }
}
