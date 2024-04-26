use eframe::{egui::Response, epaint::PathShape};
use egui_winit::egui::{self, vec2, AboveOrBelow, Align2, Id, Ui, Vec2};

static POPUP_MARGIN: Vec2 = Vec2 { x: 20.0, y: 16.0 };

#[derive(PartialEq)]
enum MoreMenuStyle {
    Simple,
    Bubble,
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

    pub fn show(&self, ui: &mut Ui, response: Response, content: impl FnOnce(&mut Ui, &mut bool)) {
        let mut active = self.load_state(ui);

        // let response = match self.style {
        //     MoreMenuStyle::Simple => {
        //         let text = egui::RichText::new("=").size(13.0);
        //         super::Button::primary(self.theme, text)
        //             .small(self.small_button)
        //             .show(ui)
        //     }
        //     MoreMenuStyle::Bubble => {
        //         let (response, painter) =
        //             ui.allocate_painter(vec2(20.0, 20.0), egui::Sense::click());
        //         let btn_rect = response.rect;
        //         let color = if response.hovered() {
        //             self.accent_color
        //         } else {
        //             ui.visuals().text_color()
        //         };
        //         let mut mesh = egui::Mesh::with_texture((&self.options_symbol).into());
        //         mesh.add_rect_with_uv(
        //             btn_rect.shrink(2.0),
        //             Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        //             color,
        //         );
        //         painter.add(egui::Shape::mesh(mesh));
        //         response
        //     }
        // };

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
                    // frame.shadow = egui::epaint::Shadow::NONE;
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
