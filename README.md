# Gossip

## Gossip is a desktop client for NOSTR

Nostr is an open social media protocol empowering lots of software such as this client. The experience is kind of like Twitter except that you control your own account, and you can post to many different independent places called "relays". People are finding many additional uses for NOSTR that go far beyond micro-blogging or chatting, but this client is focused on those.

Nostr stands for "Notes and Other Stuff Transmitted by Relays."

### Installing

See instructions for [Build from Source](#building-from-source).

#### With Release Builds

- **Debian**: See the [releases](https://github.com/mikedilger/gossip/releases) area for a file named something like `gossip-VERSION-ARCH.deb`
- **Any Linux**: See the [releases](https://github.com/mikedilger/gossip/releases) area for a file named something like `gossip.VERSION.AppImage`
- **Microsoft Windows**: See the [releases](https://github.com/mikedilger/gossip/releases) area for a file named something like `gossip.VERSION.msi`
- **MacOS**: See the [releases](https://github.com/mikedilger/gossip/releases) area for a file named something like `gossip-VERSION-Darwin-arm64.dmg` or `gossip-VERSION-Darwin-x86_64.dmg`

#### With Package Managers

[![Packaging status](https://repology.org/badge/vertical-allrepos/gossip-nostr.svg)](https://repology.org/project/gossip-nostr/versions)

With `pacman` on **Arch Linux**: [`gossip`](https://aur.archlinux.org/packages/gossip) or [`gossip-git`](https://aur.archlinux.org/packages/gossip-git) or [`gossip-bin`](https://aur.archlinux.org/packages/gossip-bin) on the AUR

With [homebrew](https://brew.sh/) on **MacOS** or **Linux**: `brew install gossip` from [`homebrew-core`](https://github.com/Homebrew/homebrew-core), or for more options `brew install nostorg/nostr/gossip` from [`homebrew-nostr`](https://github.com/nostorg/homebrew-nostr)

With [scoop](https://scoop.sh/) on **Microsoft Windows**: `gossip` from [`scoop-nostr`](https://github.com/nostorg/scoop-nostr)

```
scoop bucket add nostr https://github.com/nostorg/scoop-nostr
scoop install gossip
```

### Points of Difference

The following features make gossip different than most other nostr clients so far:

- **Desktop**: Gossip is designed to run on desktop computers, and runs on Windows, MacOS and Linux.
- **Gossip Model**: The Gossip Model was named after this client, because gossip never used a simple list of relays. From day one it tried to find posts of people that you follow wherever they are most likely to be, based on those people's relay lists as well as half a dozen other heuristics. Today multiple clients use a similar model, focused around ([NIP-65](https://github.com/nostr-protocol/nips/blob/master/65.md)). Gossip connects to all relays necessary to cover everybody you follow, while also trying to listen to the minimum number of relays necessary to do that (considering that there is overlap, and that people generally post to multiple relays). It also dynamically adjusts to relays being down or disconnecting.
- **Secure Key Handling**: Gossip handles private keys as securely as reasonable (short of hardware tokens), keeping them encrypted under a passphrase on disk, requiring that passphrase on startup, and zeroing memory before freeing it. This shouldn't really be a point of difference but few other clients bother.
- **Avoids Browser-Tech**: Gossip avoids web technologies (other than HTTP GET and WebSockets which are necessary for nostr). The web stack is huge, complex, and probably full of undiscovered vulnerabilities, presenting as a huge attack surface. This includes Javascript, the very many and ever-expanding set of web technologies built into browsers and accessible via javascript, and even HTML parsing, rendering, and CSS. This isn't just a pedantic or theoretical concern; people have already had their private key stolen from other nostr clients. We use simple OpenGL-style rendering instead. It's not as pretty but it gets the job done.
- **Performant**: Gossip aims towards being highly performant, using the LMDB database, the rust language, and coding architectures with performance always in mind. Unless you have quite old hardware, the network speed will probably be your bottleneck.
- **High user control**: Gossip has (at the time of writing) 64 different settings. When the right value is uncertain, I pick a reasonable default and give the user the mechanism to change it.
- **Privacy Options**: in case someone wishes to remain secret they should use Gossip over Tor - I recommend using QubesOS do to this. But you could use Whonix or even Tails. Don't just do it on your normal OS, because on a plain OS sometimes data leaks around Tor (things like DNS lookups). Gossip supports using native TLS certificates so you can configure trust for .onion sites. Gossip provides options to support privacy usage such as not loading avatars, not loading images, not necessarily sharing who you follow, etc.

### Screenshots

![Gossip Screenshot, Default Light Theme](assets/gossip_screenshot_light.png)
![Gossip Screenshot, Default Dark Theme](assets/gossip_screenshot_dark.png)

### nostr features supported

✅ = Fully Supported
🟩 = Partly Supported
⬜ = Not Supported (but might be in the future)
⬛ = Not Applicable

- ✅ NIP-01 - Basic protocol flow description
- ✅ NIP-02 - Contact List and Petnames
- ⬜ NIP-03 - OpenTimestamps Attestations for Events
- 🟩 NIP-04 - Encrypted Direct Message (Read Only is implemented)
- ✅ NIP-05 - Mapping Nostr keys to DNS-based internet identifiers
- ⬜ NIP-06 - Basic key derivation from mnemonic seed phrase
- ⬛ NIP-07 - window.nostr capability for web browsers (NOT APPLICABLE)
- ✅ NIP-08 - Handling Mentions
- ✅ NIP-09 - Event Deletion
- ✅ NIP-10 - Conventions for clients' use of e and p tags in text events
- ✅ NIP-11 - Relay Information Document
- ✅ NIP-13 - Proof of Work
- ✅ NIP-14 - Subject tag in text events
- ⬜ NIP-15 - Nostr Marketplace (for resilient marketplaces)
- ✅ NIP-18 - Reposts
- ✅ NIP-19 - bech32-encoded entities
- ✅ NIP-21 - nostr: URL scheme
- 🟩 NIP-23 - Long-form Content (Optional viewing, but not creating)
- 🟩 NIP-24 - Extra metadata fields and tags (Shown in profile, not treated specially)
- ✅ NIP-25 - Reactions
- ✅ NIP-26 - Delegated Event Signing
- ✅ NIP-27 - Text Note References
- ⬜ NIP-28 - Public Chat
- ⬜ NIP-30 - Custom Emoji
- ✅ NIP-31 - Dealing with Unknown Events
- ⬜ NIP-32 - Labeling
- ✅ NIP-36 - Sensitive Content
- ⬜ NIP-38 - User Statuses
- ⬜ NIP-39 - External Identities in Profiles
- ⬜ NIP-40 - Expiration Timestamp
- ✅ NIP-42 - Authentication of clients to relays
- ⬜ NIP-45 - Counting results
- ⬜ NIP-46 - Nostr Connect
- ⬜ NIP-47 - Wallet Connect
- ✅ NIP-48 - Proxy Tags
- ⬜ NIP-50 - Search Capability
- 🟩 NIP-51 - Lists
- ⬜ NIP-52 - Calendar Events
- ⬜ NIP-53 - Live Activities
- ⬜ NIP-56 - Reporting
- 🟩 NIP-57 - Lightning Zaps
- ⬜ NIP-58 - Badges
- ✅ NIP-65 - Relay List Metadata
- ⬜ NIP-72 - Moderated Communities
- ⬜ NIP-75 - Zap Goals
- ⬜ NIP-78 - Application-specific data
- ⬜ NIP-84 - Highlights
- ⬜ NIP-89 - Recommended Application Handlers
- ⬜ NIP-90 - Data Vending Machines
- ⬜ NIP-94 - File Metadata
- ⬜ NIP-98 - HTTP Auth
- ⬜ NIP-99 - Classified Listings

## Content Moderation and Curation

Gossip provides multiple for you to moderate and curate the content that you see. Some of these mechanisms leverage the work of other people such as community moderators, friends, and relay operators. Others put you in charge, but as such you will be seeing the content in order to moderate it so they don't completely insulate you from the content. Here are the mechanisms available in gossip for content moderation and feed curation

1. **No global feed and no algorithm** - Gossip has no global feed. So right from the start you are not subjected to everything that is out there. Feeds are comprised entirely of posts from people that you choose to follow. Replies to posts, on the other hand, can come from anywhere. Therefore there is still a need for moderation.
1. **Lists** - You can define lists of people and view only what those people have posted.
1. **Muting** - You can mute individual people. You can share this mute list with other clients that you use.
1. **Thread Dismissal** - You can dismiss a post and all the replies to it (however, this is temporary until client restart).
1. **Content Warnings** - Gossip shows content warnings of posts that have them, and you must approve to see the content. You can also place content warnings on any content that you post.
1. **Spam filtering** - Gossip provides a hook to filter posts via a script that you can program to do whatever you want.

Showing relay-global feeds is a possibility for the future. You can choose a relay that moderates as you wish.

In the future I intend for gossip to support one of the multiple competing standards for labelling and reporting of content (the options currently are NIP-32, NIP-56, and NIP-72), but none of these are defined well enough to be useful yet IMHO. I look forward to a time when you can subscribe to a set of moderators that you trust.

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
- openssl (debian: "libssl-dev") (this is only needed if not compiling with feature "rustls-tls")
- fontconfig (debian: "libfontconfig1-dev")
- ffmpeg support (debian: libavutil-dev libavformat-dev libavfilter-dev libavdevice-dev libxext-dev libclang-dev)  (this is only needed if compiling with feature "video-ffmpeg")

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

If you want a binary optimized for your exact processor with the newest CPU features enabled, and all gossip features enabled, do something more like this (for exact features to use, see the next section):

````bash
RUSTFLAGS="-C target-cpu=native --cfg tokio_unstable" cargo build --features=lang-cjk,video-ffmpeg --release
````

Everything gossip needs (fonts, icons) is baked into this executable. It doesn't need to find assets. So you can move the "gossip" binary and run it from anywhere.

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

## Compile Features

### TLS

Gossip has three options for TLS support:

1. Use rust-code and compiled in root certificates from webpki  (feature 'rustls-tls')
2. Use rust-code, but use your system's root certificates (feature 'rustls-tls-native', this is the default)
3. Use your system's code and your system's root certificates (feature 'native-tls')

Rust's TLS code is thought to be more secure than your systems TLS code (e.g. OpenSSL). But it is very finnicky. In particular:

- It will not accept self-signed CA certificates. If you have these on your system, it won't run at all.
- Gossip will fail to negotiate SSL with servers that don't have any strong ciphersuites. This is a feature, but not one that everybody wants.
- Gossip may not compile on hardware that the `ring` crypto library does not yet support.

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

If you are having performance issues, please see [docs/PERFORMANCE.md](docs/PERFORMANCE.md).

### Upgrading from very old versions

If you are using a version before 0.8.x, you must upgrade to a 0.8.x version and run gossip at least once in order to upgrade from SQLite3 to LMDB. This is because we have now removed the old SQLite3 code. Alternatively, just delete your old gossip directory in your [config dir](https://docs.rs/dirs/latest/dirs/fn.config_dir.html) and start fresh.

## Technology Involved

- Rust Language
- egui Rust GUI framework
- LMDB
- Tungstenite websocket library
- Tokio async task runtime
- Serde serialization/deserialization
- Speedy serialization/deserialization
- Many others

## License

MIT license ([LICENSE MIT](LICENSE.txt) or <http://opensource.org/licenses/MIT>)

### Contribution

All contributions welcome, please check the [development guidelines](docs/DEVELOPING.md) before starting to code.

Please join [Gossip Telegram Channel](https://t.me/gossipclient).

Anyone interested in replacing the GUI with something much better, or keeping it as egui but making it much better, would be greatly appreciated. The project was split into two crates (lib and bin) to make it easier to build a different UI onto the backend.

Any contribution intentionally submitted for inclusion in the work by you, shall be licensed as above, without any additional terms or conditions.

## On Nostr

### The official gossip account

nprofile1qqsrjerj9rhamu30sjnuudk3zxeh3njl852mssqng7z4up9jfj8yupqpzamhxue69uhhyetvv9ujumn0wd68ytnfdenx7tcpz4mhxue69uhkummnw3ezummcw3ezuer9wchszxmhwden5te0dehhxarj9ekkj6m9v35kcem9wghxxmmd9uq3xamnwvaz7tm0venxx6rpd9hzuur4vghsz8nhwden5te0dehhxarj94c82c3wwajkcmr0wfjx2u3wdejhgtcsfx2xk

npub189j8y280mhezlp98ecmdzydn0r8970g4hpqpx3u9tcztynywfczqqr3tg8

### Mike Dilger

nprofile1qqswuyd9ml6qcxd92h6pleptfrcqucvvjy39vg4wx7mv9wm8kakyujgpzamhxue69uhhyetvv9ujumn0wd68ytnfdenx7tcprpmhxue69uhkzapwdehhxarjwahhy6mn9e3k7mf0qyt8wumn8ghj7etyv4hzumn0wd68ytnvv9hxgtcprdmhxue69uhkummnw3ezumtfddjkg6tvvajhytnrdakj7qgnwaehxw309ahkvenrdpskjm3wwp6kytcpremhxue69uhkummnw3ez6ur4vgh8wetvd3hhyer9wghxuet59uq32amnwvaz7tmwdaehgu3wdau8gu3wv3jhvtct8l34m

npub1acg6thl5psv62405rljzkj8spesceyfz2c32udakc2ak0dmvfeyse9p35c

You can also my NIP-05 address of `mike@mikedilger.com` which will also hook you up with the relays I post to.

I'd prefer if you trusted `mike@mikedilger.com` higher than my public key at this point in time since key management is still pretty bad. That is the inverse of the normal recommendation, but my private key has not been treated very carefully as I never intended it to be my long-term key pair (it just became that over time).  Also, I fully intend to rollover my keys once gossip supports the key-rollover NIP, whatever that is (or will be).

You can tip me at my Bitcoin Lighting address: <decentbun13@walletofsatoshi.com> == lnurl1dp68gurn8ghj7ampd3kx2ar0veekzar0wd5xjtnrdakj7tnhv4kxctttdehhwm30d3h82unvwqhkgetrv4h8gcn4dccnxv563ep
