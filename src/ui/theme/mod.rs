use super::HighlightType;
use eframe::egui::{
    Color32, Context, FontData, FontDefinitions, FontTweak, Margin, Rounding, Stroke, Style,
    TextFormat, TextStyle,
};
use eframe::epaint::{FontFamily, FontId, Shadow};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

mod gossip;
pub use gossip::GossipTheme;

mod roundy;
pub use roundy::RoundyTheme;

pub fn apply_theme(theme: Theme, dark_mode: bool, ctx: &Context) {
    ctx.set_style(theme.get_style(dark_mode));
    ctx.set_fonts(theme.font_definitions());
    let mut style: eframe::egui::Style = (*ctx.style()).clone();
    style.text_styles = theme.text_styles();
    ctx.set_style(style);
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum Theme {
    GossipDefault,
    Roundy,
}

// FIXME: This enum dispatch into the ThemeDef trait objects is predictable enough
//        that we could probably macro it.
impl Theme {
    pub fn all() -> &'static [Theme] {
        &[Theme::GossipDefault, Theme::Roundy]
    }

    pub fn name(&self) -> &'static str {
        match *self {
            Theme::GossipDefault => GossipTheme::name(),
            Theme::Roundy => RoundyTheme::name(),
        }
    }

    pub fn get_style(&self, dark_mode: bool) -> Style {
        match *self {
            Theme::GossipDefault => GossipTheme::get_style(dark_mode),
            Theme::Roundy => RoundyTheme::get_style(dark_mode),
        }
    }

    pub fn font_definitions(&self) -> FontDefinitions {
        match *self {
            Theme::GossipDefault => GossipTheme::font_definitions(),
            Theme::Roundy => RoundyTheme::font_definitions(),
        }
    }

    pub fn text_styles(&self) -> BTreeMap<TextStyle, FontId> {
        match *self {
            Theme::GossipDefault => GossipTheme::text_styles(),
            Theme::Roundy => RoundyTheme::text_styles(),
        }
    }
    pub fn highlight_text_format(
        &self,
        highlight_type: HighlightType,
        dark_mode: bool,
    ) -> TextFormat {
        match *self {
            Theme::GossipDefault => GossipTheme::highlight_text_format(highlight_type, dark_mode),
            Theme::Roundy => RoundyTheme::highlight_text_format(highlight_type, dark_mode),
        }
    }

    pub fn feed_scroll_fill(&self, dark_mode: bool) -> Color32 {
        match *self {
            Theme::GossipDefault => GossipTheme::feed_scroll_fill(dark_mode),
            Theme::Roundy => RoundyTheme::feed_scroll_fill(dark_mode),
        }
    }

    pub fn feed_post_separator_stroke(&self, dark_mode: bool) -> Stroke {
        match *self {
            Theme::GossipDefault => GossipTheme::feed_post_separator_stroke(dark_mode),
            Theme::Roundy => RoundyTheme::feed_post_separator_stroke(dark_mode),
        }
    }

    pub fn feed_frame_inner_margin(&self) -> Margin {
        match *self {
            Theme::GossipDefault => GossipTheme::feed_frame_inner_margin(),
            Theme::Roundy => RoundyTheme::feed_frame_inner_margin(),
        }
    }

    pub fn feed_frame_outer_margin(&self) -> Margin {
        match *self {
            Theme::GossipDefault => GossipTheme::feed_frame_outer_margin(),
            Theme::Roundy => RoundyTheme::feed_frame_outer_margin(),
        }
    }

    pub fn feed_frame_rounding(&self) -> Rounding {
        match *self {
            Theme::GossipDefault => GossipTheme::feed_frame_rounding(),
            Theme::Roundy => RoundyTheme::feed_frame_rounding(),
        }
    }

    pub fn feed_frame_shadow(&self, dark_mode: bool) -> Shadow {
        match *self {
            Theme::GossipDefault => GossipTheme::feed_frame_shadow(dark_mode),
            Theme::Roundy => RoundyTheme::feed_frame_shadow(dark_mode),
        }
    }

    pub fn feed_frame_fill(&self, is_new: bool, is_main_event: bool, dark_mode: bool) -> Color32 {
        match *self {
            Theme::GossipDefault => GossipTheme::feed_frame_fill(is_new, is_main_event, dark_mode),
            Theme::Roundy => RoundyTheme::feed_frame_fill(is_new, is_main_event, dark_mode),
        }
    }

    pub fn feed_frame_stroke(&self, is_new: bool, is_main_event: bool, dark_mode: bool) -> Stroke {
        match *self {
            Theme::GossipDefault => {
                GossipTheme::feed_frame_stroke(is_new, is_main_event, dark_mode)
            }
            Theme::Roundy => RoundyTheme::feed_frame_stroke(is_new, is_main_event, dark_mode),
        }
    }
}

pub trait ThemeDef: Send + Sync {
    // user facing name
    fn name() -> &'static str;

    // general styling
    fn get_style(dark_mode: bool) -> Style;
    fn font_definitions() -> FontDefinitions;
    fn text_styles() -> BTreeMap<TextStyle, FontId>;
    fn highlight_text_format(highlight_type: HighlightType, dark_mode: bool) -> TextFormat;

    // feed styling
    fn feed_scroll_fill(dark_mode: bool) -> Color32;
    fn feed_post_separator_stroke(dark_mode: bool) -> Stroke;
    fn feed_frame_inner_margin() -> Margin;
    fn feed_frame_outer_margin() -> Margin;
    fn feed_frame_rounding() -> Rounding;
    fn feed_frame_shadow(dark_mode: bool) -> Shadow;
    fn feed_frame_fill(is_new: bool, is_main_event: bool, dark_mode: bool) -> Color32;
    fn feed_frame_stroke(is_new: bool, is_main_event: bool, dark_mode: bool) -> Stroke;
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
