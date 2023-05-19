use eframe::egui;
use egui::{Label, Response, Sense, Ui};
use egui_winit::egui::{Color32, Id, Rect};
use qrcode::Color;

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
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::click());
    switch_with_size_at(ui, on, size, rect.left_top(), ui.next_auto_id())
}

pub fn switch_with_size_at(
    ui: &mut Ui,
    on: &mut bool,
    size: egui::Vec2,
    pos: egui::Pos2,
    id: Id,
) -> Response {
    let rect = Rect::from_min_size(pos, size);
    let mut response = ui.interact(rect, id, egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response
        .clone()
        .on_hover_cursor(egui::CursorIcon::PointingHand);
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

pub fn switch_custom_at(
    ui: &mut Ui,
    on: &mut bool,
    size: egui::Vec2,
    pos: egui::Pos2,
    id: Id,
    knob_fill: Color32,
    on_fill: Color32,
    off_fill: Color32,
) -> Response {
    let rect = Rect::from_min_size(pos, size);
    let mut response = ui.interact(rect, id, egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    response
        .clone()
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, *on, ""));

    if ui.is_rect_visible(rect) {
        let how_on = ui.ctx().animate_bool(response.id, *on);
        let visuals = ui.style().interact_selectable(&response, *on);

        // skip expansion, keep tight
        //let rect = rect.expand(visuals.expansion);

        let radius = 0.5 * rect.height();
        // bg_fill, bg_stroke, fg_stroke, expansion
        let bg_fill = if visuals == ui.visuals().widgets.inactive {
            visuals.bg_fill
        } else if *on {
            on_fill
        } else {
            off_fill
        };
        ui.painter().rect(rect, radius, bg_fill, visuals.bg_stroke);
        let circle_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
        let center = egui::pos2(circle_x, rect.center().y);
        ui.painter()
            .circle(center, 0.875 * radius, knob_fill, visuals.fg_stroke);
    }

    response
}
