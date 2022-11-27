# Gossip

Gossip is a desktop client for nostr.

Nostr is "notes and other things transmitted by relays."

This is pre-alpha code. I hope to have the first alpha release before 10 December 2022, but I cannot promise anything.

## Features

- Asychronous design: No busy waiting, no repeatitive polling, easy on your processor.
- Portable to Linux (Debian, RedHat, Arch) MacOS and Windows
- Talks to as few relays as it needs to to keep up with the people you follow, and doesn't overload them with too heavy of requests.
- Look good in light and dark modes.

## nostr features supported

- Reads and displays type=1 (TextNote) events in a feed sorted by time created.
- Processes type=0 (Metadata) events to show user information on events in your feed (name, avatar)
- Lets you subscribe to a person (via public key and relay), but currently requires a restart
- Lets you subscribe to the author, but currently requires a restart
- Settings: feed chunk size, overlap, autofollow, but these don't work right yet.

## nostr features still in development

- [ ] Lets you subscribe to a person via a DNS ID (NIP-35)
- [ ] Lets you import an ID
- [ ] Lets you generate an ID
- [ ] Lets you choose and manage relays you post to
- [ ] Lets you post messages
- [ ] Lets you mute someone in replies
- [ ] Settings: show people you don't follow in replies

## Technology Involved

- Tauri App Framework
- Rust Language
- HTML and Javascript
- VueJS 3.x (Composition API)
- Vue-Router
- Pinia (Javascript global data storage)
- Vite (for development)
- Yarn (for development)
- SQLite 3
- Tungstenite websocket library
- Tokio async task runtime
- Serde serialization/deserialization
- Many others: (dirs, env_logger, futures, lazy_static, log, nostr-proto, rusqlite, serde_json, textnonce, thiserror)

## License

 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, shall be licensed as above, without any additional
terms or conditions.
