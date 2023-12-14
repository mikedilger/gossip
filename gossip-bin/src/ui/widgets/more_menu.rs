use eframe::epaint::PathShape;
use egui_winit::egui::{self, vec2, Color32, Id, Rect, TextureHandle, Ui, Vec2};

use crate::ui::GossipUi;

pub(in crate::ui) struct MoreMenu {
    id: Id,
    min_size: Vec2,
    hover_text: Option<String>,
    accent_color: Color32,
    options_symbol: TextureHandle,
}

impl MoreMenu {
    pub fn new(ui: &mut Ui, app: &GossipUi) -> Self {
        Self {
            id: ui.next_auto_id(),
            min_size: Vec2 { x: 0.0, y: 0.0 },
            hover_text: None,
            accent_color: app.theme.accent_color(),
            options_symbol: app.options_symbol.clone(),
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
    pub fn with_hover_text(mut self, text: String) -> Self {
        self.hover_text = Some(text);
        self
    }

    pub fn show(&self, ui: &mut Ui, active: &mut bool, content: impl FnOnce(&mut Ui)) {
        let (response, painter) = ui.allocate_painter(vec2(20.0, 20.0), egui::Sense::click());
        let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        let response = if let Some(text) = &self.hover_text {
            if !*active {
                response.on_hover_text(text)
            } else {
                response
            }
        } else {
            response
        };
        let btn_rect = response.rect;
        let color = if response.hovered() {
            self.accent_color
        } else {
            ui.visuals().text_color()
        };
        let mut mesh = egui::Mesh::with_texture((&self.options_symbol).into());
        mesh.add_rect_with_uv(
            btn_rect.shrink(2.0),
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            color,
        );
        painter.add(egui::Shape::mesh(mesh));

        if response.clicked() {
            *active ^= true;
        }

        let button_center_bottom = response.rect.center_bottom();
        let seen_on_popup_position = button_center_bottom
            + vec2(
                -(self.min_size.x - 2.0 * super::DROPDOWN_DISTANCE),
                super::DROPDOWN_DISTANCE,
            );

        let mut frame = egui::Frame::popup(ui.style());
        let area = egui::Area::new(self.id)
            .movable(false)
            .interactable(true)
            .order(egui::Order::Foreground)
            .fixed_pos(seen_on_popup_position)
            .constrain(true);
        if *active {
            let menuresp = area.show(ui.ctx(), |ui| {
                frame.fill = self.accent_color;
                frame.stroke = egui::Stroke::NONE;
                // frame.shadow = egui::epaint::Shadow::NONE;
                frame.rounding = egui::Rounding::same(5.0);
                frame.inner_margin = egui::Margin::symmetric(20.0, 16.0);
                frame.show(ui, |ui| {
                    ui.set_min_size(self.min_size);
                    let path = PathShape::convex_polygon(
                        [
                            button_center_bottom,
                            button_center_bottom
                                + vec2(super::DROPDOWN_DISTANCE, super::DROPDOWN_DISTANCE),
                            button_center_bottom
                                + vec2(-super::DROPDOWN_DISTANCE, super::DROPDOWN_DISTANCE),
                        ]
                        .to_vec(),
                        self.accent_color,
                        egui::Stroke::NONE,
                    );
                    ui.painter().add(path);

                    // now show menu content
                    content(ui);
                });
            });
            if menuresp.response.clicked_elsewhere() && !response.clicked() {
                *active = false;
            }
        }
    }
}
