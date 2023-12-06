use super::{GossipUi, Page};
use crate::ui::widgets;
use eframe::egui;
use egui::{Context, RichText, Ui, Vec2};
use egui_winit::egui::vec2;
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::{Person, PersonList, GLOBALS};
use nostr_types::{Profile, PublicKey};

pub(crate) struct ListUi {
    configure_list_menu_active: bool,
    entering_follow_someone_on_list: bool,
    clear_list_needs_confirm: bool,
}

impl ListUi {
    pub(crate) fn new() -> Self {
        Self {
            configure_list_menu_active: false,
            entering_follow_someone_on_list: false,
            clear_list_needs_confirm: false,
        }
    }
}

pub(super) fn update(
    app: &mut GossipUi,
    ctx: &Context,
    _frame: &mut eframe::Frame,
    ui: &mut Ui,
    list: PersonList,
) {
    // prepare data
    // TODO cache this to improve performance
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

    let remote_text = if let Some(private_len) = latest_event_data.private_len {
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

    // render page
    widgets::page_header(ui, format!("{} ({})", list.name(), people.len()), |ui| {
        ui.add_enabled_ui(true, |ui| {
            let min_size = vec2(50.0, 20.0);

            widgets::MoreMenu::new(&app)
                .with_min_size(min_size)
                .show(ui, &mut app.people_list.configure_list_menu_active, |ui|{
                // since we are displaying over an accent color background, load that style
                app.theme.accent_button_2_style(ui.style_mut());

                if ui.button("Clear All").clicked() {
                    app.people_list.clear_list_needs_confirm = true;
                }

                // ui.add_space(8.0);
            });
        });

        btn_h_space!(ui);

        if ui.button("Add contact").clicked() {
            app.people_list.entering_follow_someone_on_list = true;
        }
    });

    if GLOBALS.signer.is_ready() {
        ui.vertical(|ui| {
            ui.label(RichText::new(remote_text))
                .on_hover_text("This is the data in the latest list event fetched from relays");

            ui.add_space(5.0);

            // remote <-> local buttons
            ui.horizontal(|ui|{
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

                if ui
                    .button("↑ Publish ↑")
                    .on_hover_text("This publishes the list to your relays")
                    .clicked()
                {
                    let _ = GLOBALS
                        .to_overlord
                        .send(ToOverlordMessage::PushPersonList(list));
                }
            });

            ui.add_space(5.0);

            // local timestamp
            ui.label(RichText::new(format!("LOCAL: {} (size={})", ledit, people.len())))
                .on_hover_text("This is the local (and effective) list");
        });
    } else {
        ui.horizontal(|ui| {
            ui.label("You need to ");
            if ui.link("setup your identity").clicked() {
                app.set_page(ctx, Page::YourKeys);
            }
            ui.label(" to manage list events.");
        });
    }

    if app.people_list.clear_list_needs_confirm {
        const DLG_SIZE: Vec2 = vec2(250.0, 40.0);
        if widgets::modal_popup(ui, DLG_SIZE, |ui| {
            ui.vertical(|ui| {
                ui.label("Are you sure you want to clear this list?");
                ui.add_space(10.0);
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT),|ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            app.people_list.clear_list_needs_confirm = false;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::default()), |ui|{
                            if ui.button("YES, CLEAR ALL").clicked() {
                                let _ = GLOBALS
                                    .to_overlord
                                    .send(ToOverlordMessage::ClearPersonList(list));
                                app.people_list.clear_list_needs_confirm = false;
                            }
                        });
                    });
                });
            });
        }).inner.clicked() {
            app.people_list.clear_list_needs_confirm = false;
        }
    }

    ui.add_space(10.0);

    app.vert_scroll_area().show(ui, |ui| {
        for (person, public) in people.iter() {
            let row_response = widgets::list_entry::make_frame(ui)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        // Avatar first
                        let avatar = if let Some(avatar) = app.try_get_avatar(ctx, &person.pubkey) {
                            avatar
                        } else {
                            app.placeholder_avatar.clone()
                        };
                        let avatar_height = widgets::paint_avatar(ui, person, &avatar, widgets::AvatarSize::Feed).rect.height();

                        ui.add_space(20.0);

                        ui.vertical(|ui| {
                            ui.set_min_height(avatar_height);
                            ui.horizontal(|ui| {
                                ui.label(GossipUi::person_name(person));

                                ui.add_space(10.0);

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
                            });
                            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(gossip_lib::names::pubkey_short(&person.pubkey)).weak());

                                    ui.add_space(10.0);

                                    ui.label(GossipUi::richtext_from_person_nip05(person));
                                });
                            });
                        });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                            ui.set_min_height(avatar_height);
                            // actions
                            if ui.link("Remove").clicked() {
                                let _ = GLOBALS
                                    .storage
                                    .remove_person_from_list(&person.pubkey, list, None);
                            }

                            ui.add_space(20.0);

                            // private / public switch
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
            if row_response
                .response
                .interact(egui::Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked() {
                    app.set_page(ctx, Page::Person(person.pubkey));
            }
        }
    });

    if app.people_list.entering_follow_someone_on_list {
        const DLG_SIZE: Vec2 = vec2(400.0, 200.0);
        let ret = crate::ui::widgets::modal_popup(ui, DLG_SIZE, |ui| {
            ui.heading("Follow someone");

            ui.horizontal(|ui| {
                ui.label("Enter");
                ui.add(
                    text_edit_line!(app, app.follow_someone)
                        .hint_text("npub1, hex key, nprofile1, or user@domain"),
                );
            });
            if ui.button("follow").clicked() {
                if let Ok(pubkey) =
                    PublicKey::try_from_bech32_string(app.follow_someone.trim(), true)
                {
                    let _ = GLOBALS
                        .to_overlord
                        .send(ToOverlordMessage::FollowPubkey(pubkey, list, true));
                    app.people_list.entering_follow_someone_on_list = false;
                } else if let Ok(pubkey) =
                    PublicKey::try_from_hex_string(app.follow_someone.trim(), true)
                {
                    let _ = GLOBALS
                        .to_overlord
                        .send(ToOverlordMessage::FollowPubkey(pubkey, list, true));
                    app.people_list.entering_follow_someone_on_list = false;
                } else if let Ok(profile) =
                    Profile::try_from_bech32_string(app.follow_someone.trim(), true)
                {
                    let _ = GLOBALS.to_overlord.send(ToOverlordMessage::FollowNprofile(
                        profile.clone(),
                        list,
                        true,
                    ));
                    app.people_list.entering_follow_someone_on_list = false;
                } else if gossip_lib::nip05::parse_nip05(app.follow_someone.trim()).is_ok() {
                    let _ = GLOBALS.to_overlord.send(ToOverlordMessage::FollowNip05(
                        app.follow_someone.trim().to_owned(),
                        list,
                        true,
                    ));
                } else {
                    GLOBALS
                        .status_queue
                        .write()
                        .write("Invalid pubkey.".to_string());
                }
                app.follow_someone = "".to_owned();
            }
        });
        if ret.inner.clicked() {
            app.people_list.entering_follow_someone_on_list = false;
        }
    }
}
