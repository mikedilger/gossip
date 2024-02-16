use std::cmp::Ordering;

use super::{GossipUi, Page};
use crate::ui::widgets;
use eframe::egui;
use egui::{Context, Ui};
use egui_winit::egui::{Label, RichText, Sense};
use gossip_lib::{PersonList, PersonListMetadata, GLOBALS};

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    // process popups first
    let mut enabled = false;
    if let Some(list) = app.deleting_list {
        super::list::render_delete_list_dialog(ui, app, list);
    } else if app.creating_list {
        super::list::render_create_list_dialog(ui, app);
    } else if let Some(list) = app.renaming_list {
        super::list::render_rename_list_dialog(ui, app, list);
    } else {
        // only enable rest of ui when popups are not open
        enabled = true;
    }

    widgets::page_header(ui, Page::PeopleLists.name(), |ui| {
        ui.add_enabled_ui(enabled, |ui| {
            app.theme.accent_button_1_style(ui.style_mut());
            if ui.button("Create a new list").clicked() {
                app.creating_list = true;
                app.list_name_field_needs_focus = true;
            }
        });
    });

    ui.set_enabled(enabled);

    let mut all_lists = GLOBALS
        .storage
        .get_all_person_list_metadata()
        .unwrap_or_default();
    all_lists.sort_by(sort_lists);

    let color = app.theme.accent_color();

    app.vert_scroll_area()
        .id_source("people_lists_scroll")
        .enable_scrolling(enabled)
        .show(ui, |ui| {
            for (list, mut metadata) in all_lists {
                let row_response =
                    widgets::list_entry::make_frame(ui, Some(app.theme.main_content_bgcolor()))
                        .show(ui, |ui| {
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
                                                .size(18.0)
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
                                        },
                                    );
                                });
                            });
                        });

                if ui
                    .interact(
                        row_response.response.rect,
                        ui.next_auto_id(),
                        Sense::click(),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    app.set_page(ctx, Page::PeopleList(list));
                }
            }
            ui.add_space(crate::AVATAR_SIZE_F32 + 40.0);
        });
}

pub(in crate::ui) fn sort_lists(
    a: &(PersonList, PersonListMetadata),
    b: &(PersonList, PersonListMetadata),
) -> Ordering {
    if a.0 == PersonList::Followed {
        Ordering::Less
    } else if b.0 == PersonList::Followed {
        Ordering::Greater
    } else if a.0 == PersonList::Muted {
        Ordering::Less
    } else if b.0 == PersonList::Muted {
        Ordering::Greater
    } else {
        b.1.favorite
            .cmp(&a.1.favorite)
            .then(a.1.title.to_lowercase().cmp(&b.1.title.to_lowercase()))
    }
}
