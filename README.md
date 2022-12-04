# Gossip

Gossip is a desktop client for nostr.

Nostr is "Notes and Other Stuff Transmitted by Relays."

This is pre-alpha code. It is not ready for use.

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

## nostr features planned for first alpha release

- [ ] Lets you import an ID (private key)
- [ ] Lets you generate an ID (key pair)
- [ ] Lets you choose and manage relays you post to
- [ ] Lets you post messages
- [ ] Shows replies under the message they reply to
- [ ] Shows reactions (but maybe not yet react yourself)
- [ ] Lets you 'load more' older posts if you want to look back further
- [ ] Handle delete events somehow (either delete or grey out)

## nostr features planned for subsequent releases

- [ ] Lets you subscribe to a person via a DNS ID (NIP-35)
- [ ] Validates users via NIP-05
- [ ] Lets you react to other people's posts
- [ ] Lets you show events from people you don't follow if they reply to a post you do
- [ ] Lets you mute someone
- [ ] More secure private key handling
- [ ] Lets you rank relays
- [ ] Shows links as links
- [ ] Shows images inline (option to wait for your approval)
- [ ] Include a 'client' tag
- [ ] Show the 'client' tag of posts
- [ ] Support "content-warning"
- [ ] Allow browsing of relay-global events of people you dont follow
- [ ] Multiple identities
- [ ] Publish your following list
- [ ] Follow someone privately (without including in your posted following list)
- [ ] Allow viewing of other people's following lists w/ petnames
- [ ] Dismiss a message for this session only w/o deleting it

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
