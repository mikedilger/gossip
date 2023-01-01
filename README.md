# Gossip

Gossip is a desktop client for nostr.

Nostr is a social media protocol and ecosystem, kind of like Twitter, Mastodon, Gab, Post, Gettr, Farcaster, Truth social, BlueSky, Locals, Minds, Spoutable, etc, etc.... except that you control your own account and nobody can silence you so long as some relay operator somewhere allows you to post. People are finding many additional uses for nostr that go far beyond just chatting, but this client is focused on chatting.

Nostr stands for "Notes and Other Stuff Transmitted by Relays."

## Status

Gossip is currently early alpha-quality code. As of right now you can (if you aren't stopped by some bug):

- [x] Follow people by user@domain (NIP-35) or by public key (hex or bech32) plus a relay where they can be found.
- [x] See the feed of posts by those people, threaded or not, including reactions. But it's ugly and not very complete.
- [x] User-control of when to search for missing events (referred to by events you have), and a display of how many are missing.
- [x] Generate or import (hex or bech32) a private key (your identity) and it is kept by the client encrypted under a password that you need to unlock it every time you start the client.
- [x] Choose relays to post to (from among the relays that the client has seen -- it doesn't yet allow you to enter one)
- [x] Post root-level text messages (but not in reply to, nor reactions)

## Development Ideology

- **Portable** design intended for the **desktop**: This is intended to run on desktop computers, but not limited as such. The platform must be supported by rust (most are), and SQLite3 needs to store its file somewhere. The UI will run on anything that runs one of these backends:
    - OpenGL (via glium or glow)
    - OpenGL ES (via glow or wgpu)
    - WebGL (via glow)
    - Vulkan (via wgpu)
    - Metal (via wgpu)
    - DirectX 11/12 (via wgpu)
    - Browsers (via WebAssembly)
- **High-enough performance**: the network speed should be your limiting factor on performance, not the UI or any other part of the code. It doesn't matter too much how fast the code runs as long as it is always faster than the network, and I think that's definitely true for gossip. However due to our use of an immediate-mode renderer which computes and redraws frequently, we will continue to try to minimize the CPU impact of that hot loop.
- **High user control**: the plan is for the user to be in control of quite a lot of settings regarding which posts they see, which relays to talk to, and when to fetch from them, but with some sane defaults so you don't have to change anything.
- **Privacy Options**: in case someone wishes to remain secret they should use Gossip over Tor - I recommend using QubesOS do to this. But you could use Whonix or even Tails. Don't just do it on your normal OS which won't do Tor completely. Gossip will provide options to support privacy usage such as not loading avatars, having multiple identities, etc.
- **Key Security**: private keys need to be handled as securely as possible. We store the key encrypted under a passphrase on disk, and we zero out any memory that has seen either the key or the password that decrypts it. We also keep the decrypted key in just one place, the Signer, which doesn't provide access to the key directly. Eventually we will look to add hardware token support, probably first using programmable [Solo keys](https://solokeys.com/) because I have a half dozen of those.

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
- [ ] NIP-13 - Proof of Work
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
- [x] dismiss a message without blocking for future sessions
- [ ] follow people privately or publicly

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
