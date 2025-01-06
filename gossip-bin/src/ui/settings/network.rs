use crate::ui::{GossipUi, Page};
use eframe::egui;
use egui::widgets::Slider;
use egui::{Context, Ui};

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.heading("Network Settings");

    ui.add_space(10.0);

    ui.horizontal(|ui| {
        ui.checkbox(&mut app.unsaved_settings.offline, "Offline Mode")
            .on_hover_text(
                "If selected, no network requests will be issued. Takes effect on restart.",
            );
        reset_button!(app, ui, offline);
    });

    ui.horizontal(|ui| {
        ui.checkbox(&mut app.unsaved_settings.load_avatars, "Fetch Avatars").on_hover_text("If disabled, avatars will not be fetched, but cached avatars will still display. Takes effect on save.");
        reset_button!(app, ui, load_avatars);
    });

    ui.horizontal(|ui| {
        ui.checkbox(&mut app.unsaved_settings.load_media, "Fetch Media").on_hover_text("If disabled, no new media will be fetched, but cached media will still display. Takes effect on save.");
        reset_button!(app, ui, load_media);
    });

    ui.horizontal(|ui| {
        ui.checkbox(&mut app.unsaved_settings.check_nip05, "Check NIP-05").on_hover_text("If disabled, NIP-05 fetches will not be performed, but existing knowledge will be preserved, and following someone by NIP-05 will override this and do the fetch. Takes effect on save.");
        reset_button!(app, ui, check_nip05);
    });

    ui.horizontal(|ui| {
        ui.checkbox(&mut app.unsaved_settings.automatically_fetch_metadata, "Automatically Fetch Metadata").on_hover_text("If enabled, metadata that is entirely missing will be fetched as you scroll past people. Existing metadata won't be updated. Takes effect on save.");
        reset_button!(app, ui, automatically_fetch_metadata);
    });

    ui.horizontal(|ui| {
        ui.checkbox(
            &mut app.unsaved_settings.relay_connection_requires_approval,
            "Require user approval before connecting to new relays",
        );
        reset_button!(app, ui, relay_connection_requires_approval);
    });

    ui.horizontal(|ui| {
        ui.checkbox(
            &mut app.unsaved_settings.relay_auth_requires_approval,
            "Require user approval before AUTHenticating to a relay for the first time",
        );
        reset_button!(app, ui, relay_auth_requires_approval);
    });

    ui.add_space(10.0);
    ui.heading("Relay Settings");
    ui.add_space(10.0);

    ui.horizontal(|ui| {
        ui.label("Manage individual relays on the");
        if ui.link("Relays > Configure").clicked() {
            app.set_page(ctx, Page::RelaysKnownNetwork(None));
        }
        ui.label("page.");
    });

    ui.horizontal(|ui| {
        ui.label("Number of relays to query per person: ").on_hover_text("We will query N relays per person. Many people share the same relays so those will be queried about multiple people. Takes affect on restart. I recommend 2. Too many and gossip will (currently) keep connecting to new relays trying to find the unfindable, loading many events from each. Takes effect on restart.");
        ui.add(Slider::new(&mut app.unsaved_settings.num_relays_per_person, 1..=4).text("relays"));
        reset_button!(app, ui, num_relays_per_person);
    });

    ui.horizontal(|ui| {
        ui.label("Maximum following feed relays: ").on_hover_text("We will not stay connected to more than this many relays for following feed. Takes affect on restart. During these early days of nostr, I recommend capping this at around 20 to 30.");
        ui.add(Slider::new(&mut app.unsaved_settings.max_relays, 5..=100).text("relays"));
        reset_button!(app, ui, max_relays);
    });

    ui.horizontal(|ui| {
        ui.label("Number of relays to query when counting things: ")
            .on_hover_text("We will pick the N best relays we can find to do this.");
        ui.add(
            Slider::new(&mut app.unsaved_settings.num_relays_for_counting, 5..=100).text("relays"),
        );
        reset_button!(app, ui, num_relays_for_counting);
    });

    ui.add_space(10.0);
    ui.heading("HTTP Fetch Settings");
    ui.add_space(10.0);

    ui.horizontal(|ui| {
        ui.label("HTTP Connect Timeout");
        ui.add(
            Slider::new(
                &mut app.unsaved_settings.fetcher_connect_timeout_sec,
                5..=120,
            )
            .text("seconds"),
        );
        reset_button!(app, ui, fetcher_connect_timeout_sec);
    });
    ui.horizontal(|ui| {
        ui.label("HTTP Idle Timeout");
        ui.add(Slider::new(&mut app.unsaved_settings.fetcher_timeout_sec, 5..=120).text("seconds"));
        reset_button!(app, ui, fetcher_timeout_sec);
    });
    ui.horizontal(|ui| {
        ui.label("Max simultaneous HTTP requests per remote host")
            .on_hover_text(
                "If you set this too high, you may start getting 403-Forbidden or \
                 429-TooManyRequests errors from the remote host",
            );
        ui.add(
            Slider::new(
                &mut app.unsaved_settings.fetcher_max_requests_per_host,
                1..=10,
            )
            .text("requests"),
        );
        reset_button!(app, ui, fetcher_max_requests_per_host);
    });
    ui.horizontal(|ui| {
        ui.label("How long to avoid contacting a host after a minor error");
        ui.add(
            Slider::new(
                &mut app
                    .unsaved_settings
                    .fetcher_host_exclusion_on_low_error_secs,
                10..=60,
            )
            .text("seconds"),
        );
        reset_button!(app, ui, fetcher_host_exclusion_on_low_error_secs);
    });
    ui.horizontal(|ui| {
        ui.label("How long to avoid contacting a host after a medium error");
        ui.add(
            Slider::new(
                &mut app
                    .unsaved_settings
                    .fetcher_host_exclusion_on_med_error_secs,
                20..=180,
            )
            .text("seconds"),
        );
        reset_button!(app, ui, fetcher_host_exclusion_on_med_error_secs);
    });
    ui.horizontal(|ui| {
        ui.label("How long to avoid contacting a host after a major error");
        ui.add(
            Slider::new(
                &mut app
                    .unsaved_settings
                    .fetcher_host_exclusion_on_high_error_secs,
                60..=1800,
            )
            .text("seconds"),
        );
        reset_button!(app, ui, fetcher_host_exclusion_on_high_error_secs);
    });

    ui.add_space(10.0);
    ui.heading("Websocket Settings");
    ui.add_space(10.0);

    ui.horizontal(|ui| {
        ui.label("Maximum websocket message size");
        ui.add(
            Slider::new(
                &mut app.unsaved_settings.max_websocket_message_size_kb,
                256..=4096,
            )
            .text("KiB"),
        );
        reset_button!(app, ui, max_websocket_message_size_kb);
    });
    ui.horizontal(|ui| {
        ui.label("Maximum websocket frame size");
        ui.add(
            Slider::new(
                &mut app.unsaved_settings.max_websocket_frame_size_kb,
                256..=4096,
            )
            .text("KiB"),
        );
        reset_button!(app, ui, max_websocket_frame_size_kb);
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut app.unsaved_settings.websocket_accept_unmasked_frames, "Accept unmasked websocket frames?").on_hover_text("This is contrary to the standard, but some incorrect software/libraries may use unmasked frames.");
        reset_button!(app, ui, websocket_accept_unmasked_frames);
    });
    ui.horizontal(|ui| {
        ui.label("Websocket Connect Timeout");
        ui.add(
            Slider::new(
                &mut app.unsaved_settings.websocket_connect_timeout_sec,
                5..=120,
            )
            .text("seconds"),
        );
        reset_button!(app, ui, websocket_connect_timeout_sec);
    });
    ui.horizontal(|ui| {
        ui.label("Websocket Ping Frequency");
        ui.add(
            Slider::new(
                &mut app.unsaved_settings.websocket_ping_frequency_sec,
                30..=600,
            )
            .text("seconds"),
        );
        reset_button!(app, ui, websocket_ping_frequency_sec);
    });

    ui.add_space(10.0);
    ui.heading("Stale Time Settings");
    ui.add_space(10.0);

    ui.horizontal(|ui| {
        ui.label("How long before a relay list becomes stale and needs rechecking?");
        ui.add(
            Slider::new(
                &mut app.unsaved_settings.relay_list_becomes_stale_minutes,
                5..=360,
            )
            .text("minutes"),
        );
        reset_button!(app, ui, relay_list_becomes_stale_minutes);
    });
    ui.horizontal(|ui| {
        ui.label("How long before metadata becomes stale and needs rechecking?");
        ui.add(
            Slider::new(
                &mut app.unsaved_settings.metadata_becomes_stale_minutes,
                5..=360,
            )
            .text("minutes"),
        );
        reset_button!(app, ui, metadata_becomes_stale_minutes);
    });
    ui.horizontal(|ui| {
        ui.label("How long before valid nip05 becomes stale and needs rechecking?");
        ui.add(
            Slider::new(
                &mut app.unsaved_settings.nip05_becomes_stale_if_valid_hours,
                1..=24,
            )
            .text("hours"),
        );
        reset_button!(app, ui, nip05_becomes_stale_if_valid_hours);
    });
    ui.horizontal(|ui| {
        ui.label("How long before invalid nip05 becomes stale and needs rechecking?");
        ui.add(
            Slider::new(
                &mut app.unsaved_settings.nip05_becomes_stale_if_invalid_minutes,
                5..=600,
            )
            .text("minutes"),
        );
        reset_button!(app, ui, nip05_becomes_stale_if_invalid_minutes);
    });
    ui.horizontal(|ui| {
        ui.label("How long before an avatar image becomes stale and needs rechecking?");
        ui.add(
            Slider::new(&mut app.unsaved_settings.avatar_becomes_stale_hours, 1..=24).text("hours"),
        );
        reset_button!(app, ui, avatar_becomes_stale_hours);
    });
    ui.horizontal(|ui| {
        ui.label("How long before event media becomes stale and needs rechecking?");
        ui.add(
            Slider::new(&mut app.unsaved_settings.media_becomes_stale_hours, 1..=24).text("hours"),
        );
        reset_button!(app, ui, media_becomes_stale_hours);
    });

    ui.add_space(20.0);
}
