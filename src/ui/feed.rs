use super::GossipUi;
use eframe::egui;
use egui::{Align, Color32, Context, Layout, RichText, ScrollArea, TextStyle, Ui, Vec2};
use nostr_proto::PublicKey;
use tracing::info;

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    let feed = crate::globals::blocking_get_feed(true);

    //let screen_rect = ctx.input().screen_rect; // Rect

    ui.horizontal(|ui| {
        ui.text_edit_multiline(&mut app.draft);

        if ui.button("Send").clicked() {
            info!("Would send: {}", app.draft);
            app.draft = "".to_owned();

            // We need our private key
            // Then we need to create a TextNote event
            // Then we need to send it to multiple relays
            // NOT a one-liner
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

            let maybe_fevent = crate::globals::GLOBALS
                .feed_events
                .blocking_lock()
                .get(id)
                .cloned();
            if maybe_fevent.is_none() {
                continue;
            }
            let fevent = maybe_fevent.unwrap();

            if fevent.event.is_none() {
                continue;
            } // don't render related info w/o nostr event.
            let event = fevent.event.as_ref().unwrap().to_owned();

            let maybe_person = crate::globals::GLOBALS
                .people
                .blocking_lock()
                .get(&event.pubkey)
                .cloned();

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
                // Avatar first
                ui.image(&app.placeholder_avatar, Vec2 { x: 36.0, y: 36.0 });

                // Everything else next
                ui.vertical(|ui| {
                    // First row
                    ui.horizontal(|ui| {
                        if let Some(person) = maybe_person {
                            if let Some(name) = &person.name {
                                ui.label(
                                    RichText::new(name).text_style(TextStyle::Name("Bold".into())),
                                );
                            }
                        }

                        ui.separator();

                        ui.label(pubkey_short(&event.pubkey));

                        ui.with_layout(Layout::right_to_left(Align::TOP), |ui| {
                            ui.label(
                                RichText::new(crate::date_ago::date_ago(event.created_at))
                                    .text_style(TextStyle::Name("Oblique".into()))
                                    .weak(),
                            );
                        });
                    });

                    // Second row
                    ui.horizontal(|ui| {
                        if fevent.reactions.upvotes > 0 {
                            ui.label(
                                RichText::new(&format!("+{}", fevent.reactions.upvotes))
                                    .text_style(TextStyle::Name("Bold".into()))
                                    .color(Color32::DARK_GREEN),
                            );
                        }
                        if fevent.reactions.downvotes > 0 {
                            ui.label(
                                RichText::new(&format!("-{}", fevent.reactions.downvotes))
                                    .text_style(TextStyle::Name("Bold".into()))
                                    .color(Color32::DARK_RED),
                            );
                        }
                    });

                    ui.label(&event.content);
                });
            });

            ui.separator();
        }
    });
}

fn pubkey_short(pubkey: &PublicKey) -> String {
    let hex = pubkey.as_hex_string();
    format!(
        "{}_{}...{}_{}",
        &hex[0..4],
        &hex[4..8],
        &hex[56..60],
        &hex[60..64]
    )
}
