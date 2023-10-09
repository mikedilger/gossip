use super::GossipUi;
use crate::ui::feed::NoteRenderData;
use crate::ui::HighlightType;
use eframe::egui;
use egui::text::LayoutJob;
use egui::widget_text::WidgetText;
use egui::{Color32, Context, Frame, Margin, RichText, Ui};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Background {
    None,
    Input,
    Note,
    HighlightedNote,
}

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.add_space(10.0);
    ui.heading("Theme Test".to_string());
    ui.add_space(12.0);
    ui.separator();

    // On No Background
    Frame::none()
        .inner_margin(Margin::symmetric(20.0, 20.0))
        .show(ui, |ui| {
            ui.heading("No Background");
            inner(app, ui, Background::None);
        });

    // On Note Background
    let render_data = NoteRenderData {
        height: 200.0,
        is_new: false,
        is_main_event: false,
        has_repost: false,
        is_comment_mention: false,
        is_thread: false,
        is_first: true,
        is_last: true,
        thread_position: 0,
        can_post: false,
    };
    Frame::none()
        .inner_margin(app.theme.feed_frame_inner_margin(&render_data))
        .outer_margin(app.theme.feed_frame_outer_margin(&render_data))
        .rounding(app.theme.feed_frame_rounding(&render_data))
        .shadow(app.theme.feed_frame_shadow(&render_data))
        .fill(app.theme.feed_frame_fill(&render_data))
        .stroke(app.theme.feed_frame_stroke(&render_data))
        .show(ui, |ui| {
            ui.heading("Note Background");
            ui.label("(with note margins)");
            inner(app, ui, Background::Note);
        });

    // On Highlighted Note Background
    let render_data = NoteRenderData {
        height: 200.0,
        is_new: true,
        is_main_event: false,
        has_repost: false,
        is_comment_mention: false,
        is_thread: false,
        is_first: true,
        is_last: true,
        thread_position: 0,
        can_post: false,
    };
    Frame::none()
        .inner_margin(app.theme.feed_frame_inner_margin(&render_data))
        .outer_margin(app.theme.feed_frame_outer_margin(&render_data))
        .rounding(app.theme.feed_frame_rounding(&render_data))
        .shadow(app.theme.feed_frame_shadow(&render_data))
        .fill(app.theme.feed_frame_fill(&render_data))
        .stroke(app.theme.feed_frame_stroke(&render_data))
        .show(ui, |ui| {
            ui.heading("Unread Note Background");
            ui.label("(with note margins)");
            inner(app, ui, Background::HighlightedNote);
        });

    // On Input Background
    Frame::none()
        .fill(app.theme.get_style().visuals.extreme_bg_color)
        .inner_margin(Margin::symmetric(20.0, 20.0))
        .show(ui, |ui| {
            ui.heading("Input Background");
            inner(app, ui, Background::Input);
        });
}

fn inner(app: &mut GossipUi, ui: &mut Ui, background: Background) {
    let theme = app.theme;
    let accent = RichText::new("accent").color(theme.accent_color());
    let accent_complementary = RichText::new("accent complimentary (indirectly used)")
        .color(theme.accent_complementary_color());

    line(ui, accent);
    line(ui, accent_complementary);

    if background == Background::Input {
        for (ht, txt) in [
            (HighlightType::Nothing, "nothing"),
            (HighlightType::PublicKey, "public key"),
            (HighlightType::Event, "event"),
            (HighlightType::Relay, "relay"),
            (HighlightType::Hyperlink, "hyperlink"),
        ] {
            let mut highlight_job = LayoutJob::default();
            highlight_job.append(
                &format!("highlight text format for {}", txt),
                0.0,
                theme.highlight_text_format(ht),
            );
            line(ui, WidgetText::LayoutJob(highlight_job));
        }
    }

    if background == Background::Note || background == Background::HighlightedNote {
        let warning_marker =
            RichText::new("warning marker").color(theme.warning_marker_text_color());
        line(ui, warning_marker);

        let notice_marker = RichText::new("notice marker").color(theme.notice_marker_text_color());
        line(ui, notice_marker);
    }

    if background != Background::Input {
        ui.horizontal(|ui| {
            ui.label(RichText::new("•").color(Color32::from_gray(128)));
            crate::ui::widgets::break_anywhere_hyperlink_to(
                ui,
                "https://hyperlink.example.com",
                "https://hyperlink.example.com",
            );
        });
    }
}

fn line(ui: &mut Ui, label: impl Into<WidgetText>) {
    let bullet = RichText::new("•").color(Color32::from_gray(128));
    ui.horizontal(|ui| {
        ui.label(bullet);
        ui.label(label);
    });
}
