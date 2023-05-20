//#![allow(dead_code)]
use eframe::egui;
use egui::{widget_text::WidgetTextGalley, *};
use nostr_types::{PublicKeyHex, Unixtime};

use crate::{comms::ToOverlordMessage, db::DbRelay, globals::GLOBALS, ui::components};

/// Height of the list view (width always max. available)
const LIST_VIEW_HEIGHT: f32 = 80.0;
/// Height of the edit view (width always max. available)
const EDIT_VIEW_HEIGHT: f32 = 250.0;
/// Spacing of frame: left
const OUTER_MARGIN_LEFT: f32 = 0.0;
/// Spacing of frame: right
const OUTER_MARGIN_RIGHT: f32 = 5.0;
/// Spacing of frame: top
const OUTER_MARGIN_TOP: f32 = 5.0;
/// Spacing of frame: bottom
const OUTER_MARGIN_BOTTOM: f32 = 5.0;
/// Start of text (excl. outer margin): left
const TEXT_LEFT: f32 = 20.0;
/// Start of text (excl. outer margin): right
const TEXT_RIGHT: f32 = 25.0;
/// Start of text (excl. outer margin): top
const TEXT_TOP: f32 = 15.0;
/// Y-offset for first separator
const HLINE_1_Y_OFFSET: f32 = 57.0;
/// Y-offset for second separator
const HLINE_2_Y_OFFSET: f32 = 190.0;
/// Thickness of separator
const HLINE_THICKNESS: f32 = 1.5;
/// Size of edit button
const EDIT_BTN_SIZE: f32 = 20.0;
/// Spacing of stats row to heading
const STATS_Y_SPACING: f32 = 30.0;
/// Distance of usage switch-left from TEXT_RIGHT
const USAGE_SWITCH_PULL_RIGHT: f32 = 300.0;
/// Spacing of usage switches: y direction
const USAGE_SWITCH_Y_SPACING: f32 = 30.0;
/// Spacing of usage switches: x direction
const USAGE_SWITCH_X_SPACING: f32 = 150.0;
/// Center offset of switch to text
const USAGE_SWITCH_Y_OFFSET: f32 = 2.5;
/// Center offset of line to text
const USAGE_LINE_Y_OFFSET: f32 = 7.25;
/// Line start as left offset from second switch
const USAGE_LINE_X_START: f32 = -60.0;
/// Line end as left offset from second switch
const USAGE_LINE_X_END: f32 = -10.0;
/// Line thickness
const USAGE_LINE_THICKNESS: f32 = 1.0;
/// Spacing between nip11 text rows
const NIP11_Y_SPACING: f32 = 20.0;
/// Copy symbol for nip11 items copy button
const COPY_SYMBOL: &'static str = "\u{2398}";
/// Max length of title string
const TITLE_MAX_LEN: usize = 50;

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
        let height = LIST_VIEW_HEIGHT;

        ui.allocate_exact_size(vec2(available_width, height), Sense::hover())
    }

    fn allocate_edit_view(&self, ui: &mut Ui) -> (Rect, Response) {
        let available_width = ui.available_size_before_wrap().x;
        let height = EDIT_VIEW_HEIGHT;

        ui.allocate_exact_size(vec2(available_width, height), Sense::hover())
    }

    fn paint_title(&self, ui: &mut Ui, rect: &Rect) {
        let mut title = safe_truncate(self.db_relay.url.as_str(), TITLE_MAX_LEN).to_string();
        if self.db_relay.url.0.len() > TITLE_MAX_LEN {
            title.push_str("\u{2026}"); // append ellipsis
        }
        let text = RichText::new(title).size(16.5);
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
        let frame_rect = Rect::from_min_max(
            rect.min + vec2(OUTER_MARGIN_LEFT, OUTER_MARGIN_TOP),
            rect.max - vec2(OUTER_MARGIN_RIGHT, OUTER_MARGIN_BOTTOM),
        );
        let fill = ui.style().visuals.extreme_bg_color;
        ui.painter().add(epaint::RectShape {
            rect: frame_rect,
            rounding: self.rounding,
            fill,
            stroke: self.stroke.unwrap_or(Stroke::NONE),
        });
    }

    fn paint_edit_btn(&mut self, ui: &mut Ui, rect: &Rect) -> Response {
        let id: Id = (self.db_relay.url.to_string() + "edit_btn").into();
        if self.db_relay.usage_bits == 0 {
            let pos = rect.right_top() + vec2(-TEXT_RIGHT, 10.0 + OUTER_MARGIN_TOP);
            let text = RichText::new("pick up & configure");
            let accent = self.accent
                .unwrap_or(ui.style().visuals.widgets.hovered.fg_stroke.color);
            let response = draw_link_at(ui, id, pos, text.into(), Align::RIGHT, self.active,false, accent);
            if self.active && response.clicked() {
                self.view = RelayEntryView::Edit;
            }
            return response;
        } else {
            let pos = rect.right_top() + vec2(-EDIT_BTN_SIZE - TEXT_RIGHT, 10.0 + OUTER_MARGIN_TOP);
            let btn_rect = Rect::from_min_size(pos, vec2(EDIT_BTN_SIZE, EDIT_BTN_SIZE));
            let response = ui.interact(btn_rect, id, Sense::click());
            let color = if response.hovered() {
                ui.visuals().text_color()
            } else {
                self.accent
                    .unwrap_or(ui.style().visuals.widgets.hovered.fg_stroke.color)
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
        let text = WidgetText::from("Close")
            .color( ui.visuals().extreme_bg_color )
            .into_galley(ui, Some(false), 0.0, TextStyle::Button);
        let mut desired_size = text.size() + 2.0 * button_padding;
        desired_size.y = desired_size.y.at_least(ui.spacing().interact_size.y);
        let pos =
            rect.right_bottom() + vec2(-TEXT_RIGHT, -10.0 - OUTER_MARGIN_BOTTOM) - desired_size;
        let btn_rect = Rect::from_min_size(pos, desired_size);
        let response = ui.interact(btn_rect, id, Sense::click());
        response.widget_info(|| WidgetInfo::labeled(WidgetType::Button, text.text()));
        response.clone().on_hover_cursor(egui::CursorIcon::PointingHand);

        let visuals = ui.style().interact(&response);
        let accent = self.accent.unwrap_or(ui.visuals().hyperlink_color);

        {
            let fill = if response.hovered() {
                let mut hsva: ecolor::HsvaGamma  = accent.into();
                hsva.v *= 0.8;
                hsva.into()
            } else {
                accent
            };
            let stroke = Stroke::new( visuals.bg_stroke.width, accent );
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

    fn paint_lower_buttons(&self, ui: &mut Ui, rect: &Rect) -> Response {
        let line_height = ui.fonts(|f| {
            f.row_height(&FontId::default())
        });
        let pos = rect.left_bottom() + vec2(TEXT_LEFT, -10.0 -OUTER_MARGIN_BOTTOM -line_height);
        let accent = self.accent.unwrap_or(ui.style().visuals.widgets.hovered.fg_stroke.color);
        let id: Id = (self.db_relay.url.to_string() + "remove_button").into();
        let text = "Remove from personal list";
        let response = draw_link_at(ui, id, pos, text.into(), Align::Min, self.active, true, accent);
        if response.clicked() {
            // TODO remove relay
        }

        let pos = pos + vec2(200.0, 0.0);
        let id: Id = (self.db_relay.url.to_string() + "disconnect_button").into();
        let text = "Force disconnect";
        let response = draw_link_at(ui, id, pos, text.into(), Align::Min, self.active, true, accent);
        if response.clicked() {
            let _ = GLOBALS.to_overlord.send(
                ToOverlordMessage::DropRelay(self.db_relay.url.to_owned()),
            );
        }
        // pass the response back so the page knows the edit view should close
        response
    }

    fn paint_stats(&self, ui: &mut Ui, rect: &Rect, with_usage: bool) {
        {
            // ---- Success Rate ----
            let pos = rect.min + vec2(TEXT_LEFT, TEXT_TOP + STATS_Y_SPACING);
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
            let accent = self.accent
                .unwrap_or(ui.style().visuals.widgets.hovered.fg_stroke.color);
            let response = draw_link_at(ui, id, pos, text.into(), Align::Min, active, true, accent);
            if response.clicked() {
                // TODO go to following page for this relay?
            }

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
        let pos = rect.left_top() + vec2(TEXT_LEFT, TEXT_TOP + 70.0);
        if let Some(doc) = &self.db_relay.nip11 {
            if let Some(contact) = &doc.contact {
                let rect = draw_text_at(ui, pos, contact.into(), align, None, None);
                let id: Id = (self.db_relay.url.to_string() + "copy_nip11_contact").into();
                let pos = pos + vec2(rect.width() + ui.spacing().item_spacing.x, 0.0);
                let text = RichText::new(COPY_SYMBOL);
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
            let pos = pos + vec2(0.0, NIP11_Y_SPACING);
            if let Some(desc) = &doc.description {
                let desc = safe_truncate(desc.as_str(), 200); // TODO is this a good number?
                draw_text_at(ui, pos, desc.into(), align, None, None);
            }
            let pos = pos + vec2(0.0, NIP11_Y_SPACING);
            if let Some(pubkey) = &doc.pubkey {
                if let Ok(pubhex) = PublicKeyHex::try_from_str(pubkey.as_str()) {
                    let npub = pubhex.as_bech32_string();
                    let rect = draw_text_at(ui, pos, npub.clone().into(), align, None, None);
                    let id: Id = (self.db_relay.url.to_string() + "copy_nip11_npub").into();
                    let pos = pos + vec2(rect.width() + ui.spacing().item_spacing.x, 0.0);
                    let text = RichText::new(COPY_SYMBOL);
                    let (galley, response) = allocate_text_at(ui, pos, text.into(), id);
                    if response.clicked() {
                        ui.output_mut(|o| {
                            o.copied_text = npub;
                            *GLOBALS.status_message.blocking_write() = "copied to clipboard".into();
                        });
                    }
                    response.on_hover_cursor(egui::CursorIcon::PointingHand);
                    draw_text_galley_at(ui, pos, galley, None, None);
                }
            }
            let pos = pos + vec2(0.0, NIP11_Y_SPACING);
            if doc.supported_nips.len() > 0 {
                let mut text = "NIPS: ".to_string();
                for nip in &doc.supported_nips {
                    text.push_str(format!(" {},", *nip).as_str());
                }
                text.truncate(text.len() - 1); // safe because we built the string
                draw_text_at(ui, pos, text.into(), align, None, None);
            }
        }
    }

    fn paint_usage_settings(&mut self, ui: &mut Ui, rect: &Rect) {
        let knob_fill = ui.visuals().extreme_bg_color;
        let on_fill = self.accent.unwrap_or(ui.visuals().widgets.active.bg_fill);
        let off_fill = ui.visuals().widgets.inactive.bg_fill;
        let pos = rect.right_top() + vec2(-TEXT_RIGHT - USAGE_SWITCH_PULL_RIGHT, TEXT_TOP + 70.0);
        let switch_size = ui.spacing().interact_size.y * egui::vec2(2.0, 1.0);
        {
            // ---- read ----
            let id: Id = (self.db_relay.url.to_string() + "read_switch").into();
            let spos = pos - vec2(0.0, USAGE_SWITCH_Y_OFFSET);
            let response = components::switch_custom_at(
                ui,
                true,
                &mut self.usage.read,
                switch_size,
                spos,
                id,
                knob_fill,
                on_fill,
                off_fill,
            );
            if response.changed() {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::AdjustRelayUsageBit(
                        self.db_relay.url.clone(),
                        DbRelay::READ,
                        self.usage.read,
                    ));
                if self.usage.read == false {
                    // if read was turned off, inbox must also be turned off
                    self.usage.inbox = false;
                    let _ = GLOBALS
                        .to_overlord
                        .send(ToOverlordMessage::AdjustRelayUsageBit(
                            self.db_relay.url.clone(),
                            DbRelay::INBOX,
                            self.usage.inbox,
                        ));
                }
            }
            response.on_hover_text(READ_HOVER_TEXT);
            draw_text_at(
                ui,
                pos + vec2(ui.spacing().item_spacing.x + switch_size.x, 0.0),
                "Read".into(),
                Align::LEFT,
                Some(ui.visuals().text_color()),
                None,
            );
        }
        {
            // ---- connecting line ----
            let start = pos + vec2(USAGE_SWITCH_X_SPACING + USAGE_LINE_X_START, USAGE_LINE_Y_OFFSET);
            let end = pos + vec2(USAGE_SWITCH_X_SPACING + USAGE_LINE_X_END, USAGE_LINE_Y_OFFSET);
            let painter = ui.painter();
            painter.hline(start.x..=end.x, end.y, Stroke::new(USAGE_LINE_THICKNESS, ui.visuals().panel_fill));
        }
        {
            // ---- inbox ----
            let pos = pos + vec2(USAGE_SWITCH_X_SPACING, 0.0);
            let id: Id = (self.db_relay.url.to_string() + "inbox_switch").into();
            let spos = pos - vec2(0.0, USAGE_SWITCH_Y_OFFSET);
            let response = components::switch_custom_at(
                ui,
                self.usage.read,
                &mut self.usage.inbox,
                switch_size,
                spos,
                id,
                knob_fill,
                on_fill,
                off_fill,
            );
            if response.changed() {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::AdjustRelayUsageBit(
                        self.db_relay.url.clone(),
                        DbRelay::INBOX,
                        self.usage.inbox,
                    ));
            }
            response.on_hover_text(INBOX_HOVER_TEXT);
            draw_text_at(
                ui,
                pos + vec2(ui.spacing().item_spacing.x + switch_size.x, 0.0),
                "Inbox".into(),
                Align::LEFT,
                Some(ui.visuals().text_color()),
                None,
            );
        }
        let pos = pos + vec2(0.0, USAGE_SWITCH_Y_SPACING);
        {
            // ---- write ----
            let id: Id = (self.db_relay.url.to_string() + "write_switch").into();
            let spos = pos - vec2(0.0, USAGE_SWITCH_Y_OFFSET);
            let response = components::switch_custom_at(
                ui,
                true,
                &mut self.usage.write,
                switch_size,
                spos,
                id,
                knob_fill,
                on_fill,
                off_fill,
            );
            if response.changed() {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::AdjustRelayUsageBit(
                        self.db_relay.url.clone(),
                        DbRelay::WRITE,
                        self.usage.write,
                    ));

                if self.usage.write == false {
                    // if write was turned off, outbox must also be turned off
                    self.usage.outbox = false;
                    let _ = GLOBALS
                        .to_overlord
                        .send(ToOverlordMessage::AdjustRelayUsageBit(
                            self.db_relay.url.clone(),
                            DbRelay::OUTBOX,
                            self.usage.outbox,
                        ));
                }
            }
            response.on_hover_text(WRITE_HOVER_TEXT);
            draw_text_at(
                ui,
                pos + vec2(ui.spacing().item_spacing.x + switch_size.x, 0.0),
                "Write".into(),
                Align::LEFT,
                Some(ui.visuals().text_color()),
                None,
            );
        }
        {
            // ---- connecting line ----
            let start = pos + vec2(USAGE_SWITCH_X_SPACING + USAGE_LINE_X_START, USAGE_LINE_Y_OFFSET);
            let end = pos + vec2(USAGE_SWITCH_X_SPACING + USAGE_LINE_X_END, USAGE_LINE_Y_OFFSET);
            let painter = ui.painter();
            painter.hline(start.x..=end.x, end.y, Stroke::new(USAGE_LINE_THICKNESS, ui.visuals().panel_fill));
        }
        {
            // ---- outbox ----
            let pos = pos + vec2(USAGE_SWITCH_X_SPACING, 0.0);
            let id: Id = (self.db_relay.url.to_string() + "outbox_switch").into();
            let spos = pos - vec2(0.0, USAGE_SWITCH_Y_OFFSET);
            let response = components::switch_custom_at(
                ui,
                self.usage.write,
                &mut self.usage.outbox,
                switch_size,
                spos,
                id,
                knob_fill,
                on_fill,
                off_fill,
            );
            if response.changed() {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::AdjustRelayUsageBit(
                        self.db_relay.url.clone(),
                        DbRelay::OUTBOX,
                        self.usage.outbox,
                    ));
            }
            response.on_hover_text(OUTBOX_HOVER_TEXT);
            draw_text_at(
                ui,
                pos + vec2(ui.spacing().item_spacing.x + switch_size.x, 0.0),
                "Outbox".into(),
                Align::LEFT,
                Some(ui.visuals().text_color()),
                None,
            );
        }
        let pos = pos + vec2(0.0, USAGE_SWITCH_Y_SPACING);
        {
            // ---- discover ----
            let id: Id = (self.db_relay.url.to_string() + "discover_switch").into();
            let spos = pos - vec2(0.0, USAGE_SWITCH_Y_OFFSET);
            let response = components::switch_custom_at(
                ui,
                true,
                &mut self.usage.discover,
                switch_size,
                spos,
                id,
                knob_fill,
                on_fill,
                off_fill,
            );
            if response.changed() {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::AdjustRelayUsageBit(
                        self.db_relay.url.clone(),
                        DbRelay::DISCOVER,
                        self.usage.discover,
                    ));
            }
            response.on_hover_text(DISCOVER_HOVER_TEXT);
            draw_text_at(
                ui,
                pos + vec2(ui.spacing().item_spacing.x + switch_size.x, 0.0),
                "Discover".into(),
                Align::LEFT,
                Some(ui.visuals().text_color()),
                None,
            );
        }
        let pos = pos + vec2(0.0, USAGE_SWITCH_Y_SPACING);
        {
            // ---- advertise ----
            let id: Id = (self.db_relay.url.to_string() + "advertise_switch").into();
            let spos = pos - vec2(0.0, USAGE_SWITCH_Y_OFFSET);
            let response = components::switch_custom_at(
                ui,
                true,
                &mut self.usage.advertise,
                switch_size,
                spos,
                id,
                knob_fill,
                on_fill,
                off_fill,
            );
            if response.changed() {
                let _ = GLOBALS
                    .to_overlord
                    .send(ToOverlordMessage::AdjustRelayUsageBit(
                        self.db_relay.url.clone(),
                        DbRelay::ADVERTISE,
                        self.usage.advertise,
                    ));
            }
            response.on_hover_text(ADVERTISE_HOVER_TEXT);
            draw_text_at(
                ui,
                pos + vec2(ui.spacing().item_spacing.x + switch_size.x, 0.0),
                "Advertise".into(),
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
            self.paint_stats(ui, &rect, false);
            paint_hline(ui, &rect, HLINE_1_Y_OFFSET);
            self.paint_nip11(ui, &rect);
            self.paint_usage_settings(ui, &rect);
            paint_hline(ui, &rect, HLINE_2_Y_OFFSET);
            self.paint_lower_buttons(ui, &rect);
            response |= self.paint_close_btn(ui, &rect);
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
        Stroke::new(HLINE_THICKNESS, ui.visuals().panel_fill),
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
        Rect::from_x_y_ranges( pos.x - size.x ..= pos.x, pos.y ..= pos.y + size.y)
    };
    if let Some(stroke) = underline {
        let stroke = Stroke::new( stroke.width, stroke.color.gamma_multiply(0.6));
        let line_height = ui.fonts(|f| {
            f.row_height(&FontId::default())
        });
        let painter = ui.painter();
        painter.hline(
           rect.min.x ..= rect.max.x,
           rect.min.y + line_height - 2.0,
           stroke);
    }
    rect
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

fn draw_link_at(
    ui: &mut Ui,
    id: Id,
    pos: Pos2,
    text: WidgetText,
    align: Align,
    active: bool,
    secondary: bool,
    hover_color: Color32,
) -> Response {
    let (galley, response) = if align == Align::Min {
        allocate_text_at(ui, pos, text.into(), id)
    } else {
        allocate_text_right_align_at(ui, pos, text.into(), id)
    };
    let (color, stroke) = if !secondary {
        if active {
            if response.hovered() {
                (ui.visuals().text_color(), Stroke::NONE)
            } else {
                (hover_color, Stroke::new(1.0, hover_color))
            }
        } else {
            (ui.visuals().weak_text_color(), Stroke::NONE)
        }
    } else {
        if active {
            if response.hovered() {
                (hover_color, Stroke::NONE)
            } else {
                (ui.visuals().text_color(), Stroke::new(1.0, ui.visuals().text_color()))
            }
        } else {
            (ui.visuals().weak_text_color(), Stroke::NONE)
        }
    };
    response.clone().on_hover_cursor(CursorIcon::PointingHand);
    draw_text_galley_at(ui, pos, galley, Some(color), Some(stroke));
    response
}

/// UTF-8 safe truncate (String::truncate() can panic)
#[inline]
fn safe_truncate(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        None => s,
        Some((idx, _)) => &s[..idx],
    }
}
