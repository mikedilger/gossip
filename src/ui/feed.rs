use super::{GossipUi, Page};
use crate::comms::ToOverlordMessage;
use crate::feed::FeedKind;
use crate::globals::{Globals, GLOBALS};
use crate::ui::widgets::CopyButton;
use crate::AVATAR_SIZE_F32;
use eframe::egui;
use egui::{
    Align, Color32, Context, Frame, Image, Label, Layout, RichText, ScrollArea, SelectableLabel,
    Sense, Separator, Stroke, TextEdit, TextStyle, Ui, Vec2,
};
use linkify::{LinkFinder, LinkKind};
use nostr_types::{Event, EventKind, Id, IdHex, Tag};
use std::sync::atomic::Ordering;

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

    posting_area(app, ctx, frame, ui);

    ui.separator();

    match feed_kind {
        FeedKind::General => {
            let feed = GLOBALS.feed.get_general();
            render_a_feed(app, ctx, frame, ui, feed, false);
        }
        FeedKind::Replies => {
            if GLOBALS.signer.blocking_read().public_key().is_none() {
                ui.horizontal(|ui| {
                    ui.label("You need to ");
                    if ui.link("setup an identity").clicked() {
                        app.set_page(Page::Relays);
                    }
                    ui.label(" to see any replies to that identity.");
                });
            }
            let feed = GLOBALS.feed.get_replies();
            render_a_feed(app, ctx, frame, ui, feed, true);
        }
        FeedKind::Thread { .. } => {
            if let Some(parent) = GLOBALS.feed.get_thread_parent() {
                render_a_feed(app, ctx, frame, ui, vec![parent], true);
            }
        }
        FeedKind::Person(pubkeyhex) => {
            let feed = GLOBALS.feed.get_person_feed(pubkeyhex);
            render_a_feed(app, ctx, frame, ui, feed, false);
        }
    }
}

fn posting_area(app: &mut GossipUi, ctx: &Context, frame: &mut eframe::Frame, ui: &mut Ui) {
    // Posting Area
    ui.vertical(|ui| {
        if !GLOBALS.signer.blocking_read().is_ready() {
            ui.horizontal(|ui| {
                ui.label("You need to ");
                if ui.link("setup your identity").clicked() {
                    app.set_page(Page::You);
                }
                ui.label(" to post.");
            });
        } else if !GLOBALS.relays.blocking_read().iter().any(|(_, r)| r.post) {
            ui.horizontal(|ui| {
                ui.label("You need to ");
                if ui.link("choose relays").clicked() {
                    app.set_page(Page::Relays);
                }
                ui.label(" to post.");
            });
        } else {
            real_posting_area(app, ctx, frame, ui);
        }
    });
}

fn real_posting_area(app: &mut GossipUi, ctx: &Context, frame: &mut eframe::Frame, ui: &mut Ui) {
    // Maybe render post we are replying to
    if let Some(id) = app.replying_to {
        render_post_actual(
            app,
            ctx,
            frame,
            ui,
            FeedPostParams {
                id,
                indent: 0,
                as_reply_to: true,
                threaded: false,
            },
        );
    }

    ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
        // Buttons
        ui.with_layout(Layout::top_down(Align::RIGHT), |ui| {
            if ui.button("Send").clicked() && !app.draft.is_empty() {
                match app.replying_to {
                    Some(replying_to_id) => {
                        let _ = GLOBALS.to_overlord.send(ToOverlordMessage::PostReply(
                            app.draft.clone(),
                            app.draft_tags.clone(),
                            replying_to_id,
                        ));
                    }
                    None => {
                        let _ = GLOBALS.to_overlord.send(ToOverlordMessage::PostTextNote(
                            app.draft.clone(),
                            app.draft_tags.clone(),
                        ));
                    }
                }
                app.draft = "".to_owned();
                app.draft_tags = vec![];
                app.replying_to = None;
            }

            if ui.button("Cancel").clicked() {
                app.draft = "".to_owned();
                app.draft_tags = vec![];
                app.replying_to = None;
            }

            ui.add(
                TextEdit::singleline(&mut app.tag_someone)
                    .desired_width(100.0)
                    .hint_text("@username"),
            );
            if !app.tag_someone.is_empty() {
                let pairs = GLOBALS.people.search_people_to_tag(&app.tag_someone);
                if !pairs.is_empty() {
                    ui.menu_button("@", |ui| {
                        for pair in pairs {
                            if ui.button(pair.0).clicked() {
                                let idx = app
                                    .draft_tags
                                    .iter()
                                    .position(|tag| {
                                        matches!(
                                            tag,
                                            Tag::Pubkey { pubkey, .. } if pubkey.0 == *pair.1
                                        )
                                    })
                                    .unwrap_or_else(|| {
                                        app.draft_tags.push(Tag::Pubkey {
                                            pubkey: pair.1,
                                            recommended_relay_url: None, // FIXME
                                            petname: None,
                                        });
                                        app.draft_tags.len() - 1
                                    });
                                app.draft.push_str(&format!("#[{}]", idx));
                                app.tag_someone = "".to_owned();
                            }
                        }
                    });
                }
            }
        });

        // Text area
        ui.add(
            TextEdit::multiline(&mut app.draft)
                .hint_text("Type your message here")
                .desired_width(f32::INFINITY)
                .lock_focus(true),
        );
    });

    // List of tags to be applied
    for (i, tag) in app.draft_tags.iter().enumerate() {
        let rendered = match tag {
            Tag::Pubkey { pubkey, .. } => {
                if let Some(person) = GLOBALS.people.get(pubkey) {
                    match person.name {
                        Some(name) => name,
                        None => pubkey.0.clone(),
                    }
                } else {
                    pubkey.0.clone()
                }
            }
            _ => serde_json::to_string(tag).unwrap(),
        };
        ui.label(format!("{}: {}", i, rendered));
    }
}

fn render_a_feed(
    app: &mut GossipUi,
    ctx: &Context,
    frame: &mut eframe::Frame,
    ui: &mut Ui,
    feed: Vec<Id>,
    threaded: bool,
) {
    ScrollArea::vertical().show(ui, |ui| {
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

    let maybe_person = GLOBALS.people.get(&event.pubkey.into());

    let reactions = Globals::get_reactions_sync(event.id);

    let tag_re = app.tag_re.clone();

    // Person Things we can render:
    // pubkey
    // name
    // about
    // picture
    // dns_id
    // dns_id_valid
    // dns_id_last_checked
    // metadata_at
    // followed

    // Event Things we can render:
    // id
    // pubkey
    // created_at,
    // kind,
    // tags,
    // content,
    // ots,
    // sig
    // feed_related,
    // replies,
    // in_reply_to,
    // reactions,
    // deleted_reason,
    // client,
    // hashtags,
    // subject,
    // urls,
    // last_reply_at

    // Try LayoutJob

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

        ui.horizontal(|ui| {
            // Indents first (if threaded)
            if threaded {
                let space = 100.0 * (10.0 - (1000.0 / (indent as f32 + 100.0)));
                ui.add_space(space);
                if indent > 0 {
                    ui.label(RichText::new(format!("{}>", indent)).italics().weak());
                }
            }

            // Avatar first
            let avatar = if let Some(avatar) = app.try_get_avatar(ctx, &event.pubkey.into()) {
                avatar
            } else {
                app.placeholder_avatar.clone()
            };
            let size = AVATAR_SIZE_F32
                * GLOBALS.pixels_per_point_times_100.load(Ordering::Relaxed) as f32
                / 100.0;
            if ui
                .add(Image::new(&avatar, Vec2 { x: size, y: size }).sense(Sense::click()))
                .clicked()
            {
                app.set_page(Page::Person(event.pubkey.into()));
            };

            // Everything else next
            ui.vertical(|ui| {
                // First row
                ui.horizontal(|ui| {
                    GossipUi::render_person_name_line(ui, maybe_person.as_ref());

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

                    ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                        ui.menu_button(RichText::new("≡").size(28.0), |ui| {
                            if !is_main_event && ui.button("View Thread").clicked() {
                                app.set_page(Page::Feed(FeedKind::Thread {
                                    id: event.id,
                                    referenced_by: event.id,
                                }));
                            }
                            if ui.button("Copy ID").clicked() {
                                ui.output().copied_text = event.id.as_hex_string();
                            }
                            if ui.button("Dismiss").clicked() {
                                GLOBALS.dismissed.blocking_write().push(event.id);
                            }
                            if ui.button("Update Metadata").clicked() {
                                let _ = GLOBALS
                                    .to_overlord
                                    .send(ToOverlordMessage::UpdateMetadata(event.pubkey.into()));
                            }
                        });

                        if !is_main_event && ui.button("➤").on_hover_text("View Thread").clicked()
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

                ui.horizontal_wrapped(|ui| {
                    render_content(app, ui, &tag_re, &event);
                });

                ui.add_space(8.0);

                // Under row
                if !as_reply_to {
                    ui.horizontal(|ui| {
                        if ui.add(CopyButton {}).clicked() {
                            ui.output().copied_text = event.content.clone();
                        }

                        ui.add_space(24.0);

                        if ui
                            .add(Label::new(RichText::new("💬").size(18.0)).sense(Sense::click()))
                            .clicked()
                        {
                            app.replying_to = Some(event.id);

                            // Cleanup tags
                            app.draft_tags = vec![];
                            // Add a 'p' tag for the author we are replying to (except if it is our own key)
                            if let Some(pubkey) = GLOBALS.signer.blocking_read().public_key() {
                                if pubkey != event.pubkey {
                                    app.draft_tags.push(Tag::Pubkey {
                                        pubkey: event.pubkey.into(),
                                        recommended_relay_url: None, // FIXME
                                        petname: None,
                                    });
                                }
                            }
                        }

                        ui.add_space(24.0);

                        if app.settings.reactions {
                            if ui
                                .add(
                                    Label::new(RichText::new("♡").size(20.0)).sense(Sense::click()),
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
        });

        if is_main_event {
            thin_red_separator(ui);
        }
    });

    ui.separator();

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
}

fn render_content(app: &mut GossipUi, ui: &mut Ui, tag_re: &regex::Regex, event: &Event) {
    for span in LinkFinder::new()
        .kinds(&[LinkKind::Url])
        .spans(&event.content)
    {
        if span.kind().is_some() {
            ui.hyperlink_to(span.as_str(), span.as_str());
        } else {
            let s = span.as_str();
            let mut pos = 0;
            for mat in tag_re.find_iter(s) {
                ui.label(&s[pos..mat.start()]);
                let num: usize = s[mat.start() + 2..mat.end() - 1].parse::<usize>().unwrap();
                if let Some(tag) = event.tags.get(num) {
                    match tag {
                        Tag::Pubkey { pubkey, .. } => {
                            let nam = match GLOBALS.people.get(pubkey) {
                                Some(p) => match p.name {
                                    Some(n) => format!("@{}", n),
                                    None => format!("@{}", GossipUi::hex_pubkey_short(pubkey)),
                                },
                                None => format!("@{}", GossipUi::hex_pubkey_short(pubkey)),
                            };
                            if ui.link(&nam).clicked() {
                                app.set_page(Page::Person(pubkey.to_owned()));
                            };
                        }
                        Tag::Event { id, .. } => {
                            let idhex: IdHex = (*id).into();
                            let nam = format!("#{}", GossipUi::hex_id_short(&idhex));
                            if ui.link(&nam).clicked() {
                                app.set_page(Page::Feed(FeedKind::Thread {
                                    id: *id,
                                    referenced_by: event.id,
                                }));
                            };
                        }
                        Tag::Hashtag(s) => {
                            if ui.link(format!("#{}", s)).clicked() {
                                *GLOBALS.status_message.blocking_write() =
                                    "Gossip doesn't have a hashtag feed yet.".to_owned();
                            }
                        }
                        _ => {
                            if ui.link(format!("#[{}]", num)).clicked() {
                                *GLOBALS.status_message.blocking_write() =
                                    "Gossip can't handle this kind of tag link yet.".to_owned();
                            }
                        }
                    }
                }
                pos = mat.end();
            }
            ui.label(&s[pos..]);
        }
    }
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
