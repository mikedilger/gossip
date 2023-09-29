use super::{GossipUi, Page};
use crate::comms::ToOverlordMessage;
use crate::feed::FeedKind;
use crate::AVATAR_SIZE_F32;
use crate::GLOBALS;
use eframe::{egui, Frame};
use egui::widgets::Button;
use egui::{Context, Image, Label, RichText, Sense, Ui, Vec2};
use std::sync::atomic::Ordering;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut Frame, ui: &mut Ui) {
    ui.heading("Search notes and users");

    ui.add_space(12.0);

    let mut trigger_search = false;

    ui.horizontal(|ui| {
        let response = ui.add(
            text_edit_line!(app, app.search)
                .hint_text("Search for People and Notes")
                .desired_width(600.0),
        );

        if app.entering_search_page {
            response.request_focus();
            app.entering_search_page = false;
        }

        if ui.add(Button::new("Search")).clicked() {
            trigger_search = true;
        }
        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            trigger_search = true;
        }
    });

    if trigger_search {
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::Search(app.search.clone()));
    }

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(12.0);

    let people = GLOBALS.people_search_results.read().clone();
    let notes = GLOBALS.note_search_results.read().clone();

    app.vert_scroll_area().show(ui, |ui| {
        if !people.is_empty() {
            for person in people.iter() {
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

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
                        ui.label(RichText::new(crate::names::pubkey_short(&person.pubkey)).weak());
                        GossipUi::render_person_name_line(app, ui, person);
                    });
                });
            }
        }

        if !notes.is_empty() {
            for event in notes.iter() {
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(crate::date_ago::date_ago(event.created_at))
                            .italics()
                            .weak(),
                    );

                    if let Ok(Some(person)) = GLOBALS.storage.read_person(&event.pubkey) {
                        GossipUi::render_person_name_line(app, ui, &person);
                    } else {
                        ui.label(event.pubkey.as_bech32_string());
                    }
                });

                let mut summary = event
                    .content
                    .get(0..event.content.len().min(100))
                    .unwrap_or("...")
                    .replace('\n', " ");

                if summary.is_empty() {
                    // Show something they can click on anyways
                    summary = "[no event summary]".to_owned();
                }

                if ui.add(Label::new(summary).sense(Sense::click())).clicked() {
                    app.set_page(Page::Feed(FeedKind::Thread {
                        id: event.id,
                        referenced_by: event.id,
                        author: Some(event.pubkey),
                    }));
                }
            }
        }

        if people.is_empty() && notes.is_empty() {
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            ui.label("No results found.");
        }
    });
}
