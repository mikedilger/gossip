use super::FeedNoteParams;
use crate::comms::ToOverlordMessage;
use crate::feed::FeedKind;
use crate::globals::{Globals, GLOBALS};
use crate::people::DbPerson;
use crate::ui::widgets::CopyButton;
use crate::ui::{GossipUi, Page};
use crate::AVATAR_SIZE_F32;
pub const AVATAR_SIZE_REPOST_F32: f32 = 27.0; // points, not pixels
use eframe::egui::{self, Margin};
use egui::{
    Align, Context, Frame, Image, Label, Layout, RichText, Sense, Separator, Stroke, TextStyle, Ui,
    Vec2,
};
use nostr_types::{Event, EventDelegation, EventKind, IdHex, PublicKeyHex, Tag};

mod content;

#[derive(PartialEq)]
enum RepostType {
    /// Damus style, kind 6 repost where the reposted note's JSON
    /// is included in the content
    Kind6Embedded,
    /// kind 6 repost without reposted note, but has a mention tag
    Kind6Mention,
    /// Post only has whitespace and a single mention tag
    MentionOnly,
    /// Post has a comment and at least one mention tag
    CommentMention,
}

pub(super) struct NoteData {
    /// Original Event object, as received from nostr
    event: Event,
    /// Delegation status of this event
    delegation: EventDelegation,
    /// Author of this note (considers delegation)
    author: DbPerson,
    /// Deletion reason if any
    deletion: Option<String>,
    /// Do we consider this note as being a repost of another?
    repost: Option<RepostType>,
    /// A list of CACHED mentioned events and their index: (index, event)
    cached_mentions: Vec<(usize, Event)>,
    /// Known reactions to this post
    reactions: Vec<(char, usize)>,
    /// Has the current user reacted to this post?
    self_already_reacted: bool,
    /// The content modified to our display needs
    display_content: String,
}

impl NoteData {
    pub fn new(event: Event, with_inline_mentions: bool, show_long_form: bool) -> Option<NoteData> {
        // We do not filter event kinds here anymore. The feed already does that.
        // There is no sense in duplicating that work.

        let delegation = event.delegation();

        let deletion = Globals::get_deletion_sync(event.id);

        let (reactions, self_already_reacted) = Globals::get_reactions_sync(event.id);

        // build a list of all cached mentions and their index
        // only notes that are in the cache will be rendered as reposts
        let cached_mentions = {
            let mut cached_mentions = Vec::<(usize, Event)>::new();
            for (i, tag) in event.tags.iter().enumerate() {
                if let Tag::Event {
                    id,
                    recommended_relay_url: _,
                    marker,
                } = tag
                {
                    if marker.is_some() && marker.as_deref().unwrap() == "mention" {
                        if let Some(event) = GLOBALS.events.get(id) {
                            cached_mentions.push((i, event));
                        }
                    }
                }
            }
            cached_mentions
        };

        let repost = {
            let content_trim = event.content.trim();
            let content_trim_len = content_trim.chars().count();
            if event.kind == EventKind::Repost
                && serde_json::from_str::<Event>(&event.content).is_ok()
            {
                if !show_long_form {
                    let inner = serde_json::from_str::<Event>(&event.content).unwrap();
                    if inner.kind == EventKind::LongFormContent {
                        return None;
                    }
                }
                Some(RepostType::Kind6Embedded)
            } else if content_trim == "#[0]" || content_trim.is_empty() {
                if !cached_mentions.is_empty() {
                    if event.kind == EventKind::Repost {
                        Some(RepostType::Kind6Mention)
                    } else {
                        Some(RepostType::MentionOnly)
                    }
                } else {
                    None
                }
            } else if with_inline_mentions
                && content_trim_len > 4
                && content_trim.chars().nth(content_trim_len - 1).unwrap() == ']'
                && content_trim.chars().nth(content_trim_len - 3).unwrap() == '['
                && content_trim.chars().nth(content_trim_len - 4).unwrap() == '#'
                && !cached_mentions.is_empty()
            {
                // matches content that ends with a mention, avoiding use of a regex match
                Some(RepostType::CommentMention)
            } else {
                None
            }
        };

        // If delegated, use the delegated person
        let author_pubkey: PublicKeyHex = if let EventDelegation::DelegatedBy(pubkey) = delegation {
            pubkey.into()
        } else {
            event.pubkey.into()
        };

        let author = match GLOBALS.people.get(&author_pubkey) {
            Some(p) => p,
            None => DbPerson::new(author_pubkey),
        };

        // Compute the content to our needs
        let display_content = match event.kind {
            EventKind::TextNote => event.content.trim().to_string(),
            EventKind::Repost => {
                if event.content.is_empty() {
                    "#[0]".to_owned() // a bit of a hack
                } else {
                    event.content.trim().to_string()
                }
            }
            EventKind::EncryptedDirectMessage => match GLOBALS.signer.decrypt_message(&event) {
                Ok(m) => m,
                Err(_) => "DECRYPTION FAILED".to_owned(),
            },
            EventKind::LongFormContent => event.content.clone(),
            _ => "NON FEED RELATED EVENT".to_owned(),
        };

        Some(NoteData {
            event,
            delegation,
            author,
            deletion,
            repost,
            cached_mentions,
            reactions,
            self_already_reacted,
            display_content,
        })
    }
}

pub struct NoteRenderData {
    /// Available height for post
    pub height: f32,
    /// Has this post been seen yet?
    pub is_new: bool,
    /// This message is the focus of the view (formerly called is_focused)
    pub is_main_event: bool,
    /// This message is a repost of another message
    pub has_repost: bool,
    /// Is this post being mentioned within a comment
    pub is_comment_mention: bool,
    /// This message is part of a thread
    pub is_thread: bool,
    /// Is this the first post in the display?
    pub is_first: bool,
    /// Is this the last post in the display
    pub is_last: bool,
    /// Position in the thread, focused message = 0
    pub thread_position: i32,
}

pub(super) fn render_note(
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

    let note_data = {
        let maybe_event = GLOBALS.events.get(&id);
        if maybe_event.is_none() {
            return;
        }
        let event = maybe_event.unwrap();

        match NoteData::new(
            event,
            app.settings.show_first_mention,
            app.settings.show_long_form,
        ) {
            Some(nd) => nd,
            None => return,
        }
    };

    if note_data.author.muted > 0 {
        return;
    }

    let is_new = app.settings.highlight_unread_events
        && !GLOBALS.viewed_events.contains(&note_data.event.id);

    let is_main_event: bool = {
        let feed_kind = GLOBALS.feed.get_feed_kind();
        match feed_kind {
            FeedKind::Thread { id, .. } => id == note_data.event.id,
            _ => false,
        }
    };

    let height = if let Some(height) = app.height.get(&id) {
        height
    } else {
        &0.0
    };

    let render_data = NoteRenderData {
        height: *height,
        has_repost: note_data.repost.is_some(),
        is_comment_mention: false,
        is_new,
        is_thread: threaded,
        is_first,
        is_last,
        is_main_event,
        thread_position: indent as i32,
    };

    let top = ui.next_widget_position();

    ui.horizontal(|ui| {
        // Outer indents first
        app.settings.theme.feed_post_outer_indent(ui, &render_data);

        let inner_response = Frame::none()
            .inner_margin(app.settings.theme.feed_frame_inner_margin(&render_data))
            .outer_margin(app.settings.theme.feed_frame_outer_margin(&render_data))
            .rounding(app.settings.theme.feed_frame_rounding(&render_data))
            .shadow(app.settings.theme.feed_frame_shadow(&render_data))
            .fill(app.settings.theme.feed_frame_fill(&render_data))
            .stroke(app.settings.theme.feed_frame_stroke(&render_data))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    // Inner indents first
                    app.settings.theme.feed_post_inner_indent(ui, &render_data);

                    render_note_inner(app, ctx, ui, note_data, &render_data, as_reply_to, &None);
                });
            });

        // Mark post as viewed if hovered AND we are not scrolling
        if inner_response.response.hovered() && app.current_scroll_offset == 0.0 {
            GLOBALS.viewed_events.insert(id);
            GLOBALS.new_viewed_events.blocking_write().insert(id);
        }
    });

    // Store actual rendered height for future reference
    let bottom = ui.next_widget_position();
    app.height.insert(id, bottom.y - top.y);

    thin_separator(
        ui,
        app.settings.theme.feed_post_separator_stroke(&render_data),
    );

    if threaded && !as_reply_to {
        let replies = Globals::get_replies_sync(id);
        let iter = replies.iter();
        let first = replies.first();
        let last = replies.last();
        for reply_id in iter {
            super::render_note_maybe_fake(
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
}

// FIXME, create some way to limit the arguments here.
fn render_note_inner(
    app: &mut GossipUi,
    ctx: &Context,
    ui: &mut Ui,
    note_data: NoteData,
    render_data: &NoteRenderData,
    hide_footer: bool,
    parent_repost: &Option<RepostType>,
) {
    let NoteData {
        event,
        delegation,
        author,
        deletion,
        repost,
        cached_mentions: _,
        reactions,
        self_already_reacted,
        display_content,
    } = &note_data;

    let collapsed = app.collapsed.contains(&event.id);

    // Load avatar texture
    let avatar = if let Some(avatar) = app.try_get_avatar(ctx, &author.pubkey) {
        avatar
    } else {
        app.placeholder_avatar.clone()
    };

    // Determine avatar size
    let avatar_size = if parent_repost.is_none() {
        match repost {
            None | Some(RepostType::CommentMention) => AVATAR_SIZE_F32,
            Some(_) => AVATAR_SIZE_REPOST_F32,
        }
    } else {
        match parent_repost {
            None | Some(RepostType::CommentMention) => AVATAR_SIZE_REPOST_F32,
            Some(_) => AVATAR_SIZE_F32,
        }
    };

    let inner_margin = app.settings.theme.feed_frame_inner_margin(render_data);

    let avatar_margin_left = if parent_repost.is_none() {
        match repost {
            None | Some(RepostType::CommentMention) => 0.0,
            Some(_) => (AVATAR_SIZE_F32 - AVATAR_SIZE_REPOST_F32) / 2.0,
        }
    } else {
        match parent_repost {
            None | Some(RepostType::CommentMention) => {
                (AVATAR_SIZE_F32 - AVATAR_SIZE_REPOST_F32) / 2.0
            }
            Some(_) => 0.0,
        }
    };

    let hide_footer = if hide_footer {
        true
    } else if parent_repost.is_none() {
        match repost {
            None | Some(RepostType::CommentMention) => false,
            Some(_) => true,
        }
    } else {
        match parent_repost {
            None | Some(RepostType::CommentMention) => true,
            Some(_) => false,
        }
    };

    let content_pull_top = inner_margin.top + ui.style().spacing.item_spacing.y * 4.0 - avatar_size;

    let content_margin_left = AVATAR_SIZE_F32 + inner_margin.left;
    let footer_margin_left = content_margin_left;

    ui.vertical(|ui| {
        // First row

        ui.with_layout(Layout::left_to_right(Align::TOP), |ui| {
            ui.add_space(avatar_margin_left);

            // render avatar
            if ui
                .add(
                    Image::new(
                        &avatar,
                        Vec2 {
                            x: avatar_size,
                            y: avatar_size,
                        },
                    )
                    .sense(Sense::click()),
                )
                .clicked()
            {
                app.set_page(Page::Person(author.pubkey.clone()));
            };

            ui.add_space(avatar_margin_left);

            ui.add_space(3.0);

            GossipUi::render_person_name_line(app, ui, author);

            ui.horizontal_wrapped(|ui| {
                if let Some((irt, _)) = event.replies_to() {
                    ui.add_space(8.0);

                    ui.style_mut().override_text_style = Some(TextStyle::Small);
                    let idhex: IdHex = irt.into();
                    let nam = format!("▲ #{}", GossipUi::hex_id_short(&idhex));
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

                match delegation {
                    EventDelegation::InvalidDelegation(why) => {
                        let color = app.settings.theme.warning_marker_text_color();
                        ui.add(Label::new(RichText::new("INVALID DELEGATION").color(color)))
                            .on_hover_text(why);
                    }
                    EventDelegation::DelegatedBy(_) => {
                        let color = app.settings.theme.notice_marker_text_color();
                        ui.label(RichText::new("DELEGATED").color(color));
                    }
                    _ => {}
                }

                if deletion.is_some() {
                    let color = app.settings.theme.warning_marker_text_color();
                    ui.label(RichText::new("DELETED").color(color));
                }

                if event.kind == EventKind::Repost {
                    let color = app.settings.theme.notice_marker_text_color();
                    ui.label(RichText::new("REPOSTED").color(color));
                }

                if event.kind == EventKind::EncryptedDirectMessage {
                    let color = app.settings.theme.notice_marker_text_color();
                    ui.label(RichText::new("ENCRYPTED DM").color(color));
                }
            });

            ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                ui.menu_button(RichText::new("=").size(13.0), |ui| {
                    if !render_data.is_main_event && event.kind != EventKind::EncryptedDirectMessage
                    {
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
                    if ui.button("Copy Raw data").clicked() {
                        ui.output_mut(|o| {
                            o.copied_text = serde_json::to_string_pretty(&event).unwrap()
                        });
                    }
                    if ui.button("Dismiss").clicked() {
                        GLOBALS.dismissed.blocking_write().push(event.id);
                    }
                });
                ui.add_space(4.0);

                let is_thread_view: bool = {
                    let feed_kind = GLOBALS.feed.get_feed_kind();
                    matches!(feed_kind, FeedKind::Thread { .. })
                };

                if is_thread_view && event.replies_to().is_some() {
                    if collapsed {
                        let color = app.settings.theme.warning_marker_text_color();
                        if ui
                            .button(RichText::new("▼").size(13.0).color(color))
                            .on_hover_text("Expand thread")
                            .clicked()
                        {
                            app.collapsed.retain(|&id| id != event.id);
                        }
                    } else {
                        if ui
                            .button(RichText::new("△").size(13.0))
                            .on_hover_text("Collapse thread")
                            .clicked()
                        {
                            app.collapsed.push(event.id);
                        }
                    }
                    ui.add_space(4.0);
                }

                if !render_data.is_main_event && event.kind != EventKind::EncryptedDirectMessage {
                    if ui
                        .button(RichText::new("◉").size(13.0))
                        .on_hover_text("View Thread")
                        .clicked()
                    {
                        app.set_page(Page::Feed(FeedKind::Thread {
                            id: event.id,
                            referenced_by: event.id,
                        }));
                    }
                }

                ui.add_space(4.0);

                let view_seen_response =
                    ui.add(Label::new(RichText::new("👁").size(12.0)).sense(Sense::hover()));
                let popup_id =
                    ui.make_persistent_id(format!("event-seen-{}", event.id.as_hex_string()));
                if view_seen_response.hovered() {
                    ui.memory_mut(|mem| mem.open_popup(popup_id));
                }
                egui::popup::popup_above_or_below_widget(
                    ui,
                    popup_id,
                    &view_seen_response,
                    egui::AboveOrBelow::Below,
                    |ui| {
                        ui.set_min_width(200.0);
                        if let Some(urls) = GLOBALS.events.get_seen_on(&event.id) {
                            for url in urls.iter() {
                                ui.label(url.as_str());
                            }
                        } else {
                            ui.label("unknown");
                        }
                    },
                );

                ui.label(
                    RichText::new(crate::date_ago::date_ago(event.created_at))
                        .italics()
                        .weak(),
                );
            });
        });

        ui.add_space(2.0);

        // MAIN CONTENT
        if !collapsed {
            let mut append_repost: Option<NoteData> = None;
            Frame::none()
                .inner_margin(Margin {
                    left: content_margin_left,
                    bottom: 0.0,
                    right: 0.0,
                    top: 0.0,
                })
                .outer_margin(Margin {
                    left: 0.0,
                    bottom: 0.0,
                    right: 0.0,
                    top: content_pull_top,
                })
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        if app.render_raw == Some(event.id) {
                            ui.label(serde_json::to_string_pretty(&event).unwrap());
                        } else if app.render_qr == Some(event.id) {
                            app.render_qr(ui, ctx, "feedqr", display_content.trim());
                        // FIXME should this be the unmodified content (event.content)?
                        } else if event.content_warning().is_some()
                            && !app.approved.contains(&event.id)
                        {
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
                                if let Some(inner_note_data) =
                                    NoteData::new(inner_event, false, app.settings.show_long_form)
                                {
                                    append_repost = Some(inner_note_data);
                                } else {
                                    ui.label("REPOSTED EVENT IS NOT RELEVANT");
                                }
                            } else {
                                // Possible subject line
                                render_subject(ui, event);

                                // render like a kind-1 event with a mention
                                append_repost = content::render_content(
                                    app,
                                    ui,
                                    &note_data,
                                    deletion.is_some(),
                                    display_content,
                                );
                            }
                        } else {
                            // Possible subject line
                            render_subject(ui, event);

                            append_repost = content::render_content(
                                app,
                                ui,
                                &note_data,
                                deletion.is_some(),
                                display_content,
                            );
                        }
                    });
                });

            // render any repost without frame or indent
            if let Some(repost) = append_repost {
                render_repost(app, ui, ctx, &note_data, repost)
            }

            // deleted?
            if let Some(delete_reason) = &deletion {
                Frame::none()
                    .inner_margin(Margin {
                        left: footer_margin_left,
                        bottom: 0.0,
                        right: 0.0,
                        top: 8.0,
                    })
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(format!("Deletion Reason: {}", delete_reason)).italics(),
                        );
                    });
            }

            // Footer
            if !hide_footer {
                Frame::none()
                    .inner_margin(Margin {
                        left: footer_margin_left,
                        bottom: 0.0,
                        right: 0.0,
                        top: 8.0,
                    })
                    .outer_margin(Margin {
                        left: 0.0,
                        bottom: 0.0,
                        right: 0.0,
                        top: 0.0,
                    })
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            if ui
                                .add(CopyButton {})
                                .on_hover_text("Copy Contents")
                                .clicked()
                            {
                                if app.render_raw == Some(event.id) {
                                    ui.output_mut(|o| {
                                        o.copied_text = serde_json::to_string(&event).unwrap()
                                    });
                                } else {
                                    ui.output_mut(|o| o.copied_text = display_content.clone());
                                }
                            }

                            ui.add_space(24.0);

                            // Button to quote note
                            if ui
                                .add(
                                    Label::new(RichText::new("»").size(18.0)).sense(Sense::click()),
                                )
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
                                    .add(
                                        Label::new(RichText::new("💬").size(18.0))
                                            .sense(Sense::click()),
                                    )
                                    .on_hover_text("Reply")
                                    .clicked()
                                {
                                    app.replying_to = Some(event.id);
                                }

                                ui.add_space(24.0);
                            }

                            // Button to render raw
                            if ui
                                .add(
                                    Label::new(RichText::new("🥩").size(13.0))
                                        .sense(Sense::click()),
                                )
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
                                .add(
                                    Label::new(RichText::new("⚃").size(16.0)).sense(Sense::click()),
                                )
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
                                        ui.label(
                                            RichText::new(format!("{} {}", ch, count)).strong(),
                                        );
                                    }
                                }
                            }
                        });
                    });
            }
        }
    });
}

fn thin_separator(ui: &mut Ui, stroke: Stroke) {
    let mut style = ui.style_mut();
    style.visuals.widgets.noninteractive.bg_stroke = stroke;
    ui.add(Separator::default().spacing(0.0));
    ui.reset_style();
}

fn render_subject(ui: &mut Ui, event: &Event) {
    if let Some(subject) = event.subject() {
        ui.style_mut().spacing.item_spacing.x = 0.0;
        ui.style_mut().spacing.item_spacing.y = 10.0;
        ui.label(RichText::new(subject).text_style(TextStyle::Name("subject".into())));
        ui.end_row();
        ui.reset_style();
    }
}

pub(super) fn render_repost(
    app: &mut GossipUi,
    ui: &mut Ui,
    ctx: &Context,
    parent_data: &NoteData,
    repost_data: NoteData,
) {
    let render_data = NoteRenderData {
        height: 0.0,
        has_repost: repost_data.repost.is_some(),
        is_comment_mention: parent_data.repost == Some(RepostType::CommentMention),
        is_new: false,
        is_main_event: false,
        is_thread: false,
        is_first: false,
        is_last: false,
        thread_position: 0,
    };

    ui.vertical(|ui| {
        ui.add_space(
            app.settings
                .theme
                .repost_space_above_separator_before(&render_data),
        );
        thin_separator(
            ui,
            app.settings
                .theme
                .repost_separator_before_stroke(&render_data),
        );
        ui.add_space(
            app.settings
                .theme
                .repost_space_below_separator_before(&render_data),
        );
        Frame::none()
            .inner_margin(app.settings.theme.repost_inner_margin(&render_data))
            .outer_margin(app.settings.theme.repost_outer_margin(&render_data))
            .rounding(app.settings.theme.repost_rounding(&render_data))
            .shadow(app.settings.theme.repost_shadow(&render_data))
            .fill(app.settings.theme.repost_fill(&render_data))
            .stroke(app.settings.theme.repost_stroke(&render_data))
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    // FIXME: don't do this recursively
                    render_note_inner(
                        app,
                        ctx,
                        ui,
                        repost_data,
                        &render_data,
                        false,
                        &parent_data.repost,
                    );
                });
            });
        ui.add_space(
            app.settings
                .theme
                .repost_space_above_separator_after(&render_data),
        );
        thin_separator(
            ui,
            app.settings
                .theme
                .repost_separator_after_stroke(&render_data),
        );
        ui.add_space(
            app.settings
                .theme
                .repost_space_below_separator_after(&render_data),
        );
    });
}
