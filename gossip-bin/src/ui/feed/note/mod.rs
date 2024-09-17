mod content;

use std::cell::RefCell;
use std::ops::Add;
use std::rc::Rc;

use crate::notedata::{EncryptionType, NoteData, RepostType};

use super::FeedNoteParams;
use crate::ui::widgets::{
    self, AvatarSize, CopyButton, ModalEntry, MoreMenuButton, MoreMenuItem, MoreMenuSubMenu,
};
use crate::ui::{GossipUi, Page};
use crate::{AVATAR_SIZE_F32, AVATAR_SIZE_REPOST_F32};

use eframe::egui::{self, vec2, Align2, Margin, Response};
use egui::{
    Align, Context, Frame, Label, Layout, RichText, Sense, Separator, Stroke, TextStyle, Ui,
};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::{relay, DmChannel, FeedKind, ZapState, GLOBALS};
use nostr_types::{
    Event, EventDelegation, EventKind, EventReference, IdHex, NAddr, NEvent, NostrUrl, UncheckedUrl,
};
use serde::Serialize;

const CONTENT_MARGIN_RIGHT: f32 = 35.0;

#[derive(Default)]
pub struct NoteRenderData {
    /// Height of the post
    /// This is only used in feed_post_inner_indent() and is often just set to 0.0, but should
    /// be taken from app.height if we can get that data.
    pub height: f32,

    /// Has this post been seen yet?
    pub is_new: bool,

    /// This message is the focus of the view (formerly called is_focused)
    pub is_main_event: bool,

    /// Can we load an addtional thread for this note (false if already are in thread)
    pub can_load_thread: bool,

    /// Is this post being mentioned within a comment
    pub is_comment_mention: bool,

    /// This message is part of a thread
    pub is_thread: bool,

    /// Position in the thread, focused message = 0
    pub thread_position: i32,

    /// Should hide footer
    pub hide_footer: bool,

    /// Should hide nameline
    pub hide_nameline: bool,
}

pub(super) fn render_note(
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

    let mut replies = Vec::new();

    if let Some(note_ref) = app.notecache.try_update_and_get(&id) {
        // FIXME respect app.settings.show_long_form on reposts
        // FIXME drop the cached notes on recompute

        if let Ok(note_data) = note_ref.try_borrow() {
            let skip = ((note_data.muted() && read_setting!(hide_mutes_entirely))
                && !matches!(app.page, Page::Feed(FeedKind::DmChat(_)))
                && !matches!(app.page, Page::Feed(FeedKind::Person(_))))
                || (!note_data.deletions.is_empty() && !read_setting!(show_deleted_events));

            if skip {
                return;
            }

            let viewed = matches!(app.page, Page::Feed(FeedKind::Global))
                || GLOBALS
                    .db()
                    .is_event_viewed(note_data.event.id)
                    .unwrap_or(false);

            let is_new = read_setting!(highlight_unread_events) && !viewed;

            let is_main_event: bool = {
                let feed_kind = GLOBALS.feed.get_feed_kind();
                match feed_kind {
                    FeedKind::Thread { id, .. } => id == note_data.event.id,
                    _ => false,
                }
            };

            let height = match app.height.get(&id) {
                Some(h) => *h,
                None => 0.0,
            };

            let render_data = NoteRenderData {
                height,
                is_comment_mention: false,
                is_new,
                is_thread: threaded,
                is_main_event,
                can_load_thread: !is_main_event,
                thread_position: indent as i32,
                hide_footer: as_reply_to,
                hide_nameline: false,
            };

            let top = ui.next_widget_position();

            let inner_response = ui.horizontal(|ui| {
                // Outer indents first
                app.theme.feed_post_outer_indent(ui, &render_data);

                Frame::none()
                    .inner_margin(app.theme.feed_frame_inner_margin(&render_data))
                    .outer_margin(app.theme.feed_frame_outer_margin(&render_data))
                    .rounding(app.theme.feed_frame_rounding(&render_data))
                    .shadow(app.theme.feed_frame_shadow(&render_data))
                    .fill(app.theme.feed_frame_fill(&render_data))
                    .stroke(app.theme.feed_frame_stroke(&render_data))
                    .show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            // Inner indents first
                            app.theme.feed_post_inner_indent(ui, &render_data);

                            render_note_inner(app, ui, note_ref.clone(), &render_data, &None);
                        });
                    })
            });

            // Store actual rendered height for future reference
            let bottom = ui.next_widget_position();
            app.height.insert(id, bottom.y - top.y);

            // scroll to this note if it's the main note of a thread and the user hasn't scrolled yet
            if is_main_event && app.feeds.thread_needs_scroll {
                // keep auto-scrolling until user scrolls
                if app.is_scrolling() {
                    app.feeds.thread_needs_scroll = false;
                }
                // only request scrolling if the note is not completely visible
                if !ui.clip_rect().contains_rect(inner_response.response.rect) {
                    inner_response.response.scroll_to_me(Some(Align::Center));
                }
            }

            // Mark post as viewed if hovered AND we are not scrolling
            if !viewed
                && ui
                    .interact(
                        inner_response.response.rect,
                        ui.next_auto_id().with("hov"),
                        egui::Sense::hover(),
                    )
                    .hovered()
                && !app.is_scrolling()
            {
                let _ = GLOBALS.db().mark_event_viewed(id, None);
            }

            // Record if the rendered note was visible
            {
                let screen_rect = ctx.input(|i| i.screen_rect); // Rect
                let offscreen = bottom.y < 0.0 || top.y > screen_rect.max.y;
                if !offscreen {
                    // Record that this note was visibly rendered
                    app.next_visible_note_ids.push(id);
                }
            }

            thin_separator(ui, app.theme.feed_post_separator_stroke(&render_data));

            // Load replies variable for next section, while we have note_data borrowed
            if threaded && !as_reply_to && !app.collapsed.contains(&id) {
                replies = GLOBALS
                    .db()
                    .get_replies(&note_data.event)
                    .unwrap_or_default();
            }
        }

        // even if muted, continue rendering thread children
        if threaded && !as_reply_to && !app.collapsed.contains(&id) {
            let iter = replies.iter();
            for reply_id in iter {
                super::render_note_maybe_fake(
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
    }
}

pub fn render_dm_note(app: &mut GossipUi, ui: &mut Ui, feed_note_params: FeedNoteParams) {
    let FeedNoteParams {
        id,
        indent,
        as_reply_to: _,
        threaded,
    } = feed_note_params;

    if let Some(note_ref) = app.notecache.try_update_and_get(&id) {
        if let Ok(note_data) = note_ref.try_borrow() {
            let viewed = match GLOBALS.db().is_event_viewed(note_data.event.id) {
                Ok(answer) => answer,
                _ => false,
            };

            if let Ok(mut note_data) = note_ref.try_borrow_mut() {
                note_data.repost = Some(RepostType::GenericRepost);
            }

            let is_new = read_setting!(highlight_unread_events) && !viewed;

            let render_data = NoteRenderData {
                height: 0.0,
                is_comment_mention: false,
                is_new,
                is_thread: threaded,
                is_main_event: false,
                can_load_thread: false,
                thread_position: indent as i32,
                hide_footer: false,
                hide_nameline: true,
            };

            let inner_response =
                widgets::list_entry::make_frame(ui, Some(app.theme.feed_frame_fill(&render_data)))
                    .rounding(app.theme.feed_frame_rounding(&render_data))
                    .shadow(app.theme.feed_frame_shadow(&render_data))
                    .stroke(app.theme.feed_frame_stroke(&render_data))
                    .show(ui, |ui| {
                        render_note_inner(app, ui, note_ref.clone(), &render_data, &None);
                    });

            // Mark post as viewed if hovered AND we are not scrolling
            if !viewed && inner_response.response.hovered() && !app.is_scrolling() {
                let _ = GLOBALS.db().mark_event_viewed(id, None);
            }
        }
    }
}

// FIXME, create some way to limit the arguments here.
pub fn render_note_inner(
    app: &mut GossipUi,
    ui: &mut Ui,
    note_ref: Rc<RefCell<NoteData>>,
    render_data: &NoteRenderData,
    parent_repost: &Option<RepostType>,
) {
    struct EncryptionIndicator {
        pub color: egui::Color32,
        pub tooltip_ui: Box<dyn FnOnce(&mut Ui)>,
    }

    if let Ok(note) = note_ref.try_borrow() {
        let collapsed = app.collapsed.contains(&note.event.id);

        let is_dm_feed = matches!(app.page, Page::Feed(FeedKind::DmChat(_)));

        // Load avatar texture
        let avatar = if note.muted() {
            // no avatars for muted people
            app.placeholder_avatar.clone()
        } else if let Some(avatar) = app.try_get_avatar(ui.ctx(), &note.author.pubkey) {
            avatar
        } else {
            app.placeholder_avatar.clone()
        };

        let inner_margin = app.theme.feed_frame_inner_margin(render_data);

        // Determine avatar size
        let (avatar_size, avatar_margin_left, content_margin_left) = if is_dm_feed {
            (
                AvatarSize::Mini,
                0.0,
                AVATAR_SIZE_REPOST_F32 + inner_margin.left + 5.0,
            )
        } else if parent_repost.is_none() {
            match note.repost {
                None | Some(RepostType::CommentMention) => {
                    (AvatarSize::Feed, 0.0, AVATAR_SIZE_F32 + inner_margin.left)
                }
                Some(_) => (
                    AvatarSize::Mini,
                    (AVATAR_SIZE_F32 - AVATAR_SIZE_REPOST_F32) / 2.0,
                    AVATAR_SIZE_F32 + inner_margin.left,
                ),
            }
        } else {
            match parent_repost {
                None | Some(RepostType::CommentMention) => (
                    AvatarSize::Mini,
                    (AVATAR_SIZE_F32 - AVATAR_SIZE_REPOST_F32) / 2.0,
                    AVATAR_SIZE_F32 + inner_margin.left,
                ),
                Some(_) => (AvatarSize::Feed, 0.0, AVATAR_SIZE_F32 + inner_margin.left),
            }
        };

        let hide_footer = if render_data.hide_footer {
            true
        } else if parent_repost.is_none() {
            match note.repost {
                None | Some(RepostType::CommentMention) => false,
                Some(_) => true,
            }
        } else {
            match parent_repost {
                None | Some(RepostType::CommentMention) => true,
                Some(_) => false,
            }
        };

        let content_pull_top = if !render_data.hide_nameline {
            inner_margin.top + ui.style().spacing.item_spacing.y * 4.0 - avatar_size.y()
        } else {
            -avatar_size.y() - ui.style().spacing.item_spacing.y * 2.0
        };

        let content_margin_right = if render_data.hide_nameline {
            CONTENT_MARGIN_RIGHT
        } else {
            0.0
        };
        let footer_margin_left = content_margin_left;

        let content_inner_margin = Margin {
            left: content_margin_left,
            right: content_margin_right,
            top: 0.0,
            bottom: 0.0,
        };

        let content_outer_margin = Margin {
            left: 0.0,
            bottom: 0.0,
            right: 0.0,
            top: content_pull_top,
        };

        // TODO remove dependency on GossipUi struct for this decision
        let encryption_indicator = if let Page::Feed(FeedKind::DmChat(_)) = app.page {
            // show an icon that shows the encryption standard
            match note.encryption {
                EncryptionType::None => Some(EncryptionIndicator {
                    color: egui::Color32::RED,
                    tooltip_ui: Box::new(|ui: &mut Ui| {
                        ui.label("Error: Encyption in DM Channel should never be 'None'");
                    }),
                }),
                EncryptionType::Nip04 => Some(EncryptionIndicator {
                    color: app.theme.amber_400(),
                    tooltip_ui: Box::new(|ui: &mut Ui| {
                        ui.label("NIP-04 encryption. It is recomended to upgrade [link to help page] to Giftwrap (NIP-44) encryption.");
                    }),
                }),
                EncryptionType::Giftwrap => None, // Giftwrap is the new good default, we won't show an indicator
            }
        } else {
            None
        };

        let seen_location = if is_dm_feed {
            Align2::RIGHT_BOTTOM
        } else {
            Align2::RIGHT_TOP
        };

        ui.vertical(|ui| {
            // First row

            let header_response = ui.with_layout(Layout::left_to_right(Align::TOP), |ui| {
                ui.add_space(avatar_margin_left);

                // render avatar
                if is_dm_feed {
                    if widgets::paint_avatar_only(ui, &avatar, avatar_size.get_size()).clicked() {
                        app.set_page(ui.ctx(), Page::Person(note.author.pubkey));
                    };
                } else {
                    if widgets::paint_avatar(ui, &note.author, &avatar, avatar_size).clicked() {
                        app.set_page(ui.ctx(), Page::Person(note.author.pubkey));
                    };
                }

                ui.add_space(avatar_margin_left);

                ui.add_space(3.0);

                if !render_data.hide_nameline {
                    GossipUi::render_person_name_line(app, ui, &note.author, false);

                    ui.horizontal_wrapped(|ui| {
                        match note.event.replies_to() {
                            Some(EventReference::Id { id: irt, .. }) => {
                                let muted = if let Some(note_ref) =
                                    app.notecache.try_update_and_get(&irt)
                                {
                                    if let Ok(note_data) = note_ref.try_borrow() {
                                        note_data.muted()
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                };

                                ui.add_space(8.0);
                                ui.style_mut().override_text_style = Some(TextStyle::Small);
                                let idhex: IdHex = irt.into();
                                if muted {
                                    let name = "▲ (parent is muted)".to_string();
                                    let _ = ui.link(&name);
                                } else {
                                    let name =
                                        format!("▲ #{}", gossip_lib::names::hex_id_short(&idhex));
                                    if ui.link(&name).clicked() {
                                        app.set_page(
                                            ui.ctx(),
                                            Page::Feed(FeedKind::Thread {
                                                id: irt,
                                                referenced_by: note.event.id,
                                                author: Some(note.event.pubkey),
                                            }),
                                        );
                                    };
                                }
                                ui.reset_style();
                            }
                            Some(EventReference::Addr(ea)) => {
                                // Link to this parent only if we can get that event
                                if let Ok(Some(e)) = GLOBALS
                                    .db()
                                    .get_replaceable_event(ea.kind, ea.author, &ea.d)
                                {
                                    let muted = if let Some(note_ref) =
                                        app.notecache.try_update_and_get(&e.id)
                                    {
                                        if let Ok(note_data) = note_ref.try_borrow() {
                                            note_data.muted()
                                        } else {
                                            false
                                        }
                                    } else {
                                        false
                                    };

                                    ui.add_space(8.0);
                                    ui.style_mut().override_text_style = Some(TextStyle::Small);
                                    let idhex: IdHex = e.id.into();
                                    if muted {
                                        let name = "▲ (parent is muted)".to_string();
                                        let _ = ui.link(&name);
                                    } else {
                                        let name = format!(
                                            "▲ #{}",
                                            gossip_lib::names::hex_id_short(&idhex)
                                        );
                                        if ui.link(&name).clicked() {
                                            app.set_page(
                                                ui.ctx(),
                                                Page::Feed(FeedKind::Thread {
                                                    id: e.id,
                                                    referenced_by: note.event.id,
                                                    author: Some(note.event.pubkey),
                                                }),
                                            );
                                        };
                                    }
                                    ui.reset_style();
                                }
                            }
                            None => (),
                        }

                        ui.add_space(8.0);

                        if note.event.pow() > 0 {
                            let color = app.theme.notice_marker_text_color();
                            ui.label(
                                RichText::new(format!("POW={}", note.event.pow()))
                                    .color(color)
                                    .text_style(TextStyle::Small),
                            );
                        }

                        match &note.delegation {
                            EventDelegation::InvalidDelegation(why) => {
                                let color = app.theme.warning_marker_text_color();
                                ui.add(Label::new(
                                    RichText::new("INVALID DELEGATION")
                                        .color(color)
                                        .text_style(TextStyle::Small),
                                ))
                                .on_hover_text(why);
                            }
                            EventDelegation::DelegatedBy(_) => {
                                let color = app.theme.notice_marker_text_color();
                                ui.label(
                                    RichText::new("DELEGATED")
                                        .color(color)
                                        .text_style(TextStyle::Small),
                                );
                            }
                            _ => {}
                        }

                        if !note.deletions.is_empty() {
                            let color = app.theme.warning_marker_text_color();
                            ui.label(
                                RichText::new("DELETED")
                                    .color(color)
                                    .text_style(TextStyle::Small),
                            );
                        }

                        if note.repost.is_some() {
                            let color = app.theme.notice_marker_text_color();
                            ui.label(
                                RichText::new("REPOSTED")
                                    .color(color)
                                    .text_style(TextStyle::Small),
                            );
                        }

                        if let Page::Feed(FeedKind::DmChat(_)) = app.page {
                            // in dm_channel view, highlight the encryption standard
                            // this will be done later in this function
                        } else {
                            // in the other feeds, show a text that describes the message type
                            // we will not show the content itself in other feeds
                            if note.event.kind.is_direct_message_related() {
                                let color = app.theme.notice_marker_text_color();
                                if note.encryption == EncryptionType::Giftwrap {
                                    ui.label(
                                        RichText::new("PRIVATE CHAT (GIFT WRAPPED)")
                                            .color(color)
                                            .text_style(TextStyle::Small),
                                    );
                                } else {
                                    ui.label(
                                        RichText::new("PRIVATE CHAT")
                                            .color(color)
                                            .text_style(TextStyle::Small),
                                    );
                                }
                            }
                        }
                    });
                }

                let mut next_page = None;
                ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                    // show "more actions" menu
                    note_actions(app, ui, &note, render_data);

                    ui.add_space(4.0);

                    let is_thread_view: bool = {
                        let feed_kind = GLOBALS.feed.get_feed_kind();
                        matches!(feed_kind, FeedKind::Thread { .. })
                    };

                    if is_thread_view && note.event.replies_to().is_some() {
                        if collapsed {
                            let color = app.theme.warning_marker_text_color();
                            if widgets::Button::secondary(
                                &app.theme,
                                RichText::new("▼").size(13.0).color(color),
                            )
                            .small(true)
                            .show(ui)
                            .on_hover_text("Expand thread")
                            .clicked()
                            {
                                app.collapsed.retain(|&id| id != note.event.id);
                            }
                        } else {
                            if widgets::Button::secondary(&app.theme, RichText::new("△").size(13.0))
                                .small(true)
                                .show(ui)
                                .on_hover_text("Collapse thread")
                                .clicked()
                            {
                                app.collapsed.push(note.event.id);
                            }
                        }
                        ui.add_space(4.0);
                    }

                    if !render_data.is_main_event && render_data.can_load_thread {
                        if widgets::Button::secondary(&app.theme, RichText::new("◉").size(13.0))
                            .small(true)
                            .show(ui)
                            .on_hover_text("View Thread")
                            .clicked()
                        {
                            if note.event.kind.is_direct_message_related() {
                                if let Some(channel) = DmChannel::from_event(&note.event, None) {
                                    next_page = Some(Page::Feed(FeedKind::DmChat(channel)));
                                } else {
                                    GLOBALS.status_queue.write().write(
                                        "Could not determine DM channel for that note.".to_string(),
                                    );
                                }
                            } else {
                                next_page = Some(Page::Feed(FeedKind::Thread {
                                    id: note.event.id,
                                    referenced_by: note.event.id,
                                    author: Some(note.event.pubkey),
                                }));
                            }
                        }
                    }

                    ui.add_space(4.0);

                    if seen_location == Align2::RIGHT_TOP {
                        draw_seen_on(app, ui, &note);
                    }
                });

                if let Some(next_page) = next_page {
                    app.set_page(ui.ctx(), next_page);
                }
            });

            ui.add_space(2.0);

            // MAIN CONTENT
            if !collapsed {
                render_content(
                    app,
                    ui,
                    note_ref.clone(),
                    !note.deletions.is_empty(),
                    content_inner_margin,
                    content_outer_margin,
                );

                // annotations
                for (created_at, content) in note.annotations.iter() {
                    ui.label(
                        RichText::new(crate::date_ago::date_ago(*created_at))
                            .italics()
                            .weak(),
                    )
                    .on_hover_ui(|ui| {
                        if let Ok(stamp) = time::OffsetDateTime::from_unix_timestamp(created_at.0) {
                            if let Ok(formatted) =
                                stamp.format(&time::format_description::well_known::Rfc2822)
                            {
                                ui.label(formatted);
                            }
                        }
                    });

                    ui.label(format!("EDIT: {}", content));
                }

                // deleted?
                for delete_reason in &note.deletions {
                    Frame::none()
                        .inner_margin(Margin {
                            left: footer_margin_left,
                            bottom: 0.0,
                            right: 0.0,
                            top: 8.0,
                        })
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(format!("Deletion Reason: {}", delete_reason))
                                    .italics(),
                            );
                        });
                }

                // proxied?
                if let Some((proxy, id)) = note.event.proxy() {
                    Frame::none()
                        .inner_margin(Margin {
                            left: footer_margin_left,
                            bottom: 0.0,
                            right: 0.0,
                            top: 8.0,
                        })
                        .show(ui, |ui| {
                            let color = app.theme.accent_complementary_color();
                            ui.horizontal_wrapped(|ui| {
                                ui.add(Label::new(
                                    RichText::new(format!("proxied from {}: ", proxy)).color(color),
                                ));
                                crate::ui::widgets::break_anywhere_hyperlink_to(ui, &id, &id);
                            });
                        });
                }

                // Footer
                if !hide_footer {
                    let ft_inner_margin = Margin {
                        left: footer_margin_left,
                        bottom: 0.0,
                        right: 0.0,
                        top: 8.0,
                    };
                    let footer_response = Frame::none()
                        .inner_margin(ft_inner_margin)
                        .outer_margin(Margin {
                            left: 0.0,
                            bottom: 0.0,
                            right: 0.0,
                            top: 0.0,
                        })
                        .show(ui, |ui| {
                            ui.set_max_width(header_response.response.rect.width());
                            ui.horizontal(|ui| {
                                let can_sign = GLOBALS.identity.is_unlocked();

                                // Button to reply
                                if note.event.kind.is_direct_message_related() {
                                    if widgets::clickable_label(
                                        ui,
                                        can_sign,
                                        RichText::new("⏎").size(18.0),
                                    )
                                    .on_hover_text("Reply")
                                    .clicked()
                                    {
                                        if let Some(channel) =
                                            DmChannel::from_event(&note.event, None)
                                        {
                                            app.draft_needs_focus = true;
                                            app.show_post_area = true;

                                            app.set_page(
                                                ui.ctx(),
                                                Page::Feed(FeedKind::DmChat(channel.clone())),
                                            );
                                        }
                                        // FIXME: else error
                                    }
                                } else {
                                    if widgets::clickable_label(
                                        ui,
                                        can_sign,
                                        RichText::new("💬").size(18.0),
                                    )
                                    .on_hover_text("Reply")
                                    .clicked()
                                    {
                                        app.draft_needs_focus = true;
                                        app.show_post_area = true;

                                        app.draft_data.replying_to = Some(note.event.id);
                                    }
                                };

                                ui.add_space(24.0);

                                if note.event.kind != EventKind::EncryptedDirectMessage
                                    && note.event.kind != EventKind::DmChat
                                {
                                    // Button to Repost
                                    if widgets::clickable_label(
                                        ui,
                                        can_sign,
                                        RichText::new("↻").size(18.0),
                                    )
                                    .on_hover_text("Repost")
                                    .clicked()
                                    {
                                        app.show_post_area = true;
                                        app.draft_data.repost = Some(note.event.id);
                                        app.draft_data.replying_to = None;
                                    }

                                    ui.add_space(24.0);

                                    // Button to quote note
                                    if widgets::clickable_label(
                                        ui,
                                        can_sign,
                                        RichText::new("“…”").size(18.0),
                                    )
                                    .on_hover_text("Quote")
                                    .clicked()
                                    {
                                        let relays: Vec<UncheckedUrl> = note
                                            .seen_on
                                            .iter()
                                            .map(|(url, _)| url.to_unchecked_url())
                                            .take(3)
                                            .collect();

                                        if !app.draft_data.draft.ends_with(' ')
                                            && !app.draft_data.draft.is_empty()
                                        {
                                            app.draft_data.draft.push(' ');
                                        }
                                        let nostr_url: NostrUrl =
                                            if note.event.kind.is_replaceable() {
                                                let param = match note.event.parameter() {
                                                    Some(p) => p,
                                                    None => "".to_owned(),
                                                };
                                                let naddr = NAddr {
                                                    d: param,
                                                    relays: relays.clone(),
                                                    kind: note.event.kind,
                                                    author: note.event.pubkey,
                                                };
                                                naddr.into()
                                            } else {
                                                let nevent = NEvent {
                                                    id: note.event.id,
                                                    relays: relays.clone(),
                                                    author: None,
                                                    kind: None,
                                                };
                                                nevent.into()
                                            };
                                        app.draft_data.draft.push_str(&format!("{}", nostr_url));
                                        app.draft_data.repost = None;
                                        app.draft_data.replying_to = None;
                                        app.show_post_area = true;
                                        app.draft_needs_focus = true;
                                    }

                                    ui.add_space(24.0);
                                }

                                if read_setting!(enable_zap_receipts) && !note.muted() {
                                    // To zap, the user must have a lnurl, and the event must have been
                                    // seen on some relays
                                    let mut zap_lnurl: Option<String> = None;
                                    if let Some(ref metadata) = note.author.metadata() {
                                        if let Some(lnurl) = metadata.lnurl() {
                                            zap_lnurl = Some(lnurl);
                                        }
                                    }

                                    if let Some(lnurl) = zap_lnurl {
                                        if widgets::clickable_label(
                                            ui,
                                            can_sign,
                                            RichText::new("⚡").size(18.0),
                                        )
                                        .on_hover_text("ZAP")
                                        .clicked()
                                        {
                                            if GLOBALS.identity.is_unlocked() {
                                                let _ = GLOBALS.to_overlord.send(
                                                    ToOverlordMessage::ZapStart(
                                                        note.event.id,
                                                        note.event.pubkey,
                                                        UncheckedUrl(lnurl),
                                                    ),
                                                );
                                            } else {
                                                GLOBALS
                                                    .status_queue
                                                    .write()
                                                    .write("Your key is not setup.".to_string());
                                            }
                                        }
                                    } else {
                                        widgets::clickable_label(
                                            ui,
                                            false,
                                            RichText::new("⚡").size(18.0),
                                        )
                                        .on_disabled_hover_text("Note is not zappable (no lnurl)");
                                    }

                                    // Show the zap total
                                    ui.add_enabled(
                                        can_sign,
                                        Label::new(format!("{}", note.zaptotal.0 / 1000)),
                                    )
                                    .on_hover_cursor(egui::CursorIcon::Default);
                                }

                                ui.add_space(24.0);

                                // Buttons to react and reaction counts
                                if read_setting!(reactions) && !note.muted() {
                                    if let Some(reaction) = note.our_reaction {
                                        ui.label(RichText::new(reaction).size(16.0));
                                    } else if can_sign {
                                        let bar_id = ui.id().with("emoji_picker");
                                        let mut bar_state =
                                            egui::menu::BarState::load(ui.ctx(), bar_id);

                                        let button_response = ui
                                            .add(
                                                Label::new(RichText::new('♡').size(20.0))
                                                    .selectable(false)
                                                    .sense(Sense::click()),
                                            )
                                            .on_hover_cursor(egui::CursorIcon::PointingHand);

                                        bar_state.bar_menu(&button_response, |ui| {
                                            if let Some(emoji) =
                                                crate::ui::emojis::emoji_picker(ui)
                                            {
                                                let _ = GLOBALS.to_overlord.send(
                                                    ToOverlordMessage::React(
                                                        note.event.id,
                                                        note.event.pubkey,
                                                        emoji,
                                                    ),
                                                );
                                            }
                                        });
                                        bar_state.store(ui.ctx(), bar_id);
                                    } else {
                                        ui.label(RichText::new('♡').size(20.0));
                                    }

                                    let hover_ui = |ui: &mut Ui| {
                                        ui.horizontal_wrapped(|ui| {
                                            let mut col = 0;
                                            for (ch, count) in note.reactions.iter() {
                                                if *ch != '+' {
                                                    egui::Frame::none()
                                                        .inner_margin(egui::Margin::from(
                                                            ui.spacing().item_spacing,
                                                        ))
                                                        .show(ui, |ui| {
                                                            ui.add_enabled(
                                                                can_sign,
                                                                egui::Label::new(
                                                                    RichText::new(format!(
                                                                        "{} {}",
                                                                        ch, count
                                                                    ))
                                                                    .weak(),
                                                                ),
                                                            )
                                                            .on_hover_cursor(
                                                                egui::CursorIcon::Default,
                                                            );
                                                        });
                                                }

                                                col = col.add(1);
                                                if col > 5 {
                                                    ui.end_row();
                                                    col = 0;
                                                }
                                            }
                                        });
                                    };
                                    let like_count = note
                                        .reactions
                                        .iter()
                                        .find_map(
                                            |(ch, count)| {
                                                if *ch == '+' {
                                                    Some(*count)
                                                } else {
                                                    None
                                                }
                                            },
                                        )
                                        .unwrap_or_default();
                                    let reaction_count: usize = note
                                        .reactions
                                        .iter()
                                        .filter_map(|(c, s)| if *c == '+' { None } else { Some(s) })
                                        .sum();

                                    ui.add(
                                        Label::new(format!("{}+{}", like_count, reaction_count))
                                            .sense(Sense::hover()),
                                    )
                                    .on_hover_ui(hover_ui)
                                    .on_disabled_hover_ui(hover_ui);
                                }
                            });

                            // Below the note zap area
                            if let Some(zapnoteid) = app.note_being_zapped {
                                if zapnoteid == note.event.id {
                                    ui.horizontal_wrapped(|ui| {
                                        app.render_zap_area(ui);
                                    });
                                    if ui
                                        .add(CopyButton::new())
                                        .on_hover_text("Copy Invoice")
                                        .clicked()
                                    {
                                        ui.output_mut(|o| {
                                            if let ZapState::ReadyToPay(_id, ref invoice) =
                                                app.zap_state
                                            {
                                                o.copied_text = invoice.to_owned();
                                            }
                                        });
                                    }
                                }
                            }
                        });

                    // alternate seen_on location and encryption indicator
                    if seen_location == Align2::RIGHT_BOTTOM {
                        let bottom_right = egui::pos2(
                            header_response.response.rect.right() - content_margin_right,
                            footer_response.response.rect.bottom(),
                        );
                        let top_left = bottom_right
                            + vec2(
                                -150.0,
                                -footer_response.response.rect.height()
                                    + ft_inner_margin.top
                                    + ui.spacing().item_spacing.y,
                            );
                        let ui_rect = egui::Rect::from_points(&[top_left, bottom_right]);
                        ui.allocate_ui_at_rect(ui_rect, |ui| {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::default()),
                                |ui| {
                                    let response = draw_seen_on(app, ui, &note);
                                    if let Some(indicator) = encryption_indicator {
                                        let pos = response.rect.left_center() + vec2(-5.0, 0.0);
                                        const RADIUS: f32 = 7.0;
                                        ui.interact(
                                            egui::Rect::from_min_size(
                                                pos + vec2(-RADIUS * 2.0, -RADIUS),
                                                vec2(RADIUS * 2.0, RADIUS * 2.0),
                                            ),
                                            ui.next_auto_id().with("enc_ind"),
                                            egui::Sense::hover(),
                                        )
                                        .on_hover_ui(indicator.tooltip_ui);

                                        ui.painter().circle_filled(
                                            pos + vec2(-RADIUS, 0.0),
                                            RADIUS,
                                            indicator.color,
                                        );
                                    }
                                },
                            );
                        });
                    }
                }
            }
        });
    }
}

fn thin_separator(ui: &mut Ui, stroke: Stroke) {
    let style = ui.style_mut();
    style.visuals.widgets.noninteractive.bg_stroke = stroke;
    ui.add(Separator::default().spacing(0.0));
    ui.reset_style();
}

fn render_subject(ui: &mut Ui, event: &Event) {
    let subject = if let Some(subject) = event.subject() {
        subject
    } else if let Some(title) = event.title() {
        title
    } else {
        return;
    };
    ui.style_mut().spacing.item_spacing.x = 0.0;
    ui.style_mut().spacing.item_spacing.y = 10.0;
    ui.label(RichText::new(subject).text_style(TextStyle::Name("subject".into())));
    ui.end_row();
    ui.reset_style();
}

fn render_content(
    app: &mut GossipUi,
    ui: &mut Ui,
    note_ref: Rc<RefCell<NoteData>>,
    as_deleted: bool,
    content_inner_margin: Margin,
    content_outer_margin: Margin,
) {
    if let Ok(note) = note_ref.try_borrow() {
        let event = &note.event;

        let bottom_of_avatar = ui.cursor().top();

        Frame::none()
            .inner_margin(content_inner_margin)
            .outer_margin(content_outer_margin)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    let feed_kind = GLOBALS.feed.get_feed_kind();
                    if note.muted() && !matches!(feed_kind, FeedKind::Person(_)) {
                        let color = app.theme.notice_marker_text_color();
                        ui.label(
                            RichText::new("MUTED")
                                .color(color)
                                .text_style(TextStyle::Small),
                        );
                    } else if event.content_warning().is_some()
                        && !app.approved.contains(&event.id)
                        && read_setting!(approve_content_warning)
                    {
                        let text = match event.content_warning().unwrap() {
                            Some(cw) => format!("Content-Warning: {}", cw),
                            None => "Content-Warning".to_string(),
                        };
                        ui.label(RichText::new(text).monospace().italics());
                        if ui.button("Show Post").clicked() {
                            app.approved.insert(event.id);
                            app.height.remove(&event.id); // will need to be recalculated.
                        }
                    } else if note.repost == Some(RepostType::Kind6Embedded) {
                        if note.embedded_event.is_some() {
                            let inner_note_data =
                                NoteData::new(note.embedded_event.clone().unwrap());
                            let inner_ref = Rc::new(RefCell::new(inner_note_data));
                            render_repost(
                                app,
                                ui,
                                &note.repost,
                                inner_ref,
                                content_inner_margin,
                                bottom_of_avatar,
                            );
                        }
                    } else if note.repost == Some(RepostType::GenericRepost) {
                        if note.embedded_event.is_some() {
                            let inner_note_data =
                                NoteData::new(note.embedded_event.clone().unwrap());
                            let inner_ref = Rc::new(RefCell::new(inner_note_data));
                            render_repost(
                                app,
                                ui,
                                &note.repost,
                                inner_ref,
                                content_inner_margin,
                                bottom_of_avatar,
                            );
                        } else {
                            match event.mentions().first() {
                                Some(EventReference::Id { id, .. }) => {
                                    if let Some(note_data) = app.notecache.try_update_and_get(id) {
                                        // TODO block additional repost recursion
                                        render_repost(
                                            app,
                                            ui,
                                            &note.repost,
                                            note_data,
                                            content_inner_margin,
                                            bottom_of_avatar,
                                        );
                                    } else {
                                        let color = app.theme.notice_marker_text_color();
                                        ui.label(
                                            RichText::new("GENERIC REPOST EVENT NOT FOUND.")
                                                .color(color)
                                                .text_style(TextStyle::Small),
                                        );
                                    }
                                }
                                Some(EventReference::Addr(_ea)) => {
                                    //FIXME:  GET THE ID here?
                                    let color = app.theme.notice_marker_text_color();
                                    ui.label(
                                        RichText::new("GENERIC REPOST EVENT NOT YET SUPPORTED")
                                            .color(color)
                                            .text_style(TextStyle::Small),
                                    );
                                }
                                _ => {
                                    let color = app.theme.notice_marker_text_color();
                                    ui.label(
                                        RichText::new("BROKEN GENERIC REPOST EVENT")
                                            .color(color)
                                            .text_style(TextStyle::Small),
                                    );
                                }
                            }
                        }
                    } else {
                        // Possible subject line
                        render_subject(ui, event);

                        content::render_content(
                            app,
                            ui,
                            note_ref.clone(),
                            as_deleted,
                            content_inner_margin,
                            bottom_of_avatar,
                        );
                    }
                });
            });
    }
}

fn render_repost(
    app: &mut GossipUi,
    ui: &mut Ui,
    parent_repost: &Option<RepostType>,
    repost_ref: Rc<RefCell<NoteData>>,
    content_inner_margin: Margin,
    bottom_of_avatar: f32,
) {
    if let Ok(repost_data) = repost_ref.try_borrow() {
        let render_data = NoteRenderData {
            // the full note height differs from the reposted height anyways
            height: 0.0,
            is_comment_mention: *parent_repost == Some(RepostType::CommentMention),
            is_new: false,
            is_main_event: false,
            can_load_thread: true, // TODO refine this choice
            is_thread: false,
            thread_position: 0,
            hide_footer: false,
            hide_nameline: false,
        };

        let row_height = ui.cursor().height();

        let push_top = {
            let diff = bottom_of_avatar - ui.cursor().top();
            if diff > 0.0 {
                diff
            } else {
                0.0
            }
        };

        // insert a newline if the current line has text
        if ui.cursor().min.x > ui.max_rect().min.x {
            ui.end_row();
        }

        ui.vertical(|ui| {
            Frame::none()
                .inner_margin(app.theme.repost_inner_margin(&render_data))
                .outer_margin({
                    let mut margin = app.theme.repost_outer_margin(&render_data);
                    margin.left -= content_inner_margin.left;
                    margin.top += push_top;
                    margin
                })
                .rounding(app.theme.repost_rounding(&render_data))
                .shadow(app.theme.repost_shadow(&render_data))
                .fill(app.theme.repost_fill(&render_data))
                .stroke(app.theme.repost_stroke(&render_data))
                .show(ui, |ui| {
                    ui.add_space(app.theme.repost_space_above_separator_before(&render_data));
                    thin_separator(ui, app.theme.repost_separator_before_stroke(&render_data));
                    ui.add_space(app.theme.repost_space_below_separator_before(&render_data));
                    ui.horizontal_wrapped(|ui| {
                        let top = ui.next_widget_position();

                        // FIXME: don't recurse forever
                        render_note_inner(app, ui, repost_ref.clone(), &render_data, parent_repost);

                        let bottom = ui.next_widget_position();

                        // Record if the rendered repost was visible
                        {
                            let screen_rect = ui.ctx().input(|i| i.screen_rect); // Rect
                            let offscreen = bottom.y < 0.0 || top.y > screen_rect.max.y;
                            if !offscreen {
                                // Record that this note was visibly rendered
                                app.next_visible_note_ids.push(repost_data.event.id);
                            }
                        }
                    });
                    ui.add_space(app.theme.repost_space_above_separator_after(&render_data));
                    thin_separator(ui, app.theme.repost_separator_after_stroke(&render_data));
                    ui.add_space(app.theme.repost_space_below_separator_after(&render_data));
                });
        });

        ui.end_row();
        ui.set_row_height(row_height);
    }
}

fn note_actions(
    app: &mut GossipUi,
    ui: &mut Ui,
    note: &std::cell::Ref<NoteData>,
    _render_data: &NoteRenderData,
) {
    let relays: Vec<UncheckedUrl> = note
        .seen_on
        .iter()
        .map(|(url, _)| url.to_unchecked_url())
        .take(3)
        .collect();

    let text = egui::RichText::new("=").size(13.0);
    let response = widgets::Button::primary(&app.theme, text)
        .small(true)
        .show(ui);
    let menu = widgets::MoreMenu::simple(ui.auto_id_with(note.event.id))
        .with_min_size(vec2(100.0, 0.0))
        .with_max_size(vec2(140.0, ui.ctx().available_rect().height()));
    let mut items: Vec<MoreMenuItem> = Vec::new();

    // ---- Copy Text ----
    {
        let mut copy_items: Vec<MoreMenuItem> = Vec::new();

        copy_items.push(MoreMenuItem::Button(MoreMenuButton::new(
            "to Clipboard",
            Box::new(|ui, _| {
                ui.output_mut(|o| o.copied_text = note.event.content.clone());
            }),
        )));

        copy_items.push(MoreMenuItem::Button(MoreMenuButton::new(
            "with QR Code",
            Box::new(|ui, app| {
                app.render_qr = Some(note.event.id);
                if let Some((_th, x, y)) = app.generate_qr(
                    ui,
                    note.event.id.as_hex_string().as_str(),
                    note.event.content.as_str(),
                ) {
                    app.modal = Some(Rc::new(ModalEntry {
                        min_size: vec2(300.0, 200.0),
                        max_size: vec2(x * 1.2, y * 1.2).min(ui.ctx().screen_rect().size()),
                        content: Rc::new(|ui, app| {
                            ui.vertical_centered(|ui| {
                                if let Some(id) = app.render_qr {
                                    ui.add_space(10.0);
                                    ui.heading("Copy note content");
                                    ui.add_space(10.0);
                                    app.show_qr(ui, id.as_hex_string().as_str());
                                    ui.add_space(10.0);
                                }
                            });
                        }),
                        on_close: Rc::new(|app| {
                            if let Some(id) = app.render_qr.take() {
                                // delete QR to not keep private note data in memory
                                app.delete_qr(id.as_hex_string().as_str());
                            }
                            app.modal.take();
                        }),
                    }));
                }
            }),
        )));

        items.push(MoreMenuItem::SubMenu(MoreMenuSubMenu::new(
            "Copy text",
            copy_items,
            &menu,
        )));
    }

    // ---- Manage SubMenu ----
    if let Some(our_pubkey) = GLOBALS.identity.public_key() {
        if note.event.pubkey == our_pubkey {
            let mut my_items: Vec<MoreMenuItem> = Vec::new();

            if note.deletions.is_empty() {
                my_items.push(MoreMenuItem::Button(MoreMenuButton::new(
                    "Delete",
                    Box::new(|_, _| {
                        let _ = GLOBALS
                            .to_overlord
                            .send(ToOverlordMessage::DeletePost(note.event.id));
                    }),
                )));
            }

            // Annotate Button
            my_items.push(MoreMenuItem::Button(MoreMenuButton::new(
                "Annotate",
                Box::new(|_ui, app| {
                    app.draft_needs_focus = true;
                    app.show_post_area = true;

                    app.draft_data.is_annotate = true;
                    app.draft_data.replying_to = Some(note.event.id)
                }),
            )));

            // Chance to post our note again to relays it missed
            if let Ok(broadcast_relays) = relay::relays_to_post_to(&note.event) {
                if !broadcast_relays.is_empty() {
                    my_items.push(MoreMenuItem::Button(MoreMenuButton::new(
                        format!("Rebroadcast ({})", broadcast_relays.len()),
                        Box::new(|_, _| {
                            let _ = GLOBALS
                                .to_overlord
                                .send(ToOverlordMessage::PostAgain(note.event.clone()));
                        }),
                    )));
                }
            }

            items.push(MoreMenuItem::SubMenu(MoreMenuSubMenu::new(
                "Manage", my_items, &menu,
            )))
        }
    } // Manage SubMenu

    // ---- Bookmark ----
    if note.bookmarked {
        items.push(MoreMenuItem::Button(MoreMenuButton::new(
            "Unbookmark",
            Box::new(|_, _| {
                let er = note.event_reference();
                let _ = GLOBALS.to_overlord.send(ToOverlordMessage::BookmarkRm(er));
            }),
        )));
    } else {
        let mut bm_items: Vec<MoreMenuItem> = Vec::new();
        bm_items.push(MoreMenuItem::Button(MoreMenuButton::new(
            "Public",
            Box::new(|_, _| {
                let er = note.event_reference();
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::BookmarkAdd(er, false));
            }),
        )));
        bm_items.push(MoreMenuItem::Button(MoreMenuButton::new(
            "Private",
            Box::new(|_, _| {
                let er = note.event_reference();
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::BookmarkAdd(er, true));
            }),
        )));
        items.push(MoreMenuItem::SubMenu(MoreMenuSubMenu::new(
            "Bookmark", bm_items, &menu,
        )));
    } // end Bookmark

    // ---- Share ----
    if !note.event.kind.is_direct_message_related() {
        items.push(MoreMenuItem::Button(MoreMenuButton::new(
            "Share via web",
            Box::new(|ui, _| {
                let nevent = NEvent {
                    id: note.event.id,
                    relays: relays.clone(),
                    author: None,
                    kind: None,
                };
                ui.output_mut(|o| {
                    o.copied_text = format!("https://njump.me/{}", nevent.as_bech32_string())
                });
            }),
        )));
    } // end Share

    // ---- Copy ID SubMenu ----
    {
        // put all copy buttons in a submenu
        let mut copy_items: Vec<MoreMenuItem> = Vec::new();

        if note.event.kind.is_replaceable() {
            let param = match note.event.parameter() {
                Some(p) => p,
                None => "".to_owned(),
            };

            copy_items.push(MoreMenuItem::Button(MoreMenuButton::new(
                "as naddr",
                Box::new(|ui, _| {
                    let naddr = NAddr {
                        d: param,
                        relays: relays.clone(),
                        kind: note.event.kind,
                        author: note.event.pubkey,
                    };
                    let nostr_url: NostrUrl = naddr.into();
                    ui.output_mut(|o| o.copied_text = format!("{}", nostr_url));
                }),
            )));
        } else {
            copy_items.push(MoreMenuItem::Button(MoreMenuButton::new(
                "as nevent1",
                Box::new(|ui, _| {
                    let nevent = NEvent {
                        id: note.event.id,
                        relays: relays.clone(),
                        author: None,
                        kind: None,
                    };
                    let nostr_url: NostrUrl = nevent.into();
                    ui.output_mut(|o| o.copied_text = format!("{}", nostr_url));
                }),
            )));
        }
        copy_items.push(MoreMenuItem::Button(MoreMenuButton::new(
            "as note1",
            Box::new(|ui, _| {
                let nostr_url: NostrUrl = note.event.id.into();
                ui.output_mut(|o| o.copied_text = format!("{}", nostr_url));
            }),
        )));
        copy_items.push(MoreMenuItem::Button(MoreMenuButton::new(
            "as hex",
            Box::new(|ui, _| {
                ui.output_mut(|o| o.copied_text = note.event.id.as_hex_string());
            }),
        )));

        items.push(MoreMenuItem::SubMenu(MoreMenuSubMenu::new(
            "Copy event ID",
            copy_items,
            &menu,
        )));
    } // end Copy ID SubMenu

    // ---- Inspect SubMenu ----
    {
        let mut insp_items: Vec<MoreMenuItem> = Vec::new();

        // Button to show raw JSON
        insp_items.push(MoreMenuItem::Button(MoreMenuButton::new(
            "Show JSON",
            Box::new(|ui, app| {
                let json = serde_json::to_string_pretty(&note.event).unwrap_or_default();
                app.render_raw = Some((note.event.id, json));
                app.modal = Some(Rc::new(ModalEntry {
                    min_size: vec2(300.0, 200.0),
                    max_size: ui.ctx().screen_rect().size() * 0.8,
                    content: Rc::new(|ui, app| {
                        ui.vertical(|ui| {
                            if let Some((id, json)) = &app.render_raw {
                                ui.heading(id.as_bech32_string());
                                ui.add_space(20.0);
                                app.vert_scroll_area()
                                    .show(ui, |ui| {
                                        if let Ok(obj) = serde_json::from_str::<serde_json::Value>(json) {
                                            let mut writer = Vec::new();
                                            let formatter =
                                                serde_json::ser::PrettyFormatter::with_indent(b"  ");
                                            let mut ser = serde_json::Serializer::with_formatter(
                                                &mut writer,
                                                formatter,
                                            );

                                            if obj.serialize(&mut ser).is_ok() {
                                                if let Ok(str) = String::from_utf8(writer) {
                                                    egui_extras::syntax_highlighting::code_view_ui(
                                                        ui,
                                                        &egui_extras::syntax_highlighting::CodeTheme::from_style(
                                                            ui.style(),
                                                        ),
                                                        &str,
                                                        "json",
                                                    );
                                                }
                                            }
                                        }
                                    });
                            }
                        });
                    }),
                    on_close: Rc::new(|app| {
                        app.render_raw.take();
                        app.modal.take();
                    }),
                }));
            }),
        )));

        insp_items.push(MoreMenuItem::Button(MoreMenuButton::new(
            "Copy JSON",
            Box::new(|ui, _| {
                ui.output_mut(|o| {
                    o.copied_text = serde_json::to_string_pretty(&note.event).unwrap()
                });
            }),
        )));

        // Button to render QR code
        insp_items.push(MoreMenuItem::Button(MoreMenuButton::new(
            "QR Code Export",
            Box::new(|ui, app| {
                app.render_qr = Some(note.event.id);
                if let Some((_th, x, y)) = app.generate_qr(
                    ui,
                    note.event.id.as_hex_string().as_str(),
                    serde_json::to_string_pretty(&note.event).unwrap().as_str(),
                ) {
                    app.modal = Some(Rc::new(ModalEntry {
                        min_size: vec2(300.0, 200.0),
                        max_size: vec2(x * 1.2, y * 1.2).min(ui.ctx().screen_rect().size()),
                        content: Rc::new(|ui, app| {
                            ui.vertical_centered(|ui| {
                                if let Some(id) = app.render_qr {
                                    ui.add_space(10.0);
                                    ui.heading("Copy JSON");
                                    ui.add_space(10.0);
                                    app.show_qr(ui, id.as_hex_string().as_str());
                                    ui.add_space(10.0);
                                }
                            });
                        }),
                        on_close: Rc::new(|app| {
                            if let Some(id) = app.render_qr.take() {
                                // delete QR to not keep private note data in memory
                                app.delete_qr(id.as_hex_string().as_str());
                            }
                            app.modal.take();
                        }),
                    }));
                }
            }),
        )));

        insp_items.push(MoreMenuItem::Button(MoreMenuButton::new(
            "Rerender",
            Box::new(|_, app| {
                app.notecache.invalidate_note(&note.event.id);
            }),
        )));

        insp_items.push(MoreMenuItem::Button(MoreMenuButton::new(
            "Dismiss",
            Box::new(|_, _| {
                GLOBALS.dismissed.blocking_write().push(note.event.id);
                GLOBALS.feed.sync_recompute();
            }),
        )));

        items.push(MoreMenuItem::SubMenu(MoreMenuSubMenu::new(
            "Inspect", insp_items, &menu,
        )));
    } // end Inspect SubMenu

    menu.show_entries(ui, app, response, items);
}

fn draw_seen_on(app: &mut GossipUi, ui: &mut Ui, note: &std::cell::Ref<NoteData>) -> Response {
    let mut seen_on_popup_position = ui.next_widget_position();
    seen_on_popup_position.y += 18.0; // drop below the icon itself

    let response = ui.add(Label::new(RichText::new("👁").size(12.0)).sense(Sense::hover()));

    if response.hovered() {
        egui::Area::new(ui.next_auto_id().with("seen_on"))
            .movable(false)
            .interactable(false)
            // .pivot(Align2::RIGHT_TOP) // Fails to work as advertised
            .fixed_pos(seen_on_popup_position)
            // FIXME IN EGUI: constrain is moving the box left for all of these boxes
            // even if they have different IDs and don't need it.
            .constrain(true)
            .show(ui.ctx(), |ui| {
                ui.set_min_width(200.0);
                egui::Frame::popup(&app.theme.get_style()).show(ui, |ui| {
                    if !note.seen_on.is_empty() {
                        for (url, _) in note.seen_on.iter() {
                            ui.label(url.as_str());
                        }
                    } else {
                        ui.label("unknown");
                    }
                });
            });
    }

    let response2 = ui.label(
        RichText::new(crate::date_ago::date_ago(note.event.created_at))
            .italics()
            .weak(),
    );
    let response2 = response2.on_hover_ui(|ui| {
        if let Ok(stamp) = time::OffsetDateTime::from_unix_timestamp(note.event.created_at.0) {
            if let Ok(formatted) = stamp.format(&time::format_description::well_known::Rfc2822) {
                ui.label(formatted);
            }
        }
    });

    response | response2
}
