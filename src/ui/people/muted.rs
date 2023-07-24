use super::{GossipUi, Page};
use crate::globals::GLOBALS;
use crate::people::Person;
use crate::AVATAR_SIZE_F32;
use eframe::egui;
use egui::{Context, Image, RichText, ScrollArea, Sense, Ui, Vec2};
use std::sync::atomic::Ordering;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.add_space(30.0);

    let people: Vec<Person> = GLOBALS
        .people
        .get_all()
        .drain(..)
        .filter(|p| p.muted == 1)
        .collect();

    ui.heading(format!("People who are Muted ({})", people.len()));
    ui.add_space(10.0);

    ScrollArea::vertical()
        .override_scroll_delta(Vec2 {
            x: 0.0,
            y: app.current_scroll_offset,
        })
        .show(ui, |ui| {
            for person in people.iter() {
                if person.muted != 1 {
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
                        .add(Image::new(&avatar, Vec2 { x: size, y: size }).sense(Sense::click()))
                        .clicked()
                    {
                        app.set_page(Page::Person(person.pubkey));
                    };

                    ui.vertical(|ui| {
                        ui.label(RichText::new(GossipUi::pubkey_short(&person.pubkey)).weak());
                        GossipUi::render_person_name_line(app, ui, person);

                        if ui.button("UNMUTE").clicked() {
                            GLOBALS.people.mute(person.pubkey, false);
                        }
                    });
                });

                ui.add_space(4.0);

                ui.separator();
            }
        });
}
