use super::{GossipUi, Page};
use eframe::egui;
use egui::{Context, ScrollArea, Ui};

mod about;
mod stats;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.selectable_value(&mut app.page, Page::HelpHelp, "Help");
        ui.separator();
        ui.selectable_value(&mut app.page, Page::HelpStats, "Stats");
        ui.separator();
        ui.selectable_value(&mut app.page, Page::HelpAbout, "About");
        ui.separator();
    });
    ui.separator();

    if app.page == Page::HelpHelp {
        ui.add_space(24.0);
        ui.heading("Help");

        ScrollArea::vertical().show(ui, |ui| {

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            ui.heading("Gossip follows people AT RELAYS");
            ui.add_space(10.0);
            ui.label("This is a core concept. We don't connect to a community relay and see what is going on. Gossip only follows the specific people you configure it to follow, wherever they post. Therefore, you cannot just add a pubkey to follow somebody and hope they post to the relays you listen to (a pattern that doesn't work well given that there are hundreds of relays). No, instead you also need to tell gossip what relays they post to (at least one) so we can pull their posts.");
            ui.add_space(10.0);

            ui.label("NIP-35 makes this easy, since it specifies how users can share their public key AND their relays via a webserver that they control. For example, you can follow me at `mike@mikedilger.com`. That's all you need to type in.");
            ui.add_space(10.0);

            ui.label("Other ways of following people include pasting their public key (hex or bech32 format) and typing in a relay URL which should start with 'ws'.  NOTE: use CTRL-V to paste, other forms of pasting (X11 middle click) won't work.");
            ui.add_space(10.0);

            ui.horizontal_wrapped(|ui| {
                ui.label("To get started, go to the");
                if ui.link("People > Follow Someone New").clicked() {
                    app.page = Page::PeopleFollow;
                }
                ui.label("page and add people to follow. If you don't know anybody, you can follow me at NIP-35 DNS ID mike@mikedilger.com and you can find other people through me (posts I reply to or quote).");
            });
            ui.add_space(10.0);

            ui.label("FOR THE MOMENT you need to restart gossip after you add someone to follow. This will be fixed soon.");
            ui.add_space(10.0);

            ui.label("Gossip currently does not fetch your following list from nostr. Nor does it publish the list of follows you configure on gossip so you don't have to worry about it clobbering anything.");

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            ui.heading("Driving the Feed");
            ui.add_space(10.0);

            ui.label("As events come in, the feed does not update. This is to avoid the annoying problem of stuff moving while you are trying to read it, or worse yet, trying to interact with it.");
            ui.add_space(10.0);

            ui.label("So watch the button at the top which says \"Process N incoming events\". If N is a positive integer, pressing that button will update the feed with these new events. Once you do, all existing events will become black/white, and the new events will become red/yellow to highlight them. NOTE, if N=-1, that just means the UI couldn't get a lock on the object needed to count them, and it doesn't cache the data. Also, the same event can come in multiple times and get highlighted again, and some events are not posts so they wont highlight anything - don't expect the number of highlights to match the number on the button.");
            ui.add_space(10.0);

            ui.label("Events often refer to other events as replies, quotes, etc. When gossip finds out about these other events, but doesn't have them, it adds them to it's desired event list. A button at the top \"Query relays for N missing events\" allows you to try to get these events. Usually you won't be able to get them all, as most references to other events on nostr still don't include the Url where the event can be found. In those cases, we try all the relays you are currently connected to, but it's a long shot. Every time you press this button it bothers the relays, so while the \"Process N incoming events\" button can be pressed as much as you like, be courteous with the \"Query relays for N events\" button and don't spam it over and over.");
            ui.add_space(10.0);

            ui.label("Each post has a little triangle to the left of it. You can \"tip\" this triangle to open/close all replies to that post. Buttons at the top let you open/close all the posts.");
            ui.add_space(10.0);

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            ui.heading("Configuring your Identity");
            ui.add_space(10.0);

            ui.horizontal_wrapped(|ui| {
                ui.label("On the");
                if ui.link("You").clicked() {
                    app.page = Page::You;
                }
                ui.label("page you can setup your identity. If you are new, you should just press \"Generate\" and you are good to go. Otherwise you can import a private key in hex or bech32 format, although it isn't very secure to cut-n-paste and display your private key, so it will mark your key security as \"weak\". Eventually you'll be able to import your password-protected private key from a nostr relay.");
            });
            ui.add_space(10.0);

            ui.label("After generating or importing your key, gossip will save it encrypted under a password. You will need this password to unlock it every time you start gossip. Gossip handles keys securely by never displaying them and zeroing memory used for private keys and passwords before freeing it.");
            ui.add_space(10.0);

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            ui.heading("Configuring your Relays");
            ui.add_space(10.);

            ui.label("Until you configure relays to post to, your posts won't go anywhere and nobody will see them.");
            ui.add_space(10.0);

            ui.horizontal_wrapped(|ui| {
                ui.label("Go to the");
                if ui.link("Relays").clicked() {
                    app.page = Page::Relays;
                }
                ui.label("page and tick a half dozen relays that you intend to post to. If your webserver serves a nostr.json file, you can follow NIP-35 and use the same relays in that file.");
            });
            ui.add_space(10.0);

            ui.label("Gossip currently doesn't let you type in relays here. This will be fixed soon.");

            ui.label("Gossip currently does not synchronize this list of relays on the nostr network, so it will not get data you use with other clients. Nor will it clobber that data. The list is local and independent.");

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            ui.heading("Posting, Replying, Reacting");
            ui.add_space(10.0);

            ui.label("To post, after unlocking your private key, type in the box at the top of the feed and press Send.");
            ui.add_space(10.0);

            ui.label("To reply, press the reply icon at the bottom of the post you want to reply to. That post will be copied to the top of the page to make it clear what you are replying to. Type your reply and press Send.");
            ui.add_space(10.0);

            ui.label("Reacting is not implemented yet. You can see other people's reactions with +1 -1 markings on posts.");
            ui.add_space(10.0);

            ui.add_space(10.0);

        });
    } else if app.page == Page::HelpStats {
        stats::update(app, ctx, _frame, ui);
    } else if app.page == Page::HelpAbout {
        about::update(app, ctx, _frame, ui);
    }
}
