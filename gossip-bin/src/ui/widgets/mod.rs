mod avatar;
use std::sync::Arc;

pub(crate) use avatar::{paint_avatar, paint_avatar_only, AvatarSize};

mod button;
pub use button::Button;

mod contact_search;
pub(super) use contact_search::{capture_keyboard_for_search, show_contact_search};

mod copy_button;
pub(crate) mod list_entry;
pub use copy_button::{CopyButton, COPY_SYMBOL_SIZE};

mod nav_item;
use eframe::egui::{vec2, FontId, Galley, Rect};
use egui_winit::egui::text::LayoutJob;
use egui_winit::egui::{
    self, Align, FontSelection, Response, RichText, Rounding, Sense, Ui, WidgetText,
};
pub use nav_item::NavItem;

mod relay_entry;
use nostr_types::RelayUrl;
pub use relay_entry::RelayEntry;

mod modal_popup;
pub use modal_popup::{modal_popup, modal_popup_dyn, ModalEntry};

mod more_menu;
pub(super) use more_menu::{MoreMenu, MoreMenuButton, MoreMenuItem, MoreMenuSubMenu};

mod information_popup;
pub use information_popup::InformationPopup;
pub use information_popup::ProfilePopup;

mod switch;
pub use switch::switch_custom_at;
pub use switch::Switch;

mod textedit;
pub use textedit::TextEdit;

use super::assets::Assets;
use super::{GossipUi, Theme};

pub const DROPDOWN_DISTANCE: f32 = 10.0;
pub const TAGG_WIDTH: f32 = 200.0;

pub enum WidgetState {
    Default,
    Hovered,
    Active,
    Disabled,
    Focused,
}

pub fn page_header<R>(
    ui: &mut Ui,
    title: impl Into<egui::RichText>,
    right_aligned_content: impl FnOnce(&mut Ui) -> R,
) {
    let mut layout = LayoutJob::default();
    let title: RichText = title
        .into()
        .heading()
        .color(ui.visuals().widgets.noninteractive.fg_stroke.color);
    title.append_to(&mut layout, ui.style(), FontSelection::Default, Align::LEFT);
    let galley = ui.fonts(|fonts| fonts.layout_job(layout));
    page_header_layout(ui, galley, right_aligned_content)
}

pub fn page_header_layout<R>(
    ui: &mut Ui,
    galley: Arc<Galley>,
    right_aligned_content: impl FnOnce(&mut Ui) -> R,
) {
    ui.vertical(|ui| {
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.add_space(2.0);
                ui.label(galley);
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(20.0);
                right_aligned_content(ui);
            });
        });
        ui.add_space(10.0);
    });
}

/// Create a label which truncates after max_width
pub fn truncated_label(ui: &mut Ui, text: impl Into<WidgetText>, max_width: f32) -> Response {
    let mut job = text.into().into_layout_job(
        ui.style(),
        FontSelection::Default,
        ui.layout().vertical_align(),
    );
    job.sections.first_mut().unwrap().format.color =
        ui.visuals().widgets.noninteractive.fg_stroke.color;
    job.wrap.break_anywhere = true;
    job.wrap.max_width = max_width;
    job.wrap.max_rows = 1;

    // new way of creating a galley since the above only creates a job now
    let galley = ui.fonts(|fonts| fonts.layout_job(job));

    // the only way to force egui to respect all our above settings
    // is to pass in the galley directly
    ui.label(galley)
}

/// Display a relay-URL
pub fn relay_url(ui: &mut Ui, theme: &Theme, url: &RelayUrl) -> Response {
    let (symbol, color, spacer) = if url.as_url_crate_url().scheme() != "wss" {
        (
            "\u{00A0}\u{00A0}\u{1F513}",
            theme.red_500(),
            "\u{00A0}\u{00A0}\u{00A0}",
        )
    } else {
        ("", theme.accent_color(), "")
    };
    let text = format!(
        "{}{}",
        spacer,
        url.as_url_crate_url().domain().unwrap_or_default()
    );
    let response = ui.link(text);

    let mut font = FontId::default();
    font.size *= 0.7;

    ui.painter().text(
        response.rect.left_top(),
        egui::Align2::CENTER_TOP,
        symbol,
        font,
        color,
    );

    response
}

/// Create a clickable label
pub fn clickable_label(ui: &mut Ui, enabled: bool, text: impl Into<WidgetText>) -> Response {
    let label = egui::Label::new(text)
        .selectable(false)
        .sense(Sense::click());
    ui.add_enabled(enabled, label)
}

pub fn break_anywhere_hyperlink_to(ui: &mut Ui, text: impl Into<WidgetText>, url: impl ToString) {
    let mut job = text.into().into_layout_job(
        ui.style(),
        FontSelection::Default,
        ui.layout().vertical_align(),
    );
    job.wrap.break_anywhere = true;
    ui.hyperlink_to(job, url);
}

pub fn options_menu_button(ui: &mut Ui, theme: &Theme, assets: &Assets) -> Response {
    let (response, painter) = ui.allocate_painter(vec2(20.0, 20.0), egui::Sense::click());
    let btn_rect = response.rect;
    let color = if response.hovered() {
        theme.accent_color()
    } else {
        ui.visuals().text_color()
    };
    let mut mesh = egui::Mesh::with_texture((&assets.options_symbol).into());
    mesh.add_rect_with_uv(
        btn_rect.shrink(2.0),
        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        color,
    );
    painter.add(egui::Shape::mesh(mesh));
    response
}

pub fn giant_spinner(ui: &mut Ui, theme: &Theme) -> Response {
    // show a spinner
    let size = ui.available_width() / 2.0;
    ui.horizontal(|ui| {
        ui.add_space((ui.available_width() - size) / 2.0);
        let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
        {
            ui.ctx().request_repaint(); // because it is animated

            let spinner_color = if theme.dark_mode {
                theme.neutral_950()
            } else {
                egui::Color32::WHITE
            };
            let radius = (rect.height() / 2.0) - 2.0;
            let n_points = 240;
            let time = ui.input(|i| i.time);
            let start_angle = time * std::f64::consts::TAU;
            let end_angle = start_angle + 240f64.to_radians() * time.sin();
            let points: Vec<egui::Pos2> = (0..n_points)
                .map(|i| {
                    let angle = egui::lerp(start_angle..=end_angle, i as f64 / n_points as f64);
                    let (sin, cos) = angle.sin_cos();
                    rect.center() + radius * egui::vec2(cos as f32, sin as f32)
                })
                .collect();
            for point in points {
                ui.painter().circle_filled(point, 15.0, spinner_color);
            }
        }
        ui.painter().text(
            response.rect.center(),
            egui::Align2::CENTER_CENTER,
            "Loading",
            FontId::proportional(16.0),
            ui.visuals().text_color(),
        );
    })
    .response
}

pub(super) fn set_important_button_visuals(ui: &mut Ui, app: &GossipUi) {
    let visuals = ui.visuals_mut();
    visuals.widgets.inactive.weak_bg_fill = app.theme.accent_color();
    visuals.widgets.inactive.fg_stroke.width = 1.0;
    visuals.widgets.inactive.fg_stroke.color = app.theme.get_style().visuals.extreme_bg_color;
    visuals.widgets.hovered.weak_bg_fill = app.theme.navigation_text_color();
    visuals.widgets.hovered.fg_stroke.color = app.theme.accent_color();
    visuals.widgets.inactive.fg_stroke.color = app.theme.get_style().visuals.extreme_bg_color;
}

#[allow(dead_code)]
pub(crate) fn warning_frame<R>(
    ui: &mut Ui,
    app: &mut GossipUi,
    inner: impl FnOnce(&mut Ui, &mut GossipUi) -> R,
) -> R {
    egui::Frame::none()
        .outer_margin(egui::Margin {
            left: 0.0,
            right: 0.0,
            top: 10.0,
            bottom: 20.0,
        })
        .inner_margin(egui::Margin::same(20.0))
        .fill(egui::Color32::from_rgb(0xFB, 0xBF, 0x24))
        .rounding(Rounding::same(4.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                ui.visuals_mut().override_text_color =
                    Some(egui::Color32::from_rgb(0x0a, 0x0a, 0x0a));
                inner(ui, app)
            })
            .inner
        })
        .inner
}

// /// UTF-8 safe truncate (String::truncate() can panic)
// #[inline]
// pub fn safe_truncate(s: &str, max_chars: usize) -> &str {
//     let v: Vec<&str> = s.split('\n').collect();
//     let s = v.first().unwrap_or(&s);
//     match s.char_indices().nth(max_chars) {
//         None => s,
//         Some((idx, _)) => &s[..idx],
//     }
// }

// #[test]
// fn safe_truncate_single_line() {
//     let input = "0123456789";
//     let output = safe_truncate(input, 5);
//     assert_eq!(&input[0..5], output);
// }

// #[test]
// fn safe_truncate_multi_line() {
//     let input = "1234567890\nabcdefg\nhijklmn";
//     let output = safe_truncate(input, 20);
//     assert_eq!(&input[0..10], output);
// }

fn interact_widget_state(ui: &mut Ui, response: &Response) -> WidgetState {
    if response.is_pointer_button_down_on() {
        WidgetState::Active
    } else if response.has_focus() {
        WidgetState::Focused
    } else if response.hovered() || response.highlighted() {
        WidgetState::Hovered
    } else if !ui.is_enabled() {
        WidgetState::Disabled
    } else {
        WidgetState::Default
    }
}
