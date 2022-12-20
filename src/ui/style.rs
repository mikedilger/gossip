use eframe::{egui, epaint};
use egui::style::{Selection, Visuals, Widgets};
use egui::{Color32, FontData, FontDefinitions, FontFamily, FontTweak, Rounding, Stroke};
use epaint::Shadow;
use std::collections::BTreeMap;

pub(super) fn dark_mode_visuals() -> Visuals {
    Visuals {
        dark_mode: true,
        override_text_color: None,
        widgets: Widgets::default(),
        selection: Selection {
            bg_fill: Color32::from_rgb(0xb1, 0xa2, 0x96), // DONE
            stroke: Stroke::new(1.0, Color32::from_rgb(0x37, 0x96, 0x83)), // DONE
        },
        hyperlink_color: Color32::from_rgb(0x73, 0x95, 0xae), // DONE
        faint_bg_color: Color32::from_gray(0x30),             // DONE
        extreme_bg_color: Color32::from_gray(0),              // e.g. TextEdit background
        code_bg_color: Color32::from_gray(64),
        warn_fg_color: Color32::from_rgb(255, 143, 0), // orange
        error_fg_color: Color32::from_rgb(255, 0, 0),  // red
        window_rounding: Rounding::same(6.0),
        window_shadow: Shadow::big_dark(),
        window_fill: Color32::from_gray(0x24), // DONE
        window_stroke: Stroke::new(1.0, Color32::from_rgb(0x37, 0x96, 0x83)), // DONE
        panel_fill: Color32::from_gray(0x24),  // DONE
        popup_shadow: Shadow::small_dark(),
        resize_corner_size: 12.0,
        text_cursor_width: 2.0,
        text_cursor_preview: false,
        clip_rect_margin: 3.0, // should be at least half the size of the widest frame stroke + max WidgetVisuals::expansion
        button_frame: true,
        collapsing_header_frame: false,
    }
}

pub(super) fn light_mode_visuals() -> Visuals {
    Visuals {
        dark_mode: false,
        override_text_color: None,
        widgets: Widgets::light(),
        selection: Selection {
            bg_fill: Color32::from_rgb(0xb1, 0xa2, 0x96), // DONE
            stroke: Stroke::new(1.0, Color32::from_rgb(0x5d, 0x5c, 0x61)), // DONE
        },
        hyperlink_color: Color32::from_rgb(0x55, 0x7a, 0x95), // DONE
        faint_bg_color: Color32::from_gray(0xf0),             // DONE
        extreme_bg_color: Color32::from_gray(0xff),           // e.g. TextEdit background
        code_bg_color: Color32::from_gray(230),
        warn_fg_color: Color32::from_rgb(255, 100, 0), // slightly orange red. it's difficult to find a warning color that pops on bright background.
        error_fg_color: Color32::from_rgb(255, 0, 0),  // red
        window_rounding: Rounding::same(6.0),
        window_shadow: Shadow::big_light(),
        window_fill: Color32::from_gray(0xF8), // DONE
        window_stroke: Stroke::new(1.0, Color32::from_rgb(0x5d, 0x5c, 0x61)), // DONE
        panel_fill: Color32::from_gray(0xF8),  // DONE
        popup_shadow: Shadow::small_light(),
        resize_corner_size: 12.0,
        text_cursor_width: 2.0,
        text_cursor_preview: false,
        clip_rect_margin: 3.0, // should be at least half the size of the widest frame stroke + max WidgetVisuals::expansion
        button_frame: true,
        collapsing_header_frame: false,
    }
}

pub(super) fn font_definitions() -> FontDefinitions {
    let mut font_data: BTreeMap<String, FontData> = BTreeMap::new();
    let mut families = BTreeMap::new();

    // Cantarell - gnome default
    // code fonts - Inconsolata-g, Hack


    font_data.insert(
        "Inconsolata".to_owned(),
        FontData::from_static(include_bytes!("../../fonts/Inconsolata-Regular.ttf")),
    );

    font_data.insert(
        "FreeSans".to_owned(),
        FontData::from_static(include_bytes!("../../fonts/freefont-20120503/FreeSans.otf")),
    );

    // Some good looking emojis. Use as first priority:
    font_data.insert(
        "NotoColorEmoji".to_owned(),
        FontData::from_static(include_bytes!("../../fonts/NotoColorEmoji.ttf")).tweak(
            FontTweak {
                scale: 0.81,           // make it smaller
                y_offset_factor: -0.2, // move it up
                y_offset: 0.0,
            },
        ),
    );

    font_data.insert(
        "NotoSansRegular".to_owned(),
        FontData::from_static(include_bytes!("../../fonts/NotoSans-Regular.ttf")),
    );

    font_data.insert(
        "NotoSansMonoRegular".to_owned(),
        FontData::from_static(include_bytes!("../../fonts/NotoSansMono-Regular.ttf")),
    );

    families.insert(
        FontFamily::Monospace,
        vec![
            "Inconsolata".to_owned(),
            "NotoColorEmoji".to_owned(),
            "NotoSansMonoRegular".to_owned(),
        ],
    );

    families.insert(
        FontFamily::Proportional,
        vec![
            "FreeSans".to_owned(),
            "NotoColorEmoji".to_owned(),
            "NotoSansRegular".to_owned(),
        ],
    );

    FontDefinitions {
        font_data,
        families,
    }
}
