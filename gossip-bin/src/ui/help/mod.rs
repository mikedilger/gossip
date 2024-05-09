use eframe::egui;
use egui::{Context, Ui};
use gossip_lib::PersonList;

use super::{GossipUi, Page};

mod about;
mod stats;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    if app.page == Page::HelpHelp {
        ui.add_space(10.0);
        ui.heading("Troubleshooting");
        ui.add_space(12.0);

        ui.separator();

        app.vert_scroll_area().show(ui, |ui| {

            ui.add_space(10.0);
            ui.label("• HINT: Use the Back button in the Upper Left to come back to this page.");

            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                ui.label("• HINT: If this text is too small, click on");
                if ui.link("Settings").clicked() {
                    app.set_page(ctx, Page::Settings);
                }
                ui.label("and under the UI section, check \"Override DPI\" and set the value higher. You can press [Try it now] to see if you like it, and [SAVE CHANGES] to save that setting for next time.");
            });

            ui.add_space(10.0);
            ui.label("• HINT: Use CTRL-V to paste. Other unix-style pastes (e.g. middle mouse) probably won't work.");

            ui.add_space(10.0);
            ui.separator();

            ui.add_space(10.0);
            ui.heading("My followed list is empty:");
            ui.indent("contactlistempty", |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("1. Check to see if gossip has found the event by going to");
                    if ui.link("Followed").clicked() {
                        app.set_page(ctx, Page::PeopleList(PersonList::Followed));
                    }
                    ui.label("and at the top, check if REMOTE has a date. If so, press the Overwrite button to pull in the people you follow.");
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("2. Presuming the event wasn't there, go to");
                    if ui.link("My Relays").clicked() {
                        app.set_page(ctx, Page::RelaysMine);
                    }
                    ui.label("and make sure you have several relays on this page with 'W' turned on which have your contact list (kind 3) event on it. That line should also say '[Config]' on it. If not, click into it and check that Relay-picker rank is not 0. If you make changes, restart the client.");
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("3. Check the logs in the console and see if these relays are giving any kind of error.");
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("4. Double check that your contact list event is on one of these relays using some other nostr tool or service.");
                });
            });

            ui.add_space(10.0);
            ui.heading("Gossip freezes or runs slowly at startup:");
            ui.indent("freezeatstart", |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("On some combinations of storage devices and filesystems, gossip's LMDB storage may take a long time to get started. Try moving your gossip directory to a different filesyste and/or a different storage device. You can set the GOSSIP_DIR environment variable to point to your gossip directory." );
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("Your gossip storage directory is currently {}",
                                     app.about.storage_path));
                });
            });

            ui.add_space(10.0);
            ui.heading("more will be added in the future.");

            ui.add_space(10.0);
        });
    } else if app.page == Page::HelpStats {
        stats::update(app, ctx, _frame, ui);
    } else if app.page == Page::HelpAbout {
        about::update(app, ctx, _frame, ui);
    }
}
