use crate::{AVATAR_SIZE_F32, AVATAR_SIZE_REPOST_F32};
use eframe::egui::TextStyle;
use egui_winit::egui::{self, vec2, Image, Response, TextureHandle, Ui, Vec2};
use gossip_lib::{Person, PersonList, GLOBALS};

pub(crate) enum AvatarSize {
    Profile,
    Feed,
    Mini,
}

impl AvatarSize {
    #[allow(dead_code)]
    pub fn x(&self) -> f32 {
        match self {
            AvatarSize::Profile => AVATAR_SIZE_F32 * 3.0,
            AvatarSize::Feed => AVATAR_SIZE_F32,
            AvatarSize::Mini => AVATAR_SIZE_REPOST_F32,
        }
    }

    #[allow(dead_code)]
    pub fn y(&self) -> f32 {
        match self {
            AvatarSize::Profile => AVATAR_SIZE_F32 * 3.0,
            AvatarSize::Feed => AVATAR_SIZE_F32,
            AvatarSize::Mini => AVATAR_SIZE_REPOST_F32,
        }
    }

    pub fn get_size(&self) -> Vec2 {
        match self {
            AvatarSize::Profile => Vec2 {
                x: AVATAR_SIZE_F32 * 3.0,
                y: AVATAR_SIZE_F32 * 3.0,
            },
            AvatarSize::Feed => Vec2 {
                x: AVATAR_SIZE_F32,
                y: AVATAR_SIZE_F32,
            },
            AvatarSize::Mini => Vec2 {
                x: AVATAR_SIZE_REPOST_F32,
                y: AVATAR_SIZE_REPOST_F32,
            },
        }
    }

    fn get_status_size(&self) -> f32 {
        match self {
            AvatarSize::Profile => 10.0,
            AvatarSize::Feed => 5.0,
            AvatarSize::Mini => 5.0,
        }
    }

    fn get_status_stroke_width(&self) -> f32 {
        match self {
            AvatarSize::Profile => 2.0,
            AvatarSize::Feed => 1.0,
            AvatarSize::Mini => 1.0,
        }
    }
}

pub(crate) fn paint_avatar(
    ui: &mut Ui,
    person: &Person,
    avatar: &TextureHandle,
    avatar_size: AvatarSize,
) -> Response {
    let followed = person.is_in_list(PersonList::Followed);
    let muted = person.is_in_list(PersonList::Muted);
    let on_list = person.is_in_list(PersonList::Custom(2)); // TODO: change to any list
    let size = avatar_size.get_size();
    let has_dm_relays = GLOBALS.db().has_dm_relays(person.pubkey).unwrap_or(false);
    let wot = format!("{}", GLOBALS.db().read_wot(person.pubkey).unwrap_or(0));
    const UNICODE_CIRCLED_17: &str = "\u{2470}";

    let avatar_response = paint_avatar_only(ui, avatar, avatar_size.get_size());

    let status_color = match (followed, on_list, muted) {
        (true, _, false) => ui.visuals().hyperlink_color, // followed
        (false, true, false) => egui::Color32::GREEN,     // on-list
        (_, _, true) => ui.visuals().warn_fg_color,       // muted
        (false, false, false) => egui::Color32::TRANSPARENT,
    };
    if status_color != egui::Color32::TRANSPARENT {
        let center = avatar_response.rect.right_top() + vec2(-0.139 * size.x, 0.139 * size.y);
        ui.painter().circle(
            center,
            avatar_size.get_status_size(),
            status_color,
            egui::Stroke::new(
                avatar_size.get_status_stroke_width(),
                ui.visuals().panel_fill,
            ),
        );
        let rect = egui::Rect::from_center_size(
            center,
            vec2(avatar_size.get_status_size(), avatar_size.get_status_size()),
        );
        ui.interact(rect, ui.auto_id_with("status-circle"), egui::Sense::hover())
            .on_hover_text({
                let mut stat: Vec<&str> = Vec::new();
                if followed {
                    stat.push("followed")
                }
                if on_list {
                    stat.push("priority")
                }
                if muted {
                    stat.push("muted")
                }
                stat.join(", ")
            });
    }
    // paint WoT
    {
        let center = avatar_response.rect.left_bottom() + vec2(0.139 * size.x, -0.139 * size.y);
        let mut fontid = TextStyle::Body.resolve(ui.style());
        fontid.size = 9.0;
        ui.painter().circle(
            center,
            avatar_size.get_status_size() + 3.0,
            egui::Color32::WHITE,
            egui::Stroke::new(
                avatar_size.get_status_stroke_width(),
                ui.visuals().panel_fill,
            ),
        );
        ui.painter().text(
            center + vec2(0.0, -1.0),
            egui::Align2::CENTER_CENTER,
            wot,
            fontid,
            ui.visuals().hyperlink_color,
        );
    }
    if has_dm_relays {
        let center = avatar_response.rect.right_bottom() + vec2(-0.139 * size.x, -0.139 * size.y);
        let mut fontid = TextStyle::Body.resolve(ui.style());
        fontid.size = 11.0;
        ui.painter().circle(
            center,
            avatar_size.get_status_size() + 1.0,
            egui::Color32::WHITE,
            egui::Stroke::new(
                avatar_size.get_status_stroke_width(),
                ui.visuals().panel_fill,
            ),
        );
        ui.painter().text(
            center + vec2(0.0, -1.0),
            egui::Align2::CENTER_CENTER,
            UNICODE_CIRCLED_17,
            fontid,
            ui.visuals().hyperlink_color,
        );
    }
    avatar_response
}

/// Paint avatar without decorations
pub(crate) fn paint_avatar_only(ui: &mut Ui, avatar: &TextureHandle, size: Vec2) -> Response {
    let avatar_response = ui.add(
        Image::new(avatar)
            .max_size(size)
            .maintain_aspect_ratio(true)
            .sense(egui::Sense::click()),
    );
    avatar_response
}
