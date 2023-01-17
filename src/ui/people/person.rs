use super::{GossipUi, Page};
use crate::comms::ToOverlordMessage;
use crate::db::DbPerson;
use crate::feed::FeedKind;
use crate::globals::GLOBALS;
use crate::AVATAR_SIZE_F32;
use eframe::egui;
use egui::{Context, RichText, Ui, Vec2};

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    let (pubkeyhex, maybe_person) = match &app.page {
        Page::Person(pubkeyhex) => {
            let maybe_person = GLOBALS.people.get(pubkeyhex);
            (pubkeyhex.to_owned(), maybe_person)
        }
        _ => {
            ui.label("ERROR");
            return;
        }
    };

    ui.add_space(24.0);

    if let Some(person) = &maybe_person {
        ui.heading(get_name(person));
    } else {
        ui.heading(&pubkeyhex.0);
    }

    ui.horizontal(|ui| {
        // Avatar first
        let avatar = if let Some(avatar) = app.try_get_avatar(ctx, &pubkeyhex) {
            avatar
        } else {
            app.placeholder_avatar.clone()
        };
        ui.image(&avatar, Vec2 { x: AVATAR_SIZE_F32 * 3.0, y: AVATAR_SIZE_F32 * 3.0 });
        ui.vertical(|ui| {
            ui.label(RichText::new(GossipUi::hex_pubkey_short(&pubkeyhex)).weak());
            GossipUi::render_person_name_line(ui, maybe_person.as_ref());
        });
    });

    ui.add_space(12.0);

    if let Some(person) = &maybe_person {
        if let Some(about) = person.about.as_deref() {
            ui.label(about);
        }
    }

    ui.add_space(12.0);

    if let Some(person) = &maybe_person {
        #[allow(clippy::collapsible_else_if)]
        if maybe_person.is_none() || person.followed == 0 {
            if ui.button("FOLLOW").clicked() {
                GLOBALS.people.follow(&pubkeyhex, true);
            }
        } else {
            if ui.button("UNFOLLOW").clicked() {
                GLOBALS.people.follow(&pubkeyhex, false);
            }
        }
    }

    if ui.button("UPDATE METADATA").clicked() {
        let _ = GLOBALS
            .to_overlord
            .send(ToOverlordMessage::UpdateMetadata(pubkeyhex.clone()));
    }

    if ui.button("VIEW THEIR FEED").clicked() {
        app.set_page(Page::Feed(FeedKind::Person(pubkeyhex.clone())));
    }
}

fn get_name(person: &DbPerson) -> String {
    if let Some(name) = &person.name {
        name.to_owned()
    } else {
        GossipUi::hex_pubkey_short(&person.pubkey)
    }
}
