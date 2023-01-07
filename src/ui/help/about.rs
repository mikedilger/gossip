use super::GossipUi;
use eframe::egui;
use egui::{Align, Context, Layout, RichText, TextStyle, Ui};

pub(super) fn update(app: &mut GossipUi, _ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.with_layout(Layout::top_down(Align::Center), |ui| {

        ui.add_space(30.0);

        ui.image(&app.icon, app.icon.size_vec2());

        ui.add_space(15.0);

        ui.label(
            RichText::new(&app.about.name).strong()
        );

        ui.add_space(15.0);

        ui.label(
            RichText::new(&app.about.version)
                .text_style(TextStyle::Body)
        );

        ui.add_space(15.0);

        ui.label(
            RichText::new(&app.about.description)
                .text_style(TextStyle::Body)
        );

        ui.add_space(35.0);

        ui.label(
            RichText::new(format!("nostr is a protocol and specification for storing and retrieving social media events onto servers called relays. Many users store their events onto multiple relays for reliability, censorship resistance, and to spread their reach.

Users are defined by their keypair, and are known by the public key of that pair. All events they generate are signed by their private key, and verifiable by their public key.

We are storing data on your system in this file: {}. This data is only used locally by this client - the nostr protocol does not use clients as a store of data for other people. We are storing your settings, your private and public key, information about relays, and a cache of events. We cache events in your feed so that we don't have to ask relays for them again, which means less network traffic and faster startup times.
", app.about.storage_path))
                    .text_style(TextStyle::Body)
        );

        ui.add_space(22.0);

        ui.hyperlink_to("Learn More about Nostr", "https://github.com/nostr-protocol/nostr");

        ui.add_space(30.0);

        ui.hyperlink_to("Source Code", &app.about.homepage);
        ui.label(
            RichText::new("by")
                .text_style(TextStyle::Small)
        );
        ui.label(
            RichText::new(&app.about.authors)
                .text_style(TextStyle::Small)
        );

        ui.add_space(15.0);

        ui.label(
            RichText::new("This program comes with absolutely no warranty.")
                .text_style(TextStyle::Small)
        );
        ui.label(
            RichText::new("See the MIT License for details.")
                .text_style(TextStyle::Small)
        );
    });
}
