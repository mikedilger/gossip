use eframe::{
    egui::{
        Margin, Rect, Response, Rounding, Sense, Stroke, TextStyle, WidgetInfo, WidgetText,
        WidgetType,
    },
    epaint::PathShape,
};
use egui_winit::egui::{self, vec2, AboveOrBelow, Align2, Id, Ui, Vec2};

use crate::ui::{GossipUi, Theme};

const POPUP_MARGIN: Vec2 = Vec2 { x: 20.0, y: 16.0 };
const CORNER_RADIUS: f32 = 8.0;
const INNER_MARGIN: Margin = Margin {
    left: 0.0,
    right: 0.0,
    top: 15.0,
    bottom: 15.0,
};

#[derive(PartialEq)]
enum MoreMenuStyle {
    Simple,
    Bubble,
}

pub(in crate::ui) enum MoreMenuItem<'a> {
    Button(MoreMenuButton<'a>),
    SubMenu(MoreMenuSubMenu<'a>),
    Switch(MoreMenuSwitch<'a>),
}

#[allow(clippy::type_complexity)]
pub(in crate::ui) struct MoreMenuButton<'a> {
    text: WidgetText,
    on_hover_text: Option<WidgetText>,
    on_disabled_hover_text: Option<WidgetText>,
    action: Box<dyn FnOnce(&mut Ui, &mut GossipUi) + 'a>,
    enabled: bool,
}

impl<'a> MoreMenuButton<'a> {
    #[allow(clippy::type_complexity)]
    pub fn new(
        text: impl Into<WidgetText>,
        action: Box<dyn FnOnce(&mut Ui, &mut GossipUi) + 'a>,
    ) -> Self {
        Self {
            text: text.into(),
            on_hover_text: None,
            on_disabled_hover_text: None,
            action,
            enabled: true,
        }
    }

    /// Set an optional `on_hover_text`
    pub fn on_hover_text(mut self, text: impl Into<WidgetText>) -> Self {
        self.on_hover_text = Some(text.into());
        self
    }

    /// Set an optional `on_disabled_hover_text`
    pub fn on_disabled_hover_text(mut self, text: impl Into<WidgetText>) -> Self {
        self.on_disabled_hover_text = Some(text.into());
        self
    }

    /// Set `enabled` state of this button
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    fn calc_min_width(&self, ui: &Ui) -> f32 {
        let galley = ui.fonts(|f| {
            f.layout_no_wrap(
                self.text.text().into(),
                egui::TextStyle::Body.resolve(ui.style()),
                egui::Color32::BLACK,
            )
        });
        galley.rect.width()
    }

    pub fn show(self, app: &mut GossipUi, ui: &mut Ui) -> Response {
        if !self.enabled {
            ui.disable();
        }

        let response = draw_menu_button(
            ui,
            &app.theme,
            self.text,
            None,
            self.on_hover_text,
            self.on_disabled_hover_text,
        );

        // process action
        if response.clicked() {
            (self.action)(ui, app);
        }

        response
    }
}

pub(in crate::ui) struct MoreMenuSubMenu<'a> {
    title: WidgetText,
    items: Vec<MoreMenuItem<'a>>,
    enabled: bool,
    id: Id,
}

impl<'a> MoreMenuSubMenu<'a> {
    #[allow(clippy::type_complexity)]
    pub fn new(
        title: impl Into<WidgetText>,
        items: Vec<MoreMenuItem<'a>>,
        parent: &MoreMenu,
    ) -> Self {
        let title: WidgetText = title.into();
        let id = parent.id.with(title.text());
        Self {
            title,
            items,
            enabled: true,
            id,
        }
    }

    #[allow(unused)]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    fn calc_min_width(&self, ui: &Ui) -> f32 {
        let galley = ui.fonts(|f| {
            f.layout_no_wrap(
                self.title.text().into(),
                egui::TextStyle::Body.resolve(ui.style()),
                egui::Color32::BLACK,
            )
        });
        galley.rect.width()
    }

    fn close(&self, ui: &mut Ui) {
        save_state(ui, &self.id, false);

        // recurse
        for entry in &self.items {
            if let MoreMenuItem::SubMenu(menu) = entry {
                menu.close(ui);
            }
        }
    }

    pub fn show(self, app: &mut GossipUi, ui: &mut Ui) -> Response {
        if !self.enabled {
            ui.disable();
        }

        let mut open = load_state(ui, &self.id);

        let response = draw_menu_button(ui, &app.theme, self.title, Some(open), None, None);

        // TODO paint open/close arrow, use animation

        if response.hovered() {
            open = true;
        }

        // to fix egui's justify bug, determine the width of the submenu first
        let min_width = self
            .items
            .iter()
            .map(|item| match item {
                MoreMenuItem::Button(entry) => entry.calc_min_width(ui),
                MoreMenuItem::SubMenu(menu) => menu.calc_min_width(ui),
                MoreMenuItem::Switch(switch) => switch.calc_min_width(ui),
            })
            .reduce(f32::max)
            .unwrap_or(150.0)
            + 30.0;

        const SPACE: f32 = 5.0;
        let min_space = min_width - SPACE;

        // process action
        let (pivot, fixed_pos) =
            // try to the right first
            if (response.rect.right() + min_space) < ui.ctx().screen_rect().right() {
                if response.rect.top() < ui.ctx().screen_rect().center().y {
                    (Align2::LEFT_TOP, response.rect.right_top() + vec2(-SPACE, -INNER_MARGIN.top))
                } else {
                    (Align2::LEFT_BOTTOM, response.rect.right_bottom() + vec2(-SPACE, INNER_MARGIN.bottom))
                }
            } else {
                if response.rect.top() < ui.ctx().screen_rect().center().y {
                    (Align2::RIGHT_TOP, response.rect.left_top() + vec2(SPACE, -INNER_MARGIN.top))
                } else {
                    (Align2::RIGHT_BOTTOM, response.rect.left_bottom() + vec2(SPACE, INNER_MARGIN.bottom))
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
        if open {
            let menuresp = area.show(ui.ctx(), |ui| {
                let bg_color = if app.theme.dark_mode {
                    app.theme.neutral_950()
                } else {
                    app.theme.neutral_100()
                };

                let style = ui.style_mut();
                style.spacing.item_spacing.y = 0.0;
                frame.inner_margin = INNER_MARGIN;
                frame.outer_margin = Margin::same(0.0);
                frame.fill = bg_color;
                frame.stroke = egui::Stroke::NONE;
                frame.shadow = ui.style().visuals.popup_shadow;
                frame.rounding = egui::Rounding::same(CORNER_RADIUS);

                frame.show(ui, |ui| {
                    ui.set_max_width(min_width);
                    ui.vertical_centered_justified(|ui| {
                        // now show menu content
                        for item in self.items {
                            ui.scope(|ui| match item {
                                MoreMenuItem::Button(entry) => {
                                    if entry.show(app, ui).clicked() && open {
                                        open = false;
                                    }
                                }
                                MoreMenuItem::SubMenu(menu) => {
                                    if menu.show(app, ui).clicked() && open {
                                        open = false;
                                    }
                                }
                                MoreMenuItem::Switch(switch) => {
                                    switch.show(app, ui);
                                }
                            });
                        }
                    })
                });
            });

            let menu_hovered = if let Some(pos) = ui.ctx().pointer_latest_pos() {
                menuresp.response.rect.contains(pos)
            } else {
                false
            };

            // if cursor leaves button or submenu, close the submenu
            if !(ui.rect_contains_pointer(response.rect) || menu_hovered) {
                open = false;
            }
        } else {
            // close all sub-menu's
            for item in self.items {
                if let MoreMenuItem::SubMenu(menu) = item {
                    menu.close(ui);
                }
            }
        }

        save_state(ui, &self.id, open);

        response
    }
}

pub(in crate::ui) struct MoreMenuSwitch<'a> {
    text: WidgetText,
    value: bool,
    action: Box<dyn FnOnce(&mut Ui, &mut GossipUi) + 'a>,
    enabled: bool,
}

impl<'a> MoreMenuSwitch<'a> {
    #[allow(clippy::type_complexity)]
    pub fn new(
        text: impl Into<WidgetText>,
        value: bool,
        action: Box<dyn FnOnce(&mut Ui, &mut GossipUi) + 'a>,
    ) -> Self {
        Self {
            text: text.into(),
            value,
            action,
            enabled: true,
        }
    }

    #[allow(unused)]
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    fn calc_min_width(&self, ui: &Ui) -> f32 {
        let galley = ui.fonts(|f| {
            f.layout_no_wrap(
                self.text.text().into(),
                egui::TextStyle::Body.resolve(ui.style()),
                egui::Color32::BLACK,
            )
        });
        galley.rect.width() + super::Switch::small_size().x
    }

    pub fn show(self, app: &mut GossipUi, ui: &mut Ui) -> Response {
        if !self.enabled {
            ui.disable();
        }

        let mut value = self.value;
        let response = super::Switch::small(&app.theme, &mut value)
            .with_label(self.text)
            .with_padding(vec2(10.0, 5.0))
            .show(ui);

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

    pub fn bubble(id: Id, min_size: Vec2, max_size: Vec2) -> Self {
        Self {
            id,
            min_size,
            max_size,
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
        content: Vec<MoreMenuItem>,
    ) {
        let mut active = load_state(ui, &self.id);

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
                        Align2::RIGHT_BOTTOM => {
                            response.rect.center() + vec2(CORNER_RADIUS, CORNER_RADIUS)
                        }
                        Align2::LEFT_BOTTOM => {
                            response.rect.center() + vec2(-CORNER_RADIUS, CORNER_RADIUS)
                        }
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
                        Align2::RIGHT_TOP => {
                            response.rect.center() + vec2(CORNER_RADIUS, -CORNER_RADIUS)
                        }
                        Align2::LEFT_TOP => {
                            response.rect.center() + vec2(-CORNER_RADIUS, -CORNER_RADIUS)
                        }
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
                    style.spacing.item_spacing.y = 0.0;
                    frame.inner_margin = INNER_MARGIN;
                    frame.outer_margin = Margin::same(0.0);
                    frame.fill = bg_color;
                    frame.stroke = egui::Stroke::NONE;
                    frame.shadow = ui.style().visuals.popup_shadow;
                    frame.rounding = egui::Rounding::same(CORNER_RADIUS);
                } else {
                    let style = ui.style_mut();
                    style.spacing.item_spacing.y = 0.0;
                    frame.inner_margin = INNER_MARGIN;
                    frame.outer_margin = Margin::same(0.0);
                    frame.fill = bg_color;
                    frame.stroke = egui::Stroke::NONE;
                    frame.shadow = ui.style().visuals.popup_shadow;
                    frame.rounding = egui::Rounding::same(CORNER_RADIUS);
                }
                frame.show(ui, |ui| {
                    ui.set_min_size(self.min_size);
                    ui.set_max_size(self.max_size);

                    let dot_color = if app.theme.dark_mode {
                        app.theme.neutral_700()
                    } else {
                        app.theme.neutral_200()
                    };

                    match self.style {
                        MoreMenuStyle::Simple => {
                            // draw pin
                            ui.painter().circle_filled(
                                response.rect.center(),
                                CORNER_RADIUS / 2.0,
                                dot_color,
                            );
                        }
                        MoreMenuStyle::Bubble => {
                            // draw origin pointer
                            ui.painter().add(polygon);
                        }
                    }

                    if ui
                        .interact(
                            response.rect,
                            ui.auto_id_with("areaclick"),
                            egui::Sense::click(),
                        )
                        .clicked()
                    {
                        active = false;
                    }

                    ui.vertical_centered_justified(|ui| {
                        // now show menu content
                        for item in content {
                            ui.scope(|ui| match item {
                                MoreMenuItem::Button(entry) => {
                                    if entry.show(app, ui).clicked() && active {
                                        active = false;
                                    }
                                }
                                MoreMenuItem::SubMenu(menu) => {
                                    if menu.show(app, ui).clicked() && active {
                                        active = false;
                                    }
                                }
                                MoreMenuItem::Switch(switch) => {
                                    switch.show(app, ui);
                                }
                            });
                        }
                    })
                });
            });
            if menuresp.response.clicked_elsewhere() && !response.clicked() {
                // clicked outside the menu but not on the menu-button
                active = false;
            }
        } else {
            // close all sub-menu's
            for item in content {
                if let MoreMenuItem::SubMenu(menu) = item {
                    menu.close(ui);
                }
            }
        }

        save_state(ui, &self.id, active);
    }

    pub fn show(&self, ui: &mut Ui, response: Response, content: impl FnOnce(&mut Ui, &mut bool)) {
        let mut active = load_state(ui, &self.id);

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
                    frame.rounding = egui::Rounding::same(CORNER_RADIUS);
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

        save_state(ui, &self.id, active);
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

fn load_state(ui: &mut Ui, id: &Id) -> bool {
    ui.ctx()
        .data_mut(|d| d.get_temp::<bool>(*id).unwrap_or_default())
}

fn save_state(ui: &mut Ui, id: &Id, state: bool) {
    ui.ctx().data_mut(|d| d.insert_temp(*id, state));
}

fn draw_menu_button(
    ui: &mut Ui,
    theme: &Theme,
    title: WidgetText,
    force_hover: Option<bool>,
    on_hover_text: Option<WidgetText>,
    on_disabled_hover_text: Option<WidgetText>,
) -> Response {
    // layout
    let desired_size = vec2(ui.available_width(), 32.0);

    // interact
    let (rect, mut response) = ui.allocate_at_least(desired_size, Sense::click());
    response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), title.text()));
    let state = super::interact_widget_state(ui, &response);
    let state = match state {
        super::WidgetState::Default => {
            if force_hover.unwrap_or_default() {
                super::WidgetState::Hovered
            } else {
                super::WidgetState::Default
            }
        }
        _ => state,
    };

    let galley = title.into_galley(ui, None, desired_size.x, TextStyle::Button);

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

    if let Some(text) = on_hover_text {
        response = response.on_hover_text(text);
    }
    if let Some(text) = on_disabled_hover_text {
        response = response.on_disabled_hover_text(text);
    }

    response
}
