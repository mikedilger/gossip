use super::{GossipUi, NoteData, Page, RepostType};
use crate::feed::FeedKind;
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{RichText, Ui};
use lazy_static::lazy_static;
use linkify::{LinkFinder, LinkKind};
use nostr_types::{EventPointer, Id, IdHex, PublicKey, Tag};
use regex::Regex;

/// returns None or a repost
pub(super) fn render_content(
    app: &mut GossipUi,
    ui: &mut Ui,
    note: &NoteData,
    as_deleted: bool,
    content: &str,
) -> Option<NoteData> {
    lazy_static! {
        static ref TAG_RE: Regex = Regex::new(r"(\#\[\d+\])").unwrap();
        static ref NIP27_RE: Regex = Regex::new(r"(?i:nostr:[[:alnum:]]+)").unwrap();
    }

    ui.style_mut().spacing.item_spacing.x = 0.0;

    // Optional repost return
    let mut append_repost: Option<NoteData> = None;

    for span in LinkFinder::new().kinds(&[LinkKind::Url]).spans(content) {
        if span.kind().is_some() {
            let lower_span = span.as_str().to_lowercase();
            if lower_span.ends_with(".jpg")
                || lower_span.ends_with(".jpeg")
                || lower_span.ends_with(".png")
                || lower_span.ends_with(".gif")
            {
                crate::ui::widgets::break_anywhere_hyperlink_to(ui, "[ Image ]", span.as_str());
            } else if lower_span.ends_with(".mov") || lower_span.ends_with(".mp4") {
                crate::ui::widgets::break_anywhere_hyperlink_to(ui, "[ Video ]", span.as_str());
            } else {
                crate::ui::widgets::break_anywhere_hyperlink_to(ui, span.as_str(), span.as_str());
            }
        } else {
            let s = span.as_str();
            let mut pos = 0;
            for mat in TAG_RE.find_iter(s) {
                ui.label(&s[pos..mat.start()]);
                let num: usize = s[mat.start() + 2..mat.end() - 1].parse::<usize>().unwrap();
                if let Some(tag) = note.event.tags.get(num) {
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
                                            if *i == num {
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
                pos = mat.end();
            }
            let rest = &s[pos..];
            // implement NIP-27 nostr: links that include NIP-19 bech32 references
            if rest.contains("nostr:") {
                let mut nospos = 0;
                for mat in NIP27_RE.find_iter(rest) {
                    ui.label(&s[nospos..mat.start()]); // print whatever comes before the match
                    let mut link_parsed = false;
                    let link = &s[mat.start() + 6..mat.end()];
                    if link.starts_with("note1") {
                        if let Ok(id) = Id::try_from_bech32_string(link) {
                            render_event_link(app, ui, note, &id);
                            link_parsed = true;
                        }
                    } else if link.starts_with("nevent1") {
                        if let Ok(ep) = EventPointer::try_from_bech32_string(link) {
                            render_event_link(app, ui, note, &ep.id);
                            link_parsed = true;
                        }
                    } else if link.starts_with("npub1") {
                        if let Ok(pk) = PublicKey::try_from_bech32_string(link) {
                            render_profile_link(app, ui, &pk.into());
                            link_parsed = true;
                        }
                    }
                    if !link_parsed {
                        ui.label(format!("nostr:{}", link));
                    }
                    nospos = mat.end();
                }
            } else {
                if as_deleted {
                    ui.label(RichText::new(rest).strikethrough());
                } else {
                    ui.label(rest);
                }
            }
        }
    }

    ui.reset_style();

    append_repost
}

fn render_profile_link(
    app: &mut GossipUi,
    ui: &mut Ui,
    pubkey: &gossip_relay_picker::PublicKeyHex,
) {
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
