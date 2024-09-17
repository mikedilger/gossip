use eframe::egui::{self, vec2, Button};
use egui::Ui;

pub fn emoji_picker(ui: &mut Ui) -> Option<char> {
    let mut emojis = "🤙👍👌🙏🤝💪🤘👏🙌🤟🤌🫶👊👆✊\
                      🫂💜❤💟💖✨💫🌈\
                      ✔✅🔥👀💯🚀⚡🎉\
                      🍻🍺☕🍷🥂🍮🥩🍪🍓\
                      🥜👾🎯🏛🍆💀🌻💥⚠🍊🐽☦🌞\
                      😂🤣🐸🫡🤔😆😱😍😭🤯🥰😁🤨\
                      🤡🤠😎😮😅🥳😢🫠👨😄🤢🤐🙄😏🤦\
                      📖🐈🫧🕊🚩💩"
        .chars();

    let mut output: Option<char> = None;

    ui.vertical(|ui| {
        if ui.add(
            Button::new("LIKE").small()
        ).clicked() {
            output = Some('+');
        }
    });

    let mut quit: bool = false;

    loop {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                for _ in 0..10 {
                    if let Some(emoji) = emojis.next() {
                        if ui
                            .add(
                                Button::new(emoji.to_string())
                                    .min_size(vec2(20.0, 20.0))
                                    .small(),
                            )
                            .clicked()
                        {
                            output = Some(emoji);
                        }
                    } else {
                        quit = true;
                    }
                }
            });
        });

        if quit {
            break;
        }
    }

    output
}
