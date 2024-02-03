use super::FeedNoteParams;
use crate::ui::widgets::InformationPopup;
use crate::ui::{widgets, you, FeedKind, GossipUi, HighlightType, Page, Theme};
use eframe::egui;
use eframe::epaint::text::LayoutJob;
use egui::containers::CollapsingHeader;
use egui::{Align, Context, Key, Layout, Modifiers, RichText, Ui};
use egui_winit::egui::text::CCursor;
use egui_winit::egui::text_edit::{CCursorRange, TextEditOutput};
use egui_winit::egui::{vec2, AboveOrBelow, Id};
use gossip_lib::comms::ToOverlordMessage;
use gossip_lib::DmChannel;
use gossip_lib::Relay;
use gossip_lib::GLOBALS;
use memoize::memoize;
use nostr_types::{ContentSegment, NostrBech32, NostrUrl, ShatteredContent, Tag};
use std::collections::HashMap;

#[memoize]
pub fn textarea_highlighter(theme: Theme, text: String, interests: Vec<String>) -> LayoutJob {
    let mut job = LayoutJob::default();

    // Shatter
    let shattered_content = ShatteredContent::new(text.clone());

    for segment in shattered_content.segments.iter() {
        match segment {
            ContentSegment::NostrUrl(nostr_url) => {
                let chunk = format!("{}", nostr_url);
                let highlight = match nostr_url.0 {
                    NostrBech32::EventAddr(_) => HighlightType::Event,
                    NostrBech32::EventPointer(_) => HighlightType::Event,
                    NostrBech32::Id(_) => HighlightType::Event,
                    NostrBech32::Profile(_) => HighlightType::PublicKey,
                    NostrBech32::Pubkey(_) => HighlightType::PublicKey,
                    NostrBech32::Relay(_) => HighlightType::Relay,
                };
                job.append(&chunk, 0.0, theme.highlight_text_format(highlight));
            }
            ContentSegment::TagReference(i) => {
                let chunk = format!("#[{}]", i);
                // This has been unrecommended, and we have to check if 'i' is in bounds.
                // So we don't do this anymore
                // job.append(&chunk, 0.0, theme.highlight_text_format(HighlightType::Event));
                job.append(
                    &chunk,
                    0.0,
                    theme.highlight_text_format(HighlightType::Nothing),
                );
            }
            ContentSegment::Hyperlink(span) => {
                let chunk = shattered_content.slice(span).unwrap();
                job.append(
                    chunk,
                    0.0,
                    theme.highlight_text_format(HighlightType::Hyperlink),
                );
            }
            ContentSegment::Plain(span) => {
                let chunk = shattered_content.slice(span).unwrap();

                let mut found_interests: Vec<(usize, String)> = Vec::new();

                // find all interests in chunk an remember position
                for interest in &interests {
                    for (pos, str) in chunk.match_indices(interest) {
                        found_interests.push((pos, str.to_owned()));
                    }
                }

                // sort by position (so our indice access below will not crash)
                found_interests.sort_by(|a, b| a.0.cmp(&b.0));

                let mut pos = 0;
                // loop all found interests in order
                for (ipos, interest) in found_interests {
                    // output anything before the interest
                    job.append(
                        &chunk[pos..ipos],
                        0.0,
                        theme.highlight_text_format(HighlightType::Nothing),
                    );

                    // update pos
                    pos = ipos + interest.len();

                    // output the interest
                    job.append(
                        &chunk[ipos..pos],
                        0.0,
                        theme.highlight_text_format(HighlightType::Hyperlink),
                    );
                }

                // output anything after last interest
                job.append(
                    &chunk[pos..chunk.len()],
                    0.0,
                    theme.highlight_text_format(HighlightType::Nothing),
                );
            }
        }
    }

    job
}

pub(in crate::ui) fn posting_area(
    app: &mut GossipUi,
    ctx: &Context,
    frame: &mut eframe::Frame,
    ui: &mut Ui,
) {
    // Posting Area
    ui.vertical(|ui| {
        if !GLOBALS.identity.is_unlocked() {
            ui.horizontal_wrapped(|ui| {
                if GLOBALS.identity.encrypted_private_key().is_some() {
                    you::offer_unlock_priv_key(app, ui);
                } else {
                    ui.label("You need to ");
                    if ui.link("setup your private-key").clicked() {
                        app.set_page(ctx, Page::YourKeys);
                    }
                    ui.label(" to post.");
                }
            });
        } else if GLOBALS
            .storage
            .filter_relays(|r| r.has_usage_bits(Relay::WRITE))
            .unwrap_or_default()
            .is_empty()
        {
            ui.horizontal_wrapped(|ui| {
                ui.label("You need to ");
                if ui.link("choose write relays").clicked() {
                    app.set_page(ctx, Page::RelaysKnownNetwork);
                }
                ui.label(" to post.");
            });
        } else {
            let dm_channel: Option<DmChannel> = match &app.page {
                Page::Feed(FeedKind::DmChat(dm_channel)) => Some(dm_channel.clone()),
                _ => None,
            };
            match &dm_channel {
                Some(dmc) => dm_posting_area(app, ctx, frame, ui, dmc),
                None => real_posting_area(app, ctx, frame, ui),
            }
        }
    });
}

fn dm_posting_area(
    app: &mut GossipUi,
    _ctx: &Context,
    _frame: &mut eframe::Frame,
    ui: &mut Ui,
    dm_channel: &DmChannel,
) {
    let compose_area_id: egui::Id = egui::Id::new("compose_area");
    let mut send_now: bool = false;

    // Text area
    let theme = app.theme;
    let mut layouter = |ui: &Ui, text: &str, wrap_width: f32| {
        let mut layout_job = textarea_highlighter(theme, text.to_owned(), Vec::new());
        layout_job.wrap.max_width = wrap_width;
        ui.fonts(|f| f.layout_job(layout_job))
    };

    if app.dm_draft_data.include_subject {
        ui.horizontal(|ui| {
            ui.label("Subject: ");
            ui.add(
                text_edit_line!(app, app.dm_draft_data.subject)
                    .hint_text("Type subject here")
                    .desired_width(f32::INFINITY),
            );
        });
    }

    if app.dm_draft_data.include_content_warning {
        ui.horizontal(|ui| {
            ui.label("Content Warning: ");
            ui.add(
                text_edit_line!(app, app.dm_draft_data.content_warning)
                    .hint_text("Type content warning here")
                    .desired_width(f32::INFINITY),
            );
        });
    }

    ui.label(format!("DIRECT MESSAGE TO: {}", dm_channel.name()));
    ui.add_space(10.0);
    ui.label("WARNING: DMs currently have security weaknesses and the more DMs you send, the easier it becomes for a sophisticated attacker to crack your shared secret and decrypt this entire conversation.");

    let draft_response = ui.add(
        text_edit_multiline!(app, app.dm_draft_data.draft)
            .id_source(compose_area_id)
            .hint_text("Type your message here")
            .desired_width(f32::INFINITY)
            .lock_focus(true)
            .interactive(true)
            .layouter(&mut layouter),
    );
    if app.draft_needs_focus {
        app.draft_needs_focus = false;
        draft_response.request_focus();
    }

    if !app.dm_draft_data.draft.is_empty() {
        let modifiers = if cfg!(target_os = "macos") {
            Modifiers {
                command: true,
                ..Default::default()
            }
        } else {
            Modifiers {
                ctrl: true,
                ..Default::default()
            }
        };
        if ui.input_mut(|i| i.consume_key(modifiers, Key::Enter)) {
            send_now = true;
        }
    }

    ui.add_space(8.0);

    ui.horizontal(|ui| {
        if ui.button("Clear").clicked() {
            app.reset_draft();
        }

        ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
            ui.add_space(12.0);

            if ui.button("Send").clicked() && !app.dm_draft_data.draft.is_empty() {
                send_now = true;
            }

            if app.dm_draft_data.include_subject {
                if ui.button("Remove Subject").clicked() {
                    app.dm_draft_data.include_subject = false;
                    app.dm_draft_data.subject = "".to_owned();
                }
            } else if ui.button("Add Subject").clicked() {
                app.dm_draft_data.include_subject = true;
            }

            if app.dm_draft_data.include_content_warning {
                if ui.button("Remove Content Warning").clicked() {
                    app.dm_draft_data.include_content_warning = false;
                    app.dm_draft_data.content_warning = "".to_owned();
                }
            } else if ui.button("Add Content Warning").clicked() {
                app.dm_draft_data.include_content_warning = true;
            }

            // Emoji picker
            ui.menu_button(RichText::new("😀▼").size(14.0), |ui| {
                if let Some(emoji) = crate::ui::components::emoji_picker(ui) {
                    app.dm_draft_data.draft.push(emoji);
                }
            });
        });
    });

    if send_now {
        let mut tags: Vec<Tag> = Vec::new();
        if app.dm_draft_data.include_content_warning {
            tags.push(Tag::new_content_warning(&app.dm_draft_data.content_warning));
        }
        if let Some(delegatee_tag) = GLOBALS.delegation.get_delegatee_tag() {
            tags.push(delegatee_tag);
        }
        if app.dm_draft_data.include_subject {
            tags.push(Tag::new_subject(app.dm_draft_data.subject.clone()));
        }

        let _ = GLOBALS.to_overlord.send(ToOverlordMessage::Post {
            content: app.dm_draft_data.draft.clone(),
            tags,
            in_reply_to: None,
            dm_channel: Some(dm_channel.to_owned()),
        });

        app.reset_draft();
    }

    // List tags that will be applied
    // FIXME: list tags from parent event too in case of reply
    // FIXME: tag handling in overlord::post() needs to move back here so the user can control this
    for (i, bech32) in NostrBech32::find_all_in_string(&app.dm_draft_data.draft)
        .iter()
        .enumerate()
    {
        let pk = match bech32 {
            NostrBech32::Pubkey(pk) => pk,
            NostrBech32::Profile(prof) => &prof.pubkey,
            _ => continue,
        };
        let rendered = if let Ok(Some(person)) = GLOBALS.storage.read_person(pk) {
            match person.name() {
                Some(name) => name.to_owned(),
                None => format!("{}", bech32),
            }
        } else {
            format!("{}", bech32)
        };

        ui.label(format!("{}: {}", i, rendered));
    }
}

fn real_posting_area(app: &mut GossipUi, ctx: &Context, frame: &mut eframe::Frame, ui: &mut Ui) {
    // Maybe render post we are replying to or reposting

    let compose_area_id: egui::Id = egui::Id::new("compose_area");
    let mut send_now: bool = false;

    let screen_rect = ctx.input(|i| i.screen_rect);
    let window_height = screen_rect.max.y - screen_rect.min.y;

    app.vert_scroll_area()
        .max_height(window_height * 0.7)
        .show(ui, |ui| {
            if let Some(id) = app.draft_data.replying_to.or(app.draft_data.repost) {
                CollapsingHeader::new("Replying to:")
                    .default_open(true)
                    .show(ui, |ui| {
                        super::note::render_note(
                            app,
                            ctx,
                            frame,
                            ui,
                            FeedNoteParams {
                                id,
                                indent: 0,
                                as_reply_to: true,
                                threaded: false,
                                is_first: true,
                                is_last: true,
                            },
                        );
                    });
            }

            if app.draft_data.repost.is_none() {
                // Text area
                let theme = app.theme;
                let mut layouter = |ui: &Ui, text: &str, wrap_width: f32| {
                    let interests = app
                        .draft_data
                        .replacements
                        .keys()
                        .cloned()
                        .collect::<Vec<String>>();

                    let mut layout_job = textarea_highlighter(theme, text.to_owned(), interests);
                    layout_job.wrap.max_width = wrap_width;

                    ui.fonts(|f| f.layout_job(layout_job))
                };

                if app.draft_data.include_subject && app.draft_data.replying_to.is_none() {
                    ui.horizontal(|ui| {
                        ui.label("Subject: ");
                        ui.add(
                            text_edit_line!(app, app.draft_data.subject)
                                .hint_text("Type subject here")
                                .desired_width(f32::INFINITY),
                        );
                    });
                    ui.add_space(10.0);
                }

                if app.draft_data.include_content_warning {
                    ui.horizontal(|ui| {
                        ui.label("Content Warning: ");
                        ui.add(
                            text_edit_line!(app, app.draft_data.content_warning)
                                .hint_text("Type content warning here")
                                .desired_width(f32::INFINITY),
                        );
                    });
                    ui.add_space(10.0);
                }

                // if we are tagging, we will consume arrow presses and enter key
                let enter_key;
                (app.draft_data.tagging_search_selected, enter_key) =
                    if app.draft_data.tagging_search_substring.is_some() {
                        widgets::capture_keyboard_for_search(
                            ui,
                            app.draft_data.tagging_search_results.len(),
                            app.draft_data.tagging_search_selected,
                        )
                    } else {
                        (None, false)
                    };

                let text_edit_area = if app.draft_data.raw.is_empty() {
                    text_edit_multiline!(app, app.draft_data.draft)
                        .id(compose_area_id)
                        .hint_text("Type your message here. Type an '@' followed by a search string to tag someone.")
                        .desired_width(f32::INFINITY)
                        .lock_focus(true)
                        .interactive(app.draft_data.repost.is_none())
                        .layouter(&mut layouter)
                } else {
                    text_edit_multiline!(app, app.draft_data.raw)
                        .id(compose_area_id.with("_raw"))
                        .desired_width(f32::INFINITY)
                        .interactive(false)
                        .layouter(&mut layouter)
                };
                let mut output = text_edit_area.show(ui);

                if app.draft_needs_focus {
                    output.response.request_focus();
                    app.draft_needs_focus = false;
                }

                if output.response.has_focus() && !app.draft_data.draft.is_empty() {
                    let modifiers = if cfg!(target_os = "macos") {
                        Modifiers {
                            command: true,
                            ..Default::default()
                        }
                    } else {
                        Modifiers {
                            ctrl: true,
                            ..Default::default()
                        }
                    };

                    if ui.input_mut(|i| i.consume_key(modifiers, Key::Enter)) {
                        send_now = true;
                    }
                }

                // Determine if we are in tagging mode
                if output.response.changed() {
                    app.draft_data.tagging_search_substring = None;
                    let text_edit_state =
                        egui::TextEdit::load_state(ctx, compose_area_id).unwrap_or_default();
                    let ccursor_range = text_edit_state.ccursor_range().unwrap_or_default();
                    let cpos = ccursor_range.primary.index;
                    if cpos <= app.draft_data.draft.len() {
                        if let Some(captures) =
                            GLOBALS.tagging_regex.captures(&app.draft_data.draft)
                        {
                            if let Some(mat) = captures.get(1) {
                                // cursor must be within match
                                if cpos >= mat.start() && cpos <= mat.end() {
                                    // only search if this is not already a replacement
                                    if !app.draft_data.replacements.contains_key(
                                        &app.draft_data.draft[mat.start() - 1..mat.end()],
                                    ) {
                                        app.draft_data.tagging_search_substring =
                                            Some(mat.as_str().to_owned());
                                    }
                                }
                            }
                        }
                    }
                }

                // show tag hovers first, as they depend on information in the output.galley
                // do not run them after replacements are added, rather wait for the next frame
                if output.response.changed()
                    || app.draft_data.replacements_changed
                    || app.draft_data.last_textedit_rect != output.response.rect
                {
                    calc_tag_hovers(ui, app, &output);
                    app.draft_data.replacements_changed = false;
                }
                show_tag_hovers(ui, app, &mut output);

                calc_tagging_search(app);
                show_tagging_result(ui, app, &mut output, enter_key);

                app.draft_data.last_textedit_rect = output.response.rect;
            }

            ui.add_space(8.0);
        });

    ui.horizontal(|ui| {
        let send_label = if app.draft_data.repost.is_some() {
            "Repost note"
        } else {
            "Send note"
        };

        if app.draft_data.raw.is_empty() {
            // show advanced action menu
            if app.draft_data.repost.is_none() {
                widgets::MoreMenu::bubble(ui, app)
                    .with_max_size(vec2(180.0, 80.0))
                    .with_min_size(vec2(180.0, 80.0))
                    .place_above(!read_setting!(posting_area_at_top))
                    .show(ui, |ui, is_open| {
                        ui.vertical_centered_justified(|ui| {
                            app.theme.primary_button_style(ui.style_mut());
                            if app.draft_data.include_subject {
                                if ui.button("Remove Subject").clicked() {
                                    app.draft_data.include_subject = false;
                                    app.draft_data.subject = "".to_owned();
                                }
                            } else if app.draft_data.replying_to.is_none()
                                && ui.button("Add Subject").clicked()
                            {
                                app.draft_data.include_subject = true;
                                *is_open = false;
                            }

                            ui.add_space(10.0);

                            if app.draft_data.include_content_warning {
                                if ui.button("Remove Content Warning").clicked() {
                                    app.draft_data.include_content_warning = false;
                                    app.draft_data.content_warning = "".to_owned();
                                    *is_open = false;
                                }
                            } else if ui.button("Add Content Warning").clicked() {
                                app.draft_data.include_content_warning = true;
                                *is_open = false;
                            }

                            ui.add_space(10.0);

                            ui.set_enabled(!app.draft_data.replacements.is_empty());
                            if ui.button("Show raw preview").clicked() {
                                let raw = do_replacements(
                                    &app.draft_data.draft,
                                    &app.draft_data.replacements,
                                );
                                app.draft_data.raw = raw.to_owned();
                                *is_open = false;
                            }
                        });
                    });
            }

            ui.add_space(7.0);

            ui.horizontal(|ui| {
                ui.visuals_mut().hyperlink_color = ui.visuals().text_color();
                if ui.link("Cancel").clicked() {
                    app.reset_draft();
                }
            });

            ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                ui.add_space(12.0);

                ui.horizontal(|ui| {
                    app.theme.primary_button_style(ui.style_mut());
                    if ui.button(send_label).clicked()
                        && (!app.draft_data.draft.is_empty() || app.draft_data.repost.is_some())
                    {
                        send_now = true;
                    }
                });

                ui.add_space(7.0);

                if app.draft_data.repost.is_none() {
                    // Emoji picker
                    ui.menu_button(RichText::new("😀▼").size(14.0), |ui| {
                        if let Some(emoji) = crate::ui::components::emoji_picker(ui) {
                            app.draft_data.draft.push(emoji);
                        }
                    });
                }
            });
        } else {
            // raw preview
            ui.with_layout(Layout::top_down_justified(Align::Center), |ui| {
                ui.visuals_mut().hyperlink_color = ui.visuals().text_color();
                if ui.link("Go back to edit mode").clicked() {
                    app.draft_data.raw = "".to_owned();
                }
            });

            ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                ui.add_space(12.0);
                app.theme.primary_button_style(ui.style_mut());
                if ui.button(send_label).clicked()
                    && (!app.draft_data.draft.is_empty() || app.draft_data.repost.is_some())
                {
                    send_now = true;
                }
            });
        }
    });

    if send_now {
        let replaced = do_replacements(&app.draft_data.draft, &app.draft_data.replacements);

        let mut tags: Vec<Tag> = Vec::new();
        if app.draft_data.include_content_warning {
            tags.push(Tag::new_content_warning(&app.draft_data.content_warning));
        }
        if let Some(delegatee_tag) = GLOBALS.delegation.get_delegatee_tag() {
            tags.push(delegatee_tag);
        }
        if app.draft_data.include_subject {
            tags.push(Tag::new_subject(app.draft_data.subject.clone()));
        }
        match app.draft_data.replying_to {
            Some(replying_to_id) => {
                let _ = GLOBALS.to_overlord.send(ToOverlordMessage::Post {
                    content: replaced,
                    tags,
                    in_reply_to: Some(replying_to_id),
                    dm_channel: None,
                });
            }
            None => {
                if let Some(event_id) = app.draft_data.repost {
                    let _ = GLOBALS
                        .to_overlord
                        .send(ToOverlordMessage::Repost(event_id));
                } else {
                    let _ = GLOBALS.to_overlord.send(ToOverlordMessage::Post {
                        content: replaced,
                        tags,
                        in_reply_to: None,
                        dm_channel: None,
                    });
                }
            }
        }

        app.reset_draft();
    }

    // List tags that will be applied
    // FIXME: list tags from parent event too in case of reply
    // FIXME: tag handling in overlord::post() needs to move back here so the user can control this
    for (i, bech32) in NostrBech32::find_all_in_string(&app.draft_data.draft)
        .iter()
        .enumerate()
    {
        let pk = match bech32 {
            NostrBech32::Pubkey(pk) => pk,
            NostrBech32::Profile(prof) => &prof.pubkey,
            _ => continue,
        };
        let rendered = if let Ok(Some(person)) = GLOBALS.storage.read_person(pk) {
            match person.name() {
                Some(name) => name.to_owned(),
                None => format!("{}", bech32),
            }
        } else {
            format!("{}", bech32)
        };

        ui.label(format!("{}: {}", i, rendered));
    }
}

fn calc_tagging_search(app: &mut GossipUi) {
    // show tagging slector tooltip
    if let Some(search) = &app.draft_data.tagging_search_substring {
        // only do the search when search string changes
        if app.draft_data.tagging_search_substring != app.draft_data.tagging_search_searched {
            let mut pairs = GLOBALS
                .people
                .search_people_to_tag(search)
                .unwrap_or_default();
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
            app.draft_data.tagging_search_searched = Some(search.clone());
            app.draft_data.tagging_search_results = pairs.to_owned();
        }
    }
}

fn show_tagging_result(
    ui: &mut Ui,
    app: &mut GossipUi,
    output: &mut TextEditOutput,
    enter_key: bool,
) {
    let above_or_below = if read_setting!(posting_area_at_top) {
        AboveOrBelow::Below
    } else {
        AboveOrBelow::Above
    };
    let mut selected = app.draft_data.tagging_search_selected;
    widgets::show_contact_search(
        ui,
        app,
        above_or_below,
        output,
        &mut selected,
        app.draft_data.tagging_search_results.clone(),
        enter_key,
        |ui, app, output, pair| {
            // remove @ and search text
            let search = if let Some(search) = app.draft_data.tagging_search_searched.as_ref() {
                search.clone()
            } else {
                "".to_string()
            };

            // complete name and add replacement
            let name = pair.0.clone();
            let nostr_url: NostrUrl = pair.1.into();
            app.draft_data.draft = app
                .draft_data
                .draft
                .as_str()
                .replace(&format!("@{}", search), name.as_str())
                .to_string();

            // move cursor to end of replacement
            if let Some(pos) = app.draft_data.draft.find(name.as_str()) {
                let cpos = pos + name.len();
                let mut state = output.state.clone();
                let mut ccrange = CCursorRange::default();
                ccrange.primary.index = cpos;
                ccrange.secondary.index = cpos;
                state.set_ccursor_range(Some(ccrange));
                state.store(ui.ctx(), output.response.id);

                // add it to our replacement list
                app.draft_data
                    .replacements
                    .insert(name, ContentSegment::NostrUrl(nostr_url));
                app.draft_data.replacements_changed = true;

                // clear tagging search
                app.draft_data.tagging_search_substring = None;
            }
        },
    );

    app.draft_data.tagging_search_selected = selected;

    if app.draft_data.tagging_search_substring.is_none() {
        // no more search substring, clear results
        app.draft_data.tagging_search_searched = None;
        app.draft_data.tagging_search_results.clear();
    }
}

fn calc_tag_hovers(ui: &mut Ui, app: &mut GossipUi, output: &TextEditOutput) {
    let mut hovers: HashMap<Id, Box<dyn InformationPopup>> = HashMap::new();

    // find replacements in the galley and interact with them
    for (pat, content) in app.draft_data.replacements.clone() {
        for (pos, pat) in output.galley.job.text.match_indices(&pat) {
            let popup_id = ui.auto_id_with(pos);
            // find the rect that covers the replacement
            let ccstart = CCursor::new(pos);
            let ccend = CCursor::new(pos + pat.len());
            let mut cstart = output.galley.from_ccursor(ccstart);
            cstart.pcursor.prefer_next_row = true;
            let cend = output.galley.from_ccursor(ccend);
            let start_rect = output.galley.pos_from_cursor(&cstart);
            let end_rect = output.galley.pos_from_cursor(&cend);
            let interact_rect = egui::Rect::from_two_pos(
                output.text_draw_pos + start_rect.left_top().to_vec2(),
                output.text_draw_pos + end_rect.right_bottom().to_vec2(),
            );

            if let ContentSegment::NostrUrl(nostr_url) = &content {
                let maybe_pubkey = match &nostr_url.0 {
                    NostrBech32::Profile(p) => Some(p.pubkey),
                    NostrBech32::Pubkey(pk) => Some(*pk),
                    NostrBech32::EventAddr(_)
                    | NostrBech32::EventPointer(_)
                    | NostrBech32::Id(_)
                    | NostrBech32::Relay(_) => None,
                };

                if let Some(pubkey) = maybe_pubkey {
                    let avatar = if let Some(avatar) = app.try_get_avatar(ui.ctx(), &pubkey) {
                        avatar
                    } else {
                        app.placeholder_avatar.clone()
                    };

                    // create popup and store it
                    if let Ok(Some(person)) = GLOBALS.storage.read_person(&pubkey) {
                        let popup = Box::new(
                            widgets::ProfilePopup::new(popup_id, interact_rect, avatar, person)
                                .show_duration(1.0)
                                .tag(pat.to_owned()),
                        );

                        hovers.insert(popup_id, popup);
                    }
                }
            }
        }
    }

    // insert the hovers for this textedit
    if let Some(entry) = app.popups.get_mut(&output.response.id) {
        *entry = hovers;
    } else {
        app.popups.insert(output.response.id, hovers);
    }
}

fn show_tag_hovers(ui: &mut Ui, app: &mut GossipUi, output: &mut TextEditOutput) {
    if let Some(hovers) = app.popups.get_mut(&output.response.id) {
        let uitime = ui.input(|i| i.time);
        let mut deletelist: Vec<(egui::Id, String)> = Vec::new();
        for (id, popup) in &mut *hovers {
            let resp = ui.interact(popup.interact_rect(), id.with("_h"), egui::Sense::hover());
            if resp.hovered() {
                popup.set_last_seen(uitime);
            }
            if resp.hovered() || popup.get_until() > Some(uitime) {
                let above_or_below = if read_setting!(posting_area_at_top) {
                    AboveOrBelow::Below
                } else {
                    AboveOrBelow::Above
                };
                let response = popup.show(ui, above_or_below, Box::new(|ui| ui.link("remove")));

                // pointer over the popup extends its life
                if let Some(pointer_pos) = ui.ctx().pointer_latest_pos() {
                    if response.response.rect.contains(pointer_pos) {
                        popup.set_last_seen(uitime);
                    }
                }

                // 'remove' button clicked
                if response.inner.clicked() {
                    if let Some(tag) = popup.tag() {
                        deletelist.push((*id, tag.clone()));
                    }
                }
            }
        }
        for (_, tag) in deletelist {
            app.draft_data.replacements.remove(&tag);

            // re-calculate hovers
            calc_tag_hovers(ui, app, output);

            // mark textedit changed
            output.response.mark_changed();
        }
    }
}

fn do_replacements(draft: &str, replacements: &HashMap<String, ContentSegment>) -> String {
    let mut output = draft.to_owned();
    for (pat, content) in replacements {
        if let ContentSegment::NostrUrl(nostr_url) = content {
            output = output
                .as_str()
                .replace(pat, format!("nostr:{}", nostr_url.0).as_str())
                .to_string();
        }
    }
    output
}
