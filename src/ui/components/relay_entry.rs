//#![allow(dead_code)]
use eframe::egui;
use egui::{widget_text::WidgetTextGalley, *};
use nostr_types::Unixtime;

use crate::db::DbRelay;

const MARGIN_LEFT: f32 = 0.0;
const MARGIN_RIGHT: f32 = 5.0;
const MARGIN_TOP: f32 = 5.0;
const MARGIN_BOTTOM: f32 = 5.0;
const TEXT_LEFT: f32 = 10.0;
const TEXT_RIGHT: f32 = 15.0;
const TEXT_TOP: f32 = 15.0;
const BTN_SIZE: f32 = 20.0;

enum RelayEntryView {
    List,
    Edit,
}

/// Relay Entry
///
/// A relay entry has different views, which can be chosen with the
/// show_<view> functions.
///
#[must_use = "You should put this widget in an ui with `ui.add(widget);`"]
pub struct RelayEntry<'a> {
    relay: &'a DbRelay,
    view: RelayEntryView,
    user_count: Option<usize>,
    rounding: Rounding,
    fill: Option<Color32>,
    stroke: Option<Stroke>,
    accent: Option<Color32>,
    highlight: Option<Color32>,
}

impl<'a> RelayEntry<'a> {
    pub fn new(relay: &'a DbRelay) -> Self {
        Self {
            relay,
            view: RelayEntryView::List,
            user_count: None,
            rounding: Rounding::same(5.0),
            fill: None,
            stroke: None,
            accent: None,
            highlight: None,
        }
    }

    pub fn user_count(mut self, count: usize) -> Self {
        self.user_count = Some(count);
        self
    }

    pub fn rounding(mut self, rounding: Rounding) -> Self {
        self.rounding = rounding;
        self
    }

    pub fn fill(mut self, fill: Color32) -> Self {
        self.fill = Some(fill);
        self
    }

    pub fn stroke(mut self, stroke: Stroke) -> Self {
        self.stroke = Some(stroke);
        self
    }

    pub fn accent(mut self, accent: Color32) -> Self {
        self.accent = Some(accent);
        self
    }

    pub fn highlight(mut self, highlight: Color32) -> Self {
        self.highlight = Some(highlight);
        self
    }
}

impl<'a> RelayEntry<'a> {
    fn allocate_list_view(&self, ui: &mut Ui) -> (Rect, Response) {
        let available_width = ui.available_size_before_wrap().x;
        let height = 80.0;

        ui.allocate_exact_size(
            vec2(available_width, height),
            Sense::focusable_noninteractive(),
        )
    }

    fn paint_title(&self, ui: &mut Ui, rect: &Rect) {
        let text = RichText::new(self.relay.url.as_str()).size(16.5);
        let pos = rect.min + vec2( TEXT_LEFT, TEXT_TOP);
        draw_text_at(ui, pos, text.into(), Align::LEFT, Some(ui.visuals().text_color()), None);
    }

    fn paint_frame(&self, ui: &mut Ui, rect: &Rect) {
        let mut outer_rect = rect.shrink2(vec2(0.0, MARGIN_TOP));
        outer_rect.set_right(outer_rect.right() - MARGIN_RIGHT); // margin
        ui.painter().add(epaint::RectShape {
            rect: outer_rect,
            rounding: self.rounding,
            fill: self.fill.unwrap_or(ui.style().visuals.faint_bg_color),
            stroke: self.stroke.unwrap_or(Stroke::NONE),
        });
    }

    fn paint_edit_btn(&self, ui: &mut Ui, rect: &Rect) {
        if self.relay.usage_bits == 0 {
            let pos = rect.right_top() + vec2(-TEXT_RIGHT, 10.0 + MARGIN_TOP);
            let text = RichText::new( "pick up & configure" );
            let (galley, response) = allocate_text_right_align_at(ui, pos, text.into());
            let (color, stroke) = if response.hovered() {
                let color = self.accent.unwrap_or(ui.style().visuals.widgets.hovered.fg_stroke.color);
                (color, Stroke::new(1.0, color))
            } else {
                (ui.visuals().text_color(), Stroke::NONE)
            };
            if response.clicked() {
                // TODO go to edit mode
            }
            draw_text_galley_at(ui, pos, galley, Some(color), Some(stroke));
        } else {
            let pos = rect.right_top() + vec2(-BTN_SIZE -TEXT_RIGHT, 10.0 + MARGIN_TOP);
            let btn_rect = Rect::from_min_size(pos, vec2(BTN_SIZE, BTN_SIZE));
            let response = ui.interact(
                btn_rect,
                ui.next_auto_id(),
                Sense::click());
            let color = if response.hovered() {
                self.accent.unwrap_or(ui.style().visuals.widgets.hovered.fg_stroke.color)
            } else {
                ui.visuals().text_color()
            };
            let text = RichText::new("\u{2699}").size(20.0);
            draw_text_at(ui, pos, text.into(), Align::LEFT, Some(color), None);
        }
    }

    fn paint_stats(&self, ui: &mut Ui, rect: &Rect) {
        { // stats
            let pos = rect.min + vec2(TEXT_LEFT, TEXT_TOP + 30.0);
            let text = RichText::new( format!("Rate: {:.0}% ({})",
                self.relay.success_rate() * 100.0,
                self.relay.success_count));
            draw_text_at(ui, pos, text.into(), Align::LEFT, Some(ui.visuals().text_color()), None);

            let pos = pos + vec2(120.0, 0.0);
            let mut ago = "".to_string();
            if let Some(at) = self.relay.last_general_eose_at {
                ago += crate::date_ago::date_ago(Unixtime(at as i64)).as_str();
            } else {
                ago += "?";
            }
            let text = RichText::new( format!("Last event: {}", ago ) );
            draw_text_at(ui, pos, text.into(), Align::LEFT, Some(ui.visuals().text_color()), None);

            let pos = pos + vec2(110.0, 0.0);
            let mut ago = "".to_string();
            if let Some(at) = self.relay.last_connected_at {
                ago += crate::date_ago::date_ago(Unixtime(at as i64)).as_str();
            } else {
                ago += "?";
            }
            let text = RichText::new( format!("Last connection: {}", ago ) );
            draw_text_at(ui, pos, text.into(), Align::LEFT, Some(ui.visuals().text_color()), None);

            let pos = pos + vec2(140.0, 0.0);
            if let Some(count) = self.user_count {
                let text = RichText::new( format!("Following: {}", count) );
                let (galley, response) = allocate_text_at(ui, pos, text.into());
                let (color, stroke) = if response.hovered() {
                    let color = self.accent.unwrap_or(ui.style().visuals.widgets.hovered.fg_stroke.color);
                    (color, Stroke::new(1.0, color))
                } else {
                    let color = ui.visuals().text_color();
                    (color, Stroke::new(1.0, color))
                };
                if response.clicked() {
                    // TODO go to following page for this relay?
                }
                draw_text_galley_at(ui, pos, galley, Some(color), Some(stroke));
            }
        }

        { // usage bits
            let mut usage: Vec<&'static str> = Vec::new();
            if self.relay.has_usage_bits(DbRelay::READ | DbRelay::INBOX) {
                usage.push("public read");
            } else if self.relay.has_usage_bits(DbRelay::READ) {
                usage.push("private read");
            }
            if self.relay.has_usage_bits(DbRelay::WRITE | DbRelay::OUTBOX) {
                usage.push("public write");
            } else if self.relay.has_usage_bits(DbRelay::WRITE) {
                usage.push("private write");
            }
            if self.relay.has_usage_bits(DbRelay::ADVERTISE) {
                usage.push("advertise")
            }
            if self.relay.has_usage_bits(DbRelay::DISCOVER) {
                usage.push("discover")
            }
            let usage_str = usage.iter().map(|s| s.to_string()).collect::<Vec<String>>().join(", ");
;
            let usage_str = usage_str.trim_end_matches(", ");
            let pos = pos2( rect.max.x, rect.min.y) + vec2(-TEXT_RIGHT, TEXT_TOP + 30.0);
            draw_text_at(ui, pos, usage_str.into(), Align::RIGHT, Some(ui.visuals().text_color()), None);
        }
    }

    /// Do layout and position the galley in the ui, without painting it or adding widget info.
    fn update_list_view(self, ui: &mut Ui) -> Response {
        let (rect, response) = self.allocate_list_view(ui);

        // all the heavy lifting is only done if it's actually visible
        if ui.is_rect_visible(rect) {
            self.paint_frame(ui, &rect);
            self.paint_title(ui, &rect);
            self.paint_edit_btn(ui, &rect);
            self.paint_stats(ui, &rect);
        }

        response
    }

    fn update_edit_view(self, ui: &mut Ui) -> Response {
        let (_, response) = self.allocate_list_view(ui);
        response
    }
}

impl<'a> Widget for RelayEntry<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let response: Response;
        match self.view {
            RelayEntryView::List => response = self.update_list_view(ui),
            RelayEntryView::Edit => response = self.update_edit_view(ui),
        }

        response
    }
}

fn text_to_galley(ui: &mut Ui, text: WidgetText, align: Align) -> WidgetTextGalley {
    let mut text_job = text.into_text_job(
        ui.style(),
        FontSelection::Default,
        ui.layout().vertical_align(),
    );
    text_job.job.halign = align;
    ui.fonts(|f| text_job.into_galley(f))
}

fn allocate_text_at(ui: &mut Ui, pos: Pos2, text: WidgetText) -> (WidgetTextGalley,Response) {
    let galley = text_to_galley(ui, text, Align::LEFT);
    let response = ui.interact(
        Rect::from_min_size(pos, galley.galley.rect.size()),
        ui.next_auto_id(),
        Sense::click());
    (galley, response)
}

fn allocate_text_right_align_at(ui: &mut Ui, pos: Pos2, text: WidgetText) -> (WidgetTextGalley,Response) {
    let galley = text_to_galley(ui, text, Align::RIGHT);
    let grect = galley.galley.rect;
    let response = ui.interact(
        Rect::from_min_max(
            pos2(pos.x - grect.width(), pos.y),
            pos2( pos.x, pos.y + grect.height())),
        ui.next_auto_id(),
        Sense::click());
    (galley, response)
}

fn draw_text_galley_at(ui: &mut Ui,
    pos: Pos2,
    galley: WidgetTextGalley,
    color: Option<Color32>,
    underline: Option<Stroke>) -> Rect {
    let size = galley.galley.rect.size();
    let underline = underline.unwrap_or(Stroke::NONE);
    ui.painter().add(epaint::TextShape {
        pos,
        galley: galley.galley,
        override_text_color: color,
        underline,
        angle: 0.0,
    });
    Rect::from_min_size(pos, size)
}

fn draw_text_at(ui: &mut Ui,
    pos: Pos2,
    text: WidgetText,
    align: Align,
    color: Option<Color32>,
    underline: Option<Stroke>) -> Rect {
    let galley = text_to_galley(ui, text, align);
    draw_text_galley_at(ui, pos, galley, color, underline)
}
