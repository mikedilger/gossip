use eframe::egui::style::TextCursorStyle;
use egui_winit::egui::{
    self, load::SizedTexture, vec2, Color32, Rect, Rounding, Sense, Stroke, TextBuffer,
    TextureHandle, Widget, WidgetText,
};

use crate::ui::{
    assets::{self, Assets},
    Theme,
};

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
    magnifyingglass_symbol: Option<TextureHandle>,
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
            magnifyingglass_symbol: None,
        }
    }

    pub fn search(theme: &'t Theme, assets: &Assets, text: &'t mut dyn TextBuffer) -> Self {
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
            magnifyingglass_symbol: Some(assets.magnifyingglass_symbol.clone()),
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
                if let Some(symbol) = self.magnifyingglass_symbol {
                    let rect = Rect::from_center_size(
                        output.response.rect.left_center()
                            + vec2((MARGIN.left + pre_space) / 2.0, 0.0),
                        symbol.size_vec2() / (assets::SVG_OVERSAMPLE + ui.ctx().zoom_factor()),
                    );
                    egui::Image::from_texture(SizedTexture::new(symbol.id(), symbol.size_vec2()))
                        .fit_to_exact_size(rect.size())
                        .tint(if self.theme.dark_mode {
                            self.theme.neutral_500()
                        } else {
                            self.theme.neutral_400()
                        })
                        .paint_at(ui, rect);
                }
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
        let output = self.show(ui);
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
        visuals.text_cursor = TextCursorStyle {
            stroke: Stroke::new(3.0, theme.accent_color()),
            ..Default::default()
        };

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
