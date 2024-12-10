use super::GossipUi;
use eframe::egui;
use egui::{Context, RichText, Ui};
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
        let follows = match GLOBALS.follows.try_read() {
            Some(follows) => follows,
            None => {
                ui.label("Busy counting...");
                return;
            }
        };

        let who = match follows.who {
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

        let count = follows.set.len();
        ui.heading(format!("{} People Followed", count));

        let height: f32 = 48.0;

        app.vert_scroll_area()
            .show_rows(ui, height, follows.set.len(), |ui, range| {
                for follow_sortable_pubkey in follows
                    .set
                    .iter()
                    .skip(range.start)
                    .take(range.end - range.start)
                {
                    let follow_pubkey = (*follow_sortable_pubkey).into();
                    let follow_person = match PersonTable::read_record(follow_pubkey, None) {
                        Ok(Some(p)) => p,
                        _ => Person::new(follow_pubkey.to_owned()),
                    };
                    super::render_person_line(app, ctx, ui, follow_person)
                }
            });
    });
}
