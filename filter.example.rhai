// This is a sample spam filtering script for the gossip nostr
// client. The language is called Rhai, details are at:
// https://rhai.rs/book/
//
// For gossip to find your spam filtering script, put it in your
// gossip profile directory. See
//    https://docs.rs/dirs/latest/dirs/fn.data_dir.html
// to find the base directory. A subdirectory "gossip" is your
// gossip data directory which for most people is their profile
// directory too. (Note: if you use a GOSSIP_PROFILE, you'll need
// to put it one directory deeper into that profile directory).
//
// This filter is used to filter out and refuse to process incoming
// events as they flow in from relays, and also to filter which
// events get displayed in certain circumstances. It is only run on
// feed-displayable event kinds, and only events by authors you are
// not following. In case of error, nothing is filtered.
//
// You must define a function called 'filter' which returns one of
// these constant values:
//
//   DENY (the event is filtered out)
//   ALLOW (the event is allowed through)
//   MUTE (the event is filtered out, and the author is
//         automatically muted)
//
// Your script will be provided the following:
//
//   caller        - a string that is one of "Process", "Thread",
//                   "Inbox" or "Global" indicating which part of
//                   the code is running your script
//   id            - the event ID, as a hex string
//   pubkey        - the event author public key, as a hex string
//   kind          - the event kind as an integer
//   tags          - the event tags (array of array of strings)
//   content       - the event content as a string
//   muted         - if the author is in your mute list
//   name          - if we have it, the name of the author (or your
//                   petname), else an empty string
//   nip05valid    - whether nip05 is valid for the author, as a
//                   boolean
//   pow           - the Proof of Work on the event
//   seconds_known - the number of seconds that the author of the
//                   event has been known to gossip
//   spamsafe      - true only if the event came in from a relay
//                   marked as SpamSafe during Process (even if the
//                   global setting for SpamSafe is off)
//
// Here is some notes on the language and syntax:
//
// * Functions are pure. Call them with fn!() syntax to propagate
//   scope (global variables explained above) into the function.
//
// * '()' is nil.
//
// * The ?? operator coalesces values. If the first value is nil
//   it moves on to the next one.

// This is the top level main function.  It gets all the scope
// explained in the comments above.
fn filter() {
    filter_medium!()
}

fn filter_mild() {
    allow_global!() ??
        filter_known_spam!() ??
        ALLOW
}

fn filter_medium() {
    allow_global!() ??
        filter_known_spam!() ??
        reject_new_pubkeys!() ??
        ALLOW
}

fn filter_strong() {
    allow_global!() ??
        filter_known_spam!() ??
        reject_new_pubkeys!() ??
        allow_proven!() ??
        DENY
}

// ---------------------------------------------------------

// Show spam on global
//
// Global events and media are ephemeral, and people usually want
// to see everything there.
fn allow_global() {
    if caller=="Global" {
        return ALLOW;
    }

    // always return () if you don't have an answer
    ()
}

fn filter_known_spam() {
    // Block ReplyGuy
    if name.contains("ReplyGuy") || name.contains("ReplyGal") {
        return DENY;
    }

    // NOTE: This works because giftwraps are unwrapped before the
    // content is passed to this script
    if content.to_lower().contains(
        "Mr. Gift and Mrs. Wrap under the tree, KISSING!")
    {
        return DENY;
    }

    // always return () if you don't have an answer
    ()
}

// Reject events from pubkeys we have not seen before
// unless they have a high PoW.
//
// NOTE: If this turns out to be a legit person, we will
// start hearing their events 2 seconds from now, probably
// starting with their second event.
fn reject_new_pubkeys() {
    if seconds_known <= 2 && pow < 25 {
        return DENY;
    }

    // always return () if you don't have an answer
    ()
}

// Mute people that use offensive words
// (Mike Dilger does not recommend, but this is a good example)
fn mute_offensive_people() {
    if content.to_lower().contains(" kike") ||
        content.to_lower().contains("kike ") ||
        content.to_lower().contains(" nigger") ||
        content.to_lower().contains("nigger ")
    {
        return MUTE;
    }

    // always return () if you don't have an answer
    ()
}

// Allow events if proven to be good somehow
fn allow_proven() {
    // Accept if the PoW is large enough
    if pow >= 25 {
        return ALLOW;
    }

    // Accept if their NIP-05 is valid
    if nip05valid {
        return ALLOW;
    }

    // Accept if the event came through a spamsafe relay
    if spamsafe {
        return ALLOW;
    }

    // always return () if you don't have an answer
    ()
}
