use eframe::egui;
use egui::{Context, Ui};

use super::{GossipUi, Page};

mod list;
mod lists;
mod person;

pub(in crate::ui) use list::{layout_list_title, ListUi};
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
    if app.page == Page::PeopleLists {
        lists::update(app, ctx, _frame, ui);
    } else if let Page::PeopleList(plist) = app.page {
        list::update(app, ctx, _frame, ui, plist);
    } else if matches!(app.page, Page::Person(_)) {
        person::update(app, ctx, _frame, ui);
    }
}
