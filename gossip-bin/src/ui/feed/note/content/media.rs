use crate::ui::GossipUi;
use eframe::{egui, epaint};
use egui::{Image, Response, RichText, Ui};
use epaint::Vec2;
use gossip_lib::{MediaLoadingResult, GLOBALS};
use nostr_types::{FileMetadata, Url};

pub fn show_image(
    app: &mut GossipUi,
    ui: &mut Ui,
    url: Url,
    privacy_issue: bool,
    volatile: bool,
    file_metadata: Option<FileMetadata>,
) {
    // insert a newline if the current line has text
    if ui.cursor().min.x > ui.max_rect().min.x {
        ui.end_row();
    }
    let row_height = ui.cursor().height();
    let mut show_link = true;

    // Show image or loading placeholder
    if show(app, &url, privacy_issue) {
        if try_render_image(app, ui, url.clone(), volatile, file_metadata) {
            show_link = false;
        }
    }

    // Show link
    if show_link {
        let url_string = url.to_string();

        // show media toggle
        let response = if privacy_issue {
            ui.link("[ PRIVACY RISK Image ]").on_hover_text(format!("The sender might be trying to associate your nostr pubkey with your IP address. URL={}", url_string))
        } else {
            // show url on hover
            ui.link("[ Image ]").on_hover_text(url_string.clone())
        };

        if response.clicked() {
            app.media_hide_list.remove(&url);
            app.media_show_list.insert(url.clone());
            if !read_setting!(load_media) {
                GLOBALS.status_queue.write().write("Fetch Media setting is disabled. Right-click link to open in browser or copy URL".to_owned());
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
            if let Some(error) = app.has_media_loading_failed(url_string.as_str()) {
                if ui
                    .button("Retry loading ...")
                    .on_hover_text(error)
                    .clicked()
                {
                    app.retry_media(&url);
                }
            }
        });
    }

    ui.end_row();

    // workaround for egui bug where image enlarges the cursor height
    ui.set_row_height(row_height);
}

pub fn show_video(
    app: &mut GossipUi,
    ui: &mut Ui,
    url: Url,
    privacy_issue: bool,
    volatile: bool,
    file_metadata: Option<FileMetadata>,
) {
    // insert a newline if the current line has text
    if ui.cursor().min.x > ui.max_rect().min.x {
        ui.end_row();
    }
    let row_height = ui.cursor().height();
    let mut show_link = true;

    // Show video player or loading placeholder
    if show(app, &url, privacy_issue) {
        if try_render_video(app, ui, url.clone(), volatile, file_metadata) {
            show_link = false;
        }
    }

    // Show link
    if show_link {
        let url_string = url.to_string();

        // show media toggle
        let response = if privacy_issue {
            ui.link("[ PRIVACY RISK Video ]").on_hover_text(format!(
                "The sender might be trying to associate your pubkey with your IP address. URL={}",
                url_string
            ))
        } else {
            // show url on hover
            ui.link("[ Video ]").on_hover_text(url_string.clone())
        };

        if response.clicked() {
            app.media_hide_list.remove(&url);
            app.media_show_list.insert(url.clone());
            if !read_setting!(load_media) {
                GLOBALS.status_queue.write().write("Fetch Media setting is disabled. Right-click link to open in browser or copy URL".to_owned());
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
            if let Some(error) = app.has_media_loading_failed(url_string.as_str()) {
                if ui
                    .button("Retry loading ...")
                    .on_hover_text(error)
                    .clicked()
                {
                    app.retry_media(&url);
                }
            }
        });
    }

    ui.end_row();

    // workaround for egui bug where image enlarges the cursor height
    ui.set_row_height(row_height);
}

/// Try to fetch and render a piece of media
///  - return: true if successfully rendered, false otherwise
fn try_render_image(
    app: &mut GossipUi,
    ui: &mut Ui,
    url: Url,
    volatile: bool,
    file_metadata: Option<FileMetadata>,
) -> bool {
    match app.try_get_media(ui.ctx(), url.clone(), volatile, file_metadata.as_ref()) {
        MediaLoadingResult::Disabled => {
            // will render link
            false
        }
        MediaLoadingResult::Loading => {
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
                    let text = if let Some(fm) = &file_metadata {
                        if let Some(alt) = &fm.alt {
                            &format!("Loading image: {alt}")
                        } else if let Some(summary) = &fm.summary {
                            &format!("Loading image: {summary}")
                        } else {
                            "Loading image..."
                        }
                    } else {
                        "Loading image..."
                    };
                    let color = app.theme.notice_marker_text_color();
                    ui.label(RichText::new(text).color(color))
                });
            true
        }
        MediaLoadingResult::Ready(media) => {
            let size = media_scale(
                app.media_full_width_list.contains(&url),
                ui,
                media.size_vec2(),
            );

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
                    let response = ui.add(
                        Image::new(&media)
                            .max_size(size)
                            .maintain_aspect_ratio(true)
                            .sense(egui::Sense::click()),
                    );
                    if response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }

                    // full-width toggle
                    if response.clicked() {
                        if app.media_full_width_list.contains(&url) {
                            app.media_full_width_list.remove(&url);
                        } else {
                            app.media_full_width_list.insert(url.clone());
                        }
                    }

                    add_media_menu(app, ui, url, &response);
                });
            true
        }
        MediaLoadingResult::Failed(ref s) => {
            let color = app.theme.notice_marker_text_color();
            ui.label(RichText::new(format!("COULD NOT LOAD MEDIA: {s}")).color(color));
            ui.end_row();
            false
        }
    }
}

#[cfg(feature = "video-ffmpeg")]
fn try_render_video(
    app: &mut GossipUi,
    ui: &mut Ui,
    url: Url,
    volatile: bool,
    file_metadata: Option<FileMetadata>,
) -> bool {
    let show_full_width = app.media_full_width_list.contains(&url);
    match app.try_get_player(ui.ctx(), url.clone(), volatile, file_metadata.as_ref()) {
        MediaLoadingResult::Disabled => {
            // will render link
            false
        }
        MediaLoadingResult::Loading => {
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
                    let text = if let Some(fm) = &file_metadata {
                        // FIXME do blurhash
                        if let Some(alt) = &fm.alt {
                            &format!("Loading video: {alt}")
                        } else if let Some(summary) = &fm.summary {
                            &format!("Loading video: {summary}")
                        } else {
                            "Loading video..."
                        }
                    } else {
                        "Loading video..."
                    };
                    let color = app.theme.notice_marker_text_color();
                    ui.label(RichText::new(text).color(color))
                });
            true
        }
        MediaLoadingResult::Ready(player_ref) => {
            if let Ok(mut player) = player_ref.try_borrow_mut() {
                let size = media_scale(
                    show_full_width,
                    ui,
                    Vec2 {
                        x: player.width as f32,
                        y: player.height as f32,
                    },
                );

                // show the player
                if !show_full_width {
                    player.stop();
                }
                let response = player.ui(ui, [size.x, size.y]);

                // stop the player when it scrolls out of view
                if !ui.is_rect_visible(response.rect) {
                    player.stop();
                }

                add_media_menu(app, ui, url.clone(), &response);

                // TODO fix click action
                let new_rect = response.rect.shrink(size.x / 2.0);

                // full-width toggle
                if response.with_new_rect(new_rect).clicked() {
                    if app.media_full_width_list.contains(&url) {
                        app.media_full_width_list.remove(&url);
                    } else {
                        app.media_full_width_list.insert(url.clone());
                    }
                }
            }

            true
        }
        MediaLoadingResult::Failed(ref s) => {
            let color = app.theme.notice_marker_text_color();
            ui.label(RichText::new(format!("COULD NOT LOAD MEDIA: {s}")).color(color));
            ui.end_row();
            false
        }
    }
}

#[cfg(not(feature = "video-ffmpeg"))]
fn try_render_video(
    _app: &mut GossipUi,
    _ui: &mut Ui,
    _url: Url,
    _volatile: bool,
    _file_metadata: Option<FileMetadata>,
) -> bool {
    false
}

// Should we show the media, or fall back to a link?
fn show(app: &mut GossipUi, url: &Url, privacy_issue: bool) -> bool {
    // FIXME show/hide lists should persist app restarts
    let show_media_setting = read_setting!(show_media);
    let overriding_hide = app.media_hide_list.contains(url);
    let overriding_show = app.media_show_list.contains(url);
    overriding_show || (show_media_setting && !overriding_hide && !privacy_issue)
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
        if extend_area.contains(pointer_pos) && ui.is_enabled() {
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
                    app.media_hide_list.insert(url.clone());
                    app.media_show_list.remove(&url);
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
