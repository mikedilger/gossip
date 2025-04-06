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
        ui.label("Load How Many More: ")
            .on_hover_text("The number of events to load when you press Load More.");
        ui.add(Slider::new(&mut app.unsaved_settings.load_more_count, 10..=100).text("events"));
        reset_button!(app, ui, load_more_count);
    });

    ui.horizontal(|ui| {
        ui.checkbox(
            &mut app.unsaved_settings.recompute_feed_periodically,
            "Recompute feed periodically. If this is off, you will get a refresh button",
        );
        reset_button!(app, ui, recompute_feed_periodically);
    });

    ui.horizontal(|ui| {
        ui.label("Recompute feed every: ").on_hover_text("The UI redraws frequently. We recompute the feed less frequently to conserve CPU. Takes effect when the feed next recomputes. I recommend 3500.");
        ui.add(Slider::new(&mut app.unsaved_settings.feed_recompute_interval_ms, 1000..=12000).text("milliseconds"));
        reset_button!(app, ui, feed_recompute_interval_ms);
    });

    ui.horizontal(|ui| {
        ui.checkbox(
            &mut app.unsaved_settings.feed_thread_scroll_to_main_event,
            "Initially scroll to the highlighted note when entering a Thread",
        );
        reset_button!(app, ui, feed_thread_scroll_to_main_event);
    });

    ui.add_space(10.0);
    ui.heading("Event Selection Settings");
    ui.add_space(10.0);

    ui.horizontal(|ui| {
        ui.checkbox(
            &mut app.unsaved_settings.reactions,
            "Enable reactions (show and react)",
        );
        reset_button!(app, ui, reactions);
    });

    ui.horizontal(|ui| {
        ui.checkbox(
            &mut app.unsaved_settings.enable_zap_receipts,
            "Enable zap receipts",
        );
        reset_button!(app, ui, enable_zap_receipts);
    });

    ui.horizontal(|ui| {
        ui.checkbox(
            &mut app.unsaved_settings.enable_picture_events,
            "Enable picture events",
        );
        reset_button!(app, ui, enable_picture_events);
    });

    ui.horizontal(|ui| {
        ui.checkbox(
            &mut app.unsaved_settings.enable_comments,
            "Enable comment events",
        );
        reset_button!(app, ui, enable_comments);
    });

    ui.horizontal(|ui| {
        ui.checkbox(&mut app.unsaved_settings.reposts, "Enable reposts (show)");
        reset_button!(app, ui, reposts);
    });

    ui.horizontal(|ui| {
        ui.checkbox(
            &mut app.unsaved_settings.direct_messages,
            "Show Direct Messages",
        )
        .on_hover_text("Takes effect fully only on restart.");
        reset_button!(app, ui, direct_messages);
    });

    ui.horizontal(|ui| {
        ui.checkbox(
            &mut app.unsaved_settings.show_long_form,
            "Show Long-Form Posts",
        )
        .on_hover_text("Takes effect fully only on restart.");
        reset_button!(app, ui, show_long_form);
    });

    ui.add_space(10.0);
    ui.heading("Spam Settings");
    ui.add_space(10.0);

    ui.horizontal(|ui| {
        ui.checkbox(
            &mut app.unsaved_settings.avoid_spam_on_unsafe_relays,
            "Avoid spam from unsafe relays (SpamSafe)",
        )
            .on_hover_text("Unless a relay is marked as SpamSafe, replies and mentions will only be pulled from people you follow. Takes effect fully only on restart.");
        reset_button!(app, ui, avoid_spam_on_unsafe_relays);
    });

    ui.horizontal(|ui| {
        ui.checkbox(
            &mut app.unsaved_settings.limit_inbox_seeking_to_inbox_relays,
            "Limit inbox events to events that came from your inbox relays",
        )
            .on_hover_text("By default gossip looks for replies in your inbox relays, but also on other relays the parent was seen on and where any referencing event was seen. With this setting, it won't do that, which may help avoid spam. It also constructs your inbox feed only from events that came from your inbox relays.");
        reset_button!(app, ui, limit_inbox_seeking_to_inbox_relays);
    });

    ui.horizontal(|ui| {
        ui.checkbox(
            &mut app.unsaved_settings.apply_spam_filter_on_incoming_events,
            "Apply spam filtering script to incoming events",
        )
            .on_hover_text("Your filter.rhai script (if it exists) will be run to filter out spam as events flow into gossip");
        reset_button!(app, ui, apply_spam_filter_on_incoming_events);
    });

    ui.horizontal(|ui| {
        ui.checkbox(
            &mut app.unsaved_settings.apply_spam_filter_on_threads,
            "Apply spam filtering script to thread replies",
        )
            .on_hover_text(
                "Your filter.rhai script (if it exists) will be run to filter out spam in thread replies",
            );
        reset_button!(app, ui, apply_spam_filter_on_threads);
    });

    ui.horizontal(|ui| {
        ui.checkbox(
            &mut app.unsaved_settings.apply_spam_filter_on_inbox,
            "Apply spam filtering script to inbox",
        )
        .on_hover_text(
            "Your filter.rhai script (if it exists) will be run to filter out spam in your inbox",
        );
        reset_button!(app, ui, apply_spam_filter_on_inbox);
    });

    ui.horizontal(|ui| {
        ui.checkbox(
            &mut app.unsaved_settings.apply_spam_filter_on_global,
            "Apply spam filtering script to the global feed",
        )
            .on_hover_text(
                "Your filter.rhai script (if it exists) will be run to filter out spam in the global feed",
            );
        reset_button!(app, ui, apply_spam_filter_on_global);
    });

    ui.add_space(10.0);
    ui.heading("Event Content Settings");
    ui.add_space(10.0);

    ui.horizontal(|ui| {
        ui.checkbox(
            &mut app.unsaved_settings.show_mentions,
            "Render mentions inline",
        )
        .on_hover_text(if app.unsaved_settings.show_mentions {
            "Disable to just show a link to a mentioned post where it appears in the text"
        } else {
            "Enable to render a mentioned post where it appears in the text"
        });
        reset_button!(app, ui, show_mentions);
    });

    ui.horizontal(|ui| {
        ui.checkbox(&mut app.unsaved_settings.show_media, "Render all media inline automatically").on_hover_text("If off, you have to click to (potentially fetch and) render media inline. If on, all media referenced by posts in your feed will be (potentially fetched and) rendered. However, if Fetch Media is disabled, only cached media can be shown as media will not be fetched.");
        reset_button!(app, ui, show_media);
    });

    ui.horizontal(|ui| {
        ui.checkbox(&mut app.unsaved_settings.approve_content_warning, "Approve all content-warning tagged media automatically")
            .on_hover_text("If off, you have to click to show content-warning tagged media. If on, all content-warning tagged media in your feed will be rendered.");
        reset_button!(app, ui, approve_content_warning);
    });

    ui.horizontal(|ui| {
        ui.checkbox(
            &mut app.unsaved_settings.hide_mutes_entirely,
            "Hide muted events entirely, including replies to them",
        )
            .on_hover_text("If on, muted events wont be in the feed at all. If off, they will be in the feed, but the content will be replaced with the word MUTED. You will see replies to them, and you can peek at the content by viewing the note in raw form.");
        reset_button!(app, ui, hide_mutes_entirely);
    });

    ui.horizontal(|ui| {
        ui.checkbox(
            &mut app.unsaved_settings.show_deleted_events,
            "Render deleted events, but labeled as deleted",
        )
        .on_hover_text(if app.unsaved_settings.show_deleted_events {
            "Disable to exclude all deleted events from rendering"
        } else {
            "Enable to show all deleted events, but labeled as deleted"
        });
        reset_button!(app, ui, show_deleted_events);
    });

    ui.add_space(20.0);
}
