use egui_winit::egui::{Ui, InnerResponse, self};


pub fn modal_popup(
    ui: &mut Ui,
    heading: impl Into<String>,
    content: impl FnOnce(&mut Ui)) -> InnerResponse<egui::Response> {
    let dlg_size = egui::vec2(ui.min_rect().width() * 0.66, 120.0);

    egui::Area::new("hide-background-area")
        .fixed_pos(ui.ctx().screen_rect().left_top())
        .movable(false)
        .interactable(false)
        .order(egui::Order::Middle)
        .show(ui.ctx(), |ui| {
            ui.painter().rect_filled(
                ui.ctx().screen_rect(),
                egui::Rounding::same(0.0),
                egui::Color32::from_rgba_unmultiplied(0x9f, 0x9f, 0x9f, 102),
            );
        });

    let mut frame = egui::Frame::popup(ui.style());
        let area = egui::Area::new("modal-popup")
            .movable(false)
            .interactable(true)
            .order(egui::Order::Foreground)
            .fixed_pos(ui.min_rect().center() - egui::vec2(dlg_size.x / 2.0, dlg_size.y));
        area.show_open_close_animation(
            ui.ctx(),
            &frame,
            true, // TODO if we never pass false it won't show a close animation
        );
        area.show(ui.ctx(), |ui| {
            frame.fill = ui.visuals().extreme_bg_color;
            frame.inner_margin = egui::Margin::symmetric(20.0, 10.0);
            frame.show(ui, |ui| {
                ui.set_min_size(dlg_size);
                ui.set_max_size(dlg_size);

                // ui.max_rect is inner_margin size
                let tr = ui.max_rect().right_top();

                ui.vertical(|ui| {
                    let close_response = ui.horizontal(|ui| {
                        ui.heading(heading.into());
                        let rect = egui::Rect::from_x_y_ranges(tr.x..=tr.x + 10.0, tr.y - 20.0..=tr.y - 10.0);
                        ui.allocate_ui_at_rect(rect, |ui| {
                            ui.add_sized(rect.size(), super::NavItem::new("\u{274C}", false))
                        }).inner
                    }).inner;

                    content(ui);
                    close_response
                }).inner
            }).inner
        })
}
