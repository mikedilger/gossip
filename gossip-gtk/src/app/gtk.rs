use super::{App, Message, Page};
use gossip_lib::GLOBALS;
use libadwaita::prelude::*;
use relm4::component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender};
use relm4::loading_widgets::LoadingWidgets;
use relm4::gtk;
use relm4::{view, RelmWidgetExt};
use std::sync::atomic::Ordering;

#[relm4::component(async, pub)]
impl AsyncComponent for App {
    type Init = ();
    type Input = Message;
    type Output = ();
    type CommandOutput = ();

    view! {
        gtk::Window {
            set_title: Some("Gossip GTK"),
            set_default_width: 800,
            set_default_height: 600,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_spacing: 5,
                set_margin_all: 5,

                gtk::Button::with_label("Go To TBD") {
                    connect_clicked => Message::SetPage(Page::Tbd)
                },

                gtk::Button {
                    set_label: "Go To Login",
                    connect_clicked => Message::SetPage(Page::LoginPage)
                },

                match model.page {
                    Page::LoginPage => {
                        gtk::Label {
                            #[watch]
                            set_label: &format!("Page is {:?}", model.page),
                            set_margin_all: 5,
                        }
                    },
                    Page::WaitForMigration => {
                        gtk::Label {
                            #[watch]
                            set_label: &format!("Page is {:?}", model.page),
                            set_margin_all: 5,
                        }
                    },
                    Page::WaitForPruning(ref _s) => {
                        gtk::Label {
                            #[watch]
                            set_label: "Wait for pruning...",
                            set_margin_all: 5,
                        }
                    },
                    Page::Tbd => {
                        gtk::Label {
                            #[watch]
                            set_label: "TBD",
                            set_margin_all: 5,
                        }
                    }
                }
            }
        }
    }

    fn init_loading_widgets(root: Self::Root) -> Option<LoadingWidgets> {
        view! {
            #[local]
            root {
                set_title: Some("Gossip GTK"),
                set_default_size: (800, 600),

                #[name(spinner)]
                gtk::Spinner {
                    start: (),
                    set_halign: gtk::Align::Center,
                }
            }
        }
        Some(LoadingWidgets::new(root, spinner))
    }


    async fn init(
        _init: (),
        _root: Self::Root,
        _sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        // Simulate a delay
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        let model = App::new();
        let widgets = view_output!();
        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        message: Self::Input,
        _sender: AsyncComponentSender<Self>,
        _root: &Self::Root)
    {
        match message {
            Message::SetPage(page) => {
                /*
                let optstatus = GLOBALS.prune_status.read();
                if GLOBALS.wait_for_login.load(Ordering::Relaxed) {
                    self.page = Page::LoginPage;
                } else if GLOBALS.wait_for_data_migration.load(Ordering::Relaxed) {
                    self.page = Page::WaitForMigration;
                } else if let Some(status) = optstatus.as_ref() {
                    self.page = Page::WaitForPruning(status.clone());
            } else {
                */
                    self.page = page;
                //}
            }
        }
    }
}
