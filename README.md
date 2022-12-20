# Gossip

Gossip is a desktop client for nostr.

Nostr is "Notes and Other Stuff Transmitted by Relays."

This is pre-alpha code. It is not ready for use.

NOTE: After two false starts (tauri, gtk4) I'm moving to egui, which should be much easier
and faster to develop.

## Features

- Asychronous design: No busy waiting or polling.
- Portable. The UI will run on anything that runs one of these backends: OpenGL (glium, glow), OpenGL ES (glow, wgpu), WebGL (glow), Vulkan (wgpu), Metal (wgpu), DirectX 11/12 (wgpu), Browsers (WebAssembly). And rust runs very many places.
- Talks to as few relays as it needs to to keep up with the people you follow, and doesn't overload them with too heavy of requests.

## nostr features supported

This section will need updating.
- Reads and displays type=1 (TextNote) events in a feed sorted by time created.
    - Shows replies under the message they reply to
    - Shows deleted events struck out with red deleted marker.
- Shows people you subscribe to in the Person tab
    - Processes type=0 (Metadata) events to show user information on events in your feed (name, avatar)
    - Lets you subscribe to a person (via public key and relay), but currently requires a restart
- Identity:
    - Lets you generate an ID (private key) and stores that key encrypted under a passphrase, zeroing memory when finished with it where it can.
    - Lets you import an ID (private key)
- Settings: feed chunk size, overlap, autofollow, but these don't work right yet.

## nostr features planned for first alpha release

- [ ] Create your identity
- [ ] Follow people and see their feed posts in time order
- [ ] See your feed in threaded mode
- [ ] Choose and manage relays to post to
- [ ] Post messages and reactions
- [ ] Show reactions

## nostr features planned for subsequent releases

- [ ] Lets you subscribe to a person via a DNS ID (NIP-35)
- [ ] Validates users via NIP-05
- [ ] Lets you react to other people's posts
- [ ] Lets you show events from people you don't follow if they reply to a post you do
- [ ] Lets you mute someone
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
