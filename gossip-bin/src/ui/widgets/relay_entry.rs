//#![allow(dead_code)]
use eframe::egui::{self, *};
use nostr_types::{PublicKeyHex, RelayUrl, Unixtime};
use std::fmt;

use crate::ui::{widgets, GossipUi};
use gossip_lib::{comms::ToOverlordMessage, Relay, GLOBALS};

use super::{
    list_entry::{
        self, allocate_text_at, draw_link_at, draw_text_at, draw_text_galley_at, paint_hline,
        TEXT_BOTTOM, TEXT_LEFT, TEXT_RIGHT, TEXT_TOP, TITLE_FONT_SIZE,
    },
    CopyButton, COPY_SYMBOL_SIZE,
};

/// Height of the list view (width always max. available)
const LIST_VIEW_HEIGHT: f32 = 60.0;
/// Height of the list view (width always max. available)
const DETAIL_VIEW_HEIGHT: f32 = 80.0;
/// Height of the edit view (width always max. available)
const EDIT_VIEW_HEIGHT: f32 = 280.0;
/// Height required for one auth-permission drop-down
const EDIT_VIEW_AUTH_PERM_HEIGHT: f32 = 25.0;
/// Y-offset for first separator
const HLINE_1_Y_OFFSET: f32 = LIST_VIEW_HEIGHT - 12.0;
/// Y-offset for second separator
const HLINE_2_Y_OFFSET: f32 = 210.0;
/// Y top for the detail section
const DETAIL_SECTION_TOP: f32 = TEXT_TOP + LIST_VIEW_HEIGHT;
/// Size of edit button
const EDIT_BTN_SIZE: f32 = 20.0;
/// Spacing of stats row to heading
const STATS_Y_SPACING: f32 = 1.5 * TITLE_FONT_SIZE;
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
/// Start of permission section from top
const PERMISSION_SECTION_TOP: f32 = 230.0;
const PERMISSION_SECTION_SIZE: Vec2 = Vec2 { x: 223.0, y: 50.0 };
/// Spacing between nip11 text rows
const NIP11_Y_SPACING: f32 = 20.0;
/// Status symbol for status color indicator
const STATUS_SYMBOL: &str = "\u{25CF}";
/// Space reserved for status symbol before title
const STATUS_SYMBOL_SPACE: f32 = 18.0;
/// First stat column x location
const STATS_COL_1_X: f32 = TEXT_LEFT;
/// 2. stat column x offset
const STATS_COL_2_X: f32 = 130.0;
/// 3. stat column x offset
const STATS_COL_3_X: f32 = 120.0;
/// 4. stat column x offset
const STATS_COL_4_X: f32 = 120.0;
/// 5. stat column x offset
const STATS_COL_5_X: f32 = 150.0;

const READ_HOVER_TEXT: &str = "Where you actually read events from (including those tagging you, but also for other purposes).";
const INBOX_HOVER_TEXT: &str = "Where you tell others you read from. You should also check Read. These relays shouldn't require payment. It is recommended to have a few.";
const DISCOVER_HOVER_TEXT: &str = "Where you discover other people's relays lists.";
const WRITE_HOVER_TEXT: &str =
    "Where you actually write your events to. It is recommended to have a few.";
const OUTBOX_HOVER_TEXT: &str = "Where you tell others you write to. You should also check Write. It is recommended to have a few.";
const SPAMSAFE_HOVER_TEXT: &str = "Relay is trusted to filter spam. If not set, replies and mentions from unfollowed people will not be fetched from the relay (when SpamSafe is enabled in settings).";
const DM_USE_HOVER_TEXT: &str = "Use Relay to receive and send Direct Messages";

#[derive(Clone, PartialEq)]
pub enum RelayEntryView {
    List,
    Detail,
    Edit,
}

#[derive(Copy, Clone, PartialEq)]
enum Permission {
    Ask,
    Always,
    Never,
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Permission::Ask => write!(f, "Ask"),
            Permission::Always => write!(f, "Always"),
            Permission::Never => write!(f, "Never"),
        }
    }
}

impl From<Permission> for Option<bool> {
    fn from(value: Permission) -> Option<bool> {
        match value {
            Permission::Ask => None,
            Permission::Always => Some(true),
            Permission::Never => Some(false),
        }
    }
}

impl From<Option<bool>> for Permission {
    fn from(value: Option<bool>) -> Self {
        match value {
            None => Permission::Ask,
            Some(true) => Permission::Always,
            Some(false) => Permission::Never,
        }
    }
}

#[derive(Clone)]
struct UsageBits {
    read: bool,
    write: bool,
    //advertise: bool,
    inbox: bool,
    outbox: bool,
    discover: bool,
    spamsafe: bool,
    dm: bool,
}

impl UsageBits {
    fn from_usage_bits(usage_bits: u64) -> Self {
        Self {
            read: usage_bits & Relay::READ == Relay::READ,
            write: usage_bits & Relay::WRITE == Relay::WRITE,
            //advertise: usage_bits & Relay::ADVERTISE == Relay::ADVERTISE,
            inbox: usage_bits & Relay::INBOX == Relay::INBOX,
            outbox: usage_bits & Relay::OUTBOX == Relay::OUTBOX,
            discover: usage_bits & Relay::DISCOVER == Relay::DISCOVER,
            spamsafe: usage_bits & Relay::SPAMSAFE == Relay::SPAMSAFE,
            dm: usage_bits & Relay::DM == Relay::DM,
        }
    }

    // fn to_usage_bits(&self) -> u64 {
    //     let mut bits: u64 = 0;
    //     if self.read {
    //         bits |= Relay::READ
    //     }
    //     if self.write {
    //         bits |= Relay::WRITE
    //     }
    //     if self.advertise {
    //         bits |= Relay::ADVERTISE
    //     }
    //     if self.inbox {
    //         bits |= Relay::INBOX
    //     }
    //     if self.outbox {
    //         bits |= Relay::OUTBOX
    //     }
    //     if self.discover {
    //         bits |= Relay::DISCOVER
    //     }
    //     bits
    // }
}

/// Relay Entry
///
/// A relay entry has different views, which can be chosen with the
/// show_<view> functions.
///
#[derive(Clone)]
pub struct RelayEntry {
    relay: Relay,
    view: RelayEntryView,
    enabled: bool,
    connected: bool,
    timeout_until: Option<i64>,
    reasons: String,
    user_count: Option<usize>,
    usage: UsageBits,
    accent: Color32,
    accent_hover: Color32,
    bg_fill: Color32,
    // highlight: Option<Color32>,
    option_symbol: TextureId,
    auth_require_permission: bool,
    conn_require_permission: bool,
}

impl RelayEntry {
    pub(in crate::ui) fn new(relay: Relay, app: &mut GossipUi) -> Self {
        let usage = UsageBits::from_usage_bits(relay.get_usage_bits());
        let accent = app.theme.accent_color();
        let mut hsva: ecolor::HsvaGamma = accent.into();
        hsva.v *= 0.8;
        let accent_hover: Color32 = hsva.into();
        Self {
            relay,
            view: RelayEntryView::List,
            enabled: true,
            connected: false,
            timeout_until: None,
            reasons: "".into(),
            user_count: None,
            usage,
            accent,
            accent_hover,
            bg_fill: app.theme.main_content_bgcolor(),
            // highlight: None,
            option_symbol: (&app.assets.options_symbol).into(),
            auth_require_permission: false,
            conn_require_permission: false,
        }
    }

    pub fn set_edit(&mut self, edit: bool) {
        if edit {
            self.view = RelayEntryView::Edit;
        }
    }

    pub fn set_detail(&mut self, detail: bool) {
        match self.view {
            RelayEntryView::List => {
                if detail {
                    self.view = RelayEntryView::Detail;
                }
            }
            RelayEntryView::Detail => {
                if !detail {
                    self.view = RelayEntryView::List;
                }
            }
            _ => {}
        }
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn set_user_count(&mut self, count: usize) {
        self.user_count = Some(count);
    }

    pub fn set_connected(&mut self, connected: bool) {
        self.connected = connected;
    }

    pub fn set_timeout(&mut self, timeout_until: Option<i64>) {
        self.timeout_until = timeout_until;
    }

    pub fn set_reasons(&mut self, reasons: String) {
        self.reasons = reasons;
    }

    pub fn auth_require_permission(&mut self, require_permission: bool) {
        self.auth_require_permission = require_permission;
    }

    pub fn conn_require_permission(&mut self, require_permission: bool) {
        self.conn_require_permission = require_permission;
    }

    // pub fn view(&self) -> RelayEntryView {
    //     self.view.clone()
    // }
}

impl RelayEntry {
    fn paint_title(&self, ui: &mut Ui, rect: &Rect) {
        let title = self.relay.url.as_str().to_owned();
        let text = RichText::new(title).size(list_entry::TITLE_FONT_SIZE);
        let galley = list_entry::text_to_galley_max_width(
            ui,
            text.into(),
            Align::LEFT,
            rect.width() - 200.0,
        );
        let pos = rect.min + vec2(TEXT_LEFT + STATUS_SYMBOL_SPACE, TEXT_TOP);
        let rect = draw_text_galley_at(ui, pos, galley, Some(self.accent), None);
        ui.interact(rect, ui.next_auto_id(), Sense::hover())
            .on_hover_text(self.relay.url.as_str());

        // paint status indicator
        // green - connected
        // gray - disconnected
        // orange - penalty box
        // dark gray - disabled
        let symbol = RichText::new(STATUS_SYMBOL).size(15.0);
        let (color, tooltip) = if self.connected {
            let mut text = "Connected".to_string();
            if let Some(at) = self.relay.last_connected_at {
                let ago = crate::date_ago::date_ago(Unixtime(at as i64));
                text = format!("Connected since {}", ago);
            }
            (egui::Color32::from_rgb(0x63, 0xc8, 0x56), text) // green
        } else {
            if self.relay.rank == 0 {
                // ranke == 0 means disabled
                // egui::Color32::from_rgb(0xed, 0x6a, 0x5e) // red
                (egui::Color32::DARK_GRAY, "Disabled (rank=0)".to_string())
            } else {
                // show remaining time on timeout
                if let Some(timeout) = self.timeout_until {
                    let color = egui::Color32::from_rgb(0xf4, 0xbf, 0x4f); // orange
                    let remain = timeout - Unixtime::now().0;
                    let text = format!("Timeout, retry in {} seconds", remain);
                    (color, text)
                } else {
                    (egui::Color32::GRAY, "Not connected".to_string())
                }
            }
        };
        let pos = pos + vec2(-STATUS_SYMBOL_SPACE, 0.0);
        let rect = draw_text_at(ui, pos, symbol.into(), Align::LEFT, Some(color), None);

        // set tooltip
        ui.interact(rect, ui.next_auto_id(), Sense::hover())
            .on_hover_text(tooltip);
    }

    fn paint_edit_btn(&mut self, ui: &mut Ui, rect: &Rect) -> Response {
        let id = self.make_id("edit_btn");
        let pos = rect.right_top() + vec2(-EDIT_BTN_SIZE - TEXT_RIGHT, TEXT_TOP);
        let btn_rect = Rect::from_min_size(pos, vec2(EDIT_BTN_SIZE, EDIT_BTN_SIZE));
        let response = ui
            .interact(btn_rect, id, Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand)
            .on_hover_text("Configure Relay");
        let color = if response.hovered() {
            ui.visuals().text_color()
        } else {
            self.accent
        };
        let mut mesh = Mesh::with_texture(self.option_symbol);
        mesh.add_rect_with_uv(
            btn_rect.shrink(2.0),
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            color,
        );
        ui.painter().add(Shape::mesh(mesh));
        response
    }

    fn paint_close_btn(&mut self, ui: &mut Ui, rect: &Rect) -> Response {
        let id = self.make_id("close_btn");
        let button_padding = ui.spacing().button_padding;
        let galley = WidgetText::from("Close")
            .color(ui.visuals().extreme_bg_color)
            .into_galley(ui, Some(TextWrapMode::Extend), 0.0, TextStyle::Button);
        let mut desired_size = galley.size() + 4.0 * button_padding;
        desired_size.y = desired_size.y.at_least(ui.spacing().interact_size.y);
        let pos = rect.right_bottom() + vec2(-TEXT_RIGHT, -TEXT_BOTTOM) - desired_size;
        let btn_rect = Rect::from_min_size(pos, desired_size);
        let response = ui
            .interact(btn_rect, id, Sense::click())
            .on_hover_cursor(egui::CursorIcon::PointingHand);
        response.widget_info(|| {
            WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), galley.text())
        });

        let visuals = ui.style().interact(&response);
        {
            let fill = if response.hovered() {
                self.accent_hover
            } else {
                self.accent
            };
            let stroke = Stroke::new(visuals.bg_stroke.width, self.accent);
            let rounding = visuals.rounding;
            ui.painter()
                .rect(btn_rect.expand(visuals.expansion), rounding, fill, stroke);
        }

        let text_pos = ui
            .layout()
            .align_size_within_rect(galley.size(), btn_rect.shrink2(2.0 * button_padding))
            .min;
        ui.painter().galley(text_pos, galley, visuals.text_color());

        if response.clicked() {
            self.view = RelayEntryView::Detail;
        }

        response
    }

    fn paint_lower_buttons(&self, ui: &mut Ui, rect: &Rect) -> Response {
        let line_height = ui.fonts(|f| f.row_height(&FontId::default()));
        let pos = rect.left_bottom() + vec2(TEXT_LEFT, -TEXT_BOTTOM - line_height);
        let is_personal = self.relay.has_any_usage_bit();
        let id = self.make_id("remove_link");
        let text = "Remove from personal list";
        let response = draw_link_at(
            ui,
            id,
            pos,
            text.into(),
            Align::Min,
            self.enabled && is_personal,
            true,
        );
        if response.clicked() {
            modify_relay(&self.relay.url, |relay| {
                relay.clear_usage_bits(
                    Relay::DISCOVER | Relay::INBOX | Relay::OUTBOX | Relay::READ | Relay::WRITE,
                )
            });
        }

        let pos = pos + vec2(200.0, 0.0);
        let id = self.make_id("disconnect_link");
        let text = "Force disconnect";
        let can_disconnect = self.enabled && self.connected;
        let disconnect_response =
            draw_link_at(ui, id, pos, text.into(), Align::Min, can_disconnect, true);
        if can_disconnect && disconnect_response.clicked() {
            let _ = GLOBALS
                .to_overlord
                .send(ToOverlordMessage::DropRelay(self.relay.url.to_owned()));
        }

        let pos = pos + vec2(150.0, 0.0);
        let id = self.make_id("hide_unhide_link");
        let text = if self.relay.hidden {
            "Unhide Relay"
        } else {
            "Hide Relay"
        };
        let response = draw_link_at(ui, id, pos, text.into(), Align::Min, self.enabled, true);
        if response.clicked() {
            let _ = GLOBALS.to_overlord.send(ToOverlordMessage::HideOrShowRelay(
                self.relay.url.to_owned(),
                !self.relay.hidden,
            ));
        }

        // pass the response back so the page knows the edit view should close
        response
    }

    fn paint_stats(&self, ui: &mut Ui, rect: &Rect) {
        {
            // ---- Success Rate ----
            let pos = rect.min + vec2(STATS_COL_1_X, TEXT_TOP + STATS_Y_SPACING);
            let text = RichText::new(format!(
                "Rate: {:.0}% ({})",
                self.relay.success_rate() * 100.0,
                self.relay.success_count
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
            let pos = pos + vec2(STATS_COL_2_X, 0.0);
            // let mut active = self.enabled;
            let text = if let Some(count) = self.user_count {
                RichText::new(format!("Following: {}", count))
            } else {
                // active = false;
                RichText::new("Following: ---")
            };
            // let id = self.make_id("following_link");
            // let response = draw_link_at(ui, id, pos, text.into(), Align::Min, active, true);
            // if response.clicked() {
            //     // TODO go to following page for this relay?
            // }
            draw_text_at(
                ui,
                pos,
                text.into(),
                Align::LEFT,
                Some(ui.visuals().text_color()),
                None,
            );

            // ---- Last event ----
            let pos = pos + vec2(STATS_COL_3_X, 0.0);
            let mut ago = "".to_string();
            if let Some(at) = self.relay.last_general_eose_at {
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
            let pos = pos + vec2(STATS_COL_4_X, 0.0);
            let mut ago = "".to_string();
            if let Some(at) = self.relay.last_connected_at {
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

            // ---- Rank ----
            let pos = pos + vec2(STATS_COL_5_X, 0.0);
            let text = RichText::new(format!("Usage Rank: {}", self.relay.rank));
            draw_text_at(
                ui,
                pos,
                text.into(),
                Align::LEFT,
                Some(ui.visuals().text_color()),
                None,
            );
        }
    }

    fn paint_reasons(&self, ui: &mut Ui, rect: &Rect) {
        const RIGHT: f32 = -17.0;
        const SPACE: f32 = 23.0;

        // match self.view { RelayEntryView::Detail => ... }
        let right = pos2(rect.max.x, rect.min.y)
            + vec2(-TEXT_RIGHT - EDIT_BTN_SIZE - SPACE, TEXT_TOP + 4.0);

        let pos = right + vec2(RIGHT - 7.0 * SPACE, 0.0);
        draw_text_at(
            ui,
            pos,
            self.reasons.clone().into(),
            Align::RIGHT,
            Some(ui.visuals().text_color()),
            None,
        );
    }

    fn paint_low_quality(&self, ui: &mut Ui, rect: &Rect) {
        let pos = pos2(rect.max.x - 99.0, rect.min.y + 23.0);
        let (galley, response) = allocate_text_at(
            ui,
            pos,
            "low quality".into(),
            Align::Center,
            self.make_id("lq"),
        );
        draw_text_galley_at(ui, pos, galley, Some(egui::Color32::GRAY), None);
        response.on_hover_text("The relay is not configured and either has low usage, poor success, or you have disabled it.");
    }

    fn paint_usage(&self, ui: &mut Ui, rect: &Rect) {
        const RIGHT: f32 = -17.0;
        const SPACE: f32 = 23.0;

        // match self.view { RelayEntryView::Detail => ... }
        let right = pos2(rect.max.x, rect.min.y)
            + vec2(-TEXT_RIGHT - EDIT_BTN_SIZE - SPACE, TEXT_TOP + 4.0);

        let align = Align::Center;

        let bg_rect = egui::Rect::from_x_y_ranges(
            right.x - 170.0..=right.x + 3.0,
            right.y - 5.0..=right.y + 18.0,
        );
        let bg_radius = bg_rect.height() / 2.0;
        ui.painter().rect_filled(
            bg_rect,
            egui::Rounding::same(bg_radius),
            ui.visuals().code_bg_color,
        );

        fn switch(ui: &mut Ui, str: &str, on: bool) -> (RichText, Color32) {
            let active = ui.visuals().text_color();
            let inactive = ui.visuals().text_color().gamma_multiply(0.4);
            if on {
                (RichText::new(str), active)
            } else {
                (RichText::new(str), inactive)
            }
        }

        // ---- Read ----
        let pos = right + vec2(RIGHT - 6.0 * SPACE, 0.0);
        let (text, color) = switch(ui, "R", self.usage.read);
        let (galley, response) = allocate_text_at(ui, pos, text.into(), align, self.make_id("R"));
        draw_text_galley_at(ui, pos, galley, Some(color), None);
        response.on_hover_text(READ_HOVER_TEXT);

        // ---- Inbox ----
        let pos = right + vec2(RIGHT - 5.0 * SPACE, 0.0);
        let (text, color) = switch(ui, "I", self.usage.inbox);
        let (galley, response) = allocate_text_at(ui, pos, text.into(), align, self.make_id("I"));
        draw_text_galley_at(ui, pos, galley, Some(color), None);
        response.on_hover_text(INBOX_HOVER_TEXT);

        // ---- + ----
        let pos = pos - vec2(SPACE / 2.0, 0.0);
        draw_text_at(ui, pos, "+".into(), align, Some(color), None);

        // ---- Write ----
        let pos = right + vec2(RIGHT - 4.0 * SPACE, 0.0);
        let (text, color) = switch(ui, "W", self.usage.write);
        let (galley, response) = allocate_text_at(ui, pos, text.into(), align, self.make_id("W"));
        draw_text_galley_at(ui, pos, galley, Some(color), None);
        response.on_hover_text(WRITE_HOVER_TEXT);

        // ---- Outbox ----
        let pos = right + vec2(RIGHT - 3.0 * SPACE, 0.0);
        let (text, color) = switch(ui, "O", self.usage.outbox);
        let (galley, response) = allocate_text_at(ui, pos, text.into(), align, self.make_id("O"));
        draw_text_galley_at(ui, pos, galley, Some(color), None);
        response.on_hover_text(OUTBOX_HOVER_TEXT);

        // ---- + ----
        let pos = pos - vec2(SPACE / 2.0, 0.0);
        draw_text_at(ui, pos, "+".into(), align, Some(color), None);

        // ---- Discover ----
        let pos = right + vec2(RIGHT - 2.0 * SPACE, 0.0);
        let (text, color) = switch(ui, "D", self.usage.discover);
        let (galley, response) = allocate_text_at(ui, pos, text.into(), align, self.make_id("D"));
        draw_text_galley_at(ui, pos, galley, Some(color), None);
        response.on_hover_text(DISCOVER_HOVER_TEXT);

        // ---- Spamsafe ----
        let pos = right + vec2(RIGHT - 1.0 * SPACE, 0.0);
        let (text, color) = switch(ui, "S", self.usage.spamsafe);
        let (galley, response) = allocate_text_at(ui, pos, text.into(), align, self.make_id("S"));
        draw_text_galley_at(ui, pos, galley, Some(color), None);
        response.on_hover_text(SPAMSAFE_HOVER_TEXT);

        // ---- DM ----
        let pos = right + vec2(RIGHT - 0.0 * SPACE, 0.0);
        let (text, color) = switch(ui, "DM", self.usage.dm);
        let (galley, response) = allocate_text_at(ui, pos, text.into(), align, self.make_id("DM"));
        draw_text_galley_at(ui, pos, galley, Some(color), None);
        response.on_hover_text(DM_USE_HOVER_TEXT);
    }

    fn paint_nip11(&self, ui: &mut Ui, rect: &Rect) {
        let align = egui::Align::LEFT;
        let max_width = rect.width() - TEXT_RIGHT - TEXT_LEFT - USAGE_SWITCH_PULL_RIGHT - 30.0;
        let pos = rect.left_top() + vec2(TEXT_LEFT, DETAIL_SECTION_TOP);
        if let Some(doc) = &self.relay.nip11 {
            if let Some(contact) = &doc.contact {
                let rect = draw_text_at(ui, pos, contact.into(), align, None, None);
                let id = self.make_id("copy_nip11_contact");
                let pos = pos + vec2(rect.width() + ui.spacing().item_spacing.x, 0.0);
                let response = ui.interact(
                    Rect::from_min_size(pos, COPY_SYMBOL_SIZE),
                    id,
                    Sense::click(),
                );
                if response.clicked() {
                    ui.output_mut(|o| {
                        o.copied_text = contact.to_string();
                        GLOBALS
                            .status_queue
                            .write()
                            .write("copied to clipboard".to_owned());
                    });
                }
                response.on_hover_cursor(egui::CursorIcon::PointingHand);
                CopyButton::new().paint(ui, pos);
            }
            let pos = pos + vec2(0.0, NIP11_Y_SPACING);
            if let Some(desc) = &doc.description {
                let galley =
                    list_entry::text_to_galley_max_width(ui, desc.into(), align, max_width);
                let rect = draw_text_galley_at(ui, pos, galley, None, None);
                ui.interact(rect, self.make_id("nip11desc"), Sense::hover())
                    .on_hover_ui(|ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.set_max_width(400.0);
                            ui.label(desc);
                        });
                    });
            }
            let pos = pos + vec2(0.0, NIP11_Y_SPACING);
            if let Some(pubkey) = &doc.pubkey {
                if let Ok(pubhex) = PublicKeyHex::try_from_str(pubkey.as_str()) {
                    let npub = pubhex.as_bech32_string();
                    let galley = list_entry::text_to_galley_max_width(
                        ui,
                        npub.clone().into(),
                        align,
                        max_width - COPY_SYMBOL_SIZE.x,
                    );
                    let rect = draw_text_galley_at(ui, pos, galley, None, None);
                    let id = self.make_id("copy_nip11_npub");
                    let pos = pos + vec2(rect.width() + ui.spacing().item_spacing.x, 0.0);
                    let response = ui.interact(
                        Rect::from_min_size(pos, COPY_SYMBOL_SIZE),
                        id,
                        Sense::click(),
                    );
                    if response.clicked() {
                        ui.output_mut(|o| {
                            o.copied_text = npub;
                            GLOBALS
                                .status_queue
                                .write()
                                .write("copied to clipboard".into());
                        });
                    }
                    response.on_hover_cursor(egui::CursorIcon::PointingHand);
                    CopyButton::new().paint(ui, pos);
                }
            }
            let pos = pos + vec2(0.0, NIP11_Y_SPACING);
            if !doc.supported_nips.is_empty() {
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
        let knob_fill = Some(ui.visuals().extreme_bg_color);
        let on_fill = Some(self.accent);
        let off_fill_color = ui.visuals().widgets.inactive.bg_fill;
        let off_fill = Some(off_fill_color);
        let pos =
            rect.right_top() + vec2(-TEXT_RIGHT - USAGE_SWITCH_PULL_RIGHT, DETAIL_SECTION_TOP);
        let switch_size = ui.spacing().interact_size.y * egui::vec2(2.0, 1.0);
        {
            // ---- read ----
            let id = self.make_id("read_switch");
            let sw_rect = Rect::from_min_size(pos - vec2(0.0, USAGE_SWITCH_Y_OFFSET), switch_size);
            let response = widgets::switch_custom_at(
                ui,
                true,
                &mut self.usage.read,
                sw_rect,
                id,
                knob_fill,
                on_fill,
                off_fill,
            );
            if response.changed() {
                if !self.usage.read {
                    // if read was turned off, inbox must also be turned off
                    self.usage.inbox = false;
                    modify_relay(&self.relay.url, |relay| {
                        relay.adjust_usage_bit(Relay::READ, self.usage.read);
                        relay.adjust_usage_bit(Relay::INBOX, self.usage.inbox);
                    });
                } else {
                    modify_relay(&self.relay.url, |relay| {
                        relay.adjust_usage_bit(Relay::READ, self.usage.read);
                    });
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
            let start = pos
                + vec2(
                    USAGE_SWITCH_X_SPACING + USAGE_LINE_X_START,
                    USAGE_LINE_Y_OFFSET,
                );
            let end = pos
                + vec2(
                    USAGE_SWITCH_X_SPACING + USAGE_LINE_X_END,
                    USAGE_LINE_Y_OFFSET,
                );
            let painter = ui.painter();
            painter.hline(
                start.x..=end.x,
                end.y,
                Stroke::new(USAGE_LINE_THICKNESS, ui.visuals().panel_fill),
            );
        }
        {
            // ---- inbox ----
            let pos = pos + vec2(USAGE_SWITCH_X_SPACING, 0.0);
            let id = self.make_id("inbox_switch");
            let sw_rect = Rect::from_min_size(pos - vec2(0.0, USAGE_SWITCH_Y_OFFSET), switch_size);
            let response = widgets::switch_custom_at(
                ui,
                self.usage.read,
                &mut self.usage.inbox,
                sw_rect,
                id,
                knob_fill,
                on_fill,
                off_fill,
            );
            if response.changed() {
                modify_relay(&self.relay.url, |relay| {
                    relay.adjust_usage_bit(Relay::INBOX, self.usage.inbox)
                });
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
            let id = self.make_id("write_switch");
            let sw_rect = Rect::from_min_size(pos - vec2(0.0, USAGE_SWITCH_Y_OFFSET), switch_size);
            let response = widgets::switch_custom_at(
                ui,
                true,
                &mut self.usage.write,
                sw_rect,
                id,
                knob_fill,
                on_fill,
                off_fill,
            );
            if response.changed() {
                if !self.usage.write {
                    // if write was turned off, outbox must also be turned off
                    self.usage.outbox = false;
                    modify_relay(&self.relay.url, |relay| {
                        relay.adjust_usage_bit(Relay::WRITE, self.usage.write);
                        relay.adjust_usage_bit(Relay::OUTBOX, self.usage.outbox);
                    });
                } else {
                    modify_relay(&self.relay.url, |relay| {
                        relay.adjust_usage_bit(Relay::WRITE, self.usage.write);
                    });
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
            let start = pos
                + vec2(
                    USAGE_SWITCH_X_SPACING + USAGE_LINE_X_START,
                    USAGE_LINE_Y_OFFSET,
                );
            let end = pos
                + vec2(
                    USAGE_SWITCH_X_SPACING + USAGE_LINE_X_END,
                    USAGE_LINE_Y_OFFSET,
                );
            let painter = ui.painter();
            painter.hline(
                start.x..=end.x,
                end.y,
                Stroke::new(USAGE_LINE_THICKNESS, ui.visuals().panel_fill),
            );
        }
        {
            // ---- outbox ----
            let pos = pos + vec2(USAGE_SWITCH_X_SPACING, 0.0);
            let id = self.make_id("outbox_switch");
            let sw_rect = Rect::from_min_size(pos - vec2(0.0, USAGE_SWITCH_Y_OFFSET), switch_size);
            let response = widgets::switch_custom_at(
                ui,
                self.usage.write,
                &mut self.usage.outbox,
                sw_rect,
                id,
                knob_fill,
                on_fill,
                off_fill,
            );
            if response.changed() {
                modify_relay(&self.relay.url, |relay| {
                    relay.adjust_usage_bit(Relay::OUTBOX, self.usage.outbox)
                });
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
            let id = self.make_id("discover_switch");
            let sw_rect = Rect::from_min_size(pos - vec2(0.0, USAGE_SWITCH_Y_OFFSET), switch_size);
            let response = widgets::switch_custom_at(
                ui,
                true,
                &mut self.usage.discover,
                sw_rect,
                id,
                knob_fill,
                on_fill,
                off_fill,
            );
            if response.changed() {
                modify_relay(&self.relay.url, |relay| {
                    relay.adjust_usage_bit(Relay::DISCOVER, self.usage.discover)
                });
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
        /*
        {
            // ---- advertise ----
            let pos = pos + vec2(USAGE_SWITCH_X_SPACING, 0.0);
            let id = self.make_id("advertise_switch");
            let sw_rect = Rect::from_min_size(pos - vec2(0.0, USAGE_SWITCH_Y_OFFSET), switch_size);
            let response = widgets::switch_custom_at(
                ui,
                true,
                &mut self.usage.advertise,
                sw_rect,
                id,
                knob_fill,
                on_fill,
                off_fill,
            );
            if response.changed() {
                modify_relay(
                    &self.relay.url,
                    |relay| relay.adjust_usage_bit(Relay::ADVERTISE, self.usage.advertise),
                );
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
         */
        {
            // ---- spamsafe ----
            let pos = pos + vec2(USAGE_SWITCH_X_SPACING, 0.0);
            let id = self.make_id("spamsafe_switch");
            let sw_rect = Rect::from_min_size(pos - vec2(0.0, USAGE_SWITCH_Y_OFFSET), switch_size);
            let response = widgets::switch_custom_at(
                ui,
                true,
                &mut self.usage.spamsafe,
                sw_rect,
                id,
                knob_fill,
                on_fill,
                off_fill,
            );
            if response.changed() {
                modify_relay(&self.relay.url, |relay| {
                    relay.adjust_usage_bit(Relay::SPAMSAFE, self.usage.spamsafe)
                });
            }
            response.on_hover_text(SPAMSAFE_HOVER_TEXT);
            draw_text_at(
                ui,
                pos + vec2(ui.spacing().item_spacing.x + switch_size.x, 0.0),
                "Spam safe".into(),
                Align::LEFT,
                Some(ui.visuals().text_color()),
                None,
            );
        }
        let pos = pos + vec2(0.0, USAGE_SWITCH_Y_SPACING);
        {
            // ---- DM use ----
            let id = self.make_id("dm_use_switch");
            let sw_rect = Rect::from_min_size(pos - vec2(0.0, USAGE_SWITCH_Y_OFFSET), switch_size);
            let response = widgets::switch_custom_at(
                ui,
                true,
                &mut self.usage.dm,
                sw_rect,
                id,
                knob_fill,
                on_fill,
                off_fill,
            );
            if response.changed() {
                modify_relay(&self.relay.url, |relay| {
                    relay.adjust_usage_bit(Relay::DM, self.usage.dm)
                });
            }
            response.on_hover_text(DM_USE_HOVER_TEXT);
            draw_text_at(
                ui,
                pos + vec2(ui.spacing().item_spacing.x + switch_size.x, 0.0),
                "Direct Message".into(),
                Align::LEFT,
                Some(ui.visuals().text_color()),
                None,
            );
        }
        let pos = pos + vec2(0.0, USAGE_SWITCH_Y_SPACING);
        {
            // ---- rank ----
            let r = self.relay.rank;
            let mut new_r = self.relay.rank;
            let txt_color = ui.visuals().text_color();
            let on_text = ui.visuals().extreme_bg_color;
            let btn_height: f32 = ui.spacing().interact_size.y;
            let btn_round: Rounding = Rounding::same(btn_height / 2.0);
            let font: FontId = Default::default();

            let pos = pos + vec2(USAGE_SWITCH_X_SPACING, 0.0);
            {
                draw_text_at(
                    ui,
                    pos - vec2(5.0, 0.0),
                    "Relay-picker rank:".into(),
                    Align::RIGHT,
                    Some(txt_color),
                    None,
                );
            }

            {
                // -- value display --
                let rect =
                    Rect::from_min_size(pos + vec2(10.0, -4.0), vec2(40.0 + 8.0, btn_height + 4.0));
                ui.painter().rect(
                    rect,
                    btn_round,
                    ui.visuals().extreme_bg_color,
                    Stroke::new(1.0, off_fill_color),
                );
                ui.painter().text(
                    pos + vec2(34.0, 0.0),
                    Align2::CENTER_TOP,
                    format!("{}", r),
                    font.clone(),
                    txt_color,
                );
                {
                    // -- - button --
                    let rect =
                        Rect::from_min_size(pos + vec2(0.0, -2.0), vec2(btn_height, btn_height));
                    let resp = ui
                        .interact(rect, self.make_id("rank_sub"), Sense::click())
                        .on_hover_cursor(CursorIcon::PointingHand);
                    if resp.clicked() {
                        new_r = new_r.saturating_sub(1)
                    }
                    let (fill, txt) = if resp.hovered() {
                        (self.accent_hover, on_text)
                    } else {
                        (self.accent, on_text)
                    };
                    ui.painter().rect(rect, btn_round, fill, Stroke::NONE);
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        "\u{2212}",
                        font.clone(),
                        txt,
                    );
                }
                {
                    // -- + button --
                    let rect =
                        Rect::from_min_size(pos + vec2(48.0, -2.0), vec2(btn_height, btn_height));
                    let resp = ui
                        .interact(rect, self.make_id("rank_add"), Sense::click())
                        .on_hover_cursor(CursorIcon::PointingHand);
                    if resp.clicked() {
                        if new_r < 9 {
                            new_r += 1;
                        }
                    }
                    let (fill, txt) = if resp.hovered() {
                        (self.accent_hover, on_text)
                    } else {
                        (self.accent, on_text)
                    };
                    ui.painter().rect(rect, btn_round, fill, Stroke::NONE);
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        "\u{002B}",
                        font.clone(),
                        txt,
                    );
                }
            }

            if new_r != self.relay.rank {
                let _ = GLOBALS.to_overlord.send(ToOverlordMessage::RankRelay(
                    self.relay.url.clone(),
                    new_r as u8,
                ));
            }
        }
    }

    fn paint_permissions(&self, ui: &mut Ui, rect: &Rect) {
        let pos = rect.right_top()
            + vec2(
                -TEXT_RIGHT - USAGE_SWITCH_PULL_RIGHT,
                PERMISSION_SECTION_TOP,
            );

        let perm_rect = Rect::from_min_size(pos, PERMISSION_SECTION_SIZE);

        ui.allocate_ui_at_rect(perm_rect, |ui| {
            if self.conn_require_permission {
                let mut connect_permission = Permission::from(self.relay.allow_connect);
                let response = permission_combo(ui, &mut connect_permission, "Allow Connect:");
                if response.is_some() && response.unwrap().changed() {
                    modify_relay(&self.relay.url, |relay| {
                        relay.allow_connect = connect_permission.into();
                    });
                }
                ui.add_space(3.0);
            }

            if self.auth_require_permission {
                let mut auth_permission = Permission::from(self.relay.allow_auth);
                let response = permission_combo(ui, &mut auth_permission, "Allow Auth:");
                if response.is_some() && response.unwrap().changed() {
                    modify_relay(&self.relay.url, |relay| {
                        relay.allow_auth = auth_permission.into();
                    });
                }
            }
        });
    }

    fn make_id(&self, str: &str) -> Id {
        (self.relay.url.to_string() + str).into()
    }

    /// Do layout and position the galley in the ui, without painting it or adding widget info.
    fn update_list_view(mut self, ui: &mut Ui) -> Response {
        let (rect, mut response) = list_entry::allocate_space(ui, LIST_VIEW_HEIGHT);

        // all the heavy lifting is only done if it's actually visible
        if ui.is_rect_visible(rect) {
            list_entry::paint_frame(ui, &rect, Some(self.bg_fill));
            self.paint_title(ui, &rect);
            response |= self.paint_edit_btn(ui, &rect);
            if self.relay.has_any_usage_bit() || self.relay.is_good_for_advertise() {
                self.paint_usage(ui, &rect);
            } else {
                self.paint_low_quality(ui, &rect);
            }
            self.paint_reasons(ui, &rect);
        }

        response
    }

    fn update_detail_view(mut self, ui: &mut Ui) -> Response {
        let (rect, mut response) = list_entry::allocate_space(ui, DETAIL_VIEW_HEIGHT);

        // all the heavy lifting is only done if it's actually visible
        if ui.is_rect_visible(rect) {
            list_entry::paint_frame(ui, &rect, Some(self.bg_fill));
            self.paint_title(ui, &rect);
            response |= self.paint_edit_btn(ui, &rect);
            self.paint_stats(ui, &rect);
            if self.relay.has_any_usage_bit() || self.relay.is_good_for_advertise() {
                self.paint_usage(ui, &rect);
            }
            self.paint_reasons(ui, &rect);
        }

        response
    }

    fn update_edit_view(mut self, ui: &mut Ui) -> Response {
        let (height, hline2_offset) =
            match (self.auth_require_permission, self.conn_require_permission) {
                (true, true) => (
                    EDIT_VIEW_HEIGHT + 2.0 * EDIT_VIEW_AUTH_PERM_HEIGHT,
                    HLINE_2_Y_OFFSET + 2.0 * EDIT_VIEW_AUTH_PERM_HEIGHT,
                ),
                (true, false) | (false, true) => (
                    EDIT_VIEW_HEIGHT + EDIT_VIEW_AUTH_PERM_HEIGHT,
                    HLINE_2_Y_OFFSET + EDIT_VIEW_AUTH_PERM_HEIGHT,
                ),
                (false, false) => (EDIT_VIEW_HEIGHT, HLINE_2_Y_OFFSET),
            };

        let size = vec2(ui.available_width(), height);
        let rect = Rect::from_min_size(ui.next_widget_position(), size);

        let mut response = ui.interact(rect, self.make_id("frame"), egui::Sense::hover());

        // all the heavy lifting is only done if it's actually visible
        if ui.is_visible() {
            list_entry::paint_frame(ui, &rect, Some(self.bg_fill));
            self.paint_title(ui, &rect);
            self.paint_stats(ui, &rect);
            paint_hline(ui, &rect, HLINE_1_Y_OFFSET);
            self.paint_nip11(ui, &rect);
            self.paint_usage_settings(ui, &rect);
            self.paint_permissions(ui, &rect);
            paint_hline(ui, &rect, hline2_offset);
            response |= self.paint_lower_buttons(ui, &rect);
            response |= self.paint_close_btn(ui, &rect);
        }

        // the last 'allocate' call will move the cursor, so we need
        // to allocate the rect here after painting other components
        ui.allocate_rect(rect, Sense::hover());

        response
    }
}

impl Widget for RelayEntry {
    fn ui(self, ui: &mut Ui) -> Response {
        ui.visuals_mut().widgets.hovered.fg_stroke.color = self.accent;

        match self.view {
            RelayEntryView::List => self.update_list_view(ui),
            RelayEntryView::Detail => self.update_detail_view(ui),
            RelayEntryView::Edit => self.update_edit_view(ui),
        }
    }
}

fn modify_relay<M>(relay_url: &RelayUrl, mut modify: M)
where
    M: FnMut(&mut Relay),
{
    // Load relay record
    let mut relay = GLOBALS
        .storage
        .read_or_create_relay(relay_url, None)
        .unwrap();
    let old = relay.clone();

    // Run modification
    modify(&mut relay);

    // Save relay via the Overlord, so minions can be updated
    let _ = GLOBALS
        .to_overlord
        .send(ToOverlordMessage::UpdateRelay(old, relay));
}

fn permission_combo(
    ui: &mut Ui,
    permission: &mut Permission,
    title: impl Into<WidgetText>,
) -> Option<Response> {
    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
        let auth_combo = egui::ComboBox::from_id_source(ui.next_auto_id());
        let response = auth_combo
            .width(70.0)
            .selected_text(permission.to_string())
            .show_ui(ui, |ui| {
                ui.selectable_value(permission, Permission::Ask, Permission::Ask.to_string())
                    | ui.selectable_value(
                        permission,
                        Permission::Always,
                        Permission::Always.to_string(),
                    )
                    | ui.selectable_value(
                        permission,
                        Permission::Never,
                        Permission::Never.to_string(),
                    )
            })
            .inner;

        ui.add(egui::Label::new(title));
        response
    })
    .inner
}
