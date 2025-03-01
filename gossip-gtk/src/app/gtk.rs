use super::App;
use relm4::component::{AsyncComponentBuilder, AsyncComponentParts, AsyncComponentSender, AsyncComponent};
use relm4::loading_widgets::LoadingWidgets;
use relm4::{gtk, Sender};


impl AsyncComponent for App {
    type CommandOutput = ();
    type Input = ();
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
        let mut app = App::new();
        app.maybe_force_page();
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
        _message: Self::Input,
        _sender: AsyncComponentSender<Self>,
        _root: &Self::Root)
    {
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
