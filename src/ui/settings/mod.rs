use crate::settings::Settings;
use crate::ui::{GossipUi, SettingsTab};
use eframe::egui;
use egui::{Align, Context, Layout, ScrollArea, Ui, Vec2};

mod content;
mod database;
mod id;
mod network;
mod posting;
mod ui;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.heading("Settings");

    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
        let stored_settings = Settings::load();
        if stored_settings != app.settings {
            if ui.button("REVERT CHANGES").clicked() {
                app.settings = Settings::load();

                // Fully revert any DPI changes
                match app.settings.override_dpi {
                    Some(value) => {
                        app.override_dpi = true;
                        app.override_dpi_value = value;
                    }
                    None => {
                        app.override_dpi = false;
                        app.override_dpi_value = app.original_dpi_value;
                    }
                };
                let ppt: f32 = app.override_dpi_value as f32 / 72.0;
                ctx.set_pixels_per_point(ppt);
            }

            if ui.button("SAVE CHANGES").clicked() {
                // Apply DPI change
                if stored_settings.override_dpi != app.settings.override_dpi {
                    if let Some(value) = app.settings.override_dpi {
                        let ppt: f32 = value as f32 / 72.0;
                        ctx.set_pixels_per_point(ppt);
                    }
                }

                // Save new original DPI value
                if let Some(value) = app.settings.override_dpi {
                    app.original_dpi_value = value;
                }

                let _ = app.settings.save();
            }
        }
    });

    ui.add_space(10.0);
    ui.separator();

    ScrollArea::vertical()
        .id_source("settings")
        .override_scroll_delta(Vec2 {
            x: 0.0,
            y: app.current_scroll_offset,
        })
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut app.settings_tab, SettingsTab::Id, "Identity");
                ui.label("|");
                ui.selectable_value(&mut app.settings_tab, SettingsTab::Ui, "Ui");
                ui.label("|");
                ui.selectable_value(&mut app.settings_tab, SettingsTab::Content, "Content");
                ui.label("|");
                ui.selectable_value(&mut app.settings_tab, SettingsTab::Network, "Network");
                ui.label("|");
                ui.selectable_value(&mut app.settings_tab, SettingsTab::Posting, "Posting");
                ui.label("|");
                ui.selectable_value(&mut app.settings_tab, SettingsTab::Database, "Database");
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            match app.settings_tab {
                SettingsTab::Content => content::update(app, ctx, frame, ui),
                SettingsTab::Database => database::update(app, ctx, frame, ui),
                SettingsTab::Id => id::update(app, ctx, frame, ui),
                SettingsTab::Network => network::update(app, ctx, frame, ui),
                SettingsTab::Posting => posting::update(app, ctx, frame, ui),
                SettingsTab::Ui => ui::update(app, ctx, frame, ui),
            }
        });
}
