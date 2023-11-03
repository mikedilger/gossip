use egui_winit::egui::{self, Id, InnerResponse, Rect, Response, RichText, TextureHandle, Ui};
use gossip_lib::Person;
pub trait InformationPopup {
    fn id(&self) -> Id;
    fn interact_rect(&self) -> Rect;
    fn set_last_seen(&mut self, time: f64);
    fn get_until(&self) -> Option<f64>;

    fn tag(&self) -> &Option<String>;

    fn show(
        &self,
        ui: &mut Ui,
        actions: Box<dyn FnOnce(&mut Ui) -> Response>,
    ) -> InnerResponse<Response>;
}

pub struct ProfilePopup {
    id: Id,
    tag: Option<String>,
    interact_rect: Rect,
    show_until: Option<f64>,
    show_duration: Option<f64>,

    avatar: TextureHandle,
    person: Person,
}

impl ProfilePopup {
    /// Creates a new [`ProfilePopup`].
    pub fn new(id: Id, interact_rect: Rect, avatar: TextureHandle, person: Person) -> Self {
        Self {
            id,
            tag: None,
            interact_rect,
            show_until: None,
            show_duration: None,
            avatar,
            person,
        }
    }

    pub fn show_duration(mut self, time: f64) -> Self {
        self.show_duration = Some(time);
        self
    }

    pub fn tag(mut self, tag: String) -> Self {
        self.tag = Some(tag);
        self
    }
}

impl InformationPopup for ProfilePopup {
    fn id(&self) -> Id {
        self.id
    }

    fn interact_rect(&self) -> egui_winit::egui::Rect {
        self.interact_rect
    }

    fn set_last_seen(&mut self, time: f64) {
        if let Some(duration) = self.show_duration {
            self.show_until = Some(time + duration);
        } else {
            self.show_until = None;
        }
    }

    fn get_until(&self) -> Option<f64> {
        self.show_until
    }

    fn tag(&self) -> &Option<String> {
        &self.tag
    }

    fn show(
        &self,
        ui: &mut Ui,
        actions: Box<dyn FnOnce(&mut Ui) -> Response>,
    ) -> InnerResponse<Response> {
        let frame = prepare_mini_person(ui);
        let area = egui::Area::new(self.id)
            .fixed_pos(self.interact_rect.left_bottom())
            .movable(false)
            .constrain(true)
            .interactable(true)
            .order(egui::Order::Foreground);

        area.show(ui.ctx(), |ui| {
            show_mini_person(frame, ui, &self.avatar, &self.person, actions)
        })
    }
}

fn prepare_mini_person(ui: &mut Ui) -> egui::Frame {
    egui::Frame::popup(ui.style())
        .rounding(egui::Rounding::ZERO)
        .inner_margin(egui::Margin::symmetric(10.0, 5.0))
}

fn show_mini_person(
    frame: egui::Frame,
    ui: &mut Ui,
    avatar: &TextureHandle,
    person: &Person,
    actions: Box<dyn FnOnce(&mut Ui) -> Response>,
) -> Response {
    frame
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                super::paint_avatar(ui, person, avatar, super::AvatarSize::Mini);
                ui.vertical(|ui| {
                    super::truncated_label(
                        ui,
                        RichText::new(person.best_name()).small(),
                        super::TAGG_WIDTH - 33.0,
                    );

                    let mut nip05 = RichText::new(person.nip05().unwrap_or_default())
                        .weak()
                        .small();
                    if !person.nip05_valid {
                        nip05 = nip05.strikethrough()
                    }
                    super::truncated_label(ui, nip05, super::TAGG_WIDTH - 33.0);
                });
            });
            actions(ui)
        })
        .inner
}
