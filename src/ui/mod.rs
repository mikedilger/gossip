mod feed;
mod help;
mod people;
mod relays;
mod settings;
pub(crate) mod theme;
mod widgets;
mod you;

use crate::about::About;
use crate::comms::ToOverlordMessage;
use crate::error::Error;
use crate::feed::FeedKind;
use crate::globals::GLOBALS;
use crate::people::DbPerson;
use crate::settings::Settings;
use crate::ui::widgets::CopyButton;
use eframe::{egui, IconData};
use egui::{
    Color32, ColorImage, Context, Image, ImageData, Label, RichText, SelectableLabel, Sense,
    TextStyle, TextureHandle, TextureOptions, Ui, Vec2,
};
use nostr_types::{Id, IdHex, Metadata, PublicKey, PublicKeyHex};
use std::collections::{HashMap, HashSet};
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
        default_theme: eframe::Theme::Light,
        icon_data: Some(IconData {
            rgba: icon.into_raw(),
            width: icon_width,
            height: icon_height,
        }),
        initial_window_size: Some(egui::vec2(700.0, 900.0)),
        resizable: true,
        centered: true,
        vsync: true,
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        "gossip",
        options,
        Box::new(|cc| Box::new(GossipUi::new(cc))),
    ) {
        tracing::error!("Eframe error: {}", e);
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
enum Page {
    Feed(FeedKind),
    PeopleList,
    PeopleFollow,
    PeopleMuted,
    Person(PublicKeyHex),
    YourKeys,
    YourMetadata,
    RelaysLive,
    RelaysAll,
    Settings,
    HelpHelp,
    HelpStats,
    HelpAbout,
}

struct GossipUi {
    // Rendering
    next_frame: Instant,
    override_dpi: bool,
    override_dpi_value: u32,
    current_scroll_offset: f32,
    future_scroll_offset: f32,

    // QR codes being rendered (in feed or elsewhere)
    // the f32's are the recommended image size
    qr_codes: HashMap<String, Result<(TextureHandle, f32, f32), Error>>,

    // Post rendering
    render_raw: Option<Id>,
    render_qr: Option<Id>,
    viewed: HashSet<Id>,
    approved: HashSet<Id>, // content warning posts
    height: HashMap<Id, f32>,

    // Person page rendering ('npub', 'nprofile', or 'lud06')
    person_qr: Option<&'static str>,
    setting_active_person: bool,

    // Page
    page: Page,
    history: Vec<Page>,

    // General Data
    about: About,
    icon: TextureHandle,
    placeholder_avatar: TextureHandle,
    settings: Settings,
    avatars: HashMap<PublicKeyHex, TextureHandle>,
    tag_re: regex::Regex,

    // User entry: posts
    draft: String,
    tag_someone: String,
    include_subject: bool,
    subject: String,
    include_content_warning: bool,
    content_warning: String,
    replying_to: Option<Id>,

    // User entry: metadata
    editing_metadata: bool,
    metadata: Metadata,

    // User entry: general
    nprofile_follow: String,
    nip05follow: String,
    follow_pubkey: String,
    follow_pubkey_at_relay: String,
    password: String,
    password2: String,
    password3: String,
    delete_confirm: bool,
    new_metadata_fieldname: String,
    import_priv: String,
    import_pub: String,
    new_relay_url: String,
}

impl Drop for GossipUi {
    fn drop(&mut self) {
        self.password.zeroize();
        self.password2.zeroize();
        self.password3.zeroize();
    }
}

impl GossipUi {
    fn new(cctx: &eframe::CreationContext<'_>) -> Self {
        let settings = GLOBALS.settings.read().clone();

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

        {
            cctx.egui_ctx.tessellation_options_mut(|to| {
                // Less feathering
                to.feathering = true;
                to.feathering_size_in_pixels = 0.667;

                // Sharper text
                to.round_text_to_pixels = true;
            });
        }

        if !settings.light_mode {
            cctx.egui_ctx.set_style(theme::current_theme().dark_mode())
        } else {
            cctx.egui_ctx.set_style(theme::current_theme().light_mode())
        };

        cctx.egui_ctx.set_fonts(theme::current_theme().font_definitions());

        let mut style: egui::Style = (*cctx.egui_ctx.style()).clone();
        style.text_styles = theme::current_theme().text_styles();
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

        let start_page = if GLOBALS.first_run.load(Ordering::Relaxed) {
            Page::HelpHelp
        } else {
            Page::Feed(FeedKind::General)
        };

        GossipUi {
            next_frame: Instant::now(),
            override_dpi,
            override_dpi_value,
            current_scroll_offset: 0.0,
            future_scroll_offset: 0.0,
            qr_codes: HashMap::new(),
            render_raw: None,
            render_qr: None,
            viewed: HashSet::new(),
            approved: HashSet::new(),
            height: HashMap::new(),
            person_qr: None,
            setting_active_person: false,
            page: start_page,
            history: vec![],
            about: crate::about::about(),
            icon: icon_texture_handle,
            placeholder_avatar: placeholder_avatar_texture_handle,
            settings,
            avatars: HashMap::new(),
            tag_re: regex::Regex::new(r"(\#\[\d+\])").unwrap(),
            draft: "".to_owned(),
            tag_someone: "".to_owned(),
            include_subject: false,
            subject: "".to_owned(),
            include_content_warning: false,
            content_warning: "".to_owned(),
            replying_to: None,
            editing_metadata: false,
            metadata: Metadata::new(),
            nprofile_follow: "".to_owned(),
            nip05follow: "".to_owned(),
            follow_pubkey: "".to_owned(),
            follow_pubkey_at_relay: "".to_owned(),
            password: "".to_owned(),
            password2: "".to_owned(),
            password3: "".to_owned(),
            delete_confirm: false,
            new_metadata_fieldname: String::new(),
            import_priv: "".to_owned(),
            import_pub: "".to_owned(),
            new_relay_url: "".to_owned(),
        }
    }

    fn set_page(&mut self, page: Page) {
        if self.page != page {
            tracing::trace!("PUSHING HISTORY: {:?}", &self.page);
            self.history.push(self.page.clone());
            self.set_page_inner(page);

            // Clear QR codes on page switches
            self.qr_codes.clear();
            self.render_qr = None;
            self.person_qr = None;
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
            }
            Page::Feed(FeedKind::Replies) => {
                GLOBALS.feed.set_feed_to_replies();
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

    fn clear_post(&mut self) {
        self.draft = "".to_owned();
        self.tag_someone = "".to_owned();
        self.include_subject = false;
        self.subject = "".to_owned();
        self.replying_to = None;
        self.include_content_warning = false;
        self.content_warning = "".to_owned();
    }
}

impl eframe::App for GossipUi {
    fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        let max_fps = GLOBALS.settings.read().max_fps as f32;

        if self.future_scroll_offset != 0.0 {
            ctx.request_repaint();
        } else {
            // Wait until the next frame
            std::thread::sleep(self.next_frame - Instant::now());
            self.next_frame += Duration::from_secs_f32(1.0 / max_fps);

            // Redraw at least once per second
            ctx.request_repaint_after(Duration::from_secs(1));
        }

        if GLOBALS.shutting_down.load(Ordering::Relaxed) {
            frame.close();
        }

        // Smooth Scrolling
        {
            // Add the amount of scroll requested to the future
            let mut requested_scroll: f32 = 0.0;
            ctx.input_mut(|i| {
                requested_scroll = i.scroll_delta.y;
            });
            self.future_scroll_offset += requested_scroll;

            // Move by 10% of future scroll offsets
            self.current_scroll_offset = 0.1 * self.future_scroll_offset;
            self.future_scroll_offset -= self.current_scroll_offset;

            // Friction stop when slow enough
            if self.future_scroll_offset < 1.0 && self.future_scroll_offset > -1.0 {
                self.future_scroll_offset = 0.0;
            }
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
                }
                ui.separator();
                if ui
                    .add(SelectableLabel::new(
                        self.page == Page::PeopleList
                            || self.page == Page::PeopleFollow
                            || self.page == Page::PeopleMuted
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
                    .add(SelectableLabel::new(
                        self.page == Page::RelaysLive || self.page == Page::RelaysAll,
                        "Relays",
                    ))
                    .clicked()
                {
                    self.set_page(Page::RelaysLive);
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
            Page::PeopleList | Page::PeopleFollow | Page::PeopleMuted | Page::Person(_) => {
                people::update(self, ctx, frame, ui)
            }
            Page::YourKeys | Page::YourMetadata => you::update(self, ctx, frame, ui),
            Page::RelaysLive | Page::RelaysAll => relays::update(self, ctx, frame, ui),
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
            &pubkeyhex.as_str()[0..4],
            &pubkeyhex.as_str()[4..8],
            &pubkeyhex.as_str()[56..60],
            &pubkeyhex.as_str()[60..64],
        )
    }

    pub fn pubkey_short(pubkeyhex: &PublicKeyHex) -> String {
        match PublicKey::try_from_hex_string(pubkeyhex) {
            Err(_) => GossipUi::hex_pubkey_short(pubkeyhex),
            Ok(pk) => match pk.try_as_bech32_string() {
                Err(_) => GossipUi::hex_pubkey_short(pubkeyhex),
                Ok(npub) => format!("{}…", &npub.get(0..20).unwrap_or("????????????????????")),
            },
        }
    }

    pub fn hex_id_short(idhex: &IdHex) -> String {
        idhex.as_str()[0..8].to_string()
    }

    pub fn render_person_name_line(app: &mut GossipUi, ui: &mut Ui, person: &DbPerson) {
        // Let the 'People' manager know that we are interested in displaying this person.
        // It will make sure metadata is eventually available if
        // settings.automatically_fetch_metadata is enabled
        if person.metadata_at.is_none() {
            GLOBALS.people.person_of_interest(person.pubkey.clone());
        }

        ui.horizontal_wrapped(|ui| {
            let name = if let Some(name) = person.display_name() {
                name.to_owned()
            } else {
                GossipUi::pubkey_short(&person.pubkey)
            };

            ui.menu_button(&name, |ui| {
                if ui.button("Mute").clicked() {
                    GLOBALS.people.mute(&person.pubkey, true);
                }
                if person.followed == 0 && ui.button("Follow").clicked() {
                    GLOBALS.people.follow(&person.pubkey, true);
                } else if person.followed == 1 && ui.button("Unfollow").clicked() {
                    GLOBALS.people.follow(&person.pubkey, false);
                }
                if ui.button("Update Metadata").clicked() {
                    let _ = GLOBALS
                        .to_overlord
                        .send(ToOverlordMessage::UpdateMetadata(person.pubkey.clone()));
                }
                if ui.button("View Their Posts").clicked() {
                    app.set_page(Page::Feed(FeedKind::Person(person.pubkey.clone())));
                }
            });

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
                ui.output_mut(|o| o.copied_text = person.pubkey.try_as_bech32_string().unwrap());
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
                let texture_handle = ctx.load_texture(
                    pubkeyhex.to_string(),
                    color_image,
                    TextureOptions::default(),
                );
                self.avatars
                    .insert(pubkeyhex.to_owned(), texture_handle.clone());
                Some(texture_handle)
            }
            Ok(None) => None,
        }
    }

    pub fn render_qr(&mut self, ui: &mut Ui, ctx: &Context, key: &str, content: &str) {
        // Remember the UI runs this every frame.  We do NOT want to load the texture to the GPU
        // every frame, so we remember the texture handle in app.qr_codes, and only load to the GPU
        // if we don't have it yet.  We also remember if there was an error and don't try again.
        match self.qr_codes.get(key) {
            Some(Ok((texture_handle, x, y))) => {
                ui.add(Image::new(texture_handle, Vec2 { x: *x, y: *y }));
            }
            Some(Err(error)) => {
                ui.label(
                    RichText::new(format!("CANNOT LOAD QR: {}", error))
                        .color(Color32::from_rgb(160, 0, 0)),
                );
            }
            None => {
                // need bytes
                if let Ok(code) = qrcode::QrCode::new(content) {
                    let image = code.render::<image::Rgba<u8>>().build();

                    let color_image = ColorImage::from_rgba_unmultiplied(
                        [image.width() as usize, image.height() as usize],
                        image.as_flat_samples().as_slice(),
                    );

                    let texture_handle =
                        ctx.load_texture(key, color_image, TextureOptions::default());

                    // Convert image size into points for later rendering (so that it renders with
                    // the number of pixels recommended by the qrcode library)
                    let ppp = ctx.pixels_per_point();

                    self.qr_codes.insert(
                        key.to_string(),
                        Ok((
                            texture_handle,
                            image.width() as f32 / ppp,
                            image.height() as f32 / ppp,
                        )),
                    );
                } else {
                    self.qr_codes.insert(
                        key.to_string(),
                        Err(Error::General("Could not make a QR".to_string())),
                    );
                }
            }
        }
    }
}
