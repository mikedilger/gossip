use egui_winit::egui::{Ui, self, Id, Response, Rect, Widget, Color32, Stroke, vec2, Vec2};

use crate::ui::Theme;

pub struct Switch<'a> {
    value: &'a mut bool,
    size: Vec2,
    knob_fill: Option<Color32>,
    on_fill: Option<Color32>,
    off_fill: Option<Color32>,
}

impl<'a> Switch<'a> {
    #[allow(unused)]
    pub fn onoff(theme: &Theme, value: &'a mut bool) -> Self {
        Self {
            value,
            size: theme.get_style().spacing.interact_size.y * vec2(1.6, 0.8),
            knob_fill: Some(theme.get_style().visuals.extreme_bg_color),
            on_fill: Some(theme.accent_color()),
            off_fill: Some(theme.get_style().visuals.widgets.inactive.bg_fill),
        }
    }

    #[allow(unused)]
    pub fn toggle(theme: &Theme, value: &'a mut bool) -> Self {
        Self {
            value,
            size: theme.get_style().spacing.interact_size.y * vec2(1.6, 0.8),
            knob_fill: None,
            on_fill: None,
            off_fill: None,
        }
    }
}

impl<'a> Widget for Switch<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let (rect, _) = ui.allocate_exact_size(self.size, egui::Sense::hover());
        let id = ui.next_auto_id();
        switch_custom_at(ui,
            ui.is_enabled(),
            self.value,
            rect,
            id,
            self.knob_fill,
            self.on_fill,
            self.off_fill)
    }
}

pub fn switch_with_size(ui: &mut Ui, on: &mut bool, size: egui::Vec2) -> Response {
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::click());
    switch_with_size_at(ui, on, size, rect.left_top(), ui.next_auto_id())
}

pub fn switch_with_size_at(
    ui: &mut Ui,
    value: &mut bool,
    size: egui::Vec2,
    pos: egui::Pos2,
    id: Id,
) -> Response {
    let rect = Rect::from_min_size(pos, size);
    switch_custom_at(
        ui,
        ui.is_enabled(),
        value,
        rect,
        id,
        None,
        None,
        None)
}

pub fn switch_simple(ui: &mut Ui, on: bool) -> Response {
    let size = ui.spacing().interact_size.y * egui::vec2(1.6, 0.8);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::click());
    let rect = Rect::from_min_size(rect.left_top(), size);
    let id = ui.next_auto_id();
    let mut value = on;
    let mut response = switch_custom_at(ui,
        ui.is_enabled(),
        &mut value,
        rect,
        id,
        None,
        None,
        None);
    if value != on {
        response.mark_changed();
    }
    response
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
