mod feed;
mod help;
mod people;
mod relays;
mod settings;
pub(crate) mod style;
mod widgets;
mod you;

use crate::about::About;
use crate::error::Error;
use crate::feed::FeedKind;
use crate::globals::GLOBALS;
use crate::people::DbPerson;
use crate::settings::Settings;
use crate::ui::widgets::CopyButton;
use eframe::{egui, IconData, Theme};
use egui::{
    ColorImage, Context, ImageData, Label, RichText, SelectableLabel, Sense, TextStyle,
    TextureHandle, TextureOptions, Ui,
};
use nostr_types::{Id, IdHex, Metadata, PublicKey, PublicKeyHex};
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use zeroize::Zeroize;

pub fn run() -> Result<(), Error> {
    let icon_bytes = include_bytes!("../../gossip.png");
    let icon = image::load_from_memory(icon_bytes)?.to_rgba8();
    let (icon_width, icon_height) = icon.dimensions();

    let options = eframe::NativeOptions {
        decorated: true,
        drag_and_drop_support: true,
        default_theme: Theme::Light,
        icon_data: Some(IconData {
            rgba: icon.into_raw(),
            width: icon_width,
            height: icon_height,
        }),
        initial_window_size: Some(egui::vec2(700.0, 900.0)),
        resizable: true,
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        "gossip",
        options,
        Box::new(|cc| Box::new(GossipUi::new(cc))),
    );

    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
enum Page {
    Feed(FeedKind),
    PeopleList,
    PeopleFollow,
    Person(PublicKeyHex),
    YourKeys,
    YourMetadata,
    Relays,
    Settings,
    HelpHelp,
    HelpStats,
    HelpAbout,
}

struct GossipUi {
    next_frame: Instant,
    page: Page,
    history: Vec<Page>,
    about: About,
    icon: TextureHandle,
    placeholder_avatar: TextureHandle,
    draft: String,
    tag_someone: String,
    settings: Settings,
    nip05follow: String,
    follow_bech32_pubkey: String,
    follow_hex_pubkey: String,
    follow_pubkey_at_relay: String,
    password: String,
    del_password: String,
    import_priv: String,
    import_pub: String,
    replying_to: Option<Id>,
    avatars: HashMap<PublicKeyHex, TextureHandle>,
    new_relay_url: String,
    tag_re: regex::Regex,
    override_dpi: bool,
    override_dpi_value: u32,
    editing_metadata: bool,
    metadata: Metadata,
    new_metadata_fieldname: String,
}

impl Drop for GossipUi {
    fn drop(&mut self) {
        self.password.zeroize();
    }
}

impl GossipUi {
    fn new(cctx: &eframe::CreationContext<'_>) -> Self {
        let settings = GLOBALS.settings.blocking_read().clone();

        if let Some(dpi) = settings.override_dpi {
            let ppt: f32 = dpi as f32 / 72.0;
            cctx.egui_ctx.set_pixels_per_point(ppt);
            tracing::debug!("Pixels per point: {}", ppt);
        } else if let Some(ppt) = cctx.integration_info.native_pixels_per_point {
            cctx.egui_ctx.set_pixels_per_point(ppt);
            tracing::debug!("Pixels per point: {}", ppt);
        } else {
            tracing::debug!("Pixels per point: {}", cctx.egui_ctx.pixels_per_point());
        }

        // Set global pixels_per_point_times_100, used for image scaling.
        GLOBALS.pixels_per_point_times_100.store(
            (cctx.egui_ctx.pixels_per_point() * 100.0) as u32,
            Ordering::Relaxed,
        );

        if !settings.light_mode {
            cctx.egui_ctx.set_visuals(style::dark_mode_visuals());
        } else {
            cctx.egui_ctx.set_visuals(style::light_mode_visuals());
        };

        cctx.egui_ctx.set_fonts(style::font_definitions());

        let mut style: egui::Style = (*cctx.egui_ctx.style()).clone();
        style.text_styles = style::text_styles();
        cctx.egui_ctx.set_style(style);

        let icon_texture_handle = {
            let bytes = include_bytes!("../../gossip.png");
            let image = image::load_from_memory(bytes).unwrap();
            let size = [image.width() as _, image.height() as _];
            let image_buffer = image.to_rgba8();
            let pixels = image_buffer.as_flat_samples();
            cctx.egui_ctx.load_texture(
                "icon",
                ImageData::Color(ColorImage::from_rgba_unmultiplied(size, pixels.as_slice())),
                TextureOptions::default(), // magnification, minification
            )
        };

        let placeholder_avatar_texture_handle = {
            let bytes = include_bytes!("../../placeholder_avatar.png");
            let image = image::load_from_memory(bytes).unwrap();
            let size = [image.width() as _, image.height() as _];
            let image_buffer = image.to_rgba8();
            let pixels = image_buffer.as_flat_samples();
            cctx.egui_ctx.load_texture(
                "placeholder_avatar",
                ImageData::Color(ColorImage::from_rgba_unmultiplied(size, pixels.as_slice())),
                TextureOptions::default(), // magnification, minification
            )
        };

        let current_dpi = (cctx.egui_ctx.pixels_per_point() * 72.0) as u32;
        let (override_dpi, override_dpi_value): (bool, u32) = match settings.override_dpi {
            Some(v) => (true, v),
            None => (false, current_dpi),
        };

        GossipUi {
            next_frame: Instant::now(),
            page: Page::Feed(FeedKind::General),
            history: vec![],
            about: crate::about::about(),
            icon: icon_texture_handle,
            placeholder_avatar: placeholder_avatar_texture_handle,
            draft: "".to_owned(),
            tag_someone: "".to_owned(),
            settings,
            nip05follow: "".to_owned(),
            follow_bech32_pubkey: "".to_owned(),
            follow_hex_pubkey: "".to_owned(),
            follow_pubkey_at_relay: "".to_owned(),
            password: "".to_owned(),
            del_password: "".to_owned(),
            import_priv: "".to_owned(),
            import_pub: "".to_owned(),
            replying_to: None,
            avatars: HashMap::new(),
            new_relay_url: "".to_owned(),
            tag_re: regex::Regex::new(r"(\#\[\d+\])").unwrap(),
            override_dpi,
            override_dpi_value,
            editing_metadata: false,
            metadata: Metadata::new(),
            new_metadata_fieldname: String::new(),
        }
    }

    fn set_page(&mut self, page: Page) {
        if self.page != page {
            tracing::trace!("PUSHING HISTORY: {:?}", &self.page);
            self.history.push(self.page.clone());
            self.set_page_inner(page);
        }
    }

    fn back(&mut self) {
        if let Some(page) = self.history.pop() {
            tracing::trace!("POPPING HISTORY: {:?}", &page);
            self.set_page_inner(page);
        } else {
            tracing::trace!("HISTORY STUCK ON NONE");
        }
    }

    fn set_page_inner(&mut self, page: Page) {
        // Setting the page often requires some associated actions:
        match &page {
            Page::Feed(FeedKind::General) => {
                GLOBALS.feed.set_feed_to_general();
                GLOBALS.events.clear_new();
            }
            Page::Feed(FeedKind::Replies) => {
                GLOBALS.feed.set_feed_to_replies();
                GLOBALS.events.clear_new();
            }
            Page::Feed(FeedKind::Thread { id, referenced_by }) => {
                GLOBALS.feed.set_feed_to_thread(*id, *referenced_by);
            }
            Page::Feed(FeedKind::Person(pubkey)) => {
                GLOBALS.feed.set_feed_to_person(pubkey.to_owned());
            }
            _ => {}
        }
        self.page = page;
    }
}

impl eframe::App for GossipUi {
    fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        let max_fps = GLOBALS.settings.blocking_read().max_fps as f32;

        // Wait until the next frame
        std::thread::sleep(self.next_frame - Instant::now());
        self.next_frame += Duration::from_secs_f32(1.0 / max_fps);

        // Redraw at least once per second
        ctx.request_repaint_after(Duration::from_secs(1));

        if GLOBALS
            .shutting_down
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            frame.close();
        }

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            ui.horizontal(|ui| {
                let back_label_text = RichText::new("‹ Back");
                let label = if self.history.is_empty() {
                    Label::new(back_label_text.weak())
                } else {
                    Label::new(back_label_text).sense(Sense::click())
                };
                if ui.add(label).clicked() {
                    self.back();
                }
                ui.separator();
                if ui
                    .add(SelectableLabel::new(
                        matches!(self.page, Page::Feed(_)),
                        "Feed",
                    ))
                    .clicked()
                {
                    self.set_page(Page::Feed(FeedKind::General));
                    GLOBALS.events.clear_new();
                }
                ui.separator();
                if ui
                    .add(SelectableLabel::new(
                        self.page == Page::PeopleList
                            || self.page == Page::PeopleFollow
                            || matches!(self.page, Page::Person(_)),
                        "People",
                    ))
                    .clicked()
                {
                    self.set_page(Page::PeopleList);
                }
                ui.separator();
                if ui
                    .add(SelectableLabel::new(
                        self.page == Page::YourKeys || self.page == Page::YourMetadata,
                        "You",
                    ))
                    .clicked()
                {
                    self.set_page(Page::YourKeys);
                }
                ui.separator();
                if ui
                    .add(SelectableLabel::new(self.page == Page::Relays, "Relays"))
                    .clicked()
                {
                    self.set_page(Page::Relays);
                }
                ui.separator();
                if ui
                    .add(SelectableLabel::new(
                        self.page == Page::Settings,
                        "Settings",
                    ))
                    .clicked()
                {
                    self.set_page(Page::Settings);
                }
                ui.separator();
                if ui
                    .add(SelectableLabel::new(
                        self.page == Page::HelpHelp
                            || self.page == Page::HelpStats
                            || self.page == Page::HelpAbout,
                        "Help",
                    ))
                    .clicked()
                {
                    self.set_page(Page::HelpHelp);
                }
                ui.separator();
            });
        });

        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .add(
                        Label::new(GLOBALS.status_message.blocking_read().clone())
                            .sense(Sense::click()),
                    )
                    .clicked()
                {
                    *GLOBALS.status_message.blocking_write() = "".to_string();
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| match self.page {
            Page::Feed(_) => feed::update(self, ctx, frame, ui),
            Page::PeopleList | Page::PeopleFollow | Page::Person(_) => {
                people::update(self, ctx, frame, ui)
            }
            Page::YourKeys | Page::YourMetadata => you::update(self, ctx, frame, ui),
            Page::Relays => relays::update(self, ctx, frame, ui),
            Page::Settings => settings::update(self, ctx, frame, ui),
            Page::HelpHelp | Page::HelpStats | Page::HelpAbout => {
                help::update(self, ctx, frame, ui)
            }
        });
    }
}

impl GossipUi {
    pub fn hex_pubkey_short(pubkeyhex: &PublicKeyHex) -> String {
        format!(
            "{}_{}...{}_{}",
            &pubkeyhex.0[0..4],
            &pubkeyhex.0[4..8],
            &pubkeyhex.0[56..60],
            &pubkeyhex.0[60..64]
        )
    }

    pub fn hex_id_short(idhex: &IdHex) -> String {
        idhex.0[0..8].to_string()
    }

    #[allow(dead_code)]
    pub fn pubkey_long(pubkey: &PublicKey) -> String {
        let hex: PublicKeyHex = (*pubkey).into();
        hex.0
    }

    pub fn render_person_name_line(ui: &mut Ui, maybe_person: Option<&DbPerson>) {
        ui.horizontal_wrapped(|ui| {
            if let Some(person) = maybe_person {
                if let Some(name) = person.name() {
                    ui.label(RichText::new(name).strong());
                } else {
                    ui.label(RichText::new(GossipUi::hex_pubkey_short(&person.pubkey)).weak());
                }

                if person.followed > 0 {
                    ui.label("🚶");
                }

                if let Some(mut nip05) = person.nip05().map(|s| s.to_owned()) {
                    if nip05.starts_with("_@") {
                        nip05 = nip05.get(2..).unwrap().to_string();
                    }

                    if person.nip05_valid > 0 {
                        ui.label(RichText::new(nip05).monospace().small());
                    } else {
                        ui.label(RichText::new(nip05).monospace().small().strikethrough());
                    }
                }

                ui.label(RichText::new("🔑").text_style(TextStyle::Small).weak());
                if ui
                    .add(CopyButton {})
                    .on_hover_text("Copy Public Key")
                    .clicked()
                {
                    ui.output().copied_text = person.pubkey.try_as_bech32_string().unwrap();
                }
            }
        });
    }

    pub fn try_get_avatar(
        &mut self,
        ctx: &Context,
        pubkeyhex: &PublicKeyHex,
    ) -> Option<TextureHandle> {
        // Do not keep retrying if failed
        if GLOBALS.failed_avatars.blocking_read().contains(pubkeyhex) {
            return None;
        }

        if let Some(th) = self.avatars.get(pubkeyhex) {
            return Some(th.to_owned());
        }

        match GLOBALS.people.get_avatar(pubkeyhex) {
            Err(_) => {
                GLOBALS
                    .failed_avatars
                    .blocking_write()
                    .insert(pubkeyhex.to_owned());
                None
            }
            Ok(Some(color_image)) => {
                let texture_handle =
                    ctx.load_texture(pubkeyhex.0.clone(), color_image, TextureOptions::default());
                self.avatars
                    .insert(pubkeyhex.to_owned(), texture_handle.clone());
                Some(texture_handle)
            }
            Ok(None) => None,
        }
    }
}
