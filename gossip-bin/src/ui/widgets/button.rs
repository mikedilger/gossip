use egui_winit::egui::{
    self, vec2, widget_text::WidgetTextGalley, NumExt, Rect, Response, Rounding, Sense, Stroke,
    TextStyle, Ui, Vec2, Widget, WidgetInfo, WidgetText, WidgetType,
};

use super::{super::Theme, WidgetState};

#[derive(Clone, Copy)]
enum ButtonType {
    Primary,
    Secondary,
    Bordered,
}

enum ButtonVariant {
    Normal,
    Small,
    Wide,
}

/// Clickable button with text
#[must_use = "You should put this widget in an ui with `ui.add(widget);`"]
pub struct Button<'a> {
    button_type: ButtonType,
    variant: ButtonVariant,
    selected: bool,
    theme: &'a Theme,
    text: Option<WidgetText>,
}

impl<'a> Button<'a> {
    pub fn primary(theme: &'a Theme, text: impl Into<WidgetText>) -> Self {
        Self {
            button_type: ButtonType::Primary,
            variant: ButtonVariant::Normal,
            selected: false,
            theme,
            text: Some(text.into()),
        }
    }

    pub fn secondary(theme: &'a Theme, text: impl Into<WidgetText>) -> Self {
        Self {
            button_type: ButtonType::Secondary,
            variant: ButtonVariant::Normal,
            selected: false,
            theme,
            text: Some(text.into()),
        }
    }

    pub fn bordered(theme: &'a Theme, text: impl Into<WidgetText>) -> Self {
        Self {
            button_type: ButtonType::Bordered,
            variant: ButtonVariant::Normal,
            selected: false,
            theme,
            text: Some(text.into()),
        }
    }

    /// Make this a small button, suitable for embedding into text.
    pub fn small(mut self, small: bool) -> Self {
        if small {
            self.variant = ButtonVariant::Small;
        }
        self
    }

    /// Make this a wide button.
    pub fn wide(mut self, wide: bool) -> Self {
        if wide {
            self.variant = ButtonVariant::Wide;
        }
        self
    }

    /// If `true`, mark this button as "selected".
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn draw_default(self, ui: &mut Ui) -> Response {
        let (text, desired_size, padding) = Self::layout(ui, self.text, self.variant);
        let (rect, response) = Self::allocate(ui, &text, desired_size);
        Self::draw(
            ui,
            text,
            rect,
            WidgetState::Default,
            self.button_type,
            padding,
            self.theme,
        );
        response
    }

    pub fn draw_hovered(self, ui: &mut Ui) -> Response {
        let (text, desired_size, padding) = Self::layout(ui, self.text, self.variant);
        let (rect, response) = Self::allocate(ui, &text, desired_size);
        Self::draw(
            ui,
            text,
            rect,
            WidgetState::Hovered,
            self.button_type,
            padding,
            self.theme,
        );
        response
    }

    pub fn draw_active(self, ui: &mut Ui) -> Response {
        let (text, desired_size, padding) = Self::layout(ui, self.text, self.variant);
        let (rect, response) = Self::allocate(ui, &text, desired_size);
        Self::draw(
            ui,
            text,
            rect,
            WidgetState::Active,
            self.button_type,
            padding,
            self.theme,
        );
        response
    }

    pub fn draw_disabled(self, ui: &mut Ui) -> Response {
        let (text, desired_size, padding) = Self::layout(ui, self.text, self.variant);
        let (rect, response) = Self::allocate(ui, &text, desired_size);
        Self::draw(
            ui,
            text,
            rect,
            WidgetState::Disabled,
            self.button_type,
            padding,
            self.theme,
        );
        response
    }

    pub fn draw_focused(self, ui: &mut Ui) -> Response {
        let (text, desired_size, padding) = Self::layout(ui, self.text, self.variant);
        let (rect, response) = Self::allocate(ui, &text, desired_size);
        Self::draw(
            ui,
            text,
            rect,
            WidgetState::Focused,
            self.button_type,
            padding,
            self.theme,
        );
        response
    }
}

impl Widget for Button<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let (text, desired_size, padding) = Self::layout(ui, self.text, self.variant);
        let (rect, response) = Self::allocate(ui, &text, desired_size);
        let state = if response.is_pointer_button_down_on() || self.selected {
            WidgetState::Active
        } else if response.has_focus() {
            WidgetState::Focused
        } else if response.hovered() || response.highlighted() {
            WidgetState::Hovered
        } else if !ui.is_enabled() {
            WidgetState::Disabled
        } else {
            WidgetState::Default
        };
        Self::draw(ui, text, rect, state, self.button_type, padding, self.theme);
        response
    }
}

impl Button<'_> {
    fn layout(
        ui: &mut Ui,
        text: Option<WidgetText>,
        variant: ButtonVariant,
    ) -> (Option<WidgetTextGalley>, Vec2, Vec2) {
        let frame = ui.visuals().button_frame;

        let mut button_padding = if frame {
            Vec2::new(14.0, 4.0)
        } else {
            Vec2::ZERO
        };

        match variant {
            ButtonVariant::Normal => {}
            ButtonVariant::Small => {
                button_padding.y = 0.0;
            }
            ButtonVariant::Wide => {
                button_padding.x *= 3.0;
            }
        }

        let wrap = None;
        let text_wrap_width = ui.available_width() - 2.0 * button_padding.x;

        let text = text.map(|text| text.into_galley(ui, wrap, text_wrap_width, TextStyle::Button));

        let mut desired_size = Vec2::ZERO;
        if let Some(text) = &text {
            desired_size.x += text.size().x;
            desired_size.y = desired_size.y.max(text.size().y);
        }
        desired_size += 2.0 * button_padding;
        match variant {
            ButtonVariant::Wide | ButtonVariant::Normal => {
                desired_size.y = desired_size.y.at_least(ui.spacing().interact_size.y);
            }
            ButtonVariant::Small => {}
        }
        (text, desired_size, button_padding)
    }

    fn allocate(
        ui: &mut Ui,
        text: &Option<WidgetTextGalley>,
        desired_size: Vec2,
    ) -> (Rect, Response) {
        let (rect, response) = ui.allocate_at_least(desired_size, Sense::click());
        response.widget_info(|| {
            if let Some(text) = text {
                WidgetInfo::labeled(WidgetType::Button, text.text())
            } else {
                WidgetInfo::new(WidgetType::Button)
            }
        });

        if let Some(cursor) = ui.visuals().interact_cursor {
            if response.hovered {
                ui.ctx().set_cursor_icon(cursor);
            }
        }

        (rect, response)
    }

    fn draw(
        ui: &mut Ui,
        text: Option<WidgetTextGalley>,
        rect: Rect,
        state: WidgetState,
        button_type: ButtonType,
        button_padding: Vec2,
        theme: &Theme,
    ) {
        if ui.is_rect_visible(rect) {
            let no_stroke = Stroke::NONE;
            let neutral_300_stroke = Stroke::new(1.0, theme.neutral_300());
            let neutral_400_stroke = Stroke::new(1.0, theme.neutral_400());
            let neutral_500_stroke = Stroke::new(1.0, theme.neutral_500());
            let neutral_600_stroke = Stroke::new(1.0, theme.neutral_600());
            let (frame_fill, frame_stroke, text_color, under_stroke) = if ui.visuals().dark_mode {
                match state {
                    WidgetState::Default => match button_type {
                        ButtonType::Primary => (
                            theme.accent_dark(),
                            no_stroke,
                            theme.neutral_50(),
                            no_stroke,
                        ),
                        ButtonType::Secondary => (
                            theme.neutral_200(),
                            no_stroke,
                            theme.neutral_700(),
                            no_stroke,
                        ),
                        ButtonType::Bordered => (
                            theme.neutral_950(),
                            neutral_400_stroke,
                            theme.neutral_300(),
                            no_stroke,
                        ),
                    },
                    WidgetState::Hovered => match button_type {
                        ButtonType::Primary => (
                            theme.accent_dark_b20(),
                            no_stroke,
                            theme.neutral_50(),
                            no_stroke,
                        ),
                        ButtonType::Secondary => (
                            theme.neutral_50(),
                            no_stroke,
                            theme.accent_dark(),
                            no_stroke,
                        ),
                        ButtonType::Bordered => (
                            theme.neutral_950(),
                            neutral_300_stroke,
                            theme.neutral_200(),
                            no_stroke,
                        ),
                    },
                    WidgetState::Active => match button_type {
                        ButtonType::Primary => (
                            theme.accent_dark(),
                            no_stroke,
                            theme.neutral_50(),
                            no_stroke,
                        ),
                        ButtonType::Secondary => (
                            theme.neutral_200(),
                            no_stroke,
                            theme.neutral_700(),
                            no_stroke,
                        ),
                        ButtonType::Bordered => (
                            theme.neutral_950(),
                            neutral_400_stroke,
                            theme.neutral_300(),
                            no_stroke,
                        ),
                    },
                    WidgetState::Disabled => (
                        theme.neutral_700(),
                        no_stroke,
                        theme.neutral_500(),
                        no_stroke,
                    ),
                    WidgetState::Focused => match button_type {
                        ButtonType::Primary => (
                            theme.accent_dark_b20(),
                            no_stroke,
                            theme.neutral_50(),
                            neutral_300_stroke,
                        ),
                        ButtonType::Secondary => (
                            theme.neutral_50(),
                            no_stroke,
                            theme.accent_dark(),
                            neutral_400_stroke,
                        ),
                        ButtonType::Bordered => (
                            theme.neutral_950(),
                            neutral_300_stroke,
                            theme.neutral_200(),
                            neutral_500_stroke,
                        ),
                    },
                }
            } else {
                match state {
                    WidgetState::Default => match button_type {
                        ButtonType::Primary => (
                            theme.accent_light(),
                            no_stroke,
                            theme.neutral_50(),
                            no_stroke,
                        ),
                        ButtonType::Secondary => (
                            theme.neutral_700(),
                            no_stroke,
                            theme.neutral_100(),
                            no_stroke,
                        ),
                        ButtonType::Bordered => (
                            theme.neutral_100(),
                            neutral_500_stroke,
                            theme.neutral_800(),
                            no_stroke,
                        ),
                    },
                    WidgetState::Hovered => match button_type {
                        ButtonType::Primary => (
                            theme.accent_light_b20(),
                            no_stroke,
                            theme.neutral_50(),
                            no_stroke,
                        ),
                        ButtonType::Secondary => (
                            theme.neutral_900(),
                            no_stroke,
                            theme.neutral_100(),
                            no_stroke,
                        ),
                        ButtonType::Bordered => (
                            theme.neutral_50(),
                            neutral_600_stroke,
                            theme.neutral_800(),
                            no_stroke,
                        ),
                    },
                    WidgetState::Active => match button_type {
                        ButtonType::Primary => (
                            theme.accent_light(),
                            no_stroke,
                            theme.neutral_50(),
                            no_stroke,
                        ),
                        ButtonType::Secondary => (
                            theme.neutral_700(),
                            no_stroke,
                            theme.neutral_100(),
                            no_stroke,
                        ),
                        ButtonType::Bordered => (
                            theme.neutral_100(),
                            neutral_600_stroke,
                            theme.accent_light(),
                            no_stroke,
                        ),
                    },
                    WidgetState::Disabled => (
                        theme.neutral_300(),
                        no_stroke,
                        theme.neutral_400(),
                        no_stroke,
                    ),
                    WidgetState::Focused => match button_type {
                        ButtonType::Primary => (
                            theme.accent_light_b20(),
                            no_stroke,
                            theme.neutral_50(),
                            neutral_300_stroke,
                        ),
                        ButtonType::Secondary => (
                            theme.neutral_900(),
                            no_stroke,
                            theme.neutral_100(),
                            neutral_400_stroke,
                        ),
                        ButtonType::Bordered => (
                            theme.neutral_50(),
                            neutral_600_stroke,
                            theme.neutral_800(),
                            neutral_400_stroke,
                        ),
                    },
                }
            };

            let shrink = Vec2::splat(frame_stroke.width / 2.0);
            ui.painter().rect(
                rect.shrink2(shrink),
                Rounding::same(4.0),
                frame_fill,
                frame_stroke,
            );

            if let Some(text) = text {
                let text_pos = {
                    // Make sure button text is centered if within a centered layout
                    ui.layout()
                        .align_size_within_rect(text.size(), rect.shrink2(button_padding))
                        .min
                };
                let painter = ui.painter();
                let galley = text.galley;
                if text.galley_has_color {
                    painter.galley(text_pos, galley.clone());
                } else {
                    painter.galley_with_color(text_pos, galley.clone(), text_color);
                }
                let text_rect = Rect::from_min_size(text_pos, galley.rect.size());
                let shapes = egui::Shape::dashed_line(
                    &[
                        text_rect.left_bottom() + vec2(0.0, 0.0),
                        text_rect.right_bottom() + vec2(0.0, 0.0),
                    ],
                    under_stroke,
                    3.0,
                    3.0,
                );
                painter.add(shapes);
            }
        }
    }
}
