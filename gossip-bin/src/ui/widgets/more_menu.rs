use eframe::{
    egui::{
        Margin, Rect, Response, Rounding, Sense, Stroke, TextStyle, WidgetInfo, WidgetText,
        WidgetType,
    },
    epaint::PathShape,
};
use egui_winit::egui::{self, vec2, AboveOrBelow, Align2, Id, Ui, Vec2};

use crate::ui::GossipUi;

static POPUP_MARGIN: Vec2 = Vec2 { x: 20.0, y: 16.0 };

#[derive(PartialEq)]
enum MoreMenuStyle {
    Simple,
    Bubble,
}

pub(in crate::ui) struct MoreMenuEntry<'a> {
    text: WidgetText,
    action: Box<dyn FnOnce(&mut Ui, &mut GossipUi) + 'a>,
    enabled: bool,
}

impl<'a> MoreMenuEntry<'a> {
    pub fn new(
        text: impl Into<WidgetText>,
        action: Box<dyn FnOnce(&mut Ui, &mut GossipUi) + 'a>,
    ) -> Self {
        Self {
            text: text.into(),
            action,
            enabled: true,
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    fn show(self, app: &mut GossipUi, ui: &mut Ui) -> Response {
        ui.set_enabled(self.enabled);

        let theme = &app.theme;

        // layout
        let desired_size = vec2(ui.available_width(), 32.0);

        // interact
        let (rect, response) = ui.allocate_at_least(desired_size, Sense::click());
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, self.text.text()));
        let state = super::interact_widget_state(ui, &response);

        let galley = self
            .text
            .into_galley(ui, None, desired_size.x, TextStyle::Button);

        // draw
        let no_background = egui::Color32::TRANSPARENT;
        let no_stroke = Stroke::NONE;
        let neutral_100_stroke = Stroke::new(1.0, theme.neutral_100());
        let neutral_300_stroke = Stroke::new(1.0, theme.neutral_300());
        let neutral_800_stroke = Stroke::new(1.0, theme.neutral_800());
        let neutral_900_stroke = Stroke::new(1.0, theme.neutral_900());
        let neutral_950_stroke = Stroke::new(1.0, theme.neutral_950());
        let (bg_fill, text_color, separator_stroke, under_stroke) = if theme.dark_mode {
            match state {
                super::WidgetState::Default => (
                    no_background,
                    theme.neutral_300(),
                    neutral_800_stroke,
                    no_stroke,
                ),
                super::WidgetState::Hovered => (
                    theme.neutral_900(),
                    theme.neutral_100(),
                    neutral_950_stroke,
                    no_stroke,
                ),
                super::WidgetState::Active => (
                    no_background,
                    theme.accent_dark(),
                    neutral_800_stroke,
                    no_stroke,
                ),
                super::WidgetState::Disabled => (
                    no_background,
                    theme.neutral_600(),
                    neutral_800_stroke,
                    no_stroke,
                ),
                super::WidgetState::Focused => (
                    theme.neutral_900(),
                    theme.neutral_100(),
                    neutral_950_stroke,
                    neutral_100_stroke,
                ),
            }
        } else {
            match state {
                super::WidgetState::Default => (
                    no_background,
                    theme.neutral_800(),
                    neutral_300_stroke,
                    no_stroke,
                ),
                super::WidgetState::Hovered => (
                    theme.neutral_200(),
                    theme.neutral_900(),
                    neutral_300_stroke,
                    no_stroke,
                ),
                super::WidgetState::Active => (
                    no_background,
                    theme.accent_light(),
                    neutral_300_stroke,
                    no_stroke,
                ),
                super::WidgetState::Disabled => (
                    no_background,
                    theme.neutral_300(),
                    neutral_300_stroke,
                    no_stroke,
                ),
                super::WidgetState::Focused => (
                    theme.neutral_200(),
                    theme.neutral_900(),
                    neutral_300_stroke,
                    neutral_900_stroke,
                ),
            }
        };

        // background & separator lines
        ui.painter().rect_filled(rect, Rounding::same(0.0), bg_fill);
        ui.painter()
            .hline(rect.x_range(), rect.top(), separator_stroke);
        ui.painter()
            .hline(rect.x_range(), rect.bottom(), separator_stroke);

        // text
        let text_pos = {
            // Make sure button text is centered if within a centered layout
            ui.layout().align_size_within_rect(galley.size(), rect).min
        };
        let painter = ui.painter();
        painter.galley(text_pos, galley.clone(), text_color);
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

        // process action
        if response.clicked() {
            (self.action)(ui, app);
        }

        response
    }
}

pub(in crate::ui) struct MoreMenu {
    id: Id,
    min_size: Vec2,
    max_size: Vec2,
    above_or_below: Option<AboveOrBelow>,
    hover_text: Option<String>,
    style: MoreMenuStyle,
}

impl MoreMenu {
    pub fn simple(id: Id) -> Self {
        Self {
            id,
            min_size: Vec2 { x: 0.0, y: 0.0 },
            max_size: Vec2 {
                x: f32::INFINITY,
                y: f32::INFINITY,
            },
            above_or_below: None,
            hover_text: None,
            style: MoreMenuStyle::Simple,
        }
    }

    pub fn bubble(id: Id) -> Self {
        Self {
            id,
            min_size: Vec2 { x: 0.0, y: 0.0 },
            max_size: Vec2 {
                x: f32::INFINITY,
                y: f32::INFINITY,
            },
            above_or_below: None,
            hover_text: None,
            style: MoreMenuStyle::Bubble,
        }
    }

    #[allow(unused)]
    pub fn with_id(mut self, id: impl Into<Id>) -> Self {
        self.id = id.into();
        self
    }

    #[allow(unused)]
    pub fn with_min_size(mut self, min_size: Vec2) -> Self {
        self.min_size = min_size;
        self
    }

    #[allow(unused)]
    pub fn with_max_size(mut self, max_size: Vec2) -> Self {
        self.max_size = max_size;
        self
    }

    #[allow(unused)]
    pub fn with_hover_text(mut self, text: String) -> Self {
        self.hover_text = Some(text);
        self
    }

    #[allow(unused)]
    pub fn place_above(mut self, above: bool) -> Self {
        self.above_or_below = if above {
            Some(AboveOrBelow::Above)
        } else {
            Some(AboveOrBelow::Below)
        };
        self
    }

    pub fn show_entries(
        &self,
        ui: &mut Ui,
        app: &mut GossipUi,
        response: Response,
        content: Vec<MoreMenuEntry>,
    ) {
        let mut active = self.load_state(ui);

        if !ui.is_rect_visible(response.rect) {
            active = false; // close menu when the button goes out of view
        };

        let response = if let Some(text) = &self.hover_text {
            if !active {
                response.on_hover_text(text)
            } else {
                response
            }
        } else {
            response
        };

        if response.clicked() {
            active ^= true;
        }

        let pos = response.rect.center();
        let above_or_below = self
            .above_or_below
            .unwrap_or(select_above_or_below(ui, pos));
        let bg_color = if app.theme.dark_mode {
            app.theme.neutral_950()
        } else {
            app.theme.neutral_100()
        };
        let (pivot, fixed_pos, polygon) = match above_or_below {
            AboveOrBelow::Above => {
                let origin_pos = response.rect.center_top();
                let pivot = select_pivot(ui, origin_pos, AboveOrBelow::Above);
                let fixed_pos = match self.style {
                    MoreMenuStyle::Simple => match pivot {
                        Align2::RIGHT_BOTTOM => origin_pos + vec2(super::DROPDOWN_DISTANCE, -5.0),
                        Align2::LEFT_BOTTOM => origin_pos + vec2(-super::DROPDOWN_DISTANCE, -5.0),
                        _ => origin_pos,
                    },
                    MoreMenuStyle::Bubble => match pivot {
                        Align2::RIGHT_BOTTOM => {
                            origin_pos
                                + vec2(2.0 * super::DROPDOWN_DISTANCE, -super::DROPDOWN_DISTANCE)
                        }
                        Align2::LEFT_BOTTOM => {
                            origin_pos
                                + vec2(-2.0 * super::DROPDOWN_DISTANCE, -super::DROPDOWN_DISTANCE)
                        }
                        _ => origin_pos,
                    },
                };
                let path = PathShape::convex_polygon(
                    [
                        origin_pos,
                        origin_pos + vec2(super::DROPDOWN_DISTANCE, -super::DROPDOWN_DISTANCE),
                        origin_pos + vec2(-super::DROPDOWN_DISTANCE, -super::DROPDOWN_DISTANCE),
                    ]
                    .to_vec(),
                    bg_color,
                    egui::Stroke::NONE,
                );
                (pivot, fixed_pos, path)
            }
            AboveOrBelow::Below => {
                let origin_pos = response.rect.center_bottom();
                let pivot = select_pivot(ui, origin_pos, AboveOrBelow::Below);
                let fixed_pos = match self.style {
                    MoreMenuStyle::Simple => match pivot {
                        Align2::RIGHT_TOP => origin_pos + vec2(super::DROPDOWN_DISTANCE, 5.0),
                        Align2::LEFT_TOP => origin_pos + vec2(-super::DROPDOWN_DISTANCE, 5.0),
                        _ => origin_pos,
                    },
                    MoreMenuStyle::Bubble => match pivot {
                        Align2::RIGHT_TOP => {
                            origin_pos
                                + vec2(2.0 * super::DROPDOWN_DISTANCE, super::DROPDOWN_DISTANCE)
                        }
                        Align2::LEFT_TOP => {
                            origin_pos
                                + vec2(-2.0 * super::DROPDOWN_DISTANCE, super::DROPDOWN_DISTANCE)
                        }
                        _ => origin_pos,
                    },
                };
                let path = PathShape::convex_polygon(
                    [
                        origin_pos,
                        origin_pos + vec2(super::DROPDOWN_DISTANCE, super::DROPDOWN_DISTANCE),
                        origin_pos + vec2(-super::DROPDOWN_DISTANCE, super::DROPDOWN_DISTANCE),
                    ]
                    .to_vec(),
                    bg_color,
                    egui::Stroke::NONE,
                );
                (pivot, fixed_pos, path)
            }
        };

        let mut frame = egui::Frame::menu(ui.style());
        let area = egui::Area::new(self.id)
            .movable(false)
            .interactable(true)
            .order(egui::Order::Foreground)
            .pivot(pivot)
            .fixed_pos(fixed_pos)
            .constrain(true);
        if active {
            let menuresp = area.show(ui.ctx(), |ui| {
                if self.style == MoreMenuStyle::Simple {
                    // menu style from egui/src/menu.rs:set_menu_style()
                    let style = ui.style_mut();
                    style.spacing.button_padding = vec2(2.0, 0.0);
                    style.visuals.widgets.active.bg_stroke = egui::Stroke::NONE;
                    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
                    style.visuals.widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
                    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
                } else {
                    let style = ui.style_mut();
                    style.spacing.item_spacing.y = 0.0;
                    frame.inner_margin = Margin::symmetric(0.0, 15.0);
                    frame.outer_margin = Margin::same(0.0);
                    frame.fill = bg_color;
                    frame.stroke = egui::Stroke::NONE;
                    frame.shadow = ui.style().visuals.popup_shadow;
                    frame.rounding = egui::Rounding::same(8.0);
                }
                frame.show(ui, |ui| {
                    ui.set_min_size(self.min_size);
                    ui.set_max_size(self.max_size);

                    if self.style == MoreMenuStyle::Bubble {
                        // draw origin pointer
                        ui.painter().add(polygon);
                    }

                    ui.vertical_centered_justified(|ui| {
                        // now show menu content
                        for entry in content {
                            if entry.show(app, ui).clicked() && active {
                                active = false;
                            }
                        }
                    })
                });
            });
            if menuresp.response.clicked_elsewhere() && !response.clicked() {
                // clicked outside the menu but not on the menu-button
                active = false;
            }
        }

        self.save_state(ui, active);
    }

    pub fn show(&self, ui: &mut Ui, response: Response, content: impl FnOnce(&mut Ui, &mut bool)) {
        let mut active = self.load_state(ui);

        let response = if let Some(text) = &self.hover_text {
            if !active {
                response.on_hover_text(text)
            } else {
                response
            }
        } else {
            response
        };

        if response.clicked() {
            active ^= true;
        }

        let pos = response.rect.center();
        let above_or_below = self
            .above_or_below
            .unwrap_or(select_above_or_below(ui, pos));
        let bg_color = ui.visuals().window_fill();
        let (pivot, fixed_pos, polygon) = match above_or_below {
            AboveOrBelow::Above => {
                let origin_pos = response.rect.center_top();
                let pivot = select_pivot(ui, origin_pos, AboveOrBelow::Above);
                let fixed_pos = match self.style {
                    MoreMenuStyle::Simple => match pivot {
                        Align2::RIGHT_BOTTOM => origin_pos + vec2(super::DROPDOWN_DISTANCE, -5.0),
                        Align2::LEFT_BOTTOM => origin_pos + vec2(-super::DROPDOWN_DISTANCE, -5.0),
                        _ => origin_pos,
                    },
                    MoreMenuStyle::Bubble => match pivot {
                        Align2::RIGHT_BOTTOM => {
                            origin_pos
                                + vec2(2.0 * super::DROPDOWN_DISTANCE, -super::DROPDOWN_DISTANCE)
                        }
                        Align2::LEFT_BOTTOM => {
                            origin_pos
                                + vec2(-2.0 * super::DROPDOWN_DISTANCE, -super::DROPDOWN_DISTANCE)
                        }
                        _ => origin_pos,
                    },
                };
                let path = PathShape::convex_polygon(
                    [
                        origin_pos,
                        origin_pos + vec2(super::DROPDOWN_DISTANCE, -super::DROPDOWN_DISTANCE),
                        origin_pos + vec2(-super::DROPDOWN_DISTANCE, -super::DROPDOWN_DISTANCE),
                    ]
                    .to_vec(),
                    bg_color,
                    egui::Stroke::NONE,
                );
                (pivot, fixed_pos, path)
            }
            AboveOrBelow::Below => {
                let origin_pos = response.rect.center_bottom();
                let pivot = select_pivot(ui, origin_pos, AboveOrBelow::Below);
                let fixed_pos = match self.style {
                    MoreMenuStyle::Simple => match pivot {
                        Align2::RIGHT_TOP => origin_pos + vec2(super::DROPDOWN_DISTANCE, 5.0),
                        Align2::LEFT_TOP => origin_pos + vec2(-super::DROPDOWN_DISTANCE, 5.0),
                        _ => origin_pos,
                    },
                    MoreMenuStyle::Bubble => match pivot {
                        Align2::RIGHT_TOP => {
                            origin_pos
                                + vec2(2.0 * super::DROPDOWN_DISTANCE, super::DROPDOWN_DISTANCE)
                        }
                        Align2::LEFT_TOP => {
                            origin_pos
                                + vec2(-2.0 * super::DROPDOWN_DISTANCE, super::DROPDOWN_DISTANCE)
                        }
                        _ => origin_pos,
                    },
                };
                let path = PathShape::convex_polygon(
                    [
                        origin_pos,
                        origin_pos + vec2(super::DROPDOWN_DISTANCE, super::DROPDOWN_DISTANCE),
                        origin_pos + vec2(-super::DROPDOWN_DISTANCE, super::DROPDOWN_DISTANCE),
                    ]
                    .to_vec(),
                    bg_color,
                    egui::Stroke::NONE,
                );
                (pivot, fixed_pos, path)
            }
        };

        let mut frame = egui::Frame::menu(ui.style());
        let area = egui::Area::new(self.id)
            .movable(false)
            .interactable(true)
            .order(egui::Order::Foreground)
            .pivot(pivot)
            .fixed_pos(fixed_pos)
            .constrain(true);
        if active {
            let menuresp = area.show(ui.ctx(), |ui| {
                if self.style == MoreMenuStyle::Simple {
                    // menu style from egui/src/menu.rs:set_menu_style()
                    let style = ui.style_mut();
                    style.spacing.button_padding = vec2(2.0, 0.0);
                    style.visuals.widgets.active.bg_stroke = egui::Stroke::NONE;
                    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
                    style.visuals.widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
                    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
                } else {
                    frame.fill = bg_color;
                    frame.stroke = egui::Stroke::NONE;
                    frame.shadow = ui.style().visuals.popup_shadow;
                    frame.rounding = egui::Rounding::same(5.0);
                    frame.inner_margin = egui::Margin::symmetric(POPUP_MARGIN.x, POPUP_MARGIN.y);
                }
                frame.show(ui, |ui| {
                    ui.set_min_size(self.min_size);
                    ui.set_max_size(self.max_size);

                    if self.style == MoreMenuStyle::Bubble {
                        // draw origin pointer
                        ui.painter().add(polygon);
                    }

                    // now show menu content
                    content(ui, &mut active);
                });
            });
            if menuresp.response.clicked_elsewhere() && !response.clicked() {
                // clicked outside the menu but not on the menu-button
                active = false;
            }
        }

        self.save_state(ui, active);
    }

    fn load_state(&self, ui: &mut Ui) -> bool {
        ui.ctx()
            .data_mut(|d| d.get_temp::<bool>(self.id).unwrap_or_default())
    }

    fn save_state(&self, ui: &mut Ui, state: bool) {
        ui.ctx().data_mut(|d| d.insert_temp(self.id, state));
    }
}

fn select_above_or_below(ui: &mut Ui, pos: egui::Pos2) -> AboveOrBelow {
    let center = ui.ctx().screen_rect().center();
    if pos.y > center.y {
        AboveOrBelow::Above
    } else {
        AboveOrBelow::Below
    }
}

fn select_pivot(ui: &mut Ui, pos: egui::Pos2, above_or_below: AboveOrBelow) -> Align2 {
    let center = ui.ctx().screen_rect().center();
    if pos.x > center.x {
        match above_or_below {
            AboveOrBelow::Above => Align2::RIGHT_BOTTOM,
            AboveOrBelow::Below => Align2::RIGHT_TOP,
        }
    } else {
        match above_or_below {
            AboveOrBelow::Above => Align2::LEFT_BOTTOM,
            AboveOrBelow::Below => Align2::LEFT_TOP,
        }
    }
}
