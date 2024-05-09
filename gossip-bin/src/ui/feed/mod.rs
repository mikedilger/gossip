use std::sync::atomic::Ordering;

use eframe::egui::{self, Align, FontId, Rect};
use egui::{Context, RichText, Ui, Vec2};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::{FeedKind, GLOBALS};
use nostr_types::Id;

use super::theme::FeedProperties;
use super::{widgets, GossipUi, Page};

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

#[derive(Default)]
pub(super) struct Feeds {
    thread_needs_scroll: bool,
}

pub(super) fn enter_feed(app: &mut GossipUi, kind: FeedKind) {
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
}

pub(super) fn update(app: &mut GossipUi, ctx: &Context, ui: &mut Ui) {
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
    let load_more = feed_kind.can_load_more();

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
                    recompute_btn(ui);

                    if !feed.is_empty() {
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
                            if widgets::Switch::small(&app.theme, &mut app.mainfeed_include_nonroot)
                                .show(ui)
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
                    }
                },
            );
            ui.add_space(6.0);
            render_a_feed(app, ctx, ui, feed, false, &id, load_more);
        }
        FeedKind::Inbox(indirect) => {
            if read_setting!(public_key).is_none() {
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
                    recompute_btn(ui);

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(16.0);
                        ui.label(RichText::new("Everything").size(11.0));
                        if widgets::Switch::small(&app.theme, &mut app.inbox_include_indirect)
                            .show(ui)
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
            render_a_feed(app, ctx, ui, feed, false, id, load_more);
        }
        FeedKind::Thread { id, .. } => {
            if let Some(parent) = GLOBALS.feed.get_thread_parent() {
                render_a_feed(
                    app,
                    ctx,
                    ui,
                    vec![parent],
                    true,
                    &id.as_hex_string(),
                    load_more,
                );
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
                recompute_btn(ui);
            });
            ui.add_space(6.0);

            let feed = GLOBALS.feed.get_person_feed();
            render_a_feed(
                app,
                ctx,
                ui,
                feed,
                false,
                &pubkey.as_hex_string(),
                load_more,
            );
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
            ui.horizontal(|ui| {
                ui.heading(channel.name());
                recompute_btn(ui);
            });
            ui.add_space(10.0);

            let feed = GLOBALS.feed.get_dm_chat_feed();
            let id = channel.unique_id();
            render_a_feed(app, ctx, ui, feed, false, &id, load_more);
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
    feed: Vec<Id>,
    threaded: bool,
    scroll_area_id: &str,
    offer_load_more: bool,
) {
    let feed_properties = FeedProperties {
        is_thread: threaded,
    };

    let feed_newest_at_bottom = GLOBALS.storage.read_setting_feed_newest_at_bottom();

    app.vert_scroll_area()
        .auto_shrink(false)
        .stick_to_bottom(feed_newest_at_bottom)
        .id_source(scroll_area_id)
        .show(ui, |ui| {
            egui::Frame::none()
                .outer_margin(egui::Margin {
                    left: 0.0,
                    right: 14.0,
                    top: 0.0,
                    bottom: 0.0,
                })
                .rounding(app.theme.feed_scroll_rounding(&feed_properties))
                .fill(app.theme.feed_scroll_fill(&feed_properties))
                .stroke(app.theme.feed_scroll_stroke(&feed_properties))
                .show(ui, |ui| {
                    if feed_newest_at_bottom {
                        ui.add_space(50.0);
                        if offer_load_more {
                            render_load_more(app, ui)
                        }
                        ui.add_space(50.0);

                        for id in feed.iter().rev() {
                            render_note_maybe_fake(
                                app,
                                ctx,
                                ui,
                                FeedNoteParams {
                                    id: *id,
                                    indent: 0,
                                    as_reply_to: false,
                                    threaded,
                                    is_first: Some(id) == feed.last(),
                                    is_last: Some(id) == feed.first(),
                                },
                            );
                        }
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
                                    threaded,
                                    is_first: Some(id) == feed.first(),
                                    is_last: Some(id) == feed.last(),
                                },
                            );
                        }

                        ui.add_space(50.0);
                        if feed.is_empty() {
                            // show a spinner
                            let size = ui.available_width() / 2.0;
                            ui.horizontal(|ui| {
                                ui.add_space((ui.available_width() - size) / 2.0);
                                let (rect, response) = ui.allocate_exact_size(
                                    egui::vec2(size, size),
                                    egui::Sense::hover(),
                                );
                                {
                                    ui.ctx().request_repaint(); // because it is animated

                                    let spinner_color = if app.theme.dark_mode {
                                        app.theme.neutral_950()
                                    } else {
                                        egui::Color32::WHITE
                                    };
                                    let radius = (rect.height() / 2.0) - 2.0;
                                    let n_points = 240;
                                    let time = ui.input(|i| i.time);
                                    let start_angle = time * std::f64::consts::TAU;
                                    let end_angle = start_angle + 240f64.to_radians() * time.sin();
                                    let points: Vec<egui::Pos2> = (0..n_points)
                                        .map(|i| {
                                            let angle = egui::lerp(
                                                start_angle..=end_angle,
                                                i as f64 / n_points as f64,
                                            );
                                            let (sin, cos) = angle.sin_cos();
                                            rect.center()
                                                + radius * egui::vec2(cos as f32, sin as f32)
                                        })
                                        .collect();
                                    for point in points {
                                        ui.painter().circle_filled(point, 15.0, spinner_color);
                                    }
                                }
                                ui.painter().text(
                                    response.rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "Loading",
                                    FontId::proportional(16.0),
                                    ui.visuals().text_color(),
                                );
                            });
                        } else if offer_load_more {
                            render_load_more(app, ui)
                        }
                        ui.add_space(50.0);
                    }
                });
        });
}

fn render_load_more(app: &mut GossipUi, ui: &mut Ui) {
    ui.with_layout(
        egui::Layout::top_down(egui::Align::Center).with_cross_align(egui::Align::Center),
        |ui| {
            ui.spacing_mut().button_padding.x *= 3.0;
            ui.spacing_mut().button_padding.y *= 2.0;
            let response = widgets::Button::primary(&app.theme, "Load More").show(ui);
            if response.clicked() {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::LoadMoreCurrentFeed);
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
        is_first,
        is_last,
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

    // If too far off of the screen, don't actually render the post, just make some
    // space so the scrollbar isn't messed up
    let height = match app.height.get(&id) {
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

        // we also need to scroll to not-rendered notes
        if is_main_event && app.feeds.thread_needs_scroll {
            // keep auto-scrolling until user scrolls
            if app.current_scroll_offset != 0.0 {
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

fn recompute_btn(ui: &mut Ui) {
    if !read_setting!(recompute_feed_periodically) {
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
