# Gossip

## Gossip is a desktop client for nostr.

Nostr is an open social media protocol empowering lots of software such as this client. The experience is kind of like Twitter except that you control your own account, and you can post to many different independent places called "relays". People are finding many additional uses for nostr that go far beyond micro-blogging or chatting, but this client is focused on those.

Nostr stands for "Notes and Other Stuff Transmitted by Relays."

### Installing

- **ArchLinux**: https://aur.archlinux.org/packages/gossip or https://aur.archlinux.org/packages/gossip-git
- **Debian**: See the [releases](https://github.com/mikedilger/gossip/releases) area for a file named something like `gossip-VERSION-ARCH.deb.zip`
- **Microsoft Windows**: See the [releases](https://github.com/mikedilger/gossip/releases) area for a file named something like `gossip-VERSION.msi.zip`

or choose to [Build from Source](#building-from-source)

### Points of Difference

The following features make gossip different than most other nostr clients so far:

- Gossip follows people at they relays they profess to post to. That means it has to discover which relays those are (see [https://github.com/nostr-protocol/nips/blob/master/65.md](NIP-65)) and make smart relay selection choices based on things like which relays cover the most people you follow.
- Gossip handles private keys as securely as reasonable (short of hardware tokens), keeping them encrypted under a passphrase on disk, requiring that passphrase on startup, and zeroing memory.
- Gossip avoids web technologies (other than HTTP GET and WebSockets). Web technologies like HTML parsing and rendering, CSS, JavaScript and the very many web standards, are complex and represent a security hazard due to such a large attack surface. This isn't just a pedantic or theoretical concern; people have already had their private key stolen from other nostr clients. We use simple OpenGL-style rendering instead. It's not as pretty but it gets the job done.

### Status

Gossip is ready to use as a daily client if you wish. There are shortcomings, and active [development](DEVELOPING.md) is ongoing.

## Media

### Screenshot

![Gossip Screenshot, Default Light Theme](assets/gossip_screenshot_light.png)
![Gossip Screenshot, Default Dark Theme](assets/gossip_screenshot_dark.png)

### Videos

[First Gossip Video, 5 Jan 2023](https://mikedilger.com/gossip1.mp4)

[Gossip Relay Model, 29 Jan 2023](https://mikedilger.com/gossip-relay-model.mp4)

## Development Ideology

- **High user control**: The plan is for the user to be in control of quite a lot of settings regarding which posts they see, which relays to talk to, and when to fetch from them, but with some sane defaults.
- **Key Security**: Private keys need to be handled as securely as possible. We store the key encrypted under a passphrase on disk, and we zero out any memory that has seen either the key or the passphrase that decrypts it. We also keep the decrypted key in just one place, the Signer, which doesn't provide access to the key directly. Eventually we will look to add hardware token support, probably first using programmable [Solo keys](https://solokeys.com/) because I have a few of those.
- **Portable** design intended for the **desktop**: This is intended to run on desktop computers, but not limited as such. The platform must be supported by rust (most are), and SQLite3 needs to store its file somewhere. The UI will run on many backends.
- **High-enough performance**: Generally the network speed should be your limiting factor on performance, not the UI or any other part of the code. It doesn't matter too much how fast the code runs as long as it is always faster than the network, and I think that's definitely true for gossip.
- **Easy-ish on CPU/power usage**: We can't achieve this as well as other clients might because we use an immediate-mode renderer which necessarily recomputes what it draws every "frame" and may redraw many times per second. We are working hard to minimize the CPU impact of this hot loop. Try it and see.
- **Privacy Options**: in case someone wishes to remain secret they should use Gossip over Tor - I recommend using QubesOS do to this. But you could use Whonix or even Tails. Don't just do it on your normal OS which won't do Tor completely. Gossip provides options to support privacy usage such as not loading avatars, not necessarily sharing who you follow, etc. We will be adding more privacy features.

### nostr features supported

- [x] NIP-01 - Basic protocol flow description
- [x] NIP-02 - Contact List and Petnames
- [ ] NIP-03 - OpenTimestamps Attestations for Events [NOT PLANNED]
- [ ] NIP-04 - Encrypted Direct Message [PARTIAL]
- [x] NIP-05 - Mapping Nostr keys to DNS-based internet identifiers
- [ ] NIP-06 - Basic key derivation from mnemonic seed phrase
- [ ] NIP-07 - window.nostr capability for web browsers [NOT APPLICABLE]
- [x] NIP-08 - Handling Mentions
- [ ] NIP-09 - Event Deletion [PARTIAL]
- [x] NIP-10 - Conventions for clients' use of e and p tags in text events
- [x] NIP-11 - Relay Information Document
- [x] NIP-12 - Generic Tag Queries
- [x] NIP-13 - Proof of Work
- [x] NIP-14 - Subject tag in text events
- [x] NIP-15 - End of Stored Events Notice
- [x] NIP-16 - Event Treatment
- [x] NIP-19 - bech32-encoded entities
- [x] NIP-20 - Command Results
- [ ] NIP-21 - nostr: URL scheme
- [x] NIP-22 - Event created_at Limits
- [ ] NIP-23 - Long-form Content
- [x] NIP-25 - Reactions
- [x] NIP-26 - Delegated Event Signing
- [ ] NIP-28 - Public Chat
- [ ] NIP-33 - Parameterized Replaceable Events
- [ ] NIP-36 - Sensitive Content
- [ ] NIP-40 - Expiration Timestamp
- [x] NIP-42 - Authentication of clients to relays
- [ ] NIP-46 - Nostr Connect
- [ ] NIP-50 - Keywords filter
- [ ] NIP-56 - Reporting
- [ ] NIP-58 - Badges
- [x] NIP-65 - Relay List Metadata
- [ ] NIP-78 - Application-specific data

## Building from Source

### Step 0 - Possible Reset of Master Branch

If when you pull gossip it doesn't pull cleanly, I may have done a rare force-push. Run these commands to reset your master branch:

````bash
$ git fetch
$ git reset --hard origin/master
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

This binary should be portable to similar systems with similar hardware and operating system.

If you want a binary optimized for your exact processor with the newest features enabled:

````bash
$ RUSTFLAGS="-C target-cpu=native --cfg tokio_unstable" cargo build --release
````

Everything gossip needs (fonts, icons) is baked into this executable. It doesn't need to find assets. So you can move it and run it from anywhere.

To make the binary smaller,

````bash
$ strip gossip
````

### Step 5 - Do it all again

The `master` branch changes quickly.  When you want to update, do it all again, something like this:

````bash
$ git pull
$ cargo build --release
$ strip ./target/release/gossip
$ ./target/release/gossip
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

### Known Issues

#### Sqlite Constraint Issues (Foreign or Unique Key)

First you need to locate your database file. The gossip directory is under this path: https://docs.rs/dirs/4.0.0/dirs/fn.data_dir.html  The database file is `gossip.sqlite`.  Then you need to install `sqlite3` on your system.

Using `sqlite3` on your database file, the following kind of SQL can help you identify rows that violate foreign key constraints.

##### Error: Sql(SqliteFailure(Error { code: ConstraintViolation, extended_code: 2067 }, Some("UNIQUE constraint failed: person_relay.person, person_relay.relay")))

You can find which rows are duplicated using: `select a.person, a.relay FROM person_relay a INNER JOIN person_relay b WHERE a.person=b.person AND a.relay=b.relay AND a.rowid!=b.rowid;`  You'll need to delete one row from each pair (by rowid so you don't delete both of them).

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

All contributions welcome, please check the [development guidelines](DEVELOPING.md) before starting to code.

Anyone interested in replacing the GUI with something much better, or keeping it as egui but making it much better, would be greatly appreciated.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, shall be licensed as above, without any additional terms or conditions.

## On Nostr

### The official gossip account:

nprofile1qqsrjerj9rhamu30sjnuudk3zxeh3njl852mssqng7z4up9jfj8yupqpzamhxue69uhhyetvv9ujumn0wd68ytnfdenx7tcpz4mhxue69uhkummnw3ezummcw3ezuer9wchszxmhwden5te0dehhxarj9ekkj6m9v35kcem9wghxxmmd9uq3xamnwvaz7tm0venxx6rpd9hzuur4vghsz8nhwden5te0dehhxarj94c82c3wwajkcmr0wfjx2u3wdejhgtcsfx2xk

npub189j8y280mhezlp98ecmdzydn0r8970g4hpqpx3u9tcztynywfczqqr3tg8

### Mike Dilger:

nprofile1qqswuyd9ml6qcxd92h6pleptfrcqucvvjy39vg4wx7mv9wm8kakyujgpzamhxue69uhhyetvv9ujumn0wd68ytnfdenx7tcprpmhxue69uhkzapwdehhxarjwahhy6mn9e3k7mf0qyt8wumn8ghj7etyv4hzumn0wd68ytnvv9hxgtcprdmhxue69uhkummnw3ezumtfddjkg6tvvajhytnrdakj7qgnwaehxw309ahkvenrdpskjm3wwp6kytcpremhxue69uhkummnw3ez6ur4vgh8wetvd3hhyer9wghxuet59uq32amnwvaz7tmwdaehgu3wdau8gu3wv3jhvtct8l34m

npub1acg6thl5psv62405rljzkj8spesceyfz2c32udakc2ak0dmvfeyse9p35c

You can also my NIP-05 address of `mike@mikedilger.com` which will also hook you up with the relays I post to.

I'd prefer if you trusted `mike@mikedilger.com` higher than my public key at this point in time since key management is still pretty bad. That is the inverse of the normal recommendation, but my private key has not been treated very carefully as I never intended it to be my long-term keypair (it just became that over time).  Also, I fully intend to rollover my keys once gossip supports the key-rollover NIP, whatever that is (or will be).

You can tip me at my Bitcoin Lighting address: decentbun13@walletofsatoshi.com == lnurl1dp68gurn8ghj7ampd3kx2ar0veekzar0wd5xjtnrdakj7tnhv4kxctttdehhwm30d3h82unvwqhkgetrv4h8gcn4dccnxv563ep

Anything more than 500,000 sats or so should probably go through my on-chain address: bc1qx2a4qmuczvmdcqr8wauty66gkh2klywckd5wn8
