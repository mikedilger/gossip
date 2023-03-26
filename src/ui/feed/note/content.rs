use super::{GossipUi, NoteData, Page, RepostType};
use crate::feed::FeedKind;
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{RichText, Ui};
use linkify::{LinkFinder, LinkKind};
use nostr_types::{IdHex, Tag};

/// returns None or a repost
pub(super) fn render_content(
    app: &mut GossipUi,
    ui: &mut Ui,
    note: &NoteData,
    as_deleted: bool,
    content: &str,
) -> Option<NoteData> {
    let tag_re = app.tag_re.clone();
    ui.style_mut().spacing.item_spacing.x = 0.0;

    // Optional repost return
    let mut append_repost: Option<NoteData> = None;

    for span in LinkFinder::new().kinds(&[LinkKind::Url]).spans(content) {
        if span.kind().is_some() {
            if span.as_str().ends_with(".jpg")
                || span.as_str().ends_with(".jpeg")
                || span.as_str().ends_with(".png")
                || span.as_str().ends_with(".gif")
            {
                crate::ui::widgets::break_anywhere_hyperlink_to(ui, "[ Image ]", span.as_str());
            } else if span.as_str().ends_with(".mov") || span.as_str().ends_with(".mp4") {
                crate::ui::widgets::break_anywhere_hyperlink_to(ui, "[ Video ]", span.as_str());
            } else {
                crate::ui::widgets::break_anywhere_hyperlink_to(ui, span.as_str(), span.as_str());
            }
        } else {
            let s = span.as_str();
            let mut pos = 0;
            for mat in tag_re.find_iter(s) {
                ui.label(&s[pos..mat.start()]);
                let num: usize = s[mat.start() + 2..mat.end() - 1].parse::<usize>().unwrap();
                if let Some(tag) = note.event.tags.get(num) {
                    match tag {
                        Tag::Pubkey { pubkey, .. } => {
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
                                                if let Some(note_data) =
                                                    super::NoteData::new(event.clone(), true)
                                                {
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
                                // insert a newline if the current line has text
                                if ui.cursor().min.x > ui.max_rect().min.y {
                                    ui.end_row();
                                }

                                let idhex: IdHex = (*id).into();
                                let nam = format!("#{}", GossipUi::hex_id_short(&idhex));
                                if ui.link(&nam).clicked() {
                                    app.set_page(Page::Feed(FeedKind::Thread {
                                        id: *id,
                                        referenced_by: note.event.id,
                                    }));
                                };
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
            if as_deleted {
                ui.label(RichText::new(&s[pos..]).strikethrough());
            } else {
                ui.label(&s[pos..]);
            }
        }
    }

    ui.reset_style();

    append_repost
}
