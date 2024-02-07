use egui_winit::egui::{
    self, vec2, Color32, FontId, Rect, Rounding, Sense, Stroke, TextBuffer, Widget, WidgetText,
};

use crate::ui::Theme;

use super::NavItem;

pub struct TextEdit<'t> {
    theme: &'t Theme,
    text: &'t mut dyn TextBuffer,
    multiline: bool,
    desired_width: Option<f32>,
    hint_text: WidgetText,
    password: bool,
    bg_color: Option<Color32>,
    text_color: Option<Color32>,
    with_paste: bool,
    with_clear: bool,
    with_search: bool,
}

const MARGIN: egui::Margin = egui::Margin {
    left: 8.0,
    right: 8.0,
    top: 4.5,
    bottom: 4.5,
};

impl<'t> TextEdit<'t> {
    pub fn singleline(theme: &'t Theme, text: &'t mut dyn TextBuffer) -> Self {
        Self {
            theme,
            text,
            multiline: false,
            desired_width: None,
            hint_text: WidgetText::default(),
            password: false,
            bg_color: None,
            text_color: None,
            with_paste: false,
            with_clear: false,
            with_search: false,
        }
    }

    pub fn search(theme: &'t Theme, text: &'t mut dyn TextBuffer) -> Self {
        Self {
            theme,
            text,
            multiline: false,
            desired_width: None,
            hint_text: WidgetText::default(),
            password: false,
            bg_color: None,
            text_color: None,
            with_paste: false,
            with_clear: true,
            with_search: true,
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

    pub fn show(
        self,
        ui: &mut egui::Ui,
    ) -> (egui::text_edit::TextEditOutput, &'t mut dyn TextBuffer) {
        ui.scope(|ui| {
            self.set_visuals(ui);

            let pre_space = if self.with_search { 20.0 } else { 0.0 };
            let margin = egui::Margin {
                left: MARGIN.left + pre_space,
                right: MARGIN.right,
                top: MARGIN.top,
                bottom: MARGIN.bottom,
            };

            let where_to_put_background = ui.painter().add(egui::Shape::Noop);

            let mut inner = match self.multiline {
                false => egui::widgets::TextEdit::singleline(self.text),
                true => egui::widgets::TextEdit::multiline(self.text),
            }
            .frame(false)
            .password(self.password)
            .hint_text(self.hint_text.clone())
            .margin(margin); // set margin

            if let Some(width) = self.desired_width {
                inner = inner.desired_width(width);
            }

            if let Some(color) = self.text_color {
                inner = inner.text_color(color);
            }

            // ---- show inner ----
            let output = inner.show(ui);

            // ---- draw frame ----
            {
                let theme = self.theme;
                let response = &output.response;
                let frame_rect = response.rect;

                // this is how egui chooses the visual style:
                #[allow(clippy::if_same_then_else)]
                let (bg_color, frame_stroke) = if ui.visuals().dark_mode {
                    if !response.sense.interactive() {
                        (theme.neutral_800(), Stroke::new(1.0, theme.neutral_400()))
                    } else if response.is_pointer_button_down_on() || response.has_focus() {
                        (theme.neutral_800(), Stroke::new(1.0, theme.neutral_300()))
                    } else if response.hovered() || response.highlighted() {
                        (theme.neutral_800(), Stroke::new(1.0, theme.neutral_400()))
                    } else {
                        (theme.neutral_800(), Stroke::new(1.0, theme.neutral_400()))
                    }
                } else {
                    if !response.sense.interactive() {
                        (theme.neutral_50(), Stroke::new(1.0, theme.neutral_400()))
                    } else if response.is_pointer_button_down_on() || response.has_focus() {
                        (theme.neutral_50(), Stroke::new(1.0, theme.neutral_500()))
                    } else if response.hovered() || response.highlighted() {
                        (theme.neutral_50(), Stroke::new(1.0, theme.neutral_400()))
                    } else {
                        (theme.neutral_50(), Stroke::new(1.0, theme.neutral_400()))
                    }
                };

                let rounding = Rounding::same(6.0);

                let shape =
                    egui::epaint::RectShape::new(frame_rect, rounding, bg_color, frame_stroke);

                ui.painter().set(where_to_put_background, shape);
            }

            // ---- draw decorations ----
            if self.with_search {
                // search magnifying glass
                ui.painter().text(
                    output.response.rect.left_center() + vec2(MARGIN.left, 0.0),
                    egui::Align2::LEFT_CENTER,
                    "\u{1F50D}",
                    FontId::proportional(11.0),
                    ui.visuals().widgets.inactive.fg_stroke.color,
                );
            }

            if self.with_clear && !self.text.as_str().is_empty() {
                let rect = Rect::from_min_size(
                    output.response.rect.right_top() - vec2(output.response.rect.height(), 0.0),
                    vec2(output.response.rect.height(), output.response.rect.height()),
                );

                // clear button
                if ui
                    .put(
                        rect,
                        NavItem::new("\u{2715}", self.text.as_str().is_empty())
                            .color(ui.visuals().widgets.inactive.fg_stroke.color)
                            .active_color(ui.visuals().widgets.active.fg_stroke.color)
                            .hover_color(ui.visuals().hyperlink_color)
                            .sense(Sense::click()),
                    )
                    .clicked()
                {
                    self.text.clear();
                }
            }

            (output, self.text)
        })
        .inner
    }

    pub fn show_extended(
        self,
        ui: &mut egui::Ui,
        clipboard: &mut egui_winit::clipboard::Clipboard,
    ) -> egui::text_edit::TextEditOutput {
        ui.scope(|ui| {
            let with_paste = self.with_paste;
            let (output, text) = self.show(ui);

            // paste button
            if with_paste {
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
                    if let Some(paste) = clipboard.get() {
                        let index = if let Some(cursor) = output.cursor_range {
                            cursor.primary.ccursor.index
                        } else {
                            0
                        };
                        text.insert_text(paste.as_str(), index);
                    }
                }
            }

            output
        })
        .inner
    }
}

impl<'t> Widget for TextEdit<'t> {
    fn ui(self, ui: &mut egui_winit::egui::Ui) -> egui_winit::egui::Response {
        let (output, _) = self.show(ui);
        output.response
    }
}

impl TextEdit<'_> {
    fn set_visuals(&self, ui: &mut egui::Ui) {
        // this is how egui chooses the visual style:
        // if !response.sense.interactive() {
        //     &self.noninteractive
        // } else if response.is_pointer_button_down_on() || response.has_focus() {
        //     &self.active
        // } else if response.hovered() || response.highlighted() {
        //     &self.hovered
        // } else {
        //     &self.inactive
        // }
        let theme = self.theme;
        let visuals = ui.visuals_mut();

        // cursor (enabled)
        visuals.text_cursor = Stroke::new(3.0, theme.accent_color());

        if visuals.dark_mode {
            // text color (enabled)
            visuals.widgets.inactive.fg_stroke.color = theme.neutral_50();

            // text selection
            visuals.selection.bg_fill = theme.accent_color();
            visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
        } else {
            // text color (enabled)
            visuals.widgets.inactive.fg_stroke.color = theme.neutral_800();

            // text selection
            visuals.selection.bg_fill = theme.accent_color();
            visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
        }
    }
}
