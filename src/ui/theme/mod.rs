use super::HighlightType;
use eframe::egui::{
    Color32, Context, FontData, FontDefinitions, FontTweak, Margin, Rounding, Stroke, Style,
    TextFormat, TextStyle,
};
use eframe::epaint::{FontFamily, FontId, Shadow};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

mod classic;
pub use classic::ClassicTheme;

mod default;
pub use default::DefaultTheme;

mod roundy;
pub use roundy::RoundyTheme;

pub fn apply_theme(theme: Theme, ctx: &Context) {
    ctx.set_style(theme.get_style());
    ctx.set_fonts(theme.font_definitions());
    let mut style: eframe::egui::Style = (*ctx.style()).clone();
    style.text_styles = theme.text_styles();
    ctx.set_style(style);
}

// note: if we store anything inside the variants, we can't use macro_rules.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeVariant {
    Classic,
    Default,
    Roundy,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct Theme {
    pub variant: ThemeVariant,
    pub dark_mode: bool,
}

macro_rules! theme_dispatch {
    ($($variant:path, $class:ident, $name:literal),+) => {

        impl ThemeVariant {
            pub fn name(&self) -> &'static str {
                match *self {
                    $( $variant => $name, )+
                }
            }

            pub fn all() -> &'static [ThemeVariant] {
                &[
                    $( $variant, )+
                ]
            }
        }

        impl Theme {
            pub fn name(&self) -> &'static str {
                self.variant.name()
            }

            pub fn get_style(&self) -> Style {
                match self.variant {
                    $( $variant => $class::get_style(self.dark_mode), )+
                }
            }

            pub fn font_definitions(&self) -> FontDefinitions {
                match self.variant {
                    $( $variant => $class::font_definitions(), )+
                }
            }

            pub fn text_styles(&self) -> BTreeMap<TextStyle, FontId> {
                match self.variant {
                    $( $variant => $class::text_styles(), )+
                }
            }

            pub fn highlight_text_format(
                &self,
                highlight_type: HighlightType,
            ) -> TextFormat {
                match self.variant {
                    $( $variant => $class::highlight_text_format(highlight_type, self.dark_mode), )+
                }
            }

            pub fn warning_marker_text_color(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::warning_marker_text_color(self.dark_mode), )+
                }
            }

            pub fn notice_marker_text_color(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::notice_marker_text_color(self.dark_mode), )+
                }
            }

            pub fn feed_scroll_fill(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::feed_scroll_fill(self.dark_mode), )+
                }
            }

            pub fn feed_post_separator_stroke(&self) -> Stroke {
                match self.variant {
                    $( $variant => $class::feed_post_separator_stroke(self.dark_mode), )+
                }
            }

            pub fn feed_frame_inner_margin(&self) -> Margin {
                match self.variant {
                    $( $variant => $class::feed_frame_inner_margin(), )+
                }
            }

            pub fn feed_frame_outer_margin(&self) -> Margin {
                match self.variant {
                    $( $variant => $class::feed_frame_outer_margin(), )+
                }
            }

            pub fn feed_frame_rounding(&self) -> Rounding {
                match self.variant {
                    $( $variant => $class::feed_frame_rounding(), )+
                }
            }

            pub fn feed_frame_shadow(&self) -> Shadow {
                match self.variant {
                    $( $variant => $class::feed_frame_shadow(self.dark_mode), )+
                }
            }

            pub fn feed_frame_fill(&self, is_new: bool, is_main_event: bool) -> Color32 {
                match self.variant {
                    $( $variant => $class::feed_frame_fill(is_new, is_main_event, self.dark_mode), )+
                }
            }

            pub fn feed_frame_stroke(&self, is_new: bool, is_main_event: bool) -> Stroke {
                match self.variant {
                    $( $variant => $class::feed_frame_stroke(is_new, is_main_event, self.dark_mode), )+
                }
            }

            pub fn round_image(&self) -> bool {
                match self.variant {
                    $( $variant => $class::round_image(), )+
                }
            }
        }
    }
}

theme_dispatch!(
    ThemeVariant::Classic,
    ClassicTheme,
    "Classic",
    ThemeVariant::Default,
    DefaultTheme,
    "Default",
    ThemeVariant::Roundy,
    RoundyTheme,
    "Roundy"
);

pub trait ThemeDef: Send + Sync {
    // user facing name
    fn name() -> &'static str;

    // general styling
    fn get_style(dark_mode: bool) -> Style;
    fn font_definitions() -> FontDefinitions;
    fn text_styles() -> BTreeMap<TextStyle, FontId>;
    fn highlight_text_format(highlight_type: HighlightType, dark_mode: bool) -> TextFormat;
    fn warning_marker_text_color(dark_mode: bool) -> eframe::egui::Color32;
    fn notice_marker_text_color(dark_mode: bool) -> eframe::egui::Color32;

    // feed styling
    fn feed_scroll_fill(dark_mode: bool) -> Color32;
    fn feed_post_separator_stroke(dark_mode: bool) -> Stroke;
    fn feed_frame_inner_margin() -> Margin;
    fn feed_frame_outer_margin() -> Margin;
    fn feed_frame_rounding() -> Rounding;
    fn feed_frame_shadow(dark_mode: bool) -> Shadow;
    fn feed_frame_fill(is_new: bool, is_main_event: bool, dark_mode: bool) -> Color32;
    fn feed_frame_stroke(is_new: bool, is_main_event: bool, dark_mode: bool) -> Stroke;

    // image rounding
    fn round_image() -> bool;
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
