use relm4::component::{AsyncComponentParts, AsyncComponentSender, SimpleAsyncComponent};
use relm4::gtk;

pub struct App;

impl App {
    fn new() -> App {
        App
    }
}

impl SimpleAsyncComponent for App {
    type Init = ();
    type Input = ();
    type Output = ();
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
        _data: (),
        _window: Self::Root,
        _sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let model = App::new();
        let widgets = ();
        AsyncComponentParts { model, widgets }
    }

    async fn update(&mut self, _message: Self::Input, _sender: AsyncComponentSender<Self>) {
    }

    fn update_view(&self, _widgets: &mut Self::Widgets, _sender: AsyncComponentSender<Self>) {
    }
}
