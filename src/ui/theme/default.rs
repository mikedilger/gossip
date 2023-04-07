use super::{FeedProperties, NoteRenderData, ThemeDef};
use crate::ui::HighlightType;
use eframe::egui::style::{Selection, WidgetVisuals, Widgets};
use eframe::egui::{
    FontDefinitions, Margin, Pos2, RichText, Shape, Stroke, Style, TextFormat, TextStyle, Visuals,
};
use eframe::epaint::{ecolor, Color32, FontFamily, FontId, Rounding, Shadow};
use std::collections::BTreeMap;

#[derive(Default)]
pub struct DefaultTheme {}

impl ThemeDef for DefaultTheme {
    fn name() -> &'static str {
        "Default"
    }

    fn get_style(dark_mode: bool) -> Style {
        let mut style = Style::default();

        // /// `item_spacing` is inserted _after_ adding a widget, so to increase the spacing between
        // /// widgets `A` and `B` you need to change `item_spacing` before adding `A`.
        // pub item_spacing: Vec2,

        // /// Horizontal and vertical margins within a window frame.
        // pub window_margin: Margin,

        // /// Button size is text size plus this on each side
        // pub button_padding: Vec2,

        // /// Horizontal and vertical margins within a menu frame.
        // pub menu_margin: Margin,

        // /// Indent collapsing regions etc by this much.
        // pub indent: f32,

        // /// Minimum size of a [`DragValue`], color picker button, and other small widgets.
        // /// `interact_size.y` is the default height of button, slider, etc.
        // /// Anything clickable should be (at least) this size.
        // pub interact_size: Vec2, // TODO(emilk): rename min_interact_size ?

        // /// Default width of a [`Slider`].
        // pub slider_width: f32,

        // /// Default (minimum) width of a [`ComboBox`](crate::ComboBox).
        // pub combo_width: f32,

        // /// Default width of a [`TextEdit`].
        // pub text_edit_width: f32,

        // /// Checkboxes, radio button and collapsing headers have an icon at the start.
        // /// This is the width/height of the outer part of this icon (e.g. the BOX of the checkbox).
        // pub icon_width: f32,

        // /// Checkboxes, radio button and collapsing headers have an icon at the start.
        // /// This is the width/height of the inner part of this icon (e.g. the check of the checkbox).
        // pub icon_width_inner: f32,

        // /// Checkboxes, radio button and collapsing headers have an icon at the start.
        // /// This is the spacing between the icon and the text
        // pub icon_spacing: f32,

        // /// Width of a tooltip (`on_hover_ui`, `on_hover_text` etc).
        // pub tooltip_width: f32,

        // /// End indented regions with a horizontal line
        // pub indent_ends_with_horizontal_line: bool,

        // /// Height of a combo-box before showing scroll bars.
        // pub combo_height: f32,

        // pub scroll_bar_width: f32,

        // /// Make sure the scroll handle is at least this big
        // pub scroll_handle_min_length: f32,

        // /// Margin between contents and scroll bar.
        // pub scroll_bar_inner_margin: f32,

        // /// Margin between scroll bar and the outer container (e.g. right of a vertical scroll bar).
        // pub scroll_bar_outer_margin: f32,

        if dark_mode {
            style.visuals = Visuals {
                dark_mode: true,
                widgets: Widgets {
                    noninteractive: WidgetVisuals {
                        weak_bg_fill: Color32::from_gray(27),
                        bg_fill: Color32::from_white_alpha(8),
                        bg_stroke: Stroke::new(1.0, Color32::from_gray(30)), // separators, borders
                        fg_stroke: Stroke::new(1.0, Color32::from_gray(190)), // normal text color
                        rounding: Rounding::same(2.0),
                        expansion: 0.0,
                    },
                    inactive: WidgetVisuals {
                        weak_bg_fill: Color32::from_gray(20),
                        bg_fill: Color32::from_white_alpha(8),
                        bg_stroke: Stroke::new(0.0, Color32::from_gray(72)), // separators, borders
                        // The following is used for All buttons, any clickable text,
                        //    AND text inputs, whether they are inactive OR active. It's really
                        //    overloaded.
                        fg_stroke: Stroke::new(1.0, Color32::from_gray(120)), // button text
                        rounding: Rounding::same(2.0),
                        expansion: 2.0,
                    },
                    hovered: WidgetVisuals {
                        weak_bg_fill: Color32::from_gray(70),
                        bg_fill: Color32::from_gray(70),
                        bg_stroke: Stroke::new(0.0, Color32::from_gray(150)), // e.g. hover over window edge or button
                        fg_stroke: Stroke::new(1.5, Color32::from_gray(240)),
                        rounding: Rounding::same(3.0),
                        expansion: 2.0,
                    },
                    active: WidgetVisuals {
                        weak_bg_fill: Color32::from_gray(55),
                        bg_fill: Color32::from_gray(55),
                        bg_stroke: Stroke::new(0.0, Color32::WHITE),
                        fg_stroke: Stroke::new(2.0, Color32::from_gray(160)),
                        rounding: Rounding::same(2.0),
                        expansion: 2.0,
                    },
                    open: WidgetVisuals {
                        weak_bg_fill: Color32::from_gray(27),
                        bg_fill: Color32::from_gray(27),
                        bg_stroke: Stroke::new(1.0, Color32::from_gray(60)),
                        fg_stroke: Stroke::new(1.0, Color32::from_gray(210)),
                        rounding: Rounding::same(2.0),
                        expansion: 0.0,
                    },
                },

                // Background colors
                window_fill: Color32::from_gray(0x24),
                panel_fill: Color32::from_gray(10),
                faint_bg_color: Color32::from_gray(0x14),
                extreme_bg_color: Color32::from_gray(36),
                code_bg_color: Color32::from_gray(64),

                // Foreground colors
                window_stroke: Stroke::new(1.0, Color32::from_gray(230)),
                override_text_color: None,
                warn_fg_color: Color32::from_rgb(255, 143, 0), // orange
                error_fg_color: Color32::from_rgb(255, 0, 0),  // red
                hyperlink_color: Color32::from_rgb(0x73, 0x95, 0xae), // light blue?

                selection: Selection {
                    bg_fill: Color32::from_gray(40),
                    stroke: Stroke::new(0.0, Color32::from_gray(220)),
                },

                window_shadow: Shadow::big_dark(),
                popup_shadow: Shadow::small_dark(),

                indent_has_left_vline: false,
                menu_rounding: Rounding::same(2.0),
                slider_trailing_fill: true,
                striped: true,
                window_rounding: Rounding::same(6.0),
                resize_corner_size: 12.0,
                text_cursor_width: 2.0,
                text_cursor_preview: false,
                clip_rect_margin: 3.0, // should be at least half the size of the widest frame stroke + max WidgetVisuals::expansion
                button_frame: true,
                collapsing_header_frame: false,
            };
        } else {
            style.visuals = Visuals {
                dark_mode: false,
                widgets: Widgets {
                    noninteractive: WidgetVisuals {
                        weak_bg_fill: Color32::from_gray(248),
                        bg_fill: Color32::from_black_alpha(20),
                        bg_stroke: Stroke::new(1.0, Color32::from_gray(230)),
                        fg_stroke: Stroke::new(1.0, Color32::from_gray(80)), // normal text color
                        rounding: Rounding::same(2.0),
                        expansion: 0.0,
                    },
                    inactive: WidgetVisuals {
                        weak_bg_fill: Color32::from_gray(230),
                        bg_fill: Color32::from_black_alpha(20),
                        bg_stroke: Stroke::new(0.0, Color32::from_gray(192)),
                        // The following is used for All buttons, any clickable text,
                        //    AND text inputs, whether they are inactive OR active. It's really
                        //    overloaded.
                        fg_stroke: Stroke::new(1.0, Color32::from_gray(60)), // button text
                        rounding: Rounding::same(2.0),
                        expansion: 2.0,
                    },
                    hovered: WidgetVisuals {
                        weak_bg_fill: Color32::from_gray(220),
                        bg_fill: Color32::from_gray(220),
                        bg_stroke: Stroke::new(0.0, Color32::from_gray(105)), // e.g. hover over window edge or button
                        fg_stroke: Stroke::new(1.5, Color32::BLACK),
                        rounding: Rounding::same(3.0),
                        expansion: 2.0,
                    },
                    active: WidgetVisuals {
                        weak_bg_fill: Color32::from_gray(165),
                        bg_fill: Color32::from_gray(165),
                        bg_stroke: Stroke::new(0.0, Color32::BLACK),
                        fg_stroke: Stroke::new(2.0, Color32::BLACK),
                        rounding: Rounding::same(2.0),
                        expansion: 2.0,
                    },
                    open: WidgetVisuals {
                        weak_bg_fill: Color32::from_gray(220),
                        bg_fill: Color32::from_gray(220),
                        bg_stroke: Stroke::new(1.0, Color32::from_gray(160)),
                        fg_stroke: Stroke::new(1.0, Color32::BLACK),
                        rounding: Rounding::same(2.0),
                        expansion: 0.0,
                    },
                },

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
                    bg_fill: Color32::from_gray(220),                 // DONE
                    stroke: Stroke::new(1.0, Color32::from_gray(40)), // DONE
                },

                window_shadow: Shadow::big_light(),
                popup_shadow: Shadow::small_light(),

                indent_has_left_vline: false,
                menu_rounding: Rounding::same(2.0),
                slider_trailing_fill: true,
                striped: true,
                window_rounding: Rounding::same(6.0),
                resize_corner_size: 12.0,
                text_cursor_width: 2.0,
                text_cursor_preview: false,
                clip_rect_margin: 3.0, // should be at least half the size of the widest frame stroke + max WidgetVisuals::expansion
                button_frame: true,
                collapsing_header_frame: false,
            };
        }
        style
    }

    fn font_definitions() -> FontDefinitions {
        super::font_definitions() // use default gossip font definitions
    }

    fn text_styles() -> BTreeMap<TextStyle, FontId> {
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
                size: 12.5,
                family: FontFamily::Proportional,
            },
        );

        text_styles.insert(
            TextStyle::Monospace,
            FontId {
                size: 12.5,
                family: FontFamily::Monospace,
            },
        );

        text_styles.insert(
            TextStyle::Button,
            FontId {
                size: 12.5,
                family: FontFamily::Proportional,
            },
        );

        text_styles.insert(
            TextStyle::Heading,
            FontId {
                size: 16.25,
                family: FontFamily::Proportional,
            },
        );

        // for subject lines in notes
        text_styles.insert(
            TextStyle::Name("subject".into()),
            FontId {
                size: 15.0,
                family: FontFamily::Proportional,
            },
        );

        text_styles
    }

    fn highlight_text_format(highlight_type: HighlightType, dark_mode: bool) -> TextFormat {
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
                font_id: FontId::new(12.5, FontFamily::Proportional),
                color: main,
                ..Default::default()
            },
            HighlightType::PublicKey => TextFormat {
                font_id: FontId::new(12.5, FontFamily::Monospace),
                background: grey,
                color: green,
                ..Default::default()
            },
            HighlightType::Event => TextFormat {
                font_id: FontId::new(12.5, FontFamily::Monospace),
                background: grey,
                color: red,
                ..Default::default()
            },
        }
    }

    fn warning_marker_text_color(dark_mode: bool) -> eframe::egui::Color32 {
        if dark_mode {
            Color32::LIGHT_RED
        } else {
            Color32::DARK_RED
        }
    }

    fn notice_marker_text_color(dark_mode: bool) -> eframe::egui::Color32 {
        if dark_mode {
            Color32::LIGHT_BLUE
        } else {
            Color32::DARK_BLUE
        }
    }

    fn input_text_color(dark_mode: bool) -> eframe::egui::Color32 {
        if dark_mode {
            Color32::from_gray(190)
        } else {
            Color32::from_gray(60)
        }
    }

    // feed styling
    fn feed_scroll_rounding(_feed: &FeedProperties) -> Rounding {
        Rounding::none()
    }
    fn feed_scroll_fill(dark_mode: bool, _feed: &FeedProperties) -> Color32 {
        if dark_mode {
            Color32::from_gray(0)
        } else {
            Color32::from_gray(240)
        }
    }
    fn feed_scroll_stroke(_dark_mode: bool, _feed: &FeedProperties) -> Stroke {
        Stroke::NONE
    }
    fn feed_post_separator_stroke(_dark_mode: bool, _post: &NoteRenderData) -> Stroke {
        Stroke::NONE
    }
    fn feed_post_outer_indent(_ui: &mut eframe::egui::Ui, _post: &NoteRenderData) {}
    fn feed_post_inner_indent(ui: &mut eframe::egui::Ui, post: &NoteRenderData) {
        if post.is_thread {
            if post.thread_position > 0 {
                let space = 150.0 * (10.0 - (1000.0 / (post.thread_position as f32 + 100.0)));
                ui.add_space(space);

                ui.label(RichText::new(format!("{}", post.thread_position)).weak());

                let current = ui.next_widget_position();
                let start_point = Pos2::new(current.x - 12.0, current.y + 12.0);
                let end_point = Pos2::new(start_point.x, start_point.y + post.height - 60.0);

                // FIXME: rather than doing color calculations these could all be
                // precalculated. However, this is safer since if we change things it
                // still makes a matching color, whereas the other way we might forget.
                //
                // HsvaGamma has 'value' (lightness) close to perceptually even.
                let dark_mode = ui.visuals().dark_mode;
                let mut hsva: ecolor::HsvaGamma = Self::feed_frame_fill(dark_mode, post).into();
                if dark_mode {
                    hsva.v = (hsva.v + 0.05).min(1.0); // lighten
                } else {
                    hsva.v = (hsva.v - 0.05).max(0.0); // darken
                }
                let color: Color32 = hsva.into();

                let thickness = 2.0;
                ui.painter().add(Shape::line_segment(
                    [start_point, end_point],
                    Stroke::new(thickness, color),
                ));

                ui.add_space(4.0);
            }
        }
    }
    fn feed_frame_inner_margin(_post: &NoteRenderData) -> Margin {
        Margin {
            left: 10.0,
            top: 14.0,
            right: 10.0,
            bottom: 6.0,
        }
    }
    fn feed_frame_outer_margin(_post: &NoteRenderData) -> Margin {
        Margin::symmetric(0.0, 0.0)
    }
    fn feed_frame_rounding(_post: &NoteRenderData) -> Rounding {
        Rounding::same(4.0)
    }
    fn feed_frame_shadow(_dark_mode: bool, _post: &NoteRenderData) -> Shadow {
        Shadow::default()
    }
    fn feed_frame_fill(dark_mode: bool, post: &NoteRenderData) -> Color32 {
        if post.is_main_event {
            if dark_mode {
                Color32::from_rgb(16, 23, 33)
            } else {
                Color32::from_rgb(246, 252, 227)
            }
        } else if post.is_new {
            if dark_mode {
                Color32::from_rgb(34, 28, 38)
            } else {
                Color32::from_rgb(255, 255, 237)
            }
        } else {
            if dark_mode {
                Color32::from_rgb(30, 30, 30)
            } else {
                Color32::WHITE
            }
        }
    }
    fn feed_frame_stroke(_dark_mode: bool, _post: &NoteRenderData) -> Stroke {
        Stroke::NONE
    }

    fn repost_separator_before_stroke(dark_mode: bool, _post: &NoteRenderData) -> Stroke {
        if !_post.is_comment_mention {
            if dark_mode {
                Stroke::new(1.0, Color32::from_gray(60))
            } else {
                Stroke::new(1.0, Color32::from_gray(230))
            }
        } else {
            Stroke::NONE
        }
    }
    fn repost_space_above_separator_before(_post: &NoteRenderData) -> f32 {
        0.0
    }
    fn repost_space_below_separator_before(_post: &NoteRenderData) -> f32 {
        if !_post.is_comment_mention {
            20.0
        } else {
            0.0
        }
    }

    fn repost_separator_after_stroke(_dark_mode: bool, _post: &NoteRenderData) -> Stroke {
        Stroke::NONE
    }
    fn repost_space_above_separator_after(_post: &NoteRenderData) -> f32 {
        0.0
    }
    fn repost_space_below_separator_after(_post: &NoteRenderData) -> f32 {
        0.0
    }

    fn repost_inner_margin(_post: &NoteRenderData) -> Margin {
        Margin {
            left: 0.0,
            top: 14.0,
            right: 10.0,
            bottom: 7.0,
        }
    }
    fn repost_outer_margin(_post: &NoteRenderData) -> Margin {
        Margin {
            left: 0.0,
            top: 12.0,
            right: -10.0,
            bottom: 12.0,
        }
    }
    fn repost_rounding(post: &NoteRenderData) -> Rounding {
        Self::feed_frame_rounding(post)
    }
    fn repost_shadow(_dark_mode: bool, _post: &NoteRenderData) -> Shadow {
        Shadow::NONE
    }
    fn repost_fill(dark_mode: bool, post: &NoteRenderData) -> Color32 {
        if !post.is_comment_mention {
            return Color32::TRANSPARENT;
        }

        let mut hsva: ecolor::HsvaGamma = Self::feed_frame_fill(dark_mode, post).into();
        if dark_mode {
            hsva.v = (hsva.v + 0.03).min(1.0); // lighten
        } else {
            hsva.v = (hsva.v - 0.03).max(0.0); // darken
        }
        let color: Color32 = hsva.into();
        color
    }
    fn repost_stroke(dark_mode: bool, post: &NoteRenderData) -> Stroke {
        Self::feed_frame_stroke(dark_mode, post)
    }

    fn round_image() -> bool {
        true
    }
}
