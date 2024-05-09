use std::cell::RefCell;
use std::ops::AddAssign;
use std::rc::Rc;

use chrono::{DateTime, Local, Utc};
use eframe::egui::text::LayoutJob;
use eframe::egui::{self, vec2, Align, Color32, FontSelection, RichText, Sense, Style, Ui, Vec2};
use gossip_lib::{PendingItem, GLOBALS};

use self::auth_request::AuthRequest;
use self::conn_request::ConnRequest;
use self::nip46_request::Nip46Request;
use self::pending::Pending;
use super::theme::{DefaultTheme, ThemeDef};
use super::{widgets, GossipUi, Page, Theme};
mod auth_request;
mod conn_request;
mod nip46_request;
mod pending;

#[derive(PartialEq, Default)]
pub enum NotificationFilter {
    #[default]
    All,
    RelayAuthenticationRequest,
    RelayConnectionRequest,
    Nip46Request,
    PendingItem,
}

impl NotificationFilter {
    fn get_name(&self) -> String {
        match self {
            NotificationFilter::All => "All".to_owned(),
            NotificationFilter::RelayAuthenticationRequest => {
                "Relay Authentication Request".to_owned()
            }
            NotificationFilter::RelayConnectionRequest => "Relay Connection Request".to_owned(),
            NotificationFilter::Nip46Request => "NIP46 Request".to_owned(),
            NotificationFilter::PendingItem => "Pending Items".to_owned(),
        }
    }
}

pub trait Notification<'a> {
    fn timestamp(&self) -> u64;
    fn title(&self) -> RichText;
    fn matches_filter(&self, filter: &NotificationFilter) -> bool;
    fn item(&'a self) -> &'a PendingItem;
    fn get_remember(&self) -> bool;
    fn set_remember(&mut self, value: bool);
    fn show(&mut self, theme: &Theme, ui: &mut Ui) -> Option<Page>;
}

type NotificationHandle = Rc<RefCell<dyn for<'handle> Notification<'handle>>>;

pub struct NotificationData {
    active: Vec<NotificationHandle>,
    last_pending_hash: u64,
    num_notif_relays: usize,
    num_notif_pending: usize,
    filter: NotificationFilter,
}

impl NotificationData {
    pub fn new() -> Self {
        Self {
            active: Vec::new(),
            last_pending_hash: 0,
            num_notif_relays: 0,
            num_notif_pending: 0,
            filter: Default::default(),
        }
    }
}

///
/// Calc notifications
pub(super) fn calc(app: &mut GossipUi) {
    let hash = GLOBALS.pending.hash();
    // recalc if hash changed
    if app.notification_data.last_pending_hash != hash {
        app.notification_data.num_notif_relays = 0;
        app.notification_data.num_notif_pending = 0;

        let mut new_active: Vec<NotificationHandle> = Vec::new();

        for (item, time) in GLOBALS.pending.read().iter() {
            match item {
                PendingItem::RelayConnectionRequest { relay, .. } => {
                    let new_entry = ConnRequest::new(item.clone(), *time);
                    app.notification_data.num_notif_relays.add_assign(1);

                    // find old entry if any and copy setting
                    for entry in app.notification_data.active.iter() {
                        if let Ok(entry) = entry.try_borrow() {
                            match entry.item() {
                                PendingItem::RelayConnectionRequest {
                                    relay: old_relay,
                                    jobs: _,
                                } if old_relay == relay => {
                                    new_entry.borrow_mut().set_remember(entry.get_remember());
                                }
                                _ => {}
                            }
                        }
                    }

                    new_active.push(new_entry);
                }
                PendingItem::RelayAuthenticationRequest { account, relay } => {
                    let new_entry = AuthRequest::new(item.clone(), *time);
                    app.notification_data.num_notif_relays.add_assign(1);

                    // find old entry if any and copy setting
                    for entry in app.notification_data.active.iter() {
                        if let Ok(entry) = entry.try_borrow() {
                            match entry.item() {
                                PendingItem::RelayAuthenticationRequest {
                                    account: old_account,
                                    relay: old_relay,
                                } if old_account == account && old_relay == relay => {
                                    new_entry.borrow_mut().set_remember(entry.get_remember());
                                }
                                _ => {}
                            }
                        }
                    }

                    new_active.push(new_entry);
                }
                PendingItem::Nip46Request {
                    client_name,
                    account,
                    command,
                } => {
                    let new_entry = Nip46Request::new(item.clone(), *time);
                    app.notification_data.num_notif_pending.add_assign(1);

                    // find old entry if any and copy setting
                    for entry in app.notification_data.active.iter() {
                        if let Ok(entry) = entry.try_borrow() {
                            match entry.item() {
                                PendingItem::Nip46Request {
                                    client_name: old_client_name,
                                    account: old_account,
                                    command: old_command,
                                } if old_client_name == client_name
                                    && old_account == account
                                    && old_command.id == command.id =>
                                {
                                    new_entry.borrow_mut().set_remember(entry.get_remember());
                                }
                                _ => {}
                            }
                        }
                    }

                    new_active.push(new_entry);
                }
                item => {
                    new_active.push(Pending::new(item.clone(), *time));
                    app.notification_data.num_notif_pending.add_assign(1);
                }
            }
        }

        app.notification_data.active = new_active;
        app.notification_data.last_pending_hash = hash;
    }
}

///
/// Draw the notification icons
pub(super) fn draw_icons(app: &mut GossipUi, ui: &mut Ui) {
    let (no_notification_bg_color, no_notification_text_color) = if app.theme.dark_mode {
        (app.theme.neutral_800(), app.theme.neutral_400())
    } else {
        (app.theme.neutral_100(), app.theme.neutral_400())
    };

    const SIZE: Vec2 = Vec2 { x: 50.0, y: 25.0 };
    let frame_response = egui::Frame::none()
        .rounding(egui::Rounding::ZERO)
        .outer_margin(egui::Margin {
            left: -20.0,
            right: -20.0,
            top: 10.0,
            bottom: -20.0,
        })
        .inner_margin(egui::Margin {
            left: 20.0,
            right: 20.0,
            top: 7.0,
            bottom: 7.0,
        })
        .fill(if app.theme.dark_mode {
            app.theme.neutral_700()
        } else {
            app.theme.neutral_300()
        })
        .show(ui, |ui| {
            ui.set_height(33.0);
            ui.set_width(ui.available_width());
            egui_extras::StripBuilder::new(ui)
                .size(egui_extras::Size::relative(0.3))
                .size(egui_extras::Size::relative(0.3))
                .size(egui_extras::Size::relative(0.3))
                .cell_layout(egui::Layout::centered_and_justified(
                    egui::Direction::LeftToRight,
                ))
                .horizontal(|mut strip| {
                    strip.cell(|ui| {
                        ui.set_min_size(SIZE);
                        ui.set_max_size(SIZE);
                        let idx = ui.painter().add(egui::Shape::Noop);
                        let mut layout_job = LayoutJob::default();
                        let num_notifications = 0;
                        let (bg_color, num_color) = if num_notifications > 0 {
                            if app.theme.dark_mode {
                                (app.theme.neutral_800(), app.theme.neutral_200())
                            } else {
                                (app.theme.neutral_100(), app.theme.neutral_950())
                            }
                        } else {
                            (no_notification_bg_color, no_notification_text_color)
                        };
                        RichText::new("L").color(app.theme.neutral_400()).append_to(
                            &mut layout_job,
                            ui.style(),
                            FontSelection::Default,
                            Align::LEFT,
                        );
                        RichText::new(format!("{:2}", num_notifications))
                            .color(num_color)
                            .append_to(
                                &mut layout_job,
                                ui.style(),
                                FontSelection::Default,
                                Align::LEFT,
                            );
                        ui.add(
                            egui::Label::new(ui.fonts(|f| f.layout_job(layout_job)))
                                .selectable(false),
                        );
                        ui.painter().set(
                            idx,
                            egui::Shape::rect_filled(
                                ui.min_rect(),
                                ui.min_size().y / 2.0,
                                bg_color,
                            ),
                        );
                    });
                    strip.cell(|ui| {
                        ui.set_min_size(SIZE);
                        ui.set_max_size(SIZE);
                        let idx = ui.painter().add(egui::Shape::Noop);
                        let mut layout_job = LayoutJob::default();
                        let (bg_color, sym_color, num_color) =
                            if app.notification_data.num_notif_relays > 0 {
                                (
                                    app.theme.red_100(),
                                    app.theme.red_500(),
                                    app.theme.neutral_950(),
                                )
                            } else {
                                (
                                    no_notification_bg_color,
                                    no_notification_text_color,
                                    no_notification_text_color,
                                )
                            };
                        RichText::new("R").color(sym_color).append_to(
                            &mut layout_job,
                            ui.style(),
                            FontSelection::Default,
                            Align::LEFT,
                        );
                        RichText::new(format!("{:2}", app.notification_data.num_notif_relays))
                            .color(num_color)
                            .append_to(
                                &mut layout_job,
                                ui.style(),
                                FontSelection::Default,
                                Align::LEFT,
                            );
                        ui.add(
                            egui::Label::new(ui.fonts(|f| f.layout_job(layout_job)))
                                .selectable(false),
                        );
                        ui.painter().set(
                            idx,
                            egui::Shape::rect_filled(
                                ui.min_rect(),
                                ui.min_size().y / 2.0,
                                bg_color,
                            ),
                        );
                    });
                    strip.cell(|ui| {
                        ui.set_min_size(SIZE);
                        ui.set_max_size(SIZE);
                        let idx = ui.painter().add(egui::Shape::Noop);
                        let mut layout_job = LayoutJob::default();
                        let (bg_color, sym_color, num_color) =
                            if app.notification_data.num_notif_pending > 0 {
                                (
                                    app.theme.amber_100(),
                                    app.theme.amber_400(),
                                    app.theme.neutral_950(),
                                )
                            } else {
                                (
                                    no_notification_bg_color,
                                    no_notification_text_color,
                                    no_notification_text_color,
                                )
                            };
                        RichText::new("P").color(sym_color).append_to(
                            &mut layout_job,
                            ui.style(),
                            FontSelection::Default,
                            Align::LEFT,
                        );
                        RichText::new(format!("{:2}", app.notification_data.num_notif_pending))
                            .color(num_color)
                            .append_to(
                                &mut layout_job,
                                ui.style(),
                                FontSelection::Default,
                                Align::LEFT,
                            );
                        ui.add(
                            egui::Label::new(ui.fonts(|f| f.layout_job(layout_job)))
                                .selectable(false),
                        );
                        ui.painter().set(
                            idx,
                            egui::Shape::rect_filled(
                                ui.min_rect(),
                                ui.min_size().y / 2.0,
                                bg_color,
                            ),
                        );
                    });
                });
        })
        .response
        .on_hover_cursor(egui::CursorIcon::PointingHand);

    if frame_response.interact(Sense::click()).clicked() {
        app.set_page(ui.ctx(), Page::Notifications);
    }

    if app.page == Page::Notifications {
        let origin_pos = frame_response.rect.right_center() + vec2(5.0, 15.0);
        let path = egui::epaint::PathShape::convex_polygon(
            [
                origin_pos,
                origin_pos + vec2(15.0, -15.0),
                origin_pos + vec2(15.0, 15.0),
            ]
            .to_vec(),
            ui.visuals().panel_fill,
            egui::Stroke::NONE,
        );

        ui.painter().add(path);
    }
}

///
/// Show the Notifications page view
pub(super) fn update(app: &mut GossipUi, ui: &mut Ui) {
    widgets::page_header(ui, "Notifications", |ui| notification_filter_combo(app, ui));

    let mut new_page = None;
    app.vert_scroll_area().show(ui, |ui| {
        for entry in &app.notification_data.active {
            if !entry.borrow().matches_filter(&app.notification_data.filter) {
                continue;
            }
            widgets::list_entry::make_frame(ui, None).show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.set_height(37.0);

                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(unixtime_to_string(
                            entry.borrow().timestamp().try_into().unwrap_or_default(),
                        ))
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

fn unixtime_to_string(timestamp: i64) -> String {
    let time: DateTime<Utc> = DateTime::from_timestamp(timestamp, 0).unwrap_or_default();
    let local: DateTime<Local> = time.into();

    local.format("%e %b %Y %T").to_string()
}

fn manage_style(theme: &Theme, style: &mut Style) {
    let (bg_color, text_color, frame_color) = if theme.dark_mode {
        (
            theme.neutral_950(),
            theme.neutral_300(),
            theme.neutral_500(),
        )
    } else {
        (
            theme.neutral_100(),
            theme.neutral_800(),
            theme.neutral_400(),
        )
    };
    style.spacing.button_padding = vec2(8.0, 3.0);
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
        (Color32::WHITE, theme.neutral_800())
    } else {
        (theme.neutral_800(), Color32::WHITE)
    };
    style.spacing.button_padding = vec2(8.0, 3.0);
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
    theme.primary_button_style(style);
    style.spacing.button_padding = vec2(8.0, 3.0);
}

pub fn notification_filter_combo(app: &mut GossipUi, ui: &mut Ui) {
    let filter_combo = egui::ComboBox::from_id_source(egui::Id::from("NotificationFilterCombo"));
    filter_combo
        .selected_text(app.notification_data.filter.get_name())
        .width(210.0)
        .show_ui(ui, |ui| {
            ui.selectable_value(
                &mut app.notification_data.filter,
                NotificationFilter::All,
                NotificationFilter::All.get_name(),
            );
            ui.selectable_value(
                &mut app.notification_data.filter,
                NotificationFilter::RelayAuthenticationRequest,
                NotificationFilter::RelayAuthenticationRequest.get_name(),
            );
            ui.selectable_value(
                &mut app.notification_data.filter,
                NotificationFilter::RelayConnectionRequest,
                NotificationFilter::RelayConnectionRequest.get_name(),
            );
            ui.selectable_value(
                &mut app.notification_data.filter,
                NotificationFilter::Nip46Request,
                NotificationFilter::Nip46Request.get_name(),
            );
            ui.selectable_value(
                &mut app.notification_data.filter,
                NotificationFilter::PendingItem,
                NotificationFilter::PendingItem.get_name(),
            );
        });
}
