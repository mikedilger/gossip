use egui_winit::egui::{self, vec2, Color32, Rect, Rounding, Sense, Stroke, TextBuffer, Widget, WidgetText};

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
}

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
            self.set_visuals(ui);

            let mut inner = match self.multiline {
                false => egui::widgets::TextEdit::singleline(self.text),
                true => egui::widgets::TextEdit::multiline(self.text),
            }
                .frame(true)
                .password(self.password)
                .hint_text(self.hint_text.clone());

            if let Some(width) = self.desired_width {
                inner = inner.desired_width(width);
            }

            if let Some(color) = self.text_color {
                inner = inner.text_color(color);
            }

            // show inner
            let response = inner.show(ui);

            if self.with_clear {
                let rect = Rect::from_min_size(
                    response.response.rect.right_top() - vec2(response.response.rect.height(), 0.0),
                    vec2(response.response.rect.height(), response.response.rect.height()),
                );

                // search clear button
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

            response
        })
        .inner
    }

    pub fn show_extended(
        self,
        ui: &mut egui::Ui,
        clipboard: &mut egui_winit::clipboard::Clipboard,
    ) -> egui::text_edit::TextEditOutput {
        ui.scope(|ui| {
            self.set_visuals(ui);

            let mut inner = match self.multiline {
                false => egui::widgets::TextEdit::singleline(self.text),
                true => egui::widgets::TextEdit::multiline(self.text),
            }
            .password(self.password)
            .hint_text(self.hint_text.clone())
            .margin(vec2(8.0, 4.5)); // set margin


            if let Some(width) = self.desired_width {
                inner = inner.desired_width(width);
            }

            if let Some(color) = self.text_color {
                inner = inner.text_color(color);
            }

            // show inner
            let output = inner.show(ui);

            if self.with_clear {
                let rect = Rect::from_min_size(
                    output.response.rect.right_top() - vec2(output.response.rect.height(), 0.0),
                    vec2(output.response.rect.height(), output.response.rect.height()),
                );

                // search clear button
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
                    if let Some(paste) = clipboard.get() {
                        let index = if let Some(cursor) = output.cursor_range {
                            cursor.primary.ccursor.index
                        } else {
                            0
                        };
                        self.text.insert_text(paste.as_str(), index);
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
        self.show(ui).response
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
        let rounding = Rounding::same(6.0);

        // rounding
        visuals.widgets.inactive.rounding = rounding;
        visuals.widgets.noninteractive.rounding = rounding;
        visuals.widgets.active.rounding = rounding;
        visuals.widgets.hovered.rounding = rounding;

        // expansion is equally applied to all frame sides
        // it affects only the drawing of the frame and not the
        // placement or spacing
        let expansion = 0.0;
        visuals.widgets.inactive.expansion = expansion;
        visuals.widgets.noninteractive.expansion = expansion;
        visuals.widgets.active.expansion = expansion;
        visuals.widgets.hovered.expansion = expansion;

        // cursor (enabled)
        visuals.text_cursor = Stroke::new(3.0, theme.accent_light());

        if visuals.dark_mode {
            // fill (enabled)
            visuals.extreme_bg_color =
                self.bg_color.unwrap_or(theme.neutral_800());

            // text color (enabled)
            visuals.widgets.inactive.fg_stroke.color = theme.neutral_50();

            // -- enabled, not hovered, not focused
            // border stroke
            visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, theme.neutral_400());

            // -- enabled, hovered, not focused
            // border stroke
            visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, theme.neutral_400());

            // -- enabled, focused
            // border stroke
            visuals.selection.stroke = Stroke::new(1.0, theme.neutral_300());

        } else {
            // fill (any state)
            visuals.extreme_bg_color =
                self.bg_color.unwrap_or(theme.neutral_50());

            // text color (enabled)
            visuals.widgets.inactive.fg_stroke.color = theme.neutral_800();

            // -- enabled, not hovered, not focused
            // border stroke
            visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, theme.neutral_400());

            // -- enabled, hovered, not focused
            // border stroke
            visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, theme.neutral_400());

            // -- enabled, focused
            // border stroke
            visuals.selection.stroke = Stroke::new(1.0, theme.neutral_500());
        }
    }
}
