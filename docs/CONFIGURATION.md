# Configuration

Gossip was designed to be flexible and non-opinionated, so that you could do nostr your way.
However, this means that gossip has a lot of settings, and these can be daunting to deal with.
This document talks about how to configure gossip and get it to run the way you want it to
run.

These are the main kinds of configurations you can make to gossip (the last two are arguably
not configurations but this is a handy place to document them for now).

* [Configuring Your Identity](#configuring-your-identity)
* [Configuring Relays](#configuring-relays)
* [Configuring Spam Avoidance](#configuring-spam-avoidance)
* [Configuring Person Lists](#configuring-person-lists)
* [Configuring Handler Applications](#configuring-handler-applications)
* [Configuring Settings](#configuring-settings)
* [Managing Events](#managing-events)
* [Periodic Maintenance](#periodic-maintenance)

## Configuring Your Identity

When you first setup gossip, a wizard walks you through some basic setup, such as generating
a new identity or importing an existing one.  After that, you can configure your identity
from the `Account` menu on the left.

### Your Keys

`Account > Keys` is where you can view your npub, nprofile, and ncryptsec (encrypted private key) scan QR
codes for them.

You can also change your password, and export your private keys if you want to.

You can also DELETE your identity and then either generate a new account or import a different one.

Gossip does not yet support using a remote NIP-46 remote signer (bunker) to manage your private key.
But it operates as one and this can be configured under `Account > Nostr Connect`.

### Multiple Accounts

The gossip GUI operates with just one account. However, you can run multiple accounts by setting
environment variables.  If you set `GOSSIP_PROFILE` to anything, that will become a subdirectory
in the gossip data directory and each gossip profile gets it's own database, so not only can it
run as a separate account, but it gets all separate settings too.

You can also set `GOSSIP_DIR` to point to a different directory. In that case, not only do you
get a different database, you also don't share the same file cache, user interface memory, or
spam filter script.

Of course experienced unix users can just run gossip under a different unix account for the
same effect.

### Delegation

We support NIP-26 delegation under `Account > Delegation` but as far as the author is aware,
nobody uses this, it hasn't been tested for a long time, and there are nostr developers who
argue that NIP-26 should be [deprecated](https://github.com/nostr-protocol/nips/pull/1051).

### Your Profile

Yo can manage your profile at `Account > Profile`. Not only can you edit any field, you can make
up new fields. When you save, the new profile event will be published to your nostr relays.


## Configuring Relays

THIS IS VERY IMPORTANT.

When you first setup gossip, a wizard walks you through some basic setup including selecting
a few inbox, outbox and discovery relays. After that, you can configure your relays under
`Relays`.

There are three items here, `Active Relays`, `My Relays`, and `Known Network`
the only difference being that the first two filter out a bunch of relays.

`Known Network` gives you everything and in that case, very importantly you will want to adjust
how you see this list by tweaking the settings at the top.

* Sort by `Default` puts your connected relays at the top, whereas sort by `Score` ignores which
  ones are connected or configured.
* Filter `Configured` shows all your configured relays, and `All` shows all relays

To edit a relay's configuration, click the row. The main settings are the switches on the right

* _Read_ -- This makes a relay a private/hidden inbox. It will function like an inbox, but not be
  advertised as such. Gossip will look for events referencing you on this relay.
* _Inbox_ -- Just like Read, but also advertised so other clients know to deposit events tagging
  you on that relay. I recommend you have 3 or 4 of these.
* _Write_ -- This makes a relay a private/hidden outbox. It will function like an outbox, but not
  be advertised as such. Gossip will post your events here.
* _Outbox_ -- Just like Write, but also advertised so other clients know to get your events from
  this relay. I recommend you have 3 to 5 of these.
* _Discover_ -- This means the relay will be used to find other people's relay lists.
* _Spam Safe_ -- If you use a certain spam setting, this means you trust this relay to filter
  spam and so gossip will fetch replies from anybody off of this relay (as opposed to only the
  people that you follow).
* _Direct Message_ - This is like an inbox, but only for DMs.
* _Global feed_ - This means when you view the global feed, it will include events from this
  relay. The more you choose, the busier your global feed will be.
* _Search_ - This means when you do a Search Relays, that search will be sent to this relay.
  You might have a few of these, or maybe you can have just one if you find a really good one.

Before choosing a relay as an inbox or outbox, you should press `Test` to run a quick test
to see if that relay accepts events from you (for outbox), and if it accepts events from
anybody (for inbox, only if you care to receive events from strangers, which may bring in
spam).

Changes are made immediately to the local data, but only published when you advertise.

VERY IMPORTANT: After you make changes, open the upper-right menu and click "Advertise Relay List".
(If it is not there, go to `My Relays` and try again). Only click it once, and then watch the
console output as gossip contacts hundreds of relays to publish your relay list. Many relays
will reject it (not supporting NIP-65) but enough will accept it that people will
hopefully be able to find you. After a few minutes it should complete.

FYI: There is also a `View Feed` link here to view the relay's feed.

## Configuring Spam Avoidance

There are two major strategies for avoiding spam.

### Trusting Relays

You can avoid spam by trusting relays to do the spam filtering for you, and then marking
which relays you trust.

First, you have to tick the setting under `Settings > Content > Spam Settings` that
reads "Avoid spam from unsafe relays (SpamSafe)".

Then you need to (optionally) mark some relays as `Spam Safe` in the relay configuration.

With this setup, all events are loaded normally from Spam Safe relays, but from all the
other relays only events from people you follow will be loaded.

I don't use this setup and there may be multiple "leaks" whereby events that come from
unsafe relays from people you don't follw get into gossip anyways. These are technically
bugs so file issues about them and I will track them down and fix them.

### Using a Spam Filter Script

The gossip distribution (and source code) comes with a file called `filter.example.rhai`.

To use it, you need to copy it into your gossip directory and rename it to `filter.rhai`.
Your gossip directory starts [here](https://docs.rs/dirs/latest/dirs/fn.data_dir.html) and
is the subdirectory _"gossip"_.

You also need to turn on some settings indicating when to call it under `Settings > Content > Spam Settings`. See [Spam Settings](#spam-settings).

The script works out of the box, but you may wish to edit it.  There are comments at the
top explaining how it works.

I use the default script and I only tick `Apply spam filtering script to incoming events`.
I find this works very well for me. I never see spam. The only tweak I made is that I added a
few extra pubkeys that I don't want to hear from to the `filter_known_spam` function.

If you already have spam in your local database and don't want to see it, you also might
want to tick `Apply spam filtering script to thread replies` and
`Apply spam filtering script to inbox`.

## Configuring Person Lists

By default everybody gets a `Followed` feed that includes events from everybody that they
follow.  If you want feeds for smaller groups of people (e.g. "Priority" or "Bitcoin" or
"Politics" or whatever) you'll need to define those groups.  This can be done under the
`People Lists` menu.

Here you can create a new list, or manage the people in a list by clicking it's name.

You can choose to add people to lists privately too.

Synchronizing these lists is manual and is done with the buttons at the top.

Each list you create generates a Feed menu for that list only if the list is marked as
a favorite.

## Configuring Handler Applications

Nostr has many kinds of events. Gossip isn't always the best program for dealing with them
all. So gossip supports NIP-89 Recommended Application Handlers.  When any of the people
you follow publish a handler recommendation, gossip adds that handler automatically.

To view an event with a handler, click the event's hamburger menu on the right, go to `Open With`
and then click the handler. This will open the event in your web browser using that handler.

You can manage these handlers under the `Handlers` menu based on event kinds. You can turn
off handlers you don't want to show in the menu, and you can recommend handlers to your
followers.

You can also import a handler from the application author's handler event by pasting the
naddr of it. Gossip doesn't create these application handler events; consult the author of
the application and ask if they have created one.

## Configuring Settings

Settings are managed from within the application.  Use the `Settings` in the left hand menu

Perhaps suprisingly, all these detailed settings under the "Settings" menu are not very
important. Most people can mostly ignore them. The defaults are pretty good.

However they can become important if you tweak them in severe ways. Therefore, we made it
easy to return to the defaults. Every setting that you have changed from the default has
a "↶" Reset button (except for three minor exceptions). In case you make a bunch of tweaks
to see what happens and then forget which ones you messed up, you don't have to start over
from scratch you can just find and press the "↶" Reset buttons.

### Identity

**scrypt N** parameter indicates how long an encrypted private key takes to decrypt.

18 is the default. 19 is twice as long. 20 is four times as long.  The difficulty is 2^N and
you are setting N.

**Login at startup** Tick this to prompt for login at startup. Otherwise you will have to
press an unlock button and login to post anything.

### Ui

**Highlight unread events** This highlights events you haven't read yet. This data is not shared
on nostr, so it doesn't sync with other clients.

**Show posting area at the top** If this is set, the posting area will be at the top. Otherwise it
will be at the bottom (the top is the default now).

**Order feed with newest at bottom** If this is set, the newest events will be at the bottom of the
scroll area.  Otherwise the newest events will be at the top.

**Theme** Click the button (Dark or Light) to chenge the theme. It will take effect when you save.
The pulldown only has `Default` now (old themes were not updated and have been removed). You can click `Follow OS dark-mode` if you want; there are problems saving if you change any two of these at once (outstanding bug).

**Override DPI** To adjust zoom level, tick the box and slide the slider or enter a DPI number.
Then press [Test (without saving)] to see the results immediately without saving them, and save
once you have them like you want them.

**Enable WGPU** Enable this renderer if gossip doesn't work with the default renderer, although you
will probably have to do this from the [command line](COMMANDS.md).

**Maximum FPS** Gossip uses an immediate mode renderer. Every frame refresh redraws everything,
which recomputes all the logic required to redraw everything.  For this reason, it is recommended
to use a lowish framerate of about 12 FPS. But if you have a powerful computer and want a smoother experience you can set it higher.

**Show DEBUG statistics** This shows extra data in your sidebar. I use these myself they are quite entertaining.

**Inertial Scrolling** This causes scrolling keep going but to quickly slow down and stop after you
have stopped scrolling with the mouse or touchpad. If it feels or behaves oddly, disable it, because some touchpads behave quite differently and interact badly with this setting.

**Mouse Scroll-Wheel Acceleration** This adjusts mouse acceleration for the scroll wheel only.

### Content

#### Feed Settings

**Load How Many More**  When you press Load More at the bottom of a feed, this is how many more
it will load.

**Recompute feed periodically**  Default is on. Otherwise you will have a refresh button and you
can refresh manually. In either case the feed will not redraw until you press something, but
with this turned off it won't be computing in the background either and won't alert you when
new events come in.

**Recompute feed every (ms)**  If you are recomputing periodically, how often to do it.

**Initially scroll to the highlighted note when entering a thread**  When you click the circle
button to view a post in it's context, this will make the feed scroll to the note you are
viewing in context.

#### Event Selection Settings

**Enable reactions**  This downloads and shows reaction counts, and lets you react too.

**Enable zap receipts**  This downloads and shows zap totals and lets you zap.

**Enable reposts**  This shows reposted notes.

**Enable picture events**  Download and display picture events (kind 20, NIP-68)

**Show direct messages**  This doesn't seem to do anything anymore and will probably go away. You will see direct messages regardless.

**Show long-form posts**  This shows long form posts as plain text in the main feed. Gossip does not support creating these and it doesn't interpret the markdown. If you enable this, beware that every edit the author creates will generate a new event in your feed. Default is off.

#### Spam Settings

See also [Configuring Spam Avoidance](#configuring-spam-avoidance)

**Avoid spam from unsafe relays (SpamSafe)**  With this enabled, you can mark relays that filter spam as spamsafe. Only events by people you follow will be downloaded from relays not marked as spamsafe.

**Limit inbox seeking to inbox relays**  With this enabled, only your inboxes will be queried for events that tag you. Be aware that events that tag you might be fetched for other reasons from other relays and this won't stop that.

**Apply spam filtering script to incoming events**  This will apply the filter.rhai script (if found) to incoming events, and if it DENYs, the event won't be saved.

**Apply spam filtering script to thread replies**  This will apply the filter.rhai script (if found) to events rendered in a thread, and if it DENYs, the event won't be displayed.

**Apply spam filtering script to inbox**  This will apply the filter.rhai script (if found) to events rendered in your inbox, and if it DENYs, the event won't be displayed.

**Apply spam filtering script to the global feed**  This will apply the filter.rhai script (if found) to events rendered in a global feed, and if it DENYs, the event won't be displayed.

#### Event Content Settings

**Render mentions inline**  Renders mentioned posts inside of the posts mentioning them (otherwise just a link)

**Render all media inline automatically**  Renders media inside of the post (otherwise just a link)

**Approve all content-warning tagged media automatically**  Instead of showing a content-warning and a button to reveal, all content will simply be shown.

**Hide muted events entirely, including replies to them**  Instead of just marking events as muted, this hides all replies. Warning that this can be frustrating as sometimes thread views will not load.

**Render deleted events, but labeled as deleted**  This renders events that were deleted, but marks them as deleted with strikethrough.

### Network

#### Network Settings

All of these settings in this section can help preserve privacy at the expense of functionality.

**offline mode**  In this mode events won't be fetched or posted. You can start in offline mode at the login screen or with a command line command

**fetch avatars**  Whether or not to fetch avatars.

**fetch media**  Whether or not to fetch media.

**check NIP-05**  Whether or not to check NIP-05 nostr.json files

**Automatically fetch metadata**  Whether or not to automatically fetch a person's metadata periodically

**Require approval before connecting to new relays**  Whether connection to a new relay you have not configured requires your approval.

**Require approval before AUTHenticating to new relays**  Whether authentication to a new relay you have not approved previously requires your approval.

#### Relay Settings

**Number of relays to query per person**  This specifies the level of redundancy you want when following somebody's content. The default is 2.

**Maximum following feed relays**  This puts a cap on how many relays to connect to simultaneously for following people's content.

#### HTTP Fetch Settings

**HTTP connect timeout**  How long to wait for NIP-11, images and videos before giving up.

**HTTP idle timeout**  How long after connecting to wait before disconnecting once a connection goes idle.

**Max simultaneous HTTP requests per remote host**  Maximum number of simultaneous downloads per remote host. This defaults to 3 because I found many hosts will block you if you do more than that at the same time.

**How long to avoid contacting a host after a minor error**

**How long to avoid contacting a host after a medium error**

**How long to avoid contacting a host after a major error**

#### Websocket Settings

**Maximum websocket message size**  This is 1 MiB by default, and I don't recommend changing it.

**Maximum websocket frame size**  This is 1 MiB by default, and I don't recommend changing it.

**Accept unmasked websocket frames**  Such frames are technically invalid but some software uses them. It doesn't hurt.

**Websocket connect timeout**  How long to wait before timing out a websocket connection.

**Websocket ping frequency**  How often to ping a websocket connection to keep it alive.

#### Stale Time Settings

The defaults here are pretty good already.

**How long before a relay list becomes stale and needs rechecking**

**How long before metadata becomes stale and needs rechecking**

**How long before valid nip05 becomes stale and needs rechecking**

**How long before invalid nip05 becomes stale and needs rechecking**

**How long before an avatar image becomes stale and needs rechecking**

**How long before event media becomes stale and needs rechecking**

### Posting

**Undo Secnd seconds**  How long after you send can you undo it

**Proof of Work**  How many leading zero-bits to generate in the ID, indicating how much proof of work to do when posting a note.

**Add Tag client-gossip**  Adds a tag indicating your events were created with gossip

**Send User-Agent**  Sends a user agent string to relays

**Blossom Servers**  Put URLs to your blossom servers here.

### Storage

**How long to keep events** Events newer than this won't be pruned

**How long to keep downloaded files** Files newer than this won't be pruned

**Delete Old Events Now** This removes old events

**Delete Unused People Now** This cleans up the database. Each spam event creates a person record, when spam is deleted that person record lingers.

**Delete Old Downloaded Files** This removes old files


## Managing Events

In your feed, in the event on the upper right, there is a hamburger menu.  This allows you
to manage the event and multiple capabilities are hidden here.

* Copy
  * To Clipboard
  * With QR code
* Bookmark
  * Public
  * Private
* Open with (a handler)
* Copy Event ID
* Inspect
  * Show JSON
  * Copy JSON
  * QR Code Export
  * Rerender
  * Dismiss - this hides the event and all replies until restart.

## Periodic Maintenance

Over time your database will grow. If you don't have plenty of space, you might want to prune it.

You can do this at `Settings > Storage" by pressing the buttons.

Every seven days your database will be compressed on startup. This cannot be configured because
it happens before the database is opened, and your configuration is inside the database.
