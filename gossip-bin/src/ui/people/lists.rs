use super::{GossipUi, Page};
use eframe::egui;
use egui::{Context, Ui, Vec2};
use egui_winit::egui::vec2;
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
                    app.deleting_list = Some(list);
                }
            }
        });
    }
    if ui.button("Create a new list").clicked() {
        // FIXME -- prompt for a name with a popup, then create with:
        //   let _ = PersonList::allocate(name, None);
        GLOBALS
            .status_queue
            .write()
            .write("Person List Create is NOT YET IMPLEMENTED".to_string());
    }

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    if let Some(list) = app.deleting_list {
        const DLG_SIZE: Vec2 = vec2(250.0, 120.0);
        let ret = crate::ui::widgets::modal_popup(ui, DLG_SIZE, |ui| {
            ui.vertical_centered(|ui| {
                ui.label("Are you sure you want to delete:");
                ui.add_space(5.0);
                ui.heading(list.name());
                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        app.deleting_list = None;
                    }
                    if ui.button("Delete").clicked() {
                        let _ = GLOBALS
                            .to_overlord
                            .send(ToOverlordMessage::DeletePersonList(list));
                        app.deleting_list = None;
                    }
                });
            });
        });
        if ret.inner.clicked() {
            app.deleting_list = None;
        }
    }
}
