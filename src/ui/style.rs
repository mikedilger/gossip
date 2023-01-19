use crate::tags::HighlightType;
use eframe::{egui, epaint};
use egui::style::{Selection, Visuals, Widgets};
use egui::{
    Color32, FontData, FontDefinitions, FontFamily, FontId, FontTweak, Rounding, Stroke, TextStyle,
};
use epaint::text::TextFormat;
use epaint::Shadow;
use std::collections::BTreeMap;

pub(super) fn dark_mode_visuals() -> Visuals {
    Visuals {
        dark_mode: true,
        widgets: Widgets::default(),

        // Background colors
        window_fill: Color32::from_gray(0x24),
        panel_fill: Color32::from_gray(0x24),
        faint_bg_color: Color32::from_gray(0x14),
        extreme_bg_color: Color32::from_gray(0),
        code_bg_color: Color32::from_gray(64),

        // Foreground colors
        window_stroke: Stroke::new(1.0, Color32::from_rgb(0x37, 0x96, 0x83)),
        override_text_color: Some(Color32::from_gray(190)),
        warn_fg_color: Color32::from_rgb(255, 143, 0), // orange
        error_fg_color: Color32::from_rgb(255, 0, 0),  // red
        hyperlink_color: Color32::from_rgb(0x73, 0x95, 0xae),

        selection: Selection {
            bg_fill: Color32::from_rgb(0x57, 0x4a, 0x40),
            stroke: Stroke::new(1.0, Color32::from_rgb(0x37, 0x96, 0x83)),
        },

        window_shadow: Shadow::big_dark(),
        popup_shadow: Shadow::small_dark(),

        window_rounding: Rounding::same(6.0),
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
        widgets: Widgets::light(),

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

        window_rounding: Rounding::same(6.0),
        resize_corner_size: 12.0,
        text_cursor_width: 2.0,
        text_cursor_preview: false,
        clip_rect_margin: 3.0, // should be at least half the size of the widest frame stroke + max WidgetVisuals::expansion
        button_frame: true,
        collapsing_header_frame: false,
    }
}

pub(crate) fn highlight_text_format(highlight_type: HighlightType, dark_mode: bool) -> TextFormat {
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
            font_id: FontId::new(12.0, FontFamily::Proportional),
            color: main,
            ..Default::default()
        },
        HighlightType::PublicKey => TextFormat {
            font_id: FontId::new(12.0, FontFamily::Monospace),
            background: grey,
            color: green,
            ..Default::default()
        },
        HighlightType::Event => TextFormat {
            font_id: FontId::new(12.0, FontFamily::Monospace),
            background: grey,
            color: red,
            ..Default::default()
        },
    }
}

pub(super) fn text_styles() -> BTreeMap<TextStyle, FontId> {
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
            size: 12.0,
            family: FontFamily::Proportional,
        },
    );

    text_styles.insert(
        TextStyle::Monospace,
        FontId {
            size: 12.0,
            family: FontFamily::Monospace,
        },
    );

    text_styles.insert(
        TextStyle::Button,
        FontId {
            size: 13.0,
            family: FontFamily::Proportional,
        },
    );

    text_styles.insert(
        TextStyle::Heading,
        FontId {
            size: 17.0,
            family: FontFamily::Proportional,
        },
    );

    text_styles
}

pub(super) fn font_definitions() -> FontDefinitions {
    let mut font_data: BTreeMap<String, FontData> = BTreeMap::new();
    let mut families = BTreeMap::new();

    font_data.insert(
        "DejaVuSans".to_owned(),
        FontData::from_static(include_bytes!("../../fonts/DejaVuSansSansEmoji.ttf")),
    );

    font_data.insert(
        "Inconsolata".to_owned(),
        FontData::from_static(include_bytes!("../../fonts/Inconsolata-Regular.ttf")).tweak(
            FontTweak {
                scale: 1.22,            // This font is smaller than DejaVuSans
                y_offset_factor: -0.18, // and too low
                y_offset: 0.0,
            },
        ),
    );

    // Some good looking emojis. Use as first priority:
    font_data.insert(
        "NotoEmoji-Regular".to_owned(),
        FontData::from_static(include_bytes!("../../fonts/NotoEmoji-Regular.ttf")).tweak(
            FontTweak {
                scale: 1.1,             // make them a touch larger
                y_offset_factor: -0.26, // move them up
                y_offset: 0.0,
            },
        ),
    );

    families.insert(
        FontFamily::Proportional,
        vec!["DejaVuSans".to_owned(), "NotoEmoji-Regular".to_owned()],
    );

    families.insert(
        FontFamily::Monospace,
        vec!["Inconsolata".to_owned(), "NotoEmoji-Regular".to_owned()],
    );

    FontDefinitions {
        font_data,
        families,
    }
}
