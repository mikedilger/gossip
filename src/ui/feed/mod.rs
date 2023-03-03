use super::{GossipUi, Page};
use crate::feed::FeedKind;
use crate::globals::{Globals, GLOBALS};
use eframe::egui;
use egui::{Context, Frame, ScrollArea, SelectableLabel, Ui, Vec2};
use nostr_types::Id;

mod note;
mod post;

struct FeedNoteParams {
    id: Id,
    indent: usize,
    as_reply_to: bool,
    threaded: bool,
}

pub(super) fn update(app: &mut GossipUi, ctx: &Context, frame: &mut eframe::Frame, ui: &mut Ui) {
    let feed_kind = GLOBALS.feed.get_feed_kind();

    // Feed Page Selection
    ui.horizontal(|ui| {
        if ui
            .add(SelectableLabel::new(
                app.page == Page::Feed(FeedKind::Main),
                "Main feed",
            ))
            .clicked()
        {
            app.set_page(Page::Feed(FeedKind::Main));
        }
        ui.separator();
        if ui
            .add(SelectableLabel::new(
                app.page == Page::Feed(FeedKind::General),
                "Conversations",
            ))
            .clicked()
        {
            app.set_page(Page::Feed(FeedKind::General));
        }
        ui.separator();
        if ui
            .add(SelectableLabel::new(
                app.page == Page::Feed(FeedKind::Replies),
                "Inbox",
            ))
            .clicked()
        {
            app.set_page(Page::Feed(FeedKind::Replies));
        }
        if matches!(feed_kind.clone(), FeedKind::Thread { .. }) {
            ui.separator();
            if ui
                .add(SelectableLabel::new(
                    app.page == Page::Feed(feed_kind.clone()),
                    "Thread",
                ))
                .clicked()
            {
                app.set_page(Page::Feed(feed_kind.clone()));
            }
        }
        if matches!(feed_kind, FeedKind::Person(..)) {
            ui.separator();
            if ui
                .add(SelectableLabel::new(
                    app.page == Page::Feed(feed_kind.clone()),
                    "Person",
                ))
                .clicked()
            {
                app.set_page(Page::Feed(feed_kind.clone()));
            }
        }
    });

    ui.add_space(10.0);

    post::posting_area(app, ctx, frame, ui);

    ui.add_space(10.0);

    match feed_kind {
        FeedKind::Main => {
            let feed = GLOBALS.feed.get_main();
            render_a_feed(app, ctx, frame, ui, feed, false, "main");
        }
        FeedKind::General => {
            let feed = GLOBALS.feed.get_general();
            render_a_feed(app, ctx, frame, ui, feed, false, "general");
        }
        FeedKind::Replies => {
            if GLOBALS.signer.public_key().is_none() {
                ui.horizontal_wrapped(|ui| {
                    ui.label("You need to ");
                    if ui.link("setup an identity").clicked() {
                        app.set_page(Page::YourKeys);
                    }
                    ui.label(" to see any replies to that identity.");
                });
            }
            let feed = GLOBALS.feed.get_replies();
            render_a_feed(app, ctx, frame, ui, feed, false, "replies");
        }
        FeedKind::Thread { id, .. } => {
            if let Some(parent) = GLOBALS.feed.get_thread_parent() {
                render_a_feed(app, ctx, frame, ui, vec![parent], true, &id.as_hex_string());
            }
        }
        FeedKind::Person(pubkeyhex) => {
            let feed = GLOBALS.feed.get_person_feed(pubkeyhex.clone());
            render_a_feed(app, ctx, frame, ui, feed, false, pubkeyhex.as_str());
        }
    }
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
    ScrollArea::vertical()
        .id_source(scroll_area_id)
        .override_scroll_delta(Vec2 {
            x: 0.0,
            y: app.current_scroll_offset * 2.0, // double speed
        })
        .show(ui, |ui| {
            Frame::none()
                .fill(app.settings.theme.feed_scroll_fill())
                .show(ui, |ui| {
                    for id in feed.iter() {
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
                            },
                        );
                    }
                });
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
    } = feed_note_params;

    // We always get the event even offscreen so we can estimate its height
    let maybe_event = GLOBALS.events.get(&id);
    if maybe_event.is_none() {
        return;
    }
    let event = maybe_event.unwrap();

    let screen_rect = ctx.input(|i| i.screen_rect); // Rect
    let pos2 = ui.next_widget_position();

    // If too far off of the screen, don't actually render the post, just make some space
    // so the scrollbar isn't messed up
    let height = match app.height.get(&id) {
        Some(h) => *h,
        None => {
            // render the actual post and return
            // The first frame will be very slow, but it will only need to do this
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
        if threaded && !as_reply_to {
            let replies = Globals::get_replies_sync(event.id);
            for reply_id in replies {
                render_note_maybe_fake(
                    app,
                    ctx,
                    _frame,
                    ui,
                    FeedNoteParams {
                        id: reply_id,
                        indent: indent + 1,
                        as_reply_to,
                        threaded,
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
            },
        );
    }
}
