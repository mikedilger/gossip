use super::feed::NoteRenderData;
use super::HighlightType;
use eframe::egui::{
    Color32, Context, FontData, FontDefinitions, FontTweak, Margin, Rounding, Stroke, Style,
    TextFormat, TextStyle, Ui,
};
use eframe::epaint::{ecolor, FontFamily, FontId, Shadow};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

mod default;
pub use default::DefaultTheme;

pub(super) mod test_page;

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
    Default,
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct Theme {
    pub variant: ThemeVariant,
    pub dark_mode: bool,
    pub follow_os_dark_mode: bool,
}

impl Theme {
    pub fn from_settings() -> Theme {
        Theme {
            variant: match &*read_setting!(theme_variant) {
                "Default" => ThemeVariant::Default,
                _ => ThemeVariant::Default,
            },
            dark_mode: read_setting!(dark_mode),
            follow_os_dark_mode: read_setting!(follow_os_dark_mode),
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

            // Palette
            #[allow(dead_code)]
            pub fn neutral_50(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::neutral_50(), )+
                }
            }

            #[allow(dead_code)]
            pub fn neutral_100(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::neutral_100(), )+
                }
            }

            #[allow(dead_code)]
            pub fn neutral_200(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::neutral_200(), )+
                }
            }

            #[allow(dead_code)]
            pub fn neutral_300(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::neutral_300(), )+
                }
            }

            #[allow(dead_code)]
            pub fn neutral_400(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::neutral_400(), )+
                }
            }

            #[allow(dead_code)]
            pub fn neutral_500(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::neutral_500(), )+
                }
            }

            #[allow(dead_code)]
            pub fn neutral_600(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::neutral_600(), )+
                }
            }

            #[allow(dead_code)]
            pub fn neutral_700(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::neutral_700(), )+
                }
            }

            #[allow(dead_code)]
            pub fn neutral_800(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::neutral_800(), )+
                }
            }

            #[allow(dead_code)]
            pub fn neutral_900(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::neutral_900(), )+
                }
            }

            #[allow(dead_code)]
            pub fn neutral_950(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::neutral_950(), )+
                }
            }

            #[allow(dead_code)]
            pub fn accent_dark(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::accent_dark(), )+
                }
            }

            #[allow(dead_code)]
            pub fn accent_dark_b20(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::accent_dark_b20(), )+
                }
            }

            #[allow(dead_code)]
            pub fn accent_dark_w20(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::accent_dark_w20(), )+
                }
            }

            #[allow(dead_code)]
            pub fn accent_light(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::accent_light(), )+
                }
            }

            #[allow(dead_code)]
            pub fn accent_light_b20(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::accent_light_b20(), )+
                }
            }

            #[allow(dead_code)]
            pub fn accent_light_w20(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::accent_light_w20(), )+
                }
            }

            #[allow(dead_code)]
            pub fn red_500(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::red_500(), )+
                }
            }

            #[allow(dead_code)]
            pub fn lime_500(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::lime_500(), )+
                }
            }

            #[allow(dead_code)]
            pub fn amber_400(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::amber_400(), )+
                }
            }

            #[allow(dead_code)]
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
            pub fn danger_color(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::danger_color(self.dark_mode), )+
                }
            }

            pub fn main_content_bgcolor(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::main_content_bgcolor(self.dark_mode), )+
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

            pub fn on_accent_style(&self, style: &mut Style) {
                match self.variant {
                    $( $variant => $class::on_accent_style(style, self.dark_mode), )+
                }
            }

            /// primary button style
            pub fn primary_button_style(&self, style: &mut Style) {
                match self.variant {
                    $( $variant => $class::primary_button_style(style, self.dark_mode), )+
                }
            }

            /// 'danger' colored hover for accent-colored button styles
            pub fn accent_button_danger_hover(&self, style: &mut Style) {
                match self.variant {
                    $( $variant => $class::accent_button_danger_hover(style, self.dark_mode), )+
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

            pub fn navigation_text_deactivated_color(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::navigation_text_deactivated_color(self.dark_mode), )+
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

            #[allow(dead_code)]
            pub fn input_bg_color(&self) -> Color32 {
                match self.variant {
                    $( $variant => $class::input_bg_color(self.dark_mode), )+
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

theme_dispatch!(ThemeVariant::Default, DefaultTheme, "Default");

pub trait ThemeDef: Send + Sync {
    // User facing name
    fn name() -> &'static str;

    // Palette
    fn neutral_50() -> Color32;
    fn neutral_100() -> Color32;
    fn neutral_200() -> Color32;
    fn neutral_300() -> Color32;
    fn neutral_400() -> Color32;
    fn neutral_500() -> Color32;
    fn neutral_600() -> Color32;
    fn neutral_700() -> Color32;
    fn neutral_800() -> Color32;
    fn neutral_900() -> Color32;
    fn neutral_950() -> Color32;
    fn accent_dark() -> Color32;
    fn accent_dark_b20() -> Color32; // overlay 20% black
    fn accent_dark_w20() -> Color32; // overlay 20% white
    fn accent_light() -> Color32;
    fn accent_light_b20() -> Color32; // overlay 20% black
    fn accent_light_w20() -> Color32; // overlay 20% white
    fn red_500() -> Color32;
    fn lime_500() -> Color32;
    fn amber_400() -> Color32;

    // Used for strokes, lines, and text in various places
    fn accent_color(dark_mode: bool) -> Color32;

    fn accent_complementary_color(dark_mode: bool) -> Color32;

    fn danger_color(dark_mode: bool) -> Color32;

    fn main_content_bgcolor(dark_mode: bool) -> Color32;

    // Used as background for highlighting unread events
    fn highlighted_note_bgcolor(dark_mode: bool) -> Color32;

    // These styles are used by egui by default for widgets if you don't override them
    // in place.
    fn get_style(dark_mode: bool) -> Style;
    /// the style to use when displaying on-top of an accent-colored background
    fn on_accent_style(style: &mut Style, dark_mode: bool);

    // button styles
    fn primary_button_style(style: &mut Style, dark_mode: bool);
    fn secondary_button_style(style: &mut Style, dark_mode: bool);
    fn bordered_button_style(style: &mut Style, dark_mode: bool);

    /// 'danger' colored hover for accent-colored button styles
    fn accent_button_danger_hover(style: &mut Style, dark_mode: bool);

    fn font_definitions() -> FontDefinitions;
    fn text_styles() -> BTreeMap<TextStyle, FontId>;
    fn highlight_text_format(highlight_type: HighlightType, dark_mode: bool) -> TextFormat;
    fn warning_marker_text_color(dark_mode: bool) -> eframe::egui::Color32;
    fn notice_marker_text_color(dark_mode: bool) -> eframe::egui::Color32;

    fn navigation_bg_fill(dark_mode: bool) -> eframe::egui::Color32;
    fn navigation_text_deactivated_color(dark_mode: bool) -> eframe::egui::Color32;
    fn navigation_text_color(dark_mode: bool) -> eframe::egui::Color32;
    fn navigation_text_active_color(dark_mode: bool) -> eframe::egui::Color32;
    fn navigation_text_hover_color(dark_mode: bool) -> eframe::egui::Color32;
    fn navigation_header_active_color(dark_mode: bool) -> eframe::egui::Color32;

    // egui by default uses inactive.fg_stroke for multiple things (buttons, any
    // labels made clickable, and TextEdit text. We try to always override TextEdit
    // text with this color instead.
    fn input_text_color(dark_mode: bool) -> eframe::egui::Color32;
    fn input_bg_color(dark_mode: bool) -> eframe::egui::Color32;

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

    fn darken_color(color: Color32, factor: f32) -> Color32 {
        let mut hsva: ecolor::HsvaGamma = color.into();
        let original_value = hsva.v;
        hsva.v = original_value * (1.0 - factor); // Linear interpolation
        hsva.into()
    }
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
            #[cfg(not(target_os = "macos"))]
            FontTweak {
                scale: 1.22,            // This font is smaller than DejaVuSans
                y_offset_factor: -0.18, // and too low
                y_offset: 0.0,
                baseline_offset_factor: 0.0,
            },
            #[cfg(target_os = "macos")]
            FontTweak {
                scale: 1.22,            // This font is smaller than DejaVuSans
                y_offset_factor: -0.05, // and too low
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
