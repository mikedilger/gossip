use super::GossipUi;
use crate::globals::{Globals, GLOBALS};
use eframe::egui;
use egui::{Align, Color32, Context, Layout, RichText, ScrollArea, TextStyle, Ui, Vec2};
use nostr_types::{EventKind, Id};
use tracing::info;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, frame: &mut eframe::Frame, ui: &mut Ui) {
    let feed = Globals::blocking_get_feed(true);

    //let screen_rect = ctx.input().screen_rect; // Rect

    ui.horizontal(|ui| {
        ui.text_edit_multiline(&mut app.draft);

        if ui.button("Send").clicked() && !app.draft.is_empty() {
            info!("Would send: {}", app.draft);

            // We need our private key
            // Then we need to create a TextNote event
            // Then we need to send it to multiple relays
            // NOT a one-liner

            app.draft = "".to_owned();
        }
    });

    ScrollArea::vertical().show(ui, |ui| {
        for id in feed.iter() {
            // Stop rendering at the bottom of the window:
            // (This confuses the scrollbar a bit, so I'm taking it out for now)
            //let pos2 = ui.next_widget_position();
            //if pos2.y > screen_rect.max.y {
            //    break;
            //}

            render_post(app, ctx, frame, ui, *id, 0);
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
) {
    let maybe_event = GLOBALS.events.blocking_lock().get(&id).cloned();
    if maybe_event.is_none() {
        return;
    }
    let event = maybe_event.unwrap();

    // Only render TextNote events
    if event.kind != EventKind::TextNote {
        return;
    }

    let maybe_person = GLOBALS.people.blocking_lock().get(&event.pubkey).cloned();

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

    ui.horizontal(|ui| {
        // Indents first (if threaded)
        if app.settings.view_threaded {
            for _ in 0..indent {
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);
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
                        ui.label(RichText::new(name).text_style(TextStyle::Name("Bold".into())));
                    }
                }

                ui.separator();

                ui.label(RichText::new(GossipUi::pubkey_short(&event.pubkey)).weak());

                ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                    ui.label(
                        RichText::new(crate::date_ago::date_ago(event.created_at))
                            .text_style(TextStyle::Name("Oblique".into()))
                            .weak(),
                    );

                    ui.add_space(10.0);

                    ui.label("(i)").on_hover_ui(|ui| {
                        ui.label(&format!("ID: {}", event.id.as_hex_string()));
                    });
                });
            });

            // Second row
            ui.horizontal(|ui| {
                for (ch, count) in reactions.iter() {
                    if *ch == '+' {
                        ui.label(
                            RichText::new(format!("{} {}", ch, count))
                                .text_style(TextStyle::Name("Bold".into()))
                                .color(Color32::DARK_GREEN),
                        );
                    } else if *ch == '-' {
                        ui.label(
                            RichText::new(format!("{} {}", ch, count))
                                .text_style(TextStyle::Name("Bold".into()))
                                .color(Color32::DARK_RED),
                        );
                    } else {
                        ui.label(
                            RichText::new(format!("{} {}", ch, count))
                                .text_style(TextStyle::Name("Bold".into())),
                        );
                    }
                }
            });

            ui.label(&event.content);
        });
    });

    ui.separator();

    if app.settings.view_threaded {
        for reply_id in replies {
            render_post(app, _ctx, _frame, ui, reply_id, indent + 1);
        }
    }
}
