use super::theme::FeedProperties;
use super::{widgets, GossipUi, Page};
use eframe::egui;
use egui::{Context, Frame, RichText, Ui, Vec2};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::FeedKind;
use gossip_lib::GLOBALS;
use nostr_types::Id;
use std::sync::atomic::Ordering;

mod notedata;

mod notes;
pub use notes::Notes;

mod note;
pub use note::NoteRenderData;
pub(super) mod post;

struct FeedNoteParams {
    id: Id,
    indent: usize,
    as_reply_to: bool,
    threaded: bool,
    is_first: bool,
    is_last: bool,
}

pub(super) fn update(app: &mut GossipUi, ctx: &Context, frame: &mut eframe::Frame, ui: &mut Ui) {
    if GLOBALS.ui_invalidate_all.load(Ordering::Relaxed) {
        app.notes.cache_invalidate_all();
        GLOBALS.ui_invalidate_all.store(false, Ordering::Relaxed);
    } else {
        // Do per-note invalidations
        if !GLOBALS.ui_notes_to_invalidate.read().is_empty() {
            let mut handle = GLOBALS.ui_notes_to_invalidate.write();
            for id in handle.iter() {
                app.notes.cache_invalidate_note(id);
            }
            *handle = Vec::new();
        }

        // Do per-person invalidations
        if !GLOBALS.ui_people_to_invalidate.read().is_empty() {
            let mut handle = GLOBALS.ui_people_to_invalidate.write();
            for pkh in handle.iter() {
                app.notes.cache_invalidate_person(pkh);
            }
            *handle = Vec::new();
        }
    }

    let feed_kind = GLOBALS.feed.get_feed_kind();

    match feed_kind {
        FeedKind::List(list, with_replies) => {
            let metadata = GLOBALS
                .storage
                .get_person_list_metadata(list)
                .unwrap_or_default()
                .unwrap_or_default();

            let feed = GLOBALS.feed.get_followed();
            let id = format!(
                "{} {}",
                Into::<u8>::into(list),
                if with_replies { "main" } else { "general" }
            );
            ui.add_space(10.0);
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), ui.spacing().interact_size.y),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    add_left_space(ui);
                    let title_job = super::people::layout_list_title(ui, app, &metadata);
                    ui.label(title_job);
                    recompute_btn(app, ui);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(10.0);

                        if ui.button("Edit List").clicked() {
                            app.set_page(ctx, Page::PeopleList(list));
                        }

                        ui.add_space(10.0);
                        ui.label(RichText::new("Include replies").size(11.0));
                        let size = ui.spacing().interact_size.y * egui::vec2(1.6, 0.8);
                        if widgets::switch_with_size(ui, &mut app.mainfeed_include_nonroot, size)
                            .clicked()
                        {
                            app.set_page(
                                ctx,
                                Page::Feed(FeedKind::List(list, app.mainfeed_include_nonroot)),
                            );
                            ctx.data_mut(|d| {
                                d.insert_persisted(
                                    egui::Id::new("mainfeed_include_nonroot"),
                                    app.mainfeed_include_nonroot,
                                );
                            });
                        }
                        ui.label(RichText::new("Main posts").size(11.0));
                    });
                },
            );
            ui.add_space(6.0);
            render_a_feed(app, ctx, frame, ui, feed, false, &id);
        }
        FeedKind::Inbox(indirect) => {
            if app.settings.public_key.is_none() {
                ui.horizontal_wrapped(|ui| {
                    ui.label("You need to ");
                    if ui.link("setup an identity").clicked() {
                        app.set_page(ctx, Page::YourKeys);
                    }
                    ui.label(" to see any replies to that identity.");
                });
            }
            let feed = GLOBALS.feed.get_inbox();
            let id = if indirect { "activity" } else { "inbox" };
            ui.add_space(10.0);
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), ui.spacing().interact_size.y),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    add_left_space(ui);
                    ui.heading("Inbox");
                    recompute_btn(app, ui);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(10.0);
                        ui.label(RichText::new("Everything").size(11.0));
                        let size = ui.spacing().interact_size.y * egui::vec2(1.6, 0.8);
                        if widgets::switch_with_size(ui, &mut app.inbox_include_indirect, size)
                            .clicked()
                        {
                            app.set_page(
                                ctx,
                                Page::Feed(FeedKind::Inbox(app.inbox_include_indirect)),
                            );
                            ctx.data_mut(|d| {
                                d.insert_persisted(
                                    egui::Id::new("inbox_include_indirect"),
                                    app.inbox_include_indirect,
                                );
                            });
                        }
                        ui.label(RichText::new("Replies & DM").size(11.0));
                    });
                },
            );
            ui.add_space(6.0);
            render_a_feed(app, ctx, frame, ui, feed, false, id);
        }
        FeedKind::Thread { id, .. } => {
            if let Some(parent) = GLOBALS.feed.get_thread_parent() {
                render_a_feed(app, ctx, frame, ui, vec![parent], true, &id.as_hex_string());
            }
        }
        FeedKind::Person(pubkey) => {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                add_left_space(ui);
                if Some(pubkey) == GLOBALS.signer.public_key() {
                    ui.heading("My notes");
                } else {
                    ui.heading(gossip_lib::names::best_name_from_pubkey_lookup(&pubkey));
                }
                recompute_btn(app, ui);
            });
            ui.add_space(6.0);

            let feed = GLOBALS.feed.get_person_feed();
            render_a_feed(app, ctx, frame, ui, feed, false, &pubkey.as_hex_string());
        }
        FeedKind::DmChat(channel) => {
            if !GLOBALS.signer.is_ready() {
                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label("You need to ");
                    if ui.link("setup your private-key").clicked() {
                        app.set_page(ctx, Page::YourKeys);
                    }
                    ui.label(" to see DMs.");
                });
            }

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.heading(channel.name());
                recompute_btn(app, ui);
            });
            ui.add_space(10.0);

            let feed = GLOBALS.feed.get_dm_chat_feed();
            let id = channel.unique_id();
            render_a_feed(app, ctx, frame, ui, feed, false, &id);
        }
    }

    // Handle any changes due to changes in which notes are visible
    app.handle_visible_note_changes();
}

fn render_a_feed(
    app: &mut GossipUi,
    ctx: &Context,
    frame: &mut eframe::Frame,
    ui: &mut Ui,
    feed: Vec<Id>,
    threaded: bool,
    scroll_area_id: &str,
) {
    let feed_properties = FeedProperties {
        is_thread: threaded,
    };

    app.vert_scroll_area()
        .id_source(scroll_area_id)
        .show(ui, |ui| {
            Frame::none()
                .rounding(app.theme.feed_scroll_rounding(&feed_properties))
                .fill(app.theme.feed_scroll_fill(&feed_properties))
                .stroke(app.theme.feed_scroll_stroke(&feed_properties))
                .show(ui, |ui| {
                    let iter = feed.iter();
                    let first = feed.first();
                    let last = feed.last();
                    for id in iter {
                        render_note_maybe_fake(
                            app,
                            ctx,
                            frame,
                            ui,
                            FeedNoteParams {
                                id: *id,
                                indent: 0,
                                as_reply_to: false,
                                threaded,
                                is_first: Some(id) == first,
                                is_last: Some(id) == last,
                            },
                        );
                    }
                    if !feed.is_empty() {
                        ui.add_space(50.0);
                        ui.with_layout(egui::Layout::top_down(egui::Align::Center).with_cross_align(egui::Align::Center), |ui| {
                            app.theme.accent_button_1_style(ui.style_mut());
                            ui.spacing_mut().button_padding.x *= 3.0;
                            ui.spacing_mut().button_padding.y *= 2.0;
                            if ui.add(egui::Button::new("Load More")).clicked() {
                                let _ = GLOBALS
                                    .to_overlord
                                    .send(ToOverlordMessage::LoadMoreGeneralFeed);
                            }
                        });
                    }
                });
            ui.add_space(100.0);
        });
}

fn render_note_maybe_fake(
    app: &mut GossipUi,
    ctx: &Context,
    _frame: &mut eframe::Frame,
    ui: &mut Ui,
    feed_note_params: FeedNoteParams,
) {
    let FeedNoteParams {
        id,
        indent,
        as_reply_to,
        threaded,
        is_first,
        is_last,
    } = feed_note_params;

    let screen_rect = ctx.input(|i| i.screen_rect); // Rect
    let pos2 = ui.next_widget_position();

    // If too far off of the screen, don't actually render the post, just make some space
    // so the scrollbar isn't messed up
    let height = match app.height.get(&id) {
        Some(h) => *h,
        None => {
            // render the actual post and return
            // The first frame will be slow, but it will only need to do this
            // once per post.
            note::render_note(
                app,
                ctx,
                _frame,
                ui,
                FeedNoteParams {
                    id,
                    indent,
                    as_reply_to,
                    threaded,
                    is_first,
                    is_last,
                },
            );
            return;
        }
    };
    let after_the_bottom = pos2.y > screen_rect.max.y;
    let before_the_top = pos2.y + height < 0.0;

    if after_the_bottom || before_the_top {
        // Don't actually render, just make space for scrolling purposes
        ui.add_space(height);

        // Yes, and we need to fake render threads to get their approx height too.
        if threaded && !as_reply_to && !app.collapsed.contains(&id) {
            let mut replies = Vec::new();
            if let Some(note_ref) = app.notes.try_update_and_get(&id) {
                if let Ok(note_data) = note_ref.try_borrow() {
                    replies = GLOBALS
                        .storage
                        .get_replies(&note_data.event)
                        .unwrap_or_default();
                }
            }

            let iter = replies.iter();
            let first = replies.first();
            let last = replies.last();
            for reply_id in iter {
                render_note_maybe_fake(
                    app,
                    ctx,
                    _frame,
                    ui,
                    FeedNoteParams {
                        id: *reply_id,
                        indent: indent + 1,
                        as_reply_to,
                        threaded,
                        is_first: Some(reply_id) == first,
                        is_last: Some(reply_id) == last,
                    },
                );
            }
        }
    } else {
        note::render_note(
            app,
            ctx,
            _frame,
            ui,
            FeedNoteParams {
                id,
                indent,
                as_reply_to,
                threaded,
                is_first,
                is_last,
            },
        );
    }
}

fn add_left_space(ui: &mut Ui) {
    ui.add_space(2.0);
}

fn recompute_btn(app: &mut GossipUi, ui: &mut Ui) {
    if !app.settings.recompute_feed_periodically {
        if ui.link("Refresh").clicked() {
            GLOBALS.feed.sync_recompute();
        }
    }
    if GLOBALS
        .feed
        .recompute_lock
        .load(std::sync::atomic::Ordering::Relaxed)
    {
        ui.separator();
        ui.label("RECOMPUTING...");
    } else {
        ui.label(" "); // consume the same vertical space
    }
}
