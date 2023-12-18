use super::{GossipUi, Page};
use crate::ui::widgets;
use eframe::egui;
use egui::{Context, Ui, Vec2};
use egui_winit::egui::{vec2, Label, RichText, Sense};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::{FeedKind, PersonListMetadata, GLOBALS};
use nostr_types::Unixtime;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    widgets::page_header(ui, Page::PeopleLists.name(), |ui| {
        app.theme.accent_button_1_style(ui.style_mut());
        if ui.button("Create a new list").clicked() {
            app.creating_list = true;
            app.creating_list_first_run = true;
        }
    });

    let enable_scroll = true;

    let all_lists = GLOBALS
        .storage
        .get_all_person_list_metadata()
        .unwrap_or_default();
    let color = app.theme.accent_color();

    app.vert_scroll_area()
        .id_source("people_lists_scroll")
        .enable_scrolling(enable_scroll)
        .show(ui, |ui| {
            for (list, mut metadata) in all_lists {
                let row_response = widgets::list_entry::make_frame(ui).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());

                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.add(Label::new(
                                RichText::new(&metadata.title).heading().color(color),
                            ));
                            ui.label(format!("({})", metadata.len));
                            if metadata.favorite {
                                ui.add(Label::new(
                                    RichText::new("★")
                                        .color(app.theme.accent_complementary_color()),
                                ));
                            }
                            if metadata.private {
                                ui.add(Label::new(
                                    RichText::new("😎")
                                        .color(app.theme.accent_complementary_color()),
                                ));
                            }

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let len = metadata.len;
                                    super::list::render_more_list_actions(
                                        ui,
                                        app,
                                        list,
                                        &mut metadata,
                                        len,
                                        false,
                                    );
                                    ui.add_space(10.0);
                                    ui.visuals_mut().hyperlink_color = ui.visuals().text_color();
                                    if ui.link("View Feed").clicked() {
                                        app.set_page(ctx, Page::Feed(FeedKind::List(list, false)));
                                    }
                                },
                            );
                        });
                    });
                });
                if row_response
                    .response
                    .interact(Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    app.set_page(ctx, Page::PeopleList(list));
                }
            }
        });

    if let Some(list) = app.deleting_list {
        let metadata = GLOBALS
            .storage
            .get_person_list_metadata(list)
            .unwrap_or_default()
            .unwrap_or_default();

        const DLG_SIZE: Vec2 = vec2(250.0, 120.0);
        let ret = crate::ui::widgets::modal_popup(ui, DLG_SIZE, |ui| {
            ui.vertical(|ui| {
                ui.label("Are you sure you want to delete:");
                ui.add_space(10.0);
                ui.heading(metadata.title);
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            app.deleting_list = None;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::default()), |ui| {
                            if ui.button("Delete").clicked() {
                                let _ = GLOBALS
                                    .to_overlord
                                    .send(ToOverlordMessage::DeletePersonList(list));
                                app.deleting_list = None;
                            }
                        })
                    });
                });
            });
        });
        if ret.inner.clicked() {
            app.deleting_list = None;
        }
    } else if app.creating_list {
        const DLG_SIZE: Vec2 = vec2(250.0, 120.0);
        let ret = crate::ui::widgets::modal_popup(ui, DLG_SIZE, |ui| {
            ui.vertical(|ui| {
                ui.heading("New List");
                ui.add_space(10.0);
                let response = ui.add(text_edit_line!(app, app.new_list_name).hint_text("list name"));
                if app.creating_list_first_run {
                    response.request_focus();
                    app.creating_list_first_run = false;
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.add(widgets::Switch::onoff(&app.theme, &mut app.new_list_favorite));
                    ui.label("Favorite");
                });
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            app.creating_list = false;
                            app.new_list_name.clear();
                            app.new_list_favorite = false;
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::default()), |ui| {
                            app.theme.accent_button_1_style(ui.style_mut());
                            if ui.button("Create").clicked() {
                                if !app.new_list_name.is_empty() {
                                    let dtag = format!("pl{}", Unixtime::now().unwrap().0);
                                    let metadata = PersonListMetadata {
                                        dtag,
                                        title: app.new_list_name.to_owned(),
                                        favorite: app.new_list_favorite,
                                        ..Default::default()
                                    };

                                    if let Err(e) =
                                        GLOBALS.storage.allocate_person_list(&metadata, None)
                                    {
                                        GLOBALS.status_queue.write().write(format!("{}", e));
                                    } else {
                                        app.creating_list = false;
                                        app.new_list_name.clear();
                                        app.new_list_favorite = false;
                                    }
                                } else {
                                    GLOBALS
                                        .status_queue
                                        .write()
                                        .write("Person List name must not be empty".to_string());
                                }
                            }
                        });
                    });
                });
            });
        });
        if ret.inner.clicked() {
            app.creating_list = false;
            app.new_list_name.clear();
            app.new_list_favorite = false;
        }
    } else if let Some(list) = app.renaming_list {
        let metadata = GLOBALS
            .storage
            .get_person_list_metadata(list)
            .unwrap_or_default()
            .unwrap_or_default();

        const DLG_SIZE: Vec2 = vec2(250.0, 120.0);
        let ret = crate::ui::widgets::modal_popup(ui, DLG_SIZE, |ui| {
            ui.vertical(|ui| {
                ui.heading(&metadata.title);
                ui.add_space(10.0);
                ui.label("Enter new name:");
                ui.add_space(5.0);
                ui.add(
                    text_edit_line!(app, app.new_list_name)
                        .hint_text(metadata.title)
                        .desired_width(f32::INFINITY),
                );
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            app.renaming_list = None;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::default()), |ui| {
                            if ui.button("Rename").clicked() {
                                let _ = GLOBALS.storage.rename_person_list(
                                    list,
                                    app.new_list_name.clone(),
                                    None,
                                );
                                app.renaming_list = None;
                            }
                        });
                    });
                });
            });
        });
        if ret.inner.clicked() {
            app.renaming_list = None;
            app.new_list_name = "".to_owned();
        }
    }
}
