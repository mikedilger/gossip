use super::{widgets, GossipUi, Page};
use eframe::egui;
use egui::{Context, Label, RichText, Sense, Ui};
use gossip_lib::Person;

mod followers;
mod follows;
mod list;
mod lists;
mod person;

pub(in crate::ui) use list::layout_list_title;
pub(in crate::ui) use list::ListUi;
pub(in crate::ui) use lists::sort_lists;

pub(super) fn enter_page(app: &mut GossipUi) {
    if app.page == Page::PeopleLists {
        // nothing yet
    } else if let Page::PeopleList(plist) = app.page {
        list::enter_page(app, plist);
    } else if matches!(app.page, Page::Person(_)) {
        // nothing yet
    }
}

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    match app.page {
        Page::PeopleLists => lists::update(app, ctx, _frame, ui),
        Page::PeopleList(plist) => list::update(app, ctx, _frame, ui, plist),
        Page::Person(_) => person::update(app, ctx, _frame, ui),
        Page::PersonFollows(who) => follows::update(app, ctx, _frame, ui, who),
        Page::PersonFollowers(who) => followers::update(app, ctx, _frame, ui, who),
        _ => (),
    }
}

pub fn render_person_line(app: &mut GossipUi, ctx: &Context, ui: &mut Ui, person: Person) {
    let row_response = widgets::list_entry::clickable_frame(
        ui,
        app,
        Some(app.theme.main_content_bgcolor()),
        Some(app.theme.hovered_content_bgcolor()),
        |ui, app| {
            ui.horizontal(|ui| {
                // Avatar first
                let avatar = if let Some(avatar) = app.try_get_avatar(ctx, &person.pubkey) {
                    avatar
                } else {
                    app.placeholder_avatar.clone()
                };

                let mut response =
                    widgets::paint_avatar(ui, &person, &avatar, widgets::AvatarSize::Feed);

                ui.add_space(20.0);

                let response = ui
                    .vertical(|ui| {
                        ui.add_space(5.0);
                        ui.horizontal(|ui| {
                            response |= ui.add(
                                Label::new(RichText::new(person.best_name()).size(15.5))
                                    .selectable(false)
                                    .sense(Sense::click()),
                            );
                        });

                        ui.add_space(3.0);
                        response |= ui.add(
                            Label::new(GossipUi::richtext_from_person_nip05(&person).weak())
                                .selectable(false)
                                .sense(Sense::click()),
                        );
                        response
                    })
                    .inner;

                // This is just to force the background all the way across
                ui.vertical(|ui| {
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Min)
                            .with_cross_align(egui::Align::Center),
                        |_| {},
                    );
                });

                response
            })
            .inner
        },
    );

    // test what the height is
    // println!("HEIGHT = {}", row_response.inner.rect.height());

    if row_response
        .inner
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
    {
        app.set_page(ctx, Page::Person(person.pubkey));
    }
}
