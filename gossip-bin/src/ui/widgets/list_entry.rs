use eframe::egui::{FontSelection, Ui, WidgetText};
use eframe::epaint;
use egui_winit::egui::widget_text::WidgetTextGalley;
use egui_winit::egui::{
    self, pos2, vec2, Align, Color32, CursorIcon, FontId, Frame, Id, Pos2, Rect, Response,
    Rounding, Sense, Stroke,
};

/// Spacing of frame: left
pub(crate) const OUTER_MARGIN_LEFT: f32 = 0.0;
/// Spacing of frame: right
pub(crate) const OUTER_MARGIN_RIGHT: f32 = 5.0;
/// Spacing of frame: top
pub(crate) const OUTER_MARGIN_TOP: f32 = 5.0;
/// Spacing of frame: bottom
pub(crate) const OUTER_MARGIN_BOTTOM: f32 = 5.0;
/// Start of text (excl. outer margin): left
pub(crate) const TEXT_LEFT: f32 = 15.0;
/// Start of text (excl. outer margin): right
pub(crate) const TEXT_RIGHT: f32 = 20.0;
/// Start of text (excl. outer margin): top
pub(crate) const TEXT_TOP: f32 = 20.0;
/// Start of text (excl. outer margin): bottom
pub(crate) const TEXT_BOTTOM: f32 = 20.0;
/// Title font size
pub(crate) const TITLE_FONT_SIZE: f32 = 16.5;
/// Thickness of separator
const HLINE_THICKNESS: f32 = 1.5;

// ---- list view functions ----

pub(crate) fn allocate_space(ui: &mut Ui, height: f32) -> (Rect, Response) {
    let available_width = ui.available_size_before_wrap().x;
    ui.allocate_exact_size(vec2(available_width, height), Sense::hover())
}

pub(crate) fn paint_frame(ui: &mut Ui, rect: &Rect, fill: Option<Color32>) {
    let frame_rect = Rect::from_min_max(
        rect.min + vec2(OUTER_MARGIN_LEFT, OUTER_MARGIN_TOP),
        rect.max - vec2(OUTER_MARGIN_RIGHT, OUTER_MARGIN_BOTTOM),
    );
    let fill = fill.unwrap_or(ui.visuals().extreme_bg_color);
    ui.painter().add(epaint::RectShape {
        rect: frame_rect,
        rounding: Rounding::same(5.0),
        fill,
        stroke: Stroke::NONE,
        fill_texture_id: Default::default(),
        uv: Rect::ZERO,
    });
}

pub(crate) fn make_frame(ui: &Ui) -> Frame {
    Frame::none()
        .inner_margin(egui::Margin {
            left: TEXT_LEFT - OUTER_MARGIN_LEFT,
            right: TEXT_RIGHT - OUTER_MARGIN_RIGHT,
            top: TEXT_TOP - OUTER_MARGIN_TOP,
            bottom: TEXT_TOP - OUTER_MARGIN_BOTTOM,
        })
        .outer_margin(egui::Margin {
            left: OUTER_MARGIN_LEFT,
            right: OUTER_MARGIN_RIGHT,
            top: OUTER_MARGIN_TOP,
            bottom: OUTER_MARGIN_BOTTOM,
        })
        .fill(ui.visuals().extreme_bg_color)
        .rounding(egui::Rounding::same(5.0))
}

// ---- helper functions ----

pub(crate) fn paint_hline(ui: &mut Ui, rect: &Rect, y_pos: f32) {
    let painter = ui.painter();
    painter.hline(
        (rect.left() + TEXT_LEFT + 1.0)..=(rect.right() - TEXT_RIGHT - 1.0),
        painter.round_to_pixel(rect.top() + TEXT_TOP + y_pos),
        Stroke::new(HLINE_THICKNESS, ui.visuals().panel_fill),
    );
}

pub(crate) fn text_to_galley(ui: &mut Ui, text: WidgetText, align: Align) -> WidgetTextGalley {
    let mut text_job = text.into_text_job(
        ui.style(),
        FontSelection::Default,
        ui.layout().vertical_align(),
    );
    text_job.job.halign = align;
    ui.fonts(|f| text_job.into_galley(f))
}

pub(crate) fn text_to_galley_max_width(
    ui: &mut Ui,
    text: WidgetText,
    align: Align,
    max_width: f32,
) -> WidgetTextGalley {
    let mut text_job = text.into_text_job(
        ui.style(),
        FontSelection::Default,
        ui.layout().vertical_align(),
    );
    text_job.job.halign = align;
    text_job.job.wrap.break_anywhere = true;
    text_job.job.wrap.max_rows = 1;
    text_job.job.wrap.max_width = max_width;
    ui.fonts(|f| text_job.into_galley(f))
}

pub(crate) fn allocate_text_at(
    ui: &mut Ui,
    pos: Pos2,
    text: WidgetText,
    align: Align,
    id: Id,
) -> (WidgetTextGalley, Response) {
    let galley = text_to_galley(ui, text, align);
    let grect = galley.galley.rect;
    let rect = if align == Align::Min {
        Rect::from_min_size(pos, galley.galley.rect.size())
    } else if align == Align::Center {
        Rect::from_min_max(
            pos2(pos.x - grect.width() / 2.0, pos.y),
            pos2(pos.x + grect.width() / 2.0, pos.y + grect.height()),
        )
    } else {
        Rect::from_min_max(
            pos2(pos.x - grect.width(), pos.y),
            pos2(pos.x, pos.y + grect.height()),
        )
    };
    let response = ui.interact(rect, id, Sense::click());
    (galley, response)
}

pub(crate) fn draw_text_galley_at(
    ui: &mut Ui,
    pos: Pos2,
    galley: WidgetTextGalley,
    color: Option<Color32>,
    underline: Option<Stroke>,
) -> Rect {
    let size = galley.galley.rect.size();
    let halign = galley.galley.job.halign;
    let color = color.or(Some(ui.visuals().text_color()));
    ui.painter().add(epaint::TextShape {
        pos,
        galley: galley.galley,
        override_text_color: color,
        underline: Stroke::NONE,
        angle: 0.0,
    });
    let rect = if halign == Align::LEFT {
        Rect::from_min_size(pos, size)
    } else {
        Rect::from_x_y_ranges(pos.x - size.x..=pos.x, pos.y..=pos.y + size.y)
    };
    if let Some(stroke) = underline {
        let stroke = Stroke::new(stroke.width, stroke.color.gamma_multiply(0.6));
        let line_height = ui.fonts(|f| f.row_height(&FontId::default()));
        let painter = ui.painter();
        painter.hline(
            rect.min.x..=rect.max.x,
            rect.min.y + line_height - 2.0,
            stroke,
        );
    }
    rect
}

pub(crate) fn draw_text_at(
    ui: &mut Ui,
    pos: Pos2,
    text: WidgetText,
    align: Align,
    color: Option<Color32>,
    underline: Option<Stroke>,
) -> Rect {
    let galley = text_to_galley(ui, text, align);
    let color = color.or(Some(ui.visuals().text_color()));
    draw_text_galley_at(ui, pos, galley, color, underline)
}

pub(crate) fn draw_link_at(
    ui: &mut Ui,
    id: Id,
    pos: Pos2,
    text: WidgetText,
    align: Align,
    enabled: bool,
    secondary: bool,
) -> Response {
    let (galley, response) = allocate_text_at(ui, pos, text, align, id);
    let response = if enabled {
        response.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        response
    };
    let hover_color = ui.visuals().widgets.hovered.fg_stroke.color;
    let (color, stroke) = if !secondary {
        if enabled {
            if response.hovered() {
                (ui.visuals().text_color(), Stroke::NONE)
            } else {
                (hover_color, Stroke::new(1.0, hover_color))
            }
        } else {
            (ui.visuals().weak_text_color(), Stroke::NONE)
        }
    } else {
        if enabled {
            if response.hovered() {
                (hover_color, Stroke::NONE)
            } else {
                (
                    ui.visuals().text_color(),
                    Stroke::new(1.0, ui.visuals().text_color()),
                )
            }
        } else {
            (ui.visuals().weak_text_color(), Stroke::NONE)
        }
    };
    draw_text_galley_at(ui, pos, galley, Some(color), Some(stroke));
    response
}
