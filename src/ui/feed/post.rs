use super::FeedNoteParams;
use crate::comms::ToOverlordMessage;
use crate::globals::GLOBALS;
use crate::tags::{keys_from_text, notes_from_text};
use crate::ui::{GossipUi, HighlightType, Page, Theme};
use eframe::egui;
use eframe::epaint::text::LayoutJob;
use egui::{Align, Context, Layout, RichText, ScrollArea, TextEdit, Ui, Vec2};
use memoize::memoize;
use nostr_types::Tag;

#[memoize]
pub fn textarea_highlighter(theme: Theme, text: String) -> LayoutJob {
    let mut job = LayoutJob::default();

    let ids = notes_from_text(&text);
    let pks = keys_from_text(&text);

    // we will gather indices such that we can split the text in chunks
    let mut indices: Vec<(usize, HighlightType)> = vec![];
    for pk in pks {
        for m in text.match_indices(&pk.0) {
            indices.push((m.0, HighlightType::Nothing));
            indices.push((m.0 + pk.0.len(), HighlightType::PublicKey));
        }
    }
    for id in ids {
        for m in text.match_indices(&id.0) {
            indices.push((m.0, HighlightType::Nothing));
            indices.push((m.0 + id.0.len(), HighlightType::Event));
        }
    }
    indices.sort_by_key(|x| x.0);
    indices.dedup_by_key(|x| x.0);

    // add a breakpoint at the end if it doesn't exist
    if indices.is_empty() || indices[indices.len() - 1].0 != text.len() {
        indices.push((text.len(), HighlightType::Nothing));
    }

    // now we will add each chunk back to the textarea with custom formatting
    let mut curr = 0;
    for (index, highlight) in indices {
        let chunk = &text[curr..index];

        job.append(chunk, 0.0, theme.highlight_text_format(highlight));

        curr = index;
    }

    job
}

pub(super) fn posting_area(
    app: &mut GossipUi,
    ctx: &Context,
    frame: &mut eframe::Frame,
    ui: &mut Ui,
) {
    // Posting Area
    ui.vertical(|ui| {
        if !GLOBALS.signer.is_ready() {
            ui.horizontal_wrapped(|ui| {
                ui.label("You need to ");
                if ui.link("setup your identity").clicked() {
                    app.set_page(Page::YourKeys);
                }
                ui.label(" to post.");
            });
        } else if !GLOBALS.all_relays.iter().any(|r| r.value().write) {
            ui.horizontal_wrapped(|ui| {
                ui.label("You need to ");
                if ui.link("choose write relays").clicked() {
                    app.set_page(Page::RelaysAll);
                }
                ui.label(" to post.");
            });
        } else {
            real_posting_area(app, ctx, frame, ui);
        }
    });
}

fn real_posting_area(app: &mut GossipUi, ctx: &Context, frame: &mut eframe::Frame, ui: &mut Ui) {
    // Maybe render post we are replying to
    if let Some(id) = app.replying_to {
        ScrollArea::vertical()
            .max_height(200.0)
            .override_scroll_delta(Vec2 {
                x: 0.0,
                y: app.current_scroll_offset,
            })
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

    // Text area
    let theme = app.settings.theme;
    let mut layouter = |ui: &Ui, text: &str, wrap_width: f32| {
        let mut layout_job = textarea_highlighter(theme, text.to_owned());
        layout_job.wrap.max_width = wrap_width;
        ui.fonts(|f| f.layout_job(layout_job))
    };

    if app.include_subject && app.replying_to.is_none() {
        ui.horizontal(|ui| {
            ui.label("Subject: ");
            ui.add(
                TextEdit::singleline(&mut app.subject)
                    .text_color(app.settings.theme.input_text_color())
                    .hint_text("Type subject here")
                    .desired_width(f32::INFINITY),
            );
        });
    }

    if app.include_content_warning {
        ui.horizontal(|ui| {
            ui.label("Content Warning: ");
            ui.add(
                TextEdit::singleline(&mut app.content_warning)
                    .text_color(app.settings.theme.input_text_color())
                    .hint_text("Type content warning here")
                    .desired_width(f32::INFINITY),
            );
        });
    }

    ui.add(
        TextEdit::multiline(&mut app.draft)
            .text_color(app.settings.theme.input_text_color())
            .hint_text("Type your message here")
            .desired_width(f32::INFINITY)
            .lock_focus(true)
            .layouter(&mut layouter),
    );

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
            ui.add_space(12.0);
            if ui.button("Send").clicked() && !app.draft.is_empty() {
                match app.replying_to {
                    Some(replying_to_id) => {
                        let _ = GLOBALS.to_overlord.send(ToOverlordMessage::Post(
                            app.draft.clone(),
                            vec![],
                            Some(replying_to_id),
                        ));
                    }
                    None => {
                        let mut tags: Vec<Tag> = Vec::new();
                        if app.include_subject {
                            tags.push(Tag::Subject(app.subject.clone()));
                        }
                        if app.include_content_warning {
                            tags.push(Tag::ContentWarning(app.content_warning.clone()));
                        }
                        if let Some(delegatee_tag) = GLOBALS.delegation.get_delegatee_tag() {
                            tags.push(delegatee_tag);
                        }
                        let _ = GLOBALS.to_overlord.send(ToOverlordMessage::Post(
                            app.draft.clone(),
                            tags,
                            None,
                        ));
                    }
                }
                app.clear_post();
            }

            if ui.button("Cancel").clicked() {
                app.clear_post();
            }

            ui.add(
                TextEdit::singleline(&mut app.tag_someone)
                    .text_color(app.settings.theme.input_text_color())
                    .desired_width(100.0)
                    .hint_text("@username"),
            );
            if !app.tag_someone.is_empty() {
                let pairs = GLOBALS.people.search_people_to_tag(&app.tag_someone);
                if !pairs.is_empty() {
                    ui.menu_button("@", |ui| {
                        for pair in pairs {
                            if ui.button(pair.0).clicked() {
                                if !app.draft.ends_with(' ') && !app.draft.is_empty() {
                                    app.draft.push(' ');
                                }
                                app.draft.push_str(&pair.1.try_as_bech32_string().unwrap());
                                app.tag_someone = "".to_owned();
                            }
                        }
                    });
                }
            }

            if app.include_subject {
                if ui.button("Remove Subject").clicked() {
                    app.include_subject = false;
                    app.subject = "".to_owned();
                }
            } else if app.replying_to.is_none() && ui.button("Add Subject").clicked() {
                app.include_subject = true;
            }

            if app.include_content_warning {
                if ui.button("Remove Content Warning").clicked() {
                    app.include_content_warning = false;
                    app.content_warning = "".to_owned();
                }
            } else if ui.button("Add Content Warning").clicked() {
                app.include_content_warning = true;
            }

            // Emoji picker
            ui.menu_button(RichText::new("😀▼").size(14.0), |ui| {
                for emoji in "😀😁😆😅🤣😕😯👀❤💜👍🤙💯🎯🤌🙏🤝🫂⚡🍆".chars()
                {
                    if ui.button(emoji.to_string()).clicked() {
                        app.draft.push(emoji);
                    }
                }
            });
        });
    });

    // List tags that will be applied (FIXME: list tags from parent event too in case of reply)
    for (i, (npub, pubkey)) in keys_from_text(&app.draft).iter().enumerate() {
        let rendered = if let Some(person) = GLOBALS.people.get(&pubkey.as_hex_string().into()) {
            match person.name() {
                Some(name) => name.to_owned(),
                None => npub.to_owned(),
            }
        } else {
            npub.to_owned()
        };

        ui.label(format!("{}: {}", i, rendered));
    }
}
