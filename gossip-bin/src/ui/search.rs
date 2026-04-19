use super::{widgets, GossipUi, Page};
use eframe::{egui, Frame};
use egui::widgets::Button;
use egui::{Context, Label, RichText, Sense, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::namecoin::{NamecoinLookupError, NAMECOIN_RESOLVER};
use gossip_lib::{FeedKind, Person, PersonTable, Relay, Table, GLOBALS};
use nostr_types::{Event, PublicKey};
use std::sync::atomic::Ordering;

/// State of the Namecoin-based search path, independent of the existing
/// local/relay search flows.
#[derive(Debug, Clone)]
pub(crate) enum NamecoinSearchState {
    Idle,
    Loading,
    Resolved { pubkey: PublicKey, name: String },
    NotFound,
    Expired,
    Unreachable(String),
}

pub(super) fn update(
    app: &mut GossipUi,
    ctx: &Context,
    frame: &mut Frame,
    ui: &mut Ui,
    local: bool,
) {
    ui.add_space(10.0);
    if local {
        ui.heading("Search notes in local database (and users anywhere)");
    } else {
        ui.heading("Search notes on search relays");
    }

    // Warn if there are no search relays configured
    if !local {
        let search_relays: Vec<Relay> = GLOBALS
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

    ui.add_space(12.0);

    let mut trigger_search = false;

    ui.horizontal(|ui| {
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

    if trigger_search {
        if local {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::SearchLocally(app.search.clone()));
        } else {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::SearchRelays(app.search.clone()));
        }

        // Parallel Namecoin resolution if the query looks Namecoin-y.
        maybe_trigger_namecoin(app);
    }

    // Render any Namecoin resolution status under the search bar.
    render_namecoin_state(app, ctx, ui);

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(12.0);

    let people = GLOBALS.people_search_results.read().clone();
    let notes = GLOBALS.note_search_results.read().clone();

    app.vert_scroll_area().show(ui, |ui| {
        if !people.is_empty() {
            for person in people.iter() {
                render_searched_person_maybe_fake(app, ctx, frame, ui, person);
            }
        }

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

// If offscreen, this just makes blank space so the scrollbar doesn't get messed up.
// By doing this, we increase performance by a lot (not all the avatars need fetching
// at the same time)
fn render_searched_person_maybe_fake(
    app: &mut GossipUi,
    ctx: &Context,
    _frame: &mut Frame,
    ui: &mut Ui,
    person: &Person,
) {
    let screen_rect = ctx.input(|i| i.screen_rect); // Rect
    let pos2 = ui.next_widget_position();
    let height = match app.search_person_height.get(&person.pubkey) {
        Some(h) => *h,
        None => {
            let top = ui.next_widget_position();
            render_searched_person(app, ctx, ui, person);
            let bottom = ui.next_widget_position();
            app.search_person_height
                .insert(person.pubkey, bottom.y - top.y);
            return;
        }
    };

    let after_the_bottom = pos2.y > screen_rect.max.y;
    let before_the_top = pos2.y + height < 0.0;
    if after_the_bottom || before_the_top {
        // Don't actually render, just make space for scrolling purposes
        ui.add_space(height);
    } else {
        render_searched_person(app, ctx, ui, person);
    }
}

fn render_searched_person(app: &mut GossipUi, ctx: &Context, ui: &mut Ui, person: &Person) {
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        // Avatar first
        let avatar = if let Some(avatar) = app.try_get_avatar(ctx, &person.pubkey) {
            avatar
        } else {
            app.placeholder_avatar.clone()
        };
        if widgets::paint_avatar(ui, person, &avatar, widgets::AvatarSize::Feed).clicked() {
            app.set_page(ctx, Page::Person(person.pubkey));
        };

        ui.vertical(|ui| {
            ui.label(RichText::new(gossip_lib::names::pubkey_short(&person.pubkey)).weak());
            GossipUi::render_person_name_line(app, ui, person, false);
        });
    });
}

// If offscreen, this just makes blank space so the scrollbar doesn't get messed up.
// By doing this, we increase performance by a lot (not all the avatars need fetching
// at the same time)
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

/// Spawn a background Namecoin resolution if the current search query looks
/// like a Namecoin identifier and we haven't already resolved it.
fn maybe_trigger_namecoin(app: &mut GossipUi) {
    let query = app.search.trim().to_string();
    if query.is_empty() || !NAMECOIN_RESOLVER.is_namecoin_identifier_static(&query) {
        // Reset state if user cleared / changed to a non-namecoin query
        if app.namecoin_last_query.is_some() {
            *app.namecoin_search_state.write().unwrap() = NamecoinSearchState::Idle;
            app.namecoin_last_query = None;
        }
        return;
    }

    // Avoid re-spawning for the same query.
    if app.namecoin_last_query.as_deref() == Some(query.as_str()) {
        let state = app.namecoin_search_state.read().unwrap();
        if !matches!(*state, NamecoinSearchState::Idle) {
            return;
        }
    }

    app.namecoin_last_query = Some(query.clone());
    *app.namecoin_search_state.write().unwrap() = NamecoinSearchState::Loading;

    let state_handle = app.namecoin_search_state.clone();
    GLOBALS.runtime.spawn(async move {
        let result = NAMECOIN_RESOLVER.resolve(&query).await;
        let new_state = match result {
            Ok(r) => NamecoinSearchState::Resolved {
                pubkey: r.pubkey,
                name: r.identifier.name,
            },
            Err(NamecoinLookupError::NameNotFound) => NamecoinSearchState::NotFound,
            Err(NamecoinLookupError::NameExpired) => NamecoinSearchState::Expired,
            Err(NamecoinLookupError::ServersUnreachable(s)) => {
                NamecoinSearchState::Unreachable(s)
            }
            Err(NamecoinLookupError::Parse(s)) => NamecoinSearchState::Unreachable(s),
        };
        *state_handle.write().unwrap() = new_state;
    });
}

/// Render the Namecoin search status line directly under the search bar.
fn render_namecoin_state(app: &mut GossipUi, ctx: &Context, ui: &mut Ui) {
    let state = { app.namecoin_search_state.read().unwrap().clone() };
    match state {
        NamecoinSearchState::Idle => {}
        NamecoinSearchState::Loading => {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Resolving via Namecoin blockchain…");
            });
            // Keep repainting while we're waiting so the state flip shows up
            // without requiring user input.
            ctx.request_repaint();
        }
        NamecoinSearchState::Resolved { pubkey, name } => {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("🔗 Namecoin ({name})"))
                        .color(egui::Color32::from_rgb(80, 170, 80)),
                );
                // Try to get a full Person record if we have one, otherwise
                // fall back to a plain clickable pubkey label.
                let maybe_person = PersonTable::read_record(pubkey, None).ok().flatten();
                if let Some(person) = maybe_person {
                    let label_text = gossip_lib::names::pubkey_short(&person.pubkey);
                    if ui
                        .add(Label::new(RichText::new(label_text).strong()).sense(Sense::click()))
                        .clicked()
                    {
                        app.set_page(ctx, Page::Person(person.pubkey));
                    }
                    GossipUi::render_person_name_line(app, ui, &person, false);
                } else {
                    let short = gossip_lib::names::pubkey_short(&pubkey);
                    if ui
                        .add(Label::new(RichText::new(short).strong()).sense(Sense::click()))
                        .clicked()
                    {
                        app.set_page(ctx, Page::Person(pubkey));
                    }
                }
            });
        }
        NamecoinSearchState::NotFound => {
            ui.add_space(6.0);
            ui.label(
                RichText::new("Namecoin: name not found on-chain")
                    .color(egui::Color32::from_rgb(200, 120, 120)),
            );
        }
        NamecoinSearchState::Expired => {
            ui.add_space(6.0);
            ui.label(
                RichText::new("Namecoin: name has expired")
                    .color(egui::Color32::from_rgb(200, 120, 120)),
            );
        }
        NamecoinSearchState::Unreachable(s) => {
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!("Namecoin: unreachable — {s}"))
                    .color(egui::Color32::from_rgb(200, 120, 120)),
            );
        }
    }
}

/// Helper so we don't have to expose the internal `NamecoinIdentifier` type.
/// (The static `NAMECOIN_RESOLVER` is a `NamecoinResolver`, which has the
/// associated fn `is_namecoin_identifier`; but it's not a method on the
/// value. We wrap it here for ergonomics.)
trait NamecoinResolverExt {
    fn is_namecoin_identifier_static(&self, s: &str) -> bool;
}

impl NamecoinResolverExt for gossip_lib::namecoin::NamecoinResolver {
    fn is_namecoin_identifier_static(&self, s: &str) -> bool {
        gossip_lib::namecoin::NamecoinResolver::is_namecoin_identifier(s)
    }
}

fn render_searched_note(app: &mut GossipUi, ctx: &Context, ui: &mut Ui, event: &Event) {
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        ui.label(
            RichText::new(crate::date_ago::date_ago(event.created_at))
                .italics()
                .weak(),
        );

        if let Ok(Some(person)) = PersonTable::read_record(event.pubkey, None) {
            GossipUi::render_person_name_line(app, ui, &person, false);
        } else {
            ui.label(event.pubkey.as_bech32_string());
        }
    });

    let mut summary = event
        .content
        .get(0..event.content.len().min(100))
        .unwrap_or("...")
        .replace('\n', " ");

    if summary.is_empty() {
        // Show something they can click on anyways
        summary = "[no event summary]".to_owned();
    }

    if ui.add(Label::new(summary).sense(Sense::click())).clicked() {
        app.set_page(
            ctx,
            Page::Feed(FeedKind::Thread {
                id: event.id,
                referenced_by: event.id,
                author: Some(event.pubkey),
            }),
        );
    }
}
