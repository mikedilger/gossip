use super::{GossipUi, Page};
use crate::error::Error;
use crate::feed::FeedKind;
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{Color32, ColorImage, Context, Image, RichText, TextureOptions, Ui, Vec2};
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

pub(super) fn render_qr(app: &mut GossipUi, ui: &mut Ui, ctx: &Context, data: &str) {
    match &app.current_qr {
        Some(Err(e)) => {
            ui.label(
                RichText::new(format!("CANNOT LOAD QR: {}", e)).color(Color32::from_rgb(160, 0, 0)),
            );
        }
        Some(Ok(texture_handle)) => {
            ui.add(Image::new(
                texture_handle,
                Vec2 {
                    x: app.current_qr_size.0,
                    y: app.current_qr_size.1,
                },
            ));
        }
        None => {
            app.current_qr = Some(Err(Error::General("Not Yet Implemented".to_string())));
            // need bytes
            if let Ok(code) = qrcode::QrCode::new(data) {
                let image = code.render::<image::Rgba<u8>>().build();

                // Convert image size into points for later rendering
                let ppp = ctx.pixels_per_point();
                app.current_qr_size = (image.width() as f32 / ppp, image.height() as f32 / ppp);

                let color_image = ColorImage::from_rgba_unmultiplied(
                    [image.width() as usize, image.height() as usize],
                    image.as_flat_samples().as_slice(),
                );
                let texture_handle = ctx.load_texture("qr", color_image, TextureOptions::default());
                app.current_qr = Some(Ok(texture_handle));
            } else {
                app.current_qr = Some(Err(Error::General("Could not make a QR".to_string())));
            }
        }
    }
}
