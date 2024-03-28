use std::{cell::RefCell, rc::Rc};

use chrono::{DateTime, Local, Utc};
use eframe::egui::{self, vec2, Color32, RichText, Style, Ui, Vec2};
use gossip_lib::{PendingItem, GLOBALS};

use self::{
    auth_request::AuthRequest, conn_request::ConnRequest, nip46_request::Nip46Request,
    pending::Pending,
};

use super::{
    theme::{DefaultTheme, ThemeDef},
    widgets, GossipUi, Page, Theme,
};
mod auth_request;
mod conn_request;
mod nip46_request;
mod pending;

pub trait Notification {
    fn timestamp(&self) -> u64;
    fn title(&self) -> RichText;
    fn summary(&self) -> String;
    fn show(&mut self, theme: &Theme, ui: &mut Ui) -> Option<Page>;
}

type NotificationHandle = Rc<RefCell<dyn Notification>>;

pub struct NotificationData {
    active: Vec<NotificationHandle>,
    last_pending_hash: u64,
    num_pending: usize,
}

impl NotificationData {
    pub fn new() -> Self {
        Self {
            active: Vec::new(),
            last_pending_hash: 0,
            num_pending: 0,
        }
    }
}

const ALIGN: egui::Align = egui::Align::Center;
const HEIGHT: f32 = 23.0;
const TRUNC: f32 = 340.0;
const SWITCH_SIZE: Vec2 = Vec2 { x: 46.0, y: 23.0 };

///
/// Calc notifications
///
pub(super) fn calc(app: &mut GossipUi) {
    let hash = GLOBALS.pending.hash();
    // recalc if hash changed
    if app.notification_data.last_pending_hash != hash {
        app.notification_data.active.clear();

        for (item, time) in GLOBALS.pending.read().iter() {
            match item {
                PendingItem::RelayConnectionRequest(url, jobs) => app
                    .notification_data
                    .active
                    .push(ConnRequest::new(url.clone(), jobs.clone(), *time)),
                PendingItem::RelayAuthenticationRequest(pubkey, url) => app
                    .notification_data
                    .active
                    .push(AuthRequest::new(pubkey.clone(), url.clone(), *time)),
                PendingItem::Nip46Request(name, account, command) => {
                    app.notification_data.active.push(Nip46Request::new(
                        name.clone(),
                        account.clone(),
                        command.clone(),
                        *time,
                    ))
                }
                item => app
                    .notification_data
                    .active
                    .push(Pending::new(item.clone(), *time)),
            }
        }

        app.notification_data.num_pending = app.notification_data.active.len();
        app.notification_data.last_pending_hash = hash;
    }
}

///
/// Draw the notification icons
///
// pub(super) fn draw_icons(app: &mut GossipUi, ui: &mut Ui) {}

///
/// Show the Notifications page view
///
pub(super) fn update(app: &mut GossipUi, ui: &mut Ui) {
    widgets::page_header(ui, "Notifications", |_| {});

    let mut new_page = None;
    app.vert_scroll_area().show(ui, |ui| {
        for entry in &app.notification_data.active {
            widgets::list_entry::make_frame(ui, None).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.set_height(37.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(unixtime_to_string(entry.borrow().timestamp()))
                            .weak()
                            .small(),
                    );
                    ui.add_space(10.0);
                    ui.label(entry.borrow().title().small());
                });
                new_page = entry.borrow_mut().show(&app.theme, ui);
            });
            if new_page.is_some() {
                break;
            }
        }
    });
    if let Some(page) = new_page {
        app.set_page(ui.ctx(), page);
    }
}

fn unixtime_to_string(timestamp: u64) -> String {
    let time: DateTime<Utc> =
        DateTime::from_timestamp(timestamp.try_into().unwrap_or_default(), 0).unwrap_or_default();
    let local: DateTime<Local> = time.into();

    local.format("%e. %b %Y %T").to_string()
}

fn manage_style(theme: &Theme, style: &mut Style) {
    let (bg_color, text_color, frame_color) = if theme.dark_mode {
        (
            Color32::from_gray(0x0A),
            Color32::from_gray(0xD4),
            Color32::from_gray(0x73),
        )
    } else {
        (
            Color32::from_gray(0xF5),
            Color32::from_gray(0x26),
            Color32::from_gray(0xA3),
        )
    };
    style.spacing.button_padding = vec2(16.0, 4.0);
    style.visuals.widgets.noninteractive.weak_bg_fill = bg_color;
    style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, frame_color);
    style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, text_color);
    style.visuals.widgets.inactive.weak_bg_fill = bg_color;
    style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, frame_color);
    style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, text_color);
    style.visuals.widgets.hovered.weak_bg_fill =
        <DefaultTheme as ThemeDef>::darken_color(bg_color, 0.05);
    style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, text_color);
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(
        1.0,
        <DefaultTheme as ThemeDef>::darken_color(frame_color, 0.2),
    );
    style.visuals.widgets.active.weak_bg_fill =
        <DefaultTheme as ThemeDef>::darken_color(bg_color, 0.4);
    style.visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, text_color);
    style.visuals.widgets.active.bg_stroke = egui::Stroke::new(
        1.0,
        <DefaultTheme as ThemeDef>::darken_color(frame_color, 0.4),
    );
}

fn decline_style(theme: &Theme, style: &mut Style) {
    let (bg_color, text_color) = if theme.dark_mode {
        (Color32::WHITE, Color32::from_gray(0x26))
    } else {
        (Color32::from_gray(0x26), Color32::WHITE)
    };
    style.spacing.button_padding = vec2(16.0, 4.0);
    style.visuals.widgets.noninteractive.weak_bg_fill = bg_color;
    style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, text_color);
    style.visuals.widgets.inactive.weak_bg_fill = bg_color;
    style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, text_color);
    style.visuals.widgets.hovered.weak_bg_fill =
        <DefaultTheme as ThemeDef>::darken_color(bg_color, 0.2);
    style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, text_color);
    style.visuals.widgets.hovered.bg_stroke =
        egui::Stroke::new(1.0, <DefaultTheme as ThemeDef>::darken_color(bg_color, 0.2));
    style.visuals.widgets.active.weak_bg_fill =
        <DefaultTheme as ThemeDef>::darken_color(bg_color, 0.4);
    style.visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, text_color);
    style.visuals.widgets.active.bg_stroke =
        egui::Stroke::new(1.0, <DefaultTheme as ThemeDef>::darken_color(bg_color, 0.4));
}

fn approve_style(theme: &Theme, style: &mut Style) {
    theme.accent_button_1_style(style);
    style.spacing.button_padding = vec2(16.0, 4.0);
}
