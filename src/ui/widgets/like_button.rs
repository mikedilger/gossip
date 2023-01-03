use eframe::{egui, epaint};
use egui::{Color32, Pos2, Response, Sense, Shape, Ui, Vec2, Widget};
use epaint::{PathShape, Stroke};

pub struct LikeButton {}

impl LikeButton {
    fn paint(ui: &mut Ui, corner: Pos2) {
        ui.painter().add(Shape::Path(PathShape {
            points: vec![
                Pos2 {
                    x: corner.x + 8.0,
                    y: corner.y + 16.0,
                },
                Pos2 {
                    x: corner.x + 9.0,
                    y: corner.y + 15.0,
                },
                Pos2 {
                    x: corner.x + 12.0,
                    y: corner.y + 13.0,
                },
                Pos2 {
                    x: corner.x + 15.0,
                    y: corner.y + 9.0,
                },
                Pos2 {
                    x: corner.x + 16.0,
                    y: corner.y + 6.0,
                },
                Pos2 {
                    x: corner.x + 16.0,
                    y: corner.y + 4.0,
                },
                Pos2 {
                    x: corner.x + 15.0,
                    y: corner.y + 2.0,
                },
                Pos2 {
                    x: corner.x + 13.0,
                    y: corner.y + 0.0,
                },
                Pos2 {
                    x: corner.x + 12.0,
                    y: corner.y + 0.0,
                },
                Pos2 {
                    x: corner.x + 10.0,
                    y: corner.y + 1.0,
                },
                Pos2 {
                    x: corner.x + 8.0,
                    y: corner.y + 3.0,
                },
                Pos2 {
                    x: corner.x + 6.0,
                    y: corner.y + 1.0,
                },
                Pos2 {
                    x: corner.x + 4.0,
                    y: corner.y + 0.0,
                },
                Pos2 {
                    x: corner.x + 3.0,
                    y: corner.y + 0.0,
                },
                Pos2 {
                    x: corner.x + 1.0,
                    y: corner.y + 2.0,
                },
                Pos2 {
                    x: corner.x + 0.0,
                    y: corner.y + 4.0,
                },
                Pos2 {
                    x: corner.x + 0.0,
                    y: corner.y + 6.0,
                },
                Pos2 {
                    x: corner.x + 1.0,
                    y: corner.y + 9.0,
                },
                Pos2 {
                    x: corner.x + 4.0,
                    y: corner.y + 13.0,
                },
                Pos2 {
                    x: corner.x + 7.0,
                    y: corner.y + 15.0,
                },
                Pos2 {
                    x: corner.x + 8.0,
                    y: corner.y + 16.0,
                },
            ],
            closed: true,
            fill: Color32::TRANSPARENT,
            stroke: Stroke {
                width: 1.0,
                color: Color32::from_rgb(0x8d, 0x7f, 0x73),
            },
        }));
    }
}

impl Widget for LikeButton {
    fn ui(self, ui: &mut Ui) -> Response {
        let padding = ui.spacing().button_padding;
        let space = Vec2 {
            x: 16.0 + padding.x * 2.0,
            y: 16.0 + padding.y * 2.0,
        };
        let (id, rect) = ui.allocate_space(space);
        let response = ui.interact(rect, id, Sense::click());
        let shift = if response.is_pointer_button_down_on() {
            2.0
        } else {
            0.0
        };
        let pos = Pos2 {
            x: rect.min.x + padding.x + shift,
            y: rect.min.y + padding.y + shift,
        };
        Self::paint(ui, ui.painter().round_pos_to_pixels(pos));

        response
    }
}
