use std::ops::Sub;

use egui_winit::egui::{self, vec2, Color32, Id, Rect, Response, Stroke, Ui, Vec2, Widget};

use crate::ui::Theme;

use super::WidgetState;

pub struct Switch<'a> {
    value: &'a mut bool,
    size: Vec2,
    theme: &'a Theme,
}

impl<'a> Switch<'a> {
    pub fn small(theme: &'a Theme, value: &'a mut bool) -> Self {
        Self {
            value,
            size: vec2(29.0, 16.0),
            theme,
        }
    }

    pub fn large(theme: &'a Theme, value: &'a mut bool) -> Self {
        Self {
            value,
            size: vec2(40.0, 22.0),
            theme,
        }
    }

    #[deprecated(note = "Convert to small/large style")]
    pub fn onoff(theme: &'a Theme, value: &'a mut bool) -> Self {
        Self {
            value,
            size: vec2(29.0, 16.0),
            theme,
        }
    }

    fn allocate(&mut self, ui: &mut Ui) -> Response {
        let sense = if ui.is_enabled() {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        };
        // allocate the whole thing, switch + text
        let (_, response) = ui.allocate_exact_size(self.size, sense);
        response
    }
}

impl<'a> Widget for Switch<'a> {
    fn ui(mut self, ui: &mut Ui) -> Response {
        let response = self.allocate(ui);
        let (state, response) = interact(ui, response, self.value);
        draw_at(ui, self.value, response, state, self.theme)
    }
}

pub fn switch_with_size(ui: &mut Ui, on: &mut bool, size: egui::Vec2) -> Response {
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::click());
    switch_with_size_at(ui, on, size, rect.left_top(), ui.auto_id_with("switch"))
}

pub fn switch_with_size_at(
    ui: &mut Ui,
    value: &mut bool,
    size: egui::Vec2,
    pos: egui::Pos2,
    id: Id,
) -> Response {
    let rect = Rect::from_min_size(pos, size);
    switch_custom_at(ui, ui.is_enabled(), value, rect, id, None, None, None)
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
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, *value, ""));

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

fn interact(ui: &Ui, response: Response, value: &mut bool) -> (WidgetState, Response) {
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
        response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, *value, ""));
    }

    (state, response)
}

fn draw_at(
    ui: &mut Ui,
    value: &bool,
    response: Response,
    _state: WidgetState,
    theme: &Theme,
) -> Response {
    let rect = response.rect;
    if ui.is_rect_visible(rect) {
        let how_on = ui.ctx().animate_bool(response.id, *value);

        let radius = 0.5 * rect.height();
        let stroke_width = 0.5;
        let (bg_fill, frame_stroke, knob_fill, knob_stroke) = if theme.dark_mode {
            if ui.is_enabled() {
                if *value {
                    (
                        // on
                        theme.accent_dark(),
                        Stroke::new(stroke_width, theme.accent_dark()),
                        theme.neutral_50(),
                        Stroke::new(stroke_width, theme.neutral_300()),
                    )
                } else {
                    (
                        // off
                        theme.neutral_800(),
                        Stroke::new(stroke_width, theme.neutral_600()),
                        theme.neutral_100(),
                        Stroke::new(stroke_width, theme.neutral_700()),
                    )
                }
            } else {
                (
                    // disabled
                    theme.neutral_800(),
                    Stroke::new(stroke_width, theme.neutral_600()),
                    theme.neutral_700(),
                    Stroke::new(stroke_width, theme.neutral_600()),
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
                    )
                } else {
                    (
                        // off
                        theme.neutral_200(),
                        Stroke::new(stroke_width, theme.neutral_400()),
                        theme.neutral_50(),
                        Stroke::new(stroke_width, theme.neutral_300()),
                    )
                }
            } else {
                (
                    // disabled
                    theme.neutral_300(),
                    Stroke::new(stroke_width, theme.neutral_400()),
                    theme.neutral_400(),
                    Stroke::new(stroke_width, theme.neutral_300()),
                )
            }
        };

        ui.painter().rect(rect, radius, bg_fill, frame_stroke);
        let circle_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
        let center = egui::pos2(circle_x, rect.center().y);
        ui.painter()
            .circle(center, radius.sub(1.0), knob_fill, knob_stroke);
    }

    response
}
