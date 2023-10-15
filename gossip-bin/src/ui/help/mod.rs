use super::{GossipUi, Page};
use eframe::egui;
use egui::{Context, Ui};
use gossip_lib::{FeedKind, PersonList};

mod about;
mod stats;
mod theme;

pub(super) fn update(app: &mut GossipUi, ctx: &Context, _frame: &mut eframe::Frame, ui: &mut Ui) {
    if app.page == Page::HelpHelp {
        ui.add_space(10.0);
        ui.heading("Help - Getting Started");
        ui.add_space(12.0);
        ui.separator();

        app.vert_scroll_area().show(ui, |ui| {
            ui.add_space(10.0);

            ui.heading("Existing Nostr Users:");

            ui.add_space(10.0);
            ui.label("If you have used other nostr clients, here is how to get started:");
            ui.add_space(10.0);

            ui.label("• HINT: Use the Back button in the Upper Left to come back to this page.");
            ui.add_space(10.0);

            ui.horizontal_wrapped(|ui| {
                ui.label("• HINT: If this text is too small, click on");
                if ui.link("Settings").clicked() {
                    app.set_page(Page::Settings);
                }
                ui.label("and under the User Interface section, check \"Override DPI\" and set the value higher. You can press [Try it now] to see if you like it, and [SAVE CHANGES] to save that setting for next time.");
            });
            ui.add_space(10.0);

            ui.label("• HINT: Use CTRL-V to paste. Other unix-style pastes probably won't work.");
            ui.add_space(10.0);

            ui.label("• Setup your Identity");
            ui.indent("quickstartaddkey", |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("On the");
                    if ui.link("You").clicked() {
                        app.set_page(Page::YourKeys);
                    }
                    ui.label("page, add your key (public or private). If you supply your private key you will need to set a passphrase on it, and you will be able to post. But you can just use your public key if you only want to view other people's posts. You can even supply somebody else's public key and see what they see (sneaky you!)");
                });
            });
            ui.add_space(10.0);

            ui.label("• Configure your Relays");
            ui.indent("quickstartrelays", |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("On the");
                    if ui.link("Relays > Configure").clicked() {
                        app.set_page(Page::RelaysKnownNetwork);
                    }
                    ui.label("page, add a few relays that you post to and read from, and tick the \"Read\" and \"Write\" columns appropriately.\n\nWRITE RELAYS: These are used for writing posts, and for reading back your posts including your RelayList and ContactList, which we will need to get started. You should have published these from a previous client, and you should specify a relay that has these on it.\n\nREAD RELAYS: These are used to find other people's RelayList (including those embedded in ContactList events), as a fallback for users that gossip has not found yet, and more. Once gossip learns where the people you follow post, it will pick up their posts from their write relays rather than from your read relays. The more read relays you configure, the better chance you'll find everybody.");
                });
            });
            ui.add_space(10.0);

            ui.label("• Pull your Contacts List");
            ui.indent("quickstartpullcontacts", |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("On the");
                    if ui.link("People > Followed").clicked() {
                        app.set_page(Page::PeopleList);
                    }
                    ui.label("page, press [↓ Pull ↓ Overwrite] to pull down the people you follow. They won't have metadata just yet.");
                });
            });
            ui.add_space(10.0);

            ui.label("• Watch the Live Relays");
            ui.indent("quickstartliverelays", |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("On the");
                    if ui.link("Relays > Live").clicked() {
                        app.set_page(Page::RelaysActivityMonitor);
                    }
                    ui.label("page, watch the live connections. Press [Pick Again] if connections aren't being made. If people aren't being found, you may need to add different relays and try this again. Watch the console output to see if gossip is busy and wait for it to settle down a bit.");
                });
            });
            ui.add_space(10.0);

            ui.label("• Update Metadata");
            ui.indent("quickstartliverelays", |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Back on the");
                    if ui.link("People > Followed").clicked() {
                        app.set_page(Page::PeopleList);
                    }
                    ui.label("page, once the relay picking has settled down, press [Refresh Metadata]. Then give it some time. It might not be able to find everybody just yet.");
                });
            });
            ui.add_space(10.0);

            ui.label("• Enjoy the Feed");
            ui.indent("quickstartliverelays", |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("On the");
                    if ui.link("Feed > Following").clicked() {
                        app.set_page(Page::Feed(FeedKind::List(PersonList::Followed, app.mainfeed_include_nonroot)));
                    }
                    ui.label("page, enjoy the feed.");
                });
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            ui.heading("New Nostr Users:");

            ui.add_space(10.0);
            ui.label("If you are new, here is how to get started:");
            ui.add_space(10.0);

            ui.label("• HINT: Use the Back button in the Upper Left to come back to this page.");
            ui.add_space(10.0);

            ui.horizontal_wrapped(|ui| {
                ui.label("• HINT: If this text is too small, click on");
                if ui.link("Settings").clicked() {
                    app.set_page(Page::Settings);
                }
                ui.label("and under the User Interface section, check \"Override DPI\" and set the value higher. You can press [Try it now] to see if you like it, and [SAVE CHANGES] to save that setting for next time.");
            });
            ui.add_space(10.0);

            ui.label("• HINT: Use CTRL-V to paste. Other unix-style pastes probably won't work.");
            ui.add_space(10.0);

            ui.label("• Setup your Identity");
            ui.indent("quickstartaddkey", |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("On the");
                    if ui.link("You").clicked() {
                        app.set_page(Page::YourKeys);
                    }
                    ui.label("page, in the top section \"Generate a Keypair\" set a password (twice) and click [Generate Now].");
                });
            });
            ui.add_space(10.0);

            ui.label("• Configure your Relays");
            ui.indent("quickstartrelays", |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("On the");
                    if ui.link("Relays > Configure").clicked() {
                        app.set_page(Page::RelaysKnownNetwork);
                    }
                    ui.label("page, add a few relays that you post to and read from, and tick the \"Read\" and \"Write\" columns appropriately. You will need to search the Internet for nostr relays as we don't want to give special mention to any in particular.\n\nWRITE RELAYS: These are used for writing posts, and for reading back your posts including your RelayList and ContactList whenever you move clients.\n\nREAD RELAYS: These are used to find other people's RelayList (including those embedded in ContactList events), as a fallback for users that gossip has not found yet, and more. Once gossip learns where the people you follow post, it will pick up their posts from their write relays rather than from your read relays. The more read relays you configure, the better chance you'll find everybody.");
                });
            });
            ui.add_space(10.0);

            ui.label("• Follow somebody");
            ui.indent("quickstartpullcontacts", |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("On the");
                    if ui.link("People > Follow Someone New").clicked() {
                        app.set_page(Page::PeopleFollow);
                    }
                    ui.label("page, follow somebody.");
                });
            });
            ui.add_space(10.0);

            ui.label("• Watch the Live Relays");
            ui.indent("quickstartliverelays", |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("On the");
                    if ui.link("Relays > Live").clicked() {
                        app.set_page(Page::RelaysActivityMonitor);
                    }
                    ui.label("page, watch the live connections. Press [Pick Again] if connections aren't being made. If people aren't being found, you may need to add different relays and try this again. Watch the console output to see if gossip is busy and wait for it to settle down a bit.");
                });
            });
            ui.add_space(10.0);

            ui.label("• Update Metadata");
            ui.indent("quickstartliverelays", |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("On the");
                    if ui.link("People > Followed").clicked() {
                        app.set_page(Page::PeopleList);
                    }
                    ui.label("page, once the relay picking has settled down, press [Refresh Metadata]. Then give it some time. It might not be able to find everybody just yet.");
                });
            });
            ui.add_space(10.0);

            ui.label("• Enjoy the Feed");
            ui.indent("quickstartliverelays", |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("On the");
                    if ui.link("Feed > Following").clicked() {
                        app.set_page(Page::Feed(FeedKind::List(PersonList::Followed, app.mainfeed_include_nonroot)));
                    }
                    ui.label("page, enjoy the feed.");
                });
            });

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(10.0);

            ui.heading("The Feeds");
            ui.add_space(10.0);

            ui.label("Recent events from people you follow should show up on the main [Feed > Following] page in reverse chronological order, without replies or context. If you want to see the context of a post, press the right-arrow on the right hand side of the post to move to the [Feed > Thread] page. This page should show the full context, and when you enter the thread page it will ask relays questions in order to fill in the thread. But it doesn't always work smoothly.  Sometimes it is missing posts and can't quite get the full context. In fact, there are cases where the post you clicked on isn't even there (known bug). Future development on Gossip will likely improve this greatly.");
            ui.add_space(10.0);

            ui.label("There is a [Feed > Inbox] page where you can see replies to your own posts, posts that mention you, and (in the future) DMs.");

            ui.add_space(10.0);
            ui.label("Finally there is a [Feed > Person] page where you can see posts made by a person. To get there, click their name and then in the dropdown menu click [View Their Posts].");

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
            ui.add_space(10.0);

            ui.label("To react, you can click the heart. Other kinds of reactions are not yet implemented. You can see other people's reactions below the posts. If you don't like reactions, you can disable this in the settings.");
            ui.add_space(10.0);

            ui.add_space(10.0);
        });
    } else if app.page == Page::HelpStats {
        stats::update(app, ctx, _frame, ui);
    } else if app.page == Page::HelpAbout {
        about::update(app, ctx, _frame, ui);
    } else if app.page == Page::HelpTheme {
        theme::update(app, ctx, _frame, ui);
    }
}
