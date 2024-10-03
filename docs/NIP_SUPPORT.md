# NIP support

The nostr protocol is a moving target.  This page documents which NIPs gossip supports
as of which git commit.

- ✅ = Fully Supported
- 🟩 = Partly Supported
- ⬜ = Not Supported (but might be in the future)
- 🟫 = No intention to ever support
- ⬛ = Not Applicable

| NIP | Name                                 | Commit   | Support Level | Notes
| --- | ------------------------------------ | -------- | ------------- | -----
| 01  | Basic protocol flow description      | e830a73c | ✅ full       |
| 02  | Follow list                          | e830a73c | ✅ full       | Petname paths are not in use
| 03  | OpenTimestamps Attestations for Even |          | ⬜ none       |
| 04  | Encrypted Direct Message             | e830a73c | ✅ full       | Used only as fallback; See NIP-17
| 05  | Mapping Nostr keys to DNS-based inte | e830a73c | ✅ full       |
| 06  | Basic key derivation from mnemonic s |          | 🟫 none       | We don't need deterministically generated keypairs
| 07  | window.nostr capability for web brow |          | ⬛ n/a        |
| 08  | Handling Mentions                    | e830a73c | ✅ full       | NIP-27 used, but NIP-08 events are understood
| 09  | Event Deletion Request               | e830a73c | ✅ full       | User has option to see deleted events
| 10  | Conventions for clients' use of e an | 67e870d9 | 🟩 behind     | Full support to the marked commit. We genenerate marked but understand positions. We need pubkey on e tags support.
| 11  | Relay Information Document           | e830a73c | 🟩 partial    | Not all fields acted upon. We could use them to help users select relays.
| 13  | Proof of Work                        | e830a73c | ✅ full       | Generates, shows, and uses in spam filters
| 14  | Subject tag in text events           | e830a73c | ✅ full       | Both display and create.
| 15  | Nostr Marketplace (for resilient mar | e830a73c | ⬛ n/a        | Out of scope for gossip
| 17  | Private Direct Messages              | e830a73c | ✅ full       | DMs, DM groups, relay config
| 18  | Reposts                              | e830a73c | ✅ full       |
| 19  | bech32-encoded entities              | e830a73c | ✅ full       |
| 21  | nostr: URI scheme                    | e830a73c | ✅ full       |
| 23  | Long-form Content                    | e830a73c | 🟩 partial    | view as plaintext; no creation
| 24  | Extra metadata fields and tags       | e830a73c | ✅ full       |
| 25  | Reactions                            | e830a73c | 🟩 partial    | posting, showing; no downvotes, no reactions to websites, author not shown, no custom emojis
| 26  | Delegated Event Signing              | e830a73c | ✅ full       |
| 27  | Text Note References                 | e830a73c | ✅ full       |
| 28  | Public Chat                          |          | ⬜ none       |
| 29  | Relay-based Groups                   |          | ⬜ none       |
| 30  | Custom Emoji                         |          | ⬜ none       |
| 31  | Dealing with Unknown Events          | e830a73c | ✅ full       | displays it; doesn't generate custom events
| 32  | Labeling                             |          | ⬜ none       |
| 34  | git stuff                            |          | 🟫 none       |
| 35  | Torrents                             |          | 🟫 none       |
| 36  | Sensitive Content                    | e830a73c | ✅ full       | posting with it, showing it, and hiding content (optionally)
| 38  | User Statuses                        |          | ⬜ none       |
| 39  | External Identities in Profiles      |          | ⬜ none       |
| 40  | Expiration Timestamp                 |          | ⬜ none       |
| 42  | Authentication of clients to relays  | e830a73c | ✅ full       |
| 44  | Versioned Encryption                 | e830a73c | ✅ full       |
| 45  | Counting results                     |          | ⬜ none       |
| 46  | Nostr Connect                        | e830a73c | 🟩 partial    | as signer, not as client
| 47  | Wallet Connect                       |          | 🟫 none       |
| 48  | Proxy Tags                           | e830a73c | ✅ full       | shows the tag and proxy link
| 49  | Private Key Encryption               | e830a73c | ✅ full       |
| 50  | Search Capability                    |          | ⬜ none       |
| 51  | Lists                                |          | 🟩 partial    | Mute, bookmarks, DM relays, and follow sets. But none of the others.
| 52  | Calendar Events                      |          | 🟫 none       |
| 53  | Live Activities                      |          | 🟫 none       |
| 54  | Wiki                                 |          | 🟫 none       |
| 55  | Android Signer Application           |          | ⬛ n/a        |
| 56  | Reporting                            |          | ⬜ none       |
| 57  | Lightning Zaps                       |          | ✅ full       |
| 58  | Badges                               |          | ⬜ none       |
| 59  | Gift Wrap                            |          | ✅ full       |
| 64  | Chess (PGN)                          |          | 🟫 none       |
| 65  | Relay List Metadata                  |          | ✅ full       |
| 70  | Protected Events                     |          | ⬜ none       |
| 71  | Video Events                         |          | ⬜ none       |
| 72  | Moderated Communities                |          | ⬜ none       |
| 73  | External Content IDs                 |          | ⬜ none       |
| 75  | Zap Goals                            |          | ⬜ none       |
| 78  | Application-specific data            |          | ⬜ none       | We will use eventually
| 84  | Highlights                           |          | ⬜ none       |
| 89  | Recommended Application Handlers     |          | ⬜ none       | We will launch links eventually
| 90  | Data Vending Machines                |          | ⬜ none       |
| 92  | Media Attachments                    |          | ⬜ none       |
| 94  | File Metadata                        |          | ⬜ none       |
| 96  | HTTP File Storage Integration        |          | ⬜ none       |
| 98  | HTTP Auth                            |          | ⬜ none       |
| 99  | Classified Listings                  |          | ⬜ none       |
