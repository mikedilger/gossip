use super::{GossipUi, Page};
use crate::comms::ToOverlordMessage;
use crate::feed::FeedKind;
use crate::globals::{Globals, GLOBALS};
use crate::people::DbPerson;
use crate::ui::widgets::CopyButton;
use crate::AVATAR_SIZE_F32;
use eframe::egui;
use egui::{
    Align, Color32, Context, Frame, Image, Label, Layout, RichText, ScrollArea, SelectableLabel,
    Sense, Separator, Stroke, TextStyle, Ui, Vec2,
};
use nostr_types::{Event, EventKind, Id, IdHex};
use std::sync::atomic::Ordering;

mod content;
mod post;

struct FeedPostParams {
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
                app.page == Page::Feed(FeedKind::General),
                "Following",
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
                .fill(app.settings.theme.feed_scroll_fill(app.settings.dark_mode))
                .show(ui, |ui| {
                    for id in feed.iter() {
                        render_post_maybe_fake(
                            app,
                            ctx,
                            frame,
                            ui,
                            FeedPostParams {
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

fn render_post_maybe_fake(
    app: &mut GossipUi,
    ctx: &Context,
    _frame: &mut eframe::Frame,
    ui: &mut Ui,
    feed_post_params: FeedPostParams,
) {
    let FeedPostParams {
        id,
        indent,
        as_reply_to,
        threaded,
    } = feed_post_params;

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
            render_post_actual(
                app,
                ctx,
                _frame,
                ui,
                FeedPostParams {
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
                render_post_maybe_fake(
                    app,
                    ctx,
                    _frame,
                    ui,
                    FeedPostParams {
                        id: reply_id,
                        indent: indent + 1,
                        as_reply_to,
                        threaded,
                    },
                );
            }
        }
    } else {
        render_post_actual(
            app,
            ctx,
            _frame,
            ui,
            FeedPostParams {
                id,
                indent,
                as_reply_to,
                threaded,
            },
        );
    }
}

fn render_post_actual(
    app: &mut GossipUi,
    ctx: &Context,
    _frame: &mut eframe::Frame,
    ui: &mut Ui,
    feed_post_params: FeedPostParams,
) {
    let FeedPostParams {
        id,
        indent,
        as_reply_to,
        threaded,
    } = feed_post_params;

    let maybe_event = GLOBALS.events.get(&id);
    if maybe_event.is_none() {
        return;
    }
    let event = maybe_event.unwrap();

    let top = ui.next_widget_position();

    // Only render known relevent events
    let enable_reposts = GLOBALS.settings.read().reposts;
    let direct_messages = GLOBALS.settings.read().direct_messages;
    if event.kind != EventKind::TextNote
        && !(enable_reposts && (event.kind == EventKind::Repost))
        && !(direct_messages && (event.kind == EventKind::EncryptedDirectMessage))
    {
        return;
    }

    let person = match GLOBALS.people.get(&event.pubkey.into()) {
        Some(p) => p,
        None => DbPerson::new(event.pubkey.into()),
    };

    let is_new = !app.viewed.contains(&event.id);

    let is_main_event: bool = {
        let feed_kind = GLOBALS.feed.get_feed_kind();
        match feed_kind {
            FeedKind::Thread { id, .. } => id == event.id,
            _ => false,
        }
    };

    let inner_response = Frame::none()
        .inner_margin(app.settings.theme.feed_frame_inner_margin())
        .outer_margin(app.settings.theme.feed_frame_outer_margin())
        .rounding(app.settings.theme.feed_frame_rounding())
        .shadow(app.settings.theme.feed_frame_shadow(app.settings.dark_mode))
        .fill(
            app.settings
                .theme
                .feed_frame_fill(is_new, is_main_event, app.settings.dark_mode),
        )
        .stroke(
            app.settings
                .theme
                .feed_frame_stroke(is_new, is_main_event, app.settings.dark_mode),
        )
        .show(ui, |ui| {
            ui.add_space(4.0);

            ui.horizontal_wrapped(|ui| {
                // Indents first (if threaded)
                if threaded {
                    let space = 100.0 * (10.0 - (1000.0 / (indent as f32 + 100.0)));
                    ui.add_space(space);
                    if indent > 0 {
                        ui.label(RichText::new(format!("{}>", indent)).italics().weak());
                    }
                }

                if person.muted > 0 {
                    ui.label(RichText::new("MUTED POST").monospace().italics());
                } else {
                    render_post_inner(app, ctx, ui, event, person, is_main_event, as_reply_to);
                }
            });
        });

    // Mark post as viewed if hovered AND we are not scrolling
    if inner_response.response.hovered() && app.current_scroll_offset == 0.0 {
        app.viewed.insert(id);
    }

    // Store actual rendered height for future reference
    let bottom = ui.next_widget_position();
    app.height.insert(id, bottom.y - top.y);

    thin_separator(
        ui,
        app.settings
            .theme
            .feed_post_separator_stroke(app.settings.dark_mode),
    );

    if threaded && !as_reply_to {
        let replies = Globals::get_replies_sync(id);
        for reply_id in replies {
            render_post_maybe_fake(
                app,
                ctx,
                _frame,
                ui,
                FeedPostParams {
                    id: reply_id,
                    indent: indent + 1,
                    as_reply_to,
                    threaded,
                },
            );
        }
    }
}

fn render_post_inner(
    app: &mut GossipUi,
    ctx: &Context,
    ui: &mut Ui,
    event: Event,
    person: DbPerson,
    is_main_event: bool,
    as_reply_to: bool,
) {
    let deletion = Globals::get_deletion_sync(event.id);

    let (reactions, self_already_reacted) = Globals::get_reactions_sync(event.id);

    let tag_re = app.tag_re.clone();

    // Avatar first
    let avatar = if let Some(avatar) = app.try_get_avatar(ctx, &event.pubkey.into()) {
        avatar
    } else {
        app.placeholder_avatar.clone()
    };

    // If it is a repost without any comment, resize the avatar to highlight the original poster's one
    let resize_factor = if event.kind == EventKind::Repost
        && (event.content.is_empty() || serde_json::from_str::<Event>(&event.content).is_ok())
    {
        180.0
    } else {
        100.0
    };

    let size = AVATAR_SIZE_F32 * GLOBALS.pixels_per_point_times_100.load(Ordering::Relaxed) as f32
        / resize_factor;

    if ui
        .add(Image::new(&avatar, Vec2 { x: size, y: size }).sense(Sense::click()))
        .clicked()
    {
        app.set_page(Page::Person(event.pubkey.into()));
    };

    // Everything else next
    ui.vertical(|ui| {
        // First row
        ui.horizontal_wrapped(|ui| {
            GossipUi::render_person_name_line(app, ui, &person);

            if let Some((irt, _)) = event.replies_to() {
                ui.add_space(8.0);

                ui.style_mut().override_text_style = Some(TextStyle::Small);
                let idhex: IdHex = irt.into();
                let nam = format!("replies to #{}", GossipUi::hex_id_short(&idhex));
                if ui.link(&nam).clicked() {
                    app.set_page(Page::Feed(FeedKind::Thread {
                        id: irt,
                        referenced_by: event.id,
                    }));
                };
                ui.reset_style();
            }

            ui.add_space(8.0);

            if event.pow() > 0 {
                ui.label(format!("POW={}", event.pow()));
            }

            if deletion.is_some() {
                let color = if ui.visuals().dark_mode {
                    Color32::LIGHT_RED
                } else {
                    Color32::DARK_RED
                };
                ui.label(RichText::new("DELETED").color(color));
            }

            if event.kind == EventKind::Repost {
                let color = if ui.visuals().dark_mode {
                    Color32::LIGHT_BLUE
                } else {
                    Color32::DARK_BLUE
                };
                ui.label(RichText::new("REPOSTED").color(color));
            }

            if event.kind == EventKind::EncryptedDirectMessage {
                let color = if ui.visuals().dark_mode {
                    Color32::LIGHT_BLUE
                } else {
                    Color32::DARK_BLUE
                };
                ui.label(RichText::new("ENCRYPTED DM").color(color));
            }

            ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                ui.menu_button(RichText::new("📃▼").size(13.0), |ui| {
                    if !is_main_event && event.kind != EventKind::EncryptedDirectMessage {
                        if ui.button("View Thread").clicked() {
                            app.set_page(Page::Feed(FeedKind::Thread {
                                id: event.id,
                                referenced_by: event.id,
                            }));
                        }
                    }
                    if ui.button("Copy ID").clicked() {
                        ui.output_mut(|o| o.copied_text = event.id.try_as_bech32_string().unwrap());
                    }
                    if ui.button("Copy ID as hex").clicked() {
                        ui.output_mut(|o| o.copied_text = event.id.as_hex_string());
                    }
                    if ui.button("Dismiss").clicked() {
                        GLOBALS.dismissed.blocking_write().push(event.id);
                    }
                });

                if !is_main_event && event.kind != EventKind::EncryptedDirectMessage {
                    if ui
                        .button(RichText::new("➤").size(13.0))
                        .on_hover_text("View Thread")
                        .clicked()
                    {
                        app.set_page(Page::Feed(FeedKind::Thread {
                            id: event.id,
                            referenced_by: event.id,
                        }));
                    }
                }

                ui.label(
                    RichText::new(crate::date_ago::date_ago(event.created_at))
                        .italics()
                        .weak(),
                );
            });
        });

        ui.add_space(4.0);

        // Possible subject line
        if let Some(subject) = event.subject() {
            ui.label(RichText::new(subject).strong().underline());
        }

        ui.add_space(2.0);

        // MAIN CONTENT
        ui.horizontal_wrapped(|ui| {
            if app.render_raw == Some(event.id) {
                ui.label(serde_json::to_string_pretty(&event).unwrap());
            } else if app.render_qr == Some(event.id) {
                app.render_qr(ui, ctx, "feedqr", event.content.trim());
            } else if event.content_warning().is_some() && !app.approved.contains(&event.id) {
                ui.label(
                    RichText::new(format!(
                        "Content-Warning: {}",
                        event.content_warning().unwrap()
                    ))
                    .monospace()
                    .italics(),
                );
                if ui.button("Show Post").clicked() {
                    app.approved.insert(event.id);
                    app.height.remove(&event.id); // will need to be recalculated.
                }
            } else if event.kind == EventKind::Repost {
                if let Ok(inner_event) = serde_json::from_str::<Event>(&event.content) {
                    let inner_person = match GLOBALS.people.get(&inner_event.pubkey.into()) {
                        Some(p) => p,
                        None => DbPerson::new(inner_event.pubkey.into()),
                    };
                    ui.vertical(|ui| {
                        thin_repost_separator(ui);
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            render_post_inner(
                                app,
                                ctx,
                                ui,
                                inner_event,
                                inner_person,
                                false,
                                false,
                            );
                        });
                        thin_repost_separator(ui);
                    });
                } else if event.content.is_empty() {
                    content::render_content(
                        app,
                        ui,
                        &tag_re,
                        &event,
                        deletion.is_some(),
                        Some("#[0]".to_owned()),
                    );
                } else {
                    // render like a kind-1 event with a mention
                    content::render_content(app, ui, &tag_re, &event, deletion.is_some(), None);
                }
            } else {
                content::render_content(app, ui, &tag_re, &event, deletion.is_some(), None);
            }
        });

        ui.add_space(8.0);

        // deleted?
        if let Some(delete_reason) = &deletion {
            ui.label(RichText::new(format!("Deletion Reason: {}", delete_reason)).italics());
            ui.add_space(8.0);
        }

        // Under row
        if !as_reply_to {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add(CopyButton {})
                    .on_hover_text("Copy Contents")
                    .clicked()
                {
                    if app.render_raw == Some(event.id) {
                        ui.output_mut(|o| o.copied_text = serde_json::to_string(&event).unwrap());
                    } else {
                        ui.output_mut(|o| o.copied_text = event.content.clone());
                    }
                }

                ui.add_space(24.0);

                // Button to quote note
                if ui
                    .add(Label::new(RichText::new("»").size(18.0)).sense(Sense::click()))
                    .on_hover_text("Quote")
                    .clicked()
                {
                    if !app.draft.ends_with(' ') && !app.draft.is_empty() {
                        app.draft.push(' ');
                    }
                    app.draft
                        .push_str(&event.id.try_as_bech32_string().unwrap());
                }

                ui.add_space(24.0);

                // Button to reply
                if event.kind != EventKind::EncryptedDirectMessage {
                    if ui
                        .add(Label::new(RichText::new("💬").size(18.0)).sense(Sense::click()))
                        .on_hover_text("Reply")
                        .clicked()
                    {
                        app.replying_to = Some(event.id);
                    }

                    ui.add_space(24.0);
                }

                // Button to render raw
                if ui
                    .add(Label::new(RichText::new("🥩").size(13.0)).sense(Sense::click()))
                    .on_hover_text("Raw")
                    .clicked()
                {
                    if app.render_raw != Some(event.id) {
                        app.render_raw = Some(event.id);
                    } else {
                        app.render_raw = None;
                    }
                }

                ui.add_space(24.0);

                // Button to render QR code
                if ui
                    .add(Label::new(RichText::new("⚃").size(16.0)).sense(Sense::click()))
                    .on_hover_text("QR Code")
                    .clicked()
                {
                    if app.render_qr != Some(event.id) {
                        app.render_qr = Some(event.id);
                        app.qr_codes.remove("feedqr");
                    } else {
                        app.render_qr = None;
                        app.qr_codes.remove("feedqr");
                    }
                }

                ui.add_space(24.0);

                // Buttons to react and reaction counts
                if app.settings.reactions {
                    let default_reaction_icon = match self_already_reacted {
                        true => "♥",
                        false => "♡",
                    };
                    if ui
                        .add(
                            Label::new(RichText::new(default_reaction_icon).size(20.0))
                                .sense(Sense::click()),
                        )
                        .clicked()
                    {
                        let _ = GLOBALS
                            .to_overlord
                            .send(ToOverlordMessage::Like(event.id, event.pubkey));
                    }
                    for (ch, count) in reactions.iter() {
                        if *ch == '+' {
                            ui.label(format!("{}", count));
                        }
                    }
                    ui.add_space(12.0);
                    for (ch, count) in reactions.iter() {
                        if *ch != '+' {
                            ui.label(RichText::new(format!("{} {}", ch, count)).strong());
                        }
                    }
                }
            });
        }
    });
}

fn thin_repost_separator(ui: &mut Ui) {
    let color = if ui.visuals().dark_mode {
        Color32::from_gray(80)
    } else {
        Color32::from_gray(200)
    };
    thin_separator(ui, Stroke { width: 1.0, color });
}

fn thin_separator(ui: &mut Ui, stroke: Stroke) {
    let mut style = ui.style_mut();
    style.visuals.widgets.noninteractive.bg_stroke = stroke;
    ui.add(Separator::default().spacing(0.0));
    ui.reset_style();
}
