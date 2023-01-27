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
                "Replies",
            ))
            .clicked()
        {
            app.set_page(Page::Feed(FeedKind::Replies));
        }
        if matches!(feed_kind.clone(), FeedKind::Thread { .. }) {
            ui.separator();
            ui.selectable_value(&mut app.page, Page::Feed(feed_kind.clone()), "Thread");
            GLOBALS.events.clear_new();
        }
        if matches!(feed_kind, FeedKind::Person(..)) {
            ui.separator();
            ui.selectable_value(&mut app.page, Page::Feed(feed_kind.clone()), "Person");
            GLOBALS.events.clear_new();
        }
    });
    ui.separator();

    post::posting_area(app, ctx, frame, ui);

    ui.separator();

    match feed_kind {
        FeedKind::General => {
            let feed = GLOBALS.feed.get_general();
            render_a_feed(app, ctx, frame, ui, feed, false, "general");
        }
        FeedKind::Replies => {
            if GLOBALS.signer.blocking_read().public_key().is_none() {
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
            render_a_feed(app, ctx, frame, ui, feed, false, &pubkeyhex.0);
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
        .show(ui, |ui| {
            let bgcolor = if ctx.style().visuals.dark_mode {
                Color32::BLACK
            } else {
                Color32::WHITE
            };
            Frame::none().fill(bgcolor).show(ui, |ui| {
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

    let maybe_person = GLOBALS.people.get(&event.pubkey.into());

    let screen_rect = ctx.input().screen_rect; // Rect
    let pos2 = ui.next_widget_position();

    // If too far off of the screen, don't actually render the post, just make some space
    // so the scrollbar isn't messed up
    if pos2.y < -2000.0 || pos2.y > screen_rect.max.y + 2000.0 {
        // ESTIMATE HEIGHT
        // This doesn't have to be perfect, but the closer we are, the less wobbly the scroll bar is.
        // This is affected by font size, so adjust if we add that as a setting.
        // A single-line post currently is 110 pixels high.  Every additional line adds 18 pixels.
        let mut height = 92.0;
        let mut lines = event.content.lines().count();
        // presume wrapping at 80 chars, although window width makes a big diff.
        lines += event.content.lines().filter(|l| l.len() > 80).count();
        height += 18.0 * (lines as f32);

        // Muted posts are short
        if let Some(person) = maybe_person {
            if person.muted > 0 {
                height = 92.0;
            }
        }

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

    // Only render TextNote events
    if event.kind != EventKind::TextNote {
        return;
    }

    let person = match GLOBALS.people.get(&event.pubkey.into()) {
        Some(p) => p,
        None => DbPerson::new(event.pubkey.into()),
    };

    #[allow(clippy::collapsible_else_if)]
    let bgcolor = if GLOBALS.events.is_new(&event.id) {
        if ctx.style().visuals.dark_mode {
            Color32::from_rgb(60, 0, 0)
        } else {
            Color32::LIGHT_YELLOW
        }
    } else {
        if ctx.style().visuals.dark_mode {
            Color32::BLACK
        } else {
            Color32::WHITE
        }
    };

    let is_main_event: bool = {
        let feed_kind = GLOBALS.feed.get_feed_kind();
        match feed_kind {
            FeedKind::Thread { id, .. } => id == event.id,
            _ => false,
        }
    };

    Frame::none().fill(bgcolor).show(ui, |ui| {
        if is_main_event {
            thin_red_separator(ui);
        }

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
                ui.label("MUTED POST");
            } else {
                render_post_inner(app, ctx, ui, event, person, is_main_event, as_reply_to);
            }
        });

        if is_main_event {
            thin_red_separator(ui);
        }
    });

    ui.separator();

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

    let reactions = Globals::get_reactions_sync(event.id);

    let tag_re = app.tag_re.clone();

    // Avatar first
    let avatar = if let Some(avatar) = app.try_get_avatar(ctx, &event.pubkey.into()) {
        avatar
    } else {
        app.placeholder_avatar.clone()
    };
    let size =
        AVATAR_SIZE_F32 * GLOBALS.pixels_per_point_times_100.load(Ordering::Relaxed) as f32 / 100.0;
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
            GossipUi::render_person_name_line(ui, &person);

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

            ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                ui.menu_button(RichText::new("≡").size(18.0), |ui| {
                    if !is_main_event && ui.button("View Thread").clicked() {
                        app.set_page(Page::Feed(FeedKind::Thread {
                            id: event.id,
                            referenced_by: event.id,
                        }));
                    }
                    if ui.button("Copy ID").clicked() {
                        ui.output().copied_text = event.id.try_as_bech32_string().unwrap();
                    }
                    if ui.button("Copy ID as hex").clicked() {
                        ui.output().copied_text = event.id.as_hex_string();
                    }
                    if ui.button("Dismiss").clicked() {
                        GLOBALS.dismissed.blocking_write().push(event.id);
                    }
                    if ui.button("Mute").clicked() {
                        GLOBALS.people.mute(&event.pubkey.into(), true);
                    }
                    if person.followed == 0 && ui.button("Follow").clicked() {
                        GLOBALS.people.follow(&event.pubkey.into(), true);
                    } else if person.followed == 1 && ui.button("Unfollow").clicked() {
                        GLOBALS.people.follow(&event.pubkey.into(), false);
                    }
                    if ui.button("Update Metadata").clicked() {
                        let _ = GLOBALS
                            .to_overlord
                            .send(ToOverlordMessage::UpdateMetadata(event.pubkey.into()));
                    }
                });

                if !is_main_event
                    && ui
                        .button(RichText::new("➤").size(18.0))
                        .on_hover_text("View Thread")
                        .clicked()
                {
                    app.set_page(Page::Feed(FeedKind::Thread {
                        id: event.id,
                        referenced_by: event.id,
                    }));
                }

                ui.label(
                    RichText::new(crate::date_ago::date_ago(event.created_at))
                        .italics()
                        .weak(),
                );
            });
        });

        // Possible subject line
        if let Some(subject) = event.subject() {
            ui.label(RichText::new(subject).strong().underline());
        }

        // MAIN CONTENT
        ui.horizontal_wrapped(|ui| {
            if app.render_raw == Some(event.id) {
                ui.label(serde_json::to_string(&event).unwrap());
            } else if app.render_qr == Some(event.id) {
                content::render_qr(app, ui, ctx, &event.content);
            } else {
                content::render_content(app, ui, &tag_re, &event, deletion.is_some());
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
                        ui.output().copied_text = serde_json::to_string(&event).unwrap();
                    } else {
                        ui.output().copied_text = event.content.clone();
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
                if ui
                    .add(Label::new(RichText::new("💬").size(18.0)).sense(Sense::click()))
                    .on_hover_text("Reply")
                    .clicked()
                {
                    app.replying_to = Some(event.id);
                }

                ui.add_space(24.0);

                // Button to render raw
                if ui
                    .add(Label::new(RichText::new("🥩").size(16.0)).sense(Sense::click()))
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
                        app.current_qr = None;
                    } else {
                        app.render_qr = None;
                        app.current_qr = None;
                    }
                }

                ui.add_space(24.0);

                // Buttons to react and reaction counts
                if app.settings.reactions {
                    if ui
                        .add(Label::new(RichText::new("♡").size(20.0)).sense(Sense::click()))
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

fn thin_red_separator(ui: &mut Ui) {
    let mut style = ui.style_mut();
    style.visuals.widgets.noninteractive.bg_stroke = Stroke {
        width: 1.0,
        color: Color32::from_rgb(160, 0, 0),
    };
    ui.add(Separator::default().spacing(0.0));
    ui.reset_style();
}
