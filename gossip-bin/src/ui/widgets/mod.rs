mod avatar;
use std::sync::Arc;

pub(crate) use avatar::{paint_avatar, AvatarSize};

mod contact_search;
pub(super) use contact_search::{capture_keyboard_for_search, show_contact_search};

mod copy_button;
pub(crate) mod list_entry;
pub use copy_button::{CopyButton, COPY_SYMBOL_SIZE};

mod nav_item;
use crate::ui::egui::Rounding;
use eframe::egui::Galley;
use egui_winit::egui::text::LayoutJob;
use egui_winit::egui::text_edit::TextEditOutput;
use egui_winit::egui::{
    self, vec2, Align, FontSelection, Rect, Response, RichText, Rounding, Sense, Ui, WidgetText,
};
pub use nav_item::NavItem;

mod relay_entry;
pub use relay_entry::RelayEntry;

mod modal_popup;
pub use modal_popup::modal_popup;

mod more_menu;
pub(super) use more_menu::MoreMenu;

mod information_popup;
pub use information_popup::InformationPopup;
pub use information_popup::ProfilePopup;

mod switch;
pub use switch::Switch;
pub use switch::{switch_custom_at, switch_with_size};

mod textedit;
pub use textedit::TextEdit;

use super::GossipUi;

pub const DROPDOWN_DISTANCE: f32 = 10.0;
pub const TAGG_WIDTH: f32 = 200.0;

// pub fn break_anywhere_label(ui: &mut Ui, text: impl Into<WidgetText>) {
//     let mut job = text.into().into_text_job(
//         ui.style(),
//         FontSelection::Default,
//         ui.layout().vertical_align(),
//     );
//     job.job.sections.first_mut().unwrap().format.color =
//         ui.visuals().widgets.noninteractive.fg_stroke.color;
//     job.job.wrap.break_anywhere = true;
//     ui.label(job.job);
// }

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

pub fn search_field(ui: &mut Ui, field: &mut String, width: f32) -> TextEditOutput {
    // search field
    let output = TextEdit::singleline(field)
        .text_color(ui.visuals().widgets.inactive.fg_stroke.color)
        .desired_width(width)
        .show(ui);

    let rect = Rect::from_min_size(
        output.response.rect.right_top() - vec2(output.response.rect.height(), 0.0),
        vec2(output.response.rect.height(), output.response.rect.height()),
    );

    // search clear button
    if ui
        .put(
            rect,
            NavItem::new("\u{2715}", field.is_empty())
                .color(ui.visuals().widgets.inactive.fg_stroke.color)
                .active_color(ui.visuals().widgets.active.fg_stroke.color)
                .hover_color(ui.visuals().hyperlink_color)
                .sense(Sense::click()),
        )
        .clicked()
    {
        field.clear();
    }

    output
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
