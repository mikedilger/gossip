use crate::ui::{GossipUi, ThemeVariant};
use eframe::egui;
use egui::widgets::{Button, Slider};
use egui::{Context, Ui};

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.heading("UI Settings");

    ui.add_space(20.0);
    ui.checkbox(
        &mut app.unsaved_settings.highlight_unread_events,
        "Highlight unread events",
    );
    ui.checkbox(
        &mut app.unsaved_settings.posting_area_at_top,
        "Show posting area at the top instead of the bottom",
    );
    ui.checkbox(
        &mut app.unsaved_settings.feed_newest_at_bottom,
        "Order feed with newest at bottom (intead of top)",
    );

    ui.add_space(20.0);
    ui.horizontal(|ui| {
        ui.label("Theme:");
        if !app.unsaved_settings.follow_os_dark_mode {
            if app.unsaved_settings.dark_mode {
                if ui.add(Button::new("🌙 Dark")).on_hover_text("Switch to light mode").clicked() {
                    app.unsaved_settings.dark_mode = false;
                }
            } else {
                if ui.add(Button::new("☀ Light")).on_hover_text("Switch to dark mode").clicked() {
                    app.unsaved_settings.dark_mode = true;
                }
            }
        }
        let theme_combo = egui::ComboBox::from_id_source("Theme");
        theme_combo.selected_text(&app.unsaved_settings.theme_variant).show_ui(ui, |ui| {
            for theme_variant in ThemeVariant::all() {
                if ui.add(egui::widgets::SelectableLabel::new(theme_variant.name() == app.unsaved_settings.theme_variant, theme_variant.name())).clicked() {
                    app.unsaved_settings.theme_variant = theme_variant.name().to_string();
                };
            }
        });
        ui.checkbox(&mut app.unsaved_settings.follow_os_dark_mode, "Follow OS dark-mode").on_hover_text("Follow the operating system setting for dark-mode (requires app-restart to take effect)");
    });

    ui.add_space(20.0);
    ui.horizontal(|ui| {
        ui.label("Override DPI: ").on_hover_text("On some systems, DPI is not reported properly. In other cases, people like to zoom in or out. This lets you.");
        ui.checkbox(&mut app.override_dpi, "Override to ");
        ui.add(Slider::new(&mut app.override_dpi_value, 72..=250).text("DPI"));
    });
    ui.horizontal(|ui| {
        ui.add_space(10.0); // indent

        if ui.button("Reset native").clicked() {
            let native_ppt = ctx.native_pixels_per_point().unwrap_or(1.0);
            app.override_dpi_value = (native_ppt * 72.0) as u32;
            ctx.set_pixels_per_point(native_ppt);
        }

        if ui.button("Test (without saving)").clicked() {
            let ppt: f32 = app.override_dpi_value as f32 / 72.0;
            ctx.set_pixels_per_point(ppt);
        }

        // transfer to app.unsaved_settings
        app.unsaved_settings.override_dpi = if app.override_dpi {
            // Set it in settings to be saved on button press
            Some(app.override_dpi_value)
        } else {
            None
        };
    });

    ui.add_space(20.0);
    ui.horizontal(|ui| {
        ui.label("Maximum FPS: ").on_hover_text("The UI redraws every frame. By limiting the maximum FPS you can reduce load on your CPU. Takes effect immediately. I recommend 10, maybe even less.");
        ui.add(Slider::new(&mut app.unsaved_settings.max_fps, 2..=60).text("Frames per second"));
    });

    ui.add_space(20.0);
    ui.checkbox(
        &mut app.unsaved_settings.status_bar,
        "Show DEBUG statistics in sidebar",
    );

    ui.add_space(20.0);
    ui.checkbox(
        &mut app.unsaved_settings.inertial_scrolling,
        "Inertial Scrolling",
    );

    ui.add_space(10.0);
    ui.horizontal(|ui| {
        let accel = app.unsaved_settings.mouse_acceleration;
        ui.add(
            Slider::new(
                &mut app.unsaved_settings.mouse_acceleration,
                accel.min(0.5)..=accel.max(2.0),
            )
            .clamp_to_range(false)
            .text("Mouse acceleration"),
        );
    });

    ui.add_space(20.0);
}
