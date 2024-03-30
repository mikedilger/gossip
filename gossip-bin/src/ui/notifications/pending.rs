use std::{cell::RefCell, rc::Rc};

use eframe::egui::{self, Color32, Layout, RichText, Ui};
use gossip_lib::{comms::ToOverlordMessage, PendingItem, PersonList, GLOBALS};

use crate::ui::{widgets, Page, Theme};

use super::{Notification, NotificationFilter};

pub struct Pending {
    inner: gossip_lib::PendingItem,
    timestamp: u64,
}

impl Pending {
    pub fn new(inner: gossip_lib::PendingItem, timestamp: u64) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self { inner, timestamp }))
    }
}

const ALIGN: egui::Align = egui::Align::Center;
const HEIGHT: f32 = 23.0;
const TRUNC: f32 = 200.0;

impl<'a> Notification<'a> for Pending {
    fn timestamp(&self) -> u64 {
        self.timestamp
    }

    fn title(&self) -> RichText {
        RichText::new("Pending".to_uppercase()).color(Color32::from_rgb(0xFB, 0xBF, 0x24))
    }

    fn matches_filter(&self, filter: &NotificationFilter) -> bool {
        match filter {
            NotificationFilter::All => true,
            NotificationFilter::PendingItem => true,
            _ => false,
        }
    }

    fn item(&'a self) -> &'a PendingItem {
        &self.inner
    }

    fn get_remember(&self) -> bool {
        false
    }

    fn set_remember(&mut self, _value: bool) {
        // nothing
    }

    fn show(&mut self, theme: &Theme, ui: &mut Ui) -> Option<Page> {
        match self.inner {
            PendingItem::RelayAuthenticationRequest { .. } => None,
            PendingItem::RelayConnectionRequest { .. } => None,
            PendingItem::Nip46Request { .. } => None,
            PendingItem::RelayListNeverAdvertised => relay_list_never_advertised(theme, ui),
            PendingItem::RelayListChangedSinceAdvertised => {
                relay_list_changed_since_advertised(theme, ui)
            }
            PendingItem::RelayListNotAdvertisedRecently => {
                relay_list_not_advertized_recently(theme, ui)
            }
            PendingItem::PersonListNeverPublished(list) => {
                person_list_never_published(theme, ui, list)
            }
            PendingItem::PersonListOutOfSync(list) => person_list_out_of_sync(theme, ui, list),
            PendingItem::PersonListNotPublishedRecently(list) => {
                person_list_not_published_recently(theme, ui, list)
            }
        }
    }
}

fn layout(
    theme: &Theme,
    ui: &mut Ui,
    description: impl FnOnce(&Theme, &mut Ui, f32) -> Option<Page>,
    action: impl FnOnce(&Theme, &mut Ui) -> Option<Page>,
) -> Option<Page> {
    let mut new_page = None;
    // "responsive" layout
    let width = ui.available_width();
    if width > (TRUNC * 4.0) {
        ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_height(HEIGHT);
            new_page = description(theme, ui, width - TRUNC);
            ui.with_layout(Layout::right_to_left(ALIGN), |ui| {
                new_page = action(theme, ui);
            });
        });
    } else {
        ui.with_layout(Layout::top_down(egui::Align::LEFT), |ui| {
            ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
                ui.set_height(HEIGHT);
                new_page = description(theme, ui, width);
            });
            ui.with_layout(Layout::right_to_left(ALIGN), |ui| {
                ui.set_height(HEIGHT);
                new_page = action(theme, ui);
            });
        });
    };
    new_page
}

fn person_list_never_published(
    theme: &Theme,
    ui: &mut Ui,
    list: gossip_lib::PersonList1,
) -> Option<Page> {
    let metadata = GLOBALS
        .storage
        .get_person_list_metadata(list)
        .unwrap_or_default()
        .unwrap_or_default();

    let description = |_theme: &Theme, ui: &mut Ui, trunc_width: f32| -> Option<Page> {
        widgets::truncated_label(
            ui,
            format!(
                "Your Person List '{}' has never been published.",
                metadata.title
            ),
            trunc_width,
        );
        None
    };

    let action = |theme: &Theme, ui: &mut Ui| -> Option<Page> {
        let mut new_page = None;

        ui.scope(|ui| {
            super::manage_style(theme, ui.style_mut());
            if ui.button("Manage List").clicked() {
                new_page = Some(crate::ui::Page::PeopleList(list));
            }
        });
        ui.add_space(10.0);
        ui.scope(|ui| {
            super::approve_style(theme, ui.style_mut());
            if ui.button("Publish Now").clicked() {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::PushPersonList(list));
            }
        });
        new_page
    };

    layout(theme, ui, description, action)
}

fn person_list_not_published_recently(
    theme: &Theme,
    ui: &mut Ui,
    list: PersonList,
) -> Option<Page> {
    let metadata = GLOBALS
        .storage
        .get_person_list_metadata(list)
        .unwrap_or_default()
        .unwrap_or_default();

    let description = |_theme: &Theme, ui: &mut Ui, trunc_width: f32| -> Option<Page> {
        widgets::truncated_label(
            ui,
            format!(
                "Your Person List '{}' has not been published since {}",
                metadata.title,
                super::unixtime_to_string(metadata.event_created_at.0)
            ),
            trunc_width,
        );
        None
    };
    let action = |theme: &Theme, ui: &mut Ui| -> Option<Page> {
        let mut new_page = None;

        ui.scope(|ui| {
            super::manage_style(theme, ui.style_mut());
            if ui.button("Manage").clicked() {
                new_page = Some(crate::ui::Page::PeopleList(list));
            }
        });
        ui.add_space(10.0);
        ui.scope(|ui| {
            super::approve_style(theme, ui.style_mut());
            if ui.button("Publish Now").clicked() {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::PushPersonList(list));
            }
        });
        new_page
    };

    layout(theme, ui, description, action)
}

fn person_list_out_of_sync(theme: &Theme, ui: &mut Ui, list: PersonList) -> Option<Page> {
    let metadata = GLOBALS
        .storage
        .get_person_list_metadata(list)
        .unwrap_or_default()
        .unwrap_or_default();

    let description = |_theme: &Theme, ui: &mut Ui, trunc_width: f32| -> Option<Page> {
        widgets::truncated_label(
            ui,
            format!(
                "Your local Person List '{}' is out-of-sync with the one found on your relays",
                metadata.title
            ),
            trunc_width,
        );
        None
    };
    let action = |theme: &Theme, ui: &mut Ui| -> Option<Page> {
        let mut new_page = None;
        super::approve_style(theme, ui.style_mut());
        if ui.button("Manage").clicked() {
            new_page = Some(crate::ui::Page::PeopleList(list));
        }
        new_page
    };

    layout(theme, ui, description, action)
}

fn relay_list_not_advertized_recently(theme: &Theme, ui: &mut Ui) -> Option<Page> {
    let description = |_theme: &Theme, ui: &mut Ui, trunc_width: f32| -> Option<Page> {
        widgets::truncated_label(
            ui,
            "Your Relay List has not been advertised recently",
            trunc_width,
        );
        None
    };
    let action = |theme: &Theme, ui: &mut Ui| -> Option<Page> {
        let mut new_page = None;
        ui.scope(|ui| {
            super::manage_style(theme, ui.style_mut());
            if ui.button("Manage").clicked() {
                new_page = Some(crate::ui::Page::RelaysMine);
            }
        });
        ui.add_space(10.0);
        ui.scope(|ui| {
            super::approve_style(theme, ui.style_mut());
            if ui.button("Advertise Now").clicked() {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::AdvertiseRelayList);
            }
        });
        new_page
    };

    layout(theme, ui, description, action)
}

fn relay_list_changed_since_advertised(theme: &Theme, ui: &mut Ui) -> Option<Page> {
    let description = |_theme: &Theme, ui: &mut Ui, trunc_width: f32| -> Option<Page> {
        widgets::truncated_label(ui, "Your Relay List has changed localy", trunc_width);
        None
    };
    let action = |theme: &Theme, ui: &mut Ui| -> Option<Page> {
        let mut new_page = None;
        ui.scope(|ui| {
            super::manage_style(theme, ui.style_mut());
            if ui.button("Manage").clicked() {
                new_page = Some(crate::ui::Page::RelaysMine);
            }
        });
        ui.add_space(10.0);
        ui.scope(|ui| {
            super::approve_style(theme, ui.style_mut());
            if ui.button("Advertise Now").clicked() {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::AdvertiseRelayList);
            }
        });
        new_page
    };
    layout(theme, ui, description, action)
}

fn relay_list_never_advertised(theme: &Theme, ui: &mut Ui) -> Option<Page> {
    let description = |_theme: &Theme, ui: &mut Ui, trunc_width: f32| -> Option<Page> {
        widgets::truncated_label(
            ui,
            "Your Relay List has never been advertized before",
            trunc_width,
        );
        None
    };
    let action = |theme: &Theme, ui: &mut Ui| -> Option<Page> {
        let mut new_page = None;
        ui.scope(|ui| {
            super::manage_style(theme, ui.style_mut());
            if ui.button("Manage Relays").clicked() {
                new_page = Some(crate::ui::Page::RelaysMine);
            }
        });
        ui.add_space(10.0);
        ui.scope(|ui| {
            super::approve_style(theme, ui.style_mut());
            if ui.button("Advertise Now").clicked() {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::AdvertiseRelayList);
            }
        });
        new_page
    };
    layout(theme, ui, description, action)
}
