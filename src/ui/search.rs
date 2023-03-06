use super::{GossipUi, Page};
use crate::feed::FeedKind;
use crate::GLOBALS;
use eframe::{egui, Frame};
use egui::{Context, Ui};
use nostr_types::{Id, PublicKey};

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut Frame, ui: &mut Ui) {
    ui.heading("Search");

    ui.add_space(12.0);

    let mut do_search = false;

    ui.horizontal(|ui| {
        ui.label("🔎");
        let response = ui.add(
            text_edit_line!(app, app.search)
                .hint_text("npub1 or note1, other kinds of searches not yet implemented")
                .desired_width(f32::INFINITY),
        );
        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            do_search = true;
        }
    });

    if do_search {
        search_result(app, ctx, ui);
    }

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(12.0);

    ui.horizontal_wrapped(|ui| {
        ui.label(&app.search_result);
    });
}

fn search_result(app: &mut GossipUi, _ctx: &Context, _ui: &mut Ui) {
    // Maybe go to note
    if app.search.starts_with("note1") {
        if let Ok(id) = Id::try_from_bech32_string(&app.search) {
            app.search = "".to_owned();
            app.search_result = "".to_owned();
            app.set_page(Page::Feed(FeedKind::Thread {
                id,
                referenced_by: id,
            }));
            return;
        } else {
            app.search_result = "Looks like an event Id, but it isn't.".to_owned();
            return;
        }
    }

    // Maybe go to a person
    if app.search.starts_with("npub1") {
        if let Ok(pk) = PublicKey::try_from_bech32_string(&app.search) {
            // Do we have the person?
            if GLOBALS.people.get(&pk.into()).is_some() {
                app.search = "".to_owned();
                app.search_result = "".to_owned();
                app.set_page(Page::Person(pk.into()));
                return;
            } else {
                app.search_result = "Public key recognized, but not in memory. Background task is trying to load them from the database.  You might try again if you think they will be loaded.".to_owned();
                return;
            }
        } else {
            app.search_result = "Looks like a public key, but it isn't.".to_owned();
            return;
        }
    }

    // If it is a hexadecimal string
    if hex::decode(&app.search).is_ok() {
        // Try it as a public key first (only succeeds if person is in memory)
        if let Ok(pk) = PublicKey::try_from_hex_string(&app.search) {
            if GLOBALS.people.get(&pk.into()).is_some() {
                app.search = "".to_owned();
                app.search_result = "".to_owned();
                app.set_page(Page::Person(pk.into()));
                return;
            }
        }

        // Try it as an event Id next
        if let Ok(id) = Id::try_from_hex_string(&app.search) {
            app.search = "".to_owned();
            app.search_result = "".to_owned();
            app.set_page(Page::Feed(FeedKind::Thread {
                id,
                referenced_by: id,
            }));
            return;
        }
    }

    // Next, search the text contents of all notes in the database
    // and display a list of them, so the user can choose.
    // TBD.

    // If nothing worked, let them know.
    app.search_result = format!("No result for {}.\n\nFulltext search and nym search are not yet implemented, only note1 and npub1.", app.search.clone());
    app.search = "".to_owned();
}
