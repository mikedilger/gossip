# Gossip

Gossip is a desktop client for nostr.

Nostr is a social media protocol and ecosystem, kind of like Twitter [^1] except that you control your own account and nobody can silence you so long as some relay operator somewhere allows you to post. People are finding many additional uses for nostr that go far beyond just chatting, but this client is focused on chatting.

Nostr stands for "Notes and Other Stuff Transmitted by Relays."

[^1] and Mastodon, Gab, Post, Gettr, Farcaster, Truth social, BlueSky, Locals, Minds, Spoutable, etc, etc....

## Status

Gossip is currently alpha-quality code and I do not recommend using it as your main client at this point. But it is getting close to the point where I will be able to remove that reommendation to not use. So adventurous souls may try it out to see what it will be like.

As of right now you can (if you aren't stopped by some bug):

- [x] **Seeing other people's posts**
    - [x] **Follow people** by user@domain (NIP-35) or by public key (hex or bech32) plus a relay where they can be found, or by finding them in the feed, clicking their avatar, and choosing to follow them on their page... and unfollow people.
    - [x] **See a feed of posts from people you follow** including avatars and other user metadata, and reactions to these posts.
    - [X] **See threads related to a post** including ancestors and replies, although it may not be working as good as it will eventually work just yet.
    - [X] **See a list of posts of a person** on their person page
    - [X] **An ability to query relays for missing referred-to events** by pressing a button.
- [x] **Creting content**
    - [x] Generating a key that is kept securely within the client encrypted under a password that you need to unlock it every time you start the client.
    - [x] Generate or import (hex or bech32) a private key (your identity) (also kept under a password)
    - [x] Exporting your private key encrypted, or decrypted as bech32 or hex
    - [x] Choose relays to post to (from among some starting relays, plus ones the client has seen in events), including entering a relay URL directly.
    - [x] Post root-level text messages
    - [x] Post replies to other people's text messages
    - [x] React to other people's text messages, but only in the simplest way with reaction of "" which is interpreted as a like or upvote.

### Missing Critical Features

- [ ] An inbox of replies to your posts (they are loaded, but you have to go into each feed to find them, they are not collated into an inbox)
- [ ] Setting your metadata and syncing it with the network.
- [ ] Syncing the relays you use with the network
- [ ] Seeing who other people follow (contact lists)
- [ ] Choosing not to see replies and/or reactions to your own posts by people you didn't directly follow
- [ ] Good Emoji support (many are still tofu characters)
- [ ] Quoting and/or boosting posts
- [ ] Muting people
- [ ] Content of posts being rendered well (references, images, videos, etc)
- [ ] NIP-05 backend verification (many are crossed out because it's not trying)
- [ ] Controlling which relays the client may connect to (currently it will dynamically find relays and connect if it thinks those relays have the event it wants, and you can't configure it to not do that)

## Development Ideology

- **High user control**: The plan is for the user to be in control of quite a lot of settings regarding which posts they see, which relays to talk to, and when to fetch from them, but with some sane defaults.
- **Key Security**: Private keys need to be handled as securely as possible. We store the key encrypted under a passphrase on disk, and we zero out any memory that has seen either the key or the password that decrypts it. We also keep the decrypted key in just one place, the Signer, which doesn't provide access to the key directly. Eventually we will look to add hardware token support, probably first using programmable [Solo keys](https://solokeys.com/) because I have a few of those.
- **Portable** design intended for the **desktop**: This is intended to run on desktop computers, but not limited as such. The platform must be supported by rust (most are), and SQLite3 needs to store its file somewhere. The UI will run on anything that runs one of these backends:
    - OpenGL (via glium or glow)
    - OpenGL ES (via glow or wgpu)
    - WebGL (via glow)
    - Vulkan (via wgpu)
    - Metal (via wgpu)
    - DirectX 11/12 (via wgpu)
    - Browsers (via WebAssembly)
- **High-enough performance**: Generally the network speed should be your limiting factor on performance, not the UI or any other part of the code. It doesn't matter too much how fast the code runs as long as it is always faster than the network, and I think that's definitely true for gossip.
- **Easy-ish on CPU/power usage**: We can't achieve this as well as other clients might because we use an immediate-mode renderer which necessarily recomputes what it draws every "frame" and may redraw many times per second. We are working hard to minimize the CPU impact of this hot loop. Try it and see.
- **Privacy Options**: in case someone wishes to remain secret they should use Gossip over Tor - I recommend using QubesOS do to this. But you could use Whonix or even Tails. Don't just do it on your normal OS which won't do Tor completely. Gossip will provide options to support privacy usage such as not loading avatars, having multiple identities, not necessarily sharing who you follow, etc.

### nostr features supported

We intend to support the following features/NIPs:

- [x] NIP-01 - Basic protocol flow description
- [ ] NIP-02 - Contact List and Petnames
- [ ] NIP-05 - Mapping Nostr keys to DNS-based internet identifiers (partial)
- [ ] NIP-04 - Encrypted Direct Message: I doesn't believe this is a good idea to do encrypted messaging this way, as it leaks metadata and has a cryptographic weakness. But it is in common enough usage.
- [ ] NIP-08 - Handling Mentions
- [x] NIP-09 - Event Deletion
- [x] NIP-10 - Conventions for clients' use of e and p tags in text events
- [ ] NIP-11 - Relay Information Document (partial)
- [ ] NIP-12 - Generic Tag Queries
- [x] NIP-13 - Proof of Work
- [ ] NIP-14 - Subject tag in text events (partial)
- [x] NIP-15 - End of Stored Events Notice
- [ ] NIP-16 - Event Treatment
- [ ] NIP-19 - bech32-encoded entities (keys, not elsewise)
- [x] NIP-20 - Command Results
- [ ] NIP-22 - Event created_at Limits
- [ ] NIP-25 - Reactions (viewing, not yet creating)
- [ ] NIP-26 - Delegated Event Signing
- [ ] NIP-28 - Public Chat
- [x] NIP-35 - User Discovery
- [ ] NIP-36 - Sensitive Content
- [ ] NIP-40 - Expiration Timestamp

We do not intend to support the following features/NIPs:

- NIP-03 - OpenTimestamp Attestations for Events: We handle such events, but we do nothing about the ots fields in them.
- NIP-06 - Basic key derivation from mnemonic seed phrase. This is probably not applicable anyways.
- NIP-07 - window.nostr capability for web browsers. This is not applicable.

### other features worth mentioning

- [x] configurable look-back time
- [x] dark/light mode
- [x] secure handling of private keys by zeroing memory and marking them Weak if displayed or exported
- [x] exporting/importing of private keys with a passphrase
- [ ] multiple identities
- [ ] user management of relays (read/write), including ranking (partial, no ranking ui yet)
- [ ] choose to load from another relay with a button press
- [ ] choose what kinds of posts to want to see.
- [ ] block lists, word filters, etc.
- [ ] mute a specific post
- [x] dismiss a specific post without blocking for future sessions
- [ ] follow people privately or publicly (currently entirely private, not synced)

### What Gossip Isn't

Gossip doesn't use web technology (except for Websockets and HTTP fetch). There is no javascript. There is no HTML parsing. There is no HTML-based layout. There is no CSS. Because of this, there are no suprises. There are no XSS vulnerabilities. There are no CORS errors.

On the flip side, we have (currently) shitty fonts, no color emojis, and we render many frames per second which has a computation cost.

This is a trade off that works for the developer, who wants a reliable and secure client, not necessarily a flashy one.

## Building and Installing

### Step 1 - Install Rust

If you don't already have rust installed, follow the guidance at [rust-lang.org](https://www.rust-lang.org/).

### Step 2 - Install some dependencies

Most dependencies are probably already installed in your base operating system. Here are a few that sometimes arent:

- build essentials like gcc and make (debian: "build-essential")
- cmake (debian: "cmake")
- pkg-config (debian: "pkg-config")
- openssl (debian: "libssl-dev")
- fontconfig (debian: "libfontconfig1-dev")

### Step 3 - Clone this Repository

````bash
$ git clone https://github.com/mikedilger/gossip
````

### Step 4 - Compile

````bash
$ cd gossip
$ cargo build --release
````

The output will be a binary executable in `target/release/gossip`

Everything gossip needs (fonts, icons) is baked into this executable. It doesn't need to find assets. So you can move it and run it from anywhere.

To make the binary smaller

````bash
$ strip gossip
````

This binary should be portable to similar systems with similar hardware and operating system.

If you want a binary optimized for your exact processor with the newest features enabled:

````bash
$ RUSTFLAGS="-C target-cpu=native --cfg tokio_unstable" cargo build --release
````

### Step 5 - Do it all again

The `master` branch changes quickly.  When you want to update

````bash
$ git pull
$ cargo build --release
$ strip ./target/release/gossip
$ ./target/release/gossip
````

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
