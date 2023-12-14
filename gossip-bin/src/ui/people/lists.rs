use crate::ui::widgets;

use super::{GossipUi, Page};
use eframe::egui;
use egui::{Context, Ui, Vec2};
use egui_winit::egui::{vec2, Label, RichText, Sense};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::{PersonList, PersonListMetadata, GLOBALS};

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    widgets::page_header(ui, Page::PeopleLists.name(), |ui| {
        if ui.button("Create a new list").clicked() {
            app.creating_list = true;
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
            for (list, metadata) in all_lists {
                let count = GLOBALS
                    .storage
                    .get_people_in_list(list)
                    .map(|v| v.len())
                    .unwrap_or(0);
                let row_response = widgets::list_entry::make_frame(ui).show(ui, |ui| {
                    ui.set_min_width(ui.available_width());

                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.add(Label::new(
                                RichText::new(metadata.title).heading().color(color),
                            ));

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                                if matches!(list, PersonList::Custom(_)) {
                                    if ui.link("delete list").clicked() {
                                        app.deleting_list = Some(list);
                                    }
                                    ui.add_space(20.0);
                                    if ui.link("rename list").clicked() {
                                        app.renaming_list = Some(list);
                                    }
                                }
                            });
                        });
                        ui.horizontal(|ui| {
                            ui.label(format!("Entries: {} ", count));
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
                ui.add(text_edit_line!(app, app.new_list_name).hint_text("list name"));
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            app.creating_list = false;
                            app.new_list_name.clear();
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::default()), |ui| {
                            if ui.button("Create").clicked() {
                                if !app.new_list_name.is_empty() {
                                    let metadata = PersonListMetadata {
                                        dtag: app.new_list_name.to_owned(),
                                        title: app.new_list_name.to_owned(),
                                        ..Default::default()
                                    };

                                    if let Err(e) =
                                        GLOBALS.storage.allocate_person_list(&metadata, None)
                                    {
                                        GLOBALS.status_queue.write().write(format!("{}", e));
                                    } else {
                                        app.creating_list = false;
                                        app.new_list_name.clear();
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
                ui.heading(metadata.title);
                ui.label("Enter new name:");
                ui.add(
                    text_edit_line!(app, app.new_list_name)
                        .hint_text("Enter new list name")
                        .desired_width(f32::INFINITY),
                );
                if ui.button("Rename").clicked() {
                    let _ =
                        GLOBALS
                            .storage
                            .rename_person_list(list, app.new_list_name.clone(), None);
                    app.renaming_list = None;
                }
            });
        });
        if ret.inner.clicked() {
            app.renaming_list = None;
            app.new_list_name = "".to_owned();
        }
    }
}
