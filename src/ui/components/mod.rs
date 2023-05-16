mod nav_item;
mod relay_entry;

use eframe::egui;
use egui::{Label, Response, Sense, Ui};

pub use nav_item::NavItem;
pub use relay_entry::RelayEntry;

pub fn emoji_picker(ui: &mut Ui) -> Option<char> {
    let mut emojis = "😀😁😆😅😂🤣\
                      😕🥺😯😭😍🥰\
                      😊🫡🤔💀🫂👀\
                      ❤💜✨🔥⭐⚡\
                      👍🤙🤌🙏🤝🫰\
                      💯🎯✅👑🍆🚩"
        .chars();

    let mut output: Option<char> = None;

    let mut quit: bool = false;

    loop {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                for _ in 0..6 {
                    if let Some(emoji) = emojis.next() {
                        if ui
                            .add(Label::new(emoji.to_string()).sense(Sense::click()))
                            .clicked()
                        {
                            output = Some(emoji);
                        }
                    } else {
                        quit = true;
                    }
                }
            });
        });

        if quit {
            break;
        }
    }

    output
}

#[cfg(not(feature = "side-menu"))]
pub fn switch(ui: &mut Ui, on: &mut bool) -> Response {
    let desired_size = ui.spacing().interact_size.y * egui::vec2(2.0, 1.0);
    switch_with_size(ui, on, desired_size)
}

pub fn switch_with_size(ui: &mut Ui, on: &mut bool, size: egui::Vec2) -> Response {
    let (rect, mut response) = ui.allocate_exact_size(size, egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, *on, ""));

    if ui.is_rect_visible(rect) {
        let how_on = ui.ctx().animate_bool(response.id, *on);
        let visuals = ui.style().interact_selectable(&response, *on);

        // skip expansion, keep tight
        //let rect = rect.expand(visuals.expansion);

        let radius = 0.5 * rect.height();
        // bg_fill, bg_stroke, fg_stroke, expansion
        ui.painter()
            .rect(rect, radius, visuals.bg_fill, visuals.bg_stroke);
        let circle_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
        let center = egui::pos2(circle_x, rect.center().y);
        ui.painter().circle(
            center,
            0.875 * radius,
            visuals.fg_stroke.color,
            visuals.fg_stroke,
        );
    }

    response
}
