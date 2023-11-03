use super::{GossipUi, Page};
use crate::ui::widgets;
use eframe::egui;
use egui::{Context, RichText, Ui};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::{Person, PersonList, GLOBALS};

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    let muted_pubkeys = GLOBALS
        .storage
        .get_people_in_list(PersonList::Muted, None)
        .unwrap_or(vec![]);

    let mut people: Vec<Person> = Vec::new();
    for pk in &muted_pubkeys {
        if let Ok(Some(person)) = GLOBALS.storage.read_person(pk) {
            people.push(person);
        } else {
            let person = Person::new(pk.to_owned());
            let _ = GLOBALS.storage.write_person(&person, None);
            people.push(person);
        }
    }
    people.sort();

    ui.add_space(12.0);

    let latest_event_data = GLOBALS
        .people
        .latest_person_list_event_data
        .get(&PersonList::Muted)
        .map(|v| v.value().clone())
        .unwrap_or(Default::default());

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
        .on_hover_text("This is the data in the latest MuteList event fetched from relays");

    ui.add_space(10.0);

    ui.horizontal(|ui| {
        ui.add_space(30.0);

        if ui
            .button("↓ Overwrite ↓")
            .on_hover_text("This pulls down your Mute List, erasing anything that is already here")
            .clicked()
        {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::UpdatePersonList {
                    person_list: PersonList::Muted,
                    merge: false,
                });
        }
        if ui
            .button("↓ Merge ↓")
            .on_hover_text("This pulls down your Mute List, merging it into what is already here")
            .clicked()
        {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::UpdatePersonList {
                    person_list: PersonList::Muted,
                    merge: true,
                });
        }

        if GLOBALS.signer.is_ready() {
            if ui
                .button("↑ Publish ↑")
                .on_hover_text("This publishes your Mute List")
                .clicked()
            {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::PushPersonList(PersonList::Muted));
            }
        }

        if GLOBALS.signer.is_ready() {
            if app.mute_clear_needs_confirm {
                if ui.button("CANCEL").clicked() {
                    app.mute_clear_needs_confirm = false;
                }
                if ui.button("YES, CLEAR ALL").clicked() {
                    let _ = GLOBALS.to_overlord.send(ToOverlordMessage::ClearMuteList);
                    app.mute_clear_needs_confirm = false;
                }
            } else {
                if ui.button("Clear All").clicked() {
                    app.mute_clear_needs_confirm = true;
                }
            }
        }
    });

    ui.add_space(10.0);

    let last_mute_list_edit = match GLOBALS
        .storage
        .get_person_list_last_edit_time(PersonList::Muted)
    {
        Ok(Some(date)) => date,
        Ok(None) => 0,
        Err(e) => {
            tracing::error!("{}", e);
            0
        }
    };

    let mut ledit = "unknown".to_owned();
    if let Ok(stamp) = time::OffsetDateTime::from_unix_timestamp(last_mute_list_edit) {
        if let Ok(formatted) = stamp.format(time::macros::format_description!(
            "[year]-[month repr:short]-[day] ([weekday repr:short]) [hour]:[minute]"
        )) {
            ledit = formatted;
        }
    }
    ui.label(RichText::new(format!("LOCAL: {} (size={})", ledit, people.len())).size(15.0))
        .on_hover_text("This is the local (and effective) mute list");

    if !GLOBALS.signer.is_ready() {
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            ui.label("You need to ");
            if ui.link("setup your identity").clicked() {
                app.set_page(Page::YourKeys);
            }
            ui.label(" to push.");
        });
    }

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(10.0);

    ui.heading(format!("People who are Muted ({})", people.len()));
    ui.add_space(10.0);

    app.vert_scroll_area().show(ui, |ui| {
        for person in people.iter() {
            ui.horizontal(|ui| {
                // Avatar first
                let avatar = if let Some(avatar) = app.try_get_avatar(ctx, &person.pubkey) {
                    avatar
                } else {
                    app.placeholder_avatar.clone()
                };
                if widgets::paint_avatar(ui, person, &avatar, widgets::AvatarSize::Feed)
                    .clicked()
                {
                    app.set_page(Page::Person(person.pubkey));
                };

                ui.vertical(|ui| {
                    ui.label(RichText::new(gossip_lib::names::pubkey_short(&person.pubkey)).weak());
                    GossipUi::render_person_name_line(app, ui, person, false);

                    if ui.button("UNMUTE").clicked() {
                        let _ = GLOBALS.people.mute(&person.pubkey, false, true);
                    }
                });
            });

            ui.add_space(4.0);

            ui.separator();
        }
    });
}
