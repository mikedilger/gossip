use std::{ops::Sub, sync::Arc};

use egui_winit::egui::{
    self, vec2, Color32, Galley, Id, Rect, Response, Stroke, TextStyle, Ui, Vec2, Widget,
    WidgetText,
};

use crate::ui::Theme;

use super::WidgetState;

pub struct Switch<'a> {
    value: &'a mut bool,
    label: Option<WidgetText>,
    label_color: Option<Color32>,
    size: Vec2,
    padding: Vec2,
    theme: &'a Theme,
}

impl Switch<'_> {
    pub const fn small_size() -> Vec2 {
        vec2(29.0, 16.0)
    }

    pub const fn large_size() -> Vec2 {
        vec2(40.0, 22.0)
    }
}

impl<'a> Switch<'a> {
    /// Create a small switch, similar to normal line height
    pub fn small(theme: &'a Theme, value: &'a mut bool) -> Self {
        Self {
            value,
            label: None,
            label_color: None,
            size: Switch::small_size(),
            padding: Vec2::ZERO,
            theme,
        }
    }

    /// Create a large switch
    pub fn large(theme: &'a Theme, value: &'a mut bool) -> Self {
        Self {
            value,
            label: None,
            label_color: None,
            size: Switch::large_size(),
            padding: Vec2::ZERO,
            theme,
        }
    }

    /// Add a label that will be displayed to the right of the switch
    pub fn with_label(mut self, text: impl Into<WidgetText>) -> Self {
        self.label = Some(text.into());
        self
    }

    pub fn with_label_color(mut self, color: Option<Color32>) -> Self {
        self.label_color = color;
        self
    }

    pub fn with_padding(mut self, padding: Vec2) -> Self {
        self.padding = padding;
        self
    }

    pub fn show(mut self, ui: &mut Ui) -> Response {
        let (response, galley) = self.allocate(ui);
        let (state, response) = interact(ui, response, self.value, self.label);
        draw_at(
            ui,
            self.theme,
            self.value,
            response,
            self.size,
            self.padding,
            galley,
            self.label_color,
            state,
        )
    }

    // pub fn show_at(mut self, ui: &mut Ui, id: Id, rect: Rect) -> Response {
    //     let response = self.interact_at(ui, id, rect);
    //     let (state, response) = interact(ui, response, self.value);
    //     draw_at(ui, self.value, response, state, self.theme)
    // }

    fn allocate(&mut self, ui: &mut Ui) -> (Response, Option<Arc<Galley>>) {
        let (extra_width, galley) = if let Some(text) = self.label.take() {
            let available_width = ui.available_width() - self.size.y - ui.spacing().item_spacing.y;
            let galley = text.into_galley(
                ui,
                Some(egui::TextWrapMode::Truncate),
                available_width,
                TextStyle::Body,
            );
            (
                galley.rect.width() + ui.spacing().item_spacing.y,
                Some(galley),
            )
        } else {
            (0.0, None)
        };
        let sense = if ui.is_enabled() {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        };
        // allocate
        let size = self.size + vec2(extra_width, 0.0);
        let size = size + self.padding + self.padding;
        let (_, response) = ui.allocate_exact_size(size, sense);
        (response, galley)
    }

    // fn interact_at(&mut self, ui: &mut Ui, id: Id, rect: Rect) -> Response {
    //     let sense = if ui.is_enabled() {
    //         egui::Sense::click()
    //     } else {
    //         egui::Sense::hover()
    //     };
    //     // just interact
    //     ui.interact(rect, id, sense)
    // }
}

impl Widget for Switch<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        self.show(ui)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn switch_custom_at(
    ui: &mut Ui,
    enabled: bool,
    value: &mut bool,
    rect: Rect,
    id: Id,
    knob_fill: Option<Color32>,
    on_fill: Option<Color32>,
    off_fill: Option<Color32>,
) -> Response {
    let sense = if enabled {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let mut response = ui.interact(rect, id, sense);
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    response = if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response
    };
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *value, "")
    });

    if ui.is_rect_visible(rect) {
        let how_on = ui.ctx().animate_bool(response.id, *value);
        let visuals = if enabled {
            ui.style().interact_selectable(&response, *value)
        } else {
            ui.visuals().widgets.inactive
        };

        // skip expansion, keep tight
        //let rect = rect.expand(visuals.expansion);

        let radius = 0.5 * rect.height();
        // bg_fill, bg_stroke, fg_stroke, expansion
        let bg_fill = if !enabled {
            visuals.bg_fill
        } else if *value {
            on_fill.unwrap_or(visuals.bg_fill)
        } else {
            off_fill.unwrap_or(visuals.bg_fill)
        };
        let fg_stroke = if enabled {
            visuals.fg_stroke
        } else {
            visuals.bg_stroke
        };
        ui.painter()
            .rect(rect.shrink(1.0), radius, bg_fill, visuals.bg_stroke);
        let circle_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
        let center = egui::pos2(circle_x, rect.center().y);
        ui.painter().circle(
            center,
            0.875 * radius,
            knob_fill.unwrap_or(visuals.fg_stroke.color),
            Stroke::new(0.7, fg_stroke.color),
        );
    }

    response
}

fn interact(
    ui: &Ui,
    response: Response,
    value: &mut bool,
    label: Option<WidgetText>,
) -> (WidgetState, Response) {
    let (state, mut response) = if response.is_pointer_button_down_on() {
        (WidgetState::Active, response)
    } else if response.has_focus() {
        (WidgetState::Focused, response)
    } else if response.hovered() || response.highlighted() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        (WidgetState::Hovered, response)
    } else if !ui.is_enabled() {
        (WidgetState::Disabled, response)
    } else {
        (WidgetState::Default, response)
    };

    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }

    let text = label.unwrap_or("".into());
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            ui.is_enabled(),
            *value,
            text.text(),
        )
    });

    (state, response)
}

#[allow(clippy::too_many_arguments)]
fn draw_at(
    ui: &mut Ui,
    theme: &Theme,
    value: &bool,
    response: Response,
    size: Vec2,
    padding: Vec2,
    galley: Option<Arc<Galley>>,
    label_color: Option<Color32>,
    _state: WidgetState,
) -> Response {
    let rect = response.rect;
    if ui.is_rect_visible(rect) {
        let how_on = ui.ctx().animate_bool(response.id, *value);

        let radius = 0.5 * size.y;
        let stroke_width = 0.5;
        let (bg_fill, frame_stroke, knob_fill, knob_stroke, text_color) = if theme.dark_mode {
            if ui.is_enabled() {
                if *value {
                    (
                        // on
                        theme.accent_dark(),
                        Stroke::new(stroke_width, theme.accent_dark()),
                        theme.neutral_50(),
                        Stroke::new(stroke_width, theme.neutral_300()),
                        theme.neutral_50(),
                    )
                } else {
                    (
                        // off
                        theme.neutral_800(),
                        Stroke::new(stroke_width, theme.neutral_600()),
                        theme.neutral_100(),
                        Stroke::new(stroke_width, theme.neutral_700()),
                        theme.neutral_50(),
                    )
                }
            } else {
                (
                    // disabled
                    theme.neutral_800(),
                    Stroke::new(stroke_width, theme.neutral_600()),
                    theme.neutral_700(),
                    Stroke::new(stroke_width, theme.neutral_600()),
                    theme.neutral_400(),
                )
            }
        } else {
            if ui.is_enabled() {
                if *value {
                    (
                        // on
                        theme.accent_light(),
                        Stroke::new(stroke_width, theme.accent_light()),
                        theme.neutral_50(),
                        Stroke::new(stroke_width, theme.neutral_300()),
                        theme.neutral_900(),
                    )
                } else {
                    (
                        // off
                        theme.neutral_200(),
                        Stroke::new(stroke_width, theme.neutral_400()),
                        theme.neutral_50(),
                        Stroke::new(stroke_width, theme.neutral_300()),
                        theme.neutral_900(),
                    )
                }
            } else {
                (
                    // disabled
                    theme.neutral_300(),
                    Stroke::new(stroke_width, theme.neutral_400()),
                    theme.neutral_400(),
                    Stroke::new(stroke_width, theme.neutral_300()),
                    theme.neutral_400(),
                )
            }
        };

        // switch
        let switch_rect = Rect::from_min_size(rect.min + padding, size);
        ui.painter()
            .rect(switch_rect, radius, bg_fill, frame_stroke);
        let circle_x = egui::lerp(
            (switch_rect.left() + radius)..=(switch_rect.right() - radius),
            how_on,
        );
        let center = egui::pos2(circle_x, switch_rect.center().y);
        ui.painter()
            .circle(center, radius.sub(1.0), knob_fill, knob_stroke);

        // label
        if let Some(galley) = galley {
            let text_pos = switch_rect.right_top()
                + vec2(
                    ui.spacing().item_spacing.x,
                    (switch_rect.height() - galley.rect.height()) / 2.0 + 0.5,
                );
            ui.painter().galley_with_override_text_color(
                text_pos,
                galley,
                label_color.unwrap_or(text_color),
            );
        }

        if response.has_focus() {
            // focus ring
            // https://www.researchgate.net/publication/265893293_Approximation_of_a_cubic_bezier_curve_by_circular_arcs_and_vice_versa
            // figure 4, formula 7
            const K: f32 = 0.551_915_05; // 0.5519150244935105707435627;
            const PHI: f32 = std::f32::consts::PI / 4.0; // 1/8 of circle
            const GROW: f32 = 3.0; // amount to increase radius over switch rounding5
            let mut rect = switch_rect.expand(GROW);
            let rad = rect.height() / 2.0;
            rect.set_width(rect.height());
            let center = rect.center();
            let p1 = Vec2 {
                x: -rad * f32::cos(PHI),
                y: rad * f32::sin(PHI),
            };
            let p4 = Vec2 {
                x: -rad * f32::cos(PHI),
                y: -rad * f32::sin(PHI),
            };
            let p2 = Vec2 {
                x: p1.x - K * rad * f32::sin(PHI),
                y: p1.y - K * rad * f32::cos(PHI),
            };
            let p3 = Vec2 {
                x: p4.x - K * rad * f32::sin(PHI),
                y: p4.y + K * rad * f32::cos(PHI),
            };
            let points = [center + p1, center + p2, center + p3, center + p4];
            let ring = egui::epaint::CubicBezierShape::from_points_stroke(
                points,
                false,
                Color32::TRANSPARENT,
                egui::Stroke::new(1.0, frame_stroke.color),
            );
            ui.painter().add(ring);
        }
    }

    response
}
