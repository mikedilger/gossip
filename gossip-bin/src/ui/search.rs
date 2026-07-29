use crate::ui::widgets::Switch;

use super::{GossipUi, Page};
use eframe::egui::RichText;
use eframe::{egui, Frame};
use egui::widgets::Button;
use egui::{Context, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::{Relay, GLOBALS};
use nostr_types::Event;
use std::sync::atomic::Ordering;

pub(super) fn update(
    app: &mut GossipUi,
    ctx: &Context,
    frame: &mut Frame,
    ui: &mut Ui,
    local: bool,
) {
    ui.add_space(10.0);

    let mut trigger_search = false;

    ui.horizontal(|ui| {
        if local {
            ui.heading("Search notes in local database");
        } else {
            ui.heading("Search notes on search relays");
        }

        // Warn if there are no search relays configured
        if !local {
            let search_relays = GLOBALS
                .db()
                .filter_relays(|relay| relay.has_usage_bits(Relay::SEARCH))
                .unwrap_or_default();

            if search_relays.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.label("You must first configure SEARCH relays on the ");
                    if ui.link("relays").clicked() {
                        app.set_page(ctx, Page::RelaysKnownNetwork(None));
                    }
                    ui.label(" page.");
                });
                return;
            }
        }

        ui.separator();

        ui.label(RichText::new("by me"));
        if Switch::large(&app.theme, &mut app.is_search_by_me)
            .show(ui)
            .clicked()
        {
            trigger_search = true;
        }

        let response = ui.add(
            text_edit_line!(app, app.search)
                .hint_text("Search for People and Notes")
                .desired_width(600.0),
        );

        if app.entering_a_search_page {
            // Focus on the search input
            response.request_focus();

            app.entering_a_search_page = false;
        }

        if ui.add(Button::new("Search")).clicked() {
            trigger_search = true;
        }

        if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            trigger_search = true;
        }
    });

    ui.add_space(12.0);

    if trigger_search {
        let public_keys = if app.is_search_by_me {
            GLOBALS
                .identity
                .public_key()
                .map(|public_key| vec![public_key])
        } else {
            None
        };
        if local {
            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::SearchLocally(
                app.search.clone(),
                public_keys,
            ));
        } else {
            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::SearchRelays(
                app.search.clone(),
                public_keys,
            ));
        }
    }

    ui.add_space(12.0);

    let people = GLOBALS.people_search_results.read().clone(); // @TODO implement widgets
    let notes = GLOBALS.note_search_results.read().clone();

    app.vert_scroll_area().auto_shrink(false).show(ui, |ui| {
        /* @TODO implement compact view
        if !people.is_empty() {
            for person in people.iter() {
                render_searched_person_maybe_fake(app, ctx, frame, ui, person);
            }
        }*/

        if !notes.is_empty() {
            for event in notes.iter() {
                render_searched_note_maybe_fake(app, ctx, frame, ui, event);
            }
        }

        if GLOBALS.searching.load(Ordering::Relaxed) {
            app.search_started = true;
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
            ui.label("Searching...");
        } else if app.search_started && people.is_empty() && notes.is_empty() {
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
            ui.label("No results found.");
        }
        // else the results are showing
    });
}

fn render_searched_note_maybe_fake(
    app: &mut GossipUi,
    ctx: &Context,
    _frame: &mut Frame,
    ui: &mut Ui,
    event: &Event,
) {
    let screen_rect = ctx.input(|i| i.screen_rect); // Rect
    let pos2 = ui.next_widget_position();
    let height = match app.search_note_height.get(&event.id) {
        Some(h) => *h,
        None => {
            let top = ui.next_widget_position();
            render_searched_note(app, ctx, ui, event);
            let bottom = ui.next_widget_position();
            app.search_note_height.insert(event.id, bottom.y - top.y);
            return;
        }
    };

    let after_the_bottom = pos2.y > screen_rect.max.y;
    let before_the_top = pos2.y + height < 0.0;
    if after_the_bottom || before_the_top {
        // Don't actually render, just make space for scrolling purposes
        ui.add_space(height);
    } else {
        render_searched_note(app, ctx, ui, event);
    }
}

fn render_searched_note(app: &mut GossipUi, ctx: &Context, ui: &mut Ui, event: &Event) {
    super::feed::note::render_note(
        app,
        ctx,
        ui,
        super::feed::FeedNoteParams {
            id: event.id,
            indent: 0,
            as_reply_to: false,
            threaded: false,
        },
    )
}
