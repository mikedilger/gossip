use egui_winit::egui::{self, vec2, Color32, Rect, TextBuffer, Widget, WidgetText};

pub struct TextEdit<'t> {
    text: &'t mut dyn TextBuffer,
    multiline: bool,
    desired_width: Option<f32>,
    hint_text: WidgetText,
    password: bool,
    bg_color: Option<Color32>,
    text_color: Option<Color32>,
    with_paste: bool,
    with_clear: bool,
}

impl<'t> TextEdit<'t> {
    pub fn singleline(text: &'t mut dyn TextBuffer) -> Self {
        Self {
            text,
            multiline: false,
            desired_width: None,
            hint_text: WidgetText::default(),
            password: false,
            bg_color: None,
            text_color: None,
            with_paste: false,
            with_clear: false,
        }
    }

    // pub fn mutliline(text: &'t mut dyn TextBuffer)-> Self {
    //     Self {
    //         text,
    //         multiline: true,
    //         desired_width: None,
    //         hint_text: WidgetText::default(),
    //         password: false,
    //         text_color: None,
    //         with_paste: false,
    //         with_clear: false,
    //     }
    // }

    // ---- builders ----
    #[allow(unused)]
    pub fn desired_width(mut self, desired_width: f32) -> Self {
        self.desired_width = Some(desired_width);
        self
    }

    #[allow(unused)]
    pub fn hint_text(mut self, hint_text: impl Into<WidgetText>) -> Self {
        self.hint_text = hint_text.into();
        self
    }

    #[allow(unused)]
    pub fn password(mut self, password: bool) -> Self {
        self.password = password;
        self
    }

    #[allow(unused)]
    pub fn bg_color(mut self, bg_color: egui::Color32) -> Self {
        self.bg_color = Some(bg_color);
        self
    }

    #[allow(unused)]
    pub fn text_color(mut self, text_color: egui::Color32) -> Self {
        self.text_color = Some(text_color);
        self
    }

    #[allow(unused)]
    pub fn with_paste(mut self) -> Self {
        self.with_paste = true;
        self
    }

    #[allow(unused)]
    pub fn with_clear(mut self) -> Self {
        self.with_clear = true;
        self
    }

    pub fn show(self, ui: &mut egui::Ui) -> egui::text_edit::TextEditOutput {
        ui.scope(|ui| {
            if ui.visuals().dark_mode {
                ui.visuals_mut().extreme_bg_color =
                    self.bg_color.unwrap_or(egui::Color32::from_gray(0x47));
            } else {
                ui.visuals_mut().extreme_bg_color = self.bg_color.unwrap_or(Color32::WHITE);
            }

            let mut inner = match self.multiline {
                false => egui::widgets::TextEdit::singleline(self.text),
                true => egui::widgets::TextEdit::multiline(self.text),
            }
            .password(self.password)
            .hint_text(self.hint_text.clone());

            if let Some(width) = self.desired_width {
                inner = inner.desired_width(width);
            }

            if let Some(color) = self.text_color {
                inner = inner.text_color(color);
            }

            // show inner
            let output = inner.show(ui);

            // paste button
            if self.with_paste {
                let action_size = vec2(45.0, output.response.rect.height());
                let rect = Rect::from_min_size(
                    output.response.rect.right_top() - vec2(action_size.x, 0.0),
                    action_size,
                );

                if ui
                    .put(
                        rect,
                        super::NavItem::new("Paste", true)
                            .color(ui.visuals().widgets.active.fg_stroke.color)
                            .active_color(ui.visuals().widgets.active.fg_stroke.color)
                            .hover_color(ui.visuals().widgets.hovered.fg_stroke.color)
                            .sense(egui::Sense::click()),
                    )
                    .clicked()
                {
                    output.response.request_focus();
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                }
            }

            output
        })
        .inner
    }
}

impl<'t> Widget for TextEdit<'t> {
    fn ui(self, ui: &mut egui_winit::egui::Ui) -> egui_winit::egui::Response {
        self.show(ui).response
    }
}
