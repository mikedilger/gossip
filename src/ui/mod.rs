mod about;
mod feed;
mod people;
mod relays;
mod settings;
mod stats;
mod style;
mod you;

use crate::about::About;
use crate::error::Error;
use crate::globals::GLOBALS;
use crate::settings::Settings;
use eframe::{egui, IconData, Theme};
use egui::{ColorImage, Context, ImageData, TextureHandle, TextureOptions};
use nostr_types::{PublicKey, PublicKeyHex};
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
    You,
    Relays,
    Settings,
    Stats,
    About,
}

struct GossipUi {
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
        }
    }
}

impl eframe::App for GossipUi {
    fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
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
                ui.selectable_value(&mut self.page, Page::About, "About");
                ui.separator();
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| match self.page {
            Page::Feed => feed::update(self, ctx, frame, ui),
            Page::PeopleList => people::update(self, ctx, frame, ui),
            Page::PeopleFollow => people::update(self, ctx, frame, ui),
            Page::You => you::update(self, ctx, frame, ui),
            Page::Relays => relays::update(self, ctx, frame, ui),
            Page::Settings => settings::update(self, ctx, frame, ui, darkmode),
            Page::Stats => stats::update(self, ctx, frame, ui),
            Page::About => about::update(self, ctx, frame, ui),
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

    pub fn pubkey_short(pubkey: &PublicKey) -> String {
        let hex: PublicKeyHex = (*pubkey).into();
        Self::hex_pubkey_short(&hex)
    }
}
