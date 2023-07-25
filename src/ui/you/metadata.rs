use super::{GossipUi, Page};
use crate::comms::ToOverlordMessage;
use crate::db::Relay;
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{Align, Color32, Context, Layout, RichText, TextEdit, Ui};
use nostr_types::Metadata;
use serde_json::map::Map;
use serde_json::value::Value;

lazy_static! {
    pub static ref EMPTY_METADATA: Metadata = Metadata::new();
}

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.add_space(24.0);

    let public_key = match GLOBALS.signer.public_key() {
        Some(pk) => pk,
        None => {
            ui.horizontal(|ui| {
                ui.label("You need to");
                if ui.link("setup an identity").clicked() {
                    app.set_page(Page::YourKeys);
                }
                ui.label("to have metadata.");
            });
            return;
        }
    };

    let you = match GLOBALS.storage.read_person(&public_key) {
        Ok(Some(dbp)) => dbp,
        _ => {
            ui.label("I cannot find you.");
            GLOBALS.people.create_if_missing(public_key);
            return;
        }
    };

    let view_metadata: &Metadata = match &you.metadata {
        Some(m) => m,
        None => &EMPTY_METADATA,
    };

    if let Some(metadata_created_at) = you.metadata_created_at {
        if let Ok(stamp) = time::OffsetDateTime::from_unix_timestamp(metadata_created_at) {
            if let Ok(formatted) = stamp.format(&time::format_description::well_known::Rfc2822) {
                ui.label(format!("Date Stamp of Fetched Metadata is {}", formatted));
                ui.add_space(18.0);
            }
        }
    }

    let edit_color = app.settings.theme.input_text_color();
    if app.editing_metadata {
        edit_line(ui, "Name", &mut app.metadata.name, edit_color);
        ui.add_space(18.0);
        edit_line(ui, "About", &mut app.metadata.about, edit_color);
        ui.add_space(18.0);
        edit_line(ui, "Picture", &mut app.metadata.picture, edit_color);
        ui.add_space(18.0);
        edit_line(ui, "NIP-05", &mut app.metadata.nip05, edit_color);
        ui.add_space(18.0);
        edit_lines_other(ui, &mut app.metadata.other, edit_color);
        ui.add_space(18.0);
    } else {
        view_line(ui, "Name", view_metadata.name.as_ref());
        ui.add_space(18.0);
        view_line(ui, "About", view_metadata.about.as_ref());
        ui.add_space(18.0);
        view_line(ui, "Picture", view_metadata.picture.as_ref());
        ui.add_space(18.0);
        view_line(ui, "NIP-05", view_metadata.nip05.as_ref());
        ui.add_space(18.0);
        view_lines_other(ui, &view_metadata.other);
        ui.add_space(18.0);
    }

    ui.with_layout(Layout::top_down(Align::Center), |ui| {
        if app.editing_metadata {
            ui.horizontal(|ui| {
                ui.label("Add new field: ");
                ui.add(text_edit_line!(app, app.new_metadata_fieldname).desired_width(120.0));
                if ui.button("ADD").clicked() {
                    app.metadata.other.insert(
                        app.new_metadata_fieldname.clone(),
                        Value::String("".to_owned()),
                    );
                    app.new_metadata_fieldname = "".to_owned();
                }
            });
        }

        ui.horizontal(|ui| {
            if app.editing_metadata {
                if ui.button("CANCEL (revert)").clicked() {
                    app.editing_metadata = false;
                    // revert any changes:
                    app.metadata = match &you.metadata {
                        Some(m) => m.to_owned(),
                        None => Metadata::new(),
                    };
                }
                if ui
                    .button("SAVE")
                    .on_hover_text("Finishes editing, but does not push.")
                    .clicked()
                {
                    app.editing_metadata = false;
                    let _ = GLOBALS
                        .to_overlord
                        .send(ToOverlordMessage::PushMetadata(app.metadata.clone()));
                }
            } else if !GLOBALS.signer.is_ready() {
                ui.horizontal(|ui| {
                    ui.label("You need to");
                    if ui.link("unlock your private key").clicked() {
                        app.set_page(Page::YourKeys);
                    }
                    ui.label("to edit/save metadata.");
                });
            } else if !GLOBALS
                .storage
                .filter_relays(|r| r.has_usage_bits(Relay::WRITE))
                .unwrap_or(vec![])
                .is_empty()
            {
                ui.horizontal(|ui| {
                    ui.label("You need to");
                    if ui.link("configure write relays").clicked() {
                        app.set_page(Page::RelaysAll);
                    }
                    ui.label("to edit/save metadata.");
                });
            } else if ui.button("EDIT").clicked() {
                app.editing_metadata = true;
                app.metadata = view_metadata.to_owned();
            }
        });
    });
}

fn view_line(ui: &mut Ui, field: &str, data: Option<&String>) {
    ui.horizontal(|ui| {
        ui.label(&format!("{}: ", field));
        if let Some(value) = data {
            ui.label(value);
        } else {
            ui.label(RichText::new("none").italics().weak());
        }
    });
}

fn edit_line(ui: &mut Ui, field: &str, data: &mut Option<String>, edit_color: Color32) {
    ui.horizontal(|ui| {
        ui.label(&format!("{}: ", field));
        ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
            if data.is_some() {
                if ui.button("Remove").clicked() {
                    *data = None;
                } else if field == "About" {
                    ui.add(
                        TextEdit::multiline(data.as_mut().unwrap())
                            .text_color(edit_color)
                            .desired_width(f32::INFINITY),
                    );
                } else {
                    ui.add(
                        TextEdit::singleline(data.as_mut().unwrap())
                            .text_color(edit_color)
                            .desired_width(f32::INFINITY),
                    );
                }
            } else if ui.button("Add").clicked() {
                *data = Some("".to_owned());
            }
        });
    });
}

fn view_lines_other(ui: &mut Ui, other: &Map<String, Value>) {
    for (field, jsonvalue) in other.iter() {
        ui.horizontal(|ui| {
            ui.label(&format!("{}: ", field));
            if let Value::String(s) = jsonvalue {
                ui.label(s.to_owned());
            } else if let Ok(s) = serde_json::to_string(&jsonvalue) {
                ui.label(s);
            } else {
                ui.label(RichText::new("unable to render").italics().weak());
            }
        });
        ui.add_space(18.0);
    }
}

fn edit_lines_other(ui: &mut Ui, other: &mut Map<String, Value>, edit_color: Color32) {
    let mut to_remove: Vec<String> = Vec::new();
    for (field, jsonvalue) in other.iter_mut() {
        ui.horizontal(|ui| {
            ui.label(&format!("{}: ", field));
            if let Value::String(s) = jsonvalue {
                ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                    if ui.button("Remove").clicked() {
                        to_remove.push(field.to_owned());
                    }
                    ui.add(
                        TextEdit::singleline(s)
                            .text_color(edit_color)
                            .desired_width(f32::INFINITY),
                    );
                });
            }
        });
        ui.add_space(18.0);
    }
    for rem in to_remove.iter() {
        other.remove(rem);
    }
}
