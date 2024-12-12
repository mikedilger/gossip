# Gossip

## Gossip is a desktop client for NOSTR

Nostr is an open social media protocol empowering lots of software such as this client. The experience is kind of like Twitter except that you control your own account, and you can post to many different independent places called "relays". People are finding many additional uses for NOSTR that go far beyond micro-blogging or chatting, but this client is focused on those.

Nostr stands for "Notes and Other Stuff Transmitted by Relays."

## Installing and Using Gossip

See [Installation and Usage](docs/INSTALLATION_AND_USAGE.md)

## Points of Difference

The following features make gossip different than most other nostr clients so far:

- **Desktop**: Gossip is designed to run on desktop computers, and runs on Windows, MacOS and Linux.
- **Gossip Model**: The Gossip Model was named after this client, because gossip never used a simple list of relays. From day one it tried to find posts of people that you follow wherever they are most likely to be, based on those people's relay lists as well as half a dozen other heuristics. Today multiple clients use a similar model, focused around ([NIP-65](https://github.com/nostr-protocol/nips/blob/master/65.md)). Gossip connects to all relays necessary to cover everybody you follow, while also trying to listen to the minimum number of relays necessary to do that (considering that there is overlap, and that people generally post to multiple relays). It also dynamically adjusts to relays being down or disconnecting.
- **Secure Key Handling**: Gossip handles private keys as securely as reasonable (short of hardware tokens), keeping them encrypted under a passphrase on disk, requiring that passphrase on startup, and zeroing memory before freeing it. This shouldn't really be a point of difference but few other clients bother.
- **Avoids Browser-Tech**: Gossip avoids web technologies (other than HTTP GET and WebSockets which are necessary for nostr). The web stack is huge, complex, and probably full of undiscovered vulnerabilities, presenting as a huge attack surface. This includes Javascript, the very many and ever-expanding set of web technologies built into browsers and accessible via javascript, and even HTML parsing, rendering, and CSS. This isn't just a pedantic or theoretical concern; people have already had their private key stolen from other nostr clients. We use simple OpenGL-style rendering instead. It's not as pretty but it gets the job done.
- **Performant**: Gossip aims towards being highly performant, using the LMDB database, the rust language, and coding architectures with performance always in mind. Unless you have quite old hardware, the network speed will probably be your bottleneck.
- **High user control**: Gossip has (at the time of writing) 64 different settings. When the right value is uncertain, I pick a reasonable default and give the user the mechanism to change it.
- **Privacy Options**: in case someone wishes to remain secret they should use Gossip over Tor - I recommend using QubesOS do to this. But you could use Whonix or even Tails. Don't just do it on your normal OS, because on a plain OS sometimes data leaks around Tor (things like DNS lookups). Gossip supports using native TLS certificates so you can configure trust for .onion sites. Gossip provides options to support privacy usage such as not loading avatars, not loading images, not necessarily sharing who you follow, etc.

## Screenshots

![Gossip Screenshot, Default Light Theme](assets/gossip_screenshot_light.png)
![Gossip Screenshot, Default Dark Theme](assets/gossip_screenshot_dark.png)


## nostr features supported

[NIP Support](docs/NIP_SUPPORT.md)


## Content Moderation and Curation

Gossip provides multiple methods for you to moderate and curate the content that you see. Some of these mechanisms leverage the work of other people such as community moderators, friends, and relay operators. Others put you in charge, but as such you will be seeing the content in order to moderate it so they don't completely insulate you from the content. Here are the mechanisms available in gossip for content moderation and feed curation.

1. **Lists** - You can define lists of people and view only what those people have posted, rather than global content.
1. **Muting** - You can mute individual people. You can share this mute list with other clients that you use.
1. **Thread Dismissal** - You can dismiss a post and all the replies to it (however, this is temporary until client restart).
1. **Content Warnings** - Gossip shows content warnings of posts that have them, and you must approve to see the content. You can also place content warnings on any content that you post.
1. **Spam Filtering Script** - Gossip provides a hook to filter posts via a script that you can program to do whatever you want, and it is very flexible. See [configuration](docs/CONFIGURATION.md) for details.
1. **SpamSafe Relay Designation** - When the SpamSafe setting is enabled, notes from unknown persons are only fetched from relays that you have marked as SpamSafe.
1. **Ephemeral Relay Feeds** - "Global" relay feeds are ephemeral and the content disappears when you quit Gossip. Neither the notes nor the media are saved permanently to your computer.

In the future I intend for gossip to support one of the multiple competing standards for labelling and reporting of content (the options currently are NIP-32, NIP-56, and NIP-72), but none of these are defined well enough to be useful yet IMHO. I look forward to a time when you can subscribe to a set of moderators that you trust.

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

Please join our [chat](https://chachi.chat/groups.0xchat.com/R2yYwhsTcKO2b65i).

Anyone interested in replacing the GUI with something much better, or keeping it as egui but making it much better, would be greatly appreciated. The project was split into two crates (lib and bin) to make it easier to build a different UI onto the backend.

Any contribution intentionally submitted for inclusion in the work by you, shall be licensed as above, without any additional terms or conditions.

## On Nostr

### The official gossip account

nprofile1qqsrjerj9rhamu30sjnuudk3zxeh3njl852mssqng7z4up9jfj8yupqpzamhxue69uhhyetvv9ujumn0wd68ytnfdenx7tcpz4mhxue69uhkummnw3ezummcw3ezuer9wchszxmhwden5te0dehhxarj9ekkj6m9v35kcem9wghxxmmd9uq3xamnwvaz7tm0venxx6rpd9hzuur4vghsz8nhwden5te0dehhxarj94c82c3wwajkcmr0wfjx2u3wdejhgtcsfx2xk

npub189j8y280mhezlp98ecmdzydn0r8970g4hpqpx3u9tcztynywfczqqr3tg8

### Mike Dilger

nprofile1qqswuyd9ml6qcxd92h6pleptfrcqucvvjy39vg4wx7mv9wm8kakyujgpzamhxue69uhhyetvv9ujumn0wd68ytnfdenx7tcprpmhxue69uhkzapwdehhxarjwahhy6mn9e3k7mf0qyt8wumn8ghj7etyv4hzumn0wd68ytnvv9hxgtcprdmhxue69uhkummnw3ezumtfddjkg6tvvajhytnrdakj7qgnwaehxw309ahkvenrdpskjm3wwp6kytcpremhxue69uhkummnw3ez6ur4vgh8wetvd3hhyer9wghxuet59uq32amnwvaz7tmwdaehgu3wdau8gu3wv3jhvtct8l34m

npub1acg6thl5psv62405rljzkj8spesceyfz2c32udakc2ak0dmvfeyse9p35c
hex: ee11a5dff40c19a555f41fe42b48f00e618c91225622ae37b6c2bb67b76c4e49

You can also my NIP-05 address of `mike@mikedilger.com` which will also hook you up with the relays I post to.

I'd prefer if you trusted `mike@mikedilger.com` higher than my public key at this point in time since key management is still pretty bad. That is the inverse of the normal recommendation, but my private key has not been treated very carefully as I never intended it to be my long-term key pair (it just became that over time).  Also, I fully intend to rollover my keys once gossip supports the key-rollover NIP, whatever that is (or will be).

You can tip me at my Bitcoin Lighting address: <decentbun13@walletofsatoshi.com> == lnurl1dp68gurn8ghj7ampd3kx2ar0veekzar0wd5xjtnrdakj7tnhv4kxctttdehhwm30d3h82unvwqhkgetrv4h8gcn4dccnxv563ep
