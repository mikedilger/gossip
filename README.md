# Gossip

## Gossip is a desktop client for NOSTR

Nostr is an open social media protocol empowering lots of software such as this client. The experience is kind of like Twitter except that you control your own account, and you can post to many different independent places called "relays". People are finding many additional uses for NOSTR that go far beyond micro-blogging or chatting, but this client is focused on those.

Nostr stands for "Notes and Other Stuff Transmitted by Relays."

### Installing

- **Arch Linux**: [`gossip`](https://aur.archlinux.org/packages/gossip) or [`gossip-git`](https://aur.archlinux.org/packages/gossip-git) or [`gossip-bin`](https://aur.archlinux.org/packages/gossip-bin) on the AUR
- **Debian**: See the [releases](https://github.com/mikedilger/gossip/releases) area for a file named something like `gossip-VERSION-ARCH.deb`
- **Any Linux**: See the [releases](https://github.com/mikedilger/gossip/releases) area for a file named something like `gossip.VERSION.AppImage`
- **Microsoft Windows**: See the [releases](https://github.com/mikedilger/gossip/releases) area for a file named something like `gossip.VERSION.msi`
- **MacOS**: See the [releases](https://github.com/mikedilger/gossip/releases) area for a file named something like `gossip-VERSION-Darwin-arm64.dmg` or `gossip-VERSION-Darwin-x86_64.dmg`

or choose to [Build from Source](#building-from-source)

### Points of Difference

The following features make gossip different than most other nostr clients so far:

- **Desktop Portable**: Gossip is designed to run on desktop computers, and is portable to Windows, MacOS and Linux.
- **Gossip Model**: The Gossip Model was named after this client, because gossip never used a simple list of relays. From day one it tried to find posts of people that you follow wherever they are most likely to be, based on those people's relay lists as well as half a dozen other heuristics. Today multiple clients use a similar model, focused around ([NIP-65](https://github.com/nostr-protocol/nips/blob/master/65.md)). Gossip connects to all relays necessary to cover everybody you follow, while also trying to listen to the minimum number of relays necessary to do that (considering that there is overlap, and that people generally post to multiple relays). It also dynamically adjusts to relays being down or disconnecting. For further details: https://mikedilger.com/gossip-model/ 
- **Secure Key Handling**: Gossip handles private keys as securely as reasonable (short of hardware tokens), keeping them encrypted under a passphrase on disk, requiring that passphrase on startup, and zeroing memory. This shouldn't really be a point of difference but few other clients bother.
- **Avoids Browser-Tech**: Gossip avoids web technologies (other than HTTP GET and WebSockets which are necessary for nostr). Web technologies like HTML parsing and rendering, CSS, JavaScript and the very many web standards, are complex and represent a security hazard due to such a large attack surface. This isn't just a pedantic or theoretical concern; people have already had their private key stolen from other nostr clients. We use simple OpenGL-style rendering instead. It's not as pretty but it gets the job done.
- **Performant**: Gossip aims towards being highly performant, using the LMDB database, the rust language, and coding architectures with performance always in mind. Unless you have quite old hardware, the network speed will probably be your bottleneck.
- **High user control**: Gossip has (at the time of writing) 57 different settings. When the right value is uncertain, I pick a reasonable default and give the user the mechanism to change it.
- **Privacy Options**: in case someone wishes to remain secret they should use Gossip over Tor - I recommend using QubesOS do to this. But you could use Whonix or even Tails. Don't just do it on your normal OS which won't do Tor completely. Gossip provides options to support privacy usage such as not loading avatars, not necessarily sharing who you follow, etc. We will be adding more privacy features.

## Media

### Screenshots

![Gossip Screenshot, Default Light Theme](assets/gossip_screenshot_light.png)
![Gossip Screenshot, Default Dark Theme](assets/gossip_screenshot_dark.png)

### nostr features supported

- [x] NIP-01 - Basic protocol flow description
- [x] NIP-02 - Contact List and Petnames
- [ ] NIP-03 - OpenTimestamps Attestations for Events [NOT PLANNED]
- [ ] NIP-04 - Encrypted Direct Message [Read Only is implemented]
- [x] NIP-05 - Mapping Nostr keys to DNS-based internet identifiers
- [ ] NIP-06 - Basic key derivation from mnemonic seed phrase
- [ ] NIP-07 - window.nostr capability for web browsers [NOT APPLICABLE]
- [x] NIP-08 - Handling Mentions
- [x] NIP-09 - Event Deletion
- [x] NIP-10 - Conventions for clients' use of e and p tags in text events
- [x] NIP-11 - Relay Information Document
- [x] NIP-13 - Proof of Work
- [x] NIP-14 - Subject tag in text events
- [x] NIP-18 - Reposts
- [x] NIP-19 - bech32-encoded entities
- [x] NIP-21 - nostr: URL scheme
- [x] NIP-22 - Event created_at Limits
- [ ] NIP-23 - Long-form Content [Optional viewing, but not creating]
- [ ] NIP-24 - Extra metadata fields and tags
- [x] NIP-25 - Reactions
- [x] NIP-26 - Delegated Event Signing
- [x] NIP-27 - Text Note References
- [ ] NIP-28 - Public Chat
- [ ] NIP-30 - Custom Emoji
- [x] NIP-31 - Dealing with Unknown Events
- [ ] NIP-32 - Labeling
- [x] NIP-36 - Sensitive Content
- [ ] NIP-39 - External Identities in Profiles
- [ ] NIP-40 - Expiration Timestamp
- [x] NIP-42 - Authentication of clients to relays
- [ ] NIP-45 - Counting results
- [ ] NIP-46 - Nostr Connect
- [x] NIP-48 - Proxy Tags
- [ ] NIP-50 - Keywords filter
- [ ] NIP-51 - Lists
- [ ] NIP-52 - Calendar Events
- [ ] NIP-53 - Live Activities
- [ ] NIP-56 - Reporting
- [x] NIP-57 - Lightning Zaps
- [ ] NIP-58 - Badges
- [x] NIP-65 - Relay List Metadata
- [ ] NIP-72 - Moderated Communities
- [ ] NIP-78 - Application-specific data
- [ ] NIP-89 - Recommended Application Handlers
- [ ] NIP-94 - File Metadata
- [ ] NIP-98 - HTTP Auth
- [ ] NIP-99 - Classified Listings

## Building from Source

### Step 0 - Possible Reset of Master Branch

If when you pull gossip it doesn't pull cleanly, I may have done a rare force-push. Run these commands to reset your master branch:

````bash
git fetch
git reset --hard origin/master
````

### Step 1 - Install Rust

If you don't already have rust installed, follow the guidance at [rust-lang.org](https://www.rust-lang.org/).

### Step 2 - Install some dependencies

Most dependencies are probably already installed in your base operating system. Here are a few that sometimes aren't:

- build essentials like gcc and make (debian: "build-essential")
- cmake (debian: "cmake")
- pkg-config (debian: "pkg-config")
- openssl (debian: "libssl-dev")
- fontconfig (debian: "libfontconfig1-dev")
- ffmpeg support (debian: libavutil-dev libavformat-dev libavfilter-dev libavdevice-dev libxext-dev libclang-dev)

#### macOS

a. Install rust with rust-up: <https://rustup.rs/>
b. Install homebrew if you don't have it yet <https://brew.sh/>
c. Install these dependencies:

```bash
brew install cmake sdl2 pkg-config ffmpeg
```

### Step 3 - Clone this Repository

````bash
git clone https://github.com/mikedilger/gossip
````

### Step 4 - Compile

````bash
cd gossip
cargo build --release
````

The output will be a binary executable in `target/release/gossip`

This binary should be portable to similar systems with similar hardware and operating system.

If you want a binary optimized for your exact processor with the newest CPU features enabled, and all gossip features enabled:

````bash
<<<<<<< HEAD
RUSTFLAGS="-C target-cpu=native --cfg tokio_unstable" cargo build --release
=======
$ RUSTFLAGS="-C target-cpu=native --cfg tokio_unstable" cargo build --features=lang-cjk,video-ffmpeg --release
>>>>>>> Clarification in the README
````

Everything gossip needs (fonts, icons) is baked into this executable. It doesn't need to find assets. So you can move it and run it from anywhere.

To make the binary smaller,

````bash
strip gossip
````

### Step 5 - Do it all again

The `master` branch changes quickly.  When you want to update, do it all again, something like this:

````bash
git pull
cargo build --release
strip ./target/release/gossip
./target/release/gossip
````

## Compile Options

### TLS

Gossip uses rustls by default. This is an SSL library in rust, which gets compiled into the binary, meaning we won't have issues trying to find your system SSL library or system CA certificates. It also means:

- Gossip will fail to negotiate SSL with servers that don't have any strong ciphersuites. This is a feature, but not one that everybody wants.
- Gossip may not compile on hardware that the `ring` crypto library does not yet support.

If you wish to switch to your native TLS provider, use the following compile options:

````
  --no-default-features --features=native-tls
````

### Language Support

#### Chinese, Japanese and Korean character sets

Gossip by default does not include the CJK font because it is larger than all other languages put together, and most gossip users don't recognize those characters. If you do recognize such characters, you can compile in that font with:

````
  --features=lang-cjk
````

#### Other Non-Latin languages

There are so many of these (172) that it becomes a real pain to add them all. But if you need one, please ask (open an issue) and I'll add it for you.

### Video Playback

You will need to install sdl2 (follow the instructions in the [readme](https://github.com/Rust-SDL2/rust-sdl2/)) and ffmpeg on your system.

Compile with

````
  --features=video-ffmpeg
````

## Known Issues

### Performance issues

If you are having performance issues, please see [PERFORMANCE.md](docs/PERFORMANCE.md).

## Technology Involved

- Rust Language
- egui Rust GUI framework
- LMDB
- Tungstenite websocket library
- Tokio async task runtime
- Serde serialization/deserialization
- Many others

## License

MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

### Contribution

All contributions welcome, please check the [development guidelines](docs/DEVELOPING.md) before starting to code.

Please join [Gossip Telegram Channel](https://t.me/gossipclient).

Anyone interested in replacing the GUI with something much better, or keeping it as egui but making it much better, would be greatly appreciated.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, shall be licensed as above, without any additional terms or conditions.

## On Nostr

### The official gossip account

nprofile1qqsrjerj9rhamu30sjnuudk3zxeh3njl852mssqng7z4up9jfj8yupqpzamhxue69uhhyetvv9ujumn0wd68ytnfdenx7tcpz4mhxue69uhkummnw3ezummcw3ezuer9wchszxmhwden5te0dehhxarj9ekkj6m9v35kcem9wghxxmmd9uq3xamnwvaz7tm0venxx6rpd9hzuur4vghsz8nhwden5te0dehhxarj94c82c3wwajkcmr0wfjx2u3wdejhgtcsfx2xk

npub189j8y280mhezlp98ecmdzydn0r8970g4hpqpx3u9tcztynywfczqqr3tg8

### Mike Dilger

nprofile1qqswuyd9ml6qcxd92h6pleptfrcqucvvjy39vg4wx7mv9wm8kakyujgpzamhxue69uhhyetvv9ujumn0wd68ytnfdenx7tcprpmhxue69uhkzapwdehhxarjwahhy6mn9e3k7mf0qyt8wumn8ghj7etyv4hzumn0wd68ytnvv9hxgtcprdmhxue69uhkummnw3ezumtfddjkg6tvvajhytnrdakj7qgnwaehxw309ahkvenrdpskjm3wwp6kytcpremhxue69uhkummnw3ez6ur4vgh8wetvd3hhyer9wghxuet59uq32amnwvaz7tmwdaehgu3wdau8gu3wv3jhvtct8l34m

npub1acg6thl5psv62405rljzkj8spesceyfz2c32udakc2ak0dmvfeyse9p35c

You can also my NIP-05 address of `mike@mikedilger.com` which will also hook you up with the relays I post to.

I'd prefer if you trusted `mike@mikedilger.com` higher than my public key at this point in time since key management is still pretty bad. That is the inverse of the normal recommendation, but my private key has not been treated very carefully as I never intended it to be my long-term keypair (it just became that over time).  Also, I fully intend to rollover my keys once gossip supports the key-rollover NIP, whatever that is (or will be).

You can tip me at my Bitcoin Lighting address: <decentbun13@walletofsatoshi.com> == lnurl1dp68gurn8ghj7ampd3kx2ar0veekzar0wd5xjtnrdakj7tnhv4kxctttdehhwm30d3h82unvwqhkgetrv4h8gcn4dccnxv563ep
