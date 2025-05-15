use super::{widgets, GossipUi, Page};
use eframe::egui::{self, Align, Rect};
use egui::{Context, RichText, Ui, Vec2};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::relay::Relay;
use gossip_lib::DmChannel;
use gossip_lib::FeedKind;
use gossip_lib::GLOBALS;
use nostr_types::Id;
use std::sync::atomic::Ordering;

mod note;
pub use note::NoteRenderData;
pub(super) mod post;

const LONG_WAIT_TIME: f64 = 5.0; // seconds until the user has waited a long time for the feed to load

struct FeedNoteParams {
    id: Id,
    indent: usize,
    as_reply_to: bool,
    threaded: bool,
}

#[derive(Default)]
pub(super) struct Feeds {
    thread_needs_scroll: bool,
    last_enter_feed_time: f64,
}

pub(super) fn enter_feed(app: &mut GossipUi, ctx: &Context, kind: FeedKind) {
    if matches!(kind, FeedKind::Global) {
        app.global_relays = Relay::choose_relay_urls(Relay::GLOBAL, |_| true)
            .unwrap_or_default()
            .into_iter()
            .map(|u| u.into_string())
            .collect();
        if app.global_relays.is_empty() {
            app.global_relays.push(
                "You haven't configured any global relays, else they have errored recently"
                    .to_string(),
            );
        }
    }

    if let FeedKind::Thread {
        id: _,
        referenced_by: _,
        author: _,
    } = kind
    {
        if app.unsaved_settings.feed_thread_scroll_to_main_event {
            app.feeds.thread_needs_scroll = true;
        }
    }

    app.feeds.last_enter_feed_time = ctx.input(|i| i.time);

    // clear the displayed feed
    app.displayed_feed = vec![];
    app.displayed_feed_hash = None;
}

pub(super) fn update(app: &mut GossipUi, ctx: &Context, ui: &mut Ui) {
    if GLOBALS.ui_invalidate_all.load(Ordering::Relaxed) {
        app.notecache.invalidate_all();
        GLOBALS.ui_invalidate_all.store(false, Ordering::Relaxed);
    } else {
        // Do per-note invalidations
        if !GLOBALS.ui_notes_to_invalidate.read().is_empty() {
            let mut handle = GLOBALS.ui_notes_to_invalidate.write();
            for id in handle.iter() {
                app.notecache.invalidate_note(id);
            }
            *handle = Vec::new();
        }

        // Do per-person invalidations
        if !GLOBALS.ui_people_to_invalidate.read().is_empty() {
            let mut handle = GLOBALS.ui_people_to_invalidate.write();
            for pkh in handle.iter() {
                app.notecache.invalidate_person(pkh);
            }
            *handle = Vec::new();
        }
    }

    let feed_kind = GLOBALS.feed.get_feed_kind();
    let load_more = feed_kind.can_load_more();
    let long_wait = ctx.input(|i| i.time) - app.feeds.last_enter_feed_time > LONG_WAIT_TIME;

    // Each feed kind has it's own anchor key string, so we can use that for the egui
    // scroll widget id, so each scroll widget has it's own memory of where the scrollbar
    // was last set to.
    let scroll_widget_id = feed_kind.anchor_key();

    match feed_kind {
        FeedKind::List(list, mut with_replies) => {
            let metadata = GLOBALS
                .db()
                .get_person_list_metadata(list)
                .unwrap_or_default()
                .unwrap_or_default();

            ui.add_space(10.0);
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), ui.spacing().interact_size.y),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    add_left_space(ui);
                    let title_job = super::people::layout_list_title(ui, app, &metadata);
                    ui.label(title_job);
                    recompute_btn(app, ui);

                    if !app.displayed_feed.is_empty() || long_wait {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(16.0);

                            if widgets::Button::bordered(&app.theme, "Edit List")
                                .small(true)
                                .show(ui)
                                .clicked()
                            {
                                app.set_page(ctx, Page::PeopleList(list));
                            }

                            ui.add_space(10.0);
                            ui.label(RichText::new("Include replies").size(11.0));
                            if widgets::Switch::small(&app.theme, &mut with_replies)
                                .show(ui)
                                .clicked()
                            {
                                let new_feed_kind = FeedKind::List(list, with_replies);
                                super::write_feed_include_all(&new_feed_kind, ctx, with_replies);
                                app.set_page(ctx, Page::Feed(new_feed_kind));
                            }
                            ui.label(RichText::new("Main posts").size(11.0));

                            if widgets::Button::bordered(&app.theme, "Mark all read")
                                .small(true)
                                .show(ui)
                                .clicked()
                            {
                                let feed = app.displayed_feed.clone();
                                let mut txn = GLOBALS.db().get_write_txn().unwrap();
                                for id in feed.iter() {
                                    let _ = GLOBALS.db().mark_event_viewed(*id, Some(&mut txn));
                                }
                                let _ = txn.commit();
                            }
                        });
                    }
                },
            );
            ui.add_space(6.0);
            render_a_feed(app, ctx, ui, None, &scroll_widget_id, load_more);
        }
        FeedKind::Bookmarks => {
            ui.add_space(10.0);
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), ui.spacing().interact_size.y),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    add_left_space(ui);
                    ui.heading("Bookmarks");
                    recompute_btn(app, ui);
                },
            );
            ui.add_space(6.0);
            render_a_feed(app, ctx, ui, None, &scroll_widget_id, load_more);
        }
        FeedKind::Inbox(mut indirect) => {
            if GLOBALS.identity.public_key().is_none() {
                ui.horizontal_wrapped(|ui| {
                    ui.label("You need to ");
                    if ui.link("setup an identity").clicked() {
                        app.set_page(ctx, Page::YourKeys);
                    }
                    ui.label(" to see any replies to that identity.");
                });
            }
            ui.add_space(10.0);
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), ui.spacing().interact_size.y),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    add_left_space(ui);
                    ui.heading("Inbox");
                    recompute_btn(app, ui);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(16.0);
                        ui.label(RichText::new("Everything").size(11.0));
                        if widgets::Switch::small(&app.theme, &mut indirect)
                            .show(ui)
                            .clicked()
                        {
                            let new_feed_kind = FeedKind::Inbox(indirect);
                            super::write_feed_include_all(&new_feed_kind, ctx, indirect);
                            app.set_page(ctx, Page::Feed(new_feed_kind));
                        }
                        ui.label(RichText::new("Replies & DM").size(11.0));

                        if widgets::Button::bordered(&app.theme, "Mark all read")
                            .small(true)
                            .show(ui)
                            .clicked()
                        {
                            let feed = app.displayed_feed.clone();
                            let mut txn = GLOBALS.db().get_write_txn().unwrap();
                            for id in feed.iter() {
                                let _ = GLOBALS.db().mark_event_viewed(*id, Some(&mut txn));
                            }
                            let _ = txn.commit();
                        }
                    });
                },
            );
            ui.add_space(6.0);
            render_a_feed(app, ctx, ui, None, &scroll_widget_id, load_more);
        }
        FeedKind::Thread { id, .. } => {
            if let Some(parent) = GLOBALS.feed.get_thread_parent() {
                if app.notecache.try_update_and_get(&id).is_none() {
                    ui.add_space(4.0);
                    ui.label("LOADING...");
                    ui.add_space(4.0);
                } else if let Some(note_ref) = app.notecache.try_update_and_get(&parent) {
                    if let Ok(note_data) = note_ref.try_borrow() {
                        if note_data.event.replies_to().is_some() {
                            ui.add_space(4.0);
                            ui.label("CLIMBING THREAD...");
                            ui.add_space(4.0);
                        }
                    }
                }

                render_a_feed(app, ctx, ui, Some(parent), &scroll_widget_id, load_more);
            } else {
                ui.label("THREAD NOT FOUND");
            }
        }
        FeedKind::Person(pubkey) => {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                add_left_space(ui);
                if Some(pubkey) == GLOBALS.identity.public_key() {
                    ui.heading("My notes");
                } else {
                    ui.heading(gossip_lib::names::best_name_from_pubkey_lookup(&pubkey));
                }
                recompute_btn(app, ui);
            });
            ui.add_space(6.0);

            render_a_feed(app, ctx, ui, None, &scroll_widget_id, load_more);
        }
        FeedKind::Global => {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                add_left_space(ui);
                ui.heading("GLOBAL");
                recompute_btn(app, ui);
            });
            ui.label(app.global_relays.join(", "));
            ui.add_space(6.0);

            render_a_feed(app, ctx, ui, None, &scroll_widget_id, load_more);
        }
        FeedKind::Relay(relay_url) => {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                add_left_space(ui);
                ui.heading(format!("{}", &relay_url));
                recompute_btn(app, ui);
            });
            ui.add_space(6.0);

            render_a_feed(app, ctx, ui, None, &scroll_widget_id, load_more);
        }
        FeedKind::DmChat(channel) => {
            if !GLOBALS.identity.is_unlocked() {
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
            ui.allocate_ui_with_layout(
                Vec2::new(ui.available_width(), ui.spacing().interact_size.y),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    add_left_space(ui);
                    if let Some(key) = channel.keys().first() {
                        let avatar = if let Some(avatar) = app.try_get_avatar(ctx, key) {
                            avatar
                        } else {
                            app.placeholder_avatar.clone()
                        };

                        widgets::paint_avatar_only(
                            ui,
                            &avatar,
                            widgets::AvatarSize::Mini.get_size(),
                        );

                        if ui.link(RichText::new(channel.name()).heading()).clicked() {
                            app.set_page(ctx, Page::Person(key.to_owned()));
                        }
                    } else {
                        ui.heading(channel.name());
                    }
                    recompute_btn(app, ui);

                    if let Some(key) = channel.keys().first() {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(8.0);

                            if widgets::Button::bordered(&app.theme, "Profile")
                                .small(true)
                                .show(ui)
                                .clicked()
                            {
                                app.set_page(ctx, Page::Person(key.to_owned()));
                            }

                            ui.add_space(10.0);

                            if widgets::Button::bordered(&app.theme, "View notes")
                                .small(true)
                                .show(ui)
                                .clicked()
                            {
                                app.set_page(ctx, Page::Feed(FeedKind::Person(key.to_owned())));
                            }
                        });
                    }
                },
            );

            ui.add_space(6.0);
            render_dm_feed(app, ui, channel);
        }
    }

    // Handle any changes due to changes in which notes are visible
    app.handle_visible_note_changes();
}

#[allow(clippy::too_many_arguments)]
fn render_a_feed(
    app: &mut GossipUi,
    ctx: &Context,
    ui: &mut Ui,
    parent: Option<Id>,
    scroll_area_id: &str,
    offer_load_more: bool,
) {
    let feed_newest_at_bottom = GLOBALS.db().read_setting_feed_newest_at_bottom();

    // Do not render feed while switching or we will lose our scroll offset.
    // Give us the spinner while we wait
    if GLOBALS.feed.is_switching() {
        ui.add_space(50.0);
        widgets::giant_spinner(ui, &app.theme);
        return;
    }

    let feed = app.displayed_feed.clone();

    app.vert_scroll_area()
        .auto_shrink(false)
        .stick_to_bottom(feed_newest_at_bottom)
        .id_salt(scroll_area_id)
        .show(ui, |ui| {
            egui::Frame::none()
                .outer_margin(egui::Margin {
                    left: 0.0,
                    right: 14.0,
                    top: 0.0,
                    bottom: 0.0,
                })
                .rounding(app.theme.feed_scroll_rounding())
                .fill(app.theme.feed_scroll_fill())
                .stroke(app.theme.feed_scroll_stroke())
                .show(ui, |ui| {
                    if feed_newest_at_bottom {
                        ui.add_space(50.0);
                        if offer_load_more
                            && (!feed.is_empty()
                                || (ctx.input(|i| i.time) - app.feeds.last_enter_feed_time)
                                    > LONG_WAIT_TIME)
                        {
                            render_load_more(app, ui)
                        }
                        ui.add_space(50.0);

                        if let Some(id) = parent {
                            render_note_maybe_fake(
                                app,
                                ctx,
                                ui,
                                FeedNoteParams {
                                    id,
                                    indent: 0,
                                    as_reply_to: false,
                                    threaded: true,
                                },
                            );
                        } else {
                            for id in feed.iter().rev() {
                                render_note_maybe_fake(
                                    app,
                                    ctx,
                                    ui,
                                    FeedNoteParams {
                                        id: *id,
                                        indent: 0,
                                        as_reply_to: false,
                                        threaded: false,
                                    },
                                );
                            }
                        }
                    } else {
                        if let Some(id) = parent {
                            render_note_maybe_fake(
                                app,
                                ctx,
                                ui,
                                FeedNoteParams {
                                    id,
                                    indent: 0,
                                    as_reply_to: false,
                                    threaded: true,
                                },
                            );
                        } else {
                            for id in feed.iter() {
                                render_note_maybe_fake(
                                    app,
                                    ctx,
                                    ui,
                                    FeedNoteParams {
                                        id: *id,
                                        indent: 0,
                                        as_reply_to: false,
                                        threaded: false,
                                    },
                                );
                            }
                        }

                        ui.add_space(50.0);
                        if offer_load_more
                            && (!feed.is_empty()
                                || (ctx.input(|i| i.time) - app.feeds.last_enter_feed_time)
                                    > LONG_WAIT_TIME)
                        {
                            render_load_more(app, ui)
                        }
                        ui.add_space(50.0);
                    }
                });
        });
}

fn render_dm_feed(app: &mut GossipUi, ui: &mut Ui, channel: DmChannel) {
    let feed = app.displayed_feed.clone();
    let scroll_area_id = channel.name();
    let feed_newest_at_bottom = GLOBALS.db().read_setting_dm_feed_newest_at_bottom();
    let iterator: Box<dyn Iterator<Item = &Id>> = if feed_newest_at_bottom {
        Box::new(feed.iter().rev())
    } else {
        Box::new(feed.iter())
    };

    app.vert_scroll_area()
        .auto_shrink(false)
        .stick_to_bottom(feed_newest_at_bottom)
        .id_salt(scroll_area_id)
        .show(ui, |ui| {
            for id in iterator {
                note::render_dm_note(
                    app,
                    ui,
                    FeedNoteParams {
                        id: *id,
                        indent: 0,
                        as_reply_to: false,
                        threaded: false,
                    },
                );
            }
        });
}

fn render_load_more(app: &mut GossipUi, ui: &mut Ui) {
    ui.with_layout(
        egui::Layout::top_down(egui::Align::Center).with_cross_align(egui::Align::Center),
        |ui| {
            ui.spacing_mut().button_padding.x *= 3.0;
            ui.spacing_mut().button_padding.y *= 2.0;

            let relays_still_loading = GLOBALS.loading_more.load(Ordering::SeqCst);
            let msg = if relays_still_loading > 0 {
                format!("Load More (loading from {})", relays_still_loading)
            } else {
                "Load More".to_owned()
            };

            let response = widgets::Button::primary(&app.theme, msg).show(ui);
            if response.clicked() {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::LoadMoreCurrentFeed);
                app.update_once_immediately = true;
            }

            // draw some nice lines left and right of the button
            let stroke = egui::Stroke::new(1.5, ui.visuals().extreme_bg_color);
            let width = (ui.available_width() - response.rect.width()) / 2.0 - 20.0;
            let left_start = response.rect.left_center() - egui::vec2(10.0, 0.0);
            let left_end = left_start - egui::vec2(width, 0.0);
            ui.painter().line_segment([left_start, left_end], stroke);
            let right_start = response.rect.right_center() + egui::vec2(10.0, 0.0);
            let right_end = right_start + egui::vec2(width, 0.0);
            ui.painter().line_segment([right_start, right_end], stroke);
        },
    );
}

fn render_note_maybe_fake(
    app: &mut GossipUi,
    ctx: &Context,
    ui: &mut Ui,
    feed_note_params: FeedNoteParams,
) {
    let FeedNoteParams {
        id,
        indent,
        as_reply_to,
        threaded,
    } = feed_note_params;

    let screen_rect = ctx.input(|i| i.screen_rect); // Rect
    let pos2 = ui.next_widget_position();

    let is_main_event: bool = {
        let feed_kind = GLOBALS.feed.get_feed_kind();
        match feed_kind {
            FeedKind::Thread { id: thread_id, .. } => thread_id == id,
            _ => false,
        }
    };

    // If too far off of the screen, don't actually render the post, just make some space
    // so the scrollbar isn't messed up
    let height = match app.feed_note_height.get(&id) {
        Some(h) => *h,
        None => {
            // render the actual post and return
            // The first frame will be slow, but it will only need to do this
            // once per post.
            note::render_note(
                app,
                ctx,
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

        // we also need to scroll to not-rendered notes
        if is_main_event && app.feeds.thread_needs_scroll {
            // keep auto-scrolling until user scrolls
            if app.is_scrolling() {
                app.feeds.thread_needs_scroll = false;
            }
            ui.scroll_to_rect(
                Rect::from_min_size(pos2, egui::vec2(ui.available_width(), height)),
                Some(Align::Center),
            );
        }

        // Yes, and we need to fake render threads to get their approx height too.
        if threaded && !as_reply_to && !app.collapsed.contains(&id) {
            let mut replies = Vec::new();
            if let Some(note_ref) = app.notecache.try_update_and_get(&id) {
                if let Ok(note_data) = note_ref.try_borrow() {
                    replies = GLOBALS
                        .db()
                        .get_replies(&note_data.event)
                        .unwrap_or_default();
                }
            }

            let iter = replies.iter();
            for reply_id in iter {
                render_note_maybe_fake(
                    app,
                    ctx,
                    ui,
                    FeedNoteParams {
                        id: *reply_id,
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

fn add_left_space(ui: &mut Ui) {
    ui.add_space(2.0);
}

fn recompute_btn(app: &mut GossipUi, ui: &mut Ui) {
    if !read_setting!(recompute_feed_periodically) {
        if ui.link("Refresh").clicked() {
            GLOBALS.feed.sync_recompute();
            app.displayed_feed = GLOBALS.feed.get_feed_events();
            app.displayed_feed_hash = GLOBALS.feed.get_feed_hash();
        }
        return;
    }

    ui.separator();
    if GLOBALS.feed.is_recomputing() {
        ui.label("RECOMPUTING...");
    } else {
        ui.label(" "); // consume the same vertical space
    }

    let update_immediately = {
        let mut update_immediately: bool =
            matches!(GLOBALS.feed.get_feed_kind(), FeedKind::DmChat(_));
        if let Some(pubkey) = GLOBALS.identity.public_key() {
            update_immediately |= GLOBALS.feed.get_feed_kind() == FeedKind::Person(pubkey);
        }
        update_immediately
    };

    let feed_hash = GLOBALS.feed.get_feed_hash();
    if feed_hash != app.displayed_feed_hash {
        if app.displayed_feed.is_empty()
            || update_immediately
            || ui.link("Show New Updates").clicked()
        {
            app.displayed_feed = GLOBALS.feed.get_feed_events();
            app.displayed_feed_hash = feed_hash;
        } else if app.update_once_immediately {
            app.displayed_feed = GLOBALS.feed.get_feed_events();
            app.displayed_feed_hash = feed_hash;
            app.update_once_immediately = false;
        }
    }
}
