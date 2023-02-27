use eframe::egui::{TextStyle, TextFormat, FontTweak, FontData, FontDefinitions};
use eframe::epaint::{FontId, FontFamily};
use std::collections::BTreeMap;
use lazy_static::lazy_static;
use std::sync::{ Mutex, Arc, RwLock };

use crate::tags::HighlightType;

mod gossip;
mod roundy;

pub(crate) trait Theme : Send + Sync {
    // user facing name
    fn name(&self) -> &'static str;

    // our non Size derived clone
    fn make_copy(&self) -> Arc<dyn Theme>;

    // general styling
    fn dark_mode(&mut self, dark_mode: bool);
    fn is_dark_mode(&self) -> bool;
    fn get_style(&self) -> eframe::egui::Style;
    fn font_definitions(&self) -> FontDefinitions;
    fn text_styles(&self) -> BTreeMap<TextStyle, FontId>;
    fn highlight_text_format(&self, highlight_type: HighlightType) -> TextFormat;

    // feed styling
    fn feed_scroll_fill(&self) -> eframe::egui::Color32;
    fn feed_post_separator_stroke(&self) -> eframe::egui::Stroke;
    fn feed_frame_inner_margin(&self) -> eframe::egui::Margin;
    fn feed_frame_outer_margin(&self) ->  eframe::egui::Margin;
    fn feed_frame_rounding(&self) ->  eframe::egui::Rounding;
    fn feed_frame_shadow(&self) ->  eframe::epaint::Shadow;
    fn feed_frame_fill(&self, is_new: bool, is_main_event: bool) ->  eframe::egui::Color32;
    fn feed_frame_stroke(&self, is_new: bool, is_main_event: bool) ->  eframe::egui::Stroke;
}

struct ThemePicker {
    gossip_theme: Arc<RwLock<gossip::Gossip>>,
    roundy: Arc<RwLock<roundy::Roundy>>,
    current_theme: Arc<RwLock<dyn Theme>>,
}

impl ThemePicker {
    fn list_themes(&self) -> Vec<&'static str> {
        vec![
            self.gossip_theme.read().unwrap().name(),
            self.roundy.read().unwrap().name(),
        ]
    }
    fn set_dark_mode(&mut self, dark_mode: bool) {
        self.current_theme.write().unwrap().dark_mode(dark_mode);
    }
    fn pick(&mut self, name: &Option<String> ) {
        match name {
            Some(name) if name.as_str() == self.roundy.read().unwrap().name() => {
                self.current_theme = self.roundy.clone();
            },
            _ => {
                self.current_theme = self.gossip_theme.clone();
            },
        }
    }
    fn current_theme(&self) -> Arc<dyn Theme> {
        return self.current_theme.read().unwrap().make_copy()
    }
}

impl Default for ThemePicker {
    fn default() -> Self {
        let gossip_theme_rc = Arc::new( RwLock::new( gossip::Gossip::default() ));
        let me = ThemePicker {
            gossip_theme: gossip_theme_rc.clone(),
            roundy: Arc::new( RwLock::new( roundy::Roundy::default() )),
            current_theme: gossip_theme_rc,
        };
        return me
    }
}

lazy_static! {
    static ref CURRENT_THEME_INSTANCE: Mutex<ThemePicker> = Mutex::new( ThemePicker::default() );
}

pub(crate) fn list_themes() -> Vec<&'static str> {
    CURRENT_THEME_INSTANCE.lock().unwrap().list_themes()
}

/// Set dark_mode enabled or disabled
pub(crate) fn set_dark_mode( dark_mode: bool, egui_ctx: &eframe::egui::Context ) {
    let mut theme_picker = CURRENT_THEME_INSTANCE.lock().unwrap();
    theme_picker.set_dark_mode( dark_mode );

    let theme = theme_picker.current_theme();

    egui_ctx.set_style(theme.get_style());
    egui_ctx.set_fonts(theme.font_definitions());
    let mut style: eframe::egui::Style = (*egui_ctx.style()).clone();
    style.text_styles = theme.text_styles();
    egui_ctx.set_style(style);
}

/// Switch between Themes preserving dark_mode selection
pub(crate) fn switch( name: &Option<String>, egui_ctx: &eframe::egui::Context ) {
    let current_mode = CURRENT_THEME_INSTANCE.lock().unwrap().current_theme().is_dark_mode();
    CURRENT_THEME_INSTANCE.lock().unwrap().pick( name );
    set_dark_mode(current_mode, egui_ctx);
}

/// Return an Arc to the currently selected Theme
pub(crate) fn current_theme() -> Arc<dyn Theme> {
    CURRENT_THEME_INSTANCE.lock().unwrap().current_theme()
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
