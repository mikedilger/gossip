use eframe::{egui, epaint};
use egui::{Color32, Pos2, Response, Sense, Shape, Ui, Vec2, Widget};
use epaint::{PathShape, Stroke};
use tracing::debug;

pub struct CopyButton { }

impl CopyButton {
    pub fn new() -> CopyButton {
        CopyButton { }
    }

    fn paint(ui: &mut Ui, corner: Pos2) {
        ui.painter().add(Shape::Path(PathShape {
            points: vec![
                Pos2 { x: corner.x + 2.0, y: corner.y + 8.0 },
                Pos2 { x: corner.x + 0.0, y: corner.y + 8.0 },
                Pos2 { x: corner.x + 0.0, y: corner.y + 0.0 },
                Pos2 { x: corner.x + 8.0, y: corner.y + 0.0 },
                Pos2 { x: corner.x + 8.0, y: corner.y + 2.0 },
            ],
            closed: false,
            fill: Color32::TRANSPARENT,
            stroke: Stroke {
                width: 1.5,
                color: Color32::from_rgb(0xb1, 0xa2, 0x96)
            }
        }));

        ui.painter().add(Shape::Path(PathShape {
            points: vec![
                Pos2 { x: corner.x + 4.0, y: corner.y + 4.0 },
                Pos2 { x: corner.x + 4.0, y: corner.y + 12.0 },
                Pos2 { x: corner.x + 12.0, y: corner.y + 12.0 },
                Pos2 { x: corner.x + 12.0, y: corner.y + 4.0 },
                Pos2 { x: corner.x + 4.0, y: corner.y + 4.0 },
            ],
            closed: true,
            fill: Color32::TRANSPARENT,
            stroke: Stroke {
                width: 1.5,
                color: Color32::from_rgb(0xb1, 0xa2, 0x96)
            }
        }));
    }
}

impl Widget for CopyButton {
    fn ui(self, ui: &mut Ui) -> Response {
        let padding = ui.spacing().button_padding;
        let space = Vec2 {
            x: 12.0 + padding.x * 2.0,
            y: 12.0 + padding.y * 2.0
        };
        let (id, rect) = ui.allocate_space(space);
        let response = ui.interact(rect, id, Sense::click());
        let shift = if response.is_pointer_button_down_on() { 2.0 } else { 0.0 };
        let pos = Pos2 {
            x: rect.min.x + padding.x + shift,
            y: rect.min.y + padding.y + shift,
        };
        Self::paint(ui, pos);

        response
    }
}
