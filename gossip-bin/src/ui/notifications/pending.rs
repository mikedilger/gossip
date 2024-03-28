use std::{cell::RefCell, rc::Rc};

use eframe::egui::{Color32, Layout, RichText, Ui};
use gossip_lib::{comms::ToOverlordMessage, PendingItem, PersonList, GLOBALS};

use crate::ui::{Page, Theme};

use super::Notification;

pub struct Pending {
    inner: gossip_lib::PendingItem,
    timestamp: u64,
}

impl Pending {
    pub fn new(inner: gossip_lib::PendingItem, timestamp: u64) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self { inner, timestamp }))
    }
}

impl Notification for Pending {
    fn timestamp(&self) -> u64 {
        self.timestamp
    }

    fn title(&self) -> RichText {
        RichText::new("Pending".to_uppercase()).color(Color32::from_rgb(0xFB, 0xBF, 0x24))
    }

    fn summary(&self) -> String {
        todo!()
    }

    fn show(&mut self, theme: &Theme, ui: &mut Ui) -> Option<Page> {
        match self.inner {
            PendingItem::RelayAuthenticationRequest(_, _) => None,
            PendingItem::RelayConnectionRequest(_, _) => None,
            PendingItem::Nip46Request(_, _, _) => None,
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

fn person_list_never_published(
    theme: &Theme,
    ui: &mut Ui,
    list: gossip_lib::PersonList1,
) -> Option<Page> {
    let mut new_page = None;
    let metadata = GLOBALS
        .storage
        .get_person_list_metadata(list)
        .unwrap_or_default()
        .unwrap_or_default();

    ui.with_layout(Layout::left_to_right(super::ALIGN), |ui| {
        ui.set_height(super::HEIGHT);
        ui.label(format!(
            "Your Person List '{}' has never been published.",
            metadata.title
        ));
        ui.with_layout(Layout::right_to_left(super::ALIGN), |ui| {
            super::approve_style(theme, ui.style_mut());
            if ui.button("Publish Now").clicked() {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::PushPersonList(list));
            }
            ui.add_space(10.0);
            if ui.link("Manage List").clicked() {
                new_page = Some(crate::ui::Page::PeopleList(list));
            }
        });
    });

    new_page
}

fn person_list_not_published_recently(
    theme: &Theme,
    ui: &mut Ui,
    list: PersonList,
) -> Option<Page> {
    let mut new_page = None;
    let metadata = GLOBALS
        .storage
        .get_person_list_metadata(list)
        .unwrap_or_default()
        .unwrap_or_default();

    ui.with_layout(Layout::left_to_right(super::ALIGN), |ui| {
        ui.set_height(super::HEIGHT);
        ui.label(format!(
            "Your Person List '{}' has not been published since",
            metadata.title
        ));
        if let Ok(stamp) = time::OffsetDateTime::from_unix_timestamp(metadata.event_created_at.0) {
            if let Ok(formatted) = stamp.format(time::macros::format_description!(
                "[year]-[month repr:short]-[day] ([weekday repr:short]) [hour]:[minute]"
            )) {
                ui.label(formatted);
            }
        }
        ui.with_layout(Layout::right_to_left(super::ALIGN), |ui| {
            super::approve_style(theme, ui.style_mut());
            if ui.button("Publish Now").clicked() {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::PushPersonList(list));
            }
            ui.add_space(10.0);
            if ui.link("Manage List").clicked() {
                new_page = Some(crate::ui::Page::PeopleList(list));
            }
        });
    });

    new_page
}

fn person_list_out_of_sync(theme: &Theme, ui: &mut Ui, list: PersonList) -> Option<Page> {
    let mut new_page = None;
    let metadata = GLOBALS
        .storage
        .get_person_list_metadata(list)
        .unwrap_or_default()
        .unwrap_or_default();

    ui.with_layout(Layout::left_to_right(super::ALIGN), |ui| {
        ui.set_height(super::HEIGHT);
        ui.label(format!(
            "Your local Person List '{}' is out-of-sync with the one found on your relays",
            metadata.title
        ));
        ui.with_layout(Layout::right_to_left(super::ALIGN), |ui| {
            super::approve_style(theme, ui.style_mut());
            if ui.link("Manage List").clicked() {
                new_page = Some(crate::ui::Page::PeopleList(list));
            }
        });
    });

    new_page
}

fn relay_list_not_advertized_recently(theme: &Theme, ui: &mut Ui) -> Option<Page> {
    let mut new_page = None;

    ui.with_layout(Layout::left_to_right(super::ALIGN), |ui| {
        ui.set_height(super::HEIGHT);
        ui.label("Your Relay List has not been advertised recently");
        ui.with_layout(Layout::right_to_left(super::ALIGN), |ui| {
            super::approve_style(theme, ui.style_mut());
            if ui.button("Advertise Now").clicked() {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::AdvertiseRelayList);
            }
            ui.add_space(10.0);
            if ui.link("Manage Relays").clicked() {
                new_page = Some(crate::ui::Page::RelaysMine);
            }
        });
    });

    new_page
}

fn relay_list_changed_since_advertised(theme: &Theme, ui: &mut Ui) -> Option<Page> {
    let mut new_page = None;

    ui.with_layout(Layout::left_to_right(super::ALIGN), |ui| {
        ui.set_height(super::HEIGHT);
        ui.label("Your Relay List has changed localy");
        ui.with_layout(Layout::right_to_left(super::ALIGN), |ui| {
            super::approve_style(theme, ui.style_mut());
            if ui.button("Advertise Now").clicked() {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::AdvertiseRelayList);
            }
            ui.add_space(10.0);
            if ui.link("Manage Relays").clicked() {
                new_page = Some(crate::ui::Page::RelaysMine);
            }
        });
    });
    new_page
}

fn relay_list_never_advertised(theme: &Theme, ui: &mut Ui) -> Option<Page> {
    let mut new_page = None;
    ui.with_layout(Layout::left_to_right(super::ALIGN), |ui| {
        ui.set_height(super::HEIGHT);
        ui.label("Your Relay List has never been advertized before");
        ui.with_layout(Layout::right_to_left(super::ALIGN), |ui| {
            super::approve_style(theme, ui.style_mut());
            if ui.button("Advertise Now").clicked() {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::AdvertiseRelayList);
            }
            ui.add_space(10.0);
            if ui.link("Manage Relays").clicked() {
                new_page = Some(crate::ui::Page::RelaysMine);
            }
        });
    });
    new_page
}
