use super::{GossipUi, Page};
use eframe::egui;
use egui::{Context, Ui};

mod activity;
mod known;

pub(super) struct RelayUi {
    search: String,
}

impl RelayUi {
    pub fn new() -> Self {
        Self {
            search: String::new(),
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
