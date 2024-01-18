use crate::ui::{GossipUi, SettingsTab};
use eframe::egui;
use egui::{Align, Context, Layout, Ui};
use gossip_lib::Settings;

mod content;
mod database;
mod id;
mod network;
mod posting;
mod ui;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.add_space(10.0);
    ui.heading("Settings");

    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
        let stored_settings = Settings::load();
        if stored_settings != app.unsaved_settings {
            if ui.button("REVERT CHANGES").clicked() {
                app.unsaved_settings = Settings::load();

                // Fully revert any DPI changes
                match app.unsaved_settings.override_dpi {
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
                if stored_settings.override_dpi != app.unsaved_settings.override_dpi {
                    if let Some(value) = app.unsaved_settings.override_dpi {
                        let ppt: f32 = value as f32 / 72.0;
                        ctx.set_pixels_per_point(ppt);
                    }
                }

                // Save new original DPI value
                if let Some(value) = app.unsaved_settings.override_dpi {
                    app.original_dpi_value = value;
                }

                let _ = app.unsaved_settings.save();
            }
        }
    });

    ui.add_space(10.0);
    ui.separator();

    app.vert_scroll_area().id_source("settings").show(ui, |ui| {
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
            ui.selectable_value(&mut app.settings_tab, SettingsTab::Database, "Storage");
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
