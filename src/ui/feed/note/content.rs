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
use nostr_types::{EventPointer, Id, IdHex, PublicKey, Tag, UncheckedUrl};
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
            if is_image_url(&lower_span) {
                // TODO replace this with a per author setting (persisted) and a per note setting (RAM only)
                if !GLOBALS.settings.read().load_media || !try_render_media(app, ui, span.as_str())
                {
                    break_anywhere_hyperlink_to(ui, "[ Image ]", span.as_str());
                }
            } else if is_video_url(&lower_span) {
                break_anywhere_hyperlink_to(ui, "[ Video ]", span.as_str());
            } else {
                break_anywhere_hyperlink_to(ui, span.as_str(), span.as_str());
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

fn is_image_url(url: &String) -> bool {
    return url.ends_with(".jpg")
        || url.ends_with(".jpeg")
        || url.ends_with(".png")
        || url.ends_with(".gif");
}

fn is_video_url(url: &String) -> bool {
    return url.ends_with(".mov") || url.ends_with(".mp4") || url.ends_with(".webp");
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

        let ui_max = Vec2::new(
            ui.available_width(),
            ui.ctx().screen_rect().height() / 4.0,
        );
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

        success = true;
    };
    success
}
