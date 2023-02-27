use eframe::egui::{Style, TextStyle, TextFormat, FontTweak, FontData, FontDefinitions};
use eframe::epaint::{FontId, FontFamily};
use std::collections::BTreeMap;

use crate::tags::HighlightType;

mod gossip;

pub(crate) trait Theme {
    fn light_mode(&self) -> Style;
    fn dark_mode(&self) -> Style;
    fn font_definitions(&self) -> FontDefinitions;
    fn text_styles(&self) -> BTreeMap<TextStyle, FontId>;
    fn highlight_text_format(&self, highlight_type: HighlightType, dark_mode: bool) -> TextFormat;
}

pub(crate) fn current_theme() -> Box<dyn Theme> {
    Box::new( gossip::Gossip {} )
}

pub(super) fn font_definitions() -> FontDefinitions {
    let mut font_data: BTreeMap<String, FontData> = BTreeMap::new();
    let mut families = BTreeMap::new();

    font_data.insert(
        "DejaVuSans".to_owned(),
        FontData::from_static(include_bytes!("../../../fonts/DejaVuSansSansEmoji.ttf")),
    );

    font_data.insert(
        "Inconsolata".to_owned(),
        FontData::from_static(include_bytes!("../../../fonts/Inconsolata-Regular.ttf")).tweak(
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
        FontData::from_static(include_bytes!("../../../fonts/NotoEmoji-Regular.ttf")).tweak(
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
