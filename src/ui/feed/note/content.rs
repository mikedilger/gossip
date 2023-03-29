use super::{GossipUi, NoteData, Page, RepostType};
use crate::feed::FeedKind;
use crate::globals::GLOBALS;
use crate::ui::widgets::break_anywhere_hyperlink_to;
use eframe::{
    egui::{self, Image},
    epaint::Vec2,
};
use egui::{RichText, Ui};
use lazy_static::lazy_static;
use linkify::{LinkFinder, LinkKind};
use nostr_types::find_nostr_bech32_pos;
use nostr_types::{Id, IdHex, NostrBech32, NostrUrl, PublicKeyHex, Tag, UncheckedUrl};
use regex::Regex;

/// A segment of content]
#[derive(Debug)]
pub enum ContentSegment<'a> {
    NostrUrl(NostrUrl),
    TagReference(usize),
    Hyperlink(&'a str),
    Plain(&'a str),
}

/// Break content into a linear sequence of `ContentSegment`s
pub(super) fn shatter_content(mut content: &str) -> Vec<ContentSegment<'_>> {
    let mut segments: Vec<ContentSegment> = Vec::new();

    // Pass 1 - `NostrUrl`s
    while let Some((start, end)) = find_nostr_bech32_pos(content) {
        // The stuff before it
        if start >= 6 && content.get(start - 6..start) == Some("nostr:") {
            segments.append(&mut shatter_content_2(&content[..start - 6]));
        } else {
            segments.append(&mut shatter_content_2(&content[..start]));
        }

        // The Nostr Bech32 itself
        if let Some(nbech) = NostrBech32::try_from_string(&content[start..end]) {
            segments.push(ContentSegment::NostrUrl(NostrUrl(nbech)));
        } else {
            // We have a sequence which matches the Regex for a Bech32, but
            // when parsed more deeply is invalid. Treat it as plain text.
            segments.push(ContentSegment::Plain(&content[start..end]));
        }

        content = &content[end..];
    }

    // The stuff after it
    segments.append(&mut shatter_content_2(content));

    segments
}

// Pass 2 - `TagReference`s
fn shatter_content_2(content: &str) -> Vec<ContentSegment<'_>> {
    lazy_static! {
        static ref TAG_RE: Regex = Regex::new(r"(\#\[\d+\])").unwrap();
    }

    let mut segments: Vec<ContentSegment> = Vec::new();

    let mut pos = 0;
    for mat in TAG_RE.find_iter(content) {
        segments.append(&mut shatter_content_3(&content[pos..mat.start()]));
        // If panics on unwrap, something is wrong with Regex.
        let u: usize = content[mat.start() + 2..mat.end() - 1].parse().unwrap();
        segments.push(ContentSegment::TagReference(u));
        pos = mat.end();
    }

    segments.append(&mut shatter_content_3(&content[pos..]));

    segments
}

fn shatter_content_3(content: &str) -> Vec<ContentSegment<'_>> {
    let mut segments: Vec<ContentSegment> = Vec::new();

    for span in LinkFinder::new().kinds(&[LinkKind::Url]).spans(content) {
        if span.kind().is_some() {
            segments.push(ContentSegment::Hyperlink(span.as_str()));
        } else {
            if !span.as_str().is_empty() {
                segments.push(ContentSegment::Plain(span.as_str()));
            }
        }
    }

    segments
}

/// returns None or a repost
pub(super) fn render_content(
    app: &mut GossipUi,
    ui: &mut Ui,
    note: &NoteData,
    as_deleted: bool,
    content: &str,
) -> Option<NoteData> {
    ui.style_mut().spacing.item_spacing.x = 0.0;

    // Optional repost return
    let mut append_repost: Option<NoteData> = None;

    for segment in shatter_content(content) {
        match segment {
            ContentSegment::NostrUrl(nurl) => match nurl.0 {
                NostrBech32::Pubkey(pk) => {
                    render_profile_link(app, ui, &pk.into());
                }
                NostrBech32::Profile(prof) => {
                    render_profile_link(app, ui, &prof.pubkey.into());
                }
                NostrBech32::Id(id) => {
                    render_event_link(app, ui, note, &id);
                }
                NostrBech32::EventPointer(ep) => {
                    render_event_link(app, ui, note, &ep.id);
                }
            },
            ContentSegment::TagReference(num) => {
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
            }
            ContentSegment::Hyperlink(link) => {
                let lowercase = link.to_lowercase();
                if is_image_url(&lowercase) {
                    crate::ui::widgets::break_anywhere_hyperlink_to(ui, "[ Image ]", link);
                } else if is_video_url(&lowercase) {
                    crate::ui::widgets::break_anywhere_hyperlink_to(ui, "[ Video ]", link);
                } else {
                    crate::ui::widgets::break_anywhere_hyperlink_to(ui, link, link);
                }
            }
            ContentSegment::Plain(text) => {
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

fn is_image_url(url: &str) -> bool {
    url.ends_with(".jpg")
        || url.ends_with(".jpeg")
        || url.ends_with(".png")
        || url.ends_with(".gif")
        || url.ends_with(".webp")
}

fn is_video_url(url: &str) -> bool {
    url.ends_with(".mov")
        || url.ends_with(".mp4")
        || url.ends_with(".mkv")
        || url.ends_with(".webm")
}

/// Try to fetch and render a piece of media
///  - return: true if successfully rendered, false otherwise
fn try_render_media(app: &mut GossipUi, ui: &mut Ui, url_str: &str) -> bool {
    let mut success = false;
    let unchecked_url = UncheckedUrl(url_str.to_string());
    if let Some(media) = app.try_get_media(ui.ctx(), &unchecked_url) {
        // insert a newline if the current line has text
        if ui.cursor().min.x > ui.max_rect().min.x {
            ui.end_row();
        }

        let ui_max = Vec2::new(ui.available_width(), ui.ctx().screen_rect().height() / 4.0);
        let msize = media.size_vec2();
        let aspect = media.aspect_ratio();

        // determine maximum x and y sizes
        let max_x = if ui_max.x > msize.x {
            msize.x
        } else {
            ui_max.x
        };
        let max_y = if ui_max.y > msize.y {
            msize.y
        } else {
            ui_max.y
        };

        // now determine if we are constrained by x or by y and
        // calculate the resulting size
        let mut size = Vec2::new(0.0, 0.0);
        size.x = if max_x > max_y * aspect {
            max_y * aspect
        } else {
            max_x
        };
        size.y = if max_y > max_x / aspect {
            max_x / aspect
        } else {
            max_y
        };

        let row_height = ui.cursor().height();
        let url = unchecked_url.to_string();

        // render the image with a nice frame around it
        egui::Frame::none()
            .inner_margin(egui::Margin::same(0.0))
            .outer_margin(egui::Margin {
                top: ui.available_height(),
                left: 0.0,
                right: 0.0,
                bottom: ui.available_height(),
            })
            .fill(egui::Color32::GRAY)
            .rounding(ui.style().noninteractive().rounding)
            .stroke(egui::Stroke {
                width: 1.0,
                color: egui::Color32::DARK_GRAY,
            })
            .show(ui, |ui| {
                let response = ui.add(Image::new(&media, size).sense(egui::Sense::click()));
                if response.clicked() {
                    let modifiers = ui.ctx().input(|i| i.modifiers);
                    ui.ctx().output_mut(|o| {
                        o.open_url = Some(egui::output::OpenUrl {
                            url: url.clone(),
                            new_tab: modifiers.any(),
                        });
                    });
                }
                if response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            });

        // make other content continue on a new line
        ui.end_row();

        // workaround for egui bug where image enlarges the cursor height
        ui.set_row_height(row_height);

        success = true;
    };
    success
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_shatter_content() {
        let content = "My friend #[0]  wrote me this note: nostr:note10ttnuuvcs29y3k23gwrcurw2ksvgd7c2rrqlfx7urmt5m963vhss8nja90 and it might have referred to https://github.com/Giszmo/nostr.info/blob/master/assets/js/main.js";
        let pieces = shatter_content(content);
        assert_eq!(pieces.len(), 6);
        assert!(matches!(pieces[0], ContentSegment::Plain(..)));
        assert!(matches!(pieces[1], ContentSegment::TagReference(..)));
        assert!(matches!(pieces[2], ContentSegment::Plain(..)));
        assert!(matches!(pieces[3], ContentSegment::NostrUrl(..)));
        assert!(matches!(pieces[4], ContentSegment::Plain(..)));
        assert!(matches!(pieces[5], ContentSegment::Hyperlink(..)));

        let content = r#"This is a test of NIP-27 posting support referencing this note nostr:nevent1qqsqqqq9wh98g4u6e480vyp6p4w3ux2cd0mxn2rssq0w5cscsgzp2ksprpmhxue69uhkzapwdehhxarjwahhy6mn9e3k7mf0qyt8wumn8ghj7etyv4hzumn0wd68ytnvv9hxgtcpremhxue69uhkummnw3ez6ur4vgh8wetvd3hhyer9wghxuet59uq3kamnwvaz7tmwdaehgu3wd45kketyd9kxwetj9e3k7mf0qy2hwumn8ghj7mn0wd68ytn00p68ytnyv4mz7qgnwaehxw309ahkvenrdpskjm3wwp6kytcpz4mhxue69uhhyetvv9ujuerpd46hxtnfduhsz9mhwden5te0wfjkccte9ehx7um5wghxyctwvshszxthwden5te0wfjkccte9eekummjwsh8xmmrd9skctcnmzajy and again without the url data nostr:note1qqqq2aw2w3te4n2w7cgr5r2arcv4s6lkdx58pqq7af3p3qsyz4dqns2935
And referencing this person nostr:npub1acg6thl5psv62405rljzkj8spesceyfz2c32udakc2ak0dmvfeyse9p35c and again as an nprofile nostr:nprofile1qqswuyd9ml6qcxd92h6pleptfrcqucvvjy39vg4wx7mv9wm8kakyujgprdmhxue69uhkummnw3ezumtfddjkg6tvvajhytnrdakj7qg7waehxw309ahx7um5wgkhqatz9emk2mrvdaexgetj9ehx2ap0qythwumn8ghj7un9d3shjtnwdaehgu3wd9hxvme0qyt8wumn8ghj7etyv4hzumn0wd68ytnvv9hxgtcpzdmhxue69uhk7enxvd5xz6tw9ec82c30qy2hwumn8ghj7mn0wd68ytn00p68ytnyv4mz7qgcwaehxw309ashgtnwdaehgunhdaexkuewvdhk6tczkvt9n all on the same damn line even (I think)."#;
        let pieces = shatter_content(content);
        assert_eq!(pieces.len(), 9);
    }
}
