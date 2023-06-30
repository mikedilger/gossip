use super::{GossipUi, NoteData, Page, RepostType};
use crate::feed::FeedKind;
use crate::globals::GLOBALS;
use eframe::egui::Context;
use eframe::{
    egui::{self, Image, Response},
    epaint::Vec2,
};
use egui::{RichText, Ui};
use nostr_types::{ContentSegment, Id, IdHex, NostrBech32, PublicKeyHex, Span, Tag, Url};
use std::{
    cell::{Ref, RefCell},
    rc::Rc,
};

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
                ContentSegment::NostrUrl(nurl) => {
                    match &nurl.0 {
                        NostrBech32::EventAddr(ea) => {
                            // FIXME - we should link to the event instead
                            ui.label(
                                RichText::new(
                                    format!("nostr:{}", ea.as_bech32_string())
                                ).underline()
                            );
                        }
                        NostrBech32::EventPointer(ep) => {
                            let mut render_link = true;
                            if app.settings.show_mentions {
                                match note.repost {
                                    Some(RepostType::MentionOnly)
                                    | Some(RepostType::CommentMention)
                                    | Some(RepostType::Kind6Mention) => {
                                        if let Some(note_data) =
                                            app.notes.try_update_and_get(&ep.id)
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
                                    _ => (),
                                }
                            }
                            if render_link {
                                render_event_link(app, ui, note.event.id, ep.id);
                            }
                        }
                        NostrBech32::Id(id) => {
                            let mut render_link = true;
                            if app.settings.show_mentions {
                                match note.repost {
                                    Some(RepostType::MentionOnly)
                                    | Some(RepostType::CommentMention)
                                    | Some(RepostType::Kind6Mention) => {
                                        if let Some(note_data) = app.notes.try_update_and_get(id) {
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
                                    _ => (),
                                }
                            }
                            if render_link {
                                render_event_link(app, ui, note.event.id, *id);
                            }
                        }
                        NostrBech32::Profile(prof) => {
                            render_profile_link(app, ui, &prof.pubkey.into());
                        }
                        NostrBech32::Pubkey(pk) => {
                            render_profile_link(app, ui, &(*pk).into());
                        }
                        NostrBech32::Relay(url) => {
                            // FIXME - we should link to the relay page once we have those
                            ui.label(
                                RichText::new(
                                    format!("nostr:{}", url.as_bech32_string())
                                ).underline()
                            );
                        }
                    }
                }
                ContentSegment::TagReference(num) => {
                    if let Some(tag) = note.event.tags.get(*num) {
                        match tag {
                            Tag::Pubkey { pubkey, .. } => {
                                render_profile_link(app, ui, pubkey);
                            }
                            Tag::Event { id, .. } => {
                                let mut render_link = true;
                                if app.settings.show_mentions {
                                    match note.repost {
                                        Some(RepostType::MentionOnly)
                                        | Some(RepostType::CommentMention)
                                        | Some(RepostType::Kind6Mention) => {
                                            for (i, cached_id) in note.mentions.iter() {
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
                            Tag::Hashtag { hashtag, .. } => {
                                render_hashtag(ui, hashtag);
                            }
                            _ => {
                                render_unknown_reference(ui, *num);
                            }
                        }
                    }
                }
                ContentSegment::Hyperlink(linkspan) => render_hyperlink(app, ui, &note, linkspan),
                ContentSegment::Plain(textspan) => render_plain(ui, &note, textspan, as_deleted),
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
    let link = note.shattered_content.slice(linkspan).unwrap();
    if let (Ok(url), Some(nurl)) = (url::Url::try_from(link), app.try_check_url(link)) {
        if is_image_url(&url) {
            show_image_toggle(app, ui, nurl);
        } else if is_video_url(&url) {
            show_video_toggle(app, ui, nurl);
        } else {
            crate::ui::widgets::break_anywhere_hyperlink_to(ui, link, link);
        }
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
    let nam = GossipUi::display_name_from_pubkeyhex_lookup(pubkey);
    let nam = format!("@{}", nam);
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
            author: None,
        }));
    };
}

pub(super) fn render_hashtag(ui: &mut Ui, s: &String) {
    if ui.link(format!("#{}", s)).clicked() {
        GLOBALS
            .status_queue
            .write()
            .write("Gossip doesn't have a hashtag feed yet.".to_owned());
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

fn is_image_url(url: &url::Url) -> bool {
    let lower = url.path().to_lowercase();
    lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".png")
        || lower.ends_with(".gif")
        || lower.ends_with(".webp")
}

fn is_video_url(url: &url::Url) -> bool {
    let lower = url.path().to_lowercase();
    lower.ends_with(".mov")
        || lower.ends_with(".mp4")
        || lower.ends_with(".mkv")
        || lower.ends_with(".webm")
}

fn show_image_toggle(app: &mut GossipUi, ui: &mut Ui, url: Url) {
    let row_height = ui.cursor().height();
    let url_string = url.to_string();
    let mut show_link = true;

    // FIXME show/hide lists should persist app restarts
    let show_image = (app.settings.show_media && !app.media_hide_list.contains(&url))
        || (!app.settings.show_media && app.media_show_list.contains(&url));

    if show_image {
        if let Some(response) = try_render_image(app, ui, url.clone()) {
            show_link = false;

            // full-width toggle
            if response.clicked() {
                if app.media_full_width_list.contains(&url) {
                    app.media_full_width_list.remove(&url);
                } else {
                    app.media_full_width_list.insert(url.clone());
                }
            }
        }
    }

    if show_link {
        let response = ui.link("[ Image ]");
        // show url on hover
        response.clone().on_hover_text(url_string.clone());
        // show media toggle
        if response.clicked() {
            if app.settings.show_media {
                app.media_hide_list.remove(&url);
            } else {
                app.media_show_list.insert(url.clone());
            }
            if !app.settings.load_media {
                GLOBALS.status_queue.write().write(
                    "Fetch Media setting is disabled. Right-click link to open in browser or copy URL".to_owned()
                );
            }
        }
        // context menu
        response.context_menu(|ui| {
            if ui.button("Open in browser").clicked() {
                let modifiers = ui.ctx().input(|i| i.modifiers);
                ui.ctx().output_mut(|o| {
                    o.open_url = Some(egui::output::OpenUrl {
                        url: url_string.clone(),
                        new_tab: modifiers.any(),
                    });
                });
            }
            if ui.button("Copy URL").clicked() {
                ui.output_mut(|o| o.copied_text = url_string.clone());
            }
            if app.has_media_loading_failed(url_string.as_str())
                && ui.button("Retry loading ...").clicked()
            {
                app.retry_media(&url);
            }
        });
    }

    ui.end_row();

    // workaround for egui bug where image enlarges the cursor height
    ui.set_row_height(row_height);
}

/// Try to fetch and render a piece of media
///  - return: true if successfully rendered, false otherwise
fn try_render_image(app: &mut GossipUi, ui: &mut Ui, url: Url) -> Option<Response> {
    let mut response_return = None;
    if let Some(media) = app.try_get_media(ui.ctx(), url.clone()) {
        let size = media_scale(
            app.media_full_width_list.contains(&url),
            ui,
            media.size_vec2(),
        );

        // insert a newline if the current line has text
        if ui.cursor().min.x > ui.max_rect().min.x {
            ui.end_row();
        }

        // render the image with a nice frame around it
        egui::Frame::none()
            .inner_margin(egui::Margin::same(0.0))
            .outer_margin(egui::Margin {
                top: 10.0,
                left: 0.0,
                right: 0.0,
                bottom: 10.0,
            })
            .fill(egui::Color32::TRANSPARENT)
            .rounding(ui.style().noninteractive().rounding)
            .show(ui, |ui| {
                let response = ui.add(Image::new(&media, size).sense(egui::Sense::click()));
                if response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                add_media_menu(app, ui, url, &response);
                response_return = Some(response);
            });
    };
    response_return
}

fn show_video_toggle(app: &mut GossipUi, ui: &mut Ui, url: Url) {
    let row_height = ui.cursor().height();
    let url_string = url.to_string();
    let mut show_link = true;

    // FIXME show/hide lists should persist app restarts
    let show_video = (app.settings.show_media && !app.media_hide_list.contains(&url))
        || (!app.settings.show_media && app.media_show_list.contains(&url));

    if show_video {
        if let Some(response) = try_render_video(app, ui, url.clone()) {
            show_link = false;

            // full-width toggle
            if response.clicked() {
                if app.media_full_width_list.contains(&url) {
                    app.media_full_width_list.remove(&url);
                } else {
                    app.media_full_width_list.insert(url.clone());
                }
            }
        }
    }

    if show_link {
        let response = ui.link("[ Video ]");
        // show url on hover
        response.clone().on_hover_text(url_string.clone());
        // show media toggle
        if response.clicked() {
            if app.settings.show_media {
                app.media_hide_list.remove(&url);
            } else {
                app.media_show_list.insert(url.clone());
            }
            if !app.settings.load_media {
                GLOBALS.status_queue.write().write(
                    "Fetch Media setting is disabled. Right-click link to open in browser or copy URL".to_owned()
                );
            }
        }
        // context menu
        response.context_menu(|ui| {
            if ui.button("Open in browser").clicked() {
                let modifiers = ui.ctx().input(|i| i.modifiers);
                ui.ctx().output_mut(|o| {
                    o.open_url = Some(egui::output::OpenUrl {
                        url: url_string.clone(),
                        new_tab: modifiers.any(),
                    });
                });
            }
            if ui.button("Copy URL").clicked() {
                ui.output_mut(|o| o.copied_text = url_string.clone());
            }
            if app.has_media_loading_failed(url_string.as_str())
                && ui.button("Retry loading ...").clicked()
            {
                app.retry_media(&url);
            }
        });
    }

    ui.end_row();

    // workaround for egui bug where image enlarges the cursor height
    ui.set_row_height(row_height);
}

#[cfg(feature = "video-ffmpeg")]
fn try_render_video(app: &mut GossipUi, ui: &mut Ui, url: Url) -> Option<Response> {
    let mut response_return = None;
    let show_full_width = app.media_full_width_list.contains(&url);
    if let Some(player_ref) = app.try_get_player(ui.ctx(), url.clone()) {
        if let Ok(mut player) = player_ref.try_borrow_mut() {
            let size = media_scale(
                show_full_width,
                ui,
                Vec2 {
                    x: player.width as f32,
                    y: player.height as f32,
                },
            );

            // insert a newline if the current line has text
            if ui.cursor().min.x > ui.max_rect().min.x {
                ui.end_row();
            }

            // show the player
            if !show_full_width {
                player.stop();
            }
            let response = player.ui(ui, [size.x, size.y]);

            add_media_menu(app, ui, url, &response);

            // TODO fix click action
            let new_rect = response.rect.shrink(size.x / 2.0);
            response_return = Some(response.with_new_rect(new_rect))
        }
    }
    response_return
}

#[cfg(not(feature = "video-ffmpeg"))]
fn try_render_video(_app: &mut GossipUi, _ui: &mut Ui, _url: Url) -> Option<Response> {
    None
}

fn media_scale(show_full_width: bool, ui: &Ui, media_size: Vec2) -> Vec2 {
    let aspect = media_size.x / media_size.y;
    let ui_max = if show_full_width {
        Vec2::new(
            ui.available_width() * 0.9,
            ui.ctx().screen_rect().height() * 0.9,
        )
    } else {
        Vec2::new(
            ui.available_width() / 2.0,
            ui.ctx().screen_rect().height() / 3.0,
        )
    };

    // determine maximum x and y sizes
    let max_x = if ui_max.x > media_size.x {
        media_size.x
    } else {
        ui_max.x
    };
    let max_y = if ui_max.y > media_size.y {
        media_size.y
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
    size
}

fn add_media_menu(app: &mut GossipUi, ui: &mut Ui, url: Url, response: &Response) {
    // image button menu to the right of the image
    static BTN_SIZE: Vec2 = Vec2 { x: 20.0, y: 20.0 };
    static TXT_SIZE: f32 = 9.0;
    static SPACE: f32 = 10.0;
    let extend_area = egui::Rect {
        min: response.rect.right_top(),
        max: response.rect.right_bottom() + egui::Vec2::new(BTN_SIZE.x, 0.0),
    };
    let extend_area = extend_area.expand(SPACE * 2.0);
    if let Some(pointer_pos) = ui.ctx().pointer_latest_pos() {
        if extend_area.contains(pointer_pos) {
            ui.add_space(SPACE);
            ui.vertical(|ui| {
                ui.add_space(SPACE);
                if ui
                    .add_sized(
                        BTN_SIZE,
                        egui::Button::new(RichText::new("\u{274C}").size(TXT_SIZE)),
                    )
                    .on_hover_text("Hide (return to a link)")
                    .clicked()
                {
                    if app.settings.show_media {
                        app.media_hide_list.insert(url.clone());
                    } else {
                        app.media_show_list.remove(&url);
                    }
                }
                ui.add_space(SPACE);
                if ui
                    .add_sized(
                        BTN_SIZE,
                        egui::Button::new(RichText::new("\u{1F310}").size(TXT_SIZE)),
                    )
                    .on_hover_text("View in Browser")
                    .clicked()
                {
                    let modifiers = ui.ctx().input(|i| i.modifiers);
                    ui.ctx().output_mut(|o| {
                        o.open_url = Some(egui::output::OpenUrl {
                            url: url.to_string(),
                            new_tab: modifiers.any(),
                        });
                    });
                }
                ui.add_space(SPACE);
                if ui
                    .add_sized(
                        BTN_SIZE,
                        egui::Button::new(RichText::new("\u{1F4CB}").size(TXT_SIZE)),
                    )
                    .on_hover_text("Copy URL")
                    .clicked()
                {
                    ui.output_mut(|o| o.copied_text = url.to_string());
                }
            });
        }
    }
}
