use crate::comms::ToOverlordMessage;
use crate::GLOBALS;
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
        if let Ok(Some(stored_settings)) = GLOBALS.storage.read_settings() {
            if stored_settings != app.settings {
                if ui.button("REVERT CHANGES").clicked() {
                    app.settings = GLOBALS.settings.read().clone();
                }

                if ui.button("SAVE CHANGES").clicked() {
                    // Copy local settings to global settings
                    *GLOBALS.settings.write() = app.settings.clone();

                    // Tell the overlord to save them
                    let _ = GLOBALS.to_overlord.send(ToOverlordMessage::SaveSettings);
                }
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
