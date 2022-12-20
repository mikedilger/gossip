mod about;
mod feed;
mod people;
mod relays;
mod settings;
mod stats;
mod style;
mod you;

use crate::error::Error;
use eframe::{egui, IconData, Theme};
use egui::Context;

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
    People,
    You,
    Relays,
    Settings,
    Stats,
    About,
}

struct GossipUi {
    page: Page,
    initial_dark_mode_set: bool,
    fonts_installed: bool,

}

impl GossipUi {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        GossipUi {
            page: Page::Feed,
            initial_dark_mode_set: false,
            fonts_installed: false,
        }
    }
}

impl eframe::App for GossipUi {
    fn update(&mut self, ctx: &Context, frame: &mut eframe::Frame) {
        let darkmode: bool = ctx.style().visuals.dark_mode;

        if ! self.initial_dark_mode_set {
            if darkmode {
                ctx.set_visuals(style::dark_mode_visuals());
            } else {
                ctx.set_visuals(style::light_mode_visuals());
            };
            self.initial_dark_mode_set = true;
        }

        if ! self.fonts_installed {
            ctx.set_fonts(style::font_definitions());
            self.fonts_installed = true;
        }

        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.page, Page::Feed, "Feed");
                ui.separator();
                ui.selectable_value(&mut self.page, Page::People, "People");
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
            Page::People => people::update(self, ctx, frame, ui),
            Page::You => you::update(self, ctx, frame, ui),
            Page::Relays => relays::update(self, ctx, frame, ui),
            Page::Settings => settings::update(self, ctx, frame, ui, darkmode),
            Page::Stats => stats::update(self, ctx, frame, ui),
            Page::About => about::update(self, ctx, frame, ui),
        });
    }
}

