# Configuration

## Spam Filter Setup

For spam filter configuration, see [Spam Filter](SPAM_FILTER.md).

## Account > Keys

This is where you can import a key (nsec or ncryptsec) or generate a new one, or delete your
existing one.  It also shows your nprofile which you can share with other people who want to
follow you.

## Account > Profile

Here is where you set your profile. Editing these fields will change your profile on nostr.

## Settings

Settings are managed from within the application.  Use the `Settings` in the left hand menu

### Identity

**scrypt N** parameter indicates how long an encrypted private key takes to decrypt.

18 is the default. 19 is twice as long. 20 is four times as long.  The difficulty is 2^N and
you are setting N.

**Login at startup** Tick this to prompt for login at startup.

### Ui

**Highlight unread events** This highlights events you haven't read yet. This data is not shared
on nostr, so it doesn't sync with other clients.

**Show posting area at the top** If this is set, the posting area will be at the top.  Otherwise it
will be at the bottom.

**Order feed with newest at bottom** If this is set, the newest events will be at the bottom of the
scroll area.  Otherwise the newest events will be at the top.

**Theme** Click the button (Dark or Light) to chenge the theme. It will take effect when you save.

**Override DPI** To adjust zoom level, tick the box and slide the slider or enter a DPI number.
Then press [Test (without saving)] to see the results immediately without saving them, and save
once you have them like you want them.

**Enable WGPU** Enable this renderer if gossip doesn't work with the default renderer, although you
will probably have to do this from the [command line](COMMANDS.md).

**Maximum FPS** Gossip uses an immediate mode renderer. Every frame refresh redraws everything,
which recomputes all the logic required to redraw everything.  For this reason, it is recommended
to use a lowish framerate of about 10 FPS.

**Show DEBUG statistics** This shows extra data in your sidebar.

**Inertial Scrolling** This causes scrolling keep going but to quickly slow down and stop after you
have stopped scrolling with the mouse or touchpad. If it feels or behaves oddly, disable it.

**Mouse Acceleration** This adjusts mouse acceleration.

### Content

#### Feed Settings

**Load How Many More**  When you press Load More at the bottom of a feed, this is how many more
it will load.

**Recompute feed periodically**  Default is on. Otherwise you will have a refresh button and you
can refresh manually.

**Recompute feed every (ms)**  If you are recomputing periodically, how often to do it.

**Initially scroll to the highlighted note when entering a thread**

#### Event Selection Settings

**Enable reactions**  This downloads and shows reaction counts

**Enable zap receipts**  This downloads and shows zap totals

**Enable reposts**  This shows reposted notes

**Show direct messages**  This shows direct messages

**Show long-form posts**  This shows long form posts as plain text.  Gossip does not support creating these.

#### Spam Settings

**Avoid spam from unsafe relays (SpamSafe)**  With this enabled, you can mark relays that filter spam as spamsafe. Only events by people you follow will be downloaded from relays not marked as spamsafe.

**Apply spam filtering script to incoming events**  This will apply the filter.rhai script (if found) to incoming events, and if it DENYs, the event won't be saved.

**Apply spam filtering script to thread replies**  This will apply the filter.rhai script (if found) to events rendered in a thread, and if it DENYs, the event won't be displayed.

**Apply spam filtering script to inbox**  This will apply the filter.rhai script (if found) to events rendered in your inbox, and if it DENYs, the event won't be displayed.

**Apply spam filtering script to the global feed**  This will apply the filter.rhai script (if found) to events rendered in a global feed, and if it DENYs, the event won't be displayed.

#### Event Content Settings

**Render mentions inline**  Renders mentioned posts inside of the posts mentioning them (otherwise just a link)

**Render all media inline automatically**  Renders media inside of the post (otherwise just a link)

**Approve all content-warning tagged media automatically**

**Hide muted events entirely, including replies to them**  Warning that this can be frustrating as sometimes thread views will not load.

**Render delete events, but labeled as deleted**  This renders events that were deleted, but marks them as deleted with strikethrough.

### Network

#### Network Settings

**offline mode**

**fetch avatars**

**fetch media**

**check NIP-05**

**Automatically fetch metadata**

**Require approval before connecting to new relays**

**Require approval before AUTHenticating to new relays**

#### Relay Settings

**Number of relays to query per person**  This specifies the level of redundancy you want when following somebody's content. The default is 2.

**Maximum following feed relays**  This puts a cap on how many relays to connect to simultaneously for following people's content.

#### HTTP Fetch Settings

**HTTP connect timeout**

**HTTP idle timeout**

**Max simultaneous HTTP requests per remote host**

**How long to avoid contacting a host after a minor error**

**How long to avoid contacting a host after a medium error**

**How long to avoid contacting a host after a major error**

#### Websocket Settings

**Maximum websocket message size**

**Maximum websocket fram esize**

**Accept unmasked websocket frames**

**Websocket connect timeout**

**Websocket ping frequency**

#### Stale Time Settings

**How long before a relay list becomes stale and needs rechecking**

**How long before metadata becomes stale and needs rechecking**

**How long before valid nip05 becomes stale and needs rechecking**

**How long before invalid nip05 becomes stale and needs rechecking**

**How long before an avatar image becomes stale and needs rechecking**

**How long before event media becomes stale and needs rechecking**

### Posting

**Proof of Work**  How many leading zero-bits to generate in the ID, indicating how much proof of work to do when posting a note.

**Add Tag client-gossip**  Adds a tag indicating your events were created with gossip

**Send User-Agent**  Sends a user agent string to relays

### Storage

**How long to keep events** Events newer than this won't be pruned

**How long to keep downloaded files** Files newer than this won't be pruned

**Delete Old Events Now** This removes old events

**Delete Unused People Now** This cleans up the database. Each spam event creates a person record, when spam is deleted that person record lingers.

**Delete Old Downloaded Files** This removes old files
