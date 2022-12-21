use eframe::{egui, epaint};
use egui::style::{Selection, Visuals, Widgets};
use egui::{
    Color32, FontData, FontDefinitions, FontFamily, FontId, FontTweak, Rounding, Stroke, TextStyle,
};
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

pub(super) fn text_styles() -> BTreeMap<TextStyle, FontId> {
    let mut text_styles: BTreeMap<TextStyle, FontId> = BTreeMap::new();

    text_styles.insert(
        TextStyle::Small,
        FontId {
            size: 12.0,
            family: FontFamily::Proportional,
        },
    );

    text_styles.insert(
        TextStyle::Body,
        FontId {
            size: 14.0,
            family: FontFamily::Proportional,
        },
    );

    text_styles.insert(
        TextStyle::Monospace,
        FontId {
            size: 14.0,
            family: FontFamily::Monospace,
        },
    );

    text_styles.insert(
        TextStyle::Button,
        FontId {
            size: 15.0,
            family: FontFamily::Proportional,
        },
    );

    text_styles.insert(
        TextStyle::Heading,
        FontId {
            size: 16.0,
            family: FontFamily::Name("BoldOblique".into()),
        },
    );

    text_styles.insert(
        TextStyle::Name("Bold".into()),
        FontId {
            size: 14.0,
            family: FontFamily::Name("Bold".into()),
        },
    );

    text_styles.insert(
        TextStyle::Name("Oblique".into()),
        FontId {
            size: 14.0,
            family: FontFamily::Name("Oblique".into()),
        },
    );

    text_styles.insert(
        TextStyle::Name("MonoBold".into()),
        FontId {
            size: 14.0,
            family: FontFamily::Name("MonoBold".into()),
        },
    );

    text_styles.insert(
        TextStyle::Name("MonoOblique".into()),
        FontId {
            size: 14.0,
            family: FontFamily::Name("MonoOblique".into()),
        },
    );

    text_styles.insert(
        TextStyle::Name("MonoBoldOblique".into()),
        FontId {
            size: 14.0,
            family: FontFamily::Name("MonoBoldOblique".into()),
        },
    );

    text_styles
}

/*
 * We configure their font families
 *    Proportional
 *    Monospace
 * We define the following Font Families:
 *    Bold,
 *    Oblique,
 *    BoldOblique
 *    MonoBold,
 *    MonoOblique
 *    MonoBoldOblique
 */
pub(super) fn font_definitions() -> FontDefinitions {
    let mut font_data: BTreeMap<String, FontData> = BTreeMap::new();
    let mut families = BTreeMap::new();

    // Good Looking Emojis
    font_data.insert(
        "NotoColorEmoji".to_owned(),
        FontData::from_static(include_bytes!("../../fonts/noto/NotoColorEmoji.ttf")).tweak(
            FontTweak {
                scale: 0.81,           // make it smaller
                y_offset_factor: -0.2, // move it up
                y_offset: 0.0,
            },
        ),
    );

    // Proportional Regular

    font_data.insert(
        "DejaVuSansRegular".to_owned(),
        FontData::from_static(include_bytes!("../../fonts/DejaVuSans/DejaVuSans.ttf")),
    );

    font_data.insert(
        "NotoSansRegular".to_owned(),
        FontData::from_static(include_bytes!("../../fonts/noto/NotoSans-Regular.ttf")),
    );

    families.insert(
        FontFamily::Proportional,
        vec![
            "DejaVuSansRegular".to_owned(),
            "NotoColorEmoji".to_owned(),
            "NotoSansRegular".to_owned(),
        ],
    );

    // Proportional Bold

    font_data.insert(
        "DejaVuSansBold".to_owned(),
        FontData::from_static(include_bytes!("../../fonts/DejaVuSans/DejaVuSans-Bold.ttf")),
    );

    font_data.insert(
        "NotoSansBold".to_owned(),
        FontData::from_static(include_bytes!("../../fonts/noto/NotoSans-Bold.ttf")),
    );

    families.insert(
        FontFamily::Name("Bold".into()),
        vec![
            "DejaVuSansBold".to_owned(),
            "NotoColorEmoji".to_owned(),
            "NotoSansBold".to_owned(),
        ],
    );

    // Proportional Oblique

    font_data.insert(
        "DejaVuSansOblique".to_owned(),
        FontData::from_static(include_bytes!(
            "../../fonts/DejaVuSans/DejaVuSans-Oblique.ttf"
        )),
    );

    font_data.insert(
        "NotoSansOblique".to_owned(),
        FontData::from_static(include_bytes!("../../fonts/noto/NotoSans-Italic.ttf")),
    );

    families.insert(
        FontFamily::Name("Oblique".into()),
        vec![
            "DejaVuSansOblique".to_owned(),
            "NotoColorEmoji".to_owned(),
            "NotoSansOblique".to_owned(),
        ],
    );

    // Proportional Bold Oblique

    font_data.insert(
        "DejaVuSansBoldOblique".to_owned(),
        FontData::from_static(include_bytes!(
            "../../fonts/DejaVuSans/DejaVuSans-BoldOblique.ttf"
        )),
    );

    font_data.insert(
        "NotoSansBoldOblique".to_owned(),
        FontData::from_static(include_bytes!("../../fonts/noto/NotoSans-BoldItalic.ttf")),
    );

    families.insert(
        FontFamily::Name("BoldOblique".into()),
        vec![
            "DejaVuSansBoldOblique".to_owned(),
            "NotoColorEmoji".to_owned(),
            "NotoSansBoldOblique".to_owned(),
        ],
    );

    // Monospace Regular

    font_data.insert(
        "InconsolataRegular".to_owned(),
        FontData::from_static(include_bytes!(
            "../../fonts/inconsolata/Inconsolata-SemiCondensedLight.ttf"
        ))
        .tweak(FontTweak {
            scale: 1.1, // Make it bigger. Inconsolata is smaller than DejaVu.
            y_offset_factor: 0.0,
            y_offset: 0.0,
        }),
    );

    font_data.insert(
        "DejaVuSansMonoRegular".to_owned(),
        FontData::from_static(include_bytes!("../../fonts/DejaVuSans/DejaVuSansMono.ttf")),
    );

    font_data.insert(
        "NotoSansMonoRegular".to_owned(),
        FontData::from_static(include_bytes!("../../fonts/noto/NotoSansMono-Regular.ttf")),
    );

    families.insert(
        FontFamily::Monospace,
        vec![
            "InconsolataRegular".to_owned(),
            "DejaVuSansMonoRegular".to_owned(),
            "NotoColorEmoji".to_owned(),
            "NotoSansMonoRegular".to_owned(),
        ],
    );

    // Monospace Bold

    font_data.insert(
        "InconsolataBold".to_owned(),
        FontData::from_static(include_bytes!(
            "../../fonts/inconsolata/Inconsolata-SemiCondensedSemiBold.ttf"
        ))
        .tweak(FontTweak {
            scale: 1.1, // Make it bigger. Inconsolata is smaller than DejaVu.
            y_offset_factor: 0.0,
            y_offset: 0.0,
        }),
    );

    font_data.insert(
        "DejaVuSansMonoBold".to_owned(),
        FontData::from_static(include_bytes!(
            "../../fonts/DejaVuSans/DejaVuSansMono-Bold.ttf"
        )),
    );

    font_data.insert(
        "NotoSansMonoBold".to_owned(),
        FontData::from_static(include_bytes!("../../fonts/noto/NotoSansMono-Bold.ttf")),
    );

    families.insert(
        FontFamily::Name("MonoBold".into()),
        vec![
            "InconsolataBold".to_owned(),
            "DejaVuSansMonoBold".to_owned(),
            "NotoColorEmoji".to_owned(),
            "NotoSansMonoBold".to_owned(),
        ],
    );

    // Monospace Oblique

    font_data.insert(
        "DejaVuSansMonoOblique".to_owned(),
        FontData::from_static(include_bytes!(
            "../../fonts/DejaVuSans/DejaVuSansMono-Oblique.ttf"
        )),
    );

    families.insert(
        FontFamily::Name("MonoOblique".into()),
        vec![
            "DejaVuSansMonoOblique".to_owned(),
            "NotoColorEmoji".to_owned(),
            "NotoSansMonoRegular".to_owned(), // they don't have an oblique
        ],
    );

    // Monospace Bold Oblique

    font_data.insert(
        "DejaVuSansMonoBoldOblique".to_owned(),
        FontData::from_static(include_bytes!(
            "../../fonts/DejaVuSans/DejaVuSansMono-BoldOblique.ttf"
        )),
    );

    families.insert(
        FontFamily::Name("MonoBoldOblique".into()),
        vec![
            "DejaVuSansMonoBoldOblique".to_owned(),
            "NotoColorEmoji".to_owned(),
            "NotoSansMonoBold".to_owned(), // they don't have a bold oblique
        ],
    );

    FontDefinitions {
        font_data,
        families,
    }
}
