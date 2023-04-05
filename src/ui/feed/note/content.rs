use std::{
    cell::{Ref, RefCell},
    rc::Rc,
};

use super::{shatter::Span, ContentSegment, GossipUi, NoteData, Page, RepostType};
use crate::feed::FeedKind;
use crate::globals::GLOBALS;
use eframe::egui::{self, Context};
use egui::{RichText, Ui};
use nostr_types::{Id, IdHex, NostrBech32, NostrUrl, PublicKey, PublicKeyHex, Tag};

pub(super) fn render_content(
    app: &mut GossipUi,
    ui: &mut Ui,
    ctx: &Context,
    note_ref: Rc<RefCell<NoteData>>,
    as_deleted: bool,
    content_margin_left: f32,
    bottom_of_avatar: f32,
) {
    ui.style_mut().spacing.item_spacing.x = 0.0;

    if let Ok(note) = note_ref.try_borrow() {
        for segment in note.shattered_content.segments.iter() {
            match segment {
                ContentSegment::NostrUrl(nurl) => render_nostr_url(app, ui, &note, nurl),
                ContentSegment::TagReference(num) => {
                    if let Some(tag) = note.event.tags.get(*num) {
                        match tag {
                            Tag::Pubkey { pubkey, .. } => {
                                render_profile_link(app, ui, pubkey);
                            }
                            Tag::Event { id, .. } => {
                                let mut render_link = true;
                                if app.settings.show_first_mention {
                                    match note.repost {
                                        Some(RepostType::MentionOnly)
                                        | Some(RepostType::CommentMention)
                                        | Some(RepostType::Kind6Mention) => {
                                            for (i, cached_id) in note.cached_mentions.iter() {
                                                if *i == *num {
                                                    if let Some(note_data) =
                                                        app.notes.try_update_and_get(cached_id)
                                                    {
                                                        // TODO block additional repost recursion
                                                        super::render_repost(
                                                            app,
                                                            ui,
                                                            ctx,
                                                            &note.repost,
                                                            note_data,
                                                            content_margin_left,
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
                                    render_event_link(app, ui, note.event.id, *id);
                                }
                            }
                            Tag::Hashtag(s) => {
                                render_hashtag(ui, s);
                            }
                            _ => {
                                render_unknown_reference(ui, *num);
                            }
                        }
                    }
                }
                ContentSegment::Hyperlink(linkspan) => render_hyperlink(ui, &note, linkspan),
                ContentSegment::Plain(textspan) => render_plain(ui, &note, textspan, as_deleted),
            }
        }
    }

    ui.reset_style();
}

pub(super) fn render_nostr_url(
    app: &mut GossipUi,
    ui: &mut Ui,
    note: &Ref<NoteData>,
    nurl: &NostrUrl,
) {
    match &nurl.0 {
        NostrBech32::Pubkey(pk) => {
            render_profile_link(app, ui, &<PublicKey as Into<PublicKeyHex>>::into(*pk));
        }
        NostrBech32::Profile(prof) => {
            render_profile_link(app, ui, &prof.pubkey.into());
        }
        NostrBech32::Id(id) => {
            render_event_link(app, ui, note.event.id, *id);
        }
        NostrBech32::EventPointer(ep) => {
            render_event_link(app, ui, note.event.id, ep.id);
        }
    }
}

pub(super) fn render_hyperlink(ui: &mut Ui, note: &Ref<NoteData>, linkspan: &Span) {
    let link = note.shattered_content.slice(linkspan).unwrap();
    let lowercase = link.to_lowercase();
    if lowercase.ends_with(".jpg")
        || lowercase.ends_with(".jpeg")
        || lowercase.ends_with(".png")
        || lowercase.ends_with(".gif")
        || lowercase.ends_with(".webp")
    {
        crate::ui::widgets::break_anywhere_hyperlink_to(ui, "[ Image ]", link);
    } else if lowercase.ends_with(".mov")
        || lowercase.ends_with(".mp4")
        || lowercase.ends_with(".mkv")
        || lowercase.ends_with(".webm")
    {
        crate::ui::widgets::break_anywhere_hyperlink_to(ui, "[ Video ]", link);
    } else {
        crate::ui::widgets::break_anywhere_hyperlink_to(ui, link, link);
    }
}

pub(super) fn render_plain(ui: &mut Ui, note: &Ref<NoteData>, textspan: &Span, as_deleted: bool) {
    let text = note.shattered_content.slice(textspan).unwrap();
    if as_deleted {
        ui.label(RichText::new(text).strikethrough());
    } else {
        ui.label(text);
    }
}

pub(super) fn render_profile_link(app: &mut GossipUi, ui: &mut Ui, pubkey: &PublicKeyHex) {
    let nam = match GLOBALS.people.get(pubkey) {
        Some(p) => match p.name() {
            Some(n) => format!("@{}", n),
            None => format!("@{}", GossipUi::pubkey_short(pubkey)),
        },
        None => format!("@{}", GossipUi::pubkey_short(pubkey)),
    };
    if ui.link(&nam).clicked() {
        app.set_page(Page::Person(pubkey.to_owned()));
    };
}

pub(super) fn render_event_link(
    app: &mut GossipUi,
    ui: &mut Ui,
    referenced_by_id: Id,
    link_to_id: Id,
) {
    let idhex: IdHex = link_to_id.into();
    let nam = format!("#{}", GossipUi::hex_id_short(&idhex));
    if ui.link(&nam).clicked() {
        app.set_page(Page::Feed(FeedKind::Thread {
            id: link_to_id,
            referenced_by: referenced_by_id,
        }));
    };
}

pub(super) fn render_hashtag(ui: &mut Ui, s: &String) {
    if ui.link(format!("#{}", s)).clicked() {
        *GLOBALS.status_message.blocking_write() =
            "Gossip doesn't have a hashtag feed yet.".to_owned();
    }
}

pub(super) fn render_unknown_reference(ui: &mut Ui, num: usize) {
    if ui.link(format!("#[{}]", num)).clicked() {
        *GLOBALS.status_message.blocking_write() =
            "Gossip can't handle this kind of tag link yet.".to_owned();
    }
}
