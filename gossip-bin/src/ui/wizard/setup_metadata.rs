use crate::ui::widgets::list_entry::OUTER_MARGIN_RIGHT;
use crate::ui::wizard::WizardPage;
use crate::ui::{GossipUi, Page, RichText};
use eframe::egui;
use egui::{Context, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::{Person, PersonTable, Table, GLOBALS};
use nostr_types::Metadata;

use super::continue_control;
use super::wizard_state::WizardPath;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    let pubkey = match app.wizard_state.pubkey {
        Some(pk) => pk,
        None => {
            app.set_page(ctx, Page::Wizard(WizardPage::WelcomeGossip));
            return;
        }
    };

    let you = match PersonTable::read_record(pubkey, None) {
        Ok(Some(person)) => person,
        _ => {
            GLOBALS.people.create_if_missing(pubkey);
            Person::new(pubkey)
        }
    };

    let mut existing_metadata = false;
    let metadata: Metadata = match you.metadata() {
        Some(m) => {
            existing_metadata = true;
            m.to_owned()
        }
        None => Metadata::new(),
    };

    // Copy existing metadata at most once
    if existing_metadata
        && !app.wizard_state.new_user
        && !app.wizard_state.metadata_events.is_empty()
        && !app.wizard_state.metadata_copied
    {
        if let Some(n) = &metadata.name {
            app.wizard_state.metadata_name = n.to_owned();
        }
        if let Some(n) = &metadata.about {
            app.wizard_state.metadata_about = n.to_owned();
        }
        if let Some(n) = &metadata.picture {
            app.wizard_state.metadata_picture = n.to_owned();
        }
        app.wizard_state.metadata_copied = true;
    }

    egui::Grid::new("grid")
        .num_columns(2)
        .striped(false)
        .spacing([10.0, 10.0])
        .show(ui, |ui| {
            ui.label("Name:");
            let response = text_edit_line!(app, app.wizard_state.metadata_name)
                .desired_width(400.0)
                .with_paste()
                .show(ui)
                .response;
            if response.changed() {
                app.wizard_state.error = None;
            }
            ui.end_row();

            ui.label("About:");
            let response = text_edit_line!(app, app.wizard_state.metadata_about)
                .desired_width(400.0)
                .with_paste()
                .show(ui)
                .response;
            if response.changed() {
                app.wizard_state.error = None;
            }

            ui.end_row();

            ui.label("Picture URL:");
            let response = text_edit_line!(app, app.wizard_state.metadata_picture)
                .desired_width(400.0)
                .with_paste()
                .show(ui)
                .response;
            if response.changed() {
                app.wizard_state.error = None;
            }

            ui.end_row();

            ui.label(""); // fill first cell
            if WizardPath::ImportFromKey(true) == app.wizard_state.path {
                if ui.button("Undo Changes").clicked() {
                    if let Some(n) = &metadata.name {
                        app.wizard_state.metadata_name = n.to_owned();
                    }
                    if let Some(n) = &metadata.about {
                        app.wizard_state.metadata_about = n.to_owned();
                    }
                    if let Some(n) = &metadata.picture {
                        app.wizard_state.metadata_picture = n.to_owned();
                    }
                }
            }

            ui.end_row();
        });

    // error block
    if let Some(err) = &app.wizard_state.error {
        ui.add_space(10.0);
        ui.label(RichText::new(err).color(app.theme.warning_marker_text_color()));
    }

    ui.add_space(20.0);

    if GLOBALS.identity.is_unlocked() {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::default()), |ui| {
            ui.add_space(OUTER_MARGIN_RIGHT);
            ui.checkbox(
                &mut app.wizard_state.metadata_should_publish,
                "Publish this Profile",
            );
        });
        ui.add_space(10.0);
    }
    continue_control(ui, app, true, |app| {
        // Copy from form and save
        save_metadata(app, you.clone(), metadata.clone());

        if app.wizard_state.metadata_should_publish {
            // Publish
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::PushMetadata(metadata.clone()));
        }

        app.set_page(ctx, Page::Wizard(WizardPage::FollowPeople));
    });
}

fn save_metadata(app: &mut GossipUi, mut you: Person, mut metadata: Metadata) {
    // Copy from form
    metadata.name = if !app.wizard_state.metadata_name.is_empty() {
        Some(app.wizard_state.metadata_name.clone())
    } else {
        None
    };

    metadata.about = if !app.wizard_state.metadata_about.is_empty() {
        Some(app.wizard_state.metadata_about.clone())
    } else {
        None
    };

    metadata.picture = if !app.wizard_state.metadata_picture.is_empty() {
        Some(app.wizard_state.metadata_picture.clone())
    } else {
        None
    };

    // Save to database
    *you.metadata_mut() = Some(metadata);
    let _ = PersonTable::write_record(&mut you, None);
}
