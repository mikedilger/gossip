use super::{GossipUi, Page};
use crate::comms::BusMessage;
use crate::db::DbPerson;
use crate::globals::GLOBALS;
use eframe::egui;
use egui::{Context, RichText, Ui, Vec2};

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    let maybe_person = if let Some(pubkeyhex) = &app.person_view_pubkey {
        GLOBALS.people.blocking_write().get(pubkeyhex)
    } else {
        None
    };

    if maybe_person.is_none() || app.person_view_pubkey.is_none() {
        ui.label("ERROR");
    } else {
        let person = maybe_person.as_ref().unwrap();
        let pubkeyhex = app.person_view_pubkey.as_ref().unwrap().clone();

        ui.add_space(24.0);

        ui.heading(get_name(person));

        ui.horizontal(|ui| {
            // Avatar first
            let avatar = if let Some(avatar) = app.try_get_avatar(ctx, &pubkeyhex) {
                avatar
            } else {
                app.placeholder_avatar.clone()
            };
            ui.image(&avatar, Vec2 { x: 36.0, y: 36.0 });

            ui.vertical(|ui| {
                ui.label(RichText::new(GossipUi::hex_pubkey_short(&pubkeyhex)).weak());
                GossipUi::render_person_name_line(ui, Some(person));
            });
        });

        ui.add_space(12.0);

        if let Some(about) = person.about.as_deref() {
            ui.label(about);
        }

        ui.add_space(12.0);

        #[allow(clippy::collapsible_else_if)]
        if person.followed == 0 {
            if ui.button("FOLLOW").clicked() {
                GLOBALS.people.blocking_write().follow(&pubkeyhex, true);
            }
        } else {
            if ui.button("UNFOLLOW").clicked() {
                GLOBALS.people.blocking_write().follow(&pubkeyhex, false);
            }
        }

        if ui.button("UPDATE METADATA").clicked() {
            let _ = GLOBALS.to_overlord.send(BusMessage {
                target: "overlord".to_string(),
                kind: "update_metadata".to_string(),
                json_payload: serde_json::to_string(&pubkeyhex).unwrap(),
            });
        }

        if ui.button("VIEW THEIR FEED").clicked() {
            GLOBALS.feed.set_feed_to_person(pubkeyhex.clone());
            app.page = Page::FeedPerson;
        }
    }
}

fn get_name(person: &DbPerson) -> String {
    if let Some(name) = &person.name {
        name.to_owned()
    } else {
        GossipUi::hex_pubkey_short(&person.pubkey)
    }
}
