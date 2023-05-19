//#![allow(dead_code)]
use eframe::egui;
use egui::{widget_text::WidgetTextGalley, *};
use nostr_types::Unixtime;

use crate::{comms::ToOverlordMessage, db::DbRelay, globals::GLOBALS, ui::components};

const MARGIN_LEFT: f32 = 0.0;
const MARGIN_RIGHT: f32 = 5.0;
const MARGIN_TOP: f32 = 5.0;
const MARGIN_BOTTOM: f32 = 5.0;
const TEXT_LEFT: f32 = 20.0;
const TEXT_RIGHT: f32 = 25.0;
const TEXT_TOP: f32 = 15.0;
const BTN_SIZE: f32 = 20.0;

const READ_HOVER_TEXT: &str = "Where you actually read events from (including those tagging you, but also for other purposes).";
const INBOX_HOVER_TEXT: &str = "Where you tell others you read from. You should also check Read. These relays shouldn't require payment. It is recommended to have a few.";
const DISCOVER_HOVER_TEXT: &str = "Where you discover other people's relays lists.";
const WRITE_HOVER_TEXT: &str =
    "Where you actually write your events to. It is recommended to have a few.";
const OUTBOX_HOVER_TEXT: &str = "Where you tell others you write to. You should also check Write. It is recommended to have a few.";
const ADVERTISE_HOVER_TEXT: &str = "Where you advertise your relay list (inbox/outbox) to. It is recommended to advertise to lots of relays so that you can be found.";

#[derive(Clone, PartialEq)]
pub enum RelayEntryView {
    List,
    Edit,
}

#[derive(Clone)]
struct UsageBits {
    read: bool,
    write: bool,
    advertise: bool,
    inbox: bool,
    outbox: bool,
    discover: bool,
}

impl UsageBits {
    fn from_usage_bits(usage_bits: u64) -> Self {
        Self {
            read: usage_bits & DbRelay::READ == DbRelay::READ,
            write: usage_bits & DbRelay::WRITE == DbRelay::WRITE,
            advertise: usage_bits & DbRelay::ADVERTISE == DbRelay::ADVERTISE,
            inbox: usage_bits & DbRelay::INBOX == DbRelay::INBOX,
            outbox: usage_bits & DbRelay::OUTBOX == DbRelay::OUTBOX,
            discover: usage_bits & DbRelay::DISCOVER == DbRelay::DISCOVER,
        }
    }

    fn to_usage_bits(&self) -> u64 {
        let mut bits: u64 = 0;
        if self.read {
            bits |= DbRelay::READ
        }
        if self.write {
            bits |= DbRelay::WRITE
        }
        if self.advertise {
            bits |= DbRelay::ADVERTISE
        }
        if self.inbox {
            bits |= DbRelay::INBOX
        }
        if self.outbox {
            bits |= DbRelay::OUTBOX
        }
        if self.discover {
            bits |= DbRelay::DISCOVER
        }
        bits
    }
}

/// Relay Entry
///
/// A relay entry has different views, which can be chosen with the
/// show_<view> functions.
///
#[derive(Clone)]
pub struct RelayEntry {
    db_relay: DbRelay,
    view: RelayEntryView,
    active: bool,
    user_count: Option<usize>,
    usage: UsageBits,
    rounding: Rounding,
    fill: Option<Color32>,
    stroke: Option<Stroke>,
    accent: Option<Color32>,
    highlight: Option<Color32>,
    option_symbol: Option<TextureHandle>,
}

impl RelayEntry {
    pub fn new(db_relay: DbRelay) -> Self {
        let usage = UsageBits::from_usage_bits(db_relay.usage_bits);
        Self {
            db_relay,
            view: RelayEntryView::List,
            active: true,
            user_count: None,
            usage,
            rounding: Rounding::same(5.0),
            fill: None,
            stroke: None,
            accent: None,
            highlight: None,
            option_symbol: None,
        }
    }

    pub fn set_edit(&mut self, edit: bool) {
        if edit {
            self.view = RelayEntryView::Edit;
        } else {
            self.view = RelayEntryView::List;
        }
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    pub fn set_user_count(&mut self, count: usize) {
        self.user_count = Some(count);
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

    pub fn option_symbol(mut self, option_symbol: TextureHandle) -> Self {
        self.option_symbol = Some(option_symbol);
        self
    }

    pub fn view(&self) -> RelayEntryView {
        self.view.clone()
    }
}

impl RelayEntry {
    fn allocate_list_view(&self, ui: &mut Ui) -> (Rect, Response) {
        let available_width = ui.available_size_before_wrap().x;
        let height = 80.0;

        ui.allocate_exact_size(vec2(available_width, height), Sense::hover())
    }

    fn allocate_edit_view(&self, ui: &mut Ui) -> (Rect, Response) {
        let available_width = ui.available_size_before_wrap().x;
        let height = 300.0;

        ui.allocate_exact_size(vec2(available_width, height), Sense::hover())
    }

    fn paint_title(&self, ui: &mut Ui, rect: &Rect) {
        let text = RichText::new(self.db_relay.url.as_str()).size(16.5);
        let pos = rect.min + vec2(TEXT_LEFT, TEXT_TOP);
        draw_text_at(
            ui,
            pos,
            text.into(),
            Align::LEFT,
            Some(self.accent.unwrap_or(ui.visuals().text_color())),
            None,
        );
    }

    fn paint_frame(&self, ui: &mut Ui, rect: &Rect) {
        let mut outer_rect = rect.shrink2(vec2(0.0, MARGIN_TOP));
        outer_rect.set_right(outer_rect.right() - MARGIN_RIGHT); // margin
        let fill = if self.view == RelayEntryView::List {
            self.fill.unwrap_or(ui.style().visuals.faint_bg_color)
        } else {
            Color32::WHITE
        };
        ui.painter().add(epaint::RectShape {
            rect: outer_rect,
            rounding: self.rounding,
            fill,
            stroke: self.stroke.unwrap_or(Stroke::NONE),
        });
    }

    fn paint_edit_btn(&mut self, ui: &mut Ui, rect: &Rect) -> Response {
        let id: Id = (self.db_relay.url.to_string() + "edit_btn").into();
        if self.db_relay.usage_bits == 0 {
            let pos = rect.right_top() + vec2(-TEXT_RIGHT, 10.0 + MARGIN_TOP);
            let text = RichText::new("pick up & configure");
            let (galley, response) = allocate_text_right_align_at(ui, pos, text.into(), id);
            let (color, stroke) = if self.active {
                if response.hovered() {
                    let color = self
                        .accent
                        .unwrap_or(ui.style().visuals.widgets.hovered.fg_stroke.color);
                    (color, Stroke::new(1.0, color))
                } else {
                    (ui.visuals().text_color(), Stroke::NONE)
                }
            } else {
                (ui.visuals().widgets.inactive.fg_stroke.color, Stroke::NONE)
            };
            if self.active && response.clicked() {
                self.view = RelayEntryView::Edit;
            }
            response.clone().on_hover_cursor(CursorIcon::PointingHand);
            draw_text_galley_at(ui, pos, galley, Some(color), Some(stroke));
            return response;
        } else {
            let pos = rect.right_top() + vec2(-BTN_SIZE - TEXT_RIGHT, 10.0 + MARGIN_TOP);
            let btn_rect = Rect::from_min_size(pos, vec2(BTN_SIZE, BTN_SIZE));
            let response = ui.interact(btn_rect, id, Sense::click());
            let color = if response.hovered() {
                self.accent
                    .unwrap_or(ui.style().visuals.widgets.hovered.fg_stroke.color)
            } else {
                ui.visuals().text_color()
            };
            response.clone().on_hover_cursor(CursorIcon::PointingHand);
            if let Some(symbol) = &self.option_symbol {
                let mut mesh = Mesh::with_texture(symbol.into());
                mesh.add_rect_with_uv(
                    btn_rect.shrink(2.0),
                    Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                    color,
                );
                ui.painter().add(Shape::mesh(mesh));
            } else {
                let text = RichText::new("\u{2699}").size(20.0);
                draw_text_at(ui, pos, text.into(), Align::LEFT, Some(color), None);
            }
            return response;
        }
    }

    fn paint_close_btn(&mut self, ui: &mut Ui, rect: &Rect) -> Response {
        let id: Id = (self.db_relay.url.to_string() + "close_btn").into();
        let button_padding = ui.spacing().button_padding;
        let text =
            WidgetText::from("Close").into_galley(ui, Some(false), 0.0, TextStyle::Button);
        let mut desired_size = text.size() + 2.0 * button_padding;
        desired_size.y = desired_size.y.at_least(ui.spacing().interact_size.y);
        let pos = rect.right_bottom() + vec2(-TEXT_RIGHT, -10.0 - MARGIN_BOTTOM) - desired_size;
        let btn_rect = Rect::from_min_size(pos, desired_size);
        let response = ui.interact(btn_rect, id, Sense::click());
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, text.text()));

        let visuals = ui.style().interact(&response);

        {
            let fill = visuals.weak_bg_fill;
            let stroke = visuals.bg_stroke;
            let rounding = visuals.rounding;
            ui.painter()
                .rect(btn_rect.expand(visuals.expansion), rounding, fill, stroke);
        }

        let text_pos = ui
            .layout()
            .align_size_within_rect(text.size(), btn_rect.shrink2(button_padding))
            .min;
        text.paint_with_visuals(ui.painter(), text_pos, visuals);

        if response.clicked() {
            self.view = RelayEntryView::List;
        }

        return response;
    }

    fn paint_stats(&self, ui: &mut Ui, rect: &Rect, with_usage: bool) {
        {
            // ---- Success Rate ----
            let pos = rect.min + vec2(TEXT_LEFT, TEXT_TOP + 30.0);
            let text = RichText::new(format!(
                "Rate: {:.0}% ({})",
                self.db_relay.success_rate() * 100.0,
                self.db_relay.success_count
            ));
            draw_text_at(
                ui,
                pos,
                text.into(),
                Align::LEFT,
                Some(ui.visuals().text_color()),
                None,
            );

            // ---- Following ----
            let pos = pos + vec2(130.0, 0.0);
            let mut active = self.active;
            let text = if let Some(count) = self.user_count {
                RichText::new(format!("Following: {}", count))
            } else {
                active = false;
                RichText::new("Following: ---")
            };
            let id: Id = (self.db_relay.url.to_string() + "following_link").into();
            let (galley, response) = allocate_text_at(ui, pos, text.into(), id);
            let (color, stroke) = if !active {
                (ui.visuals().weak_text_color(), Stroke::NONE)
            } else if response.hovered() {
                let color = self
                    .accent
                    .unwrap_or(ui.style().visuals.widgets.hovered.fg_stroke.color);
                (color, Stroke::new(1.0, color))
            } else {
                let color = ui.visuals().text_color();
                (color, Stroke::new(1.0, color))
            };
            if response.clicked() {
                // TODO go to following page for this relay?
            }
            if active {
                response.on_hover_cursor(CursorIcon::PointingHand);
            }
            draw_text_galley_at(ui, pos, galley, Some(color), Some(stroke));

            // ---- Last event ----
            let pos = pos + vec2(120.0, 0.0);
            let mut ago = "".to_string();
            if let Some(at) = self.db_relay.last_general_eose_at {
                ago += crate::date_ago::date_ago(Unixtime(at as i64)).as_str();
            } else {
                ago += "?";
            }
            let text = RichText::new(format!("Last event: {}", ago));
            draw_text_at(
                ui,
                pos,
                text.into(),
                Align::LEFT,
                Some(ui.visuals().text_color()),
                None,
            );

            // ---- Last connection ----
            let pos = pos + vec2(120.0, 0.0);
            let mut ago = "".to_string();
            if let Some(at) = self.db_relay.last_connected_at {
                ago += crate::date_ago::date_ago(Unixtime(at as i64)).as_str();
            } else {
                ago += "?";
            }
            let text = RichText::new(format!("Last connection: {}", ago));
            draw_text_at(
                ui,
                pos,
                text.into(),
                Align::LEFT,
                Some(ui.visuals().text_color()),
                None,
            );
        }

        if with_usage {
            // usage bits
            let mut usage: Vec<&'static str> = Vec::new();
            if self.usage.read && self.usage.inbox {
                usage.push("public read");
            } else if self.usage.read {
                usage.push("private read");
            }
            if self.usage.write && self.usage.outbox {
                usage.push("public write");
            } else if self.usage.write {
                usage.push("private write");
            }
            if self.usage.advertise {
                usage.push("advertise")
            }
            if self.usage.discover {
                usage.push("discover")
            }
            let usage_str = usage
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<String>>()
                .join(", ");
            let usage_str = usage_str.trim_end_matches(", ");
            let pos = pos2(rect.max.x, rect.min.y) + vec2(-TEXT_RIGHT, TEXT_TOP + 30.0);
            draw_text_at(
                ui,
                pos,
                usage_str.into(),
                Align::RIGHT,
                Some(ui.visuals().text_color()),
                None,
            );
        }
    }

    fn paint_nip11(&self, ui: &mut Ui, rect: &Rect) {
        let align = egui::Align::LEFT;
        let pos = rect.left_top() + vec2(TEXT_LEFT, TEXT_TOP + 70.0 + 3.0);
        if let Some(doc) = &self.db_relay.nip11 {
            if let Some(contact) = &doc.contact {
                let rect = draw_text_at(ui, pos, contact.into(), align, None, None);
                let id: Id = (self.db_relay.url.to_string() + "copy_nip11_contact").into();
                let pos = pos + vec2(rect.width() + ui.spacing().item_spacing.x, 0.0);
                let text = RichText::new("\u{2398}");
                let (galley, response) = allocate_text_at(ui, pos, text.into(), id);
                if response.clicked() {
                    ui.output_mut(|o| {
                        o.copied_text = contact.to_string();
                        *GLOBALS.status_message.blocking_write() = "copied to clipboard".into();
                    });
                }
                response.on_hover_cursor(egui::CursorIcon::PointingHand);
                draw_text_galley_at(ui, pos, galley, None, None);
            }
            let pos = pos + vec2(0.0, 30.0);
            if let Some(desc) = &doc.description {
                let mut desc = desc.clone();
                desc.truncate(200); // TODO is this a good number?
                draw_text_at(ui, pos, desc.into(), align, None, None);
            }
            if let Some(pubkey) = &doc.pubkey {

            }
        }
    }

    fn paint_usage_settings(&mut self, ui: &mut Ui, rect: &Rect) {
        let pos = rect.right_top() + vec2(-TEXT_RIGHT - 250.0, TEXT_TOP + 70.0);
        let switch_size = ui.spacing().interact_size.y * egui::vec2(2.0, 1.0);
        { // ---- advertise ----
            let id: Id = (self.db_relay.url.to_string() + "advertise_switch").into();
            if components::switch_with_size_at(ui, &mut self.usage.advertise, switch_size, pos, id)
                .changed()
            {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::AdjustRelayUsageBit(
                        self.db_relay.url.clone(),
                        DbRelay::ADVERTISE,
                        self.usage.advertise,
                    ));
            }
            draw_text_at(
                ui,
                pos + vec2(ui.spacing().item_spacing.x + switch_size.x, 2.0),
                "Share publicly".into(),
                Align::LEFT,
                Some(ui.visuals().text_color()),
                None,
            );
        }
        let pos = pos + vec2(0.0, 30.0);
        { // ---- read ----
            let id: Id = (self.db_relay.url.to_string() + "read_switch").into();
            if components::switch_with_size_at(ui, &mut self.usage.read, switch_size, pos, id)
                .changed()
            {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::AdjustRelayUsageBit(
                        self.db_relay.url.clone(),
                        DbRelay::READ,
                        self.usage.read,
                    ));
            }
            draw_text_at(
                ui,
                pos + vec2(ui.spacing().item_spacing.x + switch_size.x, 2.0),
                "Read".into(),
                Align::LEFT,
                Some(ui.visuals().text_color()),
                None,
            );
        }
    }

    /// Do layout and position the galley in the ui, without painting it or adding widget info.
    fn update_list_view(mut self, ui: &mut Ui) -> Response {
        let (rect, mut response) = self.allocate_list_view(ui);

        // all the heavy lifting is only done if it's actually visible
        if ui.is_rect_visible(rect) {
            self.paint_frame(ui, &rect);
            self.paint_title(ui, &rect);
            response |= self.paint_edit_btn(ui, &rect);
            self.paint_stats(ui, &rect, true);
        }

        response
    }

    fn update_edit_view(mut self, ui: &mut Ui) -> Response {
        let (rect, mut response) = self.allocate_edit_view(ui);

        // all the heavy lifting is only done if it's actually visible
        if ui.is_rect_visible(rect) {
            self.paint_frame(ui, &rect);
            self.paint_title(ui, &rect);
            response |= self.paint_close_btn(ui, &rect);
            self.paint_stats(ui, &rect, false);
            self.paint_nip11(ui, &rect);
            paint_hline(ui, &rect, 60.0);
            self.paint_usage_settings(ui, &rect);
        }

        response
    }
}

impl Widget for RelayEntry {
    fn ui(self, ui: &mut Ui) -> Response {
        let response: Response;
        match self.view {
            RelayEntryView::List => response = self.update_list_view(ui),
            RelayEntryView::Edit => response = self.update_edit_view(ui),
        }

        response
    }
}

fn paint_hline(ui: &mut Ui, rect: &Rect, y_pos: f32) {
    let painter = ui.painter();
    painter.hline(
        (rect.left() + TEXT_LEFT + 1.0)..=(rect.right() - TEXT_RIGHT - 1.0),
        painter.round_to_pixel(rect.top() + TEXT_TOP + y_pos),
        Stroke::new( 2.0, Color32::from_gray(0xE0)),
    );
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

fn allocate_text_at(
    ui: &mut Ui,
    pos: Pos2,
    text: WidgetText,
    id: Id,
) -> (WidgetTextGalley, Response) {
    let galley = text_to_galley(ui, text, Align::LEFT);
    let response = ui.interact(
        Rect::from_min_size(pos, galley.galley.rect.size()),
        id,
        Sense::click(),
    );
    (galley, response)
}

fn allocate_text_right_align_at(
    ui: &mut Ui,
    pos: Pos2,
    text: WidgetText,
    id: Id,
) -> (WidgetTextGalley, Response) {
    let galley = text_to_galley(ui, text, Align::RIGHT);
    let grect = galley.galley.rect;
    let response = ui.interact(
        Rect::from_min_max(
            pos2(pos.x - grect.width(), pos.y),
            pos2(pos.x, pos.y + grect.height()),
        ),
        id,
        Sense::click(),
    );
    (galley, response)
}

fn draw_text_galley_at(
    ui: &mut Ui,
    pos: Pos2,
    galley: WidgetTextGalley,
    color: Option<Color32>,
    underline: Option<Stroke>,
) -> Rect {
    let size = galley.galley.rect.size();
    let color = color.or(Some(ui.visuals().text_color()));
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

fn draw_text_at(
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
