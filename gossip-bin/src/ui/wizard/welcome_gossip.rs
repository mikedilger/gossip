use eframe::egui;
use egui::{Context, Ui};
use gossip_lib::GLOBALS;

use super::continue_button;
use super::wizard_state::WizardPath;
use crate::ui::widgets::list_entry::{self, OUTER_MARGIN_RIGHT};
use crate::ui::wizard::WizardPage;
use crate::ui::{GossipUi, Page};

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    if app.wizard_state.pubkey.is_some() {
        app.set_page(ctx, Page::Wizard(WizardPage::ReadNostrConfig));
        return;
    };

    ui.add_space(10.0);
    ui.label("Gossip is a Nostr client for desktops");

    ui.add_space(10.0);
    ui.label("Please select from the following choices:");

    ui.add_space(20.0);

    render_wizard_path_choice(ui, app, WizardPath::CreateNewAccount);

    render_wizard_path_choice(ui, app, WizardPath::ImportFromKey(true));

    render_wizard_path_choice(ui, app, WizardPath::FollowOnlyNoKeys);

    ui.add_space(20.0); // vertical space
    ui.horizontal(|ui| {
        if ui.button("Exit Setup Wizard").clicked() {
            super::complete_wizard(app, ctx);
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::default()), |ui| {
            app.theme.primary_button_style(ui.style_mut());
            ui.add_space(OUTER_MARGIN_RIGHT);
            if ui.add(continue_button()).clicked() {
                match app.wizard_state.path {
                    WizardPath::CreateNewAccount => {
                        app.wizard_state.new_user = true;
                        app.set_page(ctx, Page::Wizard(WizardPage::WelcomeNostr));
                    }
                    WizardPath::ImportFromKey(_) => {
                        app.wizard_state.new_user = false;
                        app.set_page(ctx, Page::Wizard(WizardPage::ImportKeys))
                    }
                    WizardPath::FollowOnlyNoKeys => {
                        app.wizard_state.new_user = false;
                        app.wizard_state.follow_only = true;
                        let _ = GLOBALS.storage.set_flag_following_only(true, None);
                        app.set_page(ctx, Page::Wizard(WizardPage::FollowPeople));
                    }
                }
            }
        });
    });
}

fn wizard_path_name(path: WizardPath) -> &'static str {
    match path {
        WizardPath::CreateNewAccount => "Create a New Nostr Account",
        WizardPath::ImportFromKey(_) => "I Already have a Nostr Account",
        WizardPath::FollowOnlyNoKeys => "Just follow people (no account)",
    }
}

fn render_wizard_path_choice(ui: &mut Ui, app: &mut GossipUi, choice: WizardPath) {
    let selected = app.wizard_state.path == choice;
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
                ui.label(wizard_path_name(choice))
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
        app.wizard_state.path = choice;
    }
}
