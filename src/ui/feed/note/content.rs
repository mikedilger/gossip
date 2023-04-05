use super::shatter::ContentSegment;
use super::{GossipUi, NoteData, Page, RepostType};
use crate::feed::FeedKind;
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{RichText, Ui};
use nostr_types::{Id, IdHex, NostrBech32, PublicKeyHex, Tag};

/// returns None or a repost
pub(super) fn render_content(
    app: &mut GossipUi,
    ui: &mut Ui,
    note: &NoteData,
    as_deleted: bool,
) -> Option<NoteData> {
    ui.style_mut().spacing.item_spacing.x = 0.0;

    // Optional repost return
    let mut append_repost: Option<NoteData> = None;

    for segment in note.shattered_content.segments.iter() {
        match segment {
            ContentSegment::NostrUrl(nurl) => match &nurl.0 {
                NostrBech32::Pubkey(pk) => {
                    render_profile_link(app, ui, &(*pk).into());
                }
                NostrBech32::Profile(prof) => {
                    render_profile_link(app, ui, &prof.pubkey.into());
                }
                NostrBech32::Id(id) => {
                    render_event_link(app, ui, note, id);
                }
                NostrBech32::EventPointer(ep) => {
                    render_event_link(app, ui, note, &ep.id);
                }
            },
            ContentSegment::TagReference(num) => {
                if let Some(tag) = note.event.tags.get(*num) {
                    match tag {
                        Tag::Pubkey { pubkey, .. } => {
                            render_profile_link(app, ui, pubkey);
                        }
                        Tag::Event { id, .. } => {
                            let mut render_link = true;

                            // we only render the first mention, so if append_repost is_some then skip to render_link
                            // we also respect the user setting "show first mention"
                            if append_repost.is_none() && app.settings.show_first_mention {
                                match note.repost {
                                    Some(RepostType::MentionOnly)
                                    | Some(RepostType::CommentMention)
                                    | Some(RepostType::Kind6Mention) => {
                                        for (i, event) in note.cached_mentions.iter() {
                                            if i == num {
                                                // FIXME is there a way to consume just this entry in cached_mentions so
                                                //       we can avoid the clone?
                                                if let Some(note_data) = super::NoteData::new(
                                                    event.clone(),
                                                    true,
                                                    app.settings.show_long_form,
                                                ) {
                                                    append_repost = Some(note_data);
                                                    render_link = false;
                                                }
                                            }
                                        }
                                    }
                                    _ => (),
                                }
                            }
                            if render_link {
                                render_event_link(app, ui, note, id);
                            }
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
            }
            ContentSegment::Hyperlink(linkspan) => {
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
            ContentSegment::Plain(textspan) => {
                let text = note.shattered_content.slice(textspan).unwrap();
                if as_deleted {
                    ui.label(RichText::new(text).strikethrough());
                } else {
                    ui.label(text);
                }
            }
        }
    }

    ui.reset_style();

    append_repost
}

fn render_profile_link(app: &mut GossipUi, ui: &mut Ui, pubkey: &PublicKeyHex) {
    let nam = GossipUi::display_name_from_pubkeyhex_lookup(pubkey);
    if ui.link(&nam).clicked() {
        app.set_page(Page::Person(pubkey.to_owned()));
    };
}

fn render_event_link(app: &mut GossipUi, ui: &mut Ui, note: &NoteData, id: &Id) {
    let idhex: IdHex = (*id).into();
    let nam = format!("#{}", GossipUi::hex_id_short(&idhex));
    if ui.link(&nam).clicked() {
        app.set_page(Page::Feed(FeedKind::Thread {
            id: *id,
            referenced_by: note.event.id,
        }));
    };
}
