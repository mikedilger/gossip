mod avatar;
pub(crate) use avatar::{paint_avatar, AvatarSize};

mod button;
pub use button::Button;

mod contact_search;
pub(super) use contact_search::{capture_keyboard_for_search, show_contact_search};

mod copy_button;
pub(crate) mod list_entry;
pub use copy_button::{CopyButton, COPY_SYMBOL_SIZE};

mod nav_item;
use egui_winit::egui::text::LayoutJob;
use egui_winit::egui::text_edit::TextEditOutput;
use egui_winit::egui::{self, Align, FontSelection, RichText, Ui, WidgetText};
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
        .color(ui.visuals().widgets.noninteractive.fg_stroke.color);
    title.append_to(&mut layout, ui.style(), FontSelection::Default, Align::LEFT);
    page_header_layout(ui, layout, right_aligned_content)
}

pub fn page_header_layout<R>(
    ui: &mut Ui,
    galley: impl Into<WidgetText>,
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
pub fn truncated_label(ui: &mut Ui, text: impl Into<WidgetText>, max_width: f32) {
    let mut job = text.into().into_text_job(
        ui.style(),
        FontSelection::Default,
        ui.layout().vertical_align(),
    );
    job.job.sections.first_mut().unwrap().format.color =
        ui.visuals().widgets.noninteractive.fg_stroke.color;
    job.job.wrap.break_anywhere = true;
    job.job.wrap.max_width = max_width;
    job.job.wrap.max_rows = 1;
    let wgalley = ui.fonts(|fonts| job.into_galley(fonts));
    // the only way to force egui to respect all our above settings
    // is to pass in the galley directly
    ui.label(wgalley.galley);
}

pub fn break_anywhere_hyperlink_to(ui: &mut Ui, text: impl Into<WidgetText>, url: impl ToString) {
    let mut job = text.into().into_text_job(
        ui.style(),
        FontSelection::Default,
        ui.layout().vertical_align(),
    );
    job.job.wrap.break_anywhere = true;
    ui.hyperlink_to(job.job, url);
}

pub fn search_field(
    ui: &mut Ui,
    theme: &Theme,
    assets: &Assets,
    field: &mut String,
    width: f32,
) -> TextEditOutput {
    // search field
    let (output, _) = TextEdit::search(theme, assets, field)
        .text_color(ui.visuals().widgets.inactive.fg_stroke.color)
        .desired_width(width)
        .show(ui);

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
