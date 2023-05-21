use std::cmp::Ordering;

use crate::db::DbRelay;

use super::{GossipUi, Page};
use eframe::egui;
use egui::{Context, Ui};
use egui_winit::egui::{Id, vec2, Vec2, Rect};
use nostr_types::RelayUrl;

mod active;
mod mine;
mod known;

pub(super) struct RelayUi {
    /// text of search field
    search: String,
    /// how to sort relay entries
    sort: RelaySorting,
    /// which relays to include in the list
    filter: RelayFilter,
    /// an optional relay url
    edit: Option<RelayUrl>,
}

impl RelayUi {
    pub(super) fn new() -> Self {
        Self {
            search: String::new(),
            sort: RelaySorting::default(),
            filter: RelayFilter::default(),
            edit: None,
        }
    }
}

#[derive(PartialEq)]
pub(super) enum RelaySorting {
    WriteRelaysFirst,
    AdvertiseRelaysFirst,
    HighestFollowingFirst,
    HighestSuccessRateFirst,
    LowestSuccessRateFirst,
}

impl Default for RelaySorting {
    fn default() -> Self {
        RelaySorting::WriteRelaysFirst
    }
}

impl RelaySorting {
    pub fn get_name(&self) -> &str {
        match self {
            RelaySorting::WriteRelaysFirst => "Write Relays",
            RelaySorting::AdvertiseRelaysFirst => "Advertise Relays",
            RelaySorting::HighestFollowingFirst => "Following",
            RelaySorting::HighestSuccessRateFirst => "Success Rate",
            RelaySorting::LowestSuccessRateFirst => "Failure Rate",
        }
    }
}

#[derive(PartialEq)]
pub(super) enum RelayFilter {
    All,
    Write,
    Read,
    Advertise,
    Private,
}

impl Default for RelayFilter {
    fn default() -> Self {
        RelayFilter::All
    }
}

impl RelayFilter {
    pub fn get_name(&self) -> &str {
        match self {
            RelayFilter::All => "All",
            RelayFilter::Write => "Write",
            RelayFilter::Read => "Read",
            RelayFilter::Advertise => "Advertise",
            RelayFilter::Private => "Private",
        }
    }
}

///
/// Show the Relays UI
///
pub(super) fn update(app: &mut GossipUi, ctx: &Context, frame: &mut eframe::Frame, ui: &mut Ui) {
    #[cfg(not(feature = "side-menu"))]
    {
        ui.horizontal(|ui| {
            if ui
                .add(egui::SelectableLabel::new(
                    app.page == Page::RelaysActivityMonitor,
                    "Live",
                ))
                .clicked()
            {
                app.set_page(Page::RelaysActivityMonitor);
            }
            ui.separator();
            if ui
                .add(egui::SelectableLabel::new(
                    app.page == Page::RelaysKnownNetwork,
                    "Configure",
                ))
                .clicked()
            {
                app.set_page(Page::RelaysKnownNetwork);
            }
            ui.separator();
        });
        ui.separator();
    }

    if app.page == Page::RelaysActivityMonitor {
        active::update(app, ctx, frame, ui);
    } else if app.page == Page::RelaysMine {
        mine::update(app, ctx, frame, ui);
    } else if app.page == Page::RelaysKnownNetwork {
        known::update(app, ctx, frame, ui);
    }
}

pub(super) fn entry_dialog(ctx: &Context, window_open: &mut bool) {
    const DLG_SIZE: Vec2 = vec2(500.0, 150.0);

    egui::Area::new("hide-background-area")
        .fixed_pos(ctx.screen_rect().left_top())
        .movable(false)
        .interactable(false)
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            ui.painter().rect_filled(
                ctx.screen_rect(),
                egui::Rounding::same(0.0),
                egui::Color32::from_rgba_unmultiplied(0x80,0x80,0x80,0x80));
        });

    let id: Id = "relays-add-dialog".into();
    let frame = egui::Frame::popup(&ctx.style());
    let area = egui::Area::new(id)
        .movable(false)
        .interactable(true)
        .order(egui::Order::Foreground)
        .fixed_pos(ctx.screen_rect().center() - DLG_SIZE/2.0);
    area.show_open_close_animation(ctx, &frame, *window_open);
    area.show(ctx, |ui| {
        frame.show(ui, |ui|{
            ui.set_min_size(DLG_SIZE);

            let pos = ui.min_rect().left_top();

            let close_rect = Rect::from_x_y_ranges(
                pos.x + 100.0 ..= pos.x + 150.0,
                pos.y + 110.0 ..= pos.y + 130.0
            );

            if ui.put(  close_rect, egui::Button::new("Close")).clicked() {
                *window_open = false;
            }
        });
    });
}

///
/// Draw relay sort comboBox
///
pub(super) fn relay_sort_combo(app: &mut GossipUi, ui: &mut Ui) {
    let sort_combo = egui::ComboBox::from_id_source(Id::from("RelaySortCombo"));
    sort_combo
        .width(130.0)
        .selected_text("Sort by ".to_string() + app.relays.sort.get_name())
        .show_ui(ui, |ui| {
            ui.selectable_value(
                &mut app.relays.sort,
                RelaySorting::HighestFollowingFirst,
                RelaySorting::HighestFollowingFirst.get_name(),
            );
            ui.selectable_value(
                &mut app.relays.sort,
                RelaySorting::HighestSuccessRateFirst,
                RelaySorting::HighestSuccessRateFirst.get_name(),
            );
            ui.selectable_value(
                &mut app.relays.sort,
                RelaySorting::LowestSuccessRateFirst,
                RelaySorting::LowestSuccessRateFirst.get_name(),
            );
            ui.selectable_value(
                &mut app.relays.sort,
                RelaySorting::WriteRelaysFirst,
                RelaySorting::WriteRelaysFirst.get_name(),
            );
            ui.selectable_value(
                &mut app.relays.sort,
                RelaySorting::AdvertiseRelaysFirst,
                RelaySorting::AdvertiseRelaysFirst.get_name(),
            );
        });
}

///
/// Draw relay filter comboBox
///
pub(super) fn relay_filter_combo(app: &mut GossipUi, ui: &mut Ui) {
    let filter_combo = egui::ComboBox::from_id_source(Id::from("RelayFilterCombo"));
    filter_combo
        .selected_text(app.relays.filter.get_name())
        .show_ui(ui, |ui| {
            ui.selectable_value(
                &mut app.relays.filter,
                RelayFilter::All,
                RelayFilter::All.get_name(),
            );
            ui.selectable_value(
                &mut app.relays.filter,
                RelayFilter::Write,
                RelayFilter::Write.get_name(),
            );
            ui.selectable_value(
                &mut app.relays.filter,
                RelayFilter::Read,
                RelayFilter::Read.get_name(),
            );
            ui.selectable_value(
                &mut app.relays.filter,
                RelayFilter::Advertise,
                RelayFilter::Advertise.get_name(),
            );
            ui.selectable_value(
                &mut app.relays.filter,
                RelayFilter::Private,
                RelayFilter::Private.get_name(),
            );
        });
}

///
/// Filter a relay entry
/// - return: true if selected
///
pub(super) fn sort_relay(rui: &RelayUi, a: &DbRelay, b: &DbRelay) -> Ordering {
    match rui.sort {
        RelaySorting::WriteRelaysFirst => b
            .has_usage_bits(DbRelay::WRITE)
            .cmp(&a.has_usage_bits(DbRelay::WRITE))
            .then(a.url.cmp(&b.url)),
        RelaySorting::AdvertiseRelaysFirst => b
            .has_usage_bits(DbRelay::ADVERTISE)
            .cmp(&a.has_usage_bits(DbRelay::ADVERTISE))
            .then(a.url.cmp(&b.url)),
        RelaySorting::HighestFollowingFirst => a.url.cmp(&b.url), // FIXME need following numbers here
        RelaySorting::HighestSuccessRateFirst => b
            .success_rate()
            .total_cmp(&a.success_rate())
            .then(a.url.cmp(&b.url)),
        RelaySorting::LowestSuccessRateFirst => a
            .success_rate()
            .total_cmp(&b.success_rate())
            .then(a.url.cmp(&b.url)),
    }
}

///
/// Filter a relay entry
/// - return: true if selected
///
pub(super) fn filter_relay(rui: &RelayUi, ri: &DbRelay) -> bool {
    let search = if rui.search.len() > 1 {
        ri.url
            .as_str()
            .to_lowercase()
            .contains(&rui.search.to_lowercase())
    } else {
        true
    };

    let filter = match rui.filter {
        RelayFilter::All => true,
        RelayFilter::Write => ri.has_usage_bits(DbRelay::WRITE),
        RelayFilter::Read => ri.has_usage_bits(DbRelay::READ),
        RelayFilter::Advertise => ri.has_usage_bits(DbRelay::ADVERTISE),
        RelayFilter::Private => !ri.has_usage_bits(DbRelay::INBOX | DbRelay::OUTBOX),
    };

    search && filter
}
