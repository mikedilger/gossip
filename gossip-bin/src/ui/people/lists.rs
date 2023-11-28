use super::{GossipUi, Page};
use eframe::egui;
use egui::{Context, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::{PersonList, GLOBALS};

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.add_space(10.0);
    ui.horizontal_wrapped(|ui| {
        ui.add_space(2.0);
        ui.heading("Lists");
    });

    ui.add_space(10.0);

    let all_lists = PersonList::all_lists();
    for (list, listname) in all_lists {
        let count = GLOBALS
            .storage
            .get_people_in_list(list)
            .map(|v| v.len())
            .unwrap_or(0);
        ui.horizontal(|ui| {
            ui.label(format!("({}) ", count));
            if ui.link(listname).clicked() {
                app.set_page(ctx, Page::PeopleList(list));
            };
            if matches!(list, PersonList::Custom(_)) {
                if ui.button("DELETE").clicked() {
                    // FIXME -- confirm with a popup, then call the delete() function (see below)
                    GLOBALS
                        .status_queue
                        .write()
                        .write("Person List Delete is NOT YET IMPLEMENTED".to_string());
                }
            }
        });
    }
    if ui.button("Create a new list").clicked() {
        // FIXME -- prompt for a name with a popup, then call the create() function (see below)
        GLOBALS
            .status_queue
            .write()
            .write("Person List Create is NOT YET IMPLEMENTED".to_string());
    }

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);
}

fn delete(list: PersonList) {
    let _ = GLOBALS
        .to_overlord
        .send(ToOverlordMessage::ClearPersonList(list));
    let _ = list.deallocate(None);
}

fn create(name: &str) {
    let _ = PersonList::allocate(name, None);
}
