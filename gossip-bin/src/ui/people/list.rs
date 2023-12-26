use std::time::{Duration, Instant};

use super::{GossipUi, Page};
use crate::ui::widgets;
use crate::AVATAR_SIZE_F32;
use eframe::egui;
use egui::{Context, RichText, Ui, Vec2};
use egui_winit::egui::text::LayoutJob;
use egui_winit::egui::text_edit::TextEditOutput;
use egui_winit::egui::vec2;
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::{FeedKind, Person, PersonList, PersonListMetadata, GLOBALS};
use nostr_types::{Profile, PublicKey, Unixtime};

pub(in crate::ui) struct ListUi {
    // cache
    cache_last_list: Option<PersonList>,
    cache_next_refresh: Instant,
    cache_people: Vec<(Person, bool)>,
    cache_remote_tag: String,
    cache_local_tag: String,

    // add contact
    add_contact_search: String,
    add_contact_searched: Option<String>,
    add_contact_search_results: Vec<(String, PublicKey)>,
    add_contact_search_selected: Option<usize>,

    entering_follow_someone_on_list: bool,
    clear_list_needs_confirm: bool,
}

impl ListUi {
    pub(crate) fn new() -> Self {
        Self {
            // cache
            cache_last_list: None,
            cache_next_refresh: Instant::now(),
            cache_people: Vec::new(),
            cache_remote_tag: String::new(),
            cache_local_tag: String::new(),

            // add contact
            add_contact_search: String::new(),
            add_contact_searched: None,
            add_contact_search_results: Vec::new(),
            add_contact_search_selected: None,

            entering_follow_someone_on_list: false,
            clear_list_needs_confirm: false,
        }
    }
}

pub(super) fn enter_page(app: &mut GossipUi, list: PersonList) {
    refresh_list_data(app, list);
}

pub(super) fn update(
    app: &mut GossipUi,
    ctx: &Context,
    _frame: &mut eframe::Frame,
    ui: &mut Ui,
    list: PersonList,
) {
    if app.people_list.cache_next_refresh < Instant::now()
        || app.people_list.cache_last_list.is_none()
        || app.people_list.cache_last_list.unwrap() != list
    {
        refresh_list_data(app, list);
    }

    // process popups first
    if app.people_list.clear_list_needs_confirm {
        render_clear_list_confirm_popup(ui, app, list);
    }
    if app.people_list.entering_follow_someone_on_list {
        render_add_contact_popup(ui, app, list);
    }

    // disable rest of ui when popups are open
    let enabled = !app.people_list.entering_follow_someone_on_list
        && !app.people_list.clear_list_needs_confirm;

    let mut metadata = GLOBALS
        .storage
        .get_person_list_metadata(list)
        .unwrap_or_default()
        .unwrap_or_default();

    let title_job = layout_list_title(ui, app, &metadata);

    // render page
    widgets::page_header_layout(ui, title_job, |ui| {
        ui.add_enabled_ui(enabled, |ui| {
            let len = metadata.len;
            render_more_list_actions(ui, app, list, &mut metadata, len, true);
        });

        app.theme.accent_button_1_style(ui.style_mut());

        btn_h_space!(ui);

        if ui.button("Add contact").clicked() {
            app.people_list.entering_follow_someone_on_list = true;
        }

        btn_h_space!(ui);

        if ui.button("View the Feed").clicked() {
            app.set_page(
                ctx,
                Page::Feed(FeedKind::List(list, app.mainfeed_include_nonroot)),
            );
        }
    });

    ui.set_enabled(enabled);

    if GLOBALS.signer.is_ready() {
        ui.vertical(|ui| {
            ui.label(RichText::new(&app.people_list.cache_remote_tag))
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
            ui.label(RichText::new(&app.people_list.cache_local_tag))
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

    ui.add_space(10.0);

    app.vert_scroll_area().show(ui, |ui| {
        // not nice but needed because of 'app' borrow in closure
        let people = app.people_list.cache_people.clone();
        for (person, mut public) in people.iter() {
            let row_response = widgets::list_entry::make_frame(
                ui,
                Some(app.theme.main_content_bgcolor()),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Avatar first
                    let avatar = if let Some(avatar) = app.try_get_avatar(ctx, &person.pubkey) {
                        avatar
                    } else {
                        app.placeholder_avatar.clone()
                    };

                    let avatar_response =
                        widgets::paint_avatar(ui, person, &avatar, widgets::AvatarSize::Feed);

                    ui.add_space(20.0);

                    ui.vertical(|ui| {
                        ui.add_space(5.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(person.best_name()).size(15.5));

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
                        ui.add_space(3.0);
                        ui.label(GossipUi::richtext_from_person_nip05(person).weak());
                    });

                    ui.vertical(|ui| {
                        ui.with_layout(
                            egui::Layout::right_to_left(egui::Align::Min)
                                .with_cross_align(egui::Align::Center),
                            |ui| {
                                widgets::MoreMenu::simple(ui, app).show(ui, |ui, is_open| {
                                    // actions
                                    if ui.button("Remove").clicked() {
                                        let _ = GLOBALS.storage.remove_person_from_list(
                                            &person.pubkey,
                                            list,
                                            None,
                                        );
                                        *is_open = false;
                                    }
                                });

                                ui.add_space(20.0);

                                if list != PersonList::Followed {
                                    // private / public switch
                                    ui.label("Private");
                                    if ui
                                        .add(widgets::Switch::onoff(&app.theme, &mut public))
                                        .clicked()
                                    {
                                        let _ = GLOBALS.storage.add_person_to_list(
                                            &person.pubkey,
                                            list,
                                            public,
                                            None,
                                        );
                                        mark_refresh(app);
                                    }
                                }
                            },
                        );
                    });
                    if avatar_response.clicked() {
                        app.set_page(ctx, Page::Person(person.pubkey));
                    }
                });
            });
            if row_response
                .response
                .interact(egui::Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                app.set_page(ctx, Page::Person(person.pubkey));
            }
        }
        ui.add_space(AVATAR_SIZE_F32 + 40.0);
    });

    if let Some(list) = app.deleting_list {
        super::list::render_delete_list_dialog(ui, app, list);
    } else if app.creating_list {
        super::list::render_create_list_dialog(ui, app);
    } else if let Some(list) = app.renaming_list {
        super::list::render_rename_list_dialog(ui, app, list);
    }
}

pub(in crate::ui) fn layout_list_title(
    ui: &mut Ui,
    app: &mut GossipUi,
    metadata: &PersonListMetadata,
) -> LayoutJob {
    let mut layout_job = LayoutJob::default();
    let style = ui.style();
    RichText::new(format!("{} ({})", metadata.title, metadata.len))
        .heading()
        .color(ui.visuals().widgets.noninteractive.fg_stroke.color)
        .append_to(
            &mut layout_job,
            style,
            egui::FontSelection::Default,
            egui::Align::LEFT,
        );
    if metadata.favorite {
        RichText::new(" ★")
            .heading()
            .size(18.0)
            .color(app.theme.accent_complementary_color())
            .append_to(
                &mut layout_job,
                style,
                egui::FontSelection::Default,
                egui::Align::LEFT,
            );
    }
    if metadata.private {
        RichText::new(" 😎")
            .heading()
            .size(14.5)
            .color(app.theme.accent_complementary_color())
            .append_to(
                &mut layout_job,
                style,
                egui::FontSelection::Default,
                egui::Align::LEFT,
            );
    }
    layout_job
}

fn render_add_contact_popup(ui: &mut Ui, app: &mut GossipUi, list: PersonList) {
    const DLG_SIZE: Vec2 = vec2(400.0, 240.0);
    let ret = crate::ui::widgets::modal_popup(ui, DLG_SIZE, DLG_SIZE, true, |ui| {
        let enter_key;
        (app.people_list.add_contact_search_selected, enter_key) =
            if app.people_list.add_contact_search_results.is_empty() {
                (None, false)
            } else {
                widgets::capture_keyboard_for_search(
                    ui,
                    app.people_list.add_contact_search_results.len(),
                    app.people_list.add_contact_search_selected,
                )
            };

        ui.heading("Add contact to the list");
        ui.add_space(8.0);

        ui.label("Search for known contacts to add");
        ui.add_space(8.0);

        let mut output =
            widgets::search_field(ui, &mut app.people_list.add_contact_search, f32::INFINITY);

        let mut selected = app.people_list.add_contact_search_selected;
        widgets::show_contact_search(
            ui,
            app,
            egui::AboveOrBelow::Below,
            &mut output,
            &mut selected,
            app.people_list.add_contact_search_results.clone(),
            enter_key,
            |_, app, _, pair| {
                app.people_list.add_contact_search = pair.0.clone();
                app.people_list.add_contact_search_results.clear();
                app.people_list.add_contact_search_selected = None;
                app.add_contact = pair.1.as_bech32_string();
            },
        );
        app.people_list.add_contact_search_selected = selected;

        recalc_add_contact_search(app, &mut output);

        ui.add_space(8.0);

        ui.label("To add a new contact to this list enter their npub, hex key, nprofile or nip-05 address");
        ui.add_space(8.0);

        ui.add(
            text_edit_multiline!(app, app.add_contact)
                .desired_width(f32::INFINITY)
                .hint_text("npub1, hex key, nprofile1, or user@domain"),
        );

        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                    let mut try_add = false;
                    let mut want_close = false;
                    let mut can_close = false;

                    app.theme.accent_button_1_style(ui.style_mut());
                    if ui.button("Add and close").clicked() {
                        try_add |= true;
                        want_close = true;
                    }

                    btn_h_space!(ui);

                    app.theme.accent_button_2_style(ui.style_mut());
                    if ui.button("Add and continue").clicked() {
                        try_add |= true;
                    }

                    if try_add {
                        let mut add_failed = false;
                        if let Ok(pubkey) =
                            PublicKey::try_from_bech32_string(app.add_contact.trim(), true)
                        {
                            let _ = GLOBALS
                                .to_overlord
                                .send(ToOverlordMessage::FollowPubkey(pubkey, list, true));
                            can_close = true;
                            mark_refresh(app);
                        } else if let Ok(pubkey) =
                            PublicKey::try_from_hex_string(app.add_contact.trim(), true)
                        {
                            let _ = GLOBALS
                                .to_overlord
                                .send(ToOverlordMessage::FollowPubkey(pubkey, list, true));
                            can_close = true;
                            mark_refresh(app);
                        } else if let Ok(profile) =
                            Profile::try_from_bech32_string(app.add_contact.trim(), true)
                        {
                            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::FollowNprofile(
                                profile.clone(),
                                list,
                                true,
                            ));
                            can_close = true;
                            mark_refresh(app);
                        } else if gossip_lib::nip05::parse_nip05(app.add_contact.trim()).is_ok() {
                            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::FollowNip05(
                                app.add_contact.trim().to_owned(),
                                list,
                                true,
                            ));
                            can_close = true;
                            mark_refresh(app);
                        } else {
                            add_failed = true;
                            GLOBALS
                                .status_queue
                                .write()
                                .write("Invalid pubkey.".to_string());
                        }
                        if !add_failed {
                            app.add_contact = "".to_owned();
                            app.people_list.add_contact_search.clear();
                            app.people_list.add_contact_searched = None;
                            app.people_list.add_contact_search_selected = None;
                            app.people_list.add_contact_search_results.clear();
                        }
                        if want_close && can_close {
                            app.people_list.entering_follow_someone_on_list = false;
                            mark_refresh(app);
                        }
                    }
                });
            });
        });
    });
    if ret.inner.clicked() {
        app.people_list.entering_follow_someone_on_list = false;
        app.people_list.add_contact_search.clear();
        app.people_list.add_contact_searched = None;
        app.people_list.add_contact_search_selected = None;
        app.people_list.add_contact_search_results.clear();
    }
}

pub(super) fn render_delete_list_dialog(ui: &mut Ui, app: &mut GossipUi, list: PersonList) {
    let metadata = GLOBALS
        .storage
        .get_person_list_metadata(list)
        .unwrap_or_default()
        .unwrap_or_default();

    let ret = crate::ui::widgets::modal_popup(
        ui,
        vec2(250.0, 80.0),
        vec2(250.0, ui.available_height()),
        true,
        |ui| {
            ui.vertical(|ui| {
                ui.label("Are you sure you want to delete:");
                ui.add_space(10.0);
                ui.heading(metadata.title);
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button("Cancel").clicked() {
                        app.deleting_list = None;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::default()), |ui| {
                        app.theme.accent_button_1_style(ui.style_mut());
                        app.theme.accent_button_danger_hover(ui.style_mut());
                        if ui.button("Delete").clicked() {
                            let _ = GLOBALS
                                .to_overlord
                                .send(ToOverlordMessage::DeletePersonList(list));
                            app.deleting_list = None;
                            app.set_page(ui.ctx(), Page::PeopleLists);
                        }
                    })
                });
            });
        },
    );
    if ret.inner.clicked() {
        app.deleting_list = None;
    }
}

pub(super) fn render_create_list_dialog(ui: &mut Ui, app: &mut GossipUi) {
    let ret = crate::ui::widgets::modal_popup(
        ui,
        vec2(250.0, 100.0),
        vec2(250.0, ui.available_height()),
        true,
        |ui| {
            ui.vertical(|ui| {
                ui.heading("Create a new list");
                ui.add_space(5.0);
                if let Some(err) = &app.editing_list_error {
                    ui.label(egui::RichText::new(err).color(ui.visuals().error_fg_color));
                    ui.add_space(3.0);
                }
                let response =
                    ui.add(text_edit_line!(app, app.new_list_name).hint_text("list name"));
                if app.list_name_field_needs_focus {
                    response.request_focus();
                    app.list_name_field_needs_focus = false;
                }
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.add(widgets::Switch::onoff(
                        &app.theme,
                        &mut app.new_list_favorite,
                    ));
                    ui.label("Set as Favorite");
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::default()), |ui| {
                        app.theme.accent_button_1_style(ui.style_mut());
                        if ui.button("Create").clicked() {
                            app.new_list_name = app.new_list_name.trim().into();
                            if !app.new_list_name.is_empty() {
                                let dtag = format!("pl{}", Unixtime::now().unwrap().0);
                                let metadata = PersonListMetadata {
                                    dtag,
                                    title: app.new_list_name.to_owned(),
                                    favorite: app.new_list_favorite,
                                    ..Default::default()
                                };

                                if let Err(e) =
                                    GLOBALS.storage.allocate_person_list(&metadata, None)
                                {
                                    app.editing_list_error = Some(e.to_string());
                                    app.list_name_field_needs_focus = true;
                                } else {
                                    app.creating_list = false;
                                    app.new_list_name.clear();
                                    app.new_list_favorite = false;
                                    app.editing_list_error = None;
                                }
                            } else {
                                app.editing_list_error =
                                    Some("List name must not be empty".to_string());
                                app.list_name_field_needs_focus = true;
                            }
                        }
                    });
                });
            });
        },
    );
    if ret.inner.clicked() {
        app.creating_list = false;
        app.new_list_name.clear();
        app.new_list_favorite = false;
        app.editing_list_error = None;
    }
}

pub(super) fn render_rename_list_dialog(ui: &mut Ui, app: &mut GossipUi, list: PersonList) {
    let metadata = GLOBALS
        .storage
        .get_person_list_metadata(list)
        .unwrap_or_default()
        .unwrap_or_default();

    let ret = crate::ui::widgets::modal_popup(
        ui,
        vec2(250.0, 80.0),
        vec2(250.0, ui.available_height()),
        true,
        |ui| {
            ui.vertical(|ui| {
                ui.heading(&metadata.title);
                ui.add_space(5.0);
                if let Some(err) = &app.editing_list_error {
                    ui.label(egui::RichText::new(err).color(ui.visuals().error_fg_color));
                    ui.add_space(3.0);
                }
                ui.add_space(3.0);
                ui.label("Enter new name:");
                ui.add_space(5.0);
                ui.add(
                    text_edit_line!(app, app.new_list_name)
                        .hint_text(metadata.title)
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::default()), |ui| {
                        app.theme.accent_button_1_style(ui.style_mut());
                        if ui.button("Rename").clicked() {
                            app.new_list_name = app.new_list_name.trim().into();
                            if !app.new_list_name.is_empty() {
                                if let Err(e) = GLOBALS.storage.rename_person_list(
                                    list,
                                    app.new_list_name.clone(),
                                    None,
                                ) {
                                    app.editing_list_error = Some(e.to_string());
                                    app.list_name_field_needs_focus = true;
                                } else {
                                    app.renaming_list = None;
                                    app.new_list_name = "".to_owned();
                                    app.editing_list_error = None;
                                }
                            } else {
                                app.editing_list_error =
                                    Some("List name must not be empty".to_string());
                                app.list_name_field_needs_focus = true;
                            }
                        }
                    });
                });
            });
        },
    );
    if ret.inner.clicked() {
        app.renaming_list = None;
        app.new_list_name = "".to_owned();
        app.editing_list_error = None;
    }
}

pub(super) fn render_more_list_actions(
    ui: &mut Ui,
    app: &mut GossipUi,
    list: PersonList,
    metadata: &mut PersonListMetadata,
    count: usize,
    on_list: bool,
) {
    let menu = if on_list {
        widgets::MoreMenu::bubble(ui, app)
            .with_min_size(vec2(100.0, 0.0))
            .with_max_size(vec2(160.0, f32::INFINITY))
    } else {
        widgets::MoreMenu::simple(ui, app)
            .with_min_size(vec2(100.0, 0.0))
            .with_max_size(vec2(160.0, f32::INFINITY))
    };

    menu.show(ui, |ui, is_open| {
        ui.with_layout(egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
            if on_list {
                app.theme.accent_button_1_style(ui.style_mut());
                ui.spacing_mut().item_spacing.y = 10.0;
            }
            if !on_list {
                if ui.button("View Contacts").clicked() {
                    app.set_page(ui.ctx(), Page::PeopleList(list));
                    *is_open = false;
                }
            }
            if matches!(list, PersonList::Custom(_)) {
                if ui.button("Rename List").clicked() {
                    app.deleting_list = None;
                    app.renaming_list = Some(list);
                    *is_open = false;
                }
                if metadata.private {
                    if ui.button("Make Public").clicked() {
                        metadata.private = false;
                        let _ = GLOBALS
                            .storage
                            .set_person_list_metadata(list, metadata, None);
                        *is_open = false;
                    }
                } else {
                    if ui.button("Make Private").clicked() {
                        metadata.private = true;
                        let _ = GLOBALS
                            .storage
                            .set_person_list_metadata(list, metadata, None);
                        let _ = GLOBALS
                            .storage
                            .set_all_people_in_list_to_private(list, None);
                        *is_open = false;
                    }
                }
                if metadata.favorite {
                    if ui.button("Remove from Favorites").clicked() {
                        metadata.favorite = false;
                        let _ = GLOBALS
                            .storage
                            .set_person_list_metadata(list, metadata, None);
                        *is_open = false;
                    }
                } else {
                    if ui.button("Make Favorite").clicked() {
                        metadata.favorite = true;
                        let _ = GLOBALS
                            .storage
                            .set_person_list_metadata(list, metadata, None);
                        *is_open = false;
                    }
                }
                if count > 0 && on_list {
                    if ui.button("Clear All").clicked() {
                        app.people_list.clear_list_needs_confirm = true;
                        *is_open = false;
                    }
                }
                if count == 0 && ui.button("Delete List").clicked() {
                    app.renaming_list = None;
                    app.deleting_list = Some(list);
                    *is_open = false;
                }
            }
        });
    });
}

fn recalc_add_contact_search(app: &mut GossipUi, output: &mut TextEditOutput) {
    // only recalc if search text changed
    if app.people_list.add_contact_search.len() > 2 && output.cursor_range.is_some() {
        if Some(&app.people_list.add_contact_search)
            != app.people_list.add_contact_searched.as_ref()
        {
            let mut pairs = GLOBALS
                .people
                .search_people_to_tag(app.people_list.add_contact_search.as_str())
                .unwrap_or_default();
            // followed contacts first
            pairs.sort_by(|(_, ak), (_, bk)| {
                let af = GLOBALS
                    .storage
                    .is_person_in_list(ak, gossip_lib::PersonList::Followed)
                    .unwrap_or(false);
                let bf = GLOBALS
                    .storage
                    .is_person_in_list(bk, gossip_lib::PersonList::Followed)
                    .unwrap_or(false);
                bf.cmp(&af).then(std::cmp::Ordering::Greater)
            });
            app.people_list.add_contact_searched = Some(app.people_list.add_contact_search.clone());
            app.people_list.add_contact_search_results = pairs.to_owned();
        }
    } else {
        app.people_list.add_contact_searched = None;
        app.people_list.add_contact_search_results.clear();
    }
}

fn render_clear_list_confirm_popup(ui: &mut Ui, app: &mut GossipUi, list: PersonList) {
    const DLG_SIZE: Vec2 = vec2(250.0, 40.0);
    let popup = widgets::modal_popup(ui, DLG_SIZE, DLG_SIZE, true, |ui| {
        ui.vertical(|ui| {
            ui.label("Are you sure you want to clear this list?");
            ui.add_space(10.0);
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.horizontal(|ui| {
                    app.theme.accent_button_2_style(ui.style_mut());
                    if ui.button("Cancel").clicked() {
                        app.people_list.clear_list_needs_confirm = false;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::default()), |ui| {
                        app.theme.accent_button_1_style(ui.style_mut());
                        if ui.button("YES, CLEAR ALL").clicked() {
                            let _ = GLOBALS
                                .to_overlord
                                .send(ToOverlordMessage::ClearPersonList(list));
                            app.people_list.clear_list_needs_confirm = false;
                            mark_refresh(app);
                        }
                    });
                });
            });
        });
    });

    if popup.inner.clicked() {
        app.people_list.clear_list_needs_confirm = false;
    }
}

fn mark_refresh(app: &mut GossipUi) {
    app.people_list.cache_next_refresh = Instant::now();
}

fn refresh_list_data(app: &mut GossipUi, list: PersonList) {
    // prepare data
    app.people_list.cache_people = {
        let members = GLOBALS.storage.get_people_in_list(list).unwrap_or_default();

        let mut people: Vec<(Person, bool)> = Vec::new();

        for (pk, public) in &members {
            if let Ok(Some(person)) = GLOBALS.storage.read_person(pk) {
                people.push((person, *public));
            } else {
                let person = Person::new(*pk);
                let _ = GLOBALS.storage.write_person(&person, None);
                people.push((person, *public));
            }

            // They are a person of interest (to as to fetch metadata if out of date)
            GLOBALS.people.person_of_interest(*pk);
        }
        people.sort_by(|a, b| a.0.cmp(&b.0));
        people
    };

    let metadata = GLOBALS
        .storage
        .get_person_list_metadata(list)
        .unwrap_or_default()
        .unwrap_or_default();

    let mut asof = "time unknown".to_owned();
    if let Ok(stamp) = time::OffsetDateTime::from_unix_timestamp(metadata.event_created_at.0) {
        if let Ok(formatted) = stamp.format(time::macros::format_description!(
            "[year]-[month repr:short]-[day] ([weekday repr:short]) [hour]:[minute]"
        )) {
            asof = formatted;
        }
    }

    app.people_list.cache_remote_tag = if metadata.event_created_at.0 == 0 {
        "REMOTE: not found on Active Relays".to_owned()
    } else if let Some(private_len) = metadata.event_private_len {
        format!(
            "REMOTE: {} (public_len={} private_len={})",
            asof, metadata.event_public_len, private_len
        )
    } else {
        format!(
            "REMOTE: {} (public_len={})",
            asof, metadata.event_public_len
        )
    };

    let mut ledit = "time unknown".to_owned();
    if metadata.last_edit_time.0 > 0 {
        if let Ok(stamp) = time::OffsetDateTime::from_unix_timestamp(metadata.last_edit_time.0) {
            if let Ok(formatted) = stamp.format(time::macros::format_description!(
                "[year]-[month repr:short]-[day] ([weekday repr:short]) [hour]:[minute]"
            )) {
                ledit = formatted;
            }
        }
    }

    let publen = app
        .people_list
        .cache_people
        .iter()
        .filter(|(_, public)| *public)
        .count();
    let privlen = app.people_list.cache_people.len() - publen;

    app.people_list.cache_local_tag = format!(
        "LOCAL: {} (public_len={}, private_len={})",
        ledit, publen, privlen
    );

    app.people_list.cache_next_refresh = Instant::now() + Duration::new(1, 0);
    app.people_list.cache_last_list = Some(list);
}
