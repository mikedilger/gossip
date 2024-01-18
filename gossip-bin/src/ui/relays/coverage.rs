use crate::ui::{
    widgets::{
        self,
        list_entry::{self, draw_text_at, TEXT_LEFT, TEXT_RIGHT, TEXT_TOP, TITLE_FONT_SIZE},
        COPY_SYMBOL_SIZE,
    },
    GossipUi, Page, SettingsTab,
};
use egui_winit::egui::{self, vec2, Align, Context, Id, Response, RichText, Ui};
use gossip_lib::{comms::ToOverlordMessage, GLOBALS};
use nostr_types::{PublicKey, RelayUrl};

const COVERAGE_ENTRY_HEIGHT: f32 = 2.0 * TEXT_TOP + 1.5 * TITLE_FONT_SIZE + 14.0;

struct CoverageEntry<'a> {
    pk: &'a PublicKey,
    _count: &'a usize,
    relays: Vec<RelayUrl>,
    name: String,
}

impl<'a> CoverageEntry<'a> {
    pub(super) fn new(
        pk: &'a PublicKey,
        name: String,
        _count: &'a usize,
        relays: Vec<RelayUrl>,
    ) -> Self {
        Self {
            pk,
            _count,
            relays,
            name,
        }
    }

    fn make_id(&self, str: &str) -> Id {
        (self.pk.as_hex_string() + str).into()
    }

    pub(super) fn show(&self, ui: &mut Ui, app: &mut GossipUi) -> Response {
        let available_width = ui.available_size_before_wrap().x;
        let (rect, response) = ui.allocate_exact_size(
            vec2(available_width, COVERAGE_ENTRY_HEIGHT),
            egui::Sense::click(),
        );

        widgets::list_entry::paint_frame(ui, &rect, Some(app.theme.main_content_bgcolor()));

        // ---- title ----
        let pos = rect.min + vec2(TEXT_LEFT, TEXT_TOP);
        draw_text_at(
            ui,
            pos,
            RichText::new(self.name.clone())
                .size(list_entry::TITLE_FONT_SIZE)
                .into(),
            Align::LEFT,
            Some(app.theme.accent_color()),
            None,
        );

        // ---- pubkey ----
        // copy button
        {
            let pos = rect.right_top() + vec2(-TEXT_RIGHT - COPY_SYMBOL_SIZE.x, TEXT_TOP);
            let id = self.make_id("copy-pubkey");
            let response = ui.interact(
                egui::Rect::from_min_size(pos, COPY_SYMBOL_SIZE),
                id,
                egui::Sense::click(),
            );
            widgets::CopyButton::new().paint(ui, pos);
            if response
                .on_hover_text("Copy to clipboard")
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                ui.output_mut(|o| {
                    o.copied_text = self.pk.as_bech32_string();
                    GLOBALS
                        .status_queue
                        .write()
                        .write("copied to clipboard".to_owned());
                });
            }
        }

        // pubkey
        let pos = rect.right_top() + vec2(-TEXT_RIGHT - 20.0, TEXT_TOP);
        draw_text_at(
            ui,
            pos,
            gossip_lib::names::pubkey_short(self.pk).into(),
            Align::RIGHT,
            None,
            None,
        );

        // ---- connected relays ----
        let pos = rect.min + vec2(TEXT_LEFT, TEXT_TOP + (1.5 * TITLE_FONT_SIZE));
        let relays_string = self
            .relays
            .iter()
            .map(|rurl| rurl.as_str().to_owned())
            .collect::<Vec<String>>()
            .join(", ");
        draw_text_at(ui, pos, relays_string.into(), Align::LEFT, None, None);

        response
    }
}

fn find_relays_for_pubkey(pk: &PublicKey) -> Vec<RelayUrl> {
    GLOBALS
        .relay_picker
        .relay_assignments_iter()
        .filter(|f| f.pubkeys.contains(pk))
        .map(|f| f.relay_url.clone())
        .collect()
}

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    widgets::page_header(
        ui,
        format!(
            "Low Coverage Report (less than {} relays)",
            read_setting!(num_relays_per_person)
        ),
        |ui| {
            ui.spacing_mut().button_padding *= 2.0;
            if ui
                .button("Pick Relays Again")
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::RefreshScoresAndPickRelays);
            }
            ui.add_space(10.0);
            {
                widgets::set_important_button_visuals(ui, app);

                if ui
                    .button(Page::RelaysActivityMonitor.name())
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    app.set_page(ctx, Page::RelaysActivityMonitor);
                }
            }
        },
    );
    ui.horizontal_wrapped(|ui| {
        ui.label("You can change how many relays per person to query here:");
        if ui.link("Network Settings").clicked() {
            app.settings_tab = SettingsTab::Network;
            app.set_page(ctx, Page::Settings);
        }
    });
    if GLOBALS.relay_picker.pubkey_counts_iter().count() > 0 {
        ui.label(
            format!("The Relay-Picker has tried to connect to at least {} relays \
                for each person that you follow, however the pubkeys listed below are not fully covered. \
                You can manually ask the Relay-Picker to pick again, however most of the time it has already \
                tried its best.", read_setting!(num_relays_per_person)));

        ui.add_space(10.0);
        let id_source = ui.auto_id_with("relay-coverage-scroll");
        app.vert_scroll_area().id_source(id_source).show(ui, |ui| {
            for elem in GLOBALS.relay_picker.pubkey_counts_iter() {
                let pk = elem.key();
                let count = elem.value();
                let name = gossip_lib::names::best_name_from_pubkey_lookup(pk);
                let relays = find_relays_for_pubkey(pk);
                let hover_text = format!("Go to profile of {}", name);

                let entry = CoverageEntry::new(pk, name, count, relays);
                if entry
                    .show(ui, app)
                    .on_hover_text(hover_text)
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                {
                    app.set_page(ctx, Page::Person(*pk));
                }
            }

            // add one entry space at the bottom
            ui.allocate_exact_size(
                vec2(ui.available_size_before_wrap().x, COVERAGE_ENTRY_HEIGHT),
                egui::Sense::hover(),
            );
        });
    } else {
        ui.label("All followed people are fully covered.".to_owned());
    }
}
