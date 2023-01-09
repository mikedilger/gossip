use super::GossipUi;
use crate::comms::ToOverlordMessage;
use crate::GLOBALS;
use eframe::egui;
use egui::widgets::{Button, Slider};
use egui::{Align, Context, Layout, Ui};

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.heading("Settings");

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(12.0);

    ui.heading("How Many Relays to Query");

    ui.horizontal(|ui| {
        ui.label("Number of relays to query per person: ").on_hover_text("We will query N relays per person. Many people share the same relays so those will be queried about multiple people. Takes affect on restart.");
        ui.add(Slider::new(&mut app.settings.num_relays_per_person, 1..=5).text("relays"));
    });

    ui.horizontal(|ui| {
        ui.label("Maximum total number of relays to query: ")
            .on_hover_text(
                "We will not connect to more than this many relays. Takes affect on restart.",
            );
        ui.add(Slider::new(&mut app.settings.max_relays, 1..=30).text("relays"));
    });

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(12.0);

    ui.heading("How Many Posts to Load");

    ui.horizontal(|ui| {
        ui.label("Feed Chunk: ").on_hover_text("This is the amount of time backwards from now that we will load events from. You'll eventually be able to load more, one chunk at a time. Mostly takes effect on restart.");
        ui.add(Slider::new(&mut app.settings.feed_chunk, 600..=86400).text("seconds, "));
        ui.label(secs_to_string(app.settings.feed_chunk));
    });

    ui.horizontal(|ui| {
        ui.label("Overlap: ").on_hover_text("If we recently loaded events up to time T, but restarted, we will now load events starting from time T minus overlap. Takes effect on restart.");
        ui.add(Slider::new(&mut app.settings.overlap, 0..=3600).text("seconds, "));
        ui.label(secs_to_string(app.settings.overlap));
    });

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(12.0);

    ui.heading("Feed");

    ui.horizontal(|ui| {
        ui.label("Recompute feed every (milliseconds): ")
            .on_hover_text(
                "The UI redraws frequently. We recompute the feed less frequently to conserve CPU. Takes effect when the feed next recomputes.",
            );
        ui.add(Slider::new(&mut app.settings.feed_recompute_interval_ms, 250..=5000).text("milliseconds"));
    });

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(12.0);

    ui.heading("What Posts to Include");

    ui.checkbox(
        &mut app.settings.view_posts_referred_to,
        "View posts referred to by people you follow (not yet implemented)",
    )
    .on_hover_text(
        "Recommended, otherwise it's hard to understand what the person is talking about.",
    );

    ui.checkbox(&mut app.settings.view_posts_referring_to, "View posts referring to posts by people you follow (not yet implemented)")
        .on_hover_text("Not recommended, as anyone can reply to them and you'll certainly encounter spam this way.");

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(12.0);

    ui.heading("Posting");

    ui.horizontal(|ui| {
        ui.label("Proof of Work: ")
            .on_hover_text("The larger the number, the longer it takes.");
        ui.add(Slider::new(&mut app.settings.pow, 0..=40).text("leading zeroes"));
    });

    ui.add_space(12.0);

    ui.checkbox(
        &mut app.settings.set_client_tag,
        "Add tag [\"client\",\"gossip\"] to posts",
    )
    .on_hover_text("Takes effect immediately.");

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(12.0);

    ui.heading("Network");

    ui.checkbox(&mut app.settings.offline, "Offline Mode")
        .on_hover_text("If selected, no network requests will be issued. Takes effect on restart.");

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(12.0);

    ui.heading("User Interface");

    ui.horizontal(|ui| {
        ui.label("Switch to");
        #[allow(clippy::collapsible_else_if)]
        if app.settings.light_mode {
            if ui
                .add(Button::new("🌙 Dark"))
                .on_hover_text("Switch to dark mode")
                .clicked()
            {
                ui.ctx().set_visuals(super::style::dark_mode_visuals());
                app.settings.light_mode = false;
            }
        } else {
            if ui
                .add(Button::new("☀ Light"))
                .on_hover_text("Switch to light mode")
                .clicked()
            {
                ui.ctx().set_visuals(super::style::light_mode_visuals());
                app.settings.light_mode = true;
            }
        }
    });

    ui.add_space(12.0);
    ui.horizontal(|ui| {
        ui.label("Maximum FPS: ")
            .on_hover_text(
                "The UI redraws every frame. By limiting the maximum FPS you can reduce load on your CPU. Takes effect immediately.",
            );
        ui.add(Slider::new(&mut app.settings.max_fps, 10..=60).text("Frames per second"));
    });

    ui.add_space(12.0);
    ui.separator();
    ui.add_space(24.0);

    ui.with_layout(Layout::top_down(Align::Center), |ui| {
        if ui.button("SAVE CHANGES").clicked() {
            // Copy local settings to global settings
            *GLOBALS.settings.blocking_write() = app.settings.clone();

            // Tell the overlord to save them
            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::SaveSettings);
        }
    });
}

fn secs_to_string(secs: u64) -> String {
    let days = secs / 86400;
    let remainder = secs % 86400;
    let hours = remainder / 3600;
    let remainder = remainder % 3600;
    let minutes = remainder / 60;
    let seconds = remainder % 60;
    let mut output: String = String::new();
    if days > 0 {
        output.push_str(&format!(" {} days", days));
    }
    if hours > 0 {
        output.push_str(&format!(" {} hours", hours));
    }
    if minutes > 0 {
        output.push_str(&format!(" {} minutes", minutes));
    }
    output.push_str(&format!(" {} seconds", seconds));
    output
}
