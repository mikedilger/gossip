use egui_winit::egui::{Ui, InnerResponse, self};


pub fn modal_popup(
    ui: &mut Ui,
    dlg_size: egui::Vec2,
    content: impl FnOnce(&mut Ui)) -> InnerResponse<egui::Response> {

    let content = |ui: &mut Ui| {
        ui.set_min_size(dlg_size);
        ui.set_max_size(dlg_size);

        content(ui);

        // paint the close button
        // ui.max_rect is inner_margin size
        let tr = ui.max_rect().right_top();
        let rect = egui::Rect::from_x_y_ranges(tr.x - 5.0..=tr.x + 5.0, tr.y + 5.0..=tr.y + 15.0);
        ui.allocate_ui_at_rect(rect, |ui| {
            ui.add_sized(rect.size(), super::NavItem::new("\u{274C}", false))
        }).inner
    };

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
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]);
        area.show_open_close_animation(
            ui.ctx(),
            &frame,
            true, // TODO if we never pass false it won't show a close animation
        );
        area.show(ui.ctx(), |ui| {
            frame.fill = egui::Color32::WHITE;
            frame.rounding = egui::Rounding::same(10.0);
            frame.inner_margin = egui::Margin::symmetric(20.0, 10.0);
            frame.show(ui, content).inner
        })
}
