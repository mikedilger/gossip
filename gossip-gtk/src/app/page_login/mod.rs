use libadwaita::prelude::*;
use relm4::component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender};
use relm4::loading_widgets::LoadingWidgets;
use relm4::gtk;
use relm4::{view, RelmWidgetExt};



#[derive(Debug, Clone)]
pub struct PageLogin {
    pub password: String,
}

impl PageLogin {
    pub fn new() -> PageLogin {
        PageLogin {
            password: "".to_owned(),
        }
    }
}

#[relm4::component(async, pub)]
impl AsyncComponent for PageLogin {
    type Init = ();
    type Input = ();
    type Output = ();
    type CommandOutput = ();

    view! {
        #[root]
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_spacing: 5,
            set_margin_all: 5,

            gtk::Label {
                set_label: "Login Page",
                set_margin_all: 5,
            }
        }
    }

    fn init_loading_widgets(root: Self::Root) -> Option<LoadingWidgets> {
        view! {
            #[name(spinner)]
            gtk::Spinner {
                start: (),
                set_halign: gtk::Align::Center,
            }
        }
        Some(LoadingWidgets::new(root, spinner))
    }

    async fn init(
        _init: (),
        _root: Self::Root,
        _sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let model = PageLogin::new();
        let widgets = view_output!();
        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        _message: Self::Input,
        _sender: AsyncComponentSender<Self>,
        _root: &Self::Root)
    {
    }
}
