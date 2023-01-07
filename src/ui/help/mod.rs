use super::{GossipUi, Page};
use eframe::egui;
use egui::{Context, RichText, ScrollArea, Ui};

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

            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Gossip follows people").heading());
                ui.label(RichText::new("at relays").heading().italics());
            });

            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                ui.label("Gossip follows people at whichever relays they post to, ");
                ui.label(RichText::new("not").strong());
                ui.label("whichever relays you post to. This is a core concept. As the nostr network expands, it will be increasingly unlikely that the person you want to follow posts to the same relays that you do. And it will become increasingly untenable for event mirroring to be occuring on all those relays. Most clients will eventually need to work this way, except for clients that intend to be bound to a local community of relays.");
            });

            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                ui.label("The upshot of this is that when you follow someone, you will need to supply their public key");
                ui.label(RichText::new("and their relays").italics());
            });

            ui.add_space(10.0);
            ui.label("NIP-05 (used in the reverse for user discovery) makes this easy, since it specifies how users can share their public key and their relays via a webserver that they control. For example, you can follow me at `mike@mikedilger.com`. That's all you need to type in. Gossip will go to mikedilger.com, fetch the `.well-known/nostr.json` file, find the entry for `mike` and find the relays for that public key. Then it will go to those relays and pull my recent posts into your feed.");
            ui.add_space(10.0);

            ui.label("Other ways of following people include pasting their public key (hex or bech32 format) and typing in a relay URL which should start with 'ws'.  NOTE: use CTRL-V to paste, other forms of pasting (X11 middle click) won't work.");
            ui.add_space(10.0);

            ui.horizontal_wrapped(|ui| {
                ui.label("To get started, go to the");
                if ui.link("People > Follow Someone New").clicked() {
                    app.page = Page::PeopleFollow;
                }
                ui.label("page and add people to follow. If you don't know anybody, you can follow me at NIP-05 DNS ID mike@mikedilger.com and you can find other people through me (posts I reply to or quote).");
            });
            ui.add_space(10.0);

            ui.label("Gossip currently does not fetch your following list from nostr. Nor does it publish the list of follows you configure on gossip so you don't have to worry about it clobbering anything (but this will change).");

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            ui.heading("Driving the Feed");
            ui.add_space(10.0);

            ui.label("Recent events from people you follow should show up on the main [Feed > Following] page in reverse chronological order, without replies or context. If you want to see the context of a post, press the right-arrow on the right hand side of the feed to move to the [Feed > Thread] page. This page should show the full context, and when you enter the thread page it will ask relays questions in order to fill in the thread. But often it is missing posts and can't quite get the full context. In fact, there are cases where the post you clicked on isn't even there. Future development on Gossip will likely improve this greatly.");
            ui.add_space(10.0);

            ui.label("One dirty hack that helps: As events come in, they often refer to other events that have not come in yet. If you want to query the relays for these missing events, you can by pressing the QM (Query Missing) button on the feed page. Usually some but not all missing events can be found this way. But it is older code and newer event processing may have already superceded it's abilities by the time you read this. Still, worth a shot.");

            ui.add_space(10.0);
            ui.label("There is a [Feed > Replies] page where you can see replies to your own posts, with descendants (but not ancestors)");

            ui.add_space(10.0);
            ui.label("Finally there is a [Feed > Person] page where you can see posts made by a person. To get there, click an avatar, then click [VIEW THEIR FEED].");

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

            ui.label("If you are just trying Gossip out and not intending to post or react to posts yet, you can just import your public key. This way you'll be able to sync your following list (when that work is committed), and see your replies, but you won't be able to post or react to posts.");
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
                ui.label("page and tick a half dozen relays that you intend to post to. If your webserver serves a nostr.json file, you can follow NIP-05 and use the same relays in that file.");
            });
            ui.add_space(10.0);

            ui.label("Gossip currently does not synchronize this list of relays on the nostr network, so it will not get data you use with other clients. Nor will it clobber that data. The list is local and independent (but this will change).");

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            ui.heading("Posting, Replying, Reacting");
            ui.add_space(10.0);

            ui.label("To post, after unlocking your private key, type in the box at the top of the feed and press Send.");
            ui.add_space(10.0);

            ui.label("To reply, press the reply icon at the bottom of the post you want to reply to. That post will be copied to the top of the page to make it clear what you are replying to. Type your reply and press Send.");
            ui.add_space(10.0);

            ui.label("To react, you can click the heart. Other kinds of reactions are not yet implemented. You can see other people's reactions below the posts.");
            ui.add_space(10.0);

            ui.label("Quoting and Boosting content are not yet implemented.");
            ui.add_space(10.0);

            ui.add_space(10.0);

        });
    } else if app.page == Page::HelpStats {
        stats::update(app, ctx, _frame, ui);
    } else if app.page == Page::HelpAbout {
        about::update(app, ctx, _frame, ui);
    }
}
