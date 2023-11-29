mod content;

pub use super::Notes;
use std::cell::RefCell;
use std::rc::Rc;

use super::notedata::{NoteData, RepostType};

use super::FeedNoteParams;
use crate::ui::widgets::{self, AvatarSize, CopyButton};
use crate::ui::{GossipUi, Page};
use crate::{AVATAR_SIZE_F32, AVATAR_SIZE_REPOST_F32};
use eframe::egui::{self, Margin};
use egui::{
    Align, Context, Frame, Label, Layout, RichText, Sense, Separator, Stroke, TextStyle, Ui,
};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::DmChannel;
use gossip_lib::FeedKind;
use gossip_lib::{ZapState, GLOBALS};
use nostr_types::{
    Event, EventAddr, EventDelegation, EventKind, EventPointer, EventReference, IdHex, NostrUrl,
    UncheckedUrl,
};

pub struct NoteRenderData {
    /// Height of the post
    /// This is only used in feed_post_inner_indent() and is often just set to 0.0, but should
    /// be taken from app.height if we can get that data.
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

    let mut replies = Vec::new();

    if let Some(note_ref) = app.notes.try_update_and_get(&id) {
        // FIXME respect app.settings.show_long_form on reposts
        // FIXME drop the cached notes on recompute

        if let Ok(note_data) = note_ref.try_borrow() {
            let skip = ((note_data.muted() && app.settings.hide_mutes_entirely)
                && !matches!(app.page, Page::Feed(FeedKind::DmChat(_)))
                && !matches!(app.page, Page::Feed(FeedKind::Person(_))))
                || (note_data.deletion.is_some() && !app.settings.show_deleted_events);

            if skip {
                return;
            }

            let viewed = match GLOBALS.storage.is_event_viewed(note_data.event.id) {
                Ok(answer) => answer,
                _ => false,
            };

            let is_new = app.settings.highlight_unread_events && !viewed;

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

                            render_note_inner(
                                app,
                                ctx,
                                ui,
                                note_ref.clone(),
                                &render_data,
                                as_reply_to,
                                &None,
                            );
                        });
                    })
            });

            // Store actual rendered height for future reference
            let bottom = ui.next_widget_position();
            app.height.insert(id, bottom.y - top.y);

            // Mark post as viewed if hovered AND we are not scrolling
            if !viewed && inner_response.response.hovered() && app.current_scroll_offset == 0.0 {
                let _ = GLOBALS.storage.mark_event_viewed(id, None);
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
                    .storage
                    .get_replies(&note_data.event)
                    .unwrap_or_default();
            }
        }

        // even if muted, continue rendering thread children
        if threaded && !as_reply_to && !app.collapsed.contains(&id) {
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
}

// FIXME, create some way to limit the arguments here.
fn render_note_inner(
    app: &mut GossipUi,
    ctx: &Context,
    ui: &mut Ui,
    note_ref: Rc<RefCell<NoteData>>,
    render_data: &NoteRenderData,
    hide_footer: bool,
    parent_repost: &Option<RepostType>,
) {
    if let Ok(note) = note_ref.try_borrow() {
        let collapsed = app.collapsed.contains(&note.event.id);

        // Load avatar texture
        let avatar = if note.muted() {
            // no avatars for muted people
            app.placeholder_avatar.clone()
        } else if let Some(avatar) = app.try_get_avatar(ctx, &note.author.pubkey) {
            avatar
        } else {
            app.placeholder_avatar.clone()
        };

        // Determine avatar size
        let avatar_size = if parent_repost.is_none() {
            match note.repost {
                None | Some(RepostType::CommentMention) => AvatarSize::Feed,
                Some(_) => AvatarSize::Mini,
            }
        } else {
            match parent_repost {
                None | Some(RepostType::CommentMention) => AvatarSize::Mini,
                Some(_) => AvatarSize::Feed,
            }
        };

        let inner_margin = app.theme.feed_frame_inner_margin(render_data);

        let avatar_margin_left = if parent_repost.is_none() {
            match note.repost {
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

        let content_pull_top =
            inner_margin.top + ui.style().spacing.item_spacing.y * 4.0 - avatar_size.y();

        let content_margin_left = AVATAR_SIZE_F32 + inner_margin.left;
        let footer_margin_left = content_margin_left;

        let relays = match GLOBALS.storage.get_event_seen_on_relay(note.event.id) {
            Ok(vec) => vec
                .iter()
                .map(|(url, _)| url.to_unchecked_url())
                .take(3)
                .collect(),
            Err(_) => vec![],
        };

        ui.vertical(|ui| {
            // First row

            ui.with_layout(Layout::left_to_right(Align::TOP), |ui| {
                ui.add_space(avatar_margin_left);

                // render avatar
                if widgets::paint_avatar(ui, &note.author, &avatar, avatar_size).clicked() {
                    app.set_page(ctx, Page::Person(note.author.pubkey));
                };

                ui.add_space(avatar_margin_left);

                ui.add_space(3.0);

                GossipUi::render_person_name_line(app, ui, &note.author, false);

                ui.horizontal_wrapped(|ui| {
                    match note.event.replies_to() {
                        Some(EventReference::Id(irt, _, _)) => {
                            ui.add_space(8.0);
                            ui.style_mut().override_text_style = Some(TextStyle::Small);
                            let idhex: IdHex = irt.into();
                            let nam = format!("▲ #{}", gossip_lib::names::hex_id_short(&idhex));
                            if ui.link(&nam).clicked() {
                                app.set_page(
                                    ctx,
                                    Page::Feed(FeedKind::Thread {
                                        id: irt,
                                        referenced_by: note.event.id,
                                        author: Some(note.event.pubkey),
                                    }),
                                );
                            };
                            ui.reset_style();
                        }
                        Some(EventReference::Addr(ea)) => {
                            // Link to this parent only if we can get that event
                            if let Ok(Some(e)) = GLOBALS
                                .storage
                                .get_replaceable_event(ea.kind, ea.author, &ea.d)
                            {
                                ui.add_space(8.0);
                                ui.style_mut().override_text_style = Some(TextStyle::Small);
                                let idhex: IdHex = e.id.into();
                                let nam = format!("▲ #{}", gossip_lib::names::hex_id_short(&idhex));
                                if ui.link(&nam).clicked() {
                                    app.set_page(
                                        ctx,
                                        Page::Feed(FeedKind::Thread {
                                            id: e.id,
                                            referenced_by: note.event.id,
                                            author: Some(note.event.pubkey),
                                        }),
                                    );
                                };
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

                    if note.deletion.is_some() {
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
                        // don't show ENCRYPTED DM or SECURE in the dm channel itself
                    } else {
                        if note.event.kind.is_direct_message_related() {
                            let color = app.theme.notice_marker_text_color();
                            if note.secure {
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

                ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                    ui.menu_button(RichText::new("=").size(13.0), |ui| {
                        if !render_data.is_main_event {
                            if note.event.kind.is_direct_message_related() {
                                if ui.button("View DM Channel").clicked() {
                                    if let Some(channel) = DmChannel::from_event(&note.event, None)
                                    {
                                        app.set_page(ctx, Page::Feed(FeedKind::DmChat(channel)));
                                    } else {
                                        GLOBALS.status_queue.write().write(
                                            "Could not determine DM channel for that note."
                                                .to_string(),
                                        );
                                    }
                                }
                            } else {
                                if ui.button("View Thread").clicked() {
                                    app.set_page(
                                        ctx,
                                        Page::Feed(FeedKind::Thread {
                                            id: note.event.id,
                                            referenced_by: note.event.id,
                                            author: Some(note.event.pubkey),
                                        }),
                                    );
                                }
                            }
                        }

                        if note.event.kind.is_replaceable() {
                            let param = match note.event.parameter() {
                                Some(p) => p,
                                None => "".to_owned(),
                            };
                            if ui.button("Copy naddr").clicked() {
                                let event_addr = EventAddr {
                                    d: param,
                                    relays: relays.clone(),
                                    kind: note.event.kind,
                                    author: note.event.pubkey,
                                };
                                let nostr_url: NostrUrl = event_addr.into();
                                ui.output_mut(|o| o.copied_text = format!("{}", nostr_url));
                            }
                        } else {
                            if ui.button("Copy nevent").clicked() {
                                let event_pointer = EventPointer {
                                    id: note.event.id,
                                    relays: relays.clone(),
                                    author: None,
                                    kind: None,
                                };
                                let nostr_url: NostrUrl = event_pointer.into();
                                ui.output_mut(|o| o.copied_text = format!("{}", nostr_url));
                            }
                        }
                        if !note.event.kind.is_direct_message_related() {
                            if ui.button("Copy web link").clicked() {
                                let event_pointer = EventPointer {
                                    id: note.event.id,
                                    relays: relays.clone(),
                                    author: None,
                                    kind: None,
                                };
                                ui.output_mut(|o| {
                                    o.copied_text = format!(
                                        "https://njump.me/{}",
                                        event_pointer.as_bech32_string()
                                    )
                                });
                            }
                        }
                        if ui.button("Copy note1 Id").clicked() {
                            let nostr_url: NostrUrl = note.event.id.into();
                            ui.output_mut(|o| o.copied_text = format!("{}", nostr_url));
                        }
                        if ui.button("Copy hex Id").clicked() {
                            ui.output_mut(|o| o.copied_text = note.event.id.as_hex_string());
                        }
                        if ui.button("Copy Raw data").clicked() {
                            ui.output_mut(|o| {
                                o.copied_text = serde_json::to_string_pretty(&note.event).unwrap()
                            });
                        }
                        if ui.button("Dismiss").clicked() {
                            GLOBALS.dismissed.blocking_write().push(note.event.id);
                        }
                        if Some(note.event.pubkey) == app.settings.public_key
                            && note.deletion.is_none()
                        {
                            if ui.button("Delete").clicked() {
                                let _ = GLOBALS
                                    .to_overlord
                                    .send(ToOverlordMessage::DeletePost(note.event.id));
                            }
                        }
                        if ui.button("Rerender").clicked() {
                            app.notes.cache_invalidate_note(&note.event.id);
                        }
                    });
                    ui.add_space(4.0);

                    let is_thread_view: bool = {
                        let feed_kind = GLOBALS.feed.get_feed_kind();
                        matches!(feed_kind, FeedKind::Thread { .. })
                    };

                    if is_thread_view && note.event.replies_to().is_some() {
                        if collapsed {
                            let color = app.theme.warning_marker_text_color();
                            if ui
                                .button(RichText::new("▼").size(13.0).color(color))
                                .on_hover_text("Expand thread")
                                .clicked()
                            {
                                app.collapsed.retain(|&id| id != note.event.id);
                            }
                        } else {
                            if ui
                                .button(RichText::new("△").size(13.0))
                                .on_hover_text("Collapse thread")
                                .clicked()
                            {
                                app.collapsed.push(note.event.id);
                            }
                        }
                        ui.add_space(4.0);
                    }

                    if !render_data.is_main_event {
                        if ui
                            .button(RichText::new("◉").size(13.0))
                            .on_hover_text("View Thread")
                            .clicked()
                        {
                            if note.event.kind.is_direct_message_related() {
                                if let Some(channel) = DmChannel::from_event(&note.event, None) {
                                    app.set_page(ctx, Page::Feed(FeedKind::DmChat(channel)));
                                } else {
                                    GLOBALS.status_queue.write().write(
                                        "Could not determine DM channel for that note.".to_string(),
                                    );
                                }
                            } else {
                                app.set_page(
                                    ctx,
                                    Page::Feed(FeedKind::Thread {
                                        id: note.event.id,
                                        referenced_by: note.event.id,
                                        author: Some(note.event.pubkey),
                                    }),
                                );
                            }
                        }
                    }

                    ui.add_space(4.0);

                    let mut seen_on_popup_position = ui.next_widget_position();
                    seen_on_popup_position.y += 18.0; // drop below the icon itself

                    if ui
                        .add(Label::new(RichText::new("👁").size(12.0)).sense(Sense::hover()))
                        .hovered()
                    {
                        egui::Area::new(ui.next_auto_id())
                            .movable(false)
                            .interactable(false)
                            // .pivot(Align2::RIGHT_TOP) // Fails to work as advertised
                            .fixed_pos(seen_on_popup_position)
                            // FIXME IN EGUI: constrain is moving the box left for all of these boxes
                            // even if they have different IDs and don't need it.
                            .constrain(true)
                            .show(ctx, |ui| {
                                ui.set_min_width(200.0);
                                egui::Frame::popup(&app.theme.get_style()).show(ui, |ui| {
                                    if let Ok(seen_on) =
                                        GLOBALS.storage.get_event_seen_on_relay(note.event.id)
                                    {
                                        for (url, _) in seen_on.iter() {
                                            ui.label(url.as_str());
                                        }
                                    } else {
                                        ui.label("unknown");
                                    }
                                });
                            });
                    }

                    ui.label(
                        RichText::new(crate::date_ago::date_ago(note.event.created_at))
                            .italics()
                            .weak(),
                    )
                    .on_hover_ui(|ui| {
                        if let Ok(stamp) =
                            time::OffsetDateTime::from_unix_timestamp(note.event.created_at.0)
                        {
                            if let Ok(formatted) =
                                stamp.format(&time::format_description::well_known::Rfc2822)
                            {
                                ui.label(formatted);
                            }
                        }
                    });
                });
            });

            ui.add_space(2.0);

            // MAIN CONTENT
            if !collapsed {
                render_content(
                    app,
                    ui,
                    ctx,
                    note_ref.clone(),
                    note.deletion.is_some(),
                    content_margin_left,
                    content_pull_top,
                );

                // deleted?
                if let Some(delete_reason) = &note.deletion {
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
                                crate::ui::widgets::break_anywhere_hyperlink_to(ui, id, id);
                            });
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
                                    .add(CopyButton::new())
                                    .on_hover_text("Copy Contents")
                                    .clicked()
                                {
                                    if app.render_raw == Some(note.event.id) {
                                        ui.output_mut(|o| {
                                            o.copied_text =
                                                serde_json::to_string(&note.event).unwrap()
                                        });
                                    } else if note.event.kind == EventKind::EncryptedDirectMessage {
                                        ui.output_mut(|o| {
                                            if let Ok(m) =
                                                GLOBALS.signer.decrypt_message(&note.event)
                                            {
                                                o.copied_text = m
                                            } else {
                                                o.copied_text = note.event.content.clone()
                                            }
                                        });
                                    } else {
                                        ui.output_mut(|o| {
                                            o.copied_text = note.event.content.clone()
                                        });
                                    }
                                }

                                ui.add_space(24.0);

                                if GLOBALS.signer.is_ready() {
                                    if note.event.kind != EventKind::EncryptedDirectMessage
                                        && note.event.kind != EventKind::DmChat
                                    {
                                        // Button to Repost
                                        if ui
                                            .add(
                                                Label::new(RichText::new("↻").size(18.0))
                                                    .sense(Sense::click()),
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
                                        if ui
                                            .add(
                                                Label::new(RichText::new("“…”").size(18.0))
                                                    .sense(Sense::click()),
                                            )
                                            .on_hover_text("Quote")
                                            .clicked()
                                        {
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
                                                    let event_addr = EventAddr {
                                                        d: param,
                                                        relays: relays.clone(),
                                                        kind: note.event.kind,
                                                        author: note.event.pubkey,
                                                    };
                                                    event_addr.into()
                                                } else {
                                                    let event_pointer = EventPointer {
                                                        id: note.event.id,
                                                        relays: relays.clone(),
                                                        author: None,
                                                        kind: None,
                                                    };
                                                    event_pointer.into()
                                                };
                                            app.draft_data
                                                .draft
                                                .push_str(&format!("{}", nostr_url));
                                            app.draft_data.repost = None;
                                            app.draft_data.replying_to = None;
                                            app.show_post_area = true;
                                            app.draft_needs_focus = true;
                                        }

                                        ui.add_space(24.0);
                                    }

                                    // Button to reply
                                    let reply_icon = if note.event.kind.is_direct_message_related()
                                    {
                                        "⏎"
                                    } else {
                                        "💬"
                                    };

                                    if ui
                                        .add(
                                            Label::new(RichText::new(reply_icon).size(18.0))
                                                .sense(Sense::click()),
                                        )
                                        .on_hover_text("Reply")
                                        .clicked()
                                    {
                                        app.draft_needs_focus = true;
                                        app.show_post_area = true;

                                        if note.event.kind.is_direct_message_related() {
                                            if let Some(channel) =
                                                DmChannel::from_event(&note.event, None)
                                            {
                                                app.set_page(
                                                    ctx,
                                                    Page::Feed(FeedKind::DmChat(channel.clone())),
                                                );
                                                app.draft_needs_focus = true;
                                            }
                                            // FIXME: else error
                                        } else {
                                            app.draft_data.replying_to =
                                                if note.event.kind.is_direct_message_related() {
                                                    None
                                                } else {
                                                    Some(note.event.id)
                                                };
                                        }
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
                                    if app.render_raw != Some(note.event.id) {
                                        app.render_raw = Some(note.event.id);
                                        app.render_qr = None;
                                    } else {
                                        app.render_raw = None;
                                    }
                                }

                                ui.add_space(24.0);

                                // Button to render QR code
                                if ui
                                    .add(
                                        Label::new(RichText::new("⚃").size(16.0))
                                            .sense(Sense::click()),
                                    )
                                    .on_hover_text("QR Code")
                                    .clicked()
                                {
                                    if app.render_qr != Some(note.event.id) {
                                        app.render_qr = Some(note.event.id);
                                        app.render_raw = None;
                                        app.qr_codes.remove("feedqr");
                                    } else {
                                        app.render_qr = None;
                                        app.qr_codes.remove("feedqr");
                                    }
                                }

                                if app.settings.enable_zap_receipts && !note.muted() {
                                    ui.add_space(24.0);

                                    // To zap, the user must have a lnurl, and the event must have been
                                    // seen on some relays
                                    let mut zap_lnurl: Option<String> = None;
                                    if let Some(ref metadata) = note.author.metadata {
                                        if let Some(lnurl) = metadata.lnurl() {
                                            zap_lnurl = Some(lnurl);
                                        }
                                    }

                                    let mut has_seen_on_relays = false;
                                    if let Ok(seen_on) =
                                        GLOBALS.storage.get_event_seen_on_relay(note.event.id)
                                    {
                                        if !seen_on.is_empty() {
                                            has_seen_on_relays = true;
                                        }
                                    }

                                    if let Some(lnurl) = zap_lnurl {
                                        if has_seen_on_relays {
                                            if ui
                                                .add(
                                                    Label::new(RichText::new("⚡").size(18.0))
                                                        .sense(Sense::click()),
                                                )
                                                .on_hover_text("ZAP")
                                                .clicked()
                                            {
                                                if GLOBALS.signer.is_ready() {
                                                    let _ = GLOBALS.to_overlord.send(
                                                        ToOverlordMessage::ZapStart(
                                                            note.event.id,
                                                            note.event.pubkey,
                                                            UncheckedUrl(lnurl),
                                                        ),
                                                    );
                                                } else {
                                                    GLOBALS.status_queue.write().write(
                                                        "Your key is not setup.".to_string(),
                                                    );
                                                }
                                            }
                                        } else {
                                            ui.add(Label::new(
                                                RichText::new("⚡").weak().size(18.0),
                                            ))
                                            .on_hover_text("Note is not zappable (no relays)");
                                        }
                                    } else {
                                        ui.add(Label::new(RichText::new("⚡").weak().size(18.0)))
                                            .on_hover_text("Note is not zappable (no lnurl)");
                                    }

                                    // Show the zap total
                                    ui.add(Label::new(format!("{}", note.zaptotal.0 / 1000)));
                                }

                                ui.add_space(24.0);

                                // Buttons to react and reaction counts
                                if app.settings.reactions && !note.muted() {
                                    let default_reaction_icon = match note.self_already_reacted {
                                        true => "♥",
                                        false => "♡",
                                    };
                                    if ui
                                        .add(
                                            Label::new(
                                                RichText::new(default_reaction_icon).size(20.0),
                                            )
                                            .sense(Sense::click()),
                                        )
                                        .clicked()
                                    {
                                        if !GLOBALS.signer.is_ready() {
                                            GLOBALS
                                                .status_queue
                                                .write()
                                                .write("Your key is not setup.".to_string());
                                        } else {
                                            let _ =
                                                GLOBALS.to_overlord.send(ToOverlordMessage::Like(
                                                    note.event.id,
                                                    note.event.pubkey,
                                                ));
                                        }
                                    }
                                    for (ch, count) in note.reactions.iter() {
                                        if *ch == '+' {
                                            ui.label(format!("{}", count));
                                        }
                                    }
                                    ui.add_space(12.0);
                                    for (ch, count) in note.reactions.iter() {
                                        if *ch != '+' {
                                            ui.label(
                                                RichText::new(format!("{} {}", ch, count)).strong(),
                                            );
                                        }
                                    }
                                }
                            });

                            // Below the note zap area
                            if let Some(zapnoteid) = app.note_being_zapped {
                                if zapnoteid == note.event.id {
                                    ui.horizontal_wrapped(|ui| {
                                        app.render_zap_area(ui, ctx);
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
    ctx: &Context,
    note_ref: Rc<RefCell<NoteData>>,
    as_deleted: bool,
    content_margin_left: f32,
    content_pull_top: f32,
) {
    if let Ok(note) = note_ref.try_borrow() {
        let event = &note.event;

        let bottom_of_avatar = ui.cursor().top();

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
                    } else if note.muted() {
                        let color = app.theme.notice_marker_text_color();
                        ui.label(
                            RichText::new("MUTED")
                                .color(color)
                                .text_style(TextStyle::Small),
                        );
                    } else if app.render_qr == Some(event.id) {
                        app.render_qr(ui, ctx, "feedqr", event.content.trim());
                    // FIXME should this be the unmodified content (event.content)?
                    } else if event.content_warning().is_some()
                        && !app.approved.contains(&event.id)
                        && !app.settings.approve_content_warning
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
                    } else if note.repost == Some(RepostType::Kind6Embedded) {
                        if note.embedded_event.is_some() {
                            let inner_note_data =
                                NoteData::new(note.embedded_event.clone().unwrap());
                            let inner_ref = Rc::new(RefCell::new(inner_note_data));
                            render_repost(
                                app,
                                ui,
                                ctx,
                                &note.repost,
                                inner_ref,
                                content_margin_left,
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
                                ctx,
                                &note.repost,
                                inner_ref,
                                content_margin_left,
                                bottom_of_avatar,
                            );
                        } else {
                            match event.mentions().first() {
                                Some(EventReference::Id(id, _, _)) => {
                                    if let Some(note_data) = app.notes.try_update_and_get(id) {
                                        // TODO block additional repost recursion
                                        render_repost(
                                            app,
                                            ui,
                                            ctx,
                                            &note.repost,
                                            note_data,
                                            content_margin_left,
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
                            ctx,
                            note_ref.clone(),
                            as_deleted,
                            content_margin_left,
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
    ctx: &Context,
    parent_repost: &Option<RepostType>,
    repost_ref: Rc<RefCell<NoteData>>,
    content_margin_left: f32,
    bottom_of_avatar: f32,
) {
    if let Ok(repost_data) = repost_ref.try_borrow() {
        let render_data = NoteRenderData {
            // the full note height differs from the reposted height anyways
            height: 0.0,
            has_repost: repost_data.repost.is_some(),
            is_comment_mention: *parent_repost == Some(RepostType::CommentMention),
            is_new: false,
            is_main_event: false,
            is_thread: false,
            is_first: false,
            is_last: false,
            thread_position: 0,
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
                    margin.left -= content_margin_left;
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
                        render_note_inner(
                            app,
                            ctx,
                            ui,
                            repost_ref.clone(),
                            &render_data,
                            false,
                            parent_repost,
                        );

                        let bottom = ui.next_widget_position();

                        // Record if the rendered repost was visible
                        {
                            let screen_rect = ctx.input(|i| i.screen_rect); // Rect
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
