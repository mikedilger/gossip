use super::feed::NoteRenderData;
use super::HighlightType;
use eframe::egui::{
    Color32, Context, FontData, FontDefinitions, FontTweak, Margin, Rounding, Stroke, Style,
    TextFormat, TextStyle, Ui,
};
use eframe::epaint::{FontFamily, FontId, Shadow};
use gossip_lib::Settings;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

mod classic;
pub use classic::ClassicTheme;

mod default;
pub use default::DefaultTheme;

mod roundy;
pub use roundy::RoundyTheme;

pub fn apply_theme(theme: &Theme, ctx: &Context) {
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
    pub follow_os_dark_mode: bool,
}

impl Theme {
    pub fn from_settings(settings: &Settings) -> Theme {
        Theme {
            variant: match &*settings.theme_variant {
                "Classic" => ThemeVariant::Classic,
                "Default" => ThemeVariant::Default,
                "Roundy" => ThemeVariant::Roundy,
                _ => ThemeVariant::Default,
            },
            dark_mode: settings.dark_mode,
            follow_os_dark_mode: settings.follow_os_dark_mode,
        }
    }
}

pub struct FeedProperties {
    /// This is a thread
    pub is_thread: bool,
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
            #[allow(dead_code)]
            pub fn name(&self) -> &'static str {
                self.variant.name()
            }

            pub fn accent_color(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::accent_color(self.dark_mode), )+
                }
            }

            #[allow(dead_code)]
            pub fn accent_complementary_color(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::accent_complementary_color(self.dark_mode), )+
                }
            }

            #[allow(dead_code)]
            pub fn highlighted_note_bgcolor(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::highlighted_note_bgcolor(self.dark_mode), )+
                }
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

            pub fn navigation_bg_fill(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::navigation_bg_fill(self.dark_mode), )+
                }
            }

            pub fn navigation_text_color(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::navigation_text_color(self.dark_mode), )+
                }
            }

            pub fn navigation_text_active_color(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::navigation_text_active_color(self.dark_mode), )+
                }
            }

            pub fn navigation_text_hover_color(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::navigation_text_hover_color(self.dark_mode), )+
                }
            }

            pub fn navigation_header_active_color(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::navigation_header_active_color(self.dark_mode), )+
                }
            }

            pub fn input_text_color(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::input_text_color(self.dark_mode), )+
                }
            }

            pub fn feed_scroll_fill(&self, feed: &FeedProperties) -> Color32 {
                match self.variant {
                    $( $variant => $class::feed_scroll_fill(self.dark_mode, feed), )+
                }
            }

            pub fn feed_scroll_stroke(&self, feed: &FeedProperties) -> Stroke {
                match self.variant {
                    $( $variant => $class::feed_scroll_stroke(self.dark_mode, feed), )+
                }
            }

            pub fn feed_scroll_rounding(&self, feed: &FeedProperties) -> Rounding {
                match self.variant {
                    $( $variant => $class::feed_scroll_rounding(feed), )+
                }
            }

            pub fn feed_post_separator_stroke(&self, post: &NoteRenderData) -> Stroke {
                match self.variant {
                    $( $variant => $class::feed_post_separator_stroke(self.dark_mode, post), )+
                }
            }

            pub fn feed_post_outer_indent(&self, ui: &mut Ui, post: &NoteRenderData) {
                match self.variant {
                    $( $variant => $class::feed_post_outer_indent(ui, post), )+
                }
            }

            pub fn feed_post_inner_indent(&self, ui: &mut Ui, post: &NoteRenderData) {
                match self.variant {
                    $( $variant => $class::feed_post_inner_indent(ui, post), )+
                }
            }

            pub fn feed_frame_inner_margin(&self, post: &NoteRenderData) -> Margin {
                match self.variant {
                    $( $variant => $class::feed_frame_inner_margin(post), )+
                }
            }

            pub fn feed_frame_outer_margin(&self, post: &NoteRenderData) -> Margin {
                match self.variant {
                    $( $variant => $class::feed_frame_outer_margin(post), )+
                }
            }

            pub fn feed_frame_rounding(&self, post: &NoteRenderData) -> Rounding {
                match self.variant {
                    $( $variant => $class::feed_frame_rounding(post), )+
                }
            }

            pub fn feed_frame_shadow(&self, post: &NoteRenderData) -> Shadow {
                match self.variant {
                    $( $variant => $class::feed_frame_shadow(self.dark_mode, post), )+
                }
            }

            pub fn feed_frame_fill(&self, post: &NoteRenderData) -> Color32 {
                match self.variant {
                    $( $variant => $class::feed_frame_fill(self.dark_mode, post), )+
                }
            }

            pub fn feed_frame_stroke(&self, post: &NoteRenderData) -> Stroke {
                match self.variant {
                    $( $variant => $class::feed_frame_stroke(self.dark_mode, post), )+
                }
            }

            pub fn repost_separator_before_stroke(&self, post: &NoteRenderData) -> Stroke {
                match self.variant {
                    $( $variant => $class::repost_separator_before_stroke(self.dark_mode, post), )+
                }
            }

            pub fn repost_space_above_separator_before(&self, post: &NoteRenderData) -> f32 {
                match self.variant {
                    $( $variant => $class::repost_space_above_separator_before(post), )+
                }
            }

            pub fn repost_space_below_separator_before(&self, post: &NoteRenderData) -> f32 {
                match self.variant {
                    $( $variant => $class::repost_space_below_separator_before(post), )+
                }
            }

            pub fn repost_separator_after_stroke(&self, post: &NoteRenderData) -> Stroke {
                match self.variant {
                    $( $variant => $class::repost_separator_after_stroke(self.dark_mode, post), )+
                }
            }

            pub fn repost_space_above_separator_after(&self, post: &NoteRenderData) -> f32 {
                match self.variant {
                    $( $variant => $class::repost_space_above_separator_after(post), )+
                }
            }

            pub fn repost_space_below_separator_after(&self, post: &NoteRenderData) -> f32 {
                match self.variant {
                    $( $variant => $class::repost_space_below_separator_after(post), )+
                }
            }

            pub fn repost_inner_margin(&self, post: &NoteRenderData) -> Margin {
                match self.variant {
                    $( $variant => $class::repost_inner_margin(post), )+
                }
            }

            pub fn repost_outer_margin(&self, post: &NoteRenderData) -> Margin {
                match self.variant {
                    $( $variant => $class::repost_outer_margin(post), )+
                }
            }

            pub fn repost_rounding(&self, post: &NoteRenderData) -> Rounding {
                match self.variant {
                    $( $variant => $class::repost_rounding(post), )+
                }
            }

            pub fn repost_shadow(&self, post: &NoteRenderData) -> Shadow {
                match self.variant {
                    $( $variant => $class::repost_shadow(self.dark_mode, post), )+
                }
            }

            pub fn repost_fill(&self, post: &NoteRenderData) -> Color32 {
                match self.variant {
                    $( $variant => $class::repost_fill(self.dark_mode, post), )+
                }
            }

            pub fn repost_stroke(&self, post: &NoteRenderData) -> Stroke {
                match self.variant {
                    $( $variant => $class::repost_stroke(self.dark_mode, post), )+
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
    // User facing name
    fn name() -> &'static str;

    // Used for strokes, lines, and text in various places
    fn accent_color(dark_mode: bool) -> Color32;

    // Used as background for highlighting unread events
    fn highlighted_note_bgcolor(dark_mode: bool) -> Color32;

    fn accent_complementary_color(dark_mode: bool) -> Color32;

    // These styles are used by egui by default for widgets if you don't override them
    // in place.
    fn get_style(dark_mode: bool) -> Style;

    fn font_definitions() -> FontDefinitions;
    fn text_styles() -> BTreeMap<TextStyle, FontId>;
    fn highlight_text_format(highlight_type: HighlightType, dark_mode: bool) -> TextFormat;
    fn warning_marker_text_color(dark_mode: bool) -> eframe::egui::Color32;
    fn notice_marker_text_color(dark_mode: bool) -> eframe::egui::Color32;

    fn navigation_bg_fill(dark_mode: bool) -> eframe::egui::Color32;
    fn navigation_text_color(dark_mode: bool) -> eframe::egui::Color32;
    fn navigation_text_active_color(dark_mode: bool) -> eframe::egui::Color32;
    fn navigation_text_hover_color(dark_mode: bool) -> eframe::egui::Color32;
    fn navigation_header_active_color(dark_mode: bool) -> eframe::egui::Color32;

    // egui by default uses inactive.fg_stroke for multiple things (buttons, any
    // labels made clickable, and TextEdit text. We try to always override TextEdit
    // text with this color instead.
    fn input_text_color(dark_mode: bool) -> eframe::egui::Color32;

    // feed styling
    fn feed_scroll_rounding(feed: &FeedProperties) -> Rounding;
    fn feed_scroll_fill(dark_mode: bool, feed: &FeedProperties) -> Color32;
    fn feed_scroll_stroke(dark_mode: bool, feed: &FeedProperties) -> Stroke;
    fn feed_post_separator_stroke(dark_mode: bool, post: &NoteRenderData) -> Stroke;
    fn feed_post_outer_indent(ui: &mut Ui, post: &NoteRenderData);
    fn feed_post_inner_indent(ui: &mut Ui, post: &NoteRenderData);
    fn feed_frame_inner_margin(post: &NoteRenderData) -> Margin;
    fn feed_frame_outer_margin(post: &NoteRenderData) -> Margin;
    fn feed_frame_rounding(post: &NoteRenderData) -> Rounding;
    fn feed_frame_shadow(dark_mode: bool, post: &NoteRenderData) -> Shadow;
    fn feed_frame_fill(dark_mode: bool, post: &NoteRenderData) -> Color32;
    fn feed_frame_stroke(dark_mode: bool, post: &NoteRenderData) -> Stroke;
    fn repost_separator_before_stroke(dark_mode: bool, post: &NoteRenderData) -> Stroke;
    fn repost_space_above_separator_before(post: &NoteRenderData) -> f32;
    fn repost_space_below_separator_before(post: &NoteRenderData) -> f32;
    fn repost_separator_after_stroke(dark_mode: bool, post: &NoteRenderData) -> Stroke;
    fn repost_space_above_separator_after(post: &NoteRenderData) -> f32;
    fn repost_space_below_separator_after(post: &NoteRenderData) -> f32;
    fn repost_inner_margin(post: &NoteRenderData) -> Margin;
    fn repost_outer_margin(post: &NoteRenderData) -> Margin;
    fn repost_rounding(post: &NoteRenderData) -> Rounding;
    fn repost_shadow(dark_mode: bool, post: &NoteRenderData) -> Shadow;
    fn repost_fill(dark_mode: bool, post: &NoteRenderData) -> Color32;
    fn repost_stroke(dark_mode: bool, post: &NoteRenderData) -> Stroke;

    // image rounding
    fn round_image() -> bool;
}

pub(super) fn font_definitions() -> FontDefinitions {
    let mut font_data: BTreeMap<String, FontData> = BTreeMap::new();
    let mut families = BTreeMap::new();

    font_data.insert(
        "DejaVuSans".to_owned(),
        FontData::from_static(include_bytes!("../../../../fonts/DejaVuSansSansEmoji.ttf")),
    );
    font_data.insert(
        "DejaVuSansBold".to_owned(),
        FontData::from_static(include_bytes!(
            "../../../../fonts/DejaVuSans-Bold-SansEmoji.ttf"
        )),
    );

    if cfg!(feature = "lang-cjk") {
        font_data.insert(
            "NotoSansCJK".to_owned(),
            FontData::from_static(include_bytes!("../../../../fonts/NotoSansCJK-Regular.ttc")),
        );
    }

    font_data.insert(
        "Inconsolata".to_owned(),
        FontData::from_static(include_bytes!("../../../../fonts/Inconsolata-Regular.ttf")).tweak(
            FontTweak {
                scale: 1.22,            // This font is smaller than DejaVuSans
                y_offset_factor: -0.18, // and too low
                y_offset: 0.0,
                baseline_offset_factor: 0.0,
            },
        ),
    );

    // Some good looking emojis. Use as first priority:
    font_data.insert(
        "NotoEmoji-Regular".to_owned(),
        FontData::from_static(include_bytes!("../../../../fonts/NotoEmoji-Regular.ttf")).tweak(
            FontTweak {
                scale: 1.1, // make them a touch larger
                y_offset_factor: 0.0,
                y_offset: 0.0,
                baseline_offset_factor: 0.0,
            },
        ),
    );

    let mut proportional = vec!["DejaVuSans".to_owned(), "NotoEmoji-Regular".to_owned()];
    if cfg!(feature = "lang-cjk") {
        proportional.push("NotoSansCJK".to_owned());
    }

    families.insert(FontFamily::Proportional, proportional);

    families.insert(
        FontFamily::Monospace,
        vec!["Inconsolata".to_owned(), "NotoEmoji-Regular".to_owned()],
    );

    families.insert(
        FontFamily::Name("Bold".into()),
        vec!["DejaVuSansBold".to_owned()],
    );

    FontDefinitions {
        font_data,
        families,
    }
}
