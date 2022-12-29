use super::{GossipUi, Page};
use crate::comms::BusMessage;
use crate::globals::{Globals, GLOBALS};
use crate::ui::widgets::{CopyButton, ReplyButton};
use eframe::egui;
use egui::{Align, Color32, Context, Layout, RichText, ScrollArea, TextEdit, TextStyle, Ui, Vec2};
use nostr_types::{EventKind, Id};

pub(super) fn update(app: &mut GossipUi, ctx: &Context, frame: &mut eframe::Frame, ui: &mut Ui) {
    let feed = GLOBALS.feed.blocking_lock().get();

    //let screen_rect = ctx.input().screen_rect; // Rect

    let desired_count = {
        Globals::trim_desired_events_sync();
        GLOBALS.desired_events.blocking_read().len()
    };

    ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
        ui.with_layout(Layout::top_down(Align::Max), |ui| {
            if ui
                .button(&format!("Get {} missing events", desired_count))
                .clicked()
            {
                let tx = GLOBALS.to_overlord.clone();
                let _ = tx.send(BusMessage {
                    target: "overlord".to_string(),
                    kind: "get_missing_events".to_string(),
                    json_payload: serde_json::to_string("").unwrap(),
                });
            }
        });
    });

    ui.vertical(|ui| {
        if !GLOBALS.signer.blocking_read().is_ready() {
            ui.horizontal(|ui| {
                ui.label("You need to ");
                if ui.link("setup your identity").clicked() {
                    app.page = Page::You;
                }
                ui.label(" to post.");
            });
        } else if !GLOBALS.relays.blocking_read().iter().any(|(_, r)| r.post) {
            ui.horizontal(|ui| {
                ui.label("You need to ");
                if ui.link("choose relays").clicked() {
                    app.page = Page::Relays;
                }
                ui.label(" to post.");
            });
        } else {
            if let Some(id) = app.replying_to {
                render_post(app, ctx, frame, ui, id, 0, true);
            }

            ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                if ui.button("Send").clicked() && !app.draft.is_empty() {
                    let tx = GLOBALS.to_overlord.clone();
                    match app.replying_to {
                        Some(_id) => {
                            let _ = tx.send(BusMessage {
                                target: "overlord".to_string(),
                                kind: "post_reply".to_string(),
                                json_payload: serde_json::to_string(&(
                                    &app.draft,
                                    &app.replying_to,
                                ))
                                .unwrap(),
                            });
                        }
                        None => {
                            let _ = tx.send(BusMessage {
                                target: "overlord".to_string(),
                                kind: "post_textnote".to_string(),
                                json_payload: serde_json::to_string(&app.draft).unwrap(),
                            });
                        }
                    }
                    app.draft = "".to_owned();
                    app.replying_to = None;
                }
                if ui.button("Cancel").clicked() {
                    app.draft = "".to_owned();
                    app.replying_to = None;
                }

                ui.add(
                    TextEdit::multiline(&mut app.draft)
                        .hint_text("Type your message here")
                        .desired_width(f32::INFINITY)
                        .lock_focus(true),
                );
            });
        }
    });

    ui.separator();

    ScrollArea::vertical().show(ui, |ui| {
        for id in feed.iter() {
            // Stop rendering at the bottom of the window:
            // (This confuses the scrollbar a bit, so I'm taking it out for now)
            //let pos2 = ui.next_widget_position();
            //if pos2.y > screen_rect.max.y {
            //    break;
            //}

            render_post(app, ctx, frame, ui, *id, 0, false);
        }
    });
}

fn render_post(
    app: &mut GossipUi,
    _ctx: &Context,
    _frame: &mut eframe::Frame,
    ui: &mut Ui,
    id: Id,
    indent: usize,
    as_reply_to: bool,
) {
    let maybe_event = GLOBALS.events.blocking_read().get(&id).cloned();
    if maybe_event.is_none() {
        return;
    }
    let event = maybe_event.unwrap();

    // Only render TextNote events
    if event.kind != EventKind::TextNote {
        return;
    }

    let maybe_person = GLOBALS.people.blocking_read().get(&event.pubkey).cloned();

    let reactions = Globals::get_reactions_sync(event.id);
    let replies = Globals::get_replies_sync(event.id);

    // Person Things we can render:
    // pubkey
    // name
    // about
    // picture
    // dns_id
    // dns_id_valid
    // dns_id_last_checked
    // metadata_at
    // followed

    // Event Things we can render:
    // id
    // pubkey
    // created_at,
    // kind,
    // tags,
    // content,
    // ots,
    // sig
    // feed_related,
    // replies,
    // in_reply_to,
    // reactions,
    // deleted_reason,
    // client,
    // hashtags,
    // subject,
    // urls,
    // last_reply_at

    // Try LayoutJob

    let threaded = GLOBALS.settings.blocking_read().view_threaded;

    ui.horizontal(|ui| {
        // Indents first (if threaded)
        if threaded {
            let space = 16.0 * (10.0 - (100.0 / (indent as f32 + 10.0)));
            ui.add_space(space);
            if indent > 0 {
                ui.separator();
            }
        }

        // Avatar first
        ui.image(&app.placeholder_avatar, Vec2 { x: 36.0, y: 36.0 });

        // Everything else next
        ui.vertical(|ui| {
            // First row
            ui.horizontal(|ui| {
                if let Some(person) = maybe_person {
                    if let Some(name) = &person.name {
                        ui.label(RichText::new(name).strong());
                    } else {
                        ui.label(RichText::new(GossipUi::pubkey_short(&event.pubkey)).weak());
                    }
                }

                ui.add_space(8.0);

                ui.label(RichText::new("🔑").text_style(TextStyle::Small).weak());
                if ui.add(CopyButton {}).clicked() {
                    ui.output().copied_text = GossipUi::pubkey_long(&event.pubkey);
                }

                ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                    ui.menu_button(RichText::new("≡").size(28.0), |ui| {
                        if ui.button("Copy ID").clicked() {
                            ui.output().copied_text = event.id.as_hex_string();
                        }
                        if ui.button("Dismiss").clicked() {
                            GLOBALS.dismissed.blocking_write().push(event.id);
                        }
                    });

                    ui.label(
                        RichText::new(crate::date_ago::date_ago(event.created_at))
                            .italics()
                            .weak(),
                    );
                });
            });

            // Second row
            ui.horizontal(|ui| {
                for (ch, count) in reactions.iter() {
                    if *ch == '+' {
                        ui.label(
                            RichText::new(format!("{} {}", ch, count))
                                .strong()
                                .color(Color32::DARK_GREEN),
                        );
                    } else if *ch == '-' {
                        ui.label(
                            RichText::new(format!("{} {}", ch, count))
                                .strong()
                                .color(Color32::DARK_RED),
                        );
                    } else {
                        ui.label(RichText::new(format!("{} {}", ch, count)).strong());
                    }
                }
            });

            ui.label(&event.content);

            // Under row
            if !as_reply_to {
                ui.horizontal(|ui| {
                    if ui.add(CopyButton {}).clicked() {
                        ui.output().copied_text = event.content.clone();
                    }

                    ui.add_space(24.0);

                    if ui.add(ReplyButton {}).clicked() {
                        app.replying_to = Some(event.id);
                    }
                });
            }
        });
    });

    ui.separator();

    if threaded && !as_reply_to {
        for reply_id in replies {
            render_post(app, _ctx, _frame, ui, reply_id, indent + 1, as_reply_to);
        }
    }
}
