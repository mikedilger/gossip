# NIP support

This lists which [NIPs](https://github.com/nostr-protocol/nips) gossip supports.

Because NIPs change, full support cannot be guaranteed.

- ✅ = Fully Supported (at some version of the NIP)
- 🟩 = Partly Supported
- ⬜ = Not Supported (but might be in the future)
- 🟫 = No intention to ever support
- ⬛ = Not Applicable

| NIP | Name                                 | Release  | Support Level | Notes
| --- | ------------------------------------ | -------- | ------------- | -----
| 01  | Basic protocol flow description      | 0.4      | ✅ full       |
| 02  | Follow list                          | 0.4      | ✅ full       | Petname paths are not in use
| 03  | OpenTimestamps Attestations for Even |          | ⬜ none       |
| 04  | Encrypted Direct Message             | 0.8      | ✅ full       | Used only as fallback; See NIP-17
| 05  | Mapping Nostr keys to DNS-based inte | 0.4      | ✅ full       |
| 06  | Basic key derivation from mnemonic s |          | 🟫 none       | We don't need deterministically generated keypairs
| 07  | window.nostr capability for web brow |          | ⬛ n/a        |
| 08  | Handling Mentions                    | 0.4      | ✅ full       | NIP-27 used, but NIP-08 events are understood
| 09  | Event Deletion Request               | 0.6      | ✅ full       | User has option to see deleted events
| 10  | Conventions for clients' use of e an | 0.6      | ✅ full       |
| 11  | Relay Information Document           | 0.4      | 🟩 partial    | Not all fields acted upon. We could use them to help users select relays.
| 13  | Proof of Work                        | 0.4      | ✅ full       | Generates, shows, and uses in spam filters
| 14  | Subject tag in text events           | 0.4      | ✅ full       | Both display and create.
| 15  | Nostr Marketplace (for resilient mar |          | ⬛ n/a        | Out of scope for gossip
| 17  | Private Direct Messages              | 0.11     | ✅ full       | DMs, DM groups, relay config
| 18  | Reposts                              | 0.9      | ✅ full       |
| 19  | bech32-encoded entities              | 0.4      | ✅ full       |
| 21  | nostr: URI scheme                    | 0.6      | ✅ full       |
| 22  | Comment                              | 0.13     | 🟩 partial    | Rendered/indexed, but not created
| 23  | Long-form Content                    | 0.6      | 🟩 partial    | view as plaintext; no creation
| 24  | Extra metadata fields and tags       | 0.4      | ✅ full       |
| 25  | Reactions                            | 0.4      | 🟩 partial    | posting, showing; no downvotes, no reactions to websites, author not shown, no custom emojis
| 26  | Delegated Event Signing              | 0.5      | ✅ full       |
| 27  | Text Note References                 | 0.6      | ✅ full       |
| 28  | Public Chat                          |          | ⬜ none       |
| 29  | Relay-based Groups                   |          | ⬜ none       |
| 30  | Custom Emoji                         |          | ⬜ none       |
| 31  | Dealing with Unknown Events          | 0.8      | ✅ full       | displays it; doesn't generate custom events
| 32  | Labeling                             |          | ⬜ none       |
| 34  | git stuff                            |          | 🟫 none       |
| 35  | Torrents                             |          | 🟫 none       |
| 36  | Sensitive Content                    | 0.4      | ✅ full       | posting with it, showing it, and hiding content (optionally)
| 37  | Draft Events                         |          | ⬜ none       |
| 38  | User Statuses                        |          | ⬜ none       |
| 39  | External Identities in Profiles      |          | ⬜ none       |
| 40  | Expiration Timestamp                 |          | ⬜ none       |
| 42  | Authentication of clients to relays  | 0.4      | ✅ full       |
| 44  | Versioned Encryption                 | 0.11     | ✅ full       |
| 45  | Counting results                     |          | ⬜ none       |
| 46  | Nostr Connect                        | 0.10     | 🟩 partial    | as signer, not as client
| 47  | Wallet Connect                       |          | 🟫 none       |
| 48  | Proxy Tags                           | 0.8      | ✅ full       | shows the tag and proxy link
| 49  | Private Key Encryption               | 0.4      | ✅ full       |
| 50  | Search Capability                    | 0.13     | ✅ full       | local or at your configured search relays
| 51  | Lists                                | 0.9      | 🟩 partial    | Mute, bookmarks, DM relays, and follow sets. But none of the others.
| 52  | Calendar Events                      |          | 🟫 none       |
| 53  | Live Activities                      |          | 🟫 none       |
| 54  | Wiki                                 |          | 🟫 none       |
| 55  | Android Signer Application           |          | ⬛ n/a        |
| 56  | Reporting                            |          | ⬜ none       |
| 57  | Lightning Zaps                       | 0.8      | ✅ full       |
| 58  | Badges                               |          | ⬜ none       |
| 59  | Gift Wrap                            | 0.11     | ✅ full       |
| 60  | Cashu Wallet                         |          | ⬜ none       |
| 61  | Nutzaps                              |          | ⬜ none       |
| 64  | Chess (PGN)                          |          | 🟫 none       |
| 65  | Relay List Metadata                  | 0.4      | ✅ full       |
| 68  | Picture-first feeds                  |          | ⬜ none       |
| 69  | Peer-to-peer Order events            |          | ⬜ none       |
| 70  | Protected Events                     |          | ⬜ none       |
| 71  | Video Events                         |          | ⬜ none       |
| 72  | Moderated Communities                |          | ⬜ none       |
| 73  | External Content IDs                 |          | ⬜ none       |
| 75  | Zap Goals                            |          | ⬜ none       |
| 78  | Application-specific data            |          | ⬜ none       | We will use eventually
| 7D  | Threads                              |          | ⬜ none       |
| 84  | Highlights                           |          | ⬜ none       |
| 86  | Relay Management API                 |          | ⬛ n/a        |
| 89  | Recommended Application Handlers     | 0.13     | 🟩 partial    | We can only launch web handlers
| 90  | Data Vending Machines                |          | ⬜ none       |
| 92  | Media Attachments                    |          | 🟩 partial    | We use many NIP-94 fields
| 94  | File Metadata                        |          | ⬜ none       |
| 96  | HTTP File Storage Integration        |          | ⬜ none       |
| 98  | HTTP Auth                            |          | ⬜ none       |
| 99  | Classified Listings                  |          | ⬜ none       |
| C7  | Chats                                |          | ⬜ none       |

# BUD support

This list which [BUDs](https://github.com/hzrd149/blossom) gossip supports.

- ✅ = Fully Supported
- 🟩 = Partly Supported
- ⬜ = Not Supported (but might be in the future)
- 🟫 = No intention to ever support
- ⬛ = Not Applicable


| BUD | Name                                 | Release  | Support Level | Notes
| --- | ------------------------------------ | -------- | ------------- | -----
| 01  | Server requrements and blob retrieval| 0.13     | ✅ full       |
| 02  | Blob upload and management           | 0.13     | 🟩 partial    | we only PUT
| 03  | User Server List                     | 0.13     | ✅ full       |
| 04  | Mirroring blogs                      |          | ⬜ none       |
| 05  | Media optimization                   |          | ⬜ none       |
| 06  | Upload requirements                  |          | ⬜ none       |
| 08  | Nostr File Metadata Tags             |          | ⬜ none       |
