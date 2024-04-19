use std::{cell::RefCell, rc::Rc};

use eframe::egui::{self, Align, Color32, Layout, RichText, Ui};
use egui_extras::{Size, StripBuilder};
use gossip_lib::{comms::ToOverlordMessage, PendingItem, PersonList, GLOBALS};

use crate::ui::{Page, Theme};

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

const TRUNC: f32 = 180.0;

impl<'a> Notification<'a> for Pending {
    fn timestamp(&self) -> u64 {
        self.timestamp
    }

    fn title(&self) -> RichText {
        RichText::new("Pending".to_uppercase()).color(Color32::from_rgb(0xFB, 0xBF, 0x24))
    }

    fn matches_filter(&self, filter: &NotificationFilter) -> bool {
        matches!(
            filter,
            NotificationFilter::All | NotificationFilter::PendingItem
        )
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
            PendingItem::RelayListNeverAdvertised => self.relay_list_never_advertised(theme, ui),
            PendingItem::RelayListChangedSinceAdvertised => {
                self.relay_list_changed_since_advertised(theme, ui)
            }
            PendingItem::RelayListNotAdvertisedRecently => {
                self.relay_list_not_advertized_recently(theme, ui)
            }
            PendingItem::PersonListNeverPublished(list) => {
                self.person_list_never_published(theme, ui, list)
            }
            PendingItem::PersonListOutOfSync(list) => self.person_list_out_of_sync(theme, ui, list),
            PendingItem::PersonListNotPublishedRecently(list) => {
                self.person_list_not_published_recently(theme, ui, list)
            }
        }
    }
}

impl Pending {
    fn layout(
        &mut self,
        theme: &Theme,
        ui: &mut Ui,
        description: impl FnOnce(&Theme, &mut Ui) -> Option<Page>,
        action: impl FnOnce(&Theme, &mut Ui) -> Option<Page>,
    ) -> Option<Page> {
        let mut new_page = None;

        StripBuilder::new(ui)
            .size(Size::remainder())
            .size(Size::initial(TRUNC))
            .cell_layout(Layout::left_to_right(Align::Center).with_main_wrap(true))
            .horizontal(|mut strip| {
                strip.cell(|ui| {
                    new_page = description(theme, ui);
                });

                strip.cell(|ui| {
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        new_page = action(theme, ui);
                    });
                });
            });

        new_page
    }

    fn person_list_never_published(
        &mut self,
        theme: &Theme,
        ui: &mut Ui,
        list: gossip_lib::PersonList1,
    ) -> Option<Page> {
        let metadata = GLOBALS
            .storage
            .get_person_list_metadata(list)
            .unwrap_or_default()
            .unwrap_or_default();

        let description = |_theme: &Theme, ui: &mut Ui| -> Option<Page> {
            ui.label(format!(
                "Your Person List '{}' has never been published.",
                metadata.title
            ));
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

        self.layout(theme, ui, description, action)
    }

    fn person_list_not_published_recently(
        &mut self,
        theme: &Theme,
        ui: &mut Ui,
        list: PersonList,
    ) -> Option<Page> {
        let metadata = GLOBALS
            .storage
            .get_person_list_metadata(list)
            .unwrap_or_default()
            .unwrap_or_default();

        let description = |_theme: &Theme, ui: &mut Ui| -> Option<Page> {
            ui.label(format!(
                "Your Person List '{}' has not been published since {}",
                metadata.title,
                super::unixtime_to_string(metadata.event_created_at.0)
            ));
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

        self.layout(theme, ui, description, action)
    }

    fn person_list_out_of_sync(
        &mut self,
        theme: &Theme,
        ui: &mut Ui,
        list: PersonList,
    ) -> Option<Page> {
        let metadata = GLOBALS
            .storage
            .get_person_list_metadata(list)
            .unwrap_or_default()
            .unwrap_or_default();

        let description = |_theme: &Theme, ui: &mut Ui| -> Option<Page> {
            ui.label(format!(
                "Your local Person List '{}' is out-of-sync with the one found on your relays",
                metadata.title
            ));
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

        self.layout(theme, ui, description, action)
    }

    fn relay_list_not_advertized_recently(&mut self, theme: &Theme, ui: &mut Ui) -> Option<Page> {
        let description = |_theme: &Theme, ui: &mut Ui| -> Option<Page> {
            ui.label("Your Relay List has not been advertised recently");
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

        self.layout(theme, ui, description, action)
    }

    fn relay_list_changed_since_advertised(&mut self, theme: &Theme, ui: &mut Ui) -> Option<Page> {
        let description = |_theme: &Theme, ui: &mut Ui| -> Option<Page> {
            ui.label("Your Relay List has changed locally");
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
        self.layout(theme, ui, description, action)
    }

    fn relay_list_never_advertised(&mut self, theme: &Theme, ui: &mut Ui) -> Option<Page> {
        let description = |_theme: &Theme, ui: &mut Ui| -> Option<Page> {
            ui.label("Your Relay List has never been advertized before");
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
        self.layout(theme, ui, description, action)
    }
}
