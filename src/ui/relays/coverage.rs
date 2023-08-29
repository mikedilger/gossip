use egui_winit::egui::{Context, Ui, self, vec2, Response, RichText, Align, Id};
use nostr_types::{PublicKey, RelayUrl};

use crate::{globals::GLOBALS, ui::{GossipUi, widgets::{self, list_entry::{TEXT_TOP, TEXT_LEFT, self, draw_text_at, TEXT_RIGHT, allocate_text_at, draw_text_galley_at}}, Page, SettingsTab}, comms::ToOverlordMessage};

struct CoverageEntry<'a> {
    pk: &'a PublicKey,
    _count: &'a usize,
    relays: Vec<RelayUrl>,
    name: String,
}

impl<'a> CoverageEntry<'a> {
    pub(super) fn new(pk: &'a PublicKey, name: String, _count: &'a usize, relays: Vec<RelayUrl>) -> Self {
        Self {
            pk,
            _count,
            relays,
            name
        }
    }

    fn make_id(&self, str: &str) -> Id {
        (self.pk.as_hex_string() + str).into()
    }

    pub(super) fn show(&self, ui: &mut Ui, app: &mut GossipUi) -> Response {
        let available_width = ui.available_size_before_wrap().x;
        let (rect, response) = ui.allocate_exact_size(vec2(available_width, 80.0), egui::Sense::click());

        let color = if response.hovered() {
            Some(ui.style().visuals.extreme_bg_color.linear_multiply(0.2))
        } else {
            None
        };

        widgets::list_entry::paint_frame(ui, &rect, color);

        // ---- title ----
        let pos = rect.min + vec2(TEXT_LEFT, TEXT_TOP);
        draw_text_at(
            ui,
            pos,
            RichText::new(self.name.clone()).size(list_entry::TITLE_FONT_SIZE).into(),
            Align::LEFT,
            Some(app.settings.theme.accent_color()),
            None);

        // ---- pubkey ----
        // copy button
        {
            let pos = rect.right_top() + vec2(-TEXT_RIGHT, TEXT_TOP);
            let text = RichText::new(crate::ui::widgets::COPY_SYMBOL);
            let id = self.make_id("copy-pubkey");
            let (galley, response) = allocate_text_at(ui, pos, text.into(), Align::RIGHT, id);
            if response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked() {
                ui.output_mut(|o| {
                    o.copied_text = self.pk.as_bech32_string();
                    GLOBALS
                        .status_queue
                        .write()
                        .write("copied to clipboard".to_owned());
                });
            }
            draw_text_galley_at(ui, pos, galley, None, None);
        }

        // pubkey
        let pos = rect.right_top() + vec2(-TEXT_RIGHT - 20.0, TEXT_TOP);
        draw_text_at(
            ui,
            pos,
            self.pk.as_bech32_string().into(),
            Align::RIGHT,
            None,
            None);

        // ---- connected relays ----
        let pos = rect.min + vec2(TEXT_LEFT, TEXT_TOP + 30.0);
        let relays_string = self.relays.iter().map(|f| f.to_string()).collect::<Vec<String>>().join(", ");
        draw_text_at(
            ui,
            pos,
            relays_string.into(),
            Align::LEFT,
            None,
            None);

        response
    }
}

fn find_relays_for_pubkey(pk: &PublicKey) -> Vec<RelayUrl> {
    GLOBALS.relay_picker.relay_assignments_iter()
        .filter(|f| f.pubkeys.contains(pk))
        .map(|f| f.relay_url.clone())
        .collect()
}

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.add_space(10.0);
    ui.horizontal_wrapped(|ui| {
        ui.heading(format!("Low Coverage Report (less than {} relays)",  app.settings.num_relays_per_person));
        ui.add_space(10.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            ui.add_space(20.0);
            ui.spacing_mut().button_padding *= 2.0;
            {
                let visuals = ui.visuals_mut();
                visuals.widgets.inactive.weak_bg_fill = app.settings.theme.accent_color();
                visuals.widgets.inactive.fg_stroke.width = 1.0;
                visuals.widgets.inactive.fg_stroke.color = app.settings.theme.get_style().visuals.extreme_bg_color;
                visuals.widgets.hovered.weak_bg_fill = app.settings.theme.navigation_text_color();
                visuals.widgets.hovered.fg_stroke.color = app.settings.theme.accent_color();
                visuals.widgets.inactive.fg_stroke.color = app.settings.theme.get_style().visuals.extreme_bg_color;
            }
            if ui.button("Pick Relays Again")
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked() {
                let _ = GLOBALS.to_overlord.send(ToOverlordMessage::PickRelays);
            }
        });
    });
    ui.add_space(10.0);
    ui.horizontal_wrapped(|ui| {
        ui.label("You can change how many relays per person to query here:");
        if ui.link("Network Settings").clicked() {
            app.settings_tab = SettingsTab::Network;
            app.set_page(Page::Settings);
        }
    });
    if GLOBALS.relay_picker.pubkey_counts_iter().count() > 0 {
        ui.label(
            format!("The Relay-Picker has tried to connect to at least {} relays \
                for each person that you follow, however the pubkeys listed below are not fully covered. \
                You can manually ask the Relay-Picker to pick again, however most of the time it has already \
                tried its best.", app.settings.num_relays_per_person));

        ui.add_space(10.0);
        let id_source = ui.auto_id_with("relay-coverage-scroll");
        egui::ScrollArea::vertical()
            .id_source(id_source)
            .show(ui, |ui| {
            for elem in GLOBALS.relay_picker.pubkey_counts_iter() {
                let pk = elem.key();
                let count = elem.value();
                let name = GossipUi::display_name_from_pubkey_lookup(pk);
                let relays = find_relays_for_pubkey(pk);
                let hover_text = format!("Go to profile of {}", name);

                let entry = CoverageEntry::new(pk, name, count, relays);
                if entry.show(ui, app)
                    .on_hover_text(hover_text)
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                 {
                    app.set_page(Page::Person(*pk));
                }
            }
            // uncomment below to mock with people entries for development
            // for pk in GLOBALS.people.get_followed_pubkeys() {
            //     let entry = CoverageEntry::new(&pk, &0);
            //     if entry.show(ui).clicked() {
            //         app.set_page(Page::Person(pk));
            //     }
            // }
        });
    } else {
        ui.label("All followed people are fully covered.".to_owned());
    }
}
