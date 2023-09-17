use crate::comms::ToOverlordMessage;
use crate::globals::GLOBALS;
use crate::people::Person;
use crate::ui::wizard::WizardPage;
use crate::ui::{GossipUi, Page};
use eframe::egui;
use egui::{Context, Ui};
use nostr_types::Metadata;

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    let pubkey = match app.wizard_state.pubkey {
        Some(pk) => pk,
        None => {
            app.page = Page::Wizard(WizardPage::WelcomeGossip);
            return;
        }
    };

    let you = match GLOBALS.storage.read_person(&pubkey) {
        Ok(Some(person)) => person,
        _ => {
            GLOBALS.people.create_if_missing(pubkey);
            Person::new(pubkey)
        }
    };

    let mut existing_metadata = false;
    let metadata: Metadata = match &you.metadata {
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

    ui.horizontal(|ui| {
        ui.label("Name:");
        ui.add(text_edit_line!(app, app.wizard_state.metadata_name));
    });

    ui.add_space(15.0);
    ui.horizontal(|ui| {
        ui.label("About:");
        ui.add(text_edit_multiline!(app, app.wizard_state.metadata_about));
    });

    ui.add_space(15.0);
    ui.horizontal(|ui| {
        ui.label("Picture:");
        ui.add(text_edit_multiline!(app, app.wizard_state.metadata_picture));
    });

    ui.add_space(15.0);
    ui.add_space(20.0);
    if ui.button("  >  Save, Publish and Continue").clicked() {
        // Copy from form and save
        save_metadata(app, you.clone(), metadata.clone());

        // Publish
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::PushMetadata(metadata.clone()));

        app.page = Page::Wizard(WizardPage::FollowPeople);
    }

    ui.add_space(20.0);
    if ui
        .button("  >  Save and Continue without publishing")
        .clicked()
    {
        // Copy from form and save
        save_metadata(app, you.clone(), metadata.clone());

        app.page = Page::Wizard(WizardPage::FollowPeople);
    }

    ui.add_space(20.0);
    if ui
        .button("  >  Continue without saving or publishing")
        .clicked()
    {
        app.page = Page::Wizard(WizardPage::FollowPeople);
    }
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
    you.metadata = Some(metadata);
    let _ = GLOBALS.storage.write_person(&you, None);
}
