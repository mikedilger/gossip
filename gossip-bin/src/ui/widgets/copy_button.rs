use eframe::{
    egui,
    epaint::{self, ColorMode, PathStroke},
};
use egui::{Color32, Pos2, Response, Sense, Shape, Ui, Vec2, Widget};
use epaint::PathShape;

pub const COPY_SYMBOL_SIZE: Vec2 = Vec2::new(12.0, 12.0);

pub struct CopyButton {
    stroke: Option<PathStroke>,
}

impl CopyButton {
    pub(crate) fn new() -> Self {
        Self { stroke: None }
    }

    pub(crate) fn stroke(mut self, stroke: PathStroke) -> Self {
        self.stroke = Some(stroke);
        self
    }

    pub(crate) fn paint(&self, ui: &mut Ui, corner: Pos2) {
        ui.painter().add(Shape::Path(PathShape {
            points: vec![
                Pos2 {
                    x: corner.x + 2.0,
                    y: corner.y + 8.0,
                },
                Pos2 {
                    x: corner.x + 0.0,
                    y: corner.y + 8.0,
                },
                Pos2 {
                    x: corner.x + 0.0,
                    y: corner.y + 0.0,
                },
                Pos2 {
                    x: corner.x + 8.0,
                    y: corner.y + 0.0,
                },
                Pos2 {
                    x: corner.x + 8.0,
                    y: corner.y + 2.0,
                },
            ],
            closed: false,
            fill: Color32::TRANSPARENT,
            stroke: if let Some(stroke) = &self.stroke {
                stroke.clone()
            } else {
                PathStroke {
                    width: 1.0,
                    color: ColorMode::Solid(Color32::from_rgb(0x8d, 0x7f, 0x73)),
                }
            },
        }));

        ui.painter().add(Shape::Path(PathShape {
            points: vec![
                Pos2 {
                    x: corner.x + 4.0,
                    y: corner.y + 4.0,
                },
                Pos2 {
                    x: corner.x + 4.0,
                    y: corner.y + 12.0,
                },
                Pos2 {
                    x: corner.x + 12.0,
                    y: corner.y + 12.0,
                },
                Pos2 {
                    x: corner.x + 12.0,
                    y: corner.y + 4.0,
                },
                Pos2 {
                    x: corner.x + 4.0,
                    y: corner.y + 4.0,
                },
            ],
            closed: true,
            fill: Color32::TRANSPARENT,
            stroke: if let Some(stroke) = &self.stroke {
                stroke.clone()
            } else {
                PathStroke {
                    width: 1.0,
                    color: ColorMode::Solid(Color32::from_rgb(0x8d, 0x7f, 0x73)),
                }
            },
        }));
    }
}

impl Widget for CopyButton {
    fn ui(self, ui: &mut Ui) -> Response {
        let space = Vec2 { x: 16.0, y: 16.0 };
        let (id, rect) = ui.allocate_space(space);
        let response = ui.interact(rect, id, Sense::click());
        let shift = if response.is_pointer_button_down_on() {
            6.0
        } else {
            4.0
        };
        let pos = Pos2 {
            x: rect.min.x + shift,
            y: rect.min.y + shift,
        };
        self.paint(ui, ui.painter().round_pos_to_pixels(pos));

        response
    }
}
