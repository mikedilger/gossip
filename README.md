# Gossip

Gossip is a desktop client for nostr.

Nostr is a social media protocol and ecosystem, kind of like Twitter, Mastodon, Gab, Post, Gettr, Farcaster, Truth social, BlueSky, Locals, Minds, Spoutable, etc, etc.... except that you control your own account and nobody can silence you so long as some relay operator somewhere allows you to post.

Nostr stands for "Notes and Other Stuff Transmitted by Relays."

## Status

Gossip is currently pre-alpha code and not ready for use.

If you want to use it anyway, please read the [Alpha Setup](#alpha-setup) section.

## Features

- Portable design intended for the desktop: This is intended to run on desktop computers, but not limited as such. The UI will run on anything that runs one of these backends: OpenGL (glium, glow), OpenGL ES (glow, wgpu), WebGL (glow), Vulkan (wgpu), Metal (wgpu), DirectX 11/12 (wgpu), Browsers (WebAssembly). The platform must be supported by rust (most are), and SQLite3 needs to store its file somewhere.
- High-enough performance: the network speed should be your limiting factor on performance, not the UI or any other part of the code. It doesn't matter how fast the code runs as long as it is always faster than the network, and I think that's definitely true for gossip.
- High user control: the plan is for the user to be in control of quite a lot of settings regarding which posts they see, which relays to talk to, and when to fetch from them, but with some sane defaults so you don't have to change anything.

### nostr features supported

We intend to support the following features/NIPs:

- [x] NIP-01 - Basic protocol flow description
- [ ] NIP-02 - Contact List and Petnames
- [ ] NIP-05 - Mapping Nostr keys to DNS-based internet identifiers (partial)
- [ ] NIP-08 - Handling Mentions
- [x] NIP-09 - Event Deletion
- [x] NIP-10 - Conventions for clients' use of e and p tags in text events
- [ ] NIP-11 - Relay Information Document (partial)
- [ ] NIP-12 - Generic Tag Queries
- [ ] NIP-13 - Proof of Work
- [ ] NIP-14 - Subject tag in text events (partial)
- [x] NIP-15 - End of Stored Events Notice
- [ ] NIP-16 - Event Treatment
- [ ] NIP-19 - bech32-encoded entities (partial)
- [x] NIP-20 - Command Results
- [ ] NIP-22 - Event created_at Limits
- [x] NIP-25 - Reactions
- [ ] NIP-26 - Delegated Event Signing
- [ ] NIP-28 - Public Chat
- [ ] NIP-35 - User Discovery
- [ ] NIP-36 - Sensitive Content
- [ ] NIP-40 - Expiration Timestamp

We do not intend to support the following features/NIPs:

- NIP-03 - OpenTimestamp Attestations for Events: We handle such events, but we do nothing about the ots fields in them.
- NIP-04 - Encrypted Direct Message: I doesn't believe this is a good idea to do encrypted messaging this way, as it leaks metadata and has a cryptographic weakness.
- NIP-06 - Basic key derivation from mnemonic seed phrase. This is probably not applicable anyways.
- NIP-07 - window.nostr capability for web browsers. This is not applicable.

### other features worth mentioning

- [x] threaded or linear
- [x] configurable look-back time
- [x] dark/light mode
- [ ] semi-secure handling of private keys by zeroing memory and marking them Weak if displayed or exported (partial)
- [ ] exporting/importing of private keys with a passphrase (partial)
- [ ] multiple identities
- [ ] user management of relays (read/write), including ranking
- [ ] choose to load from another relay with a button press
- [ ] choose what posts to see beyond direct posts of people you follow: replies, events replied to, posts liked by people you follow, post made by friends of friends, global on a relay, or global.
- [ ] mute someone
- [ ] mute a message
- [ ] dismiss a message without blocking for future sessions
- [ ] follow people privately or publicly

## Building and Installing

### Step 1 - Install Rust

If you don't already have rust installed, follow the guidance at [rust-lang.org](https://www.rust-lang.org/).

### Step 2 - Clone this Repository

````bash
$ git clone https://github.com/mikedilger/gossip
````

### Step 3 - Compile

````bash
$ cd gossip
$ cargo build --release
````

The output will be a binary executable in `target/release/gossip`

Everything gossip needs (fonts, icons) is baked into this executable. It doesn't need to find assets. So you can move it and run it from anywhere.

To make the binary smaller

````base
$ strip gossip
````

This binary should be portable to similar systems with similar hardware and operating system.

If you want a binary optimized for your exact processor with the newest features enabled:

````bash
$ RUSTFLAGS="-C target-cpu=native --cfg tokio_unstable" cargo build --release
````

## Alpha Setup

While gossip is in alpha, you will need to do a few things manually to get it started (do this after the [Building and Installing]([building-and-installing) section above):

- SQLite3 to your gossip.sqlite file (see the About page to find where it is)
- Insert people in the person table with followed=1
- Insert at least one person_relay entry for each person
- There may be other steps needed. Development is happening fast. Feel free to ask a question
  by opening a GitHub issue.

After that, it should start following those people. You may still need to restart from time to
time as it loses connections to relays still, and some live event handling is less thorough
than startup event handling is.


## Technology Involved

- Rust Language
- egui Rust GUI framework
- SQLite 3
- Tungstenite websocket library
- Tokio async task runtime
- Serde serialization/deserialization
- Many others

## License

 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, shall be licensed as above, without any additional
terms or conditions.

## Follow me on Nostr

My public key is ee11a5dff40c19a555f41fe42b48f00e618c91225622ae37b6c2bb67b76c4e49

You can also my NIP-05/NIP-35 address of `mike@mikedilger.com` which will also hook you up with the relays I post to.

Note: I will rollover my public key once gossip is my daily driver.

## Tips

You can tip me at my Bitcoin Lighting address (lud16): lnurl1dp68gurn8ghj7ampd3kx2ar0veekzar0wd5xjtnrdakj7tnhv4kxctttdehhwm30d3h82unvwqhkgetrv4h8gcn4dccnxv563ep

You can also do that with the [Damus](https://damus.io) iOS nostr app (not yet available in gossip).
