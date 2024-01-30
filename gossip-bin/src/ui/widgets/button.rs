use egui_winit::egui::{self, WidgetText, Widget, Response, Ui};

use super::super::Theme;

enum ButtonType {
    Primary,
    Secondary
}

/// Clickable button with text
#[must_use = "You should put this widget in an ui with `ui.add(widget);`"]
pub struct Button<'a> {
    button_type: ButtonType,
    theme: &'a Theme,
    text: Option<WidgetText>,
}

impl<'a> Button<'a>{
    pub fn primary(theme: &'a Theme, text: impl Into<WidgetText>) -> Self {
        Self {
            button_type: ButtonType::Primary,
            theme,
            text: Some(text.into()),
        }
    }

    pub fn secondary(theme: &'a Theme, text: impl Into<WidgetText>) -> Self {
        Self {
            button_type: ButtonType::Secondary,
            theme,
            text: Some(text.into()),
        }
    }
}

impl Widget for Button<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        match self.button_type {
            ButtonType::Primary => self.theme.accent_button_1_style(ui.style_mut()),
            ButtonType::Secondary => self.theme.accent_button_2_style(ui.style_mut()),
        }
        let button = egui::Button::opt_image_and_text(None, self.text);
        button.ui(ui)
    }
}
