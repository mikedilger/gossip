use eframe::epaint::Color32;
use egui_winit::egui::{self, InnerResponse, Ui};

const MARGIN_X: f32 = 80.0;
const MARGIN_Y: f32 = 40.0;

pub fn modal_popup(
    ui: &mut Ui,
    dlg_size: egui::Vec2,
    content: impl FnOnce(&mut Ui),
) -> InnerResponse<egui::Response> {
    let content = |ui: &mut Ui| {
        ui.set_min_size(dlg_size);
        ui.set_max_size(dlg_size);

        content(ui);

        // paint the close button
        // ui.max_rect is inner_margin size
        let tr = ui.max_rect().right_top() + egui::vec2(MARGIN_X, -MARGIN_Y);
        let rect =
            egui::Rect::from_x_y_ranges(tr.x - 30.0..=tr.x - 15.0, tr.y + 15.0..=tr.y + 30.0);
        egui::Area::new(ui.auto_id_with("_sym"))
            .movable(false)
            .order(egui::Order::Foreground)
            .fixed_pos(rect.left_top())
            .show(ui.ctx(), |ui| {
                ui.add_sized(rect.size(), super::NavItem::new("\u{274C}", false))
            })
            .inner
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
                if ui.visuals().dark_mode {
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 1)
                } else {
                    egui::Color32::from_rgba_unmultiplied(0, 0, 0, 60)
                },
            );
        });

    let mut frame = egui::Frame::popup(ui.style());
    let area = egui::Area::new("modal-popup")
        .movable(false)
        .interactable(true)
        .order(egui::Order::Middle)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]);
    area.show_open_close_animation(
        ui.ctx(),
        &frame,
        true, // TODO if we never pass false it won't show a close animation
    );
    area.show(ui.ctx(), |ui| {
        if ui.visuals().dark_mode {
            frame.fill = ui.visuals().faint_bg_color;
        } else {
            frame.fill = Color32::WHITE;
        }
        frame.rounding = egui::Rounding::same(10.0);
        frame.inner_margin = egui::Margin::symmetric(MARGIN_X, MARGIN_Y);
        frame.show(ui, content).inner
    })
}
