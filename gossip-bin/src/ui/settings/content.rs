use crate::ui::GossipUi;
use eframe::egui;
use egui::widgets::Slider;
use egui::{Context, Ui};

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.heading("Content");

    ui.add_space(10.0);
    ui.heading("Feed Settings");
    ui.add_space(10.0);

    ui.horizontal(|ui| {
        ui.label("Feed Chunk: ").on_hover_text("This is the amount of time backwards from now that we will load events from. You'll eventually be able to load more, one chunk at a time. Mostly takes effect on restart.");
        ui.add(Slider::new(&mut app.settings.feed_chunk, 600..=86400).text("seconds, "));
        ui.label(secs_to_string(app.settings.feed_chunk));
    });

    ui.horizontal(|ui| {
        ui.label("Replies Chunk: ").on_hover_text("This is the amount of time backwards from now that we will load replies, mentions, and DMs from. You'll eventually be able to load more, one chunk at a time. Mostly takes effect on restart.");
        ui.add(Slider::new(&mut app.settings.replies_chunk, 86400..=2592000).text("seconds, "));
        ui.label(secs_to_string(app.settings.replies_chunk));
    });

    ui.horizontal(|ui| {
        ui.label("Overlap: ").on_hover_text("If we recently loaded events up to time T, but restarted, we will now load events starting from time T minus overlap. Takes effect on restart. I recommend 300 (5 minutes).");
        ui.add(Slider::new(&mut app.settings.overlap, 0..=3600).text("seconds, "));
        ui.label(secs_to_string(app.settings.overlap));
    });

    ui.checkbox(
        &mut app.settings.recompute_feed_periodically,
        "Recompute feed periodically. If this is off, you will get a refresh button",
    );

    ui.horizontal(|ui| {
        ui.label("Recompute feed every (milliseconds): ").on_hover_text("The UI redraws frequently. We recompute the feed less frequently to conserve CPU. Takes effect when the feed next recomputes. I recommend 3500.");
        ui.add(Slider::new(&mut app.settings.feed_recompute_interval_ms, 1000..=12000).text("milliseconds"));
    });

    ui.add_space(10.0);
    ui.heading("Event Selection Settings");
    ui.add_space(10.0);

    ui.checkbox(
        &mut app.settings.reactions,
        "Enable reactions (show and react)",
    );

    ui.checkbox(&mut app.settings.enable_zap_receipts, "Enable zap receipts");

    ui.checkbox(&mut app.settings.reposts, "Enable reposts (show)");

    ui.checkbox(&mut app.settings.direct_messages, "Show Direct Messages")
        .on_hover_text("Takes effect fully only on restart.");

    ui.checkbox(&mut app.settings.show_long_form, "Show Long-Form Posts")
        .on_hover_text("Takes effect fully only on restart.");

    ui.add_space(10.0);
    ui.heading("Event Content Settings");
    ui.add_space(10.0);

    ui.checkbox(&mut app.settings.show_mentions, "Render mentions inline")
        .on_hover_text(if app.settings.show_mentions {
            "Disable to just show a link to a mentioned post where it appears in the text"
        } else {
            "Enable to render a mentioned post where it appears in the text"
        });

    ui.checkbox(&mut app.settings.show_media, "Render all media inline automatically").on_hover_text("If off, you have to click to (potentially fetch and) render media inline. If on, all media referenced by posts in your feed will be (potentially fetched and) rendered. However, if Fetch Media is disabled, only cached media can be shown as media will not be fetched.");
    ui.checkbox(&mut app.settings.approve_content_warning, "Approve all content-warning tagged media automatically")
        .on_hover_text("If off, you have to click to show content-warning tagged media. If on, all content-warning tagged media in your feed will be rendered.");
    ui.checkbox(
        &mut app.settings.show_deleted_events,
        "Render delete events, but labeled as deleted",
    )
    .on_hover_text(if app.settings.show_deleted_events {
        "Disable to exclude all deleted events from rendering"
    } else {
        "Enable to show all deleted events, but labeled as deleted"
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
