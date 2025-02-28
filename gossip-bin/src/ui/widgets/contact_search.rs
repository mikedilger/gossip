use egui_winit::egui::{
    self, text_edit::TextEditOutput, AboveOrBelow, Key, Modifiers, RichText, Ui,
};
use gossip_lib::{Person, PersonTable, Table};
use nostr_types::PublicKey;

use crate::ui::GossipUi;

#[allow(clippy::too_many_arguments)]
pub(in crate::ui) fn show_contact_search(
    ui: &mut Ui,
    app: &mut GossipUi,
    above_or_below: AboveOrBelow,
    output: &mut TextEditOutput,
    selected: &mut Option<usize>,
    search_results: Vec<(String, PublicKey)>,
    enter_key: bool,
    on_select_callback: impl Fn(&mut Ui, &mut GossipUi, &mut TextEditOutput, &(String, PublicKey)),
) {
    let origin_rect = if let Some(cursor) = output.cursor_range {
        output.galley.pos_from_cursor(&cursor.primary) // position within textedit
    } else {
        output.galley.pos_from_cursor(&output.galley.end()) // position within textedit
    };

    let (pivot, fixed_pos) = match above_or_below {
        AboveOrBelow::Above => (
            egui::Align2::LEFT_BOTTOM,
            output.galley_pos + origin_rect.center_top().to_vec2(),
        ),
        AboveOrBelow::Below => (
            egui::Align2::LEFT_TOP,
            output.galley_pos + origin_rect.center_bottom().to_vec2(),
        ),
    };

    // always compute the tooltip, but it is only shown when
    // is_open is true. This is so we get the animation.
    let frame = egui::Frame::popup(ui.style())
        .rounding(egui::Rounding::ZERO)
        .inner_margin(egui::Margin::same(0.0));
    let area = egui::Area::new(ui.next_auto_id().with("tt"))
        .pivot(pivot)
        .fixed_pos(fixed_pos)
        .movable(false)
        .constrain(true)
        .interactable(true)
        .order(egui::Order::Foreground);

    let is_open = !search_results.is_empty();

    // show search results
    if is_open {
        area.show(ui.ctx(), |ui| {
            frame.show(ui, |ui| {
                app.vert_scroll_area()
                    .id_salt("contactsearch")
                    .max_width(super::TAGG_WIDTH)
                    .max_height(250.0)
                    .show(ui, |ui| {
                        for (i, pair) in search_results.iter().enumerate() {
                            let avatar = if let Some(avatar) = app.try_get_avatar(ui.ctx(), &pair.1)
                            {
                                avatar
                            } else {
                                app.placeholder_avatar.clone()
                            };

                            let frame = egui::Frame::none()
                                .rounding(egui::Rounding::ZERO)
                                .inner_margin(egui::Margin::symmetric(10.0, 5.0));
                            let mut prepared = frame.begin(ui);

                            prepared.content_ui.set_min_width(super::TAGG_WIDTH);
                            prepared.content_ui.set_max_width(super::TAGG_WIDTH);
                            prepared.content_ui.set_min_height(27.0);

                            let frame_rect = prepared.content_ui.min_rect()
                                + (prepared.frame.inner_margin + prepared.frame.outer_margin);

                            let response = ui
                                .interact(
                                    frame_rect,
                                    ui.auto_id_with(pair.1.as_hex_string()),
                                    egui::Sense::click(),
                                )
                                .on_hover_cursor(egui::CursorIcon::PointingHand);

                            // mouse hover moves selected index
                            *selected = if response.hovered() {
                                Some(i)
                            } else {
                                *selected
                            };
                            let is_selected = Some(i) == *selected;

                            {
                                // render inside of frame using prepared.content_ui
                                let ui = &mut prepared.content_ui;
                                if is_selected {
                                    app.theme.on_accent_style(ui.style_mut())
                                }
                                let person = PersonTable::read_record(pair.1, None)
                                    .unwrap_or(Some(Person::new(pair.1)))
                                    .unwrap_or(Person::new(pair.1));
                                ui.horizontal(|ui| {
                                    super::paint_avatar(
                                        ui,
                                        &person,
                                        &avatar,
                                        super::AvatarSize::Mini,
                                    );
                                    ui.vertical(|ui| {
                                        super::truncated_label(
                                            ui,
                                            RichText::new(&pair.0).small(),
                                            super::TAGG_WIDTH - 33.0,
                                        );

                                        let mut nip05 =
                                            RichText::new(person.nip05().unwrap_or_default())
                                                .weak()
                                                .small();
                                        if !person.nip05_valid {
                                            nip05 = nip05.strikethrough()
                                        }
                                        super::truncated_label(ui, nip05, super::TAGG_WIDTH - 33.0);
                                    });
                                })
                            };

                            prepared.frame.fill = if is_selected {
                                app.theme.accent_color()
                            } else {
                                egui::Color32::TRANSPARENT
                            };

                            prepared.end(ui);

                            /* This forces scroll back to the top over and over.
                               need a different solution.
                            if is_selected {
                                response.scroll_to_me(None)
                            }
                            */

                            // to workaround https://github.com/emilk/egui/issues/4147
                            // we will interact again, OVER the painted avatar and text
                            /*
                            let response = ui
                                .interact(
                                    response.rect,
                                    ui.auto_id_with(pair.1.as_hex_string()).with(2),
                                    egui::Sense::click(),
                                )
                                .on_hover_cursor(egui::CursorIcon::PointingHand);
                             */

                            let clicked = response.clicked();
                            if clicked || (enter_key && is_selected) {
                                on_select_callback(ui, app, output, pair);
                            }
                        }
                    });
            });
        });
    }
}

pub(in crate::ui) fn capture_keyboard_for_search(
    ui: &mut Ui,
    result_len: usize,
    selected: Option<usize>,
) -> (Option<usize>, bool) {
    ui.input_mut(|i| {
        // enter
        let enter = i.count_and_consume_key(Modifiers::NONE, Key::Enter) > 0;

        // up / down
        let mut index = selected.unwrap_or(0);
        let down = i.count_and_consume_key(Modifiers::NONE, Key::ArrowDown);
        let up = i.count_and_consume_key(Modifiers::NONE, Key::ArrowUp);
        index += down;
        index = index.min(result_len.saturating_sub(1));
        index = index.saturating_sub(up);

        // tab will cycle down and wrap
        let tab = i.count_and_consume_key(Modifiers::NONE, Key::Tab);
        index += tab;
        if index > result_len.saturating_sub(1) {
            index = 0;
        }

        (Some(index), enter)
    })
}
