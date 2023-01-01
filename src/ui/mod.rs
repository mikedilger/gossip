mod feed;
mod help;
mod people;
mod relays;
mod settings;
mod stats;
mod style;
mod widgets;
mod you;

use crate::about::About;
use crate::db::DbPerson;
use crate::error::Error;
use crate::globals::GLOBALS;
use crate::settings::Settings;
use crate::ui::widgets::CopyButton;
use eframe::{egui, IconData, Theme};
use egui::{
    ColorImage, Context, ImageData, RichText, TextStyle, TextureHandle, TextureOptions, Ui,
};
use nostr_types::{Id, PublicKey, PublicKeyHex};
use std::collections::{HashMap, HashSet};
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

#[derive(PartialEq)]
enum Page {
    Feed,
    PeopleFollow,
    PeopleList,
    Person,
    You,
    Relays,
    Settings,
    Stats,
    HelpHelp,
    HelpAbout,
}

struct GossipUi {
    next_frame: Instant,
    page: Page,
    about: About,
    icon: TextureHandle,
    placeholder_avatar: TextureHandle,
    draft: String,
    settings: Settings,
    nip35follow: String,
    follow_bech32_pubkey: String,
    follow_hex_pubkey: String,
    follow_pubkey_at_relay: String,
    password: String,
    import_bech32: String,
    import_hex: String,
    replying_to: Option<Id>,
    hides: Vec<Id>,
    person_view_pubkey: Option<PublicKeyHex>,
    person_view_person: Option<DbPerson>,
    person_view_name: Option<String>,
    avatars: HashMap<PublicKeyHex, TextureHandle>,
    failed_avatars: HashSet<PublicKeyHex>,
}

impl Drop for GossipUi {
    fn drop(&mut self) {
        self.password.zeroize();
    }
}

impl GossipUi {
    fn new(cctx: &eframe::CreationContext<'_>) -> Self {
        if cctx.egui_ctx.style().visuals.dark_mode {
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

        let settings = GLOBALS.settings.blocking_read().clone();

        GossipUi {
            next_frame: Instant::now(),
            page: Page::Feed,
            about: crate::about::about(),
            icon: icon_texture_handle,
            placeholder_avatar: placeholder_avatar_texture_handle,
            draft: "".to_owned(),
            settings,
            nip35follow: "".to_owned(),
            follow_bech32_pubkey: "".to_owned(),
            follow_hex_pubkey: "".to_owned(),
            follow_pubkey_at_relay: "".to_owned(),
            password: "".to_owned(),
            import_bech32: "".to_owned(),
            import_hex: "".to_owned(),
            replying_to: None,
            hides: Vec::new(),
            person_view_pubkey: None,
            person_view_person: None,
            person_view_name: None,
            avatars: HashMap::new(),
            failed_avatars: HashSet::new(),
        }
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

        let darkmode: bool = ctx.style().visuals.dark_mode;

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.page, Page::Feed, "Feed");
                ui.separator();
                ui.selectable_value(&mut self.page, Page::PeopleList, "People");
                ui.separator();
                ui.selectable_value(&mut self.page, Page::You, "You");
                ui.separator();
                ui.selectable_value(&mut self.page, Page::Relays, "Relays");
                ui.separator();
                ui.selectable_value(&mut self.page, Page::Settings, "Settings");
                ui.separator();
                ui.selectable_value(&mut self.page, Page::Stats, "Stats");
                ui.separator();
                ui.selectable_value(&mut self.page, Page::HelpHelp, "Help");
                ui.separator();
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| match self.page {
            Page::Feed => feed::update(self, ctx, frame, ui),
            Page::PeopleList => people::update(self, ctx, frame, ui),
            Page::PeopleFollow => people::update(self, ctx, frame, ui),
            Page::Person => people::update(self, ctx, frame, ui),
            Page::You => you::update(self, ctx, frame, ui),
            Page::Relays => relays::update(self, ctx, frame, ui),
            Page::Settings => settings::update(self, ctx, frame, ui, darkmode),
            Page::Stats => stats::update(self, ctx, frame, ui),
            Page::HelpHelp => help::update(self, ctx, frame, ui),
            Page::HelpAbout => help::update(self, ctx, frame, ui),
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

    #[allow(dead_code)]
    pub fn pubkey_long(pubkey: &PublicKey) -> String {
        let hex: PublicKeyHex = (*pubkey).into();
        hex.0
    }

    pub fn render_person_name_line(ui: &mut Ui, maybe_person: Option<&DbPerson>) {
        ui.horizontal(|ui| {
            if let Some(person) = maybe_person {
                if let Some(name) = &person.name {
                    ui.label(RichText::new(name).strong());
                } else {
                    ui.label(RichText::new(GossipUi::hex_pubkey_short(&person.pubkey)).weak());
                }

                if person.followed > 0 {
                    ui.label("🚶");
                }

                if let Some(dns_id) = &person.dns_id {
                    if person.dns_id_valid > 0 {
                        ui.label(RichText::new(dns_id).monospace().small());
                    } else {
                        ui.label(RichText::new(dns_id).monospace().small().strikethrough());
                    }
                }
                ui.label(RichText::new("🔑").text_style(TextStyle::Small).weak());
                if ui.add(CopyButton {}).clicked() {
                    ui.output().copied_text = person.pubkey.0.to_owned();
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
        if self.failed_avatars.contains(pubkeyhex) {
            return None;
        }

        if let Some(th) = self.avatars.get(pubkeyhex) {
            return Some(th.to_owned());
        }

        match GLOBALS.people.blocking_write().get_avatar(pubkeyhex) {
            Err(_) => {
                self.failed_avatars.insert(pubkeyhex.to_owned());
                None
            }
            Ok(Some(rgbaimage)) => {
                let size = [rgbaimage.width() as _, rgbaimage.height() as _];
                let pixels = rgbaimage.as_flat_samples();
                let texture_handle = ctx.load_texture(
                    pubkeyhex.0.clone(),
                    ImageData::Color(ColorImage::from_rgba_unmultiplied(size, pixels.as_slice())),
                    TextureOptions::default(),
                );
                self.avatars
                    .insert(pubkeyhex.to_owned(), texture_handle.clone());
                Some(texture_handle)
            }
            Ok(None) => None,
        }
    }
}
