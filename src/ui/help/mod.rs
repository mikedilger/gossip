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
            ui.label("This is a core concept. Gossip doesn't fetch posts from the same relays it is configured to post to. It trys to fetch posts from whereever your followers post them, so you need to configure at least one relay for each person you follow. Gossip will then dynamically figure out where they actually post (if it finds them at all) and keep things updated as they change where they post to. A lot of other clients are not operating like this, they are pulling from the same relays they push to and this author thinks that will not scale. Right now, these other clients work because relays are copying messages from each other somehow.");
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

            ui.label("Gossip currently does not fetch your following list from nostr. Nor does it publish the list of follows you configure on gossip so you don't have to worry about it clobbering anything.");

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            ui.heading("Driving the Feed");
            ui.add_space(10.0);

            ui.label("As events come in, they often refer to other events that have not come in yet. If you want to query the relays for these missing events, you can by pressing the QM (Query Missing) button on the feed page. Usually some but not all missing events can be found this way.");
            ui.add_space(10.0);

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

            ui.label("After generating or importing your key, gossip will save it encrypted under a password. You will need this password to unlock it every time you start gossip. Gossip handles keys securely by never displaying them and zeroing memory used for private keys and passwords before freeing it (unless you explicitly request it to be exported).");
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

            ui.label("To react, you can click the heart. Other kinds of reactions are not yet implemented. You can see other people's reactions belo the posts.");
            ui.add_space(10.0);

            ui.add_space(10.0);

        });
    } else if app.page == Page::HelpStats {
        stats::update(app, ctx, _frame, ui);
    } else if app.page == Page::HelpAbout {
        about::update(app, ctx, _frame, ui);
    }
}
