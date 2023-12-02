use super::{GossipUi, Page};
use crate::ui::widgets;
use eframe::egui;
use egui::{Context, RichText, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::{Person, PersonList, GLOBALS};

pub(super) fn update(
    app: &mut GossipUi,
    ctx: &Context,
    _frame: &mut eframe::Frame,
    ui: &mut Ui,
    list: PersonList,
) {
    let people = {
        let members = GLOBALS.storage.get_people_in_list(list).unwrap_or_default();

        let mut people: Vec<(Person, bool)> = Vec::new();

        for (pk, public) in &members {
            if let Ok(Some(person)) = GLOBALS.storage.read_person(pk) {
                people.push((person, *public));
            } else {
                let person = Person::new(pk.to_owned());
                let _ = GLOBALS.storage.write_person(&person, None);
                people.push((person, *public));
            }
        }
        people.sort_by(|a, b| a.0.cmp(&b.0));
        people
    };

    ui.add_space(12.0);

    let latest_event_data = GLOBALS
        .people
        .latest_person_list_event_data
        .get(&list)
        .map(|v| v.value().clone())
        .unwrap_or_default();

    let mut asof = "unknown".to_owned();
    if let Ok(stamp) = time::OffsetDateTime::from_unix_timestamp(latest_event_data.when.0) {
        if let Ok(formatted) = stamp.format(time::macros::format_description!(
            "[year]-[month repr:short]-[day] ([weekday repr:short]) [hour]:[minute]"
        )) {
            asof = formatted;
        }
    }

    let txt = if let Some(private_len) = latest_event_data.private_len {
        format!(
            "REMOTE: {} (public_len={} private_len={})",
            asof, latest_event_data.public_len, private_len
        )
    } else {
        format!(
            "REMOTE: {} (public_len={})",
            asof, latest_event_data.public_len
        )
    };

    ui.label(RichText::new(txt).size(15.0))
        .on_hover_text("This is the data in the latest list event fetched from relays");

    ui.add_space(10.0);

    ui.horizontal(|ui| {
        ui.add_space(30.0);

        if ui
            .button("↓ Overwrite ↓")
            .on_hover_text(
                "This imports data from the latest event, erasing anything that is already here",
            )
            .clicked()
        {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::UpdatePersonList {
                    person_list: list,
                    merge: false,
                });
        }
        if ui
            .button("↓ Merge ↓")
            .on_hover_text(
                "This imports data from the latest event, merging it into what is already here",
            )
            .clicked()
        {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::UpdatePersonList {
                    person_list: list,
                    merge: true,
                });
        }

        if GLOBALS.signer.is_ready() {
            if ui
                .button("↑ Publish ↑")
                .on_hover_text("This publishes the list to your relays")
                .clicked()
            {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::PushPersonList(list));
            }
        }

        if GLOBALS.signer.is_ready() {
            if app.clear_list_needs_confirm {
                if ui.button("CANCEL").clicked() {
                    app.clear_list_needs_confirm = false;
                }
                if ui.button("YES, CLEAR ALL").clicked() {
                    let _ = GLOBALS
                        .to_overlord
                        .send(ToOverlordMessage::ClearPersonList(list));
                    app.clear_list_needs_confirm = false;
                }
            } else {
                if ui.button("Clear All").clicked() {
                    app.clear_list_needs_confirm = true;
                }
            }
        }
    });

    ui.add_space(10.0);

    let last_list_edit = match GLOBALS.storage.get_person_list_last_edit_time(list) {
        Ok(Some(date)) => date,
        Ok(None) => 0,
        Err(e) => {
            tracing::error!("{}", e);
            0
        }
    };

    let mut ledit = "unknown".to_owned();
    if let Ok(stamp) = time::OffsetDateTime::from_unix_timestamp(last_list_edit) {
        if let Ok(formatted) = stamp.format(time::macros::format_description!(
            "[year]-[month repr:short]-[day] ([weekday repr:short]) [hour]:[minute]"
        )) {
            ledit = formatted;
        }
    }
    ui.label(RichText::new(format!("LOCAL: {} (size={})", ledit, people.len())).size(15.0))
        .on_hover_text("This is the local (and effective) list");

    if !GLOBALS.signer.is_ready() {
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            ui.label("You need to ");
            if ui.link("setup your identity").clicked() {
                app.set_page(ctx, Page::YourKeys);
            }
            ui.label(" to push.");
        });
    }

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    ui.heading(format!("{} ({})", list.name(), people.len()));
    ui.add_space(14.0);

    app.vert_scroll_area().show(ui, |ui| {
        for (person, public) in people.iter() {
            ui.horizontal(|ui| {
                // Avatar first
                let avatar = if let Some(avatar) = app.try_get_avatar(ctx, &person.pubkey) {
                    avatar
                } else {
                    app.placeholder_avatar.clone()
                };
                if widgets::paint_avatar(ui, person, &avatar, widgets::AvatarSize::Feed).clicked() {
                    app.set_page(ctx, Page::Person(person.pubkey));
                };

                ui.vertical(|ui| {
                    ui.label(RichText::new(gossip_lib::names::pubkey_short(&person.pubkey)).weak());
                    GossipUi::render_person_name_line(app, ui, person, false);
                    if !GLOBALS
                        .storage
                        .have_persons_relays(person.pubkey)
                        .unwrap_or(false)
                    {
                        ui.label(
                            RichText::new("Relay list not found")
                                .color(app.theme.warning_marker_text_color()),
                        );
                    }

                    ui.horizontal(|ui| {
                        if crate::ui::components::switch_simple(ui, *public).clicked() {
                            let _ = GLOBALS.storage.add_person_to_list(
                                &person.pubkey,
                                list,
                                !*public,
                                None,
                            );
                        }
                        ui.label(if *public { "public" } else { "private" });
                    });
                });
            });

            if ui.button("Remove").clicked() {
                let _ = GLOBALS
                    .storage
                    .remove_person_from_list(&person.pubkey, list, None);
            }

            ui.add_space(4.0);
            ui.separator();
        }
    });
}
