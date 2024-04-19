use crate::ui::widgets::list_entry::{self};
use crate::ui::wizard::WizardPage;
use crate::ui::{GossipUi, Page};
use eframe::egui;
use egui::{Context, Ui};

use super::wizard_controls;
use super::wizard_state::WizardPath;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    if app.wizard_state.pubkey.is_some() {
        app.set_page(ctx, Page::Wizard(WizardPage::ReadNostrConfig));
        return;
    };

    let selected = app.wizard_state.path == WizardPath::ImportFromKey(true);
    let response = list_entry::make_frame(ui, None)
        .stroke(if selected {
            egui::Stroke::new(1.0, app.theme.accent_color())
        } else {
            egui::Stroke::new(1.0, egui::Color32::TRANSPARENT)
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.add(egui::RadioButton::new(selected, ""));
                ui.add_space(10.0);
                ui.label("Import a Private Key")
                    .on_hover_cursor(egui::CursorIcon::Default);
            })
        });
    if ui
        .interact(
            response.response.rect,
            ui.next_auto_id(),
            egui::Sense::click(),
        )
        .clicked()
    {
        app.wizard_state.path = WizardPath::ImportFromKey(true);
    }

    let selected = app.wizard_state.path == WizardPath::ImportFromKey(false);
    let response = list_entry::make_frame(ui, None)
        .stroke(if selected {
            egui::Stroke::new(1.0, app.theme.accent_color())
        } else {
            egui::Stroke::new(1.0, egui::Color32::TRANSPARENT)
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.add(egui::RadioButton::new(selected, ""));
                ui.add_space(10.0);
                ui.label("Import a Public Key")
                    .on_hover_cursor(egui::CursorIcon::Default);
            })
        });
    if ui
        .interact(
            response.response.rect,
            ui.next_auto_id(),
            egui::Sense::click(),
        )
        .clicked()
    {
        app.wizard_state.path = WizardPath::ImportFromKey(false);
    }

    ui.add_space(20.0);
    wizard_controls(
        ui,
        app,
        true,
        |app| {
            app.set_page(ctx, Page::Wizard(WizardPage::WelcomeGossip));
        },
        |app| match app.wizard_state.path {
            super::wizard_state::WizardPath::ImportFromKey(true) => {
                app.set_page(ctx, Page::Wizard(WizardPage::ImportPrivateKey));
            }
            super::wizard_state::WizardPath::ImportFromKey(false) => {
                app.set_page(ctx, Page::Wizard(WizardPage::ImportPublicKey));
            }
            _ => {}
        },
    );
}
