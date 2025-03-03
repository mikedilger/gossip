use super::{App, Message, Page};
use gossip_lib::GLOBALS;
use relm4::component::{AsyncComponentBuilder, AsyncComponentParts, AsyncComponentSender, AsyncComponent};
use relm4::loading_widgets::LoadingWidgets;
use relm4::{gtk, Sender};
use std::sync::atomic::Ordering;


impl AsyncComponent for App {
    type CommandOutput = ();
    type Input = Message;
    type Output = ();
    type Init = ();
    type Root = gtk::Window;
    type Widgets = ();

    fn init_root() -> Self::Root {
        gtk::Window::builder()
            .title("Gossip GTK")
            .default_width(800)
            .default_height(600)
            .build()
    }

    async fn init(
        _init: (),
        _root: Self::Root,
        _sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let app = App::new();
        let widgets = ();
        AsyncComponentParts { model: app, widgets }
    }

    fn builder() -> AsyncComponentBuilder<Self> {
        AsyncComponentBuilder::<Self>::default()
    }

    fn init_loading_widgets(_root: Self::Root) -> Option<LoadingWidgets> {
        None
    }

    async fn update(
        &mut self,
        message: Self::Input,
        _sender: AsyncComponentSender<Self>,
        _root: &Self::Root)
    {
        match message {
            Message::SetPage(page) => {
                let optstatus = GLOBALS.prune_status.read();
                if GLOBALS.wait_for_login.load(Ordering::Relaxed) {
                    self.page = Page::LoginPage;
                } else if GLOBALS.wait_for_data_migration.load(Ordering::Relaxed) {
                    self.page = Page::WaitForMigration;
                } else if let Some(status) = optstatus.as_ref() {
                    self.page = Page::WaitForPruning(status.clone());
                } else {
                    self.page = page;
                }
            }
        }
    }

    async fn update_cmd(
        &mut self,
        _message: Self::CommandOutput,
        _sender: AsyncComponentSender<Self>,
        _root: &Self::Root)
    {
    }

    async fn update_cmd_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::CommandOutput,
        sender: AsyncComponentSender<Self>,
        root: &Self::Root)
    {
        self.update_cmd(message, sender.clone(), root).await;
        self.update_view(widgets, sender);
    }

    fn update_view(
        &self,
        _widgets: &mut Self::Widgets,
        _sender: AsyncComponentSender<Self>
    ) {
    }

    async fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: AsyncComponentSender<Self>,
        root: &Self::Root
    ) {
        self.update(message, sender.clone(), root).await;
        self.update_view(widgets, sender);
    }

    fn shutdown(
        &mut self,
        _widgets: &mut Self::Widgets,
        _output: Sender<Self::Output>,
    ) {
    }

    fn id(&self) -> String {
        format!("{:p}", &self)
    }
}
