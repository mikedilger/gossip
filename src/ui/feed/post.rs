use super::FeedPostParams;
use crate::comms::ToOverlordMessage;
use crate::globals::GLOBALS;
use crate::tags::{keys_from_text, textarea_highlighter};
use crate::ui::{GossipUi, Page};
use eframe::egui;
use egui::{Align, Color32, Context, Layout, RichText, ScrollArea, TextEdit, Ui};
use nostr_types::Tag;

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
        } else if !GLOBALS.relays.blocking_read().iter().any(|(_, r)| r.write) {
            ui.horizontal_wrapped(|ui| {
                ui.label("You need to ");
                if ui.link("choose relays").clicked() {
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
        ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
            super::render_post_actual(
                app,
                ctx,
                frame,
                ui,
                FeedPostParams {
                    id,
                    indent: 0,
                    as_reply_to: true,
                    threaded: false,
                },
            );
        });
    }

    // Text area
    let mut layouter = |ui: &Ui, text: &str, wrap_width: f32| {
        let mut layout_job = textarea_highlighter(text.to_owned(), ui.visuals().dark_mode);
        layout_job.wrap.max_width = wrap_width;
        ui.fonts().layout_job(layout_job)
    };

    if app.include_subject && app.replying_to.is_none() {
        ui.horizontal(|ui| {
            ui.label("Subject: ");
            ui.add(
                TextEdit::singleline(&mut app.subject)
                    .hint_text("Type subject here")
                    .text_color(if ui.visuals().dark_mode {
                        Color32::WHITE
                    } else {
                        Color32::BLACK
                    })
                    .desired_width(f32::INFINITY),
            );
        });
    }

    ui.add(
        TextEdit::multiline(&mut app.draft)
            .hint_text("Type your message here")
            .desired_width(f32::INFINITY)
            .lock_focus(true)
            .layouter(&mut layouter),
    );

    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
            if ui.button("Send").clicked() && !app.draft.is_empty() {
                match app.replying_to {
                    Some(replying_to_id) => {
                        let _ = GLOBALS.to_overlord.send(ToOverlordMessage::PostReply(
                            app.draft.clone(),
                            vec![],
                            replying_to_id,
                        ));
                    }
                    None => {
                        let mut tags: Vec<Tag> = Vec::new();
                        if app.include_subject {
                            tags.push(Tag::Subject(app.subject.clone()));
                        }
                        let _ = GLOBALS
                            .to_overlord
                            .send(ToOverlordMessage::PostTextNote(app.draft.clone(), tags));
                    }
                }
                app.clear_post();
            }

            if ui.button("Cancel").clicked() {
                app.clear_post();
            }

            ui.add(
                TextEdit::singleline(&mut app.tag_someone)
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
