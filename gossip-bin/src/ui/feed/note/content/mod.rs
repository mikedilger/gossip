mod media;

use super::{GossipUi, NoteData, Page, RepostType};
use eframe::egui;
use egui::{Button, Color32, Margin, Pos2, RichText, Stroke, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::FeedKind;
use gossip_lib::GLOBALS;
use nostr_types::{
    ContentSegment, FileMetadata, Id, NAddr, NEvent, NostrBech32, NostrUrl, ParsedTag, PublicKey,
    RelayUrl, Span,
};
use std::{
    cell::{Ref, RefCell},
    rc::Rc,
};

const MAX_POST_HEIGHT: f32 = 200.0;

pub(super) fn render_content(
    app: &mut GossipUi,
    ui: &mut Ui,
    note_ref: Rc<RefCell<NoteData>>,
    as_deleted: bool,
    content_inner_margin: Margin,
    bottom_of_avatar: f32,
) {
    ui.style_mut().spacing.item_spacing.x = 0.0;

    if let Ok(note) = note_ref.try_borrow() {
        if let Some(error) = &note.error_content {
            let color = app.theme.notice_marker_text_color();
            ui.label(RichText::new(error).color(color));
            ui.end_row();
            // fall through in case there is also shattered content to display
        }

        let content_start = ui.next_widget_position();

        for segment in note.shattered_content.segments.iter() {
            if ui.next_widget_position().y > content_start.y + MAX_POST_HEIGHT {
                if !app.opened.contains(&note.event.id) {
                    ui.end_row();
                    ui.end_row();
                    let text_color = if app.theme.dark_mode {
                        Color32::WHITE
                    } else {
                        Color32::BLACK
                    };
                    let button = Button::new("Show more ▼").stroke(Stroke::new(1.0, text_color));
                    if ui.add(button).clicked() {
                        app.opened.insert(note.event.id);
                    }
                    break;
                }
            }

            match segment {
                ContentSegment::NostrUrl(nurl) => {
                    match &nurl.0 {
                        NostrBech32::CryptSec(cs) => {
                            ui.label(RichText::new(cs.as_bech32_string()).underline());
                        }
                        NostrBech32::NAddr(ea) => {
                            render_parameterized_event_link(app, ui, note.event.id, ea);
                        }
                        NostrBech32::NEvent(ne) => {
                            let mut render_link = true;
                            if read_setting!(show_mentions) {
                                match note.repost {
                                    Some(RepostType::MentionOnly)
                                    | Some(RepostType::CommentMention)
                                    | Some(RepostType::Kind6Mention) => {
                                        if let Some(note_data) =
                                            app.notecache.try_update_and_get(&ne.id)
                                        {
                                            // TODO block additional repost recursion
                                            super::render_repost(
                                                app,
                                                ui,
                                                &note.repost,
                                                note_data,
                                                content_inner_margin,
                                                bottom_of_avatar,
                                            );
                                            render_link = false;
                                        }
                                    }
                                    _ => (),
                                }
                            }
                            if render_link {
                                render_nevent1_link(app, ui, ne.clone(), note.event.id);
                            }
                        }
                        NostrBech32::Id(id) => {
                            let mut render_link = true;
                            if read_setting!(show_mentions) {
                                match note.repost {
                                    Some(RepostType::MentionOnly)
                                    | Some(RepostType::CommentMention)
                                    | Some(RepostType::Kind6Mention) => {
                                        if let Some(note_data) =
                                            app.notecache.try_update_and_get(id)
                                        {
                                            // TODO block additional repost recursion
                                            super::render_repost(
                                                app,
                                                ui,
                                                &note.repost,
                                                note_data,
                                                content_inner_margin,
                                                bottom_of_avatar,
                                            );
                                            render_link = false;
                                        }
                                    }
                                    _ => (),
                                }
                            }
                            if render_link {
                                render_note_id_link(app, ui, note.event.id, *id);
                            }
                        }
                        NostrBech32::Profile(prof) => {
                            render_profile_link(app, ui, &prof.pubkey);
                        }
                        NostrBech32::Pubkey(pubkey) => {
                            render_profile_link(app, ui, pubkey);
                        }
                        NostrBech32::Relay(url) => {
                            if let Ok(relay_url) = RelayUrl::try_from_unchecked_url(url) {
                                render_relay_link(app, ui, relay_url);
                            } else {
                                ui.label(RichText::new(&url.0).underline());
                            }
                        }
                    }
                }
                ContentSegment::TagReference(num) => {
                    if let Some(tag) = note.event.tags.get(*num) {
                        if let Ok(parsed) = tag.parse() {
                            match parsed {
                                ParsedTag::Pubkey { pubkey, .. } => {
                                    render_profile_link(app, ui, &pubkey);
                                }
                                ParsedTag::Event {
                                    id,
                                    recommended_relay_url,
                                    ..
                                } => {
                                    let mut render_link = true;
                                    if read_setting!(show_mentions) {
                                        match note.repost {
                                            Some(RepostType::MentionOnly)
                                            | Some(RepostType::CommentMention)
                                            | Some(RepostType::Kind6Mention) => {
                                                for (i, cached_id) in note.mentions.iter() {
                                                    if *i == *num {
                                                        if let Some(note_data) = app
                                                            .notecache
                                                            .try_update_and_get(cached_id)
                                                        {
                                                            // TODO block additional repost recursion
                                                            super::render_repost(
                                                                app,
                                                                ui,
                                                                &note.repost,
                                                                note_data,
                                                                content_inner_margin,
                                                                bottom_of_avatar,
                                                            );
                                                            render_link = false;
                                                        }
                                                    }
                                                }
                                            }
                                            _ => (),
                                        }
                                    }
                                    if render_link {
                                        if let Some(rurl) = recommended_relay_url {
                                            let nevent = NEvent {
                                                id,
                                                relays: vec![rurl],
                                                kind: None,
                                                author: None,
                                            };
                                            render_nevent1_link(app, ui, nevent, note.event.id);
                                        } else {
                                            render_note_id_link(app, ui, note.event.id, id);
                                        }
                                    }
                                }
                                ParsedTag::Hashtag(hashtag) => {
                                    render_hashtag(app, ui, &hashtag);
                                }
                                _ => {
                                    render_unknown_reference(ui, *num);
                                }
                            }
                        }
                    }
                }
                ContentSegment::Hyperlink(linkspan) => render_hyperlink(app, ui, &note, linkspan),
                ContentSegment::Plain(textspan) => {
                    if render_plain(app, ui, &note, textspan, as_deleted, content_start) {
                        // returns true if it did a 'show more'
                        break;
                    }
                }
                ContentSegment::Hashtag(ht) => {
                    render_hashtag(app, ui, ht);
                }
            }
        }

        if app.opened.contains(&note.event.id) {
            ui.end_row();
            ui.end_row();
            if ui.button("Show less ▲").clicked() {
                app.opened.remove(&note.event.id);
            }
        }
    }

    ui.reset_style();
}

pub(super) fn render_hyperlink(
    app: &mut GossipUi,
    ui: &mut Ui,
    note: &Ref<NoteData>,
    linkspan: &Span,
) {
    let link = match note.shattered_content.slice(linkspan) {
        Some(l) => l,
        None => {
            tracing::error!("Corrupt note content");
            return;
        }
    };

    // Check for a matching imeta tag
    let mut file_metadata: Option<FileMetadata> = None;
    {
        let mut vec = note.event.file_metadata();
        for fm in vec.drain(..) {
            if fm.url.as_str() == link {
                file_metadata = Some(fm);
                break;
            }
        }
    }

    if let Ok(relay_url) = RelayUrl::try_from_str(link) {
        render_relay_link(app, ui, relay_url);
        return;
    }

    // In DMs, fetching an image allows someone to associate your pubkey with your IP address
    // by controlling the image URL, and since only you see the URL it must have been you
    let privacy_issue = note.direct_message;

    if let (Ok(url), Some(nurl)) = (url::Url::try_from(link), app.try_check_url(link)) {
        if let Some(mimetype) = gossip_lib::media_url_mimetype(url.path()) {
            if mimetype.starts_with("image/") {
                media::show_image(app, ui, nurl, privacy_issue, note.volatile, file_metadata);
            } else if mimetype.starts_with("video/") {
                media::show_video(app, ui, nurl, privacy_issue, note.volatile, file_metadata);
            }
        } else {
            crate::ui::widgets::break_anywhere_hyperlink_to(ui, app, link, link);
        }
    } else {
        crate::ui::widgets::break_anywhere_hyperlink_to(ui, app, link, link);
    }
}

pub(super) fn render_plain(
    app: &mut GossipUi,
    ui: &mut Ui,
    note: &Ref<NoteData>,
    textspan: &Span,
    as_deleted: bool,
    content_start: Pos2,
) -> bool {
    let text = match note.shattered_content.slice(textspan) {
        Some(t) => t,
        None => {
            tracing::error!("Corrupt note content");
            return false;
        }
    };

    let mut first = true;
    for line in text.split('\n') {
        if ui.next_widget_position().y > content_start.y + MAX_POST_HEIGHT {
            if !app.opened.contains(&note.event.id) {
                ui.end_row();
                ui.end_row();
                let text_color = if app.theme.dark_mode {
                    Color32::WHITE
                } else {
                    Color32::BLACK
                };
                let button = Button::new("Show more ▼").stroke(Stroke::new(1.0, text_color));
                if ui.add(button).clicked() {
                    app.opened.insert(note.event.id);
                }
                return true; // means we put the 'show more' button in place
            }
        }

        if !first {
            // carraige return
            ui.end_row();
        }

        if as_deleted {
            ui.label(RichText::new(line).strikethrough());
        } else {
            ui.label(line);
        }

        first = false;
    }

    false
}

pub(super) fn render_profile_link(app: &mut GossipUi, ui: &mut Ui, pubkey: &PublicKey) {
    let name = gossip_lib::names::best_name_from_pubkey_lookup(pubkey);
    if ui.link(&name).clicked() {
        app.set_page(ui.ctx(), Page::Person(pubkey.to_owned()));
    };
}

pub fn render_relay_link(app: &mut GossipUi, ui: &mut Ui, relay_url: RelayUrl) {
    if ui.link(relay_url.as_str()).clicked() {
        app.set_page(ui.ctx(), Page::RelaysKnownNetwork(Some(relay_url)));
    };
}

pub fn render_note_id_link(app: &mut GossipUi, ui: &mut Ui, referenced_by_id: Id, link_to_id: Id) {
    let nevent = NEvent {
        id: link_to_id,
        relays: vec![],
        kind: None,
        author: None,
    };
    render_nevent1_link(app, ui, nevent, referenced_by_id)
}

pub fn render_nevent1_link(app: &mut GossipUi, ui: &mut Ui, nevent: NEvent, referenced_by_id: Id) {
    let id = nevent.id;
    let nurl = NostrUrl(NostrBech32::NEvent(nevent));
    let name = format!("{}", nurl);

    if ui.link(&name).clicked() {
        app.set_page(
            ui.ctx(),
            Page::Feed(FeedKind::Thread {
                id,
                referenced_by: referenced_by_id,
                author: None,
            }),
        );
    };
}

pub(super) fn render_parameterized_event_link(
    app: &mut GossipUi,
    ui: &mut Ui,
    referenced_by_id: Id,
    naddr: &NAddr,
) {
    let name = format!("[{:?}: {}]", naddr.kind, naddr.d);
    // let name = format!("nostr:{}", naddr.as_bech32_string());
    if ui.link(&name).clicked() {
        if let Ok(Some(prevent)) =
            GLOBALS
                .db()
                .get_replaceable_event(naddr.kind, naddr.author, &naddr.d)
        {
            app.set_page(
                ui.ctx(),
                Page::Feed(FeedKind::Thread {
                    id: prevent.id,
                    referenced_by: referenced_by_id,
                    author: Some(prevent.pubkey),
                }),
            );
        } else {
            // Disclose failure
            GLOBALS
                .status_queue
                .write()
                .write("Parameterized event not found.".to_owned());

            // Start fetch
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::FetchNAddr(naddr.to_owned()));
        }
    };
}

pub(super) fn render_hashtag(app: &mut GossipUi, ui: &mut Ui, s: &String) {
    let hashtag = format!("#{}", s);
    if ui.link(&hashtag).clicked() {
        app.search = hashtag.to_ascii_lowercase();
        app.set_page(ui.ctx(), Page::SearchLocal);
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::SearchLocally(app.search.clone()));
    }
}

pub(super) fn render_unknown_reference(ui: &mut Ui, num: usize) {
    if ui.link(format!("#[{}]", num)).clicked() {
        GLOBALS
            .status_queue
            .write()
            .write("Gossip can't handle this kind of tag link yet.".to_owned());
    }
}
