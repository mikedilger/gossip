mod copy_button;
pub use copy_button::CopyButton;

mod nav_item;
pub use nav_item::NavItem;

mod relay_entry;
pub use relay_entry::RelayEntry;

use eframe::egui::{FontSelection, Ui, WidgetText};

pub fn break_anywhere_label(ui: &mut Ui, text: impl Into<WidgetText>) {
    let mut job = text.into().into_text_job(
        ui.style(),
        FontSelection::Default,
        ui.layout().vertical_align(),
    );
    job.job.sections.first_mut().unwrap().format.color =
        ui.style().visuals.widgets.noninteractive.fg_stroke.color;
    job.job.wrap.break_anywhere = true;
    ui.label(job.job);
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
