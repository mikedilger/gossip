use std::cmp::Ordering;

use crate::db::DbRelay;

use super::{GossipUi, Page};
use eframe::egui;
use egui::{Context, Ui};
use egui_winit::egui::{Id, Rect};
use nostr_types::RelayUrl;

mod activity;
mod known;

pub(super) struct RelayUi {
    search: String,
    sort: RelaySorting,
    filter: RelayFilter,
    edit: Option<RelayUrl>,
}

impl RelayUi {
    pub fn new() -> Self {
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
        activity::update(app, ctx, frame, ui);
    } else if app.page == Page::RelaysKnownNetwork {
        known::update(app, ctx, frame, ui);
    }
}

///
/// Draw relay sort comboBox
///
pub(super) fn relay_sort_combo(app: &mut GossipUi, ui: &mut Ui, id: Id) {
    let sort_combo = egui::ComboBox::from_id_source(id);
    sort_combo
        .width(130.0)
        .selected_text(app.relay_ui.sort.get_name())
        .show_ui(ui, |ui| {
            ui.selectable_value(
                &mut app.relay_ui.sort,
                RelaySorting::HighestFollowingFirst,
                RelaySorting::HighestFollowingFirst.get_name(),
            );
            ui.selectable_value(
                &mut app.relay_ui.sort,
                RelaySorting::HighestSuccessRateFirst,
                RelaySorting::HighestSuccessRateFirst.get_name(),
            );
            ui.selectable_value(
                &mut app.relay_ui.sort,
                RelaySorting::LowestSuccessRateFirst,
                RelaySorting::LowestSuccessRateFirst.get_name(),
            );
            ui.selectable_value(
                &mut app.relay_ui.sort,
                RelaySorting::WriteRelaysFirst,
                RelaySorting::WriteRelaysFirst.get_name(),
            );
            ui.selectable_value(
                &mut app.relay_ui.sort,
                RelaySorting::AdvertiseRelaysFirst,
                RelaySorting::AdvertiseRelaysFirst.get_name(),
            );
        });
}

///
/// Draw relay filter comboBox
///
pub(super) fn relay_filter_combo(app: &mut GossipUi, ui: &mut Ui, id: Id) {
    let filter_combo = egui::ComboBox::from_id_source(id);
    filter_combo
        .selected_text(app.relay_ui.filter.get_name())
        .show_ui(ui, |ui| {
            ui.selectable_value(
                &mut app.relay_ui.filter,
                RelayFilter::All,
                RelayFilter::All.get_name(),
            );
            ui.selectable_value(
                &mut app.relay_ui.filter,
                RelayFilter::Write,
                RelayFilter::Write.get_name(),
            );
            ui.selectable_value(
                &mut app.relay_ui.filter,
                RelayFilter::Read,
                RelayFilter::Read.get_name(),
            );
            ui.selectable_value(
                &mut app.relay_ui.filter,
                RelayFilter::Advertise,
                RelayFilter::Advertise.get_name(),
            );
            ui.selectable_value(
                &mut app.relay_ui.filter,
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
        RelaySorting::WriteRelaysFirst => b.has_usage_bits(DbRelay::WRITE)
                                            .cmp(&a.has_usage_bits(DbRelay::WRITE))
                                            .then(a.url.cmp(&b.url)),
        RelaySorting::AdvertiseRelaysFirst => b.has_usage_bits(DbRelay::ADVERTISE)
                                                .cmp(&a.has_usage_bits(DbRelay::ADVERTISE))
                                                .then(a.url.cmp(&b.url)),
        RelaySorting::HighestFollowingFirst => Ordering::Equal, // FIXME need following numbers here
        RelaySorting::HighestSuccessRateFirst => b.success_rate()
                                                .total_cmp(&a.success_rate())
                                                .then(a.url.cmp(&b.url)),
        RelaySorting::LowestSuccessRateFirst => a.success_rate()
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
