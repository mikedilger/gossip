use super::{GossipUi, Page};
use crate::feed::FeedKind;
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{RichText, Ui};
use linkify::{LinkFinder, LinkKind};
use nostr_types::{Event, IdHex, Tag};

pub(super) fn render_content(
    app: &mut GossipUi,
    ui: &mut Ui,
    tag_re: &regex::Regex,
    event: &Event,
    as_deleted: bool,
) {
    for span in LinkFinder::new()
        .kinds(&[LinkKind::Url])
        .spans(&event.content)
    {
        if span.kind().is_some() {
            ui.hyperlink_to(span.as_str(), span.as_str());
        } else {
            let s = span.as_str();
            let mut pos = 0;
            for mat in tag_re.find_iter(s) {
                ui.label(&s[pos..mat.start()]);
                let num: usize = s[mat.start() + 2..mat.end() - 1].parse::<usize>().unwrap();
                if let Some(tag) = event.tags.get(num) {
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
                            let idhex: IdHex = (*id).into();
                            let nam = format!("#{}", GossipUi::hex_id_short(&idhex));
                            if ui.link(&nam).clicked() {
                                app.set_page(Page::Feed(FeedKind::Thread {
                                    id: *id,
                                    referenced_by: event.id,
                                }));
                            };
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
}
