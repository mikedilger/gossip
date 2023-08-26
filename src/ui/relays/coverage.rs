use egui_winit::egui::{Context, Ui, self, vec2, Response};
use nostr_types::PublicKey;

use crate::{globals::GLOBALS, ui::{GossipUi, widgets, Page, SettingsTab}, comms::ToOverlordMessage};

struct CoverageEntry<'a> {
    pk: &'a PublicKey,
    count: &'a usize,
    name: String,
}

impl<'a> CoverageEntry<'a> {
    pub(super) fn new(pk: &'a PublicKey, count: &'a usize) -> Self {
        let name = GossipUi::display_name_from_pubkey_lookup(pk);
        Self {
            pk,
            count,
            name
        }
    }

    pub(super) fn show(&self, ui: &mut Ui) -> Response {
        let (rect, _) = widgets::list_entry::allocate_space(ui, 45.0);

        widgets::list_entry::paint_frame(ui, &rect);

        let id = ui.auto_id_with(self.pk.as_hex_string());
        let pos = rect.min + vec2(widgets::list_entry::TEXT_LEFT, widgets::list_entry::TEXT_TOP);
        let (galley, response) = widgets::list_entry::allocate_text_at(
            ui,
            pos,
            self.name.clone().into(),
            egui::Align::LEFT,
            id);

        widgets::list_entry::draw_text_galley_at(
            ui,
            pos,
            galley,
            None,
        None);

        widgets::list_entry::draw_text_at(
            ui,
            pos + vec2(response.rect.width(), 0.0),
            format!(": coverage short by {} relay(s)", self.count).into(),
            egui::Align::LEFT,
            None,
            None);

        let response = response
            .on_hover_text(format!("Go to profile of {}", self.name))
            .on_hover_cursor(egui::CursorIcon::PointingHand);

        response
    }
}

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.add_space(10.0);
    ui.horizontal_wrapped(|ui| {
        ui.heading("Coverage Report");
    });
    ui.add_space(10.0);
    ui.horizontal_wrapped(|ui| {
        ui.label("You can change how many relays per person to query here:");
        if ui.link("Network Settings").clicked() {
            app.settings_tab = SettingsTab::Network;
            app.set_page(Page::Settings);
        }
    });
    ui.add_space(10.0);

    if GLOBALS.relay_picker.pubkey_counts_iter().count() > 0 {
        ui.label(
            format!("The Relay-Picker has tried to connect to at least {} relays \
                for each person that you follow, however the pubkeys listed below are not fully covered. \
                You can manually ask the Relay-Picker to pick again, however most of the time it has already \
                tried its best.", app.settings.num_relays_per_person));

        ui.add_space(10.0);
        if ui.link("Pick Again").clicked() {
            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::PickRelays);
        }

        ui.add_space(10.0);
        let id_source = ui.auto_id_with("relay-coverage-scroll");
        egui::ScrollArea::vertical()
            .id_source(id_source)
            .show(ui, |ui| {
            for elem in GLOBALS.relay_picker.pubkey_counts_iter() {
                let pk = elem.key();
                let count = elem.value();

                let entry = CoverageEntry::new(pk, count);
                if entry.show(ui).clicked() {
                    app.set_page(Page::Person(*pk));
                }
            }
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
