mod copy_button;
pub(crate) mod list_entry;
pub use copy_button::{CopyButton, COPY_SYMBOL_SIZE};

mod nav_item;
use egui_winit::egui::{vec2, FontSelection, Rect, Response, Sense, TextEdit, Ui, WidgetText};
pub use nav_item::NavItem;

mod relay_entry;
pub use relay_entry::{RelayEntry, RelayEntryView};

use super::GossipUi;

// pub fn break_anywhere_label(ui: &mut Ui, text: impl Into<WidgetText>) {
//     let mut job = text.into().into_text_job(
//         ui.style(),
//         FontSelection::Default,
//         ui.layout().vertical_align(),
//     );
//     job.job.sections.first_mut().unwrap().format.color =
//         ui.style().visuals.widgets.noninteractive.fg_stroke.color;
//     job.job.wrap.break_anywhere = true;
//     ui.label(job.job);
// }

pub fn break_anywhere_hyperlink_to(ui: &mut Ui, text: impl Into<WidgetText>, url: impl ToString) {
    let mut job = text.into().into_text_job(
        ui.style(),
        FontSelection::Default,
        ui.layout().vertical_align(),
    );
    job.job.wrap.break_anywhere = true;
    ui.hyperlink_to(job.job, url);
}

pub fn search_filter_field(ui: &mut Ui, field: &mut String, width: f32) -> Response {
    // search field
    let response = ui.add(
        TextEdit::singleline(field)
            .text_color(ui.visuals().widgets.inactive.fg_stroke.color)
            .desired_width(width),
    );
    let rect = Rect::from_min_size(
        response.rect.right_top() - vec2(response.rect.height(), 0.0),
        vec2(response.rect.height(), response.rect.height()),
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

    response
}

pub(super) fn set_important_button_visuals(ui: &mut Ui, app: &GossipUi) {
    let visuals = ui.visuals_mut();
    visuals.widgets.inactive.weak_bg_fill = app.theme.accent_color();
    visuals.widgets.inactive.fg_stroke.width = 1.0;
    visuals.widgets.inactive.fg_stroke.color =
        app.theme.get_style().visuals.extreme_bg_color;
    visuals.widgets.hovered.weak_bg_fill = app.theme.navigation_text_color();
    visuals.widgets.hovered.fg_stroke.color = app.theme.accent_color();
    visuals.widgets.inactive.fg_stroke.color =
        app.theme.get_style().visuals.extreme_bg_color;
}
