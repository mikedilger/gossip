use std::{rc::Rc, sync::Arc};

use eframe::{
    egui::{pos2, Context, Vec2},
    epaint::{Color32, Shadow},
};
use egui_winit::egui::{self, InnerResponse, Ui};

use crate::ui::GossipUi;

const MARGIN_X: f32 = 80.0;
const MARGIN_Y: f32 = 40.0;

pub struct ModalEntry {
    pub min_size: Vec2,
    pub max_size: Vec2,
    pub content: Rc<dyn Fn(&mut Ui, &mut GossipUi)>,
    pub on_close: Rc<dyn Fn(&mut GossipUi)>,
}

/// Create a modal overlay
/// check [`response.inner`] return for clicks to the close button
pub fn modal_popup(
    ctx: &Context,
    min_size: egui::Vec2,
    max_size: egui::Vec2,
    closable: bool,
    content: impl FnOnce(&mut Ui),
) -> InnerResponse<egui::Response> {
    let content = |ui: &mut Ui| {
        ui.set_min_size(min_size);
        ui.set_max_size(max_size);

        content(ui);

        if closable {
            // paint the close button
            // ui.max_rect is inner_margin size
            let tr = ui.max_rect().right_top() + egui::vec2(MARGIN_X, -MARGIN_Y);
            let rect =
                egui::Rect::from_x_y_ranges(tr.x - 30.0..=tr.x - 15.0, tr.y + 15.0..=tr.y + 30.0);
            egui::Area::new(egui::Id::new("modal_popup_sym"))
                .movable(false)
                .order(egui::Order::Foreground)
                .fixed_pos(rect.left_top())
                .show(ui.ctx(), |ui| {
                    ui.add_sized(rect.size(), super::NavItem::new("\u{274C}", false))
                })
                .inner
        } else {
            // dummy response
            ui.allocate_response(egui::vec2(1.0, 1.0), egui::Sense::click())
        }
    };

    egui::Area::new(egui::Id::new("hide-background-area"))
        .fixed_pos(ctx.screen_rect().left_top())
        .movable(false)
        .interactable(false)
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            ui.painter().rect_filled(
                ctx.screen_rect(),
                egui::Rounding::same(0.0),
                egui::Color32::from_rgba_unmultiplied(0, 0, 0, 80),
            );
        });

    let mut frame = egui::Frame::popup(&ctx.style());
    let area = egui::Area::new(egui::Id::new("modal-popup"))
        .movable(false)
        .interactable(true)
        .constrain(true)
        .order(egui::Order::Middle)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]);
    area.show_open_close_animation(
        ctx, &frame, true, // TODO if we never pass false it won't show a close animation
    );
    area.show(ctx, |ui| {
        if ui.visuals().dark_mode {
            frame.fill = ui.visuals().faint_bg_color;
            frame.shadow = Shadow::NONE;
        } else {
            frame.fill = Color32::WHITE;
        }
        frame.rounding = egui::Rounding::same(10.0);
        frame.inner_margin = egui::Margin::symmetric(MARGIN_X, MARGIN_Y);
        frame.show(ui, content).inner
    })
}

/// Create a modal overlay
/// check [`response.inner`] return for clicks to the close button
pub fn modal_popup_dyn(
    ctx: &Context,
    app: &mut GossipUi,
    closable: bool,
    entry: Arc<ModalEntry>,
) -> InnerResponse<egui::Response> {
    let content = |ui: &mut Ui| {
        ui.set_min_size(entry.min_size);
        ui.set_max_size(entry.max_size);

        (entry.content)(ui, app);

        if closable {
            // paint the close button
            // ui.max_rect is inner_margin size
            let tr = pos2(ui.available_rect_before_wrap().right(), ui.min_rect().top())
                + egui::vec2(MARGIN_X, -MARGIN_Y);
            let rect =
                egui::Rect::from_x_y_ranges(tr.x - 30.0..=tr.x - 15.0, tr.y + 15.0..=tr.y + 30.0);
            let response = egui::Area::new(egui::Id::new("modal_popup_sym"))
                .movable(false)
                .order(egui::Order::Foreground)
                .fixed_pos(rect.left_top())
                .show(ui.ctx(), |ui| {
                    ui.add_sized(rect.size(), super::NavItem::new("\u{274C}", false))
                })
                .inner;
            if response.clicked() {
                (entry.on_close)(app);
            }
            response
        } else {
            // dummy response
            ui.allocate_response(egui::vec2(1.0, 1.0), egui::Sense::click())
        }
    };

    egui::Area::new(egui::Id::new("hide-background-area"))
        .fixed_pos(ctx.screen_rect().left_top())
        .movable(false)
        .interactable(false)
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            ui.painter().rect_filled(
                ctx.screen_rect(),
                egui::Rounding::same(0.0),
                egui::Color32::from_rgba_unmultiplied(0, 0, 0, 80),
            );
        });

    let mut frame = egui::Frame::popup(&ctx.style());
    let area = egui::Area::new(egui::Id::new("modal-popup"))
        .movable(false)
        .interactable(true)
        .constrain(true)
        .order(egui::Order::Middle)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]);
    area.show_open_close_animation(
        ctx, &frame, true, // TODO if we never pass false it won't show a close animation
    );
    let frame_response = area
        .show(ctx, |ui| {
            if ui.visuals().dark_mode {
                frame.fill = ui.visuals().faint_bg_color;
                frame.shadow = Shadow::NONE;
            } else {
                frame.fill = Color32::WHITE;
            }
            frame.rounding = egui::Rounding::same(10.0);
            frame.inner_margin = egui::Margin::symmetric(MARGIN_X, MARGIN_Y);
            frame.show(ui, content)
        })
        .inner;

    if frame_response.response.clicked_elsewhere() {
        (entry.on_close)(app);
    }

    frame_response
}
