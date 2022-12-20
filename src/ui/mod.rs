use crate::error::Error;
use eframe::{egui, IconData, Theme};
use egui::style::Style;

pub fn run() -> Result<(), Error> {
    let icon_bytes = include_bytes!("../../gossip.png");
    let icon = image::load_from_memory(icon_bytes)?.to_rgba8();
    let (icon_width, icon_height) = icon.dimensions();

    let options = eframe::NativeOptions {
        decorated: true,
        default_theme: Theme::Light,
        icon_data: Some(IconData {
            rgba: icon.into_raw(),
            width: icon_width,
            height: icon_height,
        }),
        initial_window_size: Some(egui::vec2(700.0, 900.0)),
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        "Gossip",
        options,
        Box::new(|_cc| Box::new(GossipUi::default())),
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
}

impl Default for GossipUi {
    fn default() -> Self {
        Self { page: Page::Feed }
    }
}

impl eframe::App for GossipUi {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            //ui.heading("Gossip");

            ui.horizontal(|ui| {
                // light-dark switcher
                let style: Style = (*ui.ctx().style()).clone();
                let new_visuals = style.visuals.light_dark_small_toggle_button(ui);
                if let Some(visuals) = new_visuals {
                    ui.ctx().set_visuals(visuals);
                }

                ui.selectable_value(&mut self.page, Page::Feed, "Feed");
                ui.selectable_value(&mut self.page, Page::People, "People");
                ui.selectable_value(&mut self.page, Page::You, "You");
                ui.selectable_value(&mut self.page, Page::Relays, "Relays");
                ui.selectable_value(&mut self.page, Page::Settings, "Settings");
                ui.selectable_value(&mut self.page, Page::Stats, "Stats");
                ui.selectable_value(&mut self.page, Page::About, "About");
            });

            ui.label("Hello World".to_string());
        });
    }
}
