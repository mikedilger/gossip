use super::FeedNoteParams;
use crate::comms::ToOverlordMessage;
use crate::feed::FeedKind;
use crate::globals::{Globals, GLOBALS};
use crate::people::DbPerson;
use crate::ui::widgets::CopyButton;
use crate::ui::{GossipUi, Page};
use crate::AVATAR_SIZE_F32;
pub const AVATAR_SIZE_REPOST_F32: f32 = 27.0; // points, not pixels
use eframe::egui;
use egui::{
    Align, Context, Frame, Image, Label, Layout, RichText, Sense, Separator, Stroke, TextStyle, Ui,
    Vec2,
};
use nostr_types::{Event, EventDelegation, EventKind, IdHex, PublicKeyHex};

mod content;

pub(super) struct NoteData {
    event: Event,
    delegation: EventDelegation,
    author: DbPerson,
}

impl NoteData {
    pub fn new(event: Event) -> Option<NoteData> {
        // Only render known relevent events
        let enable_reposts = GLOBALS.settings.read().reposts;
        let direct_messages = GLOBALS.settings.read().direct_messages;
        if event.kind != EventKind::TextNote
            && !(enable_reposts && (event.kind == EventKind::Repost))
            && !(direct_messages && (event.kind == EventKind::EncryptedDirectMessage))
        {
            return None;
        }

        let delegation = event.delegation();

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

        Some(NoteData {
            event,
            delegation,
            author,
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
        hide_footer,
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

        match NoteData::new(event) {
            Some(nd) => nd,
            None => return,
        }
    };

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

    // FIXME: determine all repost scenarios here
    let has_repost = note_data.event.kind == EventKind::Repost;

    let render_data = NoteRenderData {
        height: *height,
        has_repost,
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

                    if note_data.author.muted > 0 {
                        ui.label(RichText::new("MUTED POST").monospace().italics());
                    } else {
                        render_note_inner(app, ctx, ui, note_data, &render_data, hide_footer);
                    }
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

    if threaded && !hide_footer {
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
                    hide_footer,
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
) {
    let NoteData {
        event,
        delegation,
        author,
    } = note_data;

    let deletion = Globals::get_deletion_sync(event.id);

    let (reactions, self_already_reacted) = Globals::get_reactions_sync(event.id);

    let tag_re = app.tag_re.clone();

    let collapsed = app.collapsed.contains(&event.id);

    // Avatar first
    let avatar = if let Some(avatar) = app.try_get_avatar(ctx, &author.pubkey) {
        avatar
    } else {
        app.placeholder_avatar.clone()
    };

    let avatar_size = match render_data.has_repost {
        true => AVATAR_SIZE_REPOST_F32,
        false => AVATAR_SIZE_F32,
    };

    if ui
        .add(Image::new(&avatar, Vec2 { x: avatar_size, y: avatar_size }).sense(Sense::click()))
        .clicked()
    {
        app.set_page(Page::Person(author.pubkey.clone()));
    };

    let mut is_a_reply = false;

    // Everything else next
    ui.add_space(6.0);
    ui.vertical(|ui| {
        // First row
        ui.horizontal_wrapped(|ui| {
            GossipUi::render_person_name_line(app, ui, &author);

            if let Some((irt, _)) = event.replies_to() {
                is_a_reply = true;
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

                if is_thread_view && is_a_reply {
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

        // Compute the content
        let content = match event.kind {
            EventKind::TextNote => event.content.clone(),
            EventKind::Repost => {
                if event.content.is_empty() {
                    "#[0]".to_owned() // a bit of a hack
                } else {
                    event.content.clone()
                }
            }
            EventKind::EncryptedDirectMessage => match GLOBALS.signer.decrypt_message(&event) {
                Ok(m) => m,
                Err(_) => "DECRYPTION FAILED".to_owned(),
            },
            _ => "NON FEED RELATED EVENT".to_owned(),
        };

        // MAIN CONTENT
        if !collapsed {
            ui.horizontal_wrapped(|ui| {
                if app.render_raw == Some(event.id) {
                    ui.label(serde_json::to_string_pretty(&event).unwrap());
                } else if app.render_qr == Some(event.id) {
                    app.render_qr(ui, ctx, "feedqr", content.trim());
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
                    if let Ok(inner_event) = serde_json::from_str::<Event>(&content) {
                        if let Some(inner_note_data) = NoteData::new(inner_event) {
                            render_repost(app, ui, ctx, inner_note_data);
                        } else {
                            ui.label("REPOSTED EVENT IS NOT RELEVANT");
                        }
                    } else {
                        // render like a kind-1 event with a mention
                        content::render_content(
                            app,
                            ctx,
                            ui,
                            &tag_re,
                            &event,
                            deletion.is_some(),
                            &content,
                        );
                    }
                } else {
                    content::render_content(
                        app,
                        ctx,
                        ui,
                        &tag_re,
                        &event,
                        deletion.is_some(),
                        &content,
                    );
                }
            });

            ui.add_space(8.0);

            // deleted?
            if let Some(delete_reason) = &deletion {
                ui.label(RichText::new(format!("Deletion Reason: {}", delete_reason)).italics());
                ui.add_space(8.0);
            }

            // Under row
            if !hide_footer {
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
                            ui.output_mut(|o| o.copied_text = content.clone());
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
        }
    });
}

fn thin_repost_separator(ui: &mut Ui) {
    let stroke = ui.style().visuals.widgets.noninteractive.bg_stroke;
    thin_separator(ui, stroke);
}

fn thin_separator(ui: &mut Ui, stroke: Stroke) {
    let mut style = ui.style_mut();
    style.visuals.widgets.noninteractive.bg_stroke = stroke;
    ui.add(Separator::default().spacing(0.0));
    ui.reset_style();
}

pub(super) fn render_repost(
    app: &mut GossipUi,
    ui: &mut Ui,
    ctx: &Context,
    repost_data: NoteData,
) {
    let render_data = NoteRenderData {
        height: 0.0,
        has_repost: false, // FIXME should we consider allowing some recursion?
        is_new: false,
        is_main_event: false,
        is_thread: false,
        is_first: false,
        is_last: false,
        thread_position: 0,
    };

    ui.vertical(|ui| {
        thin_repost_separator(ui);
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            // FIXME: don't do this recursively
            render_note_inner(app, ctx, ui, repost_data, &render_data, false);
        });
        thin_repost_separator(ui);
    });
}
