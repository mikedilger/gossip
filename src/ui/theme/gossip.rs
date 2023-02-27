use std::collections::BTreeMap;

use crate::tags::HighlightType;

use super::Theme;
use eframe::egui;
use eframe::egui::FontDefinitions;
use eframe::egui::Style;
use eframe::egui::TextFormat;
use eframe::egui::TextStyle;
use eframe::egui::Visuals;
use eframe::egui::style::Selection;
use eframe::egui::style::WidgetVisuals;
use eframe::egui::style::Widgets;
use eframe::epaint::Color32;
use eframe::epaint::FontFamily;
use eframe::epaint::FontId;
use eframe::epaint::Rounding;
use eframe::epaint::Shadow;
use eframe::epaint::Stroke;

pub(crate) struct Gossip {}

impl Theme for Gossip {
    fn dark_mode(&self) -> Style {
        let mut style = egui::Style::default();
        style.visuals = Visuals {
            dark_mode: true,
            widgets: Widgets {
                noninteractive: WidgetVisuals {
                    weak_bg_fill: Color32::from_gray(27),
                    bg_fill: Color32::from_white_alpha(8),
                    bg_stroke: Stroke::new(1.0, Color32::from_gray(72)), // separators, borders
                    fg_stroke: Stroke::new(1.0, Color32::from_gray(190)), // normal text color
                    rounding: Rounding::same(2.0),
                    expansion: 0.0,
                },
                inactive: WidgetVisuals {
                    weak_bg_fill: Color32::from_gray(60),
                    bg_fill: Color32::from_white_alpha(8),
                    bg_stroke: Stroke::new(1.0, Color32::from_gray(72)), // separators, borders
                    fg_stroke: Stroke::new(1.0, Color32::from_gray(190)), // button text
                    rounding: Rounding::same(2.0),
                    expansion: 0.0,
                },
                hovered: WidgetVisuals {
                    weak_bg_fill: Color32::from_gray(70),
                    bg_fill: Color32::from_gray(70),
                    bg_stroke: Stroke::new(1.0, Color32::from_gray(150)), // e.g. hover over window edge or button
                    fg_stroke: Stroke::new(1.5, Color32::from_gray(240)),
                    rounding: Rounding::same(3.0),
                    expansion: 1.0,
                },
                active: WidgetVisuals {
                    weak_bg_fill: Color32::from_gray(55),
                    bg_fill: Color32::from_gray(55),
                    bg_stroke: Stroke::new(1.0, Color32::WHITE),
                    fg_stroke: Stroke::new(2.0, Color32::WHITE),
                    rounding: Rounding::same(2.0),
                    expansion: 1.0,
                },
                open: WidgetVisuals {
                    weak_bg_fill: Color32::from_gray(27),
                    bg_fill: Color32::from_gray(27),
                    bg_stroke: Stroke::new(1.0, Color32::from_gray(60)),
                    fg_stroke: Stroke::new(1.0, Color32::from_gray(210)),
                    rounding: Rounding::same(2.0),
                    expansion: 0.0,
                },
            },

            // Background colors
            window_fill: Color32::from_gray(0x24),
            panel_fill: Color32::from_gray(0x24),
            faint_bg_color: Color32::from_gray(0x14),
            extreme_bg_color: Color32::from_gray(0),
            code_bg_color: Color32::from_gray(64),

            // Foreground colors
            window_stroke: Stroke::new(1.0, Color32::from_gray(230)),
            override_text_color: None,
            warn_fg_color: Color32::from_rgb(255, 143, 0), // orange
            error_fg_color: Color32::from_rgb(255, 0, 0),  // red
            hyperlink_color: Color32::from_rgb(0x73, 0x95, 0xae), // light blue?

            selection: Selection {
                bg_fill: Color32::from_rgb(0x57, 0x4a, 0x40),
                stroke: Stroke::new(1.0, Color32::from_gray(230)),
            },

            window_shadow: Shadow::big_dark(),
            popup_shadow: Shadow::small_dark(),

            indent_has_left_vline: false,
            menu_rounding: Rounding::same(2.0),
            slider_trailing_fill: true,
            striped: true,
            window_rounding: Rounding::same(6.0),
            resize_corner_size: 12.0,
            text_cursor_width: 2.0,
            text_cursor_preview: false,
            clip_rect_margin: 3.0, // should be at least half the size of the widest frame stroke + max WidgetVisuals::expansion
            button_frame: true,
            collapsing_header_frame: false,
        };
        return style
    }

    fn light_mode(&self) -> Style {
        let mut style = egui::Style::default();
        style.visuals = Visuals {
            dark_mode: false,
            widgets: Widgets {
                noninteractive: WidgetVisuals {
                    weak_bg_fill: Color32::from_gray(248),
                    bg_fill: Color32::from_black_alpha(20),
                    bg_stroke: Stroke::new(1.0, Color32::from_gray(192)),
                    fg_stroke: Stroke::new(1.0, Color32::from_gray(80)), // normal text color
                    rounding: Rounding::same(2.0),
                    expansion: 0.0,
                },
                inactive: WidgetVisuals {
                    weak_bg_fill: Color32::from_gray(230),
                    bg_fill: Color32::from_black_alpha(20),
                    bg_stroke: Stroke::new(1.0, Color32::from_gray(192)),
                    fg_stroke: Stroke::new(1.0, Color32::from_gray(60)), // button text
                    rounding: Rounding::same(2.0),
                    expansion: 0.0,
                },
                hovered: WidgetVisuals {
                    weak_bg_fill: Color32::from_gray(220),
                    bg_fill: Color32::from_gray(220),
                    bg_stroke: Stroke::new(1.0, Color32::from_gray(105)), // e.g. hover over window edge or button
                    fg_stroke: Stroke::new(1.5, Color32::BLACK),
                    rounding: Rounding::same(3.0),
                    expansion: 1.0,
                },
                active: WidgetVisuals {
                    weak_bg_fill: Color32::from_gray(165),
                    bg_fill: Color32::from_gray(165),
                    bg_stroke: Stroke::new(1.0, Color32::BLACK),
                    fg_stroke: Stroke::new(2.0, Color32::BLACK),
                    rounding: Rounding::same(2.0),
                    expansion: 1.0,
                },
                open: WidgetVisuals {
                    weak_bg_fill: Color32::from_gray(220),
                    bg_fill: Color32::from_gray(220),
                    bg_stroke: Stroke::new(1.0, Color32::from_gray(160)),
                    fg_stroke: Stroke::new(1.0, Color32::BLACK),
                    rounding: Rounding::same(2.0),
                    expansion: 0.0,
                },
            },

            // Background colors
            window_fill: Color32::from_gray(0xed),
            panel_fill: Color32::from_gray(0xed),
            faint_bg_color: Color32::from_gray(0xf9),
            extreme_bg_color: Color32::from_gray(0xff),
            code_bg_color: Color32::from_gray(230),

            // Foreground colors
            window_stroke: Stroke::new(1.0, Color32::from_rgb(0x5d, 0x5c, 0x61)), // DONE
            override_text_color: None,
            warn_fg_color: Color32::from_rgb(255, 100, 0), // slightly orange red. it's difficult to find a warning color that pops on bright background.
            error_fg_color: Color32::from_rgb(255, 0, 0),  // red
            hyperlink_color: Color32::from_rgb(0x55, 0x7a, 0x95), // DONE

            selection: Selection {
                bg_fill: Color32::from_rgb(0xb1, 0xa2, 0x96), // DONE
                stroke: Stroke::new(1.0, Color32::from_rgb(0x5d, 0x5c, 0x61)), // DONE
            },

            window_shadow: Shadow::big_light(),
            popup_shadow: Shadow::small_light(),

            indent_has_left_vline: false,
            menu_rounding: Rounding::same(2.0),
            slider_trailing_fill: true,
            striped: true,
            window_rounding: Rounding::same(6.0),
            resize_corner_size: 12.0,
            text_cursor_width: 2.0,
            text_cursor_preview: false,
            clip_rect_margin: 3.0, // should be at least half the size of the widest frame stroke + max WidgetVisuals::expansion
            button_frame: true,
            collapsing_header_frame: false,
        };
        return style
    }

    fn font_definitions(&self) -> FontDefinitions {
        super::font_definitions() // use default gossip font definitions
    }

    fn highlight_text_format(&self, highlight_type: HighlightType, dark_mode: bool) -> TextFormat {
        let main = if dark_mode {
            Color32::WHITE
        } else {
            Color32::BLACK
        };
        let grey = if dark_mode {
            Color32::DARK_GRAY
        } else {
            Color32::LIGHT_GRAY
        };
        let green = if dark_mode {
            Color32::LIGHT_GREEN
        } else {
            Color32::DARK_GREEN
        };
        let red = if dark_mode {
            Color32::LIGHT_RED
        } else {
            Color32::DARK_RED
        };

        match highlight_type {
            HighlightType::Nothing => TextFormat {
                font_id: FontId::new(12.5, FontFamily::Proportional),
                color: main,
                ..Default::default()
            },
            HighlightType::PublicKey => TextFormat {
                font_id: FontId::new(12.5, FontFamily::Monospace),
                background: grey,
                color: green,
                ..Default::default()
            },
            HighlightType::Event => TextFormat {
                font_id: FontId::new(12.5, FontFamily::Monospace),
                background: grey,
                color: red,
                ..Default::default()
            },
        }
    }

    fn text_styles(&self) -> BTreeMap<TextStyle, FontId> {
        let mut text_styles: BTreeMap<TextStyle, FontId> = BTreeMap::new();

        text_styles.insert(
            TextStyle::Small,
            FontId {
                size: 10.75,
                family: FontFamily::Proportional,
            },
        );

        text_styles.insert(
            TextStyle::Body,
            FontId {
                size: 12.5,
                family: FontFamily::Proportional,
            },
        );

        text_styles.insert(
            TextStyle::Monospace,
            FontId {
                size: 12.5,
                family: FontFamily::Monospace,
            },
        );

        text_styles.insert(
            TextStyle::Button,
            FontId {
                size: 12.5,
                family: FontFamily::Proportional,
            },
        );

        text_styles.insert(
            TextStyle::Heading,
            FontId {
                size: 16.25,
                family: FontFamily::Proportional,
            },
        );

        text_styles
    }
}
