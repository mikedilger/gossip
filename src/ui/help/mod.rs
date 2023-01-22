use super::{GossipUi, Page};
use crate::feed::FeedKind;
use eframe::egui;
use egui::{Context, RichText, ScrollArea, Ui};

mod about;
mod stats;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.selectable_value(&mut app.page, Page::HelpHelp, "Getting Started");
        ui.separator();
        ui.selectable_value(&mut app.page, Page::HelpStats, "Stats");
        ui.separator();
        ui.selectable_value(&mut app.page, Page::HelpAbout, "About");
        ui.separator();
    });
    ui.separator();

    if app.page == Page::HelpHelp {
        ui.add_space(24.0);
        ui.heading("Help - Getting Started");

        ScrollArea::vertical().show(ui, |ui| {

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Quickstart").heading());
            });

            ui.add_space(10.0);
            ui.label("If you have used other clients, here is the procedure to get up and running on gossip:");
            ui.add_space(10.0);

            ui.label("• Setup your Identity");
            ui.indent("quickstartaddkey", |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("On the");
                    if ui.link("You").clicked() {
                        app.set_page(Page::YourKeys);
                    }
                    ui.label("page, add your key (public or private). If you supply your private key you will be able to post, but this is not necessary to use gossip as a viewing tool. You can even supply somebody else's public key and see what they see (sneaky you!)");
                });
            });
            ui.label("• Setup the relays you write to");
            ui.indent("quickstartrelays", |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("On the");
                    if ui.link("Relays").clicked() {
                        app.set_page(Page::Relays);
                    }
                    ui.label("page, add a relay that you post to (or several), and tick off \"Post Here\" (otherwise it won't pull your data from there). Remember to press \"SAVE CHANGES\" at the bottom of that page.");
                });
            });
            ui.label("• Follow yourself");
            ui.indent("quickstartfollowyou", |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("On the");
                    if ui.link("People > Follow Someone New").clicked() {
                        app.set_page(Page::PeopleFollow);
                    }
                    ui.label("page, follow yourself (specify your public key AND one of the relays you added in the previous step. If you don't add the relay, gossip can't help you).");
                });
            });
            ui.label("• Restart");
            ui.indent("quickstartrestart", |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Sorry, this insufficiency will be remedied eventually. After following somebody it doesn't rewrite the general feed subscription, but restarting does.");
                });
            });
            ui.label("• Browse your feed and explore threads");
            ui.indent("quickstartfeedthreads", |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("On the");
                    if ui.link("Feed > Following").clicked() {
                        app.set_page(Page::Feed(FeedKind::General));
                    }
                    ui.label("page, look at your posts (by default only the last 12 hours show up) and their replies (by clicking the right arrow on the right side of post to give the thread), which will give gossip some data to launch from. Hopefully you have some replies. But if not, no worry, the next step helps too.");
                });
            });
            ui.label("• Pull your Contacts List");
            ui.indent("quickstartpullcontacts", |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("On the");
                    if ui.link("People > Followed").clicked() {
                        app.set_page(Page::PeopleList);
                    }
                    ui.label("page, press [Pull Overwrite] to pull down the people you follow. Then press [Refresh Metadata] to update their metadata (it might work for some and not others, it depends if gossip knows which relays they are at yet).");
                });
            });
            ui.label("• Click Avatars to explore people");
            ui.indent("quickstartclickavatars", |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Click any of these people's avatars to get to their page, where you can update their metadata or view their posts. If you don't get any data for a person, it may be because there is no good way for gossip to know where they post to. This problem goes away after using gossip for awhile, and it remains an outstanding issue to solve.");
                });
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Gossip follows people").heading());
                ui.label(RichText::new("at relays").heading().italics());
            });

            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                ui.label("Gossip follows people at whichever relays they post to,");
                ui.label(RichText::new("not").strong());
                ui.label("some list of relays you choose to read from. This is a core concept. As the nostr network expands, it will be increasingly unlikely that the person you want to follow posts to the same relays that you do. And it will become increasingly untenable for event mirroring to be occuring on all those relays. Most clients will eventually need to work this way, except for clients that intend to be bound to a local community of relays.");
            });

            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                ui.label("Think of it like a web browser. Web browsers fetch resources that other resources (pages) refer to, and they can get them from any URL on the Internet. Gossip can find people and events at relays it has never heard of before if other events reference them as being there.");
            });

            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                ui.label("The upshot of this is that when you follow someone, you will need to supply their public key");
                ui.label(RichText::new("and their relays.").italics());
                ui.label("You do this at the");
                if ui.link("People > Follow Someone New").clicked() {
                    app.set_page(Page::PeopleFollow);
                }
                ui.label("page.");
            });

            ui.add_space(10.0);
            ui.label("There are multiple ways to supply this information:");
            ui.label("\n• Fetching your Contact List (kind 3 event)");
            ui.indent("helpfollowcontactlist", |ui| {
                ui.label("If you have used other clients and have published your contact list, after you setup your identity (next section) you can pull in your contact list. However, it probably won't include URLs for your contacts.");
            });
            ui.label("\n• nprofile");
            ui.indent("helpfollownprofile1", |ui| {
                ui.label("An nprofile string is bech32 encoded information about a person that includes their public key as well as one or more relays that they post to. As strings, they can be anywhere: on other social media, in emails, on webpages, as QR codes, etc. NOTE: as of this writing gossip does not support nprofile, but it should soon.");
            });
            ui.label("\n• NIP-05 (in reverse)");
            ui.indent("helpfollownip05", |ui| {
                ui.label("NIP-05 specifies a nostr.json file on a webserver which contains a person's name and public key, but now also can contain the relays they post to. I find it very easy and natural to follow `mike@mikedilger.com` - that is all you have to type. Using just this email-like address (email not being involved, we call it a dns id or a nip05 identifier), gossip will go to mikedilger.com, fetch the `.well-known/nostr.json` file, find the entry for `mike` and find the relays for that public key. Then it will go to those relays and pull my recent posts into your feed. Unfortunately as of this writing, I've only encountered one other person who has put relays into their nostr.json file. But I hope it (or something functionally equivalent) catches on eventually.");
            });
            ui.label("\n• Typing them both in");
            ui.indent("helpfollowtype", |ui| {
                ui.label("Entering both a key (either bech32 or hex) as well as at least one relay URL. NOTE: as of this writing, gossip only takes one relay URL when you add a person to follow.");
            });

            ui.add_space(10.0);
            ui.label("Don't worry. You won't have to do this for everybody that you follow. Mostly you just need to do it to get gossip kick-started. As you browse nostr, it collects person-relay associations automatically from many sources: p-tag recommended_relay_url hints, relay lists, contact lists, their events having being successfully found at a relay before, and more. It is planned to also allow you to manage (add, delete) these person-relay associations manually.");

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            ui.heading("Configuring your Identity");
            ui.add_space(10.0);

            ui.horizontal_wrapped(|ui| {
                ui.label("On the");
                if ui.link("You").clicked() {
                    app.set_page(Page::YourKeys);
                }
                ui.label("page you can setup your identity. If you are new, you should just press \"Generate\" and you are good to go. Otherwise you can import a private key in hex or bech32 format, although it isn't very secure to cut-n-paste and display your private key, so it will mark your key security as \"weak\". Hopefully one day soon you'll be able to import a passphrase-protected private key exported from a different client.");
            });
            ui.add_space(10.0);

            ui.label("After generating or importing your key, gossip will save it encrypted under a passphrase. You will need this passphrase to unlock it every time you start gossip. Gossip handles keys securely, never writes them to disk, never displays them (unless you request it to) and zeroing memory that was used for private keys or passphrases before freeing it.");
            ui.add_space(10.0);

            ui.label("If you are just trying Gossip out and not intending to post or react to posts yet, you can just import your public key. This way you'll be able to sync your following list, view your feed and see your replies, but you won't be able to post or react to posts.");
            ui.add_space(10.0);

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            ui.heading("The Feeds");
            ui.add_space(10.0);

            ui.label("Recent events from people you follow should show up on the main [Feed > Following] page in reverse chronological order, without replies or context. If you want to see the context of a post, press the right-arrow on the right hand side of the feed to move to the [Feed > Thread] page. This page should show the full context, and when you enter the thread page it will ask relays questions in order to fill in the thread. But it doesn't always work smoothly.  Sometimes it is missing posts and can't quite get the full context. In fact, there are cases where the post you clicked on isn't even there (known bug). Future development on Gossip will likely improve this greatly.");
            ui.add_space(10.0);

            ui.label("There is a [Feed > Replies] page where you can see replies to your own posts as well as mentions of you.");

            ui.add_space(10.0);
            ui.label("Finally there is a [Feed > Person] page where you can see posts made by a person. To get there, click an avatar, then click [VIEW THEIR FEED].");

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
                    app.set_page(Page::Relays);
                }
                ui.label("page and tick the relays that you intend to post to. If your webserver serves a nostr.json file, you can make the relays that you post to match the contents of the relay portion of that file.");
            });
            ui.add_space(10.0);

            ui.label("Gossip currently does not synchronize this list of relays on the nostr network, so it will not get relay data you use with other clients. The list is local and independent, but this is expected to change.");

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            ui.heading("Posting, Replying, Reacting");
            ui.add_space(10.0);

            ui.label("To post, after unlocking your private key, type in the box at the top of the feed and press Send.");
            ui.add_space(10.0);

            ui.label("To reply, press the reply icon at the bottom of the post you want to reply to. That post will be copied to the top of the page to make it clear what you are replying to. Type your reply and press Send.");
            ui.add_space(10.0);

            ui.label("To quote another post, press the quote icon at the bottom of the post you want to reply to, and a bech32 note string will be added into the edit box.  That will be replaced with the reference to the post you quoted when you press Send.");
            ui.add_space(10.0);

            ui.label("To tag someone, start typing their name into the @username box to the right of the posting box, then press the [@] button below to get a pulldown of matches. Pick the match you wish, and it will add a bech32 npub string to the edit box.  That will be replaced with a tag of the person you are tagging when you press Send.");

            ui.label("To react, you can click the heart. Other kinds of reactions are not yet implemented. You can see other people's reactions below the posts. If you don't like reactions, you can disable this in the settings.");
            ui.add_space(10.0);

            ui.add_space(10.0);

            ui.heading("Things You Can't Do ... Yet");
            ui.add_space(10.0);

            ui.label("We don't yet support editing or publishing your metadata, pulling or publishing your relays, encrypted direct messages, seeing other people's contact lists, seeing your follower count, hiding replies (which could always be spammy), muting people, marking sensitive content, expiration timestamps, delegated event signing, subject tags, or note deletion. But we intend to do all of that. We also don't support chat.");
        });
    } else if app.page == Page::HelpStats {
        stats::update(app, ctx, _frame, ui);
    } else if app.page == Page::HelpAbout {
        about::update(app, ctx, _frame, ui);
    }
}
