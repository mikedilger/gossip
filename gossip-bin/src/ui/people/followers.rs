use super::GossipUi;
use crate::ui::{widgets, Page};
use eframe::egui;
use egui::{Context, Label, RichText, Sense, Ui};
use gossip_lib::{Person, PersonTable, Table, GLOBALS};
use nostr_types::PublicKey;

pub(super) fn update(
    app: &mut GossipUi,
    ctx: &Context,
    _frame: &mut eframe::Frame,
    ui: &mut Ui,
    pubkey: PublicKey,
) {
    let person = match PersonTable::read_record(pubkey, None) {
        Ok(Some(p)) => p,
        _ => Person::new(pubkey.to_owned()),
    };

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.add_space(10.0);
        ui.label(
            RichText::new(person.best_name())
                .size(22.0)
                .color(app.theme.accent_color()),
        );
    });

    ui.add_space(5.0);

    ui.vertical(|ui| {
        let followers = match GLOBALS.followers.try_read() {
            Some(followers) => followers,
            None => {
                ui.label("Busy counting...");
                return;
            }
        };

        let who = match followers.who {
            Some(who) => who,
            None => {
                ui.label("NOT TRACKING ANYONE BUG");
                return;
            }
        };

        if who != pubkey {
            ui.label("MISMATCH BUG");
            return;
        }

        let count = followers.set.len();
        ui.heading(format!("{} Followers", count));

        let height: f32 = 48.0;

        app.vert_scroll_area()
            .show_rows(ui, height, followers.set.len(), |ui, range| {
                for follower_sortable_pubkey in followers
                    .set
                    .iter()
                    .skip(range.start)
                    .take(range.end - range.start)
                {
                    let follower_pubkey = (*follower_sortable_pubkey).into();
                    let follower_person = match PersonTable::read_record(follower_pubkey, None) {
                        Ok(Some(p)) => p,
                        _ => Person::new(follower_pubkey.to_owned()),
                    };
                    render_person_line(app, ctx, ui, follower_person);
                }
            });
    });
}

fn render_person_line(app: &mut GossipUi, ctx: &Context, ui: &mut Ui, person: Person) {
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
                    widgets::paint_avatar(ui, &person, &avatar, widgets::AvatarSize::Feed);

                ui.add_space(20.0);

                let response = ui
                    .vertical(|ui| {
                        ui.add_space(5.0);
                        ui.horizontal(|ui| {
                            response |= ui.add(
                                Label::new(RichText::new(person.best_name()).size(15.5))
                                    .selectable(false)
                                    .sense(Sense::click()),
                            );
                        });

                        ui.add_space(3.0);
                        response |= ui.add(
                            Label::new(GossipUi::richtext_from_person_nip05(&person).weak())
                                .selectable(false)
                                .sense(Sense::click()),
                        );
                        response
                    })
                    .inner;

                // This is just to force the background all the way across
                ui.vertical(|ui| {
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Min)
                            .with_cross_align(egui::Align::Center),
                        |_| {},
                    );
                });

                response
            })
            .inner
        },
    );

    // test what the height is
    // println!("HEIGHT = {}", row_response.inner.rect.height());

    if row_response
        .inner
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
    {
        app.set_page(ctx, Page::Person(person.pubkey));
    }
}
