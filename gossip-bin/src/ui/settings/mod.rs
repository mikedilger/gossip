use crate::ui::{GossipUi, SettingsTab};
use crate::unsaved_settings::UnsavedSettings;
use eframe::egui;
use egui::{Align, Context, Layout, Ui};

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
        let stored_settings = UnsavedSettings::load();
        if stored_settings != app.unsaved_settings {
            if ui.button("REVERT CHANGES").clicked() {
                app.unsaved_settings = UnsavedSettings::load();

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
                let mut dpi_changed = false;

                // Apply DPI change
                if stored_settings.override_dpi != app.unsaved_settings.override_dpi {
                    if let Some(value) = app.unsaved_settings.override_dpi {
                        let ppt: f32 = value as f32 / 72.0;
                        ctx.set_pixels_per_point(ppt);
                        dpi_changed = true;
                    }
                }

                // restore native if not overriding
                // this can now be done with the new 'zoom_factor' egui setting
                if !app.override_dpi {
                    ctx.set_zoom_factor(1.0);
                    dpi_changed = true;
                }

                if dpi_changed {
                    app.init_scaling(ctx);
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
