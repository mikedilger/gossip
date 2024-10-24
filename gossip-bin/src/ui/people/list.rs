use std::sync::Arc;
use std::time::{Duration, Instant};

use super::{GossipUi, Page};
use crate::ui::widgets::{self, MoreMenuButton, MoreMenuItem};
use crate::AVATAR_SIZE_F32;
use eframe::egui::{self, Galley, Label, Sense};
use egui::{Context, RichText, Ui, Vec2};
use egui_winit::egui::text::LayoutJob;
use egui_winit::egui::text_edit::TextEditOutput;
use egui_winit::egui::vec2;
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::{
    FeedKind, Freshness, People, Person, PersonList, PersonListMetadata, PersonTable, Private,
    Table, GLOBALS,
};
use nostr_types::{Profile, PublicKey, Unixtime};

pub(in crate::ui) struct ListUi {
    // cache
    cache_last_list: Option<PersonList>,
    cache_next_refresh: Instant,
    cache_people: Vec<(Person, Private)>,
    cache_remote_hash: u64,
    cache_remote_tag: String,
    cache_local_hash: u64,
    cache_local_tag: String,

    // add contact
    add_contact_search: String,
    add_contact_searched: Option<String>,
    add_contact_search_results: Vec<(String, PublicKey)>,
    add_contact_search_selected: Option<usize>,
    add_contact_error: Option<String>,

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
            cache_remote_hash: 1,
            cache_remote_tag: String::new(),
            cache_local_hash: 2,
            cache_local_tag: String::new(),

            // add contact
            add_contact_search: String::new(),
            add_contact_searched: None,
            add_contact_search_results: Vec::new(),
            add_contact_search_selected: None,
            add_contact_error: None,

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

    let metadata = GLOBALS
        .db()
        .get_person_list_metadata(list)
        .unwrap_or_default()
        .unwrap_or_default();

    // process popups first
    let mut enabled = false;
    if app.people_list.clear_list_needs_confirm {
        render_clear_list_confirm_popup(ui, app, list);
    } else if app.people_list.entering_follow_someone_on_list {
        render_add_contact_popup(ui, app, list, &metadata);
    } else if let Some(list) = app.deleting_list {
        super::list::render_delete_list_dialog(ui, app, list);
    } else if app.creating_list {
        super::list::render_create_list_dialog(ui, app);
    } else if let Some(list) = app.renaming_list {
        super::list::render_rename_list_dialog(ui, app, list);
    } else {
        // only enable rest of ui when popups are not open
        enabled = true;
    }

    let title_job = layout_list_title(ui, app, &metadata);

    // render page
    widgets::page_header_layout(ui, title_job, |ui| {
        ui.add_enabled_ui(enabled, |ui| {
            let len = metadata.len;
            render_more_list_actions(ui, app, list, &metadata, len, true);

            btn_h_space!(ui);

            if widgets::Button::primary(&app.theme, "Add contact")
                .show(ui)
                .clicked()
            {
                app.people_list.entering_follow_someone_on_list = true;
            }

            btn_h_space!(ui);

            if widgets::Button::primary(&app.theme, "View the Feed")
                .show(ui)
                .clicked()
            {
                app.set_page(
                    ctx,
                    Page::Feed(FeedKind::List(list, app.mainfeed_include_nonroot)),
                );
            }
        });
    });

    if !enabled {
        ui.disable();
    }

    ui.add_space(5.0);

    ui.vertical(|ui| {

        ui.label(RichText::new(&app.people_list.cache_remote_tag))
            .on_hover_text("This is the data in the latest list event fetched from relays");

        ui.add_space(5.0);


        // remote <-> local buttons
        ui.horizontal(|ui|{
            if app.people_list.cache_local_hash == app.people_list.cache_remote_hash {
                ui.label("List is synchronized");
                ui.add_space(10.0);
            }

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

            if GLOBALS.identity.is_unlocked() {
                if ui
                    .button("↑ Publish ↑")
                    .on_hover_text("This publishes the list to your relays")
                    .clicked()
                {
                    let _ = GLOBALS
                        .to_overlord
                        .send(ToOverlordMessage::PushPersonList(list));
                }
            } else {
                ui.horizontal(|ui| {
                    ui.label("You need to ");
                    if ui.link("setup your private-key").clicked() {
                        app.set_page(ctx, Page::YourKeys);
                    }
                    ui.label(" to push lists.");
                });
            }
        });

        ui.add_space(5.0);

        // local timestamp
        ui.label(RichText::new(&app.people_list.cache_local_tag))
            .on_hover_text("This is the local (and effective) list");
    });

    ui.add_space(10.0);

    app.vert_scroll_area().show(ui, |ui| {
        // not nice but needed because of 'app' borrow in closure
        let mut people = app.people_list.cache_people.clone();
        for (person, private) in people.iter_mut() {
            let row_response = widgets::list_entry::clickable_frame(
                ui,
                app,
                Some(app.theme.main_content_bgcolor()),
                Some(app.theme.hovered_content_bgcolor()),
                |ui, app| {
                    ui.horizontal(|ui| {
                        // Avatar first
                        let avatar = if let Some(avatar) = app.try_get_avatar(ctx, &person.pubkey) {
                            avatar
                        } else {
                            app.placeholder_avatar.clone()
                        };

                        let mut response =
                            widgets::paint_avatar(ui, person, &avatar, widgets::AvatarSize::Feed);

                        ui.add_space(20.0);

                        ui.vertical(|ui| {
                            ui.add_space(5.0);
                            ui.horizontal(|ui| {
                                response |= ui.add(
                                    Label::new(RichText::new(person.best_name()).size(15.5))
                                        .selectable(false)
                                        .sense(Sense::click()),
                                );

                                ui.add_space(10.0);

                                let mut show_fetch_now = false;
                                match People::person_needs_relay_list(person.pubkey) {
                                    Freshness::NeverSought => {
                                        response |= ui.add(
                                            Label::new(
                                                RichText::new("Relay list not found")
                                                    .color(app.theme.warning_marker_text_color()),
                                            )
                                            .selectable(false)
                                            .sense(Sense::click()),
                                        );
                                        show_fetch_now = true;
                                    }
                                    Freshness::Stale => {
                                        response |= ui.add(
                                            Label::new(
                                                RichText::new("Relay list stale")
                                                    .color(app.theme.warning_marker_text_color()),
                                            )
                                            .selectable(false)
                                            .sense(Sense::click()),
                                        );
                                        show_fetch_now = true;
                                    }
                                    Freshness::Fresh => {}
                                };
                                if show_fetch_now {
                                    if ui.add(egui::Button::new("Fetch now").small()).clicked() {
                                        let _ = GLOBALS.to_overlord.send(
                                            ToOverlordMessage::SubscribeDiscover(
                                                vec![person.pubkey],
                                                None,
                                            ),
                                        );
                                    }
                                }
                            });

                            ui.add_space(3.0);
                            response |= ui.add(
                                Label::new(GossipUi::richtext_from_person_nip05(person).weak())
                                    .selectable(false)
                                    .sense(Sense::click()),
                            );
                        });

                        ui.vertical(|ui| {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Min)
                                    .with_cross_align(egui::Align::Center),
                                |ui| {
                                    let text = egui::RichText::new("=").size(13.0);
                                    let response = widgets::Button::primary(&app.theme, text)
                                        .small(true)
                                        .show(ui);
                                    let menu = widgets::MoreMenu::bubble(
                                        ui.auto_id_with(person.pubkey),
                                        vec2(100.0, 0.0),
                                        vec2(100.0, ctx.available_rect().height()),
                                    );
                                    let mut items: Vec<MoreMenuItem> = Vec::new();

                                    // actions
                                    items.push(MoreMenuItem::Button(MoreMenuButton::new(
                                        "Remove",
                                        Box::new(|_, _| {
                                            let _ = GLOBALS.db().remove_person_from_list(
                                                &person.pubkey,
                                                list,
                                                None,
                                            );
                                        }),
                                    )));

                                    menu.show_entries(ui, app, response, items);

                                    if list != PersonList::Followed {
                                        // private / public switch
                                        ui.add(Label::new("Private").selectable(false));
                                        if ui
                                            .add(widgets::Switch::small(&app.theme, &mut private.0))
                                            .clicked()
                                        {
                                            let _ = GLOBALS.db().add_person_to_list(
                                                &person.pubkey,
                                                list,
                                                *private,
                                                None,
                                            );
                                            mark_refresh(app);
                                        }
                                    }
                                },
                            );
                            response
                        })
                        .inner
                    })
                    .inner
                },
            );
            if row_response
                .inner
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                app.set_page(ctx, Page::Person(person.pubkey));
            }
        }
        ui.add_space(AVATAR_SIZE_F32 + 40.0);
    });
}

pub(in crate::ui) fn layout_list_title(
    ui: &mut Ui,
    app: &mut GossipUi,
    metadata: &PersonListMetadata,
) -> Arc<Galley> {
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
    if *metadata.private {
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
    ui.fonts(|fonts| fonts.layout_job(layout_job))
}

fn render_add_contact_popup(
    ui: &mut Ui,
    app: &mut GossipUi,
    list: PersonList,
    metadata: &PersonListMetadata,
) {
    const DLG_SIZE: Vec2 = vec2(400.0, 260.0);
    let ret = crate::ui::widgets::modal_popup(ui.ctx(), DLG_SIZE, DLG_SIZE, true, |ui| {
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

        // error block
        ui.label(
            RichText::new(
                app.people_list
                    .add_contact_error
                    .as_ref()
                    .unwrap_or(&"".to_string()),
            )
            .color(app.theme.warning_marker_text_color()),
        );
        ui.add_space(8.0);

        ui.label("Search for known contacts to add");
        ui.add_space(8.0);

        let mut output = widgets::TextEdit::search(
            &app.theme,
            &app.assets,
            &mut app.people_list.add_contact_search,
        )
        .desired_width(f32::INFINITY)
        .show(ui);

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

                    if widgets::Button::primary(&app.theme, "Add and close")
                        .show(ui)
                        .clicked()
                    {
                        try_add |= true;
                        want_close = true;
                    }

                    btn_h_space!(ui);

                    if widgets::Button::secondary(&app.theme, "Add and continue")
                        .show(ui)
                        .clicked()
                    {
                        try_add |= true;
                    }

                    if try_add {
                        let mut add_failed = false;
                        if let Ok(pubkey) =
                            PublicKey::try_from_bech32_string(app.add_contact.trim(), true)
                        {
                            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::FollowPubkey(
                                pubkey,
                                list,
                                metadata.private,
                            ));
                            can_close = true;
                            mark_refresh(app);
                        } else if let Ok(pubkey) =
                            PublicKey::try_from_hex_string(app.add_contact.trim(), true)
                        {
                            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::FollowPubkey(
                                pubkey,
                                list,
                                metadata.private,
                            ));
                            can_close = true;
                            mark_refresh(app);
                        } else if let Ok(profile) =
                            Profile::try_from_bech32_string(app.add_contact.trim(), true)
                        {
                            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::FollowNprofile(
                                profile.clone(),
                                list,
                                metadata.private,
                            ));
                            can_close = true;
                            mark_refresh(app);
                        } else if gossip_lib::nip05::parse_nip05(app.add_contact.trim()).is_ok() {
                            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::FollowNip05(
                                app.add_contact.trim().to_owned(),
                                list,
                                metadata.private,
                            ));
                            can_close = true;
                            mark_refresh(app);
                        } else {
                            add_failed = true;
                            app.people_list.add_contact_error = Some("Invalid pubkey.".to_string());
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
        .db()
        .get_person_list_metadata(list)
        .unwrap_or_default()
        .unwrap_or_default();

    let ret = crate::ui::widgets::modal_popup(
        ui.ctx(),
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
                    if widgets::Button::secondary(&app.theme, "Cancel")
                        .show(ui)
                        .clicked()
                    {
                        app.deleting_list = None;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::default()), |ui| {
                        if widgets::Button::primary(&app.theme, "Delete")
                            .with_danger_hover()
                            .show(ui)
                            .clicked()
                        {
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
        ui.ctx(),
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
                    ui.add(widgets::Switch::small(
                        &app.theme,
                        &mut app.new_list_favorite,
                    ));
                    ui.label("Set as Favorite");
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::default()), |ui| {
                        if widgets::Button::primary(&app.theme, "Create")
                            .show(ui)
                            .clicked()
                        {
                            app.new_list_name = app.new_list_name.trim().into();
                            if !app.new_list_name.is_empty() {
                                let dtag = format!("pl{}", Unixtime::now().0);
                                let metadata = PersonListMetadata {
                                    dtag,
                                    title: app.new_list_name.to_owned(),
                                    favorite: app.new_list_favorite,
                                    ..Default::default()
                                };

                                if let Err(e) = GLOBALS.db().allocate_person_list(&metadata, None) {
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
        .db()
        .get_person_list_metadata(list)
        .unwrap_or_default()
        .unwrap_or_default();

    let ret = crate::ui::widgets::modal_popup(
        ui.ctx(),
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
                        if widgets::Button::primary(&app.theme, "Rename")
                            .show(ui)
                            .clicked()
                        {
                            app.new_list_name = app.new_list_name.trim().into();
                            if !app.new_list_name.is_empty() {
                                if let Err(e) = GLOBALS.db().rename_person_list(
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
    metadata: &PersonListMetadata,
    count: usize,
    on_list: bool,
) {
    // do not show for "Following" and "Muted"
    if !on_list && !matches!(list, PersonList::Custom(_)) {
        return;
    }

    let text = egui::RichText::new("=").size(13.0);
    let response = widgets::Button::primary(&app.theme, text)
        .small(true)
        .show(ui);

    let menu = widgets::MoreMenu::bubble(
        ui.next_auto_id(),
        vec2(100.0, 0.0),
        vec2(140.0, ui.ctx().available_rect().height()),
    );

    let mut items: Vec<MoreMenuItem> = Vec::new();
    items.push(MoreMenuItem::Button(MoreMenuButton::new(
        "Rename",
        Box::new(|_, app| {
            app.deleting_list = None;
            app.renaming_list = Some(list);
        }),
    )));

    if metadata.favorite {
        items.push(MoreMenuItem::Button(MoreMenuButton::new(
            "Unset as Favorite",
            Box::new(|_, _| {
                let mut metadata = metadata.clone();
                metadata.favorite = false;
                let _ = GLOBALS.db().set_person_list_metadata(list, &metadata, None);
            }),
        )));
    } else {
        items.push(MoreMenuItem::Button(MoreMenuButton::new(
            "Set as Favorite",
            Box::new(|_, _| {
                let mut metadata = metadata.clone();
                metadata.favorite = true;
                let _ = GLOBALS.db().set_person_list_metadata(list, &metadata, None);
            }),
        )));
    }

    if on_list {
        if metadata.private == Private(true) {
            items.push(MoreMenuItem::Button(MoreMenuButton::new(
                "Make Public",
                Box::new(|_, _| {
                    let mut metadata = metadata.clone();
                    metadata.private = Private(false);
                    let _ = GLOBALS.db().set_person_list_metadata(list, &metadata, None);
                }),
            )));
        } else {
            items.push(MoreMenuItem::Button(MoreMenuButton::new(
                "Make Private",
                Box::new(|_, _| {
                    let mut metadata = metadata.clone();
                    metadata.private = Private(true);
                    let _ = GLOBALS.db().set_person_list_metadata(list, &metadata, None);
                    let _ = GLOBALS.db().set_all_people_in_list_to_private(list, None);
                }),
            )));
        }
        items.push(MoreMenuItem::Button(
            MoreMenuButton::new(
                "Clear All",
                Box::new(|_, app| {
                    app.people_list.clear_list_needs_confirm = true;
                }),
            )
            .enabled(count > 0),
        ));

        items.push(MoreMenuItem::Button(MoreMenuButton::new(
            "Delete",
            Box::new(|_, app| {
                app.renaming_list = None;
                app.deleting_list = Some(list);
            }),
        )));
    }

    menu.show_entries(ui, app, response, items);
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
                    .db()
                    .is_person_in_list(ak, gossip_lib::PersonList::Followed)
                    .unwrap_or(false);
                let bf = GLOBALS
                    .db()
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
    let popup = widgets::modal_popup(ui.ctx(), DLG_SIZE, DLG_SIZE, true, |ui| {
        ui.vertical(|ui| {
            ui.label("Are you sure you want to clear this list?");
            ui.add_space(10.0);
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.horizontal(|ui| {
                    if widgets::Button::secondary(&app.theme, "Cancel")
                        .show(ui)
                        .clicked()
                    {
                        app.people_list.clear_list_needs_confirm = false;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::default()), |ui| {
                        if widgets::Button::primary(&app.theme, "YES, CLEAR ALL")
                            .show(ui)
                            .clicked()
                        {
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
        let members = GLOBALS.db().get_people_in_list(list).unwrap_or_default();

        let mut people: Vec<(Person, Private)> = Vec::new();

        for (pk, private) in &members {
            if let Ok(Some(person)) = PersonTable::read_record(*pk, None) {
                people.push((person, *private));
            } else {
                let mut person = Person::new(*pk);
                let _ = PersonTable::write_record(&mut person, None);
                people.push((person, *private));
            }

            // They are a person of interest (to as to fetch metadata if out of date)
            GLOBALS.people.person_of_interest(*pk);
        }
        people.sort_by(|a, b| a.0.cmp(&b.0));
        people
    };

    let metadata = GLOBALS
        .db()
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

    app.people_list.cache_remote_hash = gossip_lib::hash_person_list_event(list).unwrap_or(1);

    app.people_list.cache_remote_tag = if metadata.event_created_at.0 == 0 {
        "REMOTE: not found on Active Relays".to_owned()
    } else if let Some(private_len) = metadata.event_private_len {
        format!(
            "REMOTE: date={} (public={} private={})",
            asof, metadata.event_public_len, private_len
        )
    } else {
        format!(
            "REMOTE: date={} (public={})",
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

    let mut prvlen = app
        .people_list
        .cache_people
        .iter()
        .filter(|(_, private)| **private)
        .count();
    if list == PersonList::Followed {
        prvlen = 0;
    }
    let publen = app.people_list.cache_people.len() - prvlen;

    app.people_list.cache_local_hash = GLOBALS.db().hash_person_list(list).unwrap_or(2);

    if list == PersonList::Followed {
        app.people_list.cache_local_tag = format!("LOCAL: date={} (public={})", ledit, publen);
    } else {
        app.people_list.cache_local_tag = format!(
            "LOCAL: date={} (public={}, private={})",
            ledit, publen, prvlen
        );
    }

    app.people_list.cache_next_refresh = Instant::now() + Duration::new(1, 0);
    app.people_list.cache_last_list = Some(list);
}
