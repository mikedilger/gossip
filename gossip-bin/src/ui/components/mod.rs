use eframe::egui;
use egui::{Label, Sense, Ui};

pub fn emoji_picker(ui: &mut Ui) -> Option<char> {
    let mut emojis = "😀😁😊😆😅😂🤣\
                      😕🥺😯😭🫡😑🤭\
                      🙄😏🤔🤨😒🙃🤭\
                      😛🤢🤐😒😬😍🥰\
                      🖕💩💀👀🍆👑🚩\
                      ✨⭐⚡❤💜🫂\
                      🤌🙏🫰\
                      🤙🤝👍🔥🎯💯✅"
        .chars();

    let mut output: Option<char> = None;

    let mut quit: bool = false;

    loop {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                for _ in 0..6 {
                    if let Some(emoji) = emojis.next() {
                        if ui
                            .add(Label::new(emoji.to_string()).sense(Sense::click()))
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
