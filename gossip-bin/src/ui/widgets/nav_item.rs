use std::sync::Arc;

//#![allow(dead_code)]
use eframe::egui;
use egui::*;

/// Navigation Entry
///
/// ```
/// # egui::__run_test_ui(|ui| {
/// ui.add(egui::NavItem::new("Item1", false));
/// ui.add(egui::NavItem::new("Item2", true);
/// ui.add(egui::NavItem::new(egui::RichText::new("With formatting").underline(), false));
/// # });
/// ```
#[must_use = "You should put this widget in an ui with `ui.add(widget);`"]
pub struct NavItem {
    text: WidgetText,
    sense: Option<Sense>,
    color: Option<Color32>,
    active_color: Option<Color32>,
    hover_color: Option<Color32>,
    is_active: bool,
}

impl NavItem {
    pub fn new(text: impl Into<WidgetText>, active: bool) -> Self {
        Self {
            text: text.into(),
            sense: None,
            color: None,
            active_color: None,
            hover_color: None,
            is_active: active,
        }
    }

    /// Set an optional color
    #[inline]
    pub fn color(mut self, color: Color32) -> Self {
        self.color = Some(color);
        self
    }

    /// Set an optional active color
    #[inline]
    pub fn active_color(mut self, color: Color32) -> Self {
        self.active_color = Some(color);
        self
    }

    /// Set an optional hover color
    #[inline]
    pub fn hover_color(mut self, color: Color32) -> Self {
        self.hover_color = Some(color);
        self
    }

    /// Make the label respond to clicks and/or drags.
    ///
    /// By default, a label is inert and does not respond to click or drags.
    /// By calling this you can turn the label into a button of sorts.
    /// This will also give the label the hover-effect of a button, but without the frame.
    ///
    /// ```
    /// # use egui::{NavItem, Sense};
    /// # egui::__run_test_ui(|ui| {
    /// if ui.add(NavItem::new("click me").sense(Sense::click())).clicked() {
    ///     /* … */
    /// }
    /// # });
    /// ```
    pub fn sense(mut self, sense: Sense) -> Self {
        self.sense = Some(sense);
        self
    }
}

impl NavItem {
    /// Do layout and position the galley in the ui, without painting it or adding widget info.
    pub fn layout_in_ui(self, ui: &mut Ui) -> (Pos2, Arc<Galley>, Response) {
        let sense = self.sense.unwrap_or(Sense::click());
        if let WidgetText::Galley(galley) = self.text {
            // If the user said "use this specific galley", then just use it:
            let (rect, response) = ui.allocate_exact_size(galley.size(), sense);
            let pos = match galley.job.halign {
                Align::LEFT => rect.left_top(),
                Align::Center => rect.center_top(),
                Align::RIGHT => rect.right_top(),
            };
            return (pos, galley, response);
        }

        let valign = ui.layout().vertical_align();
        let mut job = self
            .text
            .into_layout_job(ui.style(), FontSelection::Default, valign);

        let should_wrap = ui.wrap_text();
        let available_width = ui.available_width();

        if should_wrap
            && ui.layout().main_dir() == Direction::LeftToRight
            && ui.layout().main_wrap()
            && available_width.is_finite()
        {
            // On a wrapping horizontal layout we want text to start after the previous widget,
            // then continue on the line below! This will take some extra work:

            let cursor = ui.cursor();
            let first_row_indentation = available_width - ui.available_size_before_wrap().x;
            debug_assert!(first_row_indentation.is_finite());

            job.wrap.max_width = available_width;
            job.first_row_min_height = cursor.height();
            job.halign = Align::Min;
            job.justify = false;
            if let Some(first_section) = job.sections.first_mut() {
                first_section.leading_space = first_row_indentation;
            }
            let galley = ui.fonts(|f| f.layout_job(job));

            let pos = pos2(ui.max_rect().left(), ui.cursor().top());
            assert!(!galley.rows.is_empty(), "Galleys are never empty");

            // set the row height to ensure the cursor advancement is correct. when creating a child ui such as with
            // ui.horizontal_wrapped, the initial cursor will be set to the height of the child ui. this can lead
            // to the cursor not advancing to the second row but rather expanding the height of the cursor.
            //
            // note that we do not set the row height earlier in this function as we do want to allow populating
            // `first_row_min_height` above. however it is crucial the placer knows the actual row height by
            // setting the cursor height before ui.allocate_rect() gets called.
            ui.set_row_height(galley.rows[0].height());

            // collect a response from many rows:
            let rect = galley.rows[0].rect.translate(vec2(pos.x, pos.y));
            let mut response = ui.allocate_rect(rect, sense);
            for row in galley.rows.iter().skip(1) {
                let rect = row.rect.translate(vec2(pos.x, pos.y));
                response |= ui.allocate_rect(rect, sense);
            }
            (pos, galley, response)
        } else {
            if should_wrap {
                job.wrap.max_width = available_width;
            } else {
                job.wrap.max_width = f32::INFINITY;
            };

            job.halign = ui.layout().horizontal_placement();
            job.justify = ui.layout().horizontal_justify();

            let galley = ui.fonts(|f| f.layout_job(job));
            let (rect, response) = ui.allocate_exact_size(galley.size(), sense);
            let pos = match galley.job.halign {
                Align::LEFT => rect.left_top(),
                Align::Center => rect.center_top(),
                Align::RIGHT => rect.right_top(),
            };
            (pos, galley, response)
        }
    }
}

impl Widget for NavItem {
    fn ui(self, ui: &mut Ui) -> Response {
        let is_active = self.is_active;
        let color = self.color;
        let hover_color = self.hover_color;
        let active_color = self.active_color;
        let (pos, galley, response) = self.layout_in_ui(ui);
        response
            .widget_info(|| WidgetInfo::labeled(WidgetType::Label, ui.is_enabled(), galley.text()));

        if ui.is_rect_visible(response.rect) {
            let color = if hover_color.is_some() && response.hovered() {
                hover_color
            } else if is_active && active_color.is_some() {
                active_color
            } else if color.is_some() {
                color
            } else {
                Some(ui.style().interact(&response).text_color())
            };

            response
                .clone()
                .on_hover_cursor(egui::CursorIcon::PointingHand);

            ui.painter().add(epaint::TextShape {
                pos,
                galley,
                override_text_color: color,
                underline: Stroke::NONE,
                angle: 0.0,
                fallback_color: ui.visuals().text_color(),
                opacity_factor: 1.0,
            });
        }

        response
    }
}
