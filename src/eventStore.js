import { defineStore } from 'pinia'

export const useEventStore = defineStore('events', {
    state: () => ({
        // The event_map maps event IDs to Event structures like this
        //    { id, pubkey, created_at, kind, tags, content }
        //    These are nostr-proto/Event types
        events: new Map(),

        // The metadata map maps event IDs onto Event metdata like this
        //    {
        //      id,
        //      replies: [ id, id, ... ],
        //      reactions: { upvotes, downvotes, emojis [ 😀: 2 ] }
        //    }
        metadata: new Map(),

        // The feed is a list of event IDs to be rendered in REVERSE order
        feed: [],

        // People is a map from pubkey to Person data like this
        //    { pubkey, name, about, picture,
        //      dns_id, dns_id_valid, dns_id_last_checked, followed }
        //    These are gossip::db::DbPerson types
        people: new Map(),
    }),
})
